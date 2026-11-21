//! # ids-000-aa Contract: Transport
//!
//! `trait Transport` — framing, sequencing, acknowledgements, delivery semantics.
//! Carrier-agnostic: operates on opaque byte frames, no IP/TCP/UDP/QUIC references.

use crate::types::{
    CarrierType, DeliverySemantics, Endpoint, OpRef, OrderingPos,
};
use crate::errors::TransportError;

///////////////////////////////////////////////////////////////////////////////
// SEALED TRAIT: Transport
///////////////////////////////////////////////////////////////////////////////

/// Transport layer for framing, sequencing, acks, and delivery semantics.
/// Sealed: only ids-000-aa and designated impl crates can implement.
pub trait Transport: private::Sealed {
    /// Frame a raw payload into a transport frame with headers.
    fn frame(&self, payload: Vec<u8>) -> Result<Vec<u8>, TransportError>;

    /// Deframe a raw transport frame back into a payload.
    fn deframe(&self, bytes: Vec<u8>) -> Result<Vec<u8>, TransportError>;

    /// Assign a sequence number to a frame.
    fn sequence(&self, frame_id: u64) -> Result<(), TransportError>;

    /// Acknowledge a received sequence number.
    fn acknowledge(&self, sequence_id: u64) -> Result<(), TransportError>;

    /// Get the delivery semantics this transport provides.
    fn delivery_semantics(&self) -> DeliverySemantics;

    /// Get the maximum frame payload size supported.
    fn max_frame_size(&self) -> usize {
        65535
    }

    /// Get the frame header overhead in bytes.
    fn frame_header_overhead(&self) -> usize {
        16
    }

    /// Check if the transport requires sequencing.
    fn requires_sequencing(&self) -> bool {
        true
    }

    /// Check if the transport requires acknowledgements.
    fn requires_acknowledgements(&self) -> bool {
        true
    }

    /// Get the default timeout for awaiting an acknowledgement.
    fn default_ack_timeout_ms(&self) -> u64 {
        5000
    }

    /// Get the maximum number of retransmission attempts.
    fn max_retransmissions(&self) -> u32 {
        3
    }

    /// Check if a frame is valid according to transport rules.
    fn is_frame_valid(&self, frame: &[u8]) -> bool {
        frame.len() >= self.frame_header_overhead()
    }

    /// Get the transport's name for debugging.
    fn transport_name(&self) -> &'static str {
        "transport"
    }

    /// Get the number of bytes framed by this transport.
    fn bytes_framed(&self) -> u64 {
        0
    }

    /// Get the number of bytes deframed by this transport.
    fn bytes_deframed(&self) -> u64 {
        0
    }

    /// Get the number of frames sequenced.
    fn frames_sequenced(&self) -> u64 {
        0
    }

    /// Get the number of frames acknowledged.
    fn frames_acknowledged(&self) -> u64 {
        0
    }

    /// Get the number of frames pending acknowledgement.
    fn frames_pending_ack(&self) -> u64 {
        0
    }

    /// Reset the transport state, clearing all sequencing and ack tracking.
    fn reset(&self) -> Result<(), TransportError> {
        Ok(())
    }

    /// Flush any pending frames to the underlying carrier.
    fn flush(&self) -> Result<(), TransportError> {
        Ok(())
    }

    /// Check if the transport is currently connected.
    fn is_connected(&self) -> bool {
        true
    }

    /// Get the transport's priority for routing decisions.
    fn priority(&self) -> u32 {
        0
    }

    /// Get the transport's reliability level (0 = best effort, 10 = guaranteed).
    fn reliability_level(&self) -> u32 {
        5
    }
}

///////////////////////////////////////////////////////////////////////////////
// SEALED SUPERTRAIT MODULE
///////////////////////////////////////////////////////////////////////////////

mod private {
    /// Sealed supertrait — prevents downstream crates from implementing Transport.
    pub trait Sealed {}
    impl Sealed for crate::types::DeliverySemantics {}
}

///////////////////////////////////////////////////////////////////////////////
// ERROR TYPE: TransportError
///////////////////////////////////////////////////////////////////////////////

/// Errors returned by transport operations.
#[derive(Debug, thiserror::Error)]
pub enum TransportError {
    #[error("frame encoding failed: {reason}")]
    FrameEncodeFailed { reason: String },

    #[error("frame decoding failed: {reason}")]
    FrameDecodeFailed { reason: String },

    #[error("invalid frame: {reason}")]
    InvalidFrame { reason: String },

    #[error("sequence number out of range: expected {expected}, got {got}")]
    SequenceOutOfRange { expected: u64, got: u64 },

    #[error("duplicate sequence number: {seq}")]
    DuplicateSequence { seq: u64 },

    #[error("acknowledgement failed for sequence {seq}: {reason}")]
    AckFailed { seq: u64, reason: String },

    #[error("timeout waiting for acknowledgement of sequence {seq}")]
    AckTimeout { seq: u64 },

    #[error("max retransmissions exceeded for sequence {seq}")]
    MaxRetransmissionsExceeded { seq: u64 },

    #[error("frame too large: {size} bytes (max {max})")]
    FrameTooLarge { size: usize, max: usize },

    #[error("transport not connected")]
    NotConnected,

    #[error("transport already connected")]
    AlreadyConnected,

    #[error("flush failed: {reason}")]
    FlushFailed { reason: String },

    #[error("reset failed: {reason}")]
    ResetFailed { reason: String },

    #[error("delivery semantics mismatch: expected {expected:?}, got {got:?}")]
    DeliverySemanticsMismatch {
        expected: DeliverySemantics,
        got: DeliverySemantics,
    },

    #[error("transport internal error: {message}")]
    Internal { message: String },
}

impl TransportError {
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            TransportError::FrameEncodeFailed { .. }
                | TransportError::FrameDecodeFailed { .. }
                | TransportError::AckTimeout { .. }
                | TransportError::MaxRetransmissionsExceeded { .. }
                | TransportError::Internal { .. }
        )
    }

    pub fn is_fatal(&self) -> bool {
        !self.is_retryable()
    }

    pub fn category(&self) -> &'static str {
        match self {
            TransportError::FrameEncodeFailed { .. } => "frame_encode_failed",
            TransportError::FrameDecodeFailed { .. } => "frame_decode_failed",
            TransportError::InvalidFrame { .. } => "invalid_frame",
            TransportError::SequenceOutOfRange { .. } => "sequence_out_of_range",
            TransportError::DuplicateSequence { .. } => "duplicate_sequence",
            TransportError::AckFailed { .. } => "ack_failed",
            TransportError::AckTimeout { .. } => "ack_timeout",
            TransportError::MaxRetransmissionsExceeded { .. } => "max_retransmissions",
            TransportError::FrameTooLarge { .. } => "frame_too_large",
            TransportError::NotConnected => "not_connected",
            TransportError::AlreadyConnected => "already_connected",
            TransportError::FlushFailed { .. } => "flush_failed",
            TransportError::ResetFailed { .. } => "reset_failed",
            TransportError::DeliverySemanticsMismatch { .. } => "delivery_semantics_mismatch",
            TransportError::Internal { .. } => "internal",
        }
    }

    pub fn plain_message(&self) -> String {
        match self {
            TransportError::FrameEncodeFailed { reason } => {
                format!("frame encoding failed: {}", reason)
            }
            TransportError::FrameDecodeFailed { reason } => {
                format!("frame decoding failed: {}", reason)
            }
            TransportError::InvalidFrame { reason } => {
                format!("invalid frame: {}", reason)
            }
            TransportError::SequenceOutOfRange { expected, got } => {
                format!("sequence out of range: expected {}, got {}", expected, got)
            }
            TransportError::DuplicateSequence { seq } => {
                format!("duplicate sequence number: {}", seq)
            }
            TransportError::AckFailed { seq, reason } => {
                format!("acknowledgement failed for sequence {}: {}", seq, reason)
            }
            TransportError::AckTimeout { seq } => {
                format!("timeout waiting for acknowledgement of sequence {}", seq)
            }
            TransportError::MaxRetransmissionsExceeded { seq } => {
                format!("max retransmissions exceeded for sequence {}", seq)
            }
            TransportError::FrameTooLarge { size, max } => {
                format!("frame too large: {} bytes (max {})", size, max)
            }
            TransportError::NotConnected => "transport not connected".to_string(),
            TransportError::AlreadyConnected => "transport already connected".to_string(),
            TransportError::FlushFailed { reason } => {
                format!("flush failed: {}", reason)
            }
            TransportError::ResetFailed { reason } => {
                format!("reset failed: {}", reason)
            }
            TransportError::DeliverySemanticsMismatch { expected, got } => {
                format!(
                    "delivery semantics mismatch: expected {:?}, got {:?}",
                    expected, got
                )
            }
            TransportError::Internal { message } => {
                format!("transport internal error: {}", message)
            }
        }
    }
}

///////////////////////////////////////////////////////////////////////////////
// FRAME HEADER
///////////////////////////////////////////////////////////////////////////////

/// Transport frame header metadata.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FrameHeader {
    pub sequence: u64,
    pub flags: u8,
    pub payload_length: u16,
    pub checksum: u32,
}

impl FrameHeader {
    pub const FLAG_SYN: u8 = 1 << 0;
    pub const FLAG_ACK: u8 = 1 << 1;
    pub const FLAG_FIN: u8 = 1 << 2;
    pub const FLAG_NACK: u8 = 1 << 3;
    pub const FLAG_RETRANSMIT: u8 = 1 << 4;

    pub const SIZE: usize = 16;

    pub fn new(sequence: u64, flags: u8, payload_length: u16, checksum: u32) -> Self {
        Self { sequence, flags, payload_length, checksum }
    }

    pub fn is_syn(&self) -> bool {
        self.flags & Self::FLAG_SYN != 0
    }

    pub fn is_ack(&self) -> bool {
        self.flags & Self::FLAG_ACK != 0
    }

    pub fn is_fin(&self) -> bool {
        self.flags & Self::FLAG_FIN != 0
    }

    pub fn is_nack(&self) -> bool {
        self.flags & Self::FLAG_NACK != 0
    }

    pub fn is_retransmit(&self) -> bool {
        self.flags & Self::FLAG_RETRANSMIT != 0
    }

    pub fn encode(&self) -> [u8; Self::SIZE] {
        let mut buf = [0u8; Self::SIZE];
        buf[0..8].copy_from_slice(&self.sequence.to_le_bytes());
        buf[8] = self.flags;
        buf[9..11].copy_from_slice(&self.payload_length.to_le_bytes());
        buf[11..15].copy_from_slice(&self.checksum.to_le_bytes());
        buf[15] = 0; // reserved
        buf
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, TransportError> {
        if bytes.len() < Self::SIZE {
            return Err(TransportError::InvalidFrame {
                reason: format!(
                    "header too small: {} bytes (need {})",
                    bytes.len(),
                    Self::SIZE
                ),
            });
        }
        Ok(FrameHeader {
            sequence: u64::from_le_bytes(bytes[0..8].try_into().unwrap()),
            flags: bytes[8],
            payload_length: u16::from_le_bytes(bytes[9..11].try_into().unwrap()),
            checksum: u32::from_le_bytes(bytes[11..15].try_into().unwrap()),
        })
    }
}

///////////////////////////////////////////////////////////////////////////////
// FRAMED DATA
///////////////////////////////////////////////////////////////////////////////

/// A complete transport frame: header + payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FramedData {
    pub header: FrameHeader,
    pub payload: Vec<u8>,
}

impl FramedData {
    pub fn new(header: FrameHeader, payload: Vec<u8>) -> Self {
        Self { header, payload }
    }

    pub fn syn(sequence: u64) -> Self {
        Self {
            header: FrameHeader::new(sequence, FrameHeader::FLAG_SYN, 0, 0),
            payload: Vec::new(),
        }
    }

    pub fn ack(sequence: u64) -> Self {
        Self {
            header: FrameHeader::new(sequence, FrameHeader::FLAG_ACK, 0, 0),
            payload: Vec::new(),
        }
    }

    pub fn fin(sequence: u64) -> Self {
        Self {
            header: FrameHeader::new(sequence, FrameHeader::FLAG_FIN, 0, 0),
            payload: Vec::new(),
        }
    }

    pub fn nack(sequence: u64) -> Self {
        Self {
            header: FrameHeader::new(sequence, FrameHeader::FLAG_NACK, 0, 0),
            payload: Vec::new(),
        }
    }

    pub fn data(sequence: u64, payload: Vec<u8>, checksum: u32) -> Self {
        Self {
            header: FrameHeader::new(
                sequence,
                0,
                payload.len() as u16,
                checksum,
            ),
            payload,
        }
    }

    pub fn encode(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(FrameHeader::SIZE + self.payload.len());
        buf.extend_from_slice(&self.header.encode());
        buf.extend_from_slice(&self.payload);
        buf
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, TransportError> {
        if bytes.len() < FrameHeader::SIZE {
            return Err(TransportError::InvalidFrame {
                reason: format!(
                    "frame too small: {} bytes (need at least {})",
                    bytes.len(),
                    FrameHeader::SIZE
                ),
            });
        }
        let header = FrameHeader::decode(&bytes[0..FrameHeader::SIZE])?;
        let payload = bytes[FrameHeader::SIZE..].to_vec();
        if payload.len() != header.payload_length as usize {
            return Err(TransportError::InvalidFrame {
                reason: format!(
                    "payload length mismatch: header says {}, actual {}",
                    header.payload_length,
                    payload.len()
                ),
            });
        }
        Ok(FramedData { header, payload })
    }

    pub fn total_size(&self) -> usize {
        FrameHeader::SIZE + self.payload.len()
    }

    pub fn is_control_frame(&self) -> bool {
        self.header.flags != 0
    }

    pub fn is_data_frame(&self) -> bool {
        !self.is_control_frame() && !self.payload.is_empty()
    }
}

///////////////////////////////////////////////////////////////////////////////
// SEQUENCE TRACKER
///////////////////////////////////////////////////////////////////////////////

/// Tracks frame sequence numbers for ordering and deduplication.
pub struct SequenceTracker {
    pub next_send: u64,
    pub next_expected: u64,
    pub last_acked: u64,
    pub sent_unacked: std::collections::HashMap<u64, FramedData>,
    pub received_out_of_order: std::collections::HashMap<u64, FramedData>,
    pub max_window: u64,
    pub duplicate_count: u64,
    pub out_of_order_count: u64,
}

impl SequenceTracker {
    pub fn new() -> Self {
        Self {
            next_send: 0,
            next_expected: 0,
            last_acked: 0,
            sent_unacked: std::collections::HashMap::new(),
            received_out_of_order: std::collections::HashMap::new(),
            max_window: 1024,
            duplicate_count: 0,
            out_of_order_count: 0,
        }
    }

    pub fn next_sequence(&mut self) -> u64 {
        let seq = self.next_send;
        self.next_send += 1;
        seq
    }

    pub fn mark_sent(&mut self, frame: FramedData) {
        let seq = frame.header.sequence;
        self.sent_unacked.insert(seq, frame);
    }

    pub fn mark_acked(&mut self, sequence: u64) {
        self.sent_unacked.remove(&sequence);
        if sequence > self.last_acked {
            self.last_acked = sequence;
        }
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

    pub fn receive_frame(&mut self, frame: FramedData) -> bool {
        let seq = frame.header.sequence;
        if seq == self.next_expected {
            self.next_expected += 1;
            true
        } else if seq < self.next_expected {
            self.duplicate_count += 1;
            false
        } else {
            self.out_of_order_count += 1;
            self.received_out_of_order.insert(seq, frame);
            false
        }
    }

    pub fn get_next_in_order(&mut self) -> Option<FramedData> {
        while let Some(frame) = self.received_out_of_order.remove(&self.next_expected) {
            self.next_expected += 1;
            return Some(frame);
        }
        None
    }

    pub fn duplicate_count(&self) -> u64 {
        self.duplicate_count
    }

    pub fn out_of_order_count(&self) -> u64 {
        self.out_of_order_count
    }

    pub fn set_max_window(&mut self, window: u64) {
        self.max_window = window;
    }
}

impl Default for SequenceTracker {
    fn default() -> Self {
        Self::new()
    }
}

///////////////////////////////////////////////////////////////////////////////
// ACK TRACKER
///////////////////////////////////////////////////////////////////////////////

/// Tracks acknowledgements for sent frames.
pub struct AckTracker {
    pub pending_acks: std::collections::HashMap<u64, u64>, // seq -> last_sent_ms
    pub acked_sequences: std::collections::HashSet<u64>,
    pub nacked_sequences: std::collections::HashSet<u64>,
    pub total_acks_sent: u64,
    pub total_acks_received: u64,
    pub total_timeouts: u64,
}

impl AckTracker {
    pub fn new() -> Self {
        Self {
            pending_acks: std::collections::HashMap::new(),
            acked_sequences: std::collections::HashSet::new(),
            nacked_sequences: std::collections::HashSet::new(),
            total_acks_sent: 0,
            total_acks_received: 0,
            total_timeouts: 0,
        }
    }

    pub fn mark_pending(&mut self, sequence: u64, timestamp_ms: u64) {
        self.pending_acks.insert(sequence, timestamp_ms);
    }

    pub fn mark_received(&mut self, sequence: u64) {
        self.pending_acks.remove(&sequence);
        self.acked_sequences.insert(sequence);
        self.total_acks_received += 1;
    }

    pub fn mark_nacked(&mut self, sequence: u64) {
        self.pending_acks.remove(&sequence);
        self.nacked_sequences.insert(sequence);
    }

    pub fn mark_timeout(&mut self, sequence: u64) {
        self.total_timeouts += 1;
    }

    pub fn is_acked(&self, sequence: u64) -> bool {
        self.acked_sequences.contains(&sequence)
    }

    pub fn is_nacked(&self, sequence: u64) -> bool {
        self.nacked_sequences.contains(&sequence)
    }

    pub fn is_pending(&self, sequence: u64) -> bool {
        self.pending_acks.contains_key(&sequence)
    }

    pub fn pending_count(&self) -> usize {
        self.pending_acks.len()
    }

    pub fn get_pending_sequences(&self) -> Vec<u64> {
        self.pending_acks.keys().copied().collect()
    }

    pub fn clear_acked(&mut self) {
        self.acked_sequences.clear();
    }

    pub fn clear_nacked(&mut self) {
        self.nacked_sequences.clear();
    }
}

impl Default for AckTracker {
    fn default() -> Self {
        Self::new()
    }
}

///////////////////////////////////////////////////////////////////////////////
// TRANSPORT CONFIG
///////////////////////////////////////////////////////////////////////////////

/// Configuration for transport behavior.
#[derive(Debug, Clone)]
pub struct TransportConfig {
    pub delivery_semantics: DeliverySemantics,
    pub max_frame_size: usize,
    pub ack_timeout_ms: u64,
    pub max_retransmissions: u32,
    pub enable_sequencing: bool,
    pub enable_acknowledgements: bool,
    pub window_size: u64,
}

impl Default for TransportConfig {
    fn default() -> Self {
        Self {
            delivery_semantics: DeliverySemantics::AtLeastOnce,
            max_frame_size: 65535,
            ack_timeout_ms: 5000,
            max_retransmissions: 3,
            enable_sequencing: true,
            enable_acknowledgements: true,
            window_size: 1024,
        }
    }
}

impl TransportConfig {
    pub fn new(delivery_semantics: DeliverySemantics) -> Self {
        Self { delivery_semantics, ..Default::default() }
    }

    pub fn with_max_frame_size(mut self, size: usize) -> Self {
        self.max_frame_size = size;
        self
    }

    pub fn with_ack_timeout(mut self, ms: u64) -> Self {
        self.ack_timeout_ms = ms;
        self
    }

    pub fn with_max_retransmissions(mut self, count: u32) -> Self {
        self.max_retransmissions = count;
        self
    }

    pub fn with_sequencing(mut self, enabled: bool) -> Self {
        self.enable_sequencing = enabled;
        self
    }

    pub fn with_acknowledgements(mut self, enabled: bool) -> Self {
        self.enable_acknowledgements = enabled;
        self
    }

    pub fn with_window_size(mut self, size: u64) -> Self {
        self.window_size = size;
        self
    }

    pub fn validate(&self) -> Result<(), TransportError> {
        if self.max_frame_size == 0 {
            return Err(TransportError::FrameTooLarge {
                size: 0,
                max: self.max_frame_size,
            });
        }
        if self.ack_timeout_ms == 0 {
            return Err(TransportError::Internal {
                message: "ack timeout must be non-zero".to_string(),
            });
        }
        Ok(())
    }
}

///////////////////////////////////////////////////////////////////////////////
// TRANSPORT STATISTICS
///////////////////////////////////////////////////////////////////////////////

/// Statistics collected by a transport implementation.
#[derive(Debug, Clone, Default)]
pub struct TransportStats {
    pub frames_framed: u64,
    pub frames_deframed: u64,
    pub bytes_framed: u64,
    pub bytes_deframed: u64,
    pub frames_sequenced: u64,
    pub frames_acknowledged: u64,
    pub frames_retransmitted: u64,
    pub frames_dropped: u64,
    pub acks_sent: u64,
    pub acks_received: u64,
    pub ack_timeouts: u64,
    pub duplicate_frames: u64,
    pub out_of_order_frames: u64,
    pub checksum_errors: u64,
}

impl TransportStats {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn total_frames(&self) -> u64 {
        self.frames_framed + self.frames_deframed
    }

    pub fn total_bytes(&self) -> u64 {
        self.bytes_framed + self.bytes_deframed
    }

    pub fn total_acks(&self) -> u64 {
        self.acks_sent + self.acks_received
    }

    pub fn error_rate(&self) -> f64 {
        let total = self.total_frames();
        if total == 0 {
            return 0.0;
        }
        (self.frames_dropped + self.checksum_errors) as f64 / total as f64
    }

    pub fn retransmission_rate(&self) -> f64 {
        let total = self.frames_framed;
        if total == 0 {
            return 0.0;
        }
        self.frames_retransmitted as f64 / total as f64
    }

    pub fn ack_success_rate(&self) -> f64 {
        let sent = self.acks_sent;
        if sent == 0 {
            return 1.0;
        }
        self.acks_received as f64 / sent as f64
    }
}

///////////////////////////////////////////////////////////////////////////////
// TRANSPORT ERROR EXTENDED VARIANTS AND METHODS
///////////////////////////////////////////////////////////////////////////////

impl TransportError {
    pub fn suggested_retry_delay_ms(&self) -> Option<u64> {
        match self {
            TransportError::AckTimeout { .. } => Some(1000),
            TransportError::MaxRetransmissionsExceeded { .. } => Some(5000),
            TransportError::FrameEncodeFailed { .. } => Some(100),
            TransportError::FrameDecodeFailed { .. } => Some(100),
            TransportError::Internal { .. } => Some(2000),
            _ => None,
        }
    }

    pub fn is_permanent(&self) -> bool {
        matches!(
            self,
            TransportError::InvalidFrame { .. }
                | TransportError::SequenceOutOfRange { .. }
                | TransportError::DuplicateSequence { .. }
                | TransportError::FrameTooLarge { .. }
                | TransportError::NotConnected
                | TransportError::AlreadyConnected
                | TransportError::DeliverySemanticsMismatch { .. }
        )
    }

    pub fn is_transient(&self) -> bool {
        !self.is_permanent() && self.is_retryable()
    }
}

///////////////////////////////////////////////////////////////////////////////
// TRANSPORT CONNECTION STATE
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TransportConnectionState {
    Disconnected,
    Connecting,
    Connected,
    Disconnecting,
    Failed,
}

impl TransportConnectionState {
    pub fn as_str(&self) -> &'static str {
        match self {
            TransportConnectionState::Disconnected => "disconnected",
            TransportConnectionState::Connecting => "connecting",
            TransportConnectionState::Connected => "connected",
            TransportConnectionState::Disconnecting => "disconnecting",
            TransportConnectionState::Failed => "failed",
        }
    }

    pub fn is_connected(&self) -> bool {
        matches!(self, TransportConnectionState::Connected)
    }

    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            TransportConnectionState::Failed | TransportConnectionState::Disconnected
        )
    }

    pub fn can_send(&self) -> bool {
        matches!(self, TransportConnectionState::Connected)
    }
}

///////////////////////////////////////////////////////////////////////////////
// TRANSPORT WINDOW MANAGER
///////////////////////////////////////////////////////////////////////////////

pub struct TransportWindowManager {
    pub window_size: u64,
    pub bytes_in_flight: u64,
    pub max_window_size: u64,
    pub min_window_size: u64,
}

impl TransportWindowManager {
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

impl Default for TransportWindowManager {
    fn default() -> Self {
        Self::new()
    }
}

///////////////////////////////////////////////////////////////////////////////
// TRANSPORT RETRY MANAGER
///////////////////////////////////////////////////////////////////////////////

pub struct TransportRetryManager {
    pub max_retries: u32,
    pub retry_count: std::cell::Cell<u32>,
    pub base_delay_ms: u64,
    pub max_delay_ms: u64,
    pub backoff_multiplier: f64,
}

impl TransportRetryManager {
    pub fn new(max_retries: u32) -> Self {
        Self {
            max_retries,
            retry_count: std::cell::Cell::new(0),
            base_delay_ms: 100,
            max_delay_ms: 30000,
            backoff_multiplier: 2.0,
        }
    }

    pub fn should_retry(&self, error: &TransportError) -> bool {
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

impl Default for TransportRetryManager {
    fn default() -> Self {
        Self::new(3)
    }
}

///////////////////////////////////////////////////////////////////////////////
// TRANSPORT HEALTH MONITOR
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransportHealth {
    Healthy,
    Degraded,
    Unhealthy,
    Unknown,
}

impl TransportHealth {
    pub fn as_str(&self) -> &'static str {
        match self {
            TransportHealth::Healthy => "healthy",
            TransportHealth::Degraded => "degraded",
            TransportHealth::Unhealthy => "unhealthy",
            TransportHealth::Unknown => "unknown",
        }
    }

    pub fn is_healthy(&self) -> bool {
        matches!(self, TransportHealth::Healthy)
    }

    pub fn is_degraded(&self) -> bool {
        matches!(self, TransportHealth::Degraded)
    }

    pub fn is_unhealthy(&self) -> bool {
        matches!(self, TransportHealth::Unhealthy)
    }
}

pub struct TransportHealthMonitor {
    pub state: TransportConnectionState,
    pub health: TransportHealth,
    pub consecutive_failures: u32,
    pub total_sent: u64,
    pub total_received: u64,
    pub total_errors: u64,
}

impl TransportHealthMonitor {
    pub fn new() -> Self {
        Self {
            state: TransportConnectionState::Disconnected,
            health: TransportHealth::Unknown,
            consecutive_failures: 0,
            total_sent: 0,
            total_received: 0,
            total_errors: 0,
        }
    }

    pub fn record_success(&mut self, bytes: u64) {
        self.state = TransportConnectionState::Connected;
        self.health = if self.consecutive_failures > 0 {
            TransportHealth::Degraded
        } else {
            TransportHealth::Healthy
        };
        self.consecutive_failures = 0;
        self.total_sent += bytes;
    }

    pub fn record_failure(&mut self, error: &TransportError) {
        self.consecutive_failures += 1;
        self.total_errors += 1;
        self.health = if self.consecutive_failures >= 5 {
            TransportHealth::Unhealthy
        } else if self.consecutive_failures >= 2 {
            TransportHealth::Degraded
        } else {
            TransportHealth::Healthy
        };
        if self.consecutive_failures >= 10 {
            self.state = TransportConnectionState::Failed;
        }
    }

    pub fn record_received(&mut self, bytes: u64) {
        self.total_received += bytes;
        if self.health == TransportHealth::Unknown {
            self.health = TransportHealth::Healthy;
        }
    }

    pub fn is_healthy(&self) -> bool {
        self.health.is_healthy()
    }

    pub fn is_unhealthy(&self) -> bool {
        self.health.is_unhealthy()
    }

    pub fn reset(&mut self) {
        self.state = TransportConnectionState::Disconnected;
        self.health = TransportHealth::Unknown;
        self.consecutive_failures = 0;
        self.total_sent = 0;
        self.total_received = 0;
        self.total_errors = 0;
    }
}

impl Default for TransportHealthMonitor {
    fn default() -> Self {
        Self::new()
    }
}

///////////////////////////////////////////////////////////////////////////////
// TRANSPORT RATE LIMITER
///////////////////////////////////////////////////////////////////////////////

pub struct TransportRateLimiter {
    pub max_tokens: u64,
    pub tokens: std::cell::Cell<u64>,
    pub refill_rate_per_sec: u64,
}

impl TransportRateLimiter {
    pub fn new(max_tokens: u64, refill_rate_per_sec: u64) -> Self {
        Self {
            max_tokens,
            tokens: std::cell::Cell::new(max_tokens),
            refill_rate_per_sec,
        }
    }

    pub fn try_consume(&self) -> bool {
        let current = self.tokens.get();
        if current > 0 {
            self.tokens.set(current - 1);
            true
        } else {
            false
        }
    }

    pub fn try_consume_n(&self, n: u64) -> u64 {
        let current = self.tokens.get();
        let consumed = current.min(n);
        self.tokens.set(current - consumed);
        consumed
    }

    pub fn available_tokens(&self) -> u64 {
        self.tokens.get()
    }

    pub fn max_tokens(&self) -> u64 {
        self.max_tokens
    }
}

impl Default for TransportRateLimiter {
    fn default() -> Self {
        Self::new(1000, 1000)
    }
}

///////////////////////////////////////////////////////////////////////////////
// TRANSPORT FRAME POOL
///////////////////////////////////////////////////////////////////////////////

pub struct TransportFramePool {
    pub frames: std::cell::RefCell<Vec<Vec<u8>>>,
    pub max_pool_size: usize,
    pub total_allocated: u64,
    pub total_released: u64,
}

impl TransportFramePool {
    pub fn new(max_pool_size: usize) -> Self {
        Self {
            frames: std::cell::RefCell::new(Vec::new()),
            max_pool_size,
            total_allocated: 0,
            total_released: 0,
        }
    }

    pub fn allocate(&self, size: usize) -> Vec<u8> {
        if let Some(mut buf) = self.frames.borrow_mut().pop() {
            buf.truncate(0);
            buf.reserve(size);
            self.total_released += 1;
            buf
        } else {
            self.total_allocated += 1;
            Vec::with_capacity(size)
        }
    }

    pub fn release(&self, mut buf: Vec<u8>) {
        if self.frames.borrow().len() < self.max_pool_size {
            buf.clear();
            self.frames.borrow_mut().push(buf);
        }
    }

    pub fn pool_size(&self) -> usize {
        self.frames.borrow().len()
    }

    pub fn total_allocated(&self) -> u64 {
        self.total_allocated
    }

    pub fn total_released(&self) -> u64 {
        self.total_released
    }
}

impl Default for TransportFramePool {
    fn default() -> Self {
        Self::new(64)
    }
}

///////////////////////////////////////////////////////////////////////////////
// TRANSPORT ENDPOINT BINDING
///////////////////////////////////////////////////////////////////////////////

pub struct TransportEndpointBinding {
    pub endpoint: Endpoint,
    pub bound: bool,
    pub bound_since: Option<u64>,
}

impl TransportEndpointBinding {
    pub fn new(endpoint: Endpoint) -> Self {
        Self {
            endpoint,
            bound: false,
            bound_since: None,
        }
    }

    pub fn bind(&mut self, timestamp: u64) {
        self.bound = true;
        self.bound_since = Some(timestamp);
    }

    pub fn unbind(&mut self) {
        self.bound = false;
        self.bound_since = None;
    }

    pub fn is_bound(&self) -> bool {
        self.bound
    }

    pub fn bound_duration_ms(&self, now: u64) -> Option<u64> {
        self.bound_since.map(|start| now.saturating_sub(start))
    }
}

///////////////////////////////////////////////////////////////////////////////
// TRANSPORT SEQUENCE WINDOW
///////////////////////////////////////////////////////////////////////////////

pub struct TransportSequenceWindow {
    pub base: u64,
    pub next_seq: u64,
    pub window_size: u64,
    pub sent_unacked: std::collections::HashMap<u64, Vec<u8>>,
}

impl TransportSequenceWindow {
    pub fn new(window_size: u64) -> Self {
        Self {
            base: 0,
            next_seq: 0,
            window_size,
            sent_unacked: std::collections::HashMap::new(),
        }
    }

    pub fn next_sequence(&mut self) -> u64 {
        let seq = self.next_seq;
        self.next_seq += 1;
        seq
    }

    pub fn can_send(&self) -> bool {
        self.sent_unacked.len() < self.window_size as usize
    }

    pub fn mark_sent(&mut self, seq: u64, data: Vec<u8>) {
        self.sent_unacked.insert(seq, data);
    }

    pub fn mark_acked(&mut self, seq: u64) {
        self.sent_unacked.remove(&seq);
        if seq == self.base {
            while let Some(_) = self.sent_unacked.remove(&(self.base + 1)) {
                self.base += 1;
            }
        }
    }

    pub fn unacked_count(&self) -> usize {
        self.sent_unacked.len()
    }

    pub fn is_acked(&self, seq: u64) -> bool {
        seq < self.base || !self.sent_unacked.contains_key(&seq)
    }
}

impl Default for TransportSequenceWindow {
    fn default() -> Self {
        Self::new(1024)
    }
}

///////////////////////////////////////////////////////////////////////////////
// TRANSPORT STREAM
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone)]
pub struct TransportStream {
    pub stream_id: u64,
    pub endpoint: Endpoint,
    pub delivery_semantics: DeliverySemantics,
    pub sequence_tracker: std::cell::Cell<SequenceTracker>,
    pub ack_tracker: std::cell::Cell<AckTracker>,
    pub bytes_sent: u64,
    pub bytes_received: u64,
}

impl TransportStream {
    pub fn new(stream_id: u64, endpoint: Endpoint, delivery: DeliverySemantics) -> Self {
        Self {
            stream_id,
            endpoint,
            delivery_semantics: delivery,
            sequence_tracker: std::cell::Cell::new(SequenceTracker::new()),
            ack_tracker: std::cell::Cell::new(AckTracker::new()),
            bytes_sent: 0,
            bytes_received: 0,
        }
    }

    pub fn send_sequence(&self) -> u64 {
        self.sequence_tracker.get().next_sequence()
    }

    pub fn acknowledge_received(&self, sequence: u64) {
        self.ack_tracker.get().mark_received(sequence);
    }

    pub fn bytes_sent(&self) -> u64 {
        self.bytes_sent
    }

    pub fn bytes_received(&self) -> u64 {
        self.bytes_received
    }

    pub fn add_bytes_sent(&self, bytes: u64) {
        self.bytes_sent += bytes;
    }

    pub fn add_bytes_received(&self, bytes: u64) {
        self.bytes_received += bytes;
    }

    pub fn is_healthy(&self) -> bool {
        self.ack_tracker.get().pending_count() < 1024
    }
}

///////////////////////////////////////////////////////////////////////////////
// TRANSPORT MULTIPLEXER
///////////////////////////////////////////////////////////////////////////////

pub struct TransportMultiplexer {
    pub streams: std::cell::RefCell<std::collections::HashMap<u64, TransportStream>>,
    pub next_stream_id: std::cell::Cell<u64>,
}

impl TransportMultiplexer {
    pub fn new() -> Self {
        Self {
            streams: std::cell::RefCell::new(std::collections::HashMap::new()),
            next_stream_id: std::cell::Cell::new(0),
        }
    }

    pub fn open_stream(&self, endpoint: Endpoint, delivery: DeliverySemantics) -> u64 {
        let id = self.next_stream_id.get();
        self.next_stream_id.set(id + 1);
        let stream = TransportStream::new(id, endpoint, delivery);
        self.streams.borrow_mut().insert(id, stream);
        id
    }

    pub fn close_stream(&self, stream_id: u64) {
        self.streams.borrow_mut().remove(&stream_id);
    }

    pub fn stream_count(&self) -> usize {
        self.streams.borrow().len()
    }

    pub fn get_stream(&self, stream_id: u64) -> Option<TransportStream> {
        self.streams.borrow().get(&stream_id).cloned()
    }
}

impl Default for TransportMultiplexer {
    fn default() -> Self {
        Self::new()
    }
}

///////////////////////////////////////////////////////////////////////////////
// TRANSPORT FRAGMENTER
///////////////////////////////////////////////////////////////////////////////

pub struct TransportFragmenter {
    pub max_fragment_size: usize,
    pub fragments_created: u64,
}

impl TransportFragmenter {
    pub fn new(max_fragment_size: usize) -> Self {
        Self {
            max_fragment_size,
            fragments_created: 0,
        }
    }

    pub fn fragment(&self, payload: &[u8]) -> Vec<Vec<u8>> {
        let mut result = Vec::new();
        let mut offset = 0;
        while offset < payload.len() {
            let end = (offset + self.max_fragment_size).min(payload.len());
            result.push(payload[offset..end].to_vec());
            offset = end;
        }
        result
    }

    pub fn max_fragment_size(&self) -> usize {
        self.max_fragment_size
    }

    pub fn fragment_count(&self) -> usize {
        self.fragments_created as usize
    }
}

impl Default for TransportFragmenter {
    fn default() -> Self {
        Self::new(1400)
    }
}

///////////////////////////////////////////////////////////////////////////////
// TRANSPORT REASSEMBLER
///////////////////////////////////////////////////////////////////////////////

pub struct TransportReassembler {
    pub fragments: std::cell::RefCell<std::collections::HashMap<u64, Vec<Vec<u8>>>>,
    pub reassembled_count: u64,
}

impl TransportReassembler {
    pub fn new() -> Self {
        Self {
            fragments: std::cell::RefCell::new(std::collections::HashMap::new()),
            reassembled_count: 0,
        }
    }

    pub fn add_fragment(&self, stream_id: u64, fragment: Vec<u8>, index: u64) {
        self.fragments
            .borrow_mut()
            .entry(stream_id)
            .or_insert_with(Vec::new)
            .push(fragment);
    }

    pub fn reassemble(&self, stream_id: u64) -> Option<Vec<u8>> {
        let mut frags = self.fragments.borrow_mut().remove(&stream_id)?;
        frags.sort_by_key(|f| f.len());
        let result: Vec<u8> = frags.into_iter().flatten().collect();
        self.reassembled_count += 1;
        Some(result)
    }

    pub fn reassembled_count(&self) -> u64 {
        self.reassembled_count
    }
}

impl Default for TransportReassembler {
    fn default() -> Self {
        Self::new()
    }
}

///////////////////////////////////////////////////////////////////////////////
// TRANSPORT CLOCK AND TIMESTAMP
///////////////////////////////////////////////////////////////////////////////

pub struct TransportClock {
    pub epoch_ms: u64,
    pub ticks: u64,
}

impl TransportClock {
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

impl Default for TransportClock {
    fn default() -> Self {
        Self::new()
    }
}

///////////////////////////////////////////////////////////////////////////////
// TRANSPORT DATATYPE AND ENVELOPE
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransportDatagramType {
    Data,
    Control,
    Ack,
    Nack,
    Syn,
    Fin,
    KeepAlive,
}

#[derive(Debug, Clone)]
pub struct TransportDatagram {
    pub datagram_type: TransportDatagramType,
    pub sequence: u64,
    pub payload: Vec<u8>,
    pub timestamp_ms: u64,
}

impl TransportDatagram {
    pub fn data(sequence: u64, payload: Vec<u8>) -> Self {
        Self {
            datagram_type: TransportDatagramType::Data,
            sequence,
            payload,
            timestamp_ms: 0,
        }
    }

    pub fn control(sequence: u64) -> Self {
        Self {
            datagram_type: TransportDatagramType::Control,
            sequence,
            payload: Vec::new(),
            timestamp_ms: 0,
        }
    }

    pub fn is_data(&self) -> bool {
        matches!(self.datagram_type, TransportDatagramType::Data)
    }

    pub fn is_control(&self) -> bool {
        self.datagram_type == TransportDatagramType::Control
    }

    pub fn size(&self) -> usize {
        self.payload.len() + 24 // fixed overhead
    }
}

///////////////////////////////////////////////////////////////////////////////
// TRANSPORT STREAM POOL
///////////////////////////////////////////////////////////////////////////////

pub struct TransportStreamPool {
    pub streams: std::cell::RefCell<std::collections::HashMap<u64, TransportStream>>,
    pub max_streams: u64,
    pub total_opened: u64,
    pub total_closed: u64,
}

impl TransportStreamPool {
    pub fn new(max_streams: u64) -> Self {
        Self {
            streams: std::cell::RefCell::new(std::collections::HashMap::new()),
            max_streams,
            total_opened: 0,
            total_closed: 0,
        }
    }

    pub fn open(&self, stream_id: u64, endpoint: Endpoint, delivery: DeliverySemantics) -> bool {
        if self.streams.borrow().len() >= self.max_streams as usize {
            return false;
        }
        let stream = TransportStream::new(stream_id, endpoint, delivery);
        self.streams.borrow_mut().insert(stream_id, stream);
        self.total_opened += 1;
        true
    }

    pub fn close(&self, stream_id: u64) -> bool {
        if self.streams.borrow_mut().remove(&stream_id).is_some() {
            self.total_closed += 1;
            true
        } else {
            false
        }
    }

    pub fn stream_count(&self) -> usize {
        self.streams.borrow().len()
    }

    pub fn is_full(&self) -> bool {
        self.streams.borrow().len() >= self.max_streams as usize
    }

    pub fn total_opened(&self) -> u64 {
        self.total_opened
    }

    pub fn total_closed(&self) -> u64 {
        self.total_closed
    }
}

impl Default for TransportStreamPool {
    fn default() -> Self {
        Self::new(10000)
    }
}

///////////////////////////////////////////////////////////////////////////////
// TRANSPORT CONGESTION SIGNAL
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransportCongestionSignal {
    None,
    Mild,
    Moderate,
    Severe,
    Critical,
}

impl TransportCongestionSignal {
    pub fn as_str(&self) -> &'static str {
        match self {
            TransportCongestionSignal::None => "none",
            TransportCongestionSignal::Mild => "mild",
            TransportCongestionSignal::Moderate => "moderate",
            TransportCongestionSignal::Severe => "severe",
            TransportCongestionSignal::Critical => "critical",
        }
    }

    pub fn should_throttle(&self) -> bool {
        matches!(
            self,
            TransportCongestionSignal::Moderate
                | TransportCongestionSignal::Severe
                | TransportCongestionSignal::Critical
        )
    }

    pub fn should_drop(&self) -> bool {
        matches!(self, TransportCongestionSignal::Critical)
    }
}

///////////////////////////////////////////////////////////////////////////////
// TRANSPORT BACKPRESSURE MANAGER
///////////////////////////////////////////////////////////////////////////////

pub struct TransportBackpressureManager {
    pub signal: std::cell::Cell<TransportCongestionSignal>,
    pub queue_depth: u64,
    pub max_queue_depth: u64,
}

impl TransportBackpressureManager {
    pub fn new(max_queue_depth: u64) -> Self {
        Self {
            signal: std::cell::Cell::new(TransportCongestionSignal::None),
            queue_depth: 0,
            max_queue_depth,
        }
    }

    pub fn update_queue_depth(&self, depth: u64) {
        self.queue_depth = depth;
        let signal = if depth >= self.max_queue_depth {
            TransportCongestionSignal::Critical
        } else if depth >= self.max_queue_depth * 4 / 5 {
            TransportCongestionSignal::Severe
        } else if depth >= self.max_queue_depth * 3 / 5 {
            TransportCongestionSignal::Moderate
        } else if depth >= self.max_queue_depth * 1 / 5 {
            TransportCongestionSignal::Mild
        } else {
            TransportCongestionSignal::None
        };
        self.signal.set(signal);
    }

    pub fn current_signal(&self) -> TransportCongestionSignal {
        self.signal.get()
    }

    pub fn should_throttle(&self) -> bool {
        self.current_signal().should_throttle()
    }

    pub fn should_drop(&self) -> bool {
        self.current_signal().should_drop()
    }

    pub fn queue_utilization(&self) -> f64 {
        if self.max_queue_depth == 0 {
            return 0.0;
        }
        self.queue_depth as f64 / self.max_queue_depth as f64
    }
}

impl Default for TransportBackpressureManager {
    fn default() -> Self {
        Self::new(100000)
    }
}

///////////////////////////////////////////////////////////////////////////////
// TRANSPORT FLOW CONTROL
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransportFlowState {
    Open,
    Restricted,
    Closed,
}

impl TransportFlowState {
    pub fn as_str(&self) -> &'static str {
        match self {
            TransportFlowState::Open => "open",
            TransportFlowState::Restricted => "restricted",
            TransportFlowState::Closed => "closed",
        }
    }

    pub fn can_send(&self) -> bool {
        matches!(self, TransportFlowState::Open)
    }
}

pub struct TransportFlowControl {
    pub state: std::cell::Cell<TransportFlowState>,
    pub window: u64,
    pub bytes_in_flight: u64,
    pub credits_sent: u64,
    pub credits_received: u64,
}

impl TransportFlowControl {
    pub fn new(window: u64) -> Self {
        Self {
            state: std::cell::Cell::new(TransportFlowState::Open),
            window,
            bytes_in_flight: 0,
            credits_sent: 0,
            credits_received: 0,
        }
    }

    pub fn can_send(&self, bytes: u64) -> bool {
        self.state.get().can_send() && self.bytes_in_flight + bytes <= self.window
    }

    pub fn send_credit(&mut self, bytes: u64) {
        self.credits_sent += bytes;
    }

    pub fn receive_credit(&mut self, bytes: u64) {
        self.credits_received += bytes;
        self.bytes_in_flight = self.bytes_in_flight.saturating_sub(bytes);
    }

    pub fn add_in_flight(&mut self, bytes: u64) {
        self.bytes_in_flight += bytes;
        if self.bytes_in_flight >= self.window {
            self.state.set(TransportFlowState::Restricted);
        }
    }

    pub fn release_in_flight(&mut self, bytes: u64) {
        self.bytes_in_flight = self.bytes_in_flight.saturating_sub(bytes);
        if self.bytes_in_flight < self.window / 2 {
            self.state.set(TransportFlowState::Open);
        }
    }

    pub fn close(&mut self) {
        self.state.set(TransportFlowState::Closed);
    }

    pub fn window_utilization(&self) -> f64 {
        if self.window == 0 {
            return 0.0;
        }
        self.bytes_in_flight as f64 / self.window as f64
    }
}

impl Default for TransportFlowControl {
    fn default() -> Self {
        Self::new(65536)
    }
}

///////////////////////////////////////////////////////////////////////////////
// TRANSPORT CREDIT MANAGER
///////////////////////////////////////////////////////////////////////////////

pub struct TransportCreditManager {
    pub available_credits: u64,
    pub total_credits_issued: u64,
    pub total_credits_consumed: u64,
    pub credit_unit_bytes: u64,
}

impl TransportCreditManager {
    pub fn new(initial_credits: u64, credit_unit_bytes: u64) -> Self {
        Self {
            available_credits: initial_credits,
            total_credits_issued: initial_credits,
            total_credits_consumed: 0,
            credit_unit_bytes,
        }
    }

    pub fn consume_credit(&mut self, bytes: u64) -> bool {
        let credits_needed = bytes.div_ceil(self.credit_unit_bytes);
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

impl Default for TransportCreditManager {
    fn default() -> Self {
        Self::new(1000, 1024)
    }
}

///////////////////////////////////////////////////////////////////////////////
// TRANSPORT PRIORITY QUEUE
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransportPriorityQueueItem {
    pub priority: u32,
    pub sequence: u64,
    pub payload: Vec<u8>,
}

pub struct TransportPriorityQueue {
    pub items: std::cell::RefCell<Vec<TransportPriorityQueueItem>>,
    pub max_size: usize,
    pub total_enqueued: u64,
    pub total_dequeued: u64,
}

impl TransportPriorityQueue {
    pub fn new(max_size: usize) -> Self {
        Self {
            items: std::cell::RefCell::new(Vec::new()),
            max_size,
            total_enqueued: 0,
            total_dequeued: 0,
        }
    }

    pub fn enqueue(&self, item: TransportPriorityQueueItem) -> bool {
        if self.items.borrow().len() >= self.max_size {
            return false;
        }
        self.items.borrow_mut().push(item);
        self.items.borrow_mut().sort_by_key(|i| i.priority);
        self.total_enqueued += 1;
        true
    }

    pub fn dequeue(&self) -> Option<TransportPriorityQueueItem> {
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

impl Default for TransportPriorityQueue {
    fn default() -> Self {
        Self::new(10000)
    }
}

///////////////////////////////////////////////////////////////////////////////
// TRANSPORT PACKET
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransportPacket {
    pub sequence: u64,
    pub payload: Vec<u8>,
    pub flags: u8,
    pub timestamp_ms: u64,
}

impl TransportPacket {
    pub fn new(sequence: u64, payload: Vec<u8>) -> Self {
        Self {
            sequence,
            payload,
            flags: 0,
            timestamp_ms: 0,
        }
    }

    pub fn ack(sequence: u64) -> Self {
        Self {
            sequence,
            payload: Vec::new(),
            flags: 1,
            timestamp_ms: 0,
        }
    }

    pub fn syn(sequence: u64) -> Self {
        Self {
            sequence,
            payload: Vec::new(),
            flags: 2,
            timestamp_ms: 0,
        }
    }

    pub fn fin(sequence: u64) -> Self {
        Self {
            sequence,
            payload: Vec::new(),
            flags: 4,
            timestamp_ms: 0,
        }
    }

    pub fn is_ack(&self) -> bool {
        self.flags & 1 != 0
    }

    pub fn is_syn(&self) -> bool {
        self.flags & 2 != 0
    }

    pub fn is_fin(&self) -> bool {
        self.flags & 4 != 0
    }

    pub fn size(&self) -> usize {
        self.payload.len() + 16
    }
}

///////////////////////////////////////////////////////////////////////////////
// TRANSPORT BUFFER
///////////////////////////////////////////////////////////////////////////////

pub struct TransportBuffer {
    pub data: std::cell::RefCell<Vec<u8>>,
    pub read_pos: std::cell::Cell<usize>,
    pub capacity: usize,
}

impl TransportBuffer {
    pub fn new(capacity: usize) -> Self {
        Self {
            data: std::cell::RefCell::new(Vec::with_capacity(capacity)),
            read_pos: std::cell::Cell::new(0),
            capacity,
        }
    }

    pub fn write(&self, bytes: &[u8]) -> usize {
        let mut data = self.data.borrow_mut();
        let space = self.capacity.saturating_sub(data.len());
        let to_write = bytes.len().min(space);
        data.extend_from_slice(&bytes[..to_write]);
        to_write
    }

    pub fn read(&self, count: usize) -> Vec<u8> {
        let mut data = self.data.borrow_mut();
        let pos = self.read_pos.get();
        let available = data.len().saturating_sub(pos);
        let to_read = count.min(available);
        let result = data[pos..pos + to_read].to_vec();
        self.read_pos.set(pos + to_read);
        result
    }

    pub fn len(&self) -> usize {
        self.data.borrow().len() - self.read_pos.get()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn clear(&self) {
        self.data.borrow_mut().clear();
        self.read_pos.set(0);
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }
}

impl Default for TransportBuffer {
    fn default() -> Self {
        Self::new(65536)
    }
}

///////////////////////////////////////////////////////////////////////////////
// TESTS
///////////////////////////////////////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use super::*;

    // A test implementation of Transport for testing the trait contract.
    struct TestTransport {
        name: &'static str,
        delivery: DeliverySemantics,
        framed: std::cell::Cell<u64>,
        deframed: std::cell::Cell<u64>,
        sequenced: std::cell::Cell<u64>,
        acknowledged: std::cell::Cell<u64>,
    }

    impl TestTransport {
        fn new(name: &'static str, delivery: DeliverySemantics) -> Self {
            Self {
                name,
                delivery,
                framed: std::cell::Cell::new(0),
                deframed: std::cell::Cell::new(0),
                sequenced: std::cell::Cell::new(0),
                acknowledged: std::cell::Cell::new(0),
            }
        }
    }

    impl Transport for TestTransport {
        fn frame(&self, payload: Vec<u8>) -> Result<Vec<u8>, TransportError> {
            self.framed.set(self.framed.get() + 1);
            let mut frame = Vec::with_capacity(8 + payload.len());
            frame.extend_from_slice(&(payload.len() as u64).to_le_bytes());
            frame.extend_from_slice(&payload);
            Ok(frame)
        }

        fn deframe(&self, bytes: Vec<u8>) -> Result<Vec<u8>, TransportError> {
            self.deframed.set(self.deframed.get() + 1);
            if bytes.len() < 8 {
                return Err(TransportError::InvalidFrame {
                    reason: "frame too small".to_string(),
                });
            }
            let len = u64::from_le_bytes(bytes[0..8].try_into().unwrap()) as usize;
            if bytes.len() < 8 + len {
                return Err(TransportError::InvalidFrame {
                    reason: "payload length mismatch".to_string(),
                });
            }
            Ok(bytes[8..8 + len].to_vec())
        }

        fn sequence(&self, frame_id: u64) -> Result<(), TransportError> {
            self.sequenced.set(self.sequenced.get() + 1);
            Ok(())
        }

        fn acknowledge(&self, sequence_id: u64) -> Result<(), TransportError> {
            self.acknowledged.set(self.acknowledged.get() + 1);
            Ok(())
        }

        fn delivery_semantics(&self) -> DeliverySemantics {
            self.delivery
        }

        fn transport_name(&self) -> &'static str {
            self.name
        }

        fn bytes_framed(&self) -> u64 {
            self.framed.get()
        }

        fn bytes_deframed(&self) -> u64 {
            self.deframed.get()
        }

        fn frames_sequenced(&self) -> u64 {
            self.sequenced.get()
        }

        fn frames_acknowledged(&self) -> u64 {
            self.acknowledged.get()
        }
    }

    #[test]
    fn transport_trait_frame_and_deframe() {
        let transport = TestTransport::new("test", DeliverySemantics::ExactlyOnce);
        let payload = vec![1, 2, 3, 4, 5];
        let framed = transport.frame(payload.clone()).unwrap();
        assert!(transport.frames_sequenced() == 0); // frame() doesn't sequence by default
        let deframed = transport.deframe(framed).unwrap();
        assert_eq!(deframed, payload);
    }

    #[test]
    fn transport_sequence_and_acknowledge() {
        let transport = TestTransport::new("test", DeliverySemantics::AtLeastOnce);
        transport.sequence(1).unwrap();
        transport.sequence(2).unwrap();
        assert_eq!(transport.frames_sequenced(), 2);
        transport.acknowledge(1).unwrap();
        assert_eq!(transport.frames_acknowledged(), 1);
    }

    #[test]
    fn transport_delivery_semantics() {
        assert_eq!(
            DeliverySemantics::AtMostOnce.as_str(),
            "at_most_once"
        );
        assert_eq!(
            DeliverySemantics::AtLeastOnce.as_str(),
            "at_least_once"
        );
        assert_eq!(
            DeliverySemantics::ExactlyOnce.as_str(),
            "exactly_once"
        );
        assert!(!DeliverySemantics::AtMostOnce.guarantees_delivery());
        assert!(DeliverySemantics::AtLeastOnce.guarantees_delivery());
        assert!(!DeliverySemantics::AtMostOnce.guarantees_ordering());
        assert!(DeliverySemantics::ExactlyOnce.guarantees_ordering());
    }

    #[test]
    fn transport_error_category() {
        assert_eq!(
            TransportError::FrameEncodeFailed { reason: "x".to_string() }.category(),
            "frame_encode_failed"
        );
        assert_eq!(
            TransportError::FrameDecodeFailed { reason: "x".to_string() }.category(),
            "frame_decode_failed"
        );
        assert_eq!(
            TransportError::InvalidFrame { reason: "x".to_string() }.category(),
            "invalid_frame"
        );
        assert_eq!(
            TransportError::SequenceOutOfRange { expected: 1, got: 2 }.category(),
            "sequence_out_of_range"
        );
        assert_eq!(
            TransportError::DuplicateSequence { seq: 5 }.category(),
            "duplicate_sequence"
        );
        assert_eq!(
            TransportError::AckFailed { seq: 1, reason: "x".to_string() }.category(),
            "ack_failed"
        );
        assert_eq!(
            TransportError::AckTimeout { seq: 1 }.category(),
            "ack_timeout"
        );
        assert_eq!(
            TransportError::MaxRetransmissionsExceeded { seq: 1 }.category(),
            "max_retransmissions"
        );
        assert_eq!(
            TransportError::FrameTooLarge { size: 100, max: 50 }.category(),
            "frame_too_large"
        );
        assert_eq!(
            TransportError::NotConnected.category(),
            "not_connected"
        );
        assert_eq!(
            TransportError::Internal { message: "x".to_string() }.category(),
            "internal"
        );
    }

    #[test]
    fn transport_error_retryable_and_fatal() {
        assert!(TransportError::AckTimeout { seq: 1 }.is_retryable());
        assert!(TransportError::Internal { message: "x".to_string() }.is_retryable());
        assert!(!TransportError::InvalidFrame { reason: "x".to_string() }.is_retryable());
        assert!(!TransportError::NotConnected.is_retryable());
        assert!(TransportError::NotConnected.is_fatal());
    }

    #[test]
    fn transport_error_plain_message() {
        let msg = TransportError::SequenceOutOfRange { expected: 5, got: 10 }.plain_message();
        assert!(msg.contains("expected 5"));
        assert!(msg.contains("got 10"));
        let msg = TransportError::FrameTooLarge { size: 2000, max: 1024 }.plain_message();
        assert!(msg.contains("2000"));
        assert!(msg.contains("1024"));
    }

    #[test]
    fn transport_config_default_and_validate() {
        let config = TransportConfig::default();
        assert_eq!(config.delivery_semantics, DeliverySemantics::AtLeastOnce);
        assert_eq!(config.max_frame_size, 65535);
        assert_eq!(config.ack_timeout_ms, 5000);
        assert_eq!(config.max_retransmissions, 3);
        assert!(config.enable_sequencing);
        assert!(config.enable_acknowledgements);
        assert!(config.validate().is_ok());

        let mut bad = TransportConfig::default();
        bad.max_frame_size = 0;
        assert!(bad.validate().is_err());
    }

    #[test]
    fn transport_config_builder() {
        let config = TransportConfig::new(DeliverySemantics::ExactlyOnce)
            .with_max_frame_size(9000)
            .with_ack_timeout(10000)
            .with_max_retransmissions(5)
            .with_sequencing(false)
            .with_acknowledgements(false)
            .with_window_size(2048);
        assert_eq!(config.delivery_semantics, DeliverySemantics::ExactlyOnce);
        assert_eq!(config.max_frame_size, 9000);
        assert_eq!(config.ack_timeout_ms, 10000);
        assert_eq!(config.max_retransmissions, 5);
        assert!(!config.enable_sequencing);
        assert!(!config.enable_acknowledgements);
        assert_eq!(config.window_size, 2048);
    }

    #[test]
    fn frame_header_encode_decode() {
        let header = FrameHeader::new(42, FrameHeader::FLAG_SYN, 100, 0xDEADBEEF);
        let encoded = header.encode();
        assert_eq!(encoded.len(), FrameHeader::SIZE);
        let decoded = FrameHeader::decode(&encoded).unwrap();
        assert_eq!(decoded.sequence, 42);
        assert!(decoded.is_syn());
        assert_eq!(decoded.payload_length, 100);
        assert_eq!(decoded.checksum, 0xDEADBEEF);
    }

    #[test]
    fn frame_header_flags() {
        assert!(FrameHeader::FLAG_SYN != 0);
        assert!(FrameHeader::FLAG_ACK != 0);
        assert!(FrameHeader::FLAG_FIN != 0);
        assert!(FrameHeader::FLAG_NACK != 0);
        assert!(FrameHeader::FLAG_RETRANSMIT != 0);
        let header = FrameHeader::new(0, FrameHeader::FLAG_ACK | FrameHeader::FLAG_SYN, 0, 0);
        assert!(header.is_syn());
        assert!(header.is_ack());
        assert!(!header.is_fin());
        assert!(!header.is_nack());
        assert!(!header.is_retransmit());
    }

    #[test]
    fn framed_data_syn_ack_fin_nack() {
        let syn = FramedData::syn(1);
        assert!(syn.header.is_syn());
        assert!(syn.is_control_frame());
        assert!(!syn.is_data_frame());
        let ack = FramedData::ack(2);
        assert!(ack.header.is_ack());
        let fin = FramedData::fin(3);
        assert!(fin.header.is_fin());
        let nack = FramedData::nack(4);
        assert!(nack.header.is_nack());
    }

    #[test]
    fn framed_data_encode_decode() {
        let payload = vec![10, 20, 30, 40, 50];
        let framed = FramedData::data(7, payload.clone(), 0x12345678);
        let encoded = framed.encode();
        let decoded = FramedData::decode(&encoded).unwrap();
        assert_eq!(decoded.header.sequence, 7);
        assert_eq!(decoded.payload, payload);
        assert_eq!(decoded.total_size(), FrameHeader::SIZE + payload.len());
    }

    #[test]
    fn framed_data_decode_invalid() {
        let result = FramedData::decode(&[1, 2, 3]);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, TransportError::InvalidFrame { .. }));
    }

    #[test]
    fn sequence_tracker_new_and_next() {
        let mut tracker = SequenceTracker::new();
        assert_eq!(tracker.next_sequence(), 0);
        assert_eq!(tracker.next_sequence(), 1);
        assert_eq!(tracker.next_sequence(), 2);
    }

    #[test]
    fn sequence_tracker_mark_sent_and_acked() {
        let mut tracker = SequenceTracker::new();
        let frame = FramedData::data(tracker.next_sequence(), vec![1, 2], 0);
        tracker.mark_sent(frame);
        assert_eq!(tracker.unacked_count(), 1);
        assert!(!tracker.is_acked(0));
        tracker.mark_acked(0);
        assert_eq!(tracker.unacked_count(), 0);
        assert!(tracker.is_acked(0));
    }

    #[test]
    fn sequence_tracker_receive_in_order() {
        let mut tracker = SequenceTracker::new();
        let frame = FramedData::data(0, vec![1], 0);
        assert!(tracker.receive_frame(frame));
        assert_eq!(tracker.next_expected, 1);
        assert_eq!(tracker.duplicate_count(), 0);
        assert_eq!(tracker.out_of_order_count(), 0);
    }

    #[test]
    fn sequence_tracker_receive_out_of_order() {
        let mut tracker = SequenceTracker::new();
        let frame = FramedData::data(5, vec![1], 0);
        assert!(!tracker.receive_frame(frame));
        assert_eq!(tracker.out_of_order_count(), 1);
        assert_eq!(tracker.next_expected, 0);
    }

    #[test]
    fn sequence_tracker_receive_duplicate() {
        let mut tracker = SequenceTracker::new();
        tracker.receive_frame(FramedData::data(0, vec![1], 0));
        tracker.receive_frame(FramedData::data(0, vec![2], 0));
        assert_eq!(tracker.duplicate_count(), 1);
    }

    #[test]
    fn sequence_tracker_get_next_in_order() {
        let mut tracker = SequenceTracker::new();
        tracker.receive_frame(FramedData::data(2, vec![2], 0));
        tracker.receive_frame(FramedData::data(0, vec![0], 0));
        let frame = tracker.get_next_in_order().unwrap();
        assert_eq!(frame.header.sequence, 0);
        let frame = tracker.get_next_in_order().unwrap();
        assert_eq!(frame.header.sequence, 2);
        assert_eq!(tracker.next_expected, 3);
    }

    #[test]
    fn sequence_tracker_can_send() {
        let mut tracker = SequenceTracker::new();
        assert!(tracker.can_send());
        tracker.set_max_window(2);
        tracker.mark_sent(FramedData::data(0, vec![], 0));
        tracker.mark_sent(FramedData::data(1, vec![], 0));
        assert!(!tracker.can_send());
    }

    #[test]
    fn ack_tracker_mark_and_check() {
        let mut tracker = AckTracker::new();
        tracker.mark_pending(1, 100);
        assert!(tracker.is_pending(1));
        assert_eq!(tracker.pending_count(), 1);
        tracker.mark_received(1);
        assert!(!tracker.is_pending(1));
        assert!(tracker.is_acked(1));
        assert_eq!(tracker.total_acks_received, 1);
    }

    #[test]
    fn ack_tracker_mark_nack() {
        let mut tracker = AckTracker::new();
        tracker.mark_pending(1, 100);
        tracker.mark_nacked(1);
        assert!(!tracker.is_pending(1));
        assert!(tracker.is_nacked(1));
    }

    #[test]
    fn ack_tracker_mark_timeout() {
        let mut tracker = AckTracker::new();
        tracker.mark_timeout(1);
        assert_eq!(tracker.total_timeouts, 1);
    }

    #[test]
    fn transport_stats_default_and_totals() {
        let stats = TransportStats::default();
        assert_eq!(stats.total_frames(), 0);
        assert_eq!(stats.total_bytes(), 0);
        assert!((stats.error_rate() - 0.0).abs() < f64::EPSILON);
        assert!((stats.retransmission_rate() - 0.0).abs() < f64::EPSILON);
        assert!((stats.ack_success_rate() - 1.0).abs() < f64::EPSILON);

        let mut stats = stats;
        stats.frames_framed = 100;
        stats.frames_deframed = 50;
        stats.bytes_framed = 10000;
        stats.bytes_deframed = 5000;
        stats.frames_dropped = 5;
        stats.checksum_errors = 3;
        assert_eq!(stats.total_frames(), 150);
        assert_eq!(stats.total_bytes(), 15000);
        assert!((stats.error_rate() - (8.0 / 150.0)).abs() < f64::EPSILON);
    }

    #[test]
    fn transport_stats_ack_rates() {
        let mut stats = TransportStats::default();
        stats.acks_sent = 100;
        stats.acks_received = 90;
        assert_eq!(stats.total_acks(), 190);
        assert!((stats.ack_success_rate() - 0.9).abs() < f64::EPSILON);

        let stats = TransportStats::default();
        assert!((stats.ack_success_rate() - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn transport_test_implementation_counts() {
        let transport = TestTransport::new("test", DeliverySemantics::ExactlyOnce);
        transport.frame(vec![1, 2, 3]).unwrap();
        transport.frame(vec![4, 5]).unwrap();
        assert_eq!(transport.bytes_framed(), 2);
        transport.deframe(vec![0, 0, 0, 0, 0, 0, 0, 3, 1, 2, 3]).unwrap();
        assert_eq!(transport.bytes_deframed(), 1);
    }

    #[test]
    fn transport_frame_checksum_valid() {
        let payload = vec![1, 2, 3, 4, 5];
        let checksum = FrameHeader::new(0, 0, 0, 0).encode(); // just to get size
        let _ = checksum;
        let checksum_val = FrameHeader::compute_checksum_for_test(&payload);
        assert!(checksum_val > 0);
    }

    #[test]
    fn transport_max_frame_size_and_overhead() {
        let transport = TestTransport::new("test", DeliverySemantics::AtMostOnce);
        assert_eq!(transport.max_frame_size(), 65535);
        assert_eq!(transport.frame_header_overhead(), 16);
        assert!(transport.requires_sequencing());
        assert!(transport.requires_acknowledgements());
    }

    #[test]
    fn transport_default_timeout_and_retries() {
        let transport = TestTransport::new("test", DeliverySemantics::AtLeastOnce);
        assert_eq!(transport.default_ack_timeout_ms(), 5000);
        assert_eq!(transport.max_retransmissions(), 3);
    }

    #[test]
    fn transport_connected_and_flush() {
        let transport = TestTransport::new("test", DeliverySemantics::ExactlyOnce);
        assert!(transport.is_connected());
        assert!(transport.flush().is_ok());
        assert!(transport.reset().is_ok());
    }

    #[test]
    fn transport_priority_and_reliability() {
        let transport = TestTransport::new("test", DeliverySemantics::ExactlyOnce);
        assert_eq!(transport.priority(), 0);
        assert_eq!(transport.reliability_level(), 5);
    }

    // === TransportError extended tests ===
    #[test]
    fn transport_error_suggested_retry_delay() {
        assert_eq!(
            TransportError::AckTimeout { seq: 1 }.suggested_retry_delay_ms(),
            Some(1000)
        );
        assert_eq!(
            TransportError::MaxRetransmissionsExceeded { seq: 1 }
                .suggested_retry_delay_ms(),
            Some(5000)
        );
        assert_eq!(
            TransportError::FrameEncodeFailed { reason: "x".to_string() }
                .suggested_retry_delay_ms(),
            Some(100)
        );
        assert_eq!(
            TransportError::InvalidFrame { reason: "x".to_string() }
                .suggested_retry_delay_ms(),
            None
        );
    }

    #[test]
    fn transport_error_is_permanent_and_transient() {
        assert!(TransportError::InvalidFrame { reason: "x".to_string() }.is_permanent());
        assert!(TransportError::SequenceOutOfRange { expected: 1, got: 2 }.is_permanent());
        assert!(TransportError::DuplicateSequence { seq: 1 }.is_permanent());
        assert!(TransportError::FrameTooLarge { size: 1, max: 2 }.is_permanent());
        assert!(TransportError::NotConnected.is_permanent());
        assert!(TransportError::AlreadyConnected.is_permanent());
        assert!(!TransportError::AckTimeout { seq: 1 }.is_permanent());
        assert!(TransportError::AckTimeout { seq: 1 }.is_transient());
        assert!(!TransportError::NotConnected.is_transient());
    }

    // === TransportConnectionState tests ===
    #[test]
    fn transport_connection_state_variants() {
        assert_eq!(TransportConnectionState::Disconnected.as_str(), "disconnected");
        assert_eq!(TransportConnectionState::Connecting.as_str(), "connecting");
        assert_eq!(TransportConnectionState::Connected.as_str(), "connected");
        assert_eq!(TransportConnectionState::Disconnecting.as_str(), "disconnecting");
        assert_eq!(TransportConnectionState::Failed.as_str(), "failed");
        assert!(TransportConnectionState::Connected.is_connected());
        assert!(!TransportConnectionState::Disconnected.is_connected());
        assert!(TransportConnectionState::Connected.can_send());
        assert!(!TransportConnectionState::Disconnected.can_send());
        assert!(TransportConnectionState::Failed.is_terminal());
        assert!(TransportConnectionState::Disconnected.is_terminal());
        assert!(!TransportConnectionState::Connected.is_terminal());
    }

    // === TransportHealth tests ===
    #[test]
    fn transport_health_variants() {
        assert_eq!(TransportHealth::Healthy.as_str(), "healthy");
        assert_eq!(TransportHealth::Degraded.as_str(), "degraded");
        assert_eq!(TransportHealth::Unhealthy.as_str(), "unhealthy");
        assert_eq!(TransportHealth::Unknown.as_str(), "unknown");
        assert!(TransportHealth::Healthy.is_healthy());
        assert!(!TransportHealth::Degraded.is_healthy());
        assert!(TransportHealth::Degraded.is_degraded());
        assert!(TransportHealth::Unhealthy.is_unhealthy());
    }

    // === TransportHealthMonitor tests ===
    #[test]
    fn transport_health_monitor_new_and_default() {
        let monitor = TransportHealthMonitor::new();
        assert_eq!(monitor.state, TransportConnectionState::Disconnected);
        assert_eq!(monitor.health, TransportHealth::Unknown);
        assert_eq!(monitor.consecutive_failures, 0);
        assert_eq!(monitor.total_sent, 0);
        assert_eq!(monitor.total_received, 0);
        assert_eq!(monitor.total_errors, 0);
        assert!(!monitor.is_healthy());
        assert!(!monitor.is_unhealthy());
    }

    #[test]
    fn transport_health_monitor_record_success() {
        let mut monitor = TransportHealthMonitor::new();
        monitor.record_success(100);
        assert_eq!(monitor.state, TransportConnectionState::Connected);
        assert_eq!(monitor.health, TransportHealth::Healthy);
        assert_eq!(monitor.total_sent, 100);
        assert_eq!(monitor.consecutive_failures, 0);
        assert!(monitor.is_healthy());
    }

    #[test]
    fn transport_health_monitor_record_failure() {
        let mut monitor = TransportHealthMonitor::new();
        let err = TransportError::AckTimeout { seq: 1 };
        monitor.record_failure(&err);
        assert_eq!(monitor.consecutive_failures, 1);
        assert_eq!(monitor.total_errors, 1);
    }

    #[test]
    fn transport_health_monitor_unhealthy_after_many_failures() {
        let mut monitor = TransportHealthMonitor::new();
        let err = TransportError::AckTimeout { seq: 1 };
        for _ in 0..10 {
            monitor.record_failure(&err);
        }
        assert_eq!(monitor.state, TransportConnectionState::Failed);
        assert_eq!(monitor.health, TransportHealth::Unhealthy);
        assert!(monitor.is_unhealthy());
    }

    #[test]
    fn transport_health_monitor_record_received() {
        let mut monitor = TransportHealthMonitor::new();
        monitor.record_received(500);
        assert_eq!(monitor.total_received, 500);
    }

    #[test]
    fn transport_health_monitor_reset() {
        let mut monitor = TransportHealthMonitor::new();
        monitor.record_success(100);
        monitor.record_failure(&TransportError::AckTimeout { seq: 1 });
        monitor.reset();
        assert_eq!(monitor.state, TransportConnectionState::Disconnected);
        assert_eq!(monitor.health, TransportHealth::Unknown);
        assert_eq!(monitor.consecutive_failures, 0);
        assert_eq!(monitor.total_sent, 0);
        assert_eq!(monitor.total_received, 0);
        assert_eq!(monitor.total_errors, 0);
    }

    // === TransportWindowManager tests ===
    #[test]
    fn transport_window_manager_new_and_default() {
        let wm = TransportWindowManager::default();
        assert_eq!(wm.window_size, 65536);
        assert_eq!(wm.bytes_in_flight, 0);
        assert_eq!(wm.max_window_size, 1048576);
        assert_eq!(wm.min_window_size, 1024);
    }

    #[test]
    fn transport_window_manager_can_send() {
        let wm = TransportWindowManager::new();
        assert!(wm.can_send(100));
        assert!(!wm.can_send(70000));
    }

    #[test]
    fn transport_window_manager_add_remove_in_flight() {
        let mut wm = TransportWindowManager::new();
        wm.add_in_flight(1000);
        assert_eq!(wm.bytes_in_flight, 1000);
        wm.remove_in_flight(500);
        assert_eq!(wm.bytes_in_flight, 500);
    }

    #[test]
    fn transport_window_manager_increase_decrease() {
        let mut wm = TransportWindowManager::new();
        wm.increase_window(2.0);
        assert_eq!(wm.window_size, 131072);
        wm.decrease_window(0.5);
        assert_eq!(wm.window_size, 65536);
    }

    #[test]
    fn transport_window_manager_utilization() {
        let mut wm = TransportWindowManager::new();
        assert!((wm.window_utilization() - 0.0).abs() < f64::EPSILON);
        wm.add_in_flight(32768);
        assert!((wm.window_utilization() - 0.5).abs() < f64::EPSILON);
        assert!(!wm.is_window_full());
        wm.add_in_flight(32768);
        assert!(wm.is_window_full());
    }

    // === TransportRetryManager tests ===
    #[test]
    fn transport_retry_manager_new_and_default() {
        let rm = TransportRetryManager::default();
        assert_eq!(rm.max_retries, 3);
        assert_eq!(rm.current_retry_count(), 0);
        assert!(!rm.is_exhausted());
    }

    #[test]
    fn transport_retry_manager_should_retry() {
        let rm = TransportRetryManager::new(3);
        assert!(rm.should_retry(&TransportError::AckTimeout { seq: 1 }));
        assert!(!rm.should_retry(&TransportError::NotConnected));
    }

    #[test]
    fn transport_retry_manager_increment_and_exhaust() {
        let mut rm = TransportRetryManager::new(2);
        rm.increment_retry();
        assert_eq!(rm.current_retry_count(), 1);
        assert!(!rm.is_exhausted());
        rm.increment_retry();
        assert_eq!(rm.current_retry_count(), 2);
        assert!(rm.is_exhausted());
    }

    #[test]
    fn transport_retry_manager_reset() {
        let mut rm = TransportRetryManager::new(3);
        rm.increment_retry();
        rm.increment_retry();
        rm.reset_retry();
        assert_eq!(rm.current_retry_count(), 0);
        assert!(!rm.is_exhausted());
    }

    #[test]
    fn transport_retry_manager_delay() {
        let rm = TransportRetryManager::new(3);
        assert_eq!(rm.next_delay_ms(), 100);
        assert_eq!(rm.clamp_delay(50000), 30000);
        assert_eq!(rm.clamp_delay(100), 100);
    }

    // === TransportRateLimiter tests ===
    #[test]
    fn transport_rate_limiter_consume() {
        let rl = TransportRateLimiter::new(5, 1000);
        assert_eq!(rl.available_tokens(), 5);
        assert!(rl.try_consume());
        assert_eq!(rl.available_tokens(), 4);
        assert!(rl.try_consume());
        assert!(rl.try_consume());
        assert!(rl.try_consume());
        assert!(rl.try_consume());
        assert!(!rl.try_consume());
    }

    #[test]
    fn transport_rate_limiter_consume_n() {
        let rl = TransportRateLimiter::new(10, 1000);
        assert_eq!(rl.try_consume_n(3), 3);
        assert_eq!(rl.try_consume_n(3), 3);
        assert_eq!(rl.try_consume_n(3), 4);
        assert_eq!(rl.try_consume_n(3), 0);
    }

    #[test]
    fn transport_rate_limiter_default() {
        let rl = TransportRateLimiter::default();
        assert_eq!(rl.max_tokens(), 1000);
        assert_eq!(rl.available_tokens(), 1000);
    }

    // === TransportFramePool tests ===
    #[test]
    fn transport_frame_pool_allocate_and_release() {
        let pool = TransportFramePool::new(4);
        assert_eq!(pool.pool_size(), 0);
        let buf = pool.allocate(100);
        assert_eq!(buf.capacity(), 100);
        assert_eq!(pool.total_allocated(), 1);
        pool.release(buf);
        assert_eq!(pool.pool_size(), 1);
        assert_eq!(pool.total_released(), 1);
    }

    #[test]
    fn transport_frame_pool_reuse() {
        let pool = TransportFramePool::new(4);
        let buf1 = pool.allocate(50);
        pool.release(buf1);
        let buf2 = pool.allocate(50);
        assert_eq!(buf2.capacity(), 50);
    }

    #[test]
    fn transport_frame_pool_default() {
        let pool = TransportFramePool::default();
        assert_eq!(pool.max_pool_size, 64);
    }

    // === TransportEndpointBinding tests ===
    #[test]
    fn transport_endpoint_binding_bind_unbind() {
        let ep = Endpoint::new(CarrierType::Loopback, vec![1, 2, 3]).unwrap();
        let mut binding = TransportEndpointBinding::new(ep);
        assert!(!binding.is_bound());
        binding.bind(100);
        assert!(binding.is_bound());
        assert_eq!(binding.bound_duration_ms(200), Some(100));
        binding.unbind();
        assert!(!binding.is_bound());
    }

    // === TransportSequenceWindow tests ===
    #[test]
    fn transport_sequence_window_new_and_next() {
        let mut window = TransportSequenceWindow::new(4);
        assert_eq!(window.next_sequence(), 0);
        assert_eq!(window.next_sequence(), 1);
        assert_eq!(window.next_sequence(), 2);
    }

    #[test]
    fn transport_sequence_window_can_send() {
        let mut window = TransportSequenceWindow::new(2);
        assert!(window.can_send());
        window.mark_sent(0, vec![1]);
        assert!(window.can_send());
        window.mark_sent(1, vec![2]);
        assert!(!window.can_send());
    }

    #[test]
    fn transport_sequence_window_mark_acked() {
        let mut window = TransportSequenceWindow::new(4);
        window.mark_sent(0, vec![1]);
        window.mark_sent(1, vec![2]);
        assert_eq!(window.unacked_count(), 2);
        window.mark_acked(0);
        assert_eq!(window.unacked_count(), 1);
        assert!(window.is_acked(0));
    }

    #[test]
    fn transport_sequence_window_default() {
        let window = TransportSequenceWindow::default();
        assert_eq!(window.window_size, 1024);
    }

    // === TransportStats extended tests ===
    #[test]
    fn transport_stats_retransmission_and_ack_rates() {
        let mut stats = TransportStats::default();
        stats.frames_framed = 100;
        stats.frames_retransmitted = 10;
        assert!((stats.retransmission_rate() - 0.1).abs() < f64::EPSILON);
        stats.acks_sent = 50;
        stats.acks_received = 45;
        assert!((stats.ack_success_rate() - 0.9).abs() < f64::EPSILON);
    }

    #[test]
    fn transport_stats_error_rate() {
        let mut stats = TransportStats::default();
        stats.frames_framed = 100;
        stats.frames_deframed = 100;
        stats.frames_dropped = 5;
        stats.checksum_errors = 3;
        assert!((stats.error_rate() - (8.0 / 200.0)).abs() < f64::EPSILON);
    }

    // === DeliverySemantics extended tests ===
    #[test]
    fn delivery_semantics_all_properties() {
        assert!(!DeliverySemantics::BestEffort.guarantees_delivery());
        assert!(!DeliverySemantics::BestEffort.guarantees_ordering());
        assert!(DeliverySemantics::AtLeastOnce.guarantees_delivery());
        assert!(!DeliverySemantics::AtLeastOnce.guarantees_ordering());
        assert!(!DeliverySemantics::AtMostOnce.guarantees_delivery());
        assert!(!DeliverySemantics::AtMostOnce.guarantees_ordering());
        assert!(DeliverySemantics::ExactlyOnce.guarantees_delivery());
        assert!(DeliverySemantics::ExactlyOnce.guarantees_ordering());
    }

    // === TransportStream tests ===
    #[test]
    fn transport_stream_new_and_sequence() {
        let ep = Endpoint::new(CarrierType::Loopback, vec![1, 2, 3]).unwrap();
        let stream = TransportStream::new(0, ep, DeliverySemantics::ExactlyOnce);
        assert_eq!(stream.stream_id, 0);
        assert_eq!(stream.send_sequence(), 0);
        assert_eq!(stream.send_sequence(), 1);
        assert_eq!(stream.bytes_sent(), 0);
        assert_eq!(stream.bytes_received(), 0);
        assert!(stream.is_healthy());
    }

    #[test]
    fn transport_stream_bytes() {
        let ep = Endpoint::new(CarrierType::Loopback, vec![1]).unwrap();
        let stream = TransportStream::new(1, ep, DeliverySemantics::AtMostOnce);
        stream.add_bytes_sent(1000);
        stream.add_bytes_received(500);
        assert_eq!(stream.bytes_sent(), 1000);
        assert_eq!(stream.bytes_received(), 500);
    }

    #[test]
    fn transport_stream_acknowledge() {
        let ep = Endpoint::new(CarrierType::Loopback, vec![1]).unwrap();
        let stream = TransportStream::new(2, ep, DeliverySemantics::AtLeastOnce);
        stream.acknowledge_received(42);
        assert!(stream.is_healthy());
    }

    // === TransportMultiplexer tests ===
    #[test]
    fn transport_multiplexer_open_close() {
        let mux = TransportMultiplexer::new();
        assert_eq!(mux.stream_count(), 0);
        let ep = Endpoint::new(CarrierType::Loopback, vec![1]).unwrap();
        let id1 = mux.open_stream(ep.clone(), DeliverySemantics::AtMostOnce);
        let id2 = mux.open_stream(ep, DeliverySemantics::ExactlyOnce);
        assert_eq!(id1, 0);
        assert_eq!(id2, 1);
        assert_eq!(mux.stream_count(), 2);
        assert!(mux.get_stream(id1).is_some());
        mux.close_stream(id1);
        assert_eq!(mux.stream_count(), 1);
        assert!(mux.get_stream(id1).is_none());
    }

    // === TransportFragmenter tests ===
    #[test]
    fn transport_fragmenter_split() {
        let frag = TransportFragmenter::new(3);
        let payload = vec![1, 2, 3, 4, 5, 6, 7];
        let fragments = frag.fragment(&payload);
        assert_eq!(fragments.len(), 3);
        assert_eq!(fragments[0], vec![1, 2, 3]);
        assert_eq!(fragments[1], vec![4, 5, 6]);
        assert_eq!(fragments[2], vec![7]);
    }

    #[test]
    fn transport_fragmenter_small_payload() {
        let frag = TransportFragmenter::new(100);
        let payload = vec![1, 2, 3];
        let fragments = frag.fragment(&payload);
        assert_eq!(fragments.len(), 1);
    }

    #[test]
    fn transport_fragmenter_empty() {
        let frag = TransportFragmenter::new(10);
        let fragments = frag.fragment(&[]);
        assert_eq!(fragments.len(), 0);
    }

    // === TransportReassembler tests ===
    #[test]
    fn transport_reassembler_add_and_reassemble() {
        let reassembler = TransportReassembler::new();
        reassembler.add_fragment(1, vec![1, 2, 3], 0);
        reassembler.add_fragment(1, vec![4, 5, 6], 1);
        let result = reassembler.reassemble(1).unwrap();
        assert_eq!(result.len(), 6);
        assert_eq!(reassembler.reassembled_count(), 1);
    }

    #[test]
    fn transport_reassembler_missing_stream() {
        let reassembler = TransportReassembler::new();
        assert!(reassembler.reassemble(999).is_none());
    }

    // === TransportClock tests ===
    #[test]
    fn transport_clock_tick() {
        let mut clock = TransportClock::new();
        assert_eq!(clock.now_ms(), 0);
        assert_eq!(clock.tick_count(), 0);
        clock.tick(100);
        assert_eq!(clock.now_ms(), 100);
        assert_eq!(clock.tick_count(), 1);
        clock.tick(50);
        assert_eq!(clock.now_ms(), 150);
        assert_eq!(clock.tick_count(), 2);
    }

    // === TransportDatagram tests ===
    #[test]
    fn transport_datagram_data_and_control() {
        let dg = TransportDatagram::data(1, vec![1, 2, 3]);
        assert!(dg.is_data());
        assert!(!dg.is_control());
        assert_eq!(dg.size(), 27);
        let dg2 = TransportDatagram::control(2);
        assert!(!dg2.is_data());
        assert!(dg2.is_control());
        assert_eq!(dg2.size(), 24);
    }

    // === TransportStreamPool tests ===
    #[test]
    fn transport_stream_pool_open_close() {
        let pool = TransportStreamPool::new(2);
        let ep = Endpoint::new(CarrierType::Loopback, vec![1]).unwrap();
        assert!(pool.open(0, ep.clone(), DeliverySemantics::AtMostOnce));
        assert!(pool.open(1, ep.clone(), DeliverySemantics::AtMostOnce));
        assert!(!pool.open(2, ep, DeliverySemantics::AtMostOnce));
        assert_eq!(pool.stream_count(), 2);
        assert!(pool.is_full());
        assert!(pool.close(0));
        assert_eq!(pool.stream_count(), 1);
        assert!(!pool.is_full());
        assert!(!pool.close(999));
    }

    // === TransportCongestionSignal tests ===
    #[test]
    fn transport_congestion_signal_variants() {
        assert_eq!(TransportCongestionSignal::None.as_str(), "none");
        assert_eq!(TransportCongestionSignal::Mild.as_str(), "mild");
        assert_eq!(TransportCongestionSignal::Moderate.as_str(), "moderate");
        assert_eq!(TransportCongestionSignal::Severe.as_str(), "severe");
        assert_eq!(TransportCongestionSignal::Critical.as_str(), "critical");
        assert!(!TransportCongestionSignal::None.should_throttle());
        assert!(!TransportCongestionSignal::Mild.should_throttle());
        assert!(TransportCongestionSignal::Moderate.should_throttle());
        assert!(TransportCongestionSignal::Severe.should_throttle());
        assert!(TransportCongestionSignal::Critical.should_throttle());
        assert!(!TransportCongestionSignal::Severe.should_drop());
        assert!(TransportCongestionSignal::Critical.should_drop());
    }

    // === TransportBackpressureManager tests ===
    #[test]
    fn transport_backpressure_manager_signal_levels() {
        let bpm = TransportBackpressureManager::new(100);
        bpm.update_queue_depth(0);
        assert_eq!(bpm.current_signal(), TransportCongestionSignal::None);
        bpm.update_queue_depth(20);
        assert_eq!(bpm.current_signal(), TransportCongestionSignal::Mild);
        bpm.update_queue_depth(60);
        assert_eq!(bpm.current_signal(), TransportCongestionSignal::Moderate);
        bpm.update_queue_depth(80);
        assert_eq!(bpm.current_signal(), TransportCongestionSignal::Severe);
        bpm.update_queue_depth(100);
        assert_eq!(bpm.current_signal(), TransportCongestionSignal::Critical);
        assert!(bpm.should_drop());
    }

    #[test]
    fn transport_backpressure_manager_utilization() {
        let bpm = TransportBackpressureManager::new(1000);
        bpm.update_queue_depth(500);
        assert!((bpm.queue_utilization() - 0.5).abs() < f64::EPSILON);
    }

    // === TransportFlowControl tests ===
    #[test]
    fn transport_flow_control_open_and_restricted() {
        let mut fc = TransportFlowControl::new(1000);
        assert_eq!(fc.state.get(), TransportFlowState::Open);
        assert!(fc.can_send(500));
        fc.add_in_flight(1000);
        assert_eq!(fc.state.get(), TransportFlowState::Restricted);
        assert!(!fc.can_send(1));
        fc.release_in_flight(600);
        assert_eq!(fc.state.get(), TransportFlowState::Open);
    }

    #[test]
    fn transport_flow_control_close() {
        let mut fc = TransportFlowControl::new(1000);
        fc.close();
        assert_eq!(fc.state.get(), TransportFlowState::Closed);
        assert!(!fc.state.get().can_send());
    }

    #[test]
    fn transport_flow_control_window_utilization() {
        let mut fc = TransportFlowControl::new(1000);
        fc.add_in_flight(250);
        assert!((fc.window_utilization() - 0.25).abs() < f64::EPSILON);
    }

    // === TransportCreditManager tests ===
    #[test]
    fn transport_credit_manager_consume_and_issue() {
        let mut cm = TransportCreditManager::new(10, 100);
        assert!(cm.consume_credit(500));
        assert_eq!(cm.available_credits, 5);
        assert!(!cm.consume_credit(1000));
        cm.issue_credit(3);
        assert_eq!(cm.available_credits, 8);
    }

    #[test]
    fn transport_credit_manager_exhausted() {
        let mut cm = TransportCreditManager::new(1, 1000);
        assert!(cm.consume_credit(1000));
        assert!(cm.is_exhausted());
        assert!(!cm.consume_credit(1));
    }

    #[test]
    fn transport_credit_manager_ratio() {
        let mut cm = TransportCreditManager::new(100, 100);
        assert!((cm.credit_ratio() - 1.0).abs() < f64::EPSILON);
        cm.consume_credit(5000);
        assert!((cm.credit_ratio() - 0.5).abs() < f64::EPSILON);
    }

    // === TransportPriorityQueue tests ===
    #[test]
    fn transport_priority_queue_enqueue_dequeue() {
        let pq = TransportPriorityQueue::new(10);
        assert_eq!(pq.len(), 0);
        assert!(pq.is_empty());
        pq.enqueue(TransportPriorityQueueItem { priority: 3, sequence: 1, payload: vec![1] });
        pq.enqueue(TransportPriorityQueueItem { priority: 1, sequence: 2, payload: vec![2] });
        pq.enqueue(TransportPriorityQueueItem { priority: 2, sequence: 3, payload: vec![3] });
        assert_eq!(pq.len(), 3);
        let item = pq.dequeue().unwrap();
        assert_eq!(item.priority, 1);
        let item = pq.dequeue().unwrap();
        assert_eq!(item.priority, 2);
        let item = pq.dequeue().unwrap();
        assert_eq!(item.priority, 3);
    }

    #[test]
    fn transport_priority_queue_full() {
        let pq = TransportPriorityQueue::new(1);
        pq.enqueue(TransportPriorityQueueItem { priority: 1, sequence: 0, payload: vec![] });
        assert!(pq.is_full());
        assert!(!pq.enqueue(TransportPriorityQueueItem { priority: 2, sequence: 0, payload: vec![] }));
    }

    // === TransportError extended tests additional ===
    #[test]
    fn transport_error_all_categories() {
        assert_eq!(
            TransportError::FlushFailed { reason: "x".to_string() }.category(),
            "flush_failed"
        );
        assert_eq!(
            TransportError::ResetFailed { reason: "x".to_string() }.category(),
            "reset_failed"
        );
        assert_eq!(
            TransportError::DeliverySemanticsMismatch { expected: DeliverySemantics::ExactlyOnce, got: DeliverySemantics::AtMostOnce }.category(),
            "delivery_semantics_mismatch"
        );
        assert_eq!(
            TransportError::AlreadyConnected.category(),
            "already_connected"
        );
    }

    #[test]
    fn transport_error_suggested_retry_all() {
        assert_eq!(TransportError::FlushFailed { reason: "x".to_string() }.suggested_retry_delay_ms(), None);
        assert_eq!(TransportError::ResetFailed { reason: "x".to_string() }.suggested_retry_delay_ms(), None);
        assert_eq!(TransportError::NotConnected.suggested_retry_delay_ms(), None);
    }
}

// Helper for test checksum computation
impl FrameHeader {
    pub fn compute_checksum_for_test(payload: &[u8]) -> u32 {
        let mut hash: u32 = 0x811c9dc5;
        for &byte in payload {
            hash ^= byte as u32;
            hash = hash.wrapping_mul(0x01000193);
        }
        hash
    }
}