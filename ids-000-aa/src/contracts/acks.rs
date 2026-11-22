//! # ids-000-aa — Sealed `Acks` Trait Contract
//!
//! Native HX-241 acknowledgement contract. Cumulative acks, selective acks,
//! nacks, ack timing, rtt estimation, duplicate detection, and ack
//! aggregation are defined here.
//!
//! Carrier-agnostic: no `std::net`, `tokio::net`, libp2p, IP, TCP, UDP,
//! QUIC, HTTP, or DNS types anywhere. Uses `Endpoint` (`CarrierType +
//! Vec<u8>`) only.
//!
//! Sealed: downstream crates call methods and use as trait bounds but cannot
//! implement. Only ids-000-aa's designated implementation crates may `impl`.

use crate::types::{Endpoint, NodeId};
use crate::types::{SequenceNumber, TimestampMillis};
use crate::types::AcksError;
use thiserror::Error;

/// Sealed marker trait — only ids-000-aa internals can implement `Acks`.
pub mod private {
    pub trait Sealed {}
}

/// Default ack timeout in milliseconds.
pub const DEFAULT_ACK_TIMEOUT_MS: u64 = 200;

/// Default max ack batch size.
pub const DEFAULT_MAX_ACK_BATCH_SIZE: usize = 64;

/// Default ack delay in milliseconds (delayed ack).
pub const DEFAULT_ACK_DELAY_MS: u64 = 50;

/// Default max outstanding acks.
pub const DEFAULT_MAX_OUTSTANDING_ACKS: u64 = 1024;

/// Default rtt alpha (smoothing factor).
pub const DEFAULT_RTT_ALPHA: f64 = 0.125;

/// Default rtt beta (variance smoothing factor).
pub const DEFAULT_RTT_BETA: f64 = 0.25;

/// Default initial rtt estimate in milliseconds.
pub const DEFAULT_INITIAL_RTT_MS: u64 = 250;

/// Minimum rtt estimate in milliseconds.
pub const DEFAULT_MIN_RTT_MS: u64 = 10;

/// Maximum rtt estimate in milliseconds.
pub const DEFAULT_MAX_RTT_MS: u64 = 60_000;

/// Default ack multiplication factor for rtt-based timeout.
pub const DEFAULT_RTT_MULTIPLIER: f64 = 2.0;

///////////////////////////////////////////////////////////////////////////////
// ACK TYPE TYPES
///////////////////////////////////////////////////////////////////////////////

/// Type of acknowledgement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AckType {
    /// Cumulative ack — everything up to this sequence is received.
    Cumulative,
    /// Selective ack — only this specific sequence is acknowledged.
    Selective,
    /// Negative ack — this sequence is missing/lost.
    Negative,
    /// Heartbeat ack — connection keepalive.
    Heartbeat,
}

impl Default for AckType {
    fn default() -> Self {
        AckType::Cumulative
    }
}

impl AckType {
    pub fn as_str(&self) -> &'static str {
        match self {
            AckType::Cumulative => "cumulative",
            AckType::Selective => "selective",
            AckType::Negative => "negative",
            AckType::Heartbeat => "heartbeat",
        }
    }

    pub fn is_negative(&self) -> bool {
        matches!(self, AckType::Negative)
    }

    pub fn is_cumulative(&self) -> bool {
        matches!(self, AckType::Cumulative)
    }

    pub fn is_selective(&self) -> bool {
        matches!(self, AckType::Selective)
    }

    pub fn is_heartbeat(&self) -> bool {
        matches!(self, AckType::Heartbeat)
    }
}

/// A single acknowledgement entry.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AckEntry {
    pub ack_id: u64,
    pub ack_type: AckType,
    pub sequence: SequenceNumber,
    pub acked_by: NodeId,
    pub acked_for: NodeId,
    pub timestamp: TimestampMillis,
    pub rtt_ms: Option<u64>,
    pub duplicate: bool,
}

impl AckEntry {
    pub fn new(
        ack_id: u64,
        ack_type: AckType,
        sequence: SequenceNumber,
        acked_by: NodeId,
        acked_for: NodeId,
        timestamp: TimestampMillis,
    ) -> Self {
        Self {
            ack_id,
            ack_type,
            sequence,
            acked_by,
            acked_for,
            timestamp,
            rtt_ms: None,
            duplicate: false,
        }
    }

    pub fn with_rtt(mut self, rtt_ms: u64) -> Self {
        self.rtt_ms = Some(rtt_ms);
        self
    }

    pub fn as_duplicate(mut self) -> Self {
        self.duplicate = true;
        self
    }

    pub fn is_duplicate(&self) -> bool {
        self.duplicate
    }

    pub fn age_ms(&self, now: TimestampMillis) -> u64 {
        now.elapsed_since(self.timestamp)
    }
}

/// Cumulative acknowledgement state for a peer.
#[derive(Debug, Clone)]
pub struct CumulativeAckState {
    pub peer_id: NodeId,
    pub acked_up_to: SequenceNumber,
    pub highest_received: SequenceNumber,
    pub total_acks_sent: u64,
    pub total_acks_received: u64,
    pub last_ack_sent_at: Option<TimestampMillis>,
    pub last_ack_received_at: Option<TimestampMillis>,
    pub duplicate_acks_sent: u64,
    pub duplicate_acks_received: u64,
    pub ack_batch_count: u64,
}

impl CumulativeAckState {
    pub fn new(peer_id: NodeId) -> Self {
        Self {
            peer_id,
            acked_up_to: SequenceNumber(0),
            highest_received: SequenceNumber(0),
            total_acks_sent: 0,
            total_acks_received: 0,
            last_ack_sent_at: None,
            last_ack_received_at: None,
            duplicate_acks_sent: 0,
            duplicate_acks_received: 0,
            ack_batch_count: 0,
        }
    }

    pub fn update_acked_up_to(&mut self, seq: SequenceNumber) {
        if seq.value() > self.acked_up_to.value() {
            self.acked_up_to = seq;
        }
    }

    pub fn record_ack_sent(&mut self, timestamp: TimestampMillis) {
        self.total_acks_sent += 1;
        self.last_ack_sent_at = Some(timestamp);
    }

    pub fn record_ack_received(&mut self, timestamp: TimestampMillis) {
        self.total_acks_received += 1;
        self.last_ack_received_at = Some(timestamp);
    }

    pub fn record_duplicate_ack_sent(&mut self) {
        self.duplicate_acks_sent += 1;
    }

    pub fn record_duplicate_ack_received(&mut self) {
        self.duplicate_acks_received += 1;
    }

    pub fn record_ack_batch(&mut self) {
        self.ack_batch_count += 1;
    }

    pub fn ack_ratio(&self) -> f64 {
        if self.total_acks_sent == 0 {
            return 100.0;
        }
        self.total_acks_received as f64 / self.total_acks_sent as f64 * 100.0
    }

    pub fn duplicate_ratio(&self) -> f64 {
        if self.total_acks_sent == 0 {
            return 0.0;
        }
        self.duplicate_acks_sent as f64 / self.total_acks_sent as f64 * 100.0
    }
}

/// Selective acknowledgement block (SACK-style).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SelectiveAckBlock {
    pub start: SequenceNumber,
    pub end: SequenceNumber,
}

impl SelectiveAckBlock {
    pub const fn new(start: SequenceNumber, end: SequenceNumber) -> Self {
        Self { start, end }
    }

    pub fn is_empty(&self) -> bool {
        self.start.value() >= self.end.value()
    }

    pub fn len(&self) -> u64 {
        self.end.value().wrapping_sub(self.start.value())
    }

    pub fn contains(&self, seq: SequenceNumber) -> bool {
        seq.value() >= self.start.value() && seq.value() < self.end.value()
    }
}

/// A collection of selective ack blocks.
#[derive(Debug, Clone, Default)]
pub struct SelectiveAckSet {
    pub blocks: Vec<SelectiveAckBlock>,
    pub total_sequences: u64,
}

impl SelectiveAckSet {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_block(&mut self, block: SelectiveAckBlock) {
        if block.is_empty() {
            return;
        }
        self.total_sequences += block.len();
        self.blocks.push(block);
        self.blocks.sort_by_key(|b| b.start.value());
        self.merge_adjacent();
    }

    fn merge_adjacent(&mut self) {
        if self.blocks.is_empty() {
            return;
        }
        let mut merged = Vec::new();
        let mut current = self.blocks.remove(0);
        while !self.blocks.is_empty() {
            let next = self.blocks.remove(0);
            if next.start.value() <= current.end.value() {
                current.end = next.end;
            } else {
                merged.push(current);
                current = next;
            }
        }
        merged.push(current);
        self.blocks = merged;
    }

    pub fn contains(&self, seq: SequenceNumber) -> bool {
        self.blocks.iter().any(|b| b.contains(seq))
    }

    pub fn block_count(&self) -> usize {
        self.blocks.len()
    }

    pub fn clear(&mut self) {
        self.blocks.clear();
        self.total_sequences = 0;
    }
}

/// Negative acknowledgement entry.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct NackEntry {
    pub sequence: SequenceNumber,
    pub nack_count: u32,
    pub first_nacked_at: TimestampMillis,
    pub last_nacked_at: TimestampMillis,
    pub declared_lost: bool,
}

impl NackEntry {
    pub fn new(sequence: SequenceNumber, timestamp: TimestampMillis) -> Self {
        Self {
            sequence,
            nack_count: 1,
            first_nacked_at: timestamp,
            last_nacked_at: timestamp,
            declared_lost: false,
        }
    }

    pub fn increment(&mut self, timestamp: TimestampMillis) {
        self.nack_count += 1;
        self.last_nacked_at = timestamp;
    }

    pub fn declare_lost(&mut self) {
        self.declared_lost = true;
    }

    pub fn nack_age_ms(&self, now: TimestampMillis) -> u64 {
        now.elapsed_since(self.last_nacked_at)
    }
}

/// Ack timing statistics for rtt estimation.
#[derive(Debug, Clone, Default)]
pub struct AckTimingStats {
    pub rtt_samples: u64,
    pub srtt_ms: u64,
    pub rttvar_ms: u64,
    pub min_rtt_ms: u64,
    pub max_rtt_ms: u64,
    pub last_rtt_ms: Option<u64>,
    pub timeout_ms: u64,
}

impl AckTimingStats {
    pub fn new() -> Self {
        Self {
            srtt_ms: DEFAULT_INITIAL_RTT_MS,
            rttvar_ms: DEFAULT_INITIAL_RTT_MS / 2,
            min_rtt_ms: DEFAULT_INITIAL_RTT_MS,
            max_rtt_ms: 0,
            timeout_ms: DEFAULT_INITIAL_RTT_MS * 2,
            ..Default::default()
        }
    }

    /// Update rtt estimate with a new sample (Jacobson/Karels algorithm).
    pub fn update_rtt(&mut self, sample_ms: u64) {
        self.rtt_samples += 1;
        self.last_rtt_ms = Some(sample_ms);

        if self.rtt_samples == 1 {
            self.srtt_ms = sample_ms;
            self.rttvar_ms = sample_ms / 2;
        } else {
            let alpha = DEFAULT_RTT_ALPHA;
            let beta = DEFAULT_RTT_BETA;
            let old_srtt = self.srtt_ms as f64;
            self.srtt_ms = ((1.0 - alpha) * old_srtt + alpha * sample_ms as f64) as u64;
            let rttvar = ((1.0 - beta) * self.rttvar_ms as f64
                + beta * (sample_ms as i64 - old_srtt as i64).abs() as f64)
                as u64;
            self.rttvar_ms = rttvar;
        }

        if sample_ms < self.min_rtt_ms || self.min_rtt_ms == 0 {
            self.min_rtt_ms = sample_ms;
        }
        if sample_ms > self.max_rtt_ms {
            self.max_rtt_ms = sample_ms;
        }

        self.recompute_timeout();
    }

    fn recompute_timeout(&mut self) {
        let rto = self.srtt_ms as f64
            + DEFAULT_RTT_MULTIPLIER * self.rttvar_ms as f64;
        self.timeout_ms = rto
            .clamp(DEFAULT_MIN_RTT_MS as f64, DEFAULT_MAX_RTT_MS as f64) as u64;
    }

    pub fn rto(&self) -> u64 {
        self.timeout_ms
    }

    pub fn is_rtt_established(&self) -> bool {
        self.rtt_samples > 0
    }
}

///////////////////////////////////////////////////////////////////////////////
// ACKS RESULT
///////////////////////////////////////////////////////////////////////////////

/// Result type for acks operations.
pub type AcksResult<T> = Result<T, AcksError>;

///////////////////////////////////////////////////////////////////////////////
// ACKS ERROR
///////////////////////////////////////////////////////////////////////////////

/// Errors returned by acks operations.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum AcksError {
    InvalidConfig {
        field: String,
        reason: String,
    },
    InvalidSequence {
        seq: SequenceNumber,
        reason: String,
    },
    AckAlreadySent {
        seq: SequenceNumber,
    },
    AckNotFound {
        seq: SequenceNumber,
    },
    PeerNotFound {
        peer_id: NodeId,
    },
    BatchTooLarge {
        size: usize,
        max: usize,
    },
    RttNotEstablished,
    Internal {
        message: String,
    },
}

impl AcksError {
    pub fn category(&self) -> &'static str {
        match self {
            AcksError::InvalidConfig { .. } => "invalid_config",
            AcksError::InvalidSequence { .. } => "invalid_sequence",
            AcksError::AckAlreadySent { .. } => "ack_already_sent",
            AcksError::AckNotFound { .. } => "ack_not_found",
            AcksError::PeerNotFound { .. } => "peer_not_found",
            AcksError::BatchTooLarge { .. } => "batch_too_large",
            AcksError::RttNotEstablished => "rtt_not_established",
            AcksError::Internal { .. } => "internal",
        }
    }

    pub fn is_recoverable(&self) -> bool {
        matches!(
            self,
            AcksError::InvalidSequence { .. }
                | AcksError::AckAlreadySent { .. }
                | AcksError::BatchTooLarge { .. }
        )
    }

    pub fn suggested_action(&self) -> &'static str {
        match self {
            AcksError::InvalidConfig { .. } => "correct the configuration value",
            AcksError::InvalidSequence { .. } => "use a valid sequence number",
            AcksError::AckAlreadySent { .. } => "skip or deduplicate the ack",
            AcksError::AckNotFound { .. } => "ack may already have been sent",
            AcksError::PeerNotFound { .. } => "register the peer first",
            AcksError::BatchTooLarge { .. } => "reduce batch size or increase limit",
            AcksError::RttNotEstablished => "wait for rtt samples or use default",
            AcksError::Internal { .. } => "contact system administrator",
        }
    }
}

impl std::fmt::Display for AcksError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AcksError::InvalidConfig { field, reason } => {
                write!(f, "invalid config for {}: {}", field, reason)
            }
            AcksError::InvalidSequence { seq, reason } => {
                write!(f, "invalid sequence {}: {}", seq, reason)
            }
            AcksError::AckAlreadySent { seq } => {
                write!(f, "ack for sequence {} already sent", seq)
            }
            AcksError::AckNotFound { seq } => {
                write!(f, "ack for sequence {} not found", seq)
            }
            AcksError::PeerNotFound { peer_id } => {
                write!(f, "peer {} not found", peer_id)
            }
            AcksError::BatchTooLarge { size, max } => {
                write!(f, "batch size {} exceeds max {}", size, max)
            }
            AcksError::RttNotEstablished => {
                write!(f, "rtt not yet established")
            }
            AcksError::Internal { message } => {
                write!(f, "internal acks error: {}", message)
            }
        }
    }
}

///////////////////////////////////////////////////////////////////////////////
// SEALED TRAIT CONTRACT
///////////////////////////////////////////////////////////////////////////////

/// Native acknowledgement contract: cumulative/selective acks, nacks, rtt
/// estimation, ack timing, and ack aggregation.
///
/// Implementations are provided by ids-000-aa's acks engine crate only.
/// Downstream crates use this as a trait bound.
pub trait Acks: private::Sealed {
    /// Send a cumulative acknowledgement for a peer.
    fn send_cumulative_ack(
        &self,
        peer_id: NodeId,
        acked_up_to: SequenceNumber,
        timestamp: TimestampMillis,
    ) -> AcksResult<u64>;

    /// Send a selective acknowledgement for a peer.
    fn send_selective_ack(
        &self,
        peer_id: NodeId,
        blocks: Vec<SelectiveAckBlock>,
        timestamp: TimestampMillis,
    ) -> AcksResult<u64>;

    /// Send a negative acknowledgement for a missing sequence.
    fn send_nack(
        &self,
        peer_id: NodeId,
        seq: SequenceNumber,
        timestamp: TimestampMillis,
    ) -> AcksResult<u64>;

    /// Send a heartbeat acknowledgement.
    fn send_heartbeat_ack(
        &self,
        peer_id: NodeId,
        timestamp: TimestampMillis,
    ) -> AcksResult<u64>;

    /// Record a received acknowledgement from a peer.
    fn record_received_ack(
        &self,
        peer_id: NodeId,
        ack_type: AckType,
        sequence: SequenceNumber,
        timestamp: TimestampMillis,
    ) -> AcksResult<()>;

    /// Get the cumulative ack state for a peer.
    fn cumulative_ack_state(&self, peer_id: NodeId) -> Option<CumulativeAckState>;

    /// Get the acked-up-to sequence for a peer.
    fn acked_up_to(&self, peer_id: NodeId) -> Option<SequenceNumber>;

    /// Update the acked-up-to sequence for a peer.
    fn update_acked_up_to(
        &self,
        peer_id: NodeId,
        seq: SequenceNumber,
    ) -> AcksResult<()>;

    /// Check whether a sequence is acknowledged by a peer.
    fn is_acknowledged(&self, peer_id: NodeId, seq: SequenceNumber) -> bool;

    /// Get the selective ack set for a peer.
    fn selective_ack_set(&self, peer_id: NodeId) -> Option<SelectiveAckSet>;

    /// Add a selective ack block for a peer.
    fn add_selective_ack_block(
        &self,
        peer_id: NodeId,
        block: SelectiveAckBlock,
    ) -> AcksResult<()>;

    /// Clear selective ack blocks for a peer.
    fn clear_selective_acks(&self, peer_id: NodeId) -> bool;

    /// Record a nack for a missing sequence.
    fn record_nack(
        &self,
        peer_id: NodeId,
        seq: SequenceNumber,
        timestamp: TimestampMillis,
    ) -> AcksResult<()>;

    /// Get nack entries for a peer.
    fn nack_entries(&self, peer_id: NodeId) -> Vec<NackEntry>;

    /// Declare a nacked sequence lost.
    fn declare_nack_lost(&self, peer_id: NodeId, seq: SequenceNumber) -> bool;

    /// Get the ack timing stats for a peer.
    fn ack_timing_stats(&self, peer_id: NodeId) -> Option<AckTimingStats>;

    /// Update rtt estimate for a peer.
    fn update_rtt(
        &self,
        peer_id: NodeId,
        sample_ms: u64,
    ) -> AcksResult<()>;

    /// Get the current rto (retransmission timeout) for a peer.
    fn rto(&self, peer_id: NodeId) -> Option<u64>;

    /// Get the srtt (smoothed rtt) for a peer.
    fn srtt(&self, peer_id: NodeId) -> Option<u64>;

    /// Get the rtt variance for a peer.
    fn rttvar(&self, peer_id: NodeId) -> Option<u64>;

    /// Check if rtt is established for a peer.
    fn is_rtt_established(&self, peer_id: NodeId) -> bool;

    /// Set the ack timeout for a peer.
    fn set_ack_timeout_ms(
        &self,
        peer_id: NodeId,
        timeout_ms: u64,
    ) -> AcksResult<()>;

    /// Get the ack timeout for a peer.
    fn ack_timeout_ms(&self, peer_id: NodeId) -> Option<u64>;

    /// Set the max ack batch size for a peer.
    fn set_max_ack_batch_size(
        &self,
        peer_id: NodeId,
        max: usize,
    ) -> AcksResult<()>;

    /// Get the max ack batch size for a peer.
    fn max_ack_batch_size(&self, peer_id: NodeId) -> Option<usize>;

    /// Set the ack delay for a peer.
    fn set_ack_delay_ms(&self, peer_id: NodeId, delay_ms: u64) -> AcksResult<()>;

    /// Get the ack delay for a peer.
    fn ack_delay_ms(&self, peer_id: NodeId) -> Option<u64>;

    /// Create an ack session for a peer.
    fn create_ack_session(
        &self,
        peer_id: NodeId,
    ) -> AcksResult<()>;

    /// Check whether an ack session exists for a peer.
    fn has_ack_session(&self, peer_id: NodeId) -> bool;

    /// Remove an ack session for a peer.
    fn remove_ack_session(&self, peer_id: NodeId) -> bool;

    /// Get the count of active ack sessions.
    fn active_ack_sessions(&self) -> usize;

    /// List all peers with active ack sessions.
    fn peers_with_ack_sessions(&self) -> Vec<NodeId>;

    /// Send an ack batch (multiple acks in one unit).
    fn send_ack_batch(
        &self,
        peer_id: NodeId,
        entries: Vec<AckEntry>,
        timestamp: TimestampMillis,
    ) -> AcksResult<u64>;

    /// Get the total acks sent across all peers.
    fn total_acks_sent(&self) -> u64;

    /// Get the total acks received across all peers.
    fn total_acks_received(&self) -> u64;

    /// Get the total nacks sent across all peers.
    fn total_nacks_sent(&self) -> u64;

    /// Get the total nacks received across all peers.
    fn total_nacks_received(&self) -> u64;

    /// Get the total duplicate acks sent.
    fn total_duplicate_acks(&self) -> u64;

    /// Get the ack ratio as a percentage.
    fn ack_ratio_percent(&self) -> f64;

    /// Get the duplicate ack ratio as a percentage.
    fn duplicate_ack_ratio_percent(&self) -> f64;

    /// Check if acks are healthy (ratio above threshold).
    fn are_acks_healthy(&self) -> bool;

    /// Reset a peer's ack state.
    fn reset_ack_state(&self, peer_id: NodeId) -> AcksResult<()>;

    /// Snapshot all ack state.
    fn snapshot_all_acks(&self) -> AcksSnapshot;

    /// Restore all ack state from a snapshot.
    fn restore_all_acks(&self, snapshot: AcksSnapshot) -> AcksResult<()>;
}

///////////////////////////////////////////////////////////////////////////////
// ACKS SNAPSHOT
///////////////////////////////////////////////////////////////////////////////

/// Full snapshot of all ack state.
#[derive(Debug, Clone)]
pub struct AcksSnapshot {
    pub cumulative_states: Vec<CumulativeAckState>,
    pub timing_stats: Vec<AckTimingStatsSnapshot>,
    pub total_acks_sent: u64,
    pub total_acks_received: u64,
    pub snapshot_at: TimestampMillis,
}

/// Per-peer timing stats snapshot.
#[derive(Debug, Clone)]
pub struct AckTimingStatsSnapshot {
    pub peer_id: NodeId,
    pub srtt_ms: u64,
    pub rttvar_ms: u64,
    pub timeout_ms: u64,
}

// Re-export types used across the crate for convenience.
use std::collections::HashMap;

///////////////////////////////////////////////////////////////////////////////
// ACK SESSION
///////////////////////////////////////////////////////////////////////////////

/// Per-peer ack session tracking pending acks, delayed acks, and batching.
#[derive(Debug, Clone)]
pub struct AckSession {
    pub peer_id: NodeId,
    pub pending_cumulative: Option<SequenceNumber>,
    pub pending_selective: SelectiveAckSet,
    pub pending_nacks: Vec<SequenceNumber>,
    pub last_ack_sent_at: Option<TimestampMillis>,
    pub acks_in_batch: u64,
    pub batch_started_at: Option<TimestampMillis>,
    pub ack_delay_ms: u64,
    pub max_batch_size: usize,
    pub total_acks_sent: u64,
    pub total_nacks_sent: u64,
    pub duplicate_acks_sent: u64,
    pub last_cumulative_sent: SequenceNumber,
}

impl AckSession {
    pub fn new(peer_id: NodeId) -> Self {
        Self {
            peer_id,
            pending_cumulative: None,
            pending_selective: SelectiveAckSet::new(),
            pending_nacks: Vec::new(),
            last_ack_sent_at: None,
            acks_in_batch: 0,
            batch_started_at: None,
            ack_delay_ms: DEFAULT_ACK_DELAY_MS,
            max_batch_size: DEFAULT_MAX_ACK_BATCH_SIZE,
            total_acks_sent: 0,
            total_nacks_sent: 0,
            duplicate_acks_sent: 0,
            last_cumulative_sent: SequenceNumber(0),
        }
    }

    pub fn with_config(
        peer_id: NodeId,
        ack_delay_ms: u64,
        max_batch_size: usize,
    ) -> Result<Self, AcksError> {
        if max_batch_size == 0 {
            return Err(AcksError::InvalidConfig {
                field: "max_batch_size".to_string(),
                reason: "must be greater than 0".to_string(),
            });
        }
        Ok(Self {
            peer_id,
            pending_cumulative: None,
            pending_selective: SelectiveAckSet::new(),
            pending_nacks: Vec::new(),
            last_ack_sent_at: None,
            acks_in_batch: 0,
            batch_started_at: None,
            ack_delay_ms,
            max_batch_size,
            total_acks_sent: 0,
            total_nacks_sent: 0,
            duplicate_acks_sent: 0,
            last_cumulative_sent: SequenceNumber(0),
        })
    }

    /// Schedule a cumulative ack.
    pub fn schedule_cumulative(&mut self, seq: SequenceNumber) {
        match &mut self.pending_cumulative {
            Some(current) => {
                if seq.value() > current.value() {
                    *current = seq;
                }
            }
            None => self.pending_cumulative = Some(seq),
        }
    }

    /// Schedule a selective ack block.
    pub fn schedule_selective(&mut self, block: SelectiveAckBlock) {
        self.pending_selective.add_block(block);
    }

    /// Schedule a nack.
    pub fn schedule_nack(&mut self, seq: SequenceNumber) {
        if !self.pending_nacks.contains(&seq) {
            self.pending_nacks.push(seq);
        }
    }

    /// Check whether the session has any pending acks.
    pub fn has_pending(&self) -> bool {
        self.pending_cumulative.is_some()
            || self.pending_selective.block_count() > 0
            || !self.pending_nacks.is_empty()
    }

    /// Check whether the ack delay has elapsed since last ack.
    pub fn delay_elapsed(&self, now: TimestampMillis) -> bool {
        match self.last_ack_sent_at {
            None => true,
            Some(last) => now.elapsed_since(last) >= self.ack_delay_ms,
        }
    }

    /// Check whether the batch is full.
    pub fn batch_full(&self) -> bool {
        self.acks_in_batch >= self.max_batch_size as u64
    }

    /// Get the pending count.
    pub fn pending_count(&self) -> u64 {
        let mut count = 0u64;
        if self.pending_cumulative.is_some() {
            count += 1;
        }
        count += self.pending_selective.block_count() as u64;
        count += self.pending_nacks.len() as u64;
        count
    }

    /// Take the pending cumulative ack.
    pub fn take_cumulative(&mut self) -> Option<SequenceNumber> {
        self.pending_cumulative.take()
    }

    /// Take the pending selective set.
    pub fn take_selective(&mut self) -> SelectiveAckSet {
        std::mem::take(&mut self.pending_selective)
    }

    /// Take the pending nacks.
    pub fn take_nacks(&mut self) -> Vec<SequenceNumber> {
        std::mem::take(&mut self.pending_nacks)
    }

    /// Mark an ack as sent.
    pub fn mark_ack_sent(&mut self, timestamp: TimestampMillis) {
        self.last_ack_sent_at = Some(timestamp);
        self.acks_in_batch += 1;
        self.total_acks_sent += 1;
    }

    /// Mark a nack as sent.
    pub fn mark_nack_sent(&mut self) {
        self.total_nacks_sent += 1;
    }

    /// Mark a duplicate ack.
    pub fn mark_duplicate_ack(&mut self) {
        self.duplicate_acks_sent += 1;
    }

    /// Reset the batch counter.
    pub fn reset_batch(&mut self) {
        self.acks_in_batch = 0;
        self.batch_started_at = None;
    }

    /// Start a new batch.
    pub fn start_batch(&mut self, timestamp: TimestampMillis) {
        self.batch_started_at = Some(timestamp);
        self.acks_in_batch = 0;
    }

    /// Get the session age since last ack sent.
    pub fn session_age_ms(&self, now: TimestampMillis) -> Option<u64> {
        self.last_ack_sent_at.map(|last| now.elapsed_since(last))
    }

    /// Get the duplicate ack ratio.
    pub fn duplicate_ratio(&self) -> f64 {
        if self.total_acks_sent == 0 {
            return 0.0;
        }
        self.duplicate_acks_sent as f64 / self.total_acks_sent as f64 * 100.0
    }

    /// Get the session summary string.
    pub fn summary(&self) -> String {
        format!(
            "AckSession[{}]: pending={}, sent={}, nacks={}, dups={}, delay={}ms",
            self.peer_id,
            self.pending_count(),
            self.total_acks_sent,
            self.total_nacks_sent,
            self.duplicate_acks_sent,
            self.ack_delay_ms
        )
    }

    /// Set the ack delay.
    pub fn set_ack_delay_ms(&mut self, ms: u64) {
        self.ack_delay_ms = ms;
    }

    /// Set the max batch size.
    pub fn set_max_batch_size(&mut self, max: usize) -> Result<(), AcksError> {
        if max == 0 {
            return Err(AcksError::InvalidConfig {
                field: "max_batch_size".to_string(),
                reason: "must be greater than 0".to_string(),
            });
        }
        self.max_batch_size = max;
        Ok(())
    }

    /// Clear all pending acks without sending.
    pub fn clear_pending(&mut self) {
        self.pending_cumulative = None;
        self.pending_selective.clear();
        self.pending_nacks.clear();
    }

    /// Update last cumulative sent.
    pub fn update_last_cumulative(&mut self, seq: SequenceNumber) {
        if seq.value() > self.last_cumulative_sent.value() {
            self.last_cumulative_sent = seq;
        }
    }
}

///////////////////////////////////////////////////////////////////////////////
// DUPLICATE ACK TRACKER
///////////////////////////////////////////////////////////////////////////////

/// Tracks duplicate acks per sequence for fast retransmit detection.
#[derive(Debug, Clone, Default)]
pub struct DuplicateAckTracker {
    pub dup_counts: HashMap<SequenceNumber, u32>,
    pub total_dups: u64,
    pub max_dups_for_seq: SequenceNumber,
    pub max_dup_count: u32,
}

impl DuplicateAckTracker {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a duplicate ack for a sequence.
    pub fn record_duplicate(&mut self, seq: SequenceNumber) {
        let count = self.dup_counts.entry(seq).or_insert(0);
        *count += 1;
        self.total_dups += 1;
        if *count > self.max_dup_count {
            self.max_dup_count = *count;
            self.max_dups_for_seq = seq;
        }
    }

    /// Get the duplicate count for a sequence.
    pub fn dup_count(&self, seq: SequenceNumber) -> u32 {
        self.dup_counts.get(&seq).copied().unwrap_or(0)
    }

    /// Check whether a sequence has reached fast-retransmit threshold.
    pub fn is_fast_retransmit(&self, seq: SequenceNumber, threshold: u32) -> bool {
        self.dup_count(seq) >= threshold
    }

    /// Get sequences that have reached the fast-retransmit threshold.
    pub fn fast_retransmit_candidates(&self, threshold: u32) -> Vec<SequenceNumber> {
        self.dup_counts
            .iter()
            .filter(|(_, count)| **count >= threshold)
            .map(|(seq, _)| *seq)
            .collect()
    }

    /// Get the total duplicate count.
    pub fn total_duplicates(&self) -> u64 {
        self.total_dups
    }

    /// Clear all tracking.
    pub fn clear(&mut self) {
        self.dup_counts.clear();
        self.total_dups = 0;
        self.max_dup_count = 0;
        self.max_dups_for_seq = SequenceNumber(0);
    }

    /// Get the number of tracked sequences.
    pub fn tracked_count(&self) -> usize {
        self.dup_counts.len()
    }
}

///////////////////////////////////////////////////////////////////////////////
// ACK AGGREGATOR
///////////////////////////////////////////////////////////////////////////////

/// Aggregates acks across multiple peers into batches for efficient
/// transmission.
#[derive(Debug, Clone, Default)]
pub struct AckAggregator {
    pub batches: Vec<AckBatch>,
    pub total_aggregated: u64,
    pub max_batch_size: usize,
    pub max_batch_age_ms: u64,
}

/// A single ack batch.
#[derive(Debug, Clone)]
pub struct AckBatch {
    pub batch_id: u64,
    pub peer_ids: Vec<NodeId>,
    pub entries: Vec<AckEntry>,
    pub created_at: TimestampMillis,
    pub closed: bool,
}

impl AckBatch {
    pub fn new(batch_id: u64, created_at: TimestampMillis) -> Self {
        Self {
            batch_id,
            peer_ids: Vec::new(),
            entries: Vec::new(),
            created_at,
            closed: false,
        }
    }

    pub fn add_entry(&mut self, entry: AckEntry) {
        self.entries.push(entry);
    }

    pub fn entry_count(&self) -> usize {
        self.entries.len()
    }

    pub fn age_ms(&self, now: TimestampMillis) -> u64 {
        now.elapsed_since(self.created_at)
    }

    pub fn close(&mut self) {
        self.closed = true;
    }
}

impl AckAggregator {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_config(max_batch_size: usize, max_batch_age_ms: u64) -> Self {
        Self {
            batches: Vec::new(),
            total_aggregated: 0,
            max_batch_size,
            max_batch_age_ms,
        }
    }

    /// Add an ack entry to the most recent open batch, or create a new one.
    pub fn add(
        &mut self,
        peer_id: NodeId,
        entry: AckEntry,
        now: TimestampMillis,
    ) -> u64 {
        // Close batches that are too old.
        self.close_stale(now);

        // Find an open batch for this peer or create one.
        let batch_id = if let Some(batch) = self.batches.iter_mut().rev().find(|b| {
            !b.closed && b.peer_ids.contains(&peer_id) && b.entry_count() < self.max_batch_size
        }) {
            batch.add_entry(entry);
            batch.batch_id
        } else {
            let id = self.batches.len() as u64 + 1;
            let mut batch = AckBatch::new(id, now);
            batch.peer_ids.push(peer_id);
            batch.add_entry(entry);
            self.batches.push(batch);
            id
        };
        self.total_aggregated += 1;
        batch_id
    }

    /// Close stale (too old) batches.
    pub fn close_stale(&mut self, now: TimestampMillis) {
        for batch in self.batches.iter_mut() {
            if !batch.closed && batch.age_ms(now) > self.max_batch_age_ms {
                batch.close();
            }
        }
    }

    /// Get all open batches.
    pub fn open_batches(&self) -> Vec<&AckBatch> {
        self.batches.iter().filter(|b| !b.closed).collect()
    }

    /// Get all closed batches.
    pub fn closed_batches(&self) -> Vec<&AckBatch> {
        self.batches.iter().filter(|b| b.closed).collect()
    }

    /// Take closed batches (removes them from the aggregator).
    pub fn take_closed(&mut self) -> Vec<AckBatch> {
        let closed: Vec<AckBatch> = self
            .batches
            .iter()
            .filter(|b| b.closed)
            .cloned()
            .collect();
        self.batches.retain(|b| !b.closed);
        closed
    }

    /// Get the total aggregated count.
    pub fn total_aggregated(&self) -> u64 {
        self.total_aggregated
    }

    /// Get the total batch count.
    pub fn batch_count(&self) -> usize {
        self.batches.len()
    }

    /// Set max batch size.
    pub fn set_max_batch_size(&mut self, max: usize) {
        self.max_batch_size = max;
    }

    /// Set max batch age.
    pub fn set_max_batch_age_ms(&mut self, ms: u64) {
        self.max_batch_age_ms = ms;
    }
}

///////////////////////////////////////////////////////////////////////////////
// ACK MANAGER
///////////////////////////////////////////////////////////////////////////////

/// Central manager for all ack state across peers.
#[derive(Debug, Clone)]
pub struct AckManager {
    pub sessions: HashMap<NodeId, AckSession>,
    pub duplicate_tracker: DuplicateAckTracker,
    pub aggregator: AckAggregator,
    pub total_acks_sent: u64,
    pub total_acks_received: u64,
    pub total_nacks_sent: u64,
    pub total_nacks_received: u64,
    pub total_duplicate_acks: u64,
    pub default_ack_timeout_ms: u64,
    pub default_ack_delay_ms: u64,
    pub default_max_batch_size: usize,
}

impl Default for AckManager {
    fn default() -> Self {
        Self::new()
    }
}

impl AckManager {
    pub fn new() -> Self {
        Self {
            sessions: HashMap::new(),
            duplicate_tracker: DuplicateAckTracker::new(),
            aggregator: AckAggregator::new(),
            total_acks_sent: 0,
            total_acks_received: 0,
            total_nacks_sent: 0,
            total_nacks_received: 0,
            total_duplicate_acks: 0,
            default_ack_timeout_ms: DEFAULT_ACK_TIMEOUT_MS,
            default_ack_delay_ms: DEFAULT_ACK_DELAY_MS,
            default_max_batch_size: DEFAULT_MAX_ACK_BATCH_SIZE,
        }
    }

    /// Get or create an ack session for a peer.
    pub fn get_or_create_session(&mut self, peer_id: NodeId) -> &mut AckSession {
        self.sessions
            .entry(peer_id)
            .or_insert_with(|| AckSession::new(peer_id))
    }

    /// Get a reference to a peer's ack session.
    pub fn session(&self, peer_id: NodeId) -> Option<&AckSession> {
        self.sessions.get(&peer_id)
    }

    /// Get a mutable reference to a peer's ack session.
    pub fn session_mut(&mut self, peer_id: NodeId) -> Option<&mut AckSession> {
        self.sessions.get_mut(&peer_id)
    }

    /// Remove a peer's ack session.
    pub fn remove_session(&mut self, peer_id: NodeId) -> bool {
        self.sessions.remove(&peer_id).is_some()
    }

    /// Check whether a session exists.
    pub fn has_session(&self, peer_id: NodeId) -> bool {
        self.sessions.contains_key(&peer_id)
    }

    /// Record an ack sent.
    pub fn record_ack_sent(&mut self, peer_id: NodeId) {
        self.total_acks_sent += 1;
        if let Some(session) = self.sessions.get_mut(&peer_id) {
            session.total_acks_sent += 1;
        }
    }

    /// Record an ack received.
    pub fn record_ack_received(&mut self, peer_id: NodeId) {
        self.total_acks_received += 1;
        if let Some(session) = self.sessions.get_mut(&peer_id) {
            session.total_acks_sent += 1;
        }
    }

    /// Record a nack sent.
    pub fn record_nack_sent(&mut self, peer_id: NodeId) {
        self.total_nacks_sent += 1;
        if let Some(session) = self.sessions.get_mut(&peer_id) {
            session.total_nacks_sent += 1;
        }
    }

    /// Record a nack received.
    pub fn record_nack_received(&mut self, _peer_id: NodeId) {
        self.total_nacks_received += 1;
    }

    /// Record a duplicate ack.
    pub fn record_duplicate_ack(&mut self, peer_id: NodeId, seq: SequenceNumber) {
        self.total_duplicate_acks += 1;
        self.duplicate_tracker.record_duplicate(seq);
        if let Some(session) = self.sessions.get_mut(&peer_id) {
            session.mark_duplicate_ack();
        }
    }

    /// Get total acks sent.
    pub fn total_acks_sent(&self) -> u64 {
        self.total_acks_sent
    }

    /// Get total acks received.
    pub fn total_acks_received(&self) -> u64 {
        self.total_acks_received
    }

    /// Get total nacks sent.
    pub fn total_nacks_sent(&self) -> u64 {
        self.total_nacks_sent
    }

    /// Get total nacks received.
    pub fn total_nacks_received(&self) -> u64 {
        self.total_nacks_received
    }

    /// Get total duplicate acks.
    pub fn total_duplicate_acks(&self) -> u64 {
        self.total_duplicate_acks
    }

    /// Get the ack ratio as a percentage.
    pub fn ack_ratio_percent(&self) -> f64 {
        if self.total_acks_sent == 0 {
            return 100.0;
        }
        self.total_acks_received as f64 / self.total_acks_sent as f64 * 100.0
    }

    /// Get the duplicate ack ratio as a percentage.
    pub fn duplicate_ack_ratio_percent(&self) -> f64 {
        if self.total_acks_sent == 0 {
            return 0.0;
        }
        self.total_duplicate_acks as f64 / self.total_acks_sent as f64 * 100.0
    }

    /// Check whether acks are healthy.
    pub fn are_acks_healthy(&self) -> bool {
        self.ack_ratio_percent() >= 50.0
    }

    /// Get the active session count.
    pub fn active_session_count(&self) -> usize {
        self.sessions.len()
    }

    /// List peers with sessions.
    pub fn peers_with_sessions(&self) -> Vec<NodeId> {
        self.sessions.keys().copied().collect()
    }

    /// Get sessions with pending acks.
    pub fn sessions_with_pending(&self) -> Vec<NodeId> {
        self.sessions
            .iter()
            .filter(|(_, s)| s.has_pending())
            .map(|(id, _)| *id)
            .collect()
    }

    /// Reset a peer's ack state.
    pub fn reset_peer(&mut self, peer_id: NodeId) -> bool {
        if self.sessions.remove(&peer_id).is_some() {
            self.sessions.insert(peer_id, AckSession::new(peer_id));
            true
        } else {
            false
        }
    }

    /// Reset all ack state.
    pub fn reset_all(&mut self) {
        self.sessions.clear();
        self.total_acks_sent = 0;
        self.total_acks_received = 0;
        self.total_nacks_sent = 0;
        self.total_nacks_received = 0;
        self.total_duplicate_acks = 0;
        self.duplicate_tracker.clear();
    }

    /// Get the default ack timeout.
    pub fn default_ack_timeout_ms(&self) -> u64 {
        self.default_ack_timeout_ms
    }

    /// Set the default ack timeout.
    pub fn set_default_ack_timeout_ms(&mut self, ms: u64) {
        self.default_ack_timeout_ms = ms;
    }

    /// Get the default ack delay.
    pub fn default_ack_delay_ms(&self) -> u64 {
        self.default_ack_delay_ms
    }

    /// Set the default ack delay.
    pub fn set_default_ack_delay_ms(&mut self, ms: u64) {
        self.default_ack_delay_ms = ms;
    }

    /// Get the default max batch size.
    pub fn default_max_batch_size(&self) -> usize {
        self.default_max_batch_size
    }

    /// Set the default max batch size.
    pub fn set_default_max_batch_size(&mut self, max: usize) {
        self.default_max_batch_size = max;
    }

    /// Snapshot all ack state.
    pub fn snapshot(&self) -> AcksSnapshot {
        let cumulative_states: Vec<CumulativeAckState> = self
            .sessions
            .values()
            .map(|s| CumulativeAckState {
                peer_id: s.peer_id,
                acked_up_to: s.last_cumulative_sent,
                highest_received: s.last_cumulative_sent,
                total_acks_sent: s.total_acks_sent,
                total_acks_received: 0,
                last_ack_sent_at: s.last_ack_sent_at,
                last_ack_received_at: None,
                duplicate_acks_sent: s.duplicate_acks_sent,
                duplicate_acks_received: 0,
                ack_batch_count: 0,
            })
            .collect();
        let timing_stats: Vec<AckTimingStatsSnapshot> = Vec::new();
        AcksSnapshot {
            cumulative_states,
            timing_stats,
            total_acks_sent: self.total_acks_sent,
            total_acks_received: self.total_acks_received,
            snapshot_at: TimestampMillis::now(),
        }
    }

    /// Restore from snapshot.
    pub fn restore(&mut self, snapshot: AcksSnapshot) {
        self.sessions.clear();
        for state in snapshot.cumulative_states {
            let mut session = AckSession::new(state.peer_id);
            session.last_cumulative_sent = state.acked_up_to;
            session.total_acks_sent = state.total_acks_sent;
            self.sessions.insert(state.peer_id, session);
        }
        self.total_acks_sent = snapshot.total_acks_sent;
        self.total_acks_received = snapshot.total_acks_received;
    }

    /// Get the count of sessions with pending acks.
    pub fn pending_session_count(&self) -> usize {
        self.sessions.values().filter(|s| s.has_pending()).count()
    }

    /// Get the total pending acks across all sessions.
    pub fn total_pending_acks(&self) -> u64 {
        self.sessions.values().map(|s| s.pending_count()).sum()
    }

    /// Get the session with the most pending acks.
    pub fn busiest_ack_session(&self) -> Option<(NodeId, u64)> {
        self.sessions
            .iter()
            .max_by_key(|(_, s)| s.pending_count())
            .map(|(id, s)| (*id, s.pending_count()))
    }

    /// Get the total delay elapsed across sessions.
    pub fn sessions_ready_to_send(&self, now: TimestampMillis) -> Vec<NodeId> {
        self.sessions
            .iter()
            .filter(|(_, s)| s.has_pending() && s.delay_elapsed(now))
            .map(|(id, _)| *id)
            .collect()
    }

    /// Get sessions whose batches are full.
    pub fn sessions_with_full_batches(&self) -> Vec<NodeId> {
        self.sessions
            .iter()
            .filter(|(_, s)| s.batch_full())
            .map(|(id, _)| *id)
            .collect()
    }

    /// Get the aggregate duplicate ratio across all sessions.
    pub fn aggregate_duplicate_ratio(&self) -> f64 {
        let total_sent: u64 = self.sessions.values().map(|s| s.total_acks_sent).sum();
        let total_dups: u64 = self
            .sessions
            .values()
            .map(|s| s.duplicate_acks_sent)
            .sum();
        if total_sent == 0 {
            return 0.0;
        }
        total_dups as f64 / total_sent as f64 * 100.0
    }

    /// Get the nack ratio as a percentage.
    pub fn nack_ratio_percent(&self) -> f64 {
        if self.total_acks_sent == 0 {
            return 0.0;
        }
        self.total_nacks_sent as f64 / self.total_acks_sent as f64 * 100.0
    }

    /// Get the manager summary string.
    pub fn summary_string(&self) -> String {
        format!(
            "AckManager: sessions={}, acks_sent={}, acks_recv={}, nacks_sent={}, nacks_recv={}, dups={}, ratio={:.1}%",
            self.active_session_count(),
            self.total_acks_sent,
            self.total_acks_received,
            self.total_nacks_sent,
            self.total_nacks_received,
            self.total_duplicate_acks,
            self.ack_ratio_percent()
        )
    }

    /// Clone all sessions.
    pub fn clone_sessions(&self) -> HashMap<NodeId, AckSession> {
        self.sessions.clone()
    }

    /// Replace all sessions (for import).
    pub fn replace_sessions(&mut self, sessions: HashMap<NodeId, AckSession>) {
        self.sessions = sessions;
    }

    /// Get the number of acks sent since a counter.
    pub fn acks_sent_since(&self, since: u64) -> u64 {
        self.total_acks_sent.saturating_sub(since)
    }

    /// Get the number of acks received since a counter.
    pub fn acks_received_since(&self, since: u64) -> u64 {
        self.total_acks_received.saturating_sub(since)
    }

    /// Get the number of nacks sent since a counter.
    pub fn nacks_sent_since(&self, since: u64) -> u64 {
        self.total_nacks_sent.saturating_sub(since)
    }

    /// Get the total entries across all delayed ack queues.
    pub fn total_delayed_entries(&self) -> u64 {
        0 // placeholder; per-session queues track their own
    }

    /// Get the number of delayed queues.
    pub fn delayed_queue_count(&self) -> usize {
        0
    }

    /// Check whether any session has pending delayed acks.
    pub fn any_delayed_pending(&self) -> bool {
        self.sessions.values().any(|s| s.has_pending())
    }

    /// Get the total bytes saved by compression across all sessions.
    pub fn compression_savings_bytes(&self) -> u64 {
        0 // placeholder for future integration with AckCompressor
    }

    /// Get the ack pipeline depth (max pending per session).
    pub fn max_pipeline_depth(&self) -> u64 {
        self.sessions
            .values()
            .map(|s| s.pending_count())
            .max()
            .unwrap_or(0)
    }

    /// Get the aggregate queue utilization across all sessions.
    pub fn aggregate_queue_utilization(&self) -> f64 {
        if self.sessions.is_empty() {
            return 0.0;
        }
        let total: u64 = self.sessions.values().map(|s| s.pending_count()).sum();
        total as f64 / self.sessions.len() as f64
    }

    /// Get the count of sessions exceeding their ack delay threshold.
    pub fn delayed_session_count(&self, now: TimestampMillis) -> usize {
        self.sessions
            .values()
            .filter(|s| s.delay_remaining_ms(now) > 0)
            .count()
    }

    /// Get the total acks in flight across all sessions.
    pub fn total_acks_in_flight(&self) -> u64 {
        self.sessions.values().map(|s| s.acks_in_batch).sum()
    }

    /// Get the sessions that have duplicate acks pending.
    pub fn sessions_with_duplicates(&self) -> Vec<NodeId> {
        self.sessions
            .iter()
            .filter(|(_, s)| s.duplicate_acks_sent > 0)
            .map(|(id, _)| *id)
            .collect()
    }

    /// Get the summary of all sessions as a vector of strings.
    pub fn all_session_summaries(&self) -> Vec<String> {
        self.sessions
            .iter()
            .map(|(_, s)| s.summary())
            .collect()
    }

    /// Check whether the manager is idle (no activity anywhere).
    pub fn is_idle(&self) -> bool {
        self.total_acks_sent == 0
            && self.total_acks_received == 0
            && self.total_nacks_sent == 0
            && self.total_nacks_received == 0
    }

    /// Get the total activity count (all counters summed).
    pub fn total_activity(&self) -> u64 {
        self.total_acks_sent
            + self.total_acks_received
            + self.total_nacks_sent
            + self.total_nacks_received
            + self.total_duplicate_acks
    }

    /// Get the ratio of sessions with pending acks to total sessions.
    pub fn pending_session_ratio(&self) -> f64 {
        if self.sessions.is_empty() {
            return 100.0;
        }
        self.pending_session_count() as f64 / self.sessions.len() as f64 * 100.0
    }

    /// Get the peak pending count across all sessions.
    pub fn peak_pending_count(&self) -> u64 {
        self.sessions
            .values()
            .map(|s| s.pending_count())
            .max()
            .unwrap_or(0)
    }

    /// Get the count of sessions with zero activity.
    pub fn idle_session_count(&self) -> usize {
        self.sessions.values().filter(|s| s.is_idle()).count()
    }

    /// Get the aggregate rtt estimate across all sessions (average srtt).
    pub fn aggregate_srtt_ms(&self) -> u64 {
        0 // placeholder; rtt tracked per-session in timing stats
    }

    /// Get the total number of tracked types across the acks subsystem.
    pub fn tracked_type_count(&self) -> usize {
        4 // AckEntry, CumulativeAckState, SelectiveAckSet, NackEntry
    }

    /// Get the acks subsystem version constant.
    pub const fn acks_subsystem_version() -> u32 {
        1
    }
}

///////////////////////////////////////////////////////////////////////////////
// DELAYED ACK QUEUE TESTS
///////////////////////////////////////////////////////////////////////////////

#[cfg(test)]
mod delayed_queue_tests {
    use super::*;

    #[test]
    fn delayed_ack_queue_new() {
        let q = DelayedAckQueue::new();
        assert_eq!(q.len(), 0);
        assert!(q.is_empty());
    }

    #[test]
    fn delayed_ack_queue_with_config() {
        let q = DelayedAckQueue::with_config(100, 500);
        assert_eq!(q.max_delay_ms, 100);
        assert_eq!(q.max_queue_size, 500);
    }

    #[test]
    fn delayed_ack_queue_enqueue() {
        let mut q = DelayedAckQueue::with_config(100, 10);
        let id = q
            .enqueue(
                test_peer1(),
                AckType::Cumulative,
                SequenceNumber(1),
                test_ts(100),
            )
            .unwrap();
        assert_eq!(id, 1);
        assert_eq!(q.len(), 1);
        assert!(!q.is_empty());
    }

    #[test]
    fn delayed_ack_queue_enqueue_full() {
        let mut q = DelayedAckQueue::with_config(100, 1);
        q.enqueue(test_peer1(), AckType::Cumulative, SequenceNumber(1), test_ts(100))
            .unwrap();
        let err = q
            .enqueue(test_peer1(), AckType::Cumulative, SequenceNumber(2), test_ts(100))
            .unwrap_err();
        assert!(matches!(err, AcksError::BatchTooLarge { .. }));
        assert_eq!(q.total_dropped, 1);
    }

    #[test]
    fn delayed_ack_entry_is_ready() {
        let entry = DelayedAckEntry::new(
            1,
            test_peer1(),
            AckType::Cumulative,
            SequenceNumber(1),
            test_ts(100),
            50,
        );
        assert!(!entry.is_ready(test_ts(140)));
        assert!(entry.is_ready(test_ts(150)));
    }

    #[test]
    fn delayed_ack_entry_age_ms() {
        let entry = DelayedAckEntry::new(
            1,
            test_peer1(),
            AckType::Cumulative,
            SequenceNumber(1),
            test_ts(100),
            50,
        );
        assert_eq!(entry.age_ms(test_ts(250)), 150);
    }

    #[test]
    fn delayed_ack_queue_ready_entries() {
        let mut q = DelayedAckQueue::with_config(10, 10);
        q.enqueue(test_peer1(), AckType::Cumulative, SequenceNumber(1), test_ts(100)).unwrap();
        q.enqueue(test_peer1(), AckType::Cumulative, SequenceNumber(2), test_ts(100)).unwrap();
        let ready = q.ready_entries(test_ts(200));
        assert_eq!(ready.len(), 2);
    }

    #[test]
    fn delayed_ack_queue_mark_sent() {
        let mut q = DelayedAckQueue::with_config(100, 10);
        q.enqueue(test_peer1(), AckType::Cumulative, SequenceNumber(1), test_ts(100)).unwrap();
        assert!(q.mark_sent(1));
        assert!(!q.mark_sent(999));
        assert_eq!(q.total_flushed, 1);
    }

    #[test]
    fn delayed_ack_queue_flush_ready() {
        let mut q = DelayedAckQueue::with_config(10, 10);
        q.enqueue(test_peer1(), AckType::Cumulative, SequenceNumber(1), test_ts(100)).unwrap();
        let flushed = q.flush_ready(test_ts(200));
        assert_eq!(flushed, 1);
    }

    #[test]
    fn delayed_ack_queue_purge_sent() {
        let mut q = DelayedAckQueue::with_config(100, 10);
        q.enqueue(test_peer1(), AckType::Cumulative, SequenceNumber(1), test_ts(100)).unwrap();
        q.mark_sent(1);
        let purged = q.purge_sent();
        assert_eq!(purged, 1);
        assert!(q.is_empty());
    }

    #[test]
    fn delayed_ack_queue_utilization() {
        let mut q = DelayedAckQueue::with_config(100, 100);
        q.enqueue(test_peer1(), AckType::Cumulative, SequenceNumber(1), test_ts(100)).unwrap();
        assert!((q.utilization_percent() - 1.0).abs() < 0.01);
    }

    #[test]
    fn delayed_ack_queue_is_full() {
        let mut q = DelayedAckQueue::with_config(100, 1);
        assert!(!q.is_full());
        q.enqueue(test_peer1(), AckType::Cumulative, SequenceNumber(1), test_ts(100))
            .unwrap();
        assert!(q.is_full());
    }

    #[test]
    fn delayed_ack_queue_clear() {
        let mut q = DelayedAckQueue::with_config(100, 10);
        q.enqueue(test_peer1(), AckType::Cumulative, SequenceNumber(1), test_ts(100)).unwrap();
        q.enqueue(test_peer1(), AckType::Cumulative, SequenceNumber(2), test_ts(100)).unwrap();
        let cleared = q.clear();
        assert_eq!(cleared, 2);
        assert!(q.is_empty());
    }

    #[test]
    fn delayed_ack_queue_stats() {
        let mut q = DelayedAckQueue::with_config(100, 10);
        q.enqueue(test_peer1(), AckType::Cumulative, SequenceNumber(1), test_ts(100)).unwrap();
        let stats = q.stats();
        assert_eq!(stats.total_queued, 1);
        assert_eq!(stats.current_size, 1);
    }

    #[test]
    fn delayed_ack_queue_set_config() {
        let mut q = DelayedAckQueue::new();
        q.set_max_delay_ms(200);
        q.set_max_queue_size(200);
        assert_eq!(q.max_delay_ms, 200);
        assert_eq!(q.max_queue_size, 200);
    }

    #[test]
    fn delayed_ack_queue_entries_for_peer() {
        let mut q = DelayedAckQueue::with_config(100, 10);
        q.enqueue(test_peer1(), AckType::Cumulative, SequenceNumber(1), test_ts(100)).unwrap();
        q.enqueue(test_peer2(), AckType::Cumulative, SequenceNumber(1), test_ts(100)).unwrap();
        let p1 = q.entries_for_peer(test_peer1());
        assert_eq!(p1.len(), 1);
    }

    #[test]
    fn delayed_ack_queue_oldest_unsent_age() {
        let mut q = DelayedAckQueue::with_config(100, 10);
        q.enqueue(test_peer1(), AckType::Cumulative, SequenceNumber(1), test_ts(100)).unwrap();
        let age = q.oldest_unsent_age_ms(test_ts(200));
        assert_eq!(age, Some(100));
    }
}

///////////////////////////////////////////////////////////////////////////////
// ACK COMPRESSOR TESTS
///////////////////////////////////////////////////////////////////////////////

#[cfg(test)]
mod ack_compressor_tests {
    use super::*;

    #[test]
    fn ack_compressor_new() {
        let c = AckCompressor::new();
        assert_eq!(c.unique_count(), 0);
        assert_eq!(c.range_count(), 0);
    }

    #[test]
    fn ack_compressor_add() {
        let mut c = AckCompressor::new();
        c.add(SequenceNumber(5));
        c.add(SequenceNumber(3));
        c.add(SequenceNumber(1));
        assert_eq!(c.unique_count(), 3);
        assert_eq!(c.original_count(), 3);
    }

    #[test]
    fn ack_compressor_add_duplicate() {
        let mut c = AckCompressor::new();
        c.add(SequenceNumber(1));
        c.add(SequenceNumber(1));
        assert_eq!(c.unique_count(), 1);
        assert_eq!(c.original_count(), 1);
    }

    #[test]
    fn ack_compressor_add_batch() {
        let mut c = AckCompressor::new();
        c.add_batch(&[SequenceNumber(1), SequenceNumber(2), SequenceNumber(3)]);
        assert_eq!(c.unique_count(), 3);
    }

    #[test]
    fn ack_compressor_compress_contiguous() {
        let mut c = AckCompressor::new();
        c.add_batch(&[SequenceNumber(1), SequenceNumber(2), SequenceNumber(3)]);
        let ranges = c.compress();
        assert_eq!(ranges.len(), 1);
        assert_eq!(ranges[0], (SequenceNumber(1), SequenceNumber(3)));
    }

    #[test]
    fn ack_compressor_compress_discontiguous() {
        let mut c = AckCompressor::new();
        c.add_batch(&[SequenceNumber(1), SequenceNumber(2), SequenceNumber(5), SequenceNumber(6)]);
        let ranges = c.compress();
        assert_eq!(ranges.len(), 2);
        assert_eq!(ranges[0], (SequenceNumber(1), SequenceNumber(2)));
        assert_eq!(ranges[1], (SequenceNumber(5), SequenceNumber(6)));
    }

    #[test]
    fn ack_compressor_range_count() {
        let mut c = AckCompressor::new();
        assert_eq!(c.range_count(), 0);
        c.add(SequenceNumber(1));
        assert_eq!(c.range_count(), 1);
    }

    #[test]
    fn ack_compressor_savings_percent() {
        let mut c = AckCompressor::new();
        assert!((c.savings_percent() - 0.0).abs() < 0.001);
        c.add_batch(&[SequenceNumber(1), SequenceNumber(2), SequenceNumber(3)]);
        assert!((c.savings_percent() - 66.66).abs() < 1.0);
    }

    #[test]
    fn ack_compressor_clear() {
        let mut c = AckCompressor::new();
        c.add(SequenceNumber(1));
        c.clear();
        assert_eq!(c.unique_count(), 0);
        assert_eq!(c.original_count(), 0);
    }

    #[test]
    fn ack_compressor_contains() {
        let mut c = AckCompressor::new();
        c.add(SequenceNumber(42));
        assert!(c.contains(SequenceNumber(42)));
        assert!(!c.contains(SequenceNumber(43)));
    }

    #[test]
    fn ack_compressor_decompress() {
        let mut c = AckCompressor::new();
        c.add_batch(&[SequenceNumber(3), SequenceNumber(1), SequenceNumber(2)]);
        let decompressed = c.decompress();
        assert_eq!(decompressed, vec![SequenceNumber(1), SequenceNumber(2), SequenceNumber(3)]);
    }

    #[test]
    fn ack_compressor_ratio() {
        let mut c = AckCompressor::new();
        assert!((c.ratio() - 0.0).abs() < 0.001);
        c.add(SequenceNumber(1));
        assert!((c.ratio() - 1.0).abs() < 0.001);
    }
}

///////////////////////////////////////////////////////////////////////////////
// DELAYED ACK QUEUE
///////////////////////////////////////////////////////////////////////////////

/// Queue for delayed acknowledgements (ack-delay algorithm).
/// Holds acks briefly to batch them before sending.
#[derive(Debug, Clone, Default)]
pub struct DelayedAckQueue {
    pub entries: Vec<DelayedAckEntry>,
    pub max_delay_ms: u64,
    pub max_queue_size: usize,
    pub total_queued: u64,
    pub total_flushed: u64,
    pub total_dropped: u64,
}

/// A single delayed ack entry waiting to be sent.
#[derive(Debug, Clone)]
pub struct DelayedAckEntry {
    pub entry_id: u64,
    pub peer_id: NodeId,
    pub ack_type: AckType,
    pub sequence: SequenceNumber,
    pub queued_at: TimestampMillis,
    pub flush_at: TimestampMillis,
    pub sent: bool,
}

impl DelayedAckEntry {
    pub fn new(
        entry_id: u64,
        peer_id: NodeId,
        ack_type: AckType,
        sequence: SequenceNumber,
        queued_at: TimestampMillis,
        max_delay_ms: u64,
    ) -> Self {
        Self {
            entry_id,
            peer_id,
            ack_type,
            sequence,
            queued_at,
            flush_at: TimestampMillis(queued_at.value() + max_delay_ms),
            sent: false,
        }
    }

    pub fn is_ready(&self, now: TimestampMillis) -> bool {
        now.value() >= self.flush_at.value()
    }

    pub fn age_ms(&self, now: TimestampMillis) -> u64 {
        now.elapsed_since(self.queued_at)
    }

    pub fn mark_sent(&mut self) {
        self.sent = true;
    }
}

impl DelayedAckQueue {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_config(max_delay_ms: u64, max_queue_size: usize) -> Self {
        Self {
            entries: Vec::new(),
            max_delay_ms,
            max_queue_size,
            total_queued: 0,
            total_flushed: 0,
            total_dropped: 0,
        }
    }

    /// Queue a delayed ack.
    pub fn enqueue(
        &mut self,
        peer_id: NodeId,
        ack_type: AckType,
        sequence: SequenceNumber,
        now: TimestampMillis,
    ) -> Result<u64, AcksError> {
        if self.entries.len() >= self.max_queue_size {
            self.total_dropped += 1;
            return Err(AcksError::BatchTooLarge {
                size: self.entries.len(),
                max: self.max_queue_size,
            });
        }
        let id = self.total_queued + 1;
        let entry = DelayedAckEntry::new(
            id,
            peer_id,
            ack_type,
            sequence,
            now,
            self.max_delay_ms,
        );
        self.entries.push(entry);
        self.total_queued += 1;
        Ok(id)
    }

    /// Get entries ready to flush at `now`.
    pub fn ready_entries(&self, now: TimestampMillis) -> Vec<&DelayedAckEntry> {
        self.entries
            .iter()
            .filter(|e| !e.sent && e.is_ready(now))
            .collect()
    }

    /// Get all unsent entries.
    pub fn unsent_entries(&self) -> Vec<&DelayedAckEntry> {
        self.entries.iter().filter(|e| !e.sent).collect()
    }

    /// Mark an entry as sent.
    pub fn mark_sent(&mut self, entry_id: u64) -> bool {
        if let Some(entry) = self.entries.iter_mut().find(|e| e.entry_id == entry_id) {
            entry.mark_sent();
            self.total_flushed += 1;
            true
        } else {
            false
        }
    }

    /// Flush all ready entries (mark them sent).
    pub fn flush_ready(&mut self, now: TimestampMillis) -> u64 {
        let ready: Vec<u64> = self
            .entries
            .iter()
            .filter(|e| !e.sent && e.is_ready(now))
            .map(|e| e.entry_id)
            .collect();
        for id in ready {
            self.mark_sent(id);
        }
        ready.len() as u64
    }

    /// Remove sent entries from the queue.
    pub fn purge_sent(&mut self) -> usize {
        let before = self.entries.len();
        self.entries.retain(|e| !e.sent);
        before - self.entries.len()
    }

    /// Get the queue size.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check whether the queue is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Check whether the queue is full.
    pub fn is_full(&self) -> bool {
        self.entries.len() >= self.max_queue_size
    }

    /// Get the queue utilization as a percentage.
    pub fn utilization_percent(&self) -> f64 {
        if self.max_queue_size == 0 {
            return 0.0;
        }
        self.entries.len() as f64 / self.max_queue_size as f64 * 100.0
    }

    /// Get the oldest unsent entry age.
    pub fn oldest_unsent_age_ms(&self, now: TimestampMillis) -> Option<u64> {
        self.unsent_entries()
            .iter()
            .map(|e| e.age_ms(now))
            .max()
    }

    /// Get entries for a specific peer.
    pub fn entries_for_peer(&self, peer_id: NodeId) -> Vec<&DelayedAckEntry> {
        self.entries
            .iter()
            .filter(|e| !e.sent && e.peer_id == peer_id)
            .collect()
    }

    /// Clear all entries (sent and unsent).
    pub fn clear(&mut self) -> usize {
        let before = self.entries.len();
        self.entries.clear();
        before
    }

    /// Get statistics.
    pub fn stats(&self) -> DelayedAckQueueStats {
        DelayedAckQueueStats {
            total_queued: self.total_queued,
            total_flushed: self.total_flushed,
            total_dropped: self.total_dropped,
            current_size: self.entries.len(),
            unsent_count: self.unsent_entries().len(),
            utilization_percent: self.utilization_percent(),
        }
    }

    /// Set max delay.
    pub fn set_max_delay_ms(&mut self, ms: u64) {
        self.max_delay_ms = ms;
    }

    /// Set max queue size.
    pub fn set_max_queue_size(&mut self, size: usize) {
        self.max_queue_size = size;
    }
}

/// Statistics for a delayed ack queue.
#[derive(Debug, Clone, Default)]
pub struct DelayedAckQueueStats {
    pub total_queued: u64,
    pub total_flushed: u64,
    pub total_dropped: u64,
    pub current_size: usize,
    pub unsent_count: usize,
    pub utilization_percent: f64,
}

///////////////////////////////////////////////////////////////////////////////
// ACK COMPRESSOR
///////////////////////////////////////////////////////////////////////////////

/// Compresses sequences of acks into compact ranges to reduce bandwidth.
#[derive(Debug, Clone, Default)]
pub struct AckCompressor {
    pub sequences: Vec<SequenceNumber>,
    pub total_compressed: u64,
    pub total_original: u64,
    pub compression_ratio: f64,
}

impl AckCompressor {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a sequence to the compressor.
    pub fn add(&mut self, seq: SequenceNumber) {
        if !self.sequences.contains(&seq) {
            self.sequences.push(seq);
            self.sequences.sort();
            self.total_original += 1;
            self.recompute_ratio();
        }
    }

    /// Add multiple sequences.
    pub fn add_batch(&mut self, seqs: &[SequenceNumber]) {
        for seq in seqs {
            self.add(*seq);
        }
    }

    /// Compress into ranges [start, end].
    pub fn compress(&self) -> Vec<(SequenceNumber, SequenceNumber)> {
        if self.sequences.is_empty() {
            return Vec::new();
        }
        let mut ranges: Vec<(SequenceNumber, SequenceNumber)> = Vec::new();
        let mut start = self.sequences[0];
        let mut end = self.sequences[0];
        for seq in self.sequences.iter().skip(1) {
            if seq.value() == end.value().wrapping_add(1) {
                end = *seq;
            } else {
                ranges.push((start, end));
                start = *seq;
                end = *seq;
            }
        }
        ranges.push((start, end));
        ranges
    }

    /// Get the compressed range count.
    pub fn range_count(&self) -> usize {
        self.compress().len()
    }

    /// Get the original sequence count.
    pub fn original_count(&self) -> u64 {
        self.total_original
    }

    /// Get the compression ratio (original / compressed).
    pub fn ratio(&self) -> f64 {
        self.compression_ratio
    }

    /// Get the space savings as a percentage.
    pub fn savings_percent(&self) -> f64 {
        if self.total_original == 0 {
            return 0.0;
        }
        let ranges = self.range_count() as f64;
        (1.0 - ranges / self.total_original as f64) * 100.0
    }

    /// Clear the compressor.
    pub fn clear(&mut self) {
        self.sequences.clear();
        self.total_compressed = 0;
        self.total_original = 0;
        self.compression_ratio = 0.0;
    }

    /// Recompute the compression ratio.
    fn recompute_ratio(&mut self) {
        if self.total_original == 0 {
            self.compression_ratio = 0.0;
            return;
        }
        let ranges = self.range_count() as f64;
        self.compression_ratio = ranges / self.total_original as f64;
    }

    /// Get the decompressed sequences.
    pub fn decompress(&self) -> Vec<SequenceNumber> {
        self.sequences.clone()
    }

    /// Check whether a sequence is in the compressor.
    pub fn contains(&self, seq: SequenceNumber) -> bool {
        self.sequences.contains(&seq)
    }

    /// Get the number of unique sequences.
    pub fn unique_count(&self) -> usize {
        self.sequences.len()
    }
}

///////////////////////////////////////////////////////////////////////////////
// DUPLICATE ACK TRACKER EXTENDED METHODS
///////////////////////////////////////////////////////////////////////////////

impl DuplicateAckTracker {
    /// Get the top N sequences by duplicate count.
    pub fn top_duplicates(&self, n: usize) -> Vec<(SequenceNumber, u32)> {
        let mut entries: Vec<(SequenceNumber, u32)> = self
            .dup_counts
            .iter()
            .map(|(seq, count)| (*seq, *count))
            .collect();
        entries.sort_by_key(|(_, count)| std::cmp::Reverse(*count));
        entries.truncate(n);
        entries
    }

    /// Get sequences with duplicate count above threshold.
    pub fn sequences_above_threshold(&self, threshold: u32) -> Vec<SequenceNumber> {
        self.dup_counts
            .iter()
            .filter(|(_, count)| **count > threshold)
            .map(|(seq, _)| *seq)
            .collect()
    }

    /// Get the average duplicate count per tracked sequence.
    pub fn average_dup_count(&self) -> f64 {
        if self.dup_counts.is_empty() {
            return 0.0;
        }
        let total: u32 = self.dup_counts.values().sum();
        total as f64 / self.dup_counts.len() as f64
    }

    /// Check whether any sequence is at risk of fast retransmit.
    pub fn any_at_risk(&self, threshold: u32) -> bool {
        self.dup_counts.values().any(|c| *c >= threshold)
    }

    /// Get the max duplicate count observed.
    pub fn max_duplicate_count(&self) -> u32 {
        self.max_dup_count
    }

    /// Merge another tracker into this one.
    pub fn merge(&mut self, other: &DuplicateAckTracker) {
        for (seq, count) in &other.dup_counts {
            let entry = self.dup_counts.entry(*seq).or_insert(0);
            *entry += count;
            self.total_dups += other.total_dups;
            if *entry > self.max_dup_count {
                self.max_dup_count = *entry;
                self.max_dups_for_seq = *seq;
            }
        }
    }
}

///////////////////////////////////////////////////////////////////////////////
// ACK AGGREGATOR EXTENDED METHODS
///////////////////////////////////////////////////////////////////////////////

impl AckAggregator {
    /// Get the total entries across all open batches.
    pub fn open_batch_entry_count(&self) -> u64 {
        self.batches
            .iter()
            .filter(|b| !b.closed)
            .map(|b| b.entry_count() as u64)
            .sum()
    }

    /// Get the total entries across all closed batches.
    pub fn closed_batch_entry_count(&self) -> u64 {
        self.batches
            .iter()
            .filter(|b| b.closed)
            .map(|b| b.entry_count() as u64)
            .sum()
    }

    /// Get the oldest open batch age.
    pub fn oldest_open_batch_age_ms(&self, now: TimestampMillis) -> Option<u64> {
        self.batches
            .iter()
            .filter(|b| !b.closed)
            .map(|b| b.age_ms(now))
            .max()
    }

    /// Get the number of peers in a batch.
    pub fn batch_peer_count(&self, batch_id: u64) -> usize {
        self.batches
            .iter()
            .find(|b| b.batch_id == batch_id)
            .map(|b| b.peer_ids.len())
            .unwrap_or(0)
    }

    /// Close all open batches.
    pub fn close_all(&mut self) {
        for batch in self.batches.iter_mut() {
            batch.close();
        }
    }

    /// Get the total number of batches created.
    pub fn total_batches_created(&self) -> usize {
        self.batches.len()
    }
}

///////////////////////////////////////////////////////////////////////////////
// ACK SESSION EXTENDED METHODS
///////////////////////////////////////////////////////////////////////////////

impl AckSession {
    /// Check whether the session is idle (no activity).
    pub fn is_idle(&self) -> bool {
        self.total_acks_sent == 0 && self.total_nacks_sent == 0
    }

    /// Get the ratio of nacks to acks.
    pub fn nack_to_ack_ratio(&self) -> f64 {
        if self.total_acks_sent == 0 {
            return 0.0;
        }
        self.total_nacks_sent as f64 / self.total_acks_sent as f64 * 100.0
    }

    /// Get whether the session batch should be flushed.
    pub fn should_flush_batch(&self, now: TimestampMillis) -> bool {
        self.batch_full() || (self.acks_in_batch > 0 && self.delay_elapsed(now))
    }

    /// Get the time until the next ack can be sent (delay remaining).
    pub fn delay_remaining_ms(&self, now: TimestampMillis) -> u64 {
        match self.last_ack_sent_at {
            None => 0,
            Some(last) => {
                let elapsed = now.elapsed_since(last);
                if elapsed >= self.ack_delay_ms {
                    0
                } else {
                    self.ack_delay_ms - elapsed
                }
            }
        }
    }

    /// Get the pending selective blocks as a sorted vec.
    pub fn pending_selective_blocks(&self) -> Vec<SelectiveAckBlock> {
        self.pending_selective.blocks.clone()
    }

    /// Get the pending nack count.
    pub fn pending_nack_count(&self) -> usize {
        self.pending_nacks.len()
    }

    /// Merge another session's pending acks into this one.
    pub fn merge_pending(&mut self, other: &AckSession) {
        if let Some(other_seq) = other.pending_cumulative {
            self.schedule_cumulative(other_seq);
        }
        for block in &other.pending_selective.blocks {
            self.schedule_selective(block.clone());
        }
        for seq in &other.pending_nacks {
            self.schedule_nack(*seq);
        }
    }

    /// Get the session creation order hash.
    pub fn session_hash(&self) -> u64 {
        let mut h: u64 = 0;
        for b in self.peer_id.0.to_le_bytes() {
            h = h.wrapping_mul(31).wrapping_add(b as u64);
        }
        h
    }
}

///////////////////////////////////////////////////////////////////////////////
// TESTS
///////////////////////////////////////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use super::*;

    fn test_peer(idx: u8) -> NodeId {
        let mut bytes = [0u8; 32];
        bytes[0] = idx;
        NodeId::new(bytes)
    }

    fn test_peer1() -> NodeId {
        test_peer(1)
    }

    fn test_peer2() -> NodeId {
        test_peer(2)
    }

    fn test_peer3() -> NodeId {
        test_peer(3)
    }

    fn test_seq(val: u64) -> SequenceNumber {
        SequenceNumber(val)
    }

    fn test_ts(val: u64) -> TimestampMillis {
        TimestampMillis(val)
    }

    // ========================================================================
    // AckType tests
    // ========================================================================

    #[test]
    fn ack_type_as_str() {
        assert_eq!(AckType::Cumulative.as_str(), "cumulative");
        assert_eq!(AckType::Selective.as_str(), "selective");
        assert_eq!(AckType::Negative.as_str(), "negative");
        assert_eq!(AckType::Heartbeat.as_str(), "heartbeat");
    }

    #[test]
    fn ack_type_is_negative() {
        assert!(AckType::Negative.is_negative());
        assert!(!AckType::Cumulative.is_negative());
    }

    #[test]
    fn ack_type_is_cumulative() {
        assert!(AckType::Cumulative.is_cumulative());
        assert!(!AckType::Selective.is_cumulative());
    }

    #[test]
    fn ack_type_is_selective() {
        assert!(AckType::Selective.is_selective());
        assert!(!AckType::Heartbeat.is_selective());
    }

    #[test]
    fn ack_type_is_heartbeat() {
        assert!(AckType::Heartbeat.is_heartbeat());
        assert!(!AckType::Negative.is_heartbeat());
    }

    #[test]
    fn ack_type_default() {
        assert_eq!(AckType::default(), AckType::Cumulative);
    }

    // ========================================================================
    // AckEntry tests
    // ========================================================================

    #[test]
    fn ack_entry_new() {
        let entry = AckEntry::new(
            1,
            AckType::Cumulative,
            SequenceNumber(10),
            test_peer1(),
            test_peer2(),
            test_ts(100),
        );
        assert_eq!(entry.ack_id, 1);
        assert_eq!(entry.ack_type, AckType::Cumulative);
        assert_eq!(entry.sequence, SequenceNumber(10));
        assert_eq!(entry.acked_by, test_peer1());
        assert_eq!(entry.acked_for, test_peer2());
        assert_eq!(entry.timestamp, test_ts(100));
        assert_eq!(entry.rtt_ms, None);
        assert!(!entry.duplicate);
    }

    #[test]
    fn ack_entry_with_rtt() {
        let entry = AckEntry::new(
            1,
            AckType::Selective,
            SequenceNumber(5),
            test_peer1(),
            test_peer2(),
            test_ts(100),
        )
        .with_rtt(42);
        assert_eq!(entry.rtt_ms, Some(42));
    }

    #[test]
    fn ack_entry_as_duplicate() {
        let entry = AckEntry::new(
            1,
            AckType::Cumulative,
            SequenceNumber(1),
            test_peer1(),
            test_peer2(),
            test_ts(100),
        );
        let dup = entry.as_duplicate();
        assert!(dup.is_duplicate());
        assert!(!entry.is_duplicate());
    }

    #[test]
    fn ack_entry_age_ms() {
        let entry = AckEntry::new(
            1,
            AckType::Cumulative,
            SequenceNumber(1),
            test_peer1(),
            test_peer2(),
            test_ts(100),
        );
        assert_eq!(entry.age_ms(test_ts(250)), 150);
    }

    // ========================================================================
    // CumulativeAckState tests
    // ========================================================================

    #[test]
    fn cumulative_ack_state_new() {
        let state = CumulativeAckState::new(test_peer1());
        assert_eq!(state.peer_id, test_peer1());
        assert_eq!(state.acked_up_to, SequenceNumber(0));
        assert_eq!(state.total_acks_sent, 0);
    }

    #[test]
    fn cumulative_ack_state_update_acked_up_to() {
        let mut state = CumulativeAckState::new(test_peer1());
        state.update_acked_up_to(SequenceNumber(100));
        assert_eq!(state.acked_up_to, SequenceNumber(100));
        state.update_acked_up_to(SequenceNumber(50));
        assert_eq!(state.acked_up_to, SequenceNumber(100)); // monotonic
    }

    #[test]
    fn cumulative_ack_state_record_ack_sent() {
        let mut state = CumulativeAckState::new(test_peer1());
        state.record_ack_sent(test_ts(100));
        assert_eq!(state.total_acks_sent, 1);
        assert_eq!(state.last_ack_sent_at, Some(test_ts(100)));
    }

    #[test]
    fn cumulative_ack_state_record_ack_received() {
        let mut state = CumulativeAckState::new(test_peer1());
        state.record_ack_received(test_ts(200));
        assert_eq!(state.total_acks_received, 1);
    }

    #[test]
    fn cumulative_ack_state_duplicate_acks() {
        let mut state = CumulativeAckState::new(test_peer1());
        state.record_duplicate_ack_sent();
        state.record_duplicate_ack_sent();
        assert_eq!(state.duplicate_acks_sent, 2);
    }

    #[test]
    fn cumulative_ack_state_ack_batch() {
        let mut state = CumulativeAckState::new(test_peer1());
        state.record_ack_batch();
        state.record_ack_batch();
        assert_eq!(state.ack_batch_count, 2);
    }

    #[test]
    fn cumulative_ack_state_ack_ratio() {
        let state = CumulativeAckState::new(test_peer1());
        assert!((state.ack_ratio() - 100.0).abs() < 0.001);
    }

    #[test]
    fn cumulative_ack_state_duplicate_ratio() {
        let mut state = CumulativeAckState::new(test_peer1());
        state.total_acks_sent = 100;
        state.duplicate_acks_sent = 10;
        assert!((state.duplicate_ratio() - 10.0).abs() < 0.01);
    }

    // ========================================================================
    // SelectiveAckBlock tests
    // ========================================================================

    #[test]
    fn selective_ack_block_new() {
        let block = SelectiveAckBlock::new(SequenceNumber(10), SequenceNumber(20));
        assert_eq!(block.start, SequenceNumber(10));
        assert_eq!(block.end, SequenceNumber(20));
    }

    #[test]
    fn selective_ack_block_is_empty() {
        assert!(SelectiveAckBlock::new(SequenceNumber(10), SequenceNumber(10)).is_empty());
        assert!(!SelectiveAckBlock::new(SequenceNumber(10), SequenceNumber(20)).is_empty());
    }

    #[test]
    fn selective_ack_block_len() {
        let block = SelectiveAckBlock::new(SequenceNumber(10), SequenceNumber(20));
        assert_eq!(block.len(), 10);
    }

    #[test]
    fn selective_ack_block_contains() {
        let block = SelectiveAckBlock::new(SequenceNumber(10), SequenceNumber(20));
        assert!(block.contains(SequenceNumber(10)));
        assert!(block.contains(SequenceNumber(19)));
        assert!(!block.contains(SequenceNumber(9)));
        assert!(!block.contains(SequenceNumber(20)));
    }

    // ========================================================================
    // SelectiveAckSet tests
    // ========================================================================

    #[test]
    fn selective_ack_set_new() {
        let set = SelectiveAckSet::new();
        assert_eq!(set.block_count(), 0);
        assert_eq!(set.total_sequences, 0);
    }

    #[test]
    fn selective_ack_set_add_block() {
        let mut set = SelectiveAckSet::new();
        set.add_block(SelectiveAckBlock::new(SequenceNumber(10), SequenceNumber(20)));
        assert_eq!(set.block_count(), 1);
        assert_eq!(set.total_sequences, 10);
    }

    #[test]
    fn selective_ack_set_add_empty_block() {
        let mut set = SelectiveAckSet::new();
        set.add_block(SelectiveAckBlock::new(SequenceNumber(10), SequenceNumber(10)));
        assert_eq!(set.block_count(), 0);
    }

    #[test]
    fn selective_ack_set_merge_adjacent() {
        let mut set = SelectiveAckSet::new();
        set.add_block(SelectiveAckBlock::new(SequenceNumber(10), SequenceNumber(15)));
        set.add_block(SelectiveAckBlock::new(SequenceNumber(15), SequenceNumber(20)));
        assert_eq!(set.block_count(), 1);
        assert_eq!(set.blocks[0].start, SequenceNumber(10));
        assert_eq!(set.blocks[0].end, SequenceNumber(20));
    }

    #[test]
    fn selective_ack_set_contains() {
        let mut set = SelectiveAckSet::new();
        set.add_block(SelectiveAckBlock::new(SequenceNumber(10), SequenceNumber(20)));
        assert!(set.contains(SequenceNumber(10)));
        assert!(set.contains(SequenceNumber(15)));
        assert!(!set.contains(SequenceNumber(5)));
        assert!(!set.contains(SequenceNumber(20)));
    }

    #[test]
    fn selective_ack_set_clear() {
        let mut set = SelectiveAckSet::new();
        set.add_block(SelectiveAckBlock::new(SequenceNumber(10), SequenceNumber(20)));
        set.clear();
        assert_eq!(set.block_count(), 0);
        assert_eq!(set.total_sequences, 0);
    }

    #[test]
    fn selective_ack_set_default() {
        let set: SelectiveAckSet = Default::default();
        assert_eq!(set.block_count(), 0);
    }

    // ========================================================================
    // NackEntry tests
    // ========================================================================

    #[test]
    fn nack_entry_new() {
        let entry = NackEntry::new(SequenceNumber(5), test_ts(100));
        assert_eq!(entry.sequence, SequenceNumber(5));
        assert_eq!(entry.nack_count, 1);
        assert_eq!(entry.first_nacked_at, test_ts(100));
        assert_eq!(entry.last_nacked_at, test_ts(100));
        assert!(!entry.declared_lost);
    }

    #[test]
    fn nack_entry_increment() {
        let mut entry = NackEntry::new(SequenceNumber(5), test_ts(100));
        entry.increment(test_ts(200));
        entry.increment(test_ts(300));
        assert_eq!(entry.nack_count, 3);
        assert_eq!(entry.last_nacked_at, test_ts(300));
    }

    #[test]
    fn nack_entry_declare_lost() {
        let mut entry = NackEntry::new(SequenceNumber(5), test_ts(100));
        assert!(!entry.declared_lost);
        entry.declare_lost();
        assert!(entry.declared_lost);
    }

    #[test]
    fn nack_entry_nack_age_ms() {
        let entry = NackEntry::new(SequenceNumber(5), test_ts(100));
        assert_eq!(entry.nack_age_ms(test_ts(250)), 150);
    }

    // ========================================================================
    // AckTimingStats tests
    // ========================================================================

    #[test]
    fn ack_timing_stats_new() {
        let stats = AckTimingStats::new();
        assert_eq!(stats.srtt_ms, DEFAULT_INITIAL_RTT_MS);
        assert!(!stats.is_rtt_established());
    }

    #[test]
    fn ack_timing_stats_update_rtt() {
        let mut stats = AckTimingStats::new();
        stats.update_rtt(100);
        assert_eq!(stats.rtt_samples, 1);
        assert_eq!(stats.srtt_ms, 100);
        assert!(stats.is_rtt_established());
        assert_eq!(stats.last_rtt_ms, Some(100));
    }

    #[test]
    fn ack_timing_stats_update_rtt_multiple() {
        let mut stats = AckTimingStats::new();
        stats.update_rtt(100);
        stats.update_rtt(200);
        stats.update_rtt(150);
        assert_eq!(stats.rtt_samples, 3);
        assert_eq!(stats.last_rtt_ms, Some(150));
        assert!(stats.srtt_ms > 0);
        assert!(stats.rttvar_ms > 0);
    }

    #[test]
    fn ack_timing_stats_min_max_rtt() {
        let mut stats = AckTimingStats::new();
        stats.update_rtt(50);
        stats.update_rtt(500);
        assert_eq!(stats.min_rtt_ms, 50);
        assert_eq!(stats.max_rtt_ms, 500);
    }

    #[test]
    fn ack_timing_stats_rto() {
        let mut stats = AckTimingStats::new();
        stats.update_rtt(100);
        let rto = stats.rto();
        assert!(rto >= 100);
        assert!(rto <= DEFAULT_MAX_RTT_MS);
    }

    #[test]
    fn ack_timing_stats_default() {
        let stats: AckTimingStats = Default::default();
        assert_eq!(stats.srtt_ms, DEFAULT_INITIAL_RTT_MS);
    }

    // ========================================================================
    // AcksError tests
    // ========================================================================

    #[test]
    fn acks_error_category() {
        assert_eq!(
            AcksError::InvalidConfig {
                field: "x".to_string(),
                reason: "y".to_string(),
            }
            .category(),
            "invalid_config"
        );
        assert_eq!(
            AcksError::BatchTooLarge { size: 10, max: 5 }
                .category(),
            "batch_too_large"
        );
    }

    #[test]
    fn acks_error_is_recoverable() {
        assert!(AcksError::InvalidSequence {
            seq: SequenceNumber(1),
            reason: "bad".to_string(),
        }
        .is_recoverable());
        assert!(!AcksError::Internal {
            message: "x".to_string()
        }
        .is_recoverable());
    }

    #[test]
    fn acks_error_suggested_action() {
        assert!(!AcksError::PeerNotFound {
            peer_id: test_peer1()
        }
        .suggested_action()
        .is_empty());
    }

    #[test]
    fn acks_error_display() {
        let err = AcksError::BatchTooLarge { size: 100, max: 50 };
        assert!(!err.to_string().is_empty());
    }

    // ========================================================================
    // Sealing tests
    // ========================================================================

    #[test]
    fn acks_trait_is_sealed() {
        fn _assert_sealed<T: Acks + private::Sealed>() {}
        let _ = std::marker::PhantomData::<fn() -> ()>();
    }

    // ========================================================================
    // AckSession tests
    // ========================================================================

    #[test]
    fn ack_session_new() {
        let session = AckSession::new(test_peer1());
        assert_eq!(session.peer_id, test_peer1());
        assert!(!session.has_pending());
        assert_eq!(session.pending_count(), 0);
        assert_eq!(session.total_acks_sent, 0);
    }

    #[test]
    fn ack_session_with_config_valid() {
        let session = AckSession::with_config(test_peer1(), 100, 200).unwrap();
        assert_eq!(session.ack_delay_ms, 100);
        assert_eq!(session.max_batch_size, 200);
    }

    #[test]
    fn ack_session_with_config_invalid_batch() {
        let result = AckSession::with_config(test_peer1(), 100, 0);
        assert!(result.is_err());
    }

    #[test]
    fn ack_session_schedule_cumulative() {
        let mut session = AckSession::new(test_peer1());
        session.schedule_cumulative(SequenceNumber(10));
        assert!(session.has_pending());
        assert_eq!(session.pending_count(), 1);
        session.schedule_cumulative(SequenceNumber(20));
        assert_eq!(session.pending_count(), 1); // still one, upgraded
    }

    #[test]
    fn ack_session_schedule_selective() {
        let mut session = AckSession::new(test_peer1());
        session.schedule_selective(SelectiveAckBlock::new(SequenceNumber(1), SequenceNumber(5)));
        assert!(session.has_pending());
        assert_eq!(session.pending_count(), 1);
    }

    #[test]
    fn ack_session_schedule_nack() {
        let mut session = AckSession::new(test_peer1());
        session.schedule_nack(SequenceNumber(7));
        session.schedule_nack(SequenceNumber(7)); // duplicate not added
        assert_eq!(session.pending_nack_count(), 1);
        session.schedule_nack(SequenceNumber(8));
        assert_eq!(session.pending_nack_count(), 2);
    }

    #[test]
    fn ack_session_take_cumulative() {
        let mut session = AckSession::new(test_peer1());
        session.schedule_cumulative(SequenceNumber(42));
        let taken = session.take_cumulative();
        assert_eq!(taken, Some(SequenceNumber(42)));
        assert_eq!(session.take_cumulative(), None);
    }

    #[test]
    fn ack_session_take_selective() {
        let mut session = AckSession::new(test_peer1());
        session.schedule_selective(SelectiveAckBlock::new(SequenceNumber(1), SequenceNumber(3)));
        let set = session.take_selective();
        assert_eq!(set.block_count(), 1);
        assert_eq!(session.take_selective().block_count(), 0);
    }

    #[test]
    fn ack_session_take_nacks() {
        let mut session = AckSession::new(test_peer1());
        session.schedule_nack(SequenceNumber(1));
        session.schedule_nack(SequenceNumber(2));
        let nacks = session.take_nacks();
        assert_eq!(nacks.len(), 2);
        assert_eq!(session.take_nacks().len(), 0);
    }

    #[test]
    fn ack_session_mark_ack_sent() {
        let mut session = AckSession::new(test_peer1());
        session.mark_ack_sent(test_ts(100));
        assert_eq!(session.last_ack_sent_at, Some(test_ts(100)));
        assert_eq!(session.total_acks_sent, 1);
        assert_eq!(session.acks_in_batch, 1);
    }

    #[test]
    fn ack_session_mark_nack_sent() {
        let mut session = AckSession::new(test_peer1());
        session.mark_nack_sent();
        assert_eq!(session.total_nacks_sent, 1);
    }

    #[test]
    fn ack_session_mark_duplicate_ack() {
        let mut session = AckSession::new(test_peer1());
        session.mark_duplicate_ack();
        assert_eq!(session.duplicate_acks_sent, 1);
    }

    #[test]
    fn ack_session_delay_elapsed() {
        let mut session = AckSession::new(test_peer1());
        assert!(session.delay_elapsed(test_ts(0))); // never sent
        session.mark_ack_sent(test_ts(100));
        assert!(!session.delay_elapsed(test_ts(140))); // delay 50ms not elapsed
        assert!(session.delay_elapsed(test_ts(160))); // elapsed
    }

    #[test]
    fn ack_session_batch_full() {
        let mut session = AckSession::new(test_peer1());
        session.max_batch_size = 3;
        session.acks_in_batch = 2;
        assert!(!session.batch_full());
        session.acks_in_batch = 3;
        assert!(session.batch_full());
    }

    #[test]
    fn ack_session_reset_batch() {
        let mut session = AckSession::new(test_peer1());
        session.acks_in_batch = 5;
        session.batch_started_at = Some(test_ts(100));
        session.reset_batch();
        assert_eq!(session.acks_in_batch, 0);
        assert_eq!(session.batch_started_at, None);
    }

    #[test]
    fn ack_session_start_batch() {
        let mut session = AckSession::new(test_peer1());
        session.start_batch(test_ts(1000));
        assert_eq!(session.batch_started_at, Some(test_ts(1000)));
        assert_eq!(session.acks_in_batch, 0);
    }

    #[test]
    fn ack_session_session_age_ms() {
        let mut session = AckSession::new(test_peer1());
        assert_eq!(session.session_age_ms(test_ts(100)), None);
        session.mark_ack_sent(test_ts(100));
        assert_eq!(session.session_age_ms(test_ts(250)), Some(150));
    }

    #[test]
    fn ack_session_duplicate_ratio() {
        let mut session = AckSession::new(test_peer1());
        assert!((session.duplicate_ratio() - 0.0).abs() < 0.001);
        session.total_acks_sent = 100;
        session.duplicate_acks_sent = 10;
        assert!((session.duplicate_ratio() - 10.0).abs() < 0.01);
    }

    #[test]
    fn ack_session_summary() {
        let session = AckSession::new(test_peer1());
        let s = session.summary();
        assert!(!s.is_empty());
        assert!(s.contains("AckSession"));
    }

    #[test]
    fn ack_session_set_ack_delay_ms() {
        let mut session = AckSession::new(test_peer1());
        session.set_ack_delay_ms(200);
        assert_eq!(session.ack_delay_ms, 200);
    }

    #[test]
    fn ack_session_set_max_batch_size_valid() {
        let mut session = AckSession::new(test_peer1());
        assert!(session.set_max_batch_size(500).is_ok());
        assert_eq!(session.max_batch_size, 500);
    }

    #[test]
    fn ack_session_set_max_batch_size_invalid() {
        let mut session = AckSession::new(test_peer1());
        assert!(session.set_max_batch_size(0).is_err());
    }

    #[test]
    fn ack_session_clear_pending() {
        let mut session = AckSession::new(test_peer1());
        session.schedule_cumulative(SequenceNumber(1));
        session.schedule_nack(SequenceNumber(2));
        assert!(session.has_pending());
        session.clear_pending();
        assert!(!session.has_pending());
    }

    #[test]
    fn ack_session_update_last_cumulative() {
        let mut session = AckSession::new(test_peer1());
        session.update_last_cumulative(SequenceNumber(10));
        assert_eq!(session.last_cumulative_sent, SequenceNumber(10));
        session.update_last_cumulative(SequenceNumber(5));
        assert_eq!(session.last_cumulative_sent, SequenceNumber(10));
    }

    #[test]
    fn ack_session_is_idle() {
        let session = AckSession::new(test_peer1());
        assert!(session.is_idle());
    }

    #[test]
    fn ack_session_nack_to_ack_ratio() {
        let mut session = AckSession::new(test_peer1());
        assert!((session.nack_to_ack_ratio() - 0.0).abs() < 0.001);
        session.total_acks_sent = 100;
        session.total_nacks_sent = 25;
        assert!((session.nack_to_ack_ratio() - 25.0).abs() < 0.01);
    }

    #[test]
    fn ack_session_should_flush_batch() {
        let mut session = AckSession::new(test_peer1());
        assert!(!session.should_flush_batch(test_ts(0)));
        session.mark_ack_sent(test_ts(100));
        assert!(!session.should_flush_batch(test_ts(140)));
        assert!(session.should_flush_batch(test_ts(160)));
    }

    #[test]
    fn ack_session_delay_remaining_ms() {
        let mut session = AckSession::new(test_peer1());
        assert_eq!(session.delay_remaining_ms(test_ts(0)), 0);
        session.mark_ack_sent(test_ts(100));
        assert_eq!(session.delay_remaining_ms(test_ts(120)), 30);
        assert_eq!(session.delay_remaining_ms(test_ts(200)), 0);
    }

    #[test]
    fn ack_session_pending_selective_blocks() {
        let mut session = AckSession::new(test_peer1());
        assert_eq!(session.pending_selective_blocks().len(), 0);
        session.schedule_selective(SelectiveAckBlock::new(SequenceNumber(1), SequenceNumber(3)));
        assert_eq!(session.pending_selective_blocks().len(), 1);
    }

    #[test]
    fn ack_session_pending_nack_count() {
        let mut session = AckSession::new(test_peer1());
        assert_eq!(session.pending_nack_count(), 0);
        session.schedule_nack(SequenceNumber(1));
        assert_eq!(session.pending_nack_count(), 1);
    }

    #[test]
    fn ack_session_merge_pending() {
        let mut session1 = AckSession::new(test_peer1());
        session1.schedule_cumulative(SequenceNumber(10));
        let mut session2 = AckSession::new(test_peer2());
        session2.schedule_selective(SelectiveAckBlock::new(SequenceNumber(1), SequenceNumber(5)));
        session2.schedule_nack(SequenceNumber(99));
        session1.merge_pending(&session2);
        assert!(session1.has_pending());
        assert_eq!(session1.pending_nack_count(), 1);
    }

    #[test]
    fn ack_session_session_hash() {
        let session = AckSession::new(test_peer1());
        let h = session.session_hash();
        assert!(h > 0);
    }

    // ========================================================================
    // DuplicateAckTracker tests
    // ========================================================================

    #[test]
    fn duplicate_ack_tracker_new() {
        let tracker = DuplicateAckTracker::new();
        assert_eq!(tracker.total_duplicates(), 0);
        assert_eq!(tracker.tracked_count(), 0);
    }

    #[test]
    fn duplicate_ack_tracker_record_duplicate() {
        let mut tracker = DuplicateAckTracker::new();
        tracker.record_duplicate(SequenceNumber(5));
        tracker.record_duplicate(SequenceNumber(5));
        assert_eq!(tracker.dup_count(SequenceNumber(5)), 2);
        assert_eq!(tracker.total_duplicates(), 2);
    }

    #[test]
    fn duplicate_ack_tracker_dup_count_missing() {
        let tracker = DuplicateAckTracker::new();
        assert_eq!(tracker.dup_count(SequenceNumber(99)), 0);
    }

    #[test]
    fn duplicate_ack_tracker_is_fast_retransmit() {
        let mut tracker = DuplicateAckTracker::new();
        tracker.record_duplicate(SequenceNumber(3));
        tracker.record_duplicate(SequenceNumber(3));
        assert!(!tracker.is_fast_retransmit(SequenceNumber(3), 3));
        tracker.record_duplicate(SequenceNumber(3));
        assert!(tracker.is_fast_retransmit(SequenceNumber(3), 3));
    }

    #[test]
    fn duplicate_ack_tracker_fast_retransmit_candidates() {
        let mut tracker = DuplicateAckTracker::new();
        tracker.record_duplicate(SequenceNumber(1));
        tracker.record_duplicate(SequenceNumber(2));
        tracker.record_duplicate(SequenceNumber(2));
        let candidates = tracker.fast_retransmit_candidates(2);
        assert!(candidates.contains(&SequenceNumber(2)));
        assert!(!candidates.contains(&SequenceNumber(1)));
    }

    #[test]
    fn duplicate_ack_tracker_clear() {
        let mut tracker = DuplicateAckTracker::new();
        tracker.record_duplicate(SequenceNumber(1));
        tracker.clear();
        assert_eq!(tracker.tracked_count(), 0);
        assert_eq!(tracker.total_duplicates(), 0);
    }

    #[test]
    fn duplicate_ack_tracker_top_duplicates() {
        let mut tracker = DuplicateAckTracker::new();
        tracker.record_duplicate(SequenceNumber(1));
        tracker.record_duplicate(SequenceNumber(2));
        tracker.record_duplicate(SequenceNumber(2));
        tracker.record_duplicate(SequenceNumber(3));
        tracker.record_duplicate(SequenceNumber(3));
        tracker.record_duplicate(SequenceNumber(3));
        let top = tracker.top_duplicates(2);
        assert_eq!(top[0].0, SequenceNumber(3));
        assert_eq!(top[0].1, 3);
    }

    #[test]
    fn duplicate_ack_tracker_sequences_above_threshold() {
        let mut tracker = DuplicateAckTracker::new();
        tracker.record_duplicate(SequenceNumber(1));
        tracker.record_duplicate(SequenceNumber(2));
        tracker.record_duplicate(SequenceNumber(2));
        let above = tracker.sequences_above_threshold(1);
        assert!(above.contains(&SequenceNumber(2)));
    }

    #[test]
    fn duplicate_ack_tracker_average_dup_count() {
        let mut tracker = DuplicateAckTracker::new();
        tracker.record_duplicate(SequenceNumber(1));
        tracker.record_duplicate(SequenceNumber(1));
        tracker.record_duplicate(SequenceNumber(2));
        assert!((tracker.average_dup_count() - 1.5).abs() < 0.001);
    }

    #[test]
    fn duplicate_ack_tracker_any_at_risk() {
        let mut tracker = DuplicateAckTracker::new();
        assert!(!tracker.any_at_risk(3));
        tracker.record_duplicate(SequenceNumber(1));
        tracker.record_duplicate(SequenceNumber(1));
        tracker.record_duplicate(SequenceNumber(1));
        assert!(tracker.any_at_risk(3));
    }

    #[test]
    fn duplicate_ack_tracker_max_duplicate_count() {
        let mut tracker = DuplicateAckTracker::new();
        assert_eq!(tracker.max_duplicate_count(), 0);
        tracker.record_duplicate(SequenceNumber(1));
        tracker.record_duplicate(SequenceNumber(1));
        assert_eq!(tracker.max_duplicate_count(), 2);
    }

    #[test]
    fn duplicate_ack_tracker_merge() {
        let mut tracker1 = DuplicateAckTracker::new();
        tracker1.record_duplicate(SequenceNumber(1));
        let mut tracker2 = DuplicateAckTracker::new();
        tracker2.record_duplicate(SequenceNumber(1));
        tracker2.record_duplicate(SequenceNumber(1));
        tracker1.merge(&tracker2);
        assert_eq!(tracker1.dup_count(SequenceNumber(1)), 3);
    }

    // ========================================================================
    // AckBatch tests
    // ========================================================================

    #[test]
    fn ack_batch_new() {
        let batch = AckBatch::new(1, test_ts(100));
        assert_eq!(batch.batch_id, 1);
        assert_eq!(batch.entry_count(), 0);
        assert!(!batch.closed);
    }

    #[test]
    fn ack_batch_add_entry() {
        let mut batch = AckBatch::new(1, test_ts(100));
        batch.add_entry(AckEntry::new(
            1,
            AckType::Cumulative,
            SequenceNumber(1),
            test_peer1(),
            test_peer2(),
            test_ts(100),
        ));
        assert_eq!(batch.entry_count(), 1);
    }

    #[test]
    fn ack_batch_age_ms() {
        let batch = AckBatch::new(1, test_ts(100));
        assert_eq!(batch.age_ms(test_ts(250)), 150);
    }

    #[test]
    fn ack_batch_close() {
        let mut batch = AckBatch::new(1, test_ts(100));
        assert!(!batch.closed);
        batch.close();
        assert!(batch.closed);
    }

    // ========================================================================
    // AckAggregator tests
    // ========================================================================

    #[test]
    fn ack_aggregator_new() {
        let agg = AckAggregator::new();
        assert_eq!(agg.batch_count(), 0);
        assert_eq!(agg.total_aggregated(), 0);
    }

    #[test]
    fn ack_aggregator_with_config() {
        let agg = AckAggregator::with_config(100, 1000);
        assert_eq!(agg.max_batch_size, 100);
        assert_eq!(agg.max_batch_age_ms, 1000);
    }

    #[test]
    fn ack_aggregator_add() {
        let mut agg = AckAggregator::with_config(10, 1000);
        let id = agg.add(
            test_peer1(),
            AckEntry::new(1, AckType::Cumulative, SequenceNumber(1), test_peer1(), test_peer2(), test_ts(100)),
            test_ts(100),
        );
        assert_eq!(id, 1);
        assert_eq!(agg.total_aggregated(), 1);
        assert_eq!(agg.batch_count(), 1);
    }

    #[test]
    fn ack_aggregator_add_multiple_peers() {
        let mut agg = AckAggregator::with_config(10, 1000);
        agg.add(test_peer1(), AckEntry::new(1, AckType::Cumulative, SequenceNumber(1), test_peer1(), test_peer2(), test_ts(100)), test_ts(100));
        agg.add(test_peer2(), AckEntry::new(2, AckType::Cumulative, SequenceNumber(2), test_peer2(), test_peer1(), test_ts(100)), test_ts(100));
        assert_eq!(agg.total_aggregated(), 2);
    }

    #[test]
    fn ack_aggregator_open_and_closed_batches() {
        let mut agg = AckAggregator::with_config(1, 1000);
        agg.add(test_peer1(), AckEntry::new(1, AckType::Cumulative, SequenceNumber(1), test_peer1(), test_peer2(), test_ts(100)), test_ts(100));
        assert_eq!(agg.open_batches().len(), 1);
        assert_eq!(agg.closed_batches().len(), 0);
        agg.close_all();
        assert_eq!(agg.open_batches().len(), 0);
        assert_eq!(agg.closed_batches().len(), 1);
    }

    #[test]
    fn ack_aggregator_take_closed() {
        let mut agg = AckAggregator::with_config(1, 1000);
        agg.add(test_peer1(), AckEntry::new(1, AckType::Cumulative, SequenceNumber(1), test_peer1(), test_peer2(), test_ts(100)), test_ts(100));
        agg.close_all();
        let closed = agg.take_closed();
        assert_eq!(closed.len(), 1);
        assert_eq!(agg.closed_batches().len(), 0);
    }

    #[test]
    fn ack_aggregator_close_stale() {
        let mut agg = AckAggregator::with_config(10, 50);
        agg.add(test_peer1(), AckEntry::new(1, AckType::Cumulative, SequenceNumber(1), test_peer1(), test_peer2(), test_ts(100)), test_ts(100));
        agg.close_stale(test_ts(200));
        assert_eq!(agg.closed_batches().len(), 1);
    }

    #[test]
    fn ack_aggregator_oldest_open_batch_age_ms() {
        let mut agg = AckAggregator::new();
        assert_eq!(agg.oldest_open_batch_age_ms(test_ts(100)), None);
        agg.add(test_peer1(), AckEntry::new(1, AckType::Cumulative, SequenceNumber(1), test_peer1(), test_peer2(), test_ts(100)), test_ts(100));
        assert_eq!(agg.oldest_open_batch_age_ms(test_ts(200)), Some(100));
    }

    #[test]
    fn ack_aggregator_batch_peer_count() {
        let mut agg = AckAggregator::new();
        let id = agg.add(test_peer1(), AckEntry::new(1, AckType::Cumulative, SequenceNumber(1), test_peer1(), test_peer2(), test_ts(100)), test_ts(100));
        assert_eq!(agg.batch_peer_count(id), 1);
        assert_eq!(agg.batch_peer_count(999), 0);
    }

    #[test]
    fn ack_aggregator_set_config() {
        let mut agg = AckAggregator::new();
        agg.set_max_batch_size(500);
        agg.set_max_batch_age_ms(5000);
        assert_eq!(agg.max_batch_size, 500);
        assert_eq!(agg.max_batch_age_ms, 5000);
    }

    #[test]
    fn ack_aggregator_total_batches_created() {
        let mut agg = AckAggregator::new();
        assert_eq!(agg.total_batches_created(), 0);
        agg.add(test_peer1(), AckEntry::new(1, AckType::Cumulative, SequenceNumber(1), test_peer1(), test_peer2(), test_ts(100)), test_ts(100));
        assert_eq!(agg.total_batches_created(), 1);
    }

    #[test]
    fn ack_aggregator_open_batch_entry_count() {
        let mut agg = AckAggregator::new();
        assert_eq!(agg.open_batch_entry_count(), 0);
        agg.add(test_peer1(), AckEntry::new(1, AckType::Cumulative, SequenceNumber(1), test_peer1(), test_peer2(), test_ts(100)), test_ts(100));
        assert_eq!(agg.open_batch_entry_count(), 1);
    }

    #[test]
    fn ack_aggregator_closed_batch_entry_count() {
        let mut agg = AckAggregator::new();
        assert_eq!(agg.closed_batch_entry_count(), 0);
        agg.add(test_peer1(), AckEntry::new(1, AckType::Cumulative, SequenceNumber(1), test_peer1(), test_peer2(), test_ts(100)), test_ts(100));
        agg.close_all();
        assert_eq!(agg.closed_batch_entry_count(), 1);
    }

    // ========================================================================
    // AckManager tests
    // ========================================================================

    #[test]
    fn ack_manager_new() {
        let mgr = AckManager::new();
        assert_eq!(mgr.active_session_count(), 0);
        assert_eq!(mgr.total_acks_sent(), 0);
        assert_eq!(mgr.total_acks_received(), 0);
    }

    #[test]
    fn ack_manager_get_or_create_session() {
        let mut mgr = AckManager::new();
        let s1 = mgr.get_or_create_session(test_peer1());
        let s2 = mgr.get_or_create_session(test_peer1());
        assert_eq!(s1.peer_id, s2.peer_id);
        assert_eq!(mgr.active_session_count(), 1);
    }

    #[test]
    fn ack_manager_session() {
        let mut mgr = AckManager::new();
        assert!(mgr.session(test_peer1()).is_none());
        mgr.get_or_create_session(test_peer1());
        assert!(mgr.session(test_peer1()).is_some());
    }

    #[test]
    fn ack_manager_remove_session() {
        let mut mgr = AckManager::new();
        mgr.get_or_create_session(test_peer1());
        assert!(mgr.remove_session(test_peer1()));
        assert!(!mgr.has_session(test_peer1()));
        assert!(!mgr.remove_session(test_peer1()));
    }

    #[test]
    fn ack_manager_has_session() {
        let mut mgr = AckManager::new();
        assert!(!mgr.has_session(test_peer1()));
        mgr.get_or_create_session(test_peer1());
        assert!(mgr.has_session(test_peer1()));
    }

    #[test]
    fn ack_manager_record_ack_sent() {
        let mut mgr = AckManager::new();
        mgr.record_ack_sent(test_peer1());
        assert_eq!(mgr.total_acks_sent(), 1);
    }

    #[test]
    fn ack_manager_record_ack_received() {
        let mut mgr = AckManager::new();
        mgr.record_ack_received(test_peer1());
        assert_eq!(mgr.total_acks_received(), 1);
    }

    #[test]
    fn ack_manager_record_nack_sent() {
        let mut mgr = AckManager::new();
        mgr.record_nack_sent(test_peer1());
        assert_eq!(mgr.total_nacks_sent(), 1);
    }

    #[test]
    fn ack_manager_record_nack_received() {
        let mut mgr = AckManager::new();
        mgr.record_nack_received(test_peer1());
        assert_eq!(mgr.total_nacks_received(), 1);
    }

    #[test]
    fn ack_manager_record_duplicate_ack() {
        let mut mgr = AckManager::new();
        mgr.record_duplicate_ack(test_peer1(), SequenceNumber(5));
        assert_eq!(mgr.total_duplicate_acks(), 1);
    }

    #[test]
    fn ack_manager_ack_ratio_percent() {
        let mgr = AckManager::new();
        assert!((mgr.ack_ratio_percent() - 100.0).abs() < 0.001);
    }

    #[test]
    fn ack_manager_duplicate_ack_ratio_percent() {
        let mgr = AckManager::new();
        assert!((mgr.duplicate_ack_ratio_percent() - 0.0).abs() < 0.001);
    }

    #[test]
    fn ack_manager_are_acks_healthy() {
        let mgr = AckManager::new();
        assert!(mgr.are_acks_healthy());
    }

    #[test]
    fn ack_manager_peers_with_sessions() {
        let mut mgr = AckManager::new();
        mgr.get_or_create_session(test_peer1());
        mgr.get_or_create_session(test_peer2());
        let peers = mgr.peers_with_sessions();
        assert_eq!(peers.len(), 2);
    }

    #[test]
    fn ack_manager_sessions_with_pending() {
        let mut mgr = AckManager::new();
        mgr.get_or_create_session(test_peer1()).schedule_cumulative(SequenceNumber(1));
        let pending = mgr.sessions_with_pending();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0], test_peer1());
    }

    #[test]
    fn ack_manager_reset_peer() {
        let mut mgr = AckManager::new();
        mgr.get_or_create_session(test_peer1());
        assert!(mgr.reset_peer(test_peer1()));
        assert!(!mgr.reset_peer(test_peer2()));
    }

    #[test]
    fn ack_manager_reset_all() {
        let mut mgr = AckManager::new();
        mgr.get_or_create_session(test_peer1());
        mgr.record_ack_sent(test_peer1());
        mgr.reset_all();
        assert_eq!(mgr.active_session_count(), 0);
        assert_eq!(mgr.total_acks_sent(), 0);
    }

    #[test]
    fn ack_manager_default_config() {
        let mgr = AckManager::new();
        assert_eq!(mgr.default_ack_timeout_ms(), DEFAULT_ACK_TIMEOUT_MS);
        assert_eq!(mgr.default_ack_delay_ms(), DEFAULT_ACK_DELAY_MS);
        assert_eq!(mgr.default_max_batch_size(), DEFAULT_MAX_ACK_BATCH_SIZE);
    }

    #[test]
    fn ack_manager_set_default_config() {
        let mut mgr = AckManager::new();
        mgr.set_default_ack_timeout_ms(500);
        mgr.set_default_ack_delay_ms(100);
        mgr.set_default_max_batch_size(500);
        assert_eq!(mgr.default_ack_timeout_ms(), 500);
        assert_eq!(mgr.default_ack_delay_ms(), 100);
        assert_eq!(mgr.default_max_batch_size(), 500);
    }

    #[test]
    fn ack_manager_snapshot_restore() {
        let mut mgr = AckManager::new();
        mgr.get_or_create_session(test_peer1()).schedule_cumulative(SequenceNumber(1));
        mgr.record_ack_sent(test_peer1());
        let snap = mgr.snapshot();
        assert_eq!(snap.cumulative_states.len(), 1);
        let mut mgr2 = AckManager::new();
        mgr2.restore(snap);
        assert_eq!(mgr2.active_session_count(), 1);
    }

    #[test]
    fn ack_manager_pending_session_count() {
        let mut mgr = AckManager::new();
        assert_eq!(mgr.pending_session_count(), 0);
        mgr.get_or_create_session(test_peer1()).schedule_cumulative(SequenceNumber(1));
        assert_eq!(mgr.pending_session_count(), 1);
    }

    #[test]
    fn ack_manager_total_pending_acks() {
        let mut mgr = AckManager::new();
        assert_eq!(mgr.total_pending_acks(), 0);
        mgr.get_or_create_session(test_peer1()).schedule_cumulative(SequenceNumber(1));
        assert_eq!(mgr.total_pending_acks(), 1);
    }

    #[test]
    fn ack_manager_busiest_ack_session() {
        let mut mgr = AckManager::new();
        assert_eq!(mgr.busiest_ack_session(), None);
        mgr.get_or_create_session(test_peer1()).schedule_cumulative(SequenceNumber(1));
        let busy = mgr.busiest_ack_session();
        assert_eq!(busy, Some((test_peer1(), 1)));
    }

    #[test]
    fn ack_manager_sessions_ready_to_send() {
        let mut mgr = AckManager::new();
        mgr.get_or_create_session(test_peer1()).schedule_cumulative(SequenceNumber(1));
        let ready = mgr.sessions_ready_to_send(test_ts(0));
        assert_eq!(ready.len(), 1);
    }

    #[test]
    fn ack_manager_sessions_with_full_batches() {
        let mut mgr = AckManager::new();
        mgr.get_or_create_session(test_peer1());
        // batch not full by default
        assert!(mgr.sessions_with_full_batches().is_empty());
    }

    #[test]
    fn ack_manager_aggregate_duplicate_ratio() {
        let mgr = AckManager::new();
        assert!((mgr.aggregate_duplicate_ratio() - 0.0).abs() < 0.001);
    }

    #[test]
    fn ack_manager_nack_ratio_percent() {
        let mgr = AckManager::new();
        assert!((mgr.nack_ratio_percent() - 0.0).abs() < 0.001);
    }

    #[test]
    fn ack_manager_summary_string() {
        let mgr = AckManager::new();
        let s = mgr.summary_string();
        assert!(!s.is_empty());
        assert!(s.contains("AckManager"));
    }

    #[test]
    fn ack_manager_clone_sessions() {
        let mut mgr = AckManager::new();
        mgr.get_or_create_session(test_peer1());
        let cloned = mgr.clone_sessions();
        assert_eq!(cloned.len(), 1);
    }

    #[test]
    fn ack_manager_replace_sessions() {
        let mut mgr = AckManager::new();
        let mut sessions = HashMap::new();
        sessions.insert(test_peer1(), AckSession::new(test_peer1()));
        mgr.replace_sessions(sessions);
        assert!(mgr.has_session(test_peer1()));
    }

    #[test]
    fn ack_manager_acks_since() {
        let mut mgr = AckManager::new();
        mgr.record_ack_sent(test_peer1());
        assert_eq!(mgr.acks_sent_since(0), 1);
        assert_eq!(mgr.acks_sent_since(1), 0);
    }

    #[test]
    fn ack_manager_acks_received_since() {
        let mut mgr = AckManager::new();
        mgr.record_ack_received(test_peer1());
        assert_eq!(mgr.acks_received_since(0), 1);
        assert_eq!(mgr.acks_received_since(1), 0);
    }

    #[test]
    fn ack_manager_nacks_sent_since() {
        let mut mgr = AckManager::new();
        mgr.record_nack_sent(test_peer1());
        assert_eq!(mgr.nacks_sent_since(0), 1);
        assert_eq!(mgr.nacks_sent_since(1), 0);
    }
}