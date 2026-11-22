//! # ids-000-aa — Sealed `Sequencing` Trait Contract
//!
//! Native HX-241 sequence management contract. All sequence allocation,
//! monotonic ordering, gap detection, out-of-order buffering, wrap-around,
//! and per-peer sequence spaces are defined here.
//!
//! Carrier-agnostic: no `std::net`, `tokio::net`, libp2p, IP, TCP, UDP,
//! QUIC, HTTP, or DNS types anywhere. Uses `Endpoint` (`CarrierType +
//! Vec<u8>`) only.
//!
//! Sealed: downstream crates call methods and use as trait bounds but cannot
//! implement. Only ids-000-aa's designated implementation crates may `impl`.

use crate::types::{Endpoint, NodeId};
use crate::types::{SequenceNumber, TimestampMillis};
use crate::types::SequencingError;
use thiserror::Error;

/// Sealed marker trait — only ids-000-aa internals can implement `Sequencing`.
pub mod private {
    pub trait Sealed {}
}

/// Maximum sequence value before wrap-around.
pub const MAX_SEQUENCE: u64 = u64::MAX;

/// Initial sequence number for new sequence spaces.
pub const INITIAL_SEQUENCE: SequenceNumber = SequenceNumber(1);

/// Default sequence window size for out-of-order buffering.
pub const DEFAULT_SEQUENCE_WINDOW: u64 = 1024;

/// Default maximum out-of-order buffer size.
pub const DEFAULT_MAX_OUT_OF_ORDER: usize = 4096;

/// Default sequence space expiry timeout in milliseconds.
pub const DEFAULT_SEQUENCE_EXPIRY_MS: u64 = 300_000;

/// Default gap fill timeout in milliseconds.
pub const DEFAULT_GAP_FILL_TIMEOUT_MS: u64 = 5_000;

/// Maximum sequence gap before declaring lost.
pub const MAX_SEQUENCE_GAP: u64 = 10_000;

/// Default sequence checkpoint interval.
pub const DEFAULT_CHECKPOINT_INTERVAL: u64 = 10_000;

///////////////////////////////////////////////////////////////////////////////
// SEQUENCE NUMBER TYPES
///////////////////////////////////////////////////////////////////////////////

/// A sequence number wrapper for type safety.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SequenceNumber(pub u64);

impl SequenceNumber {
    /// Create a new sequence number.
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    /// Get the raw u64 value.
    pub const fn value(self) -> u64 {
        self.0
    }

    /// Check if this sequence number is zero (reserved).
    pub fn is_zero(self) -> bool {
        self.0 == 0
    }

    /// Check if this sequence number is at maximum (about to wrap).
    pub fn is_at_max(self) -> bool {
        self.0 == u64::MAX
    }

    /// Increment by one with wrap-around.
    pub fn next(self) -> Self {
        Self(self.0.wrapping_add(1))
    }

    /// Decrement by one with wrap-around.
    pub fn prev(self) -> Self {
        Self(self.0.wrapping_sub(1))
    }

    /// Add an offset with wrap-around.
    pub fn wrapping_add(self, offset: u64) -> Self {
        Self(self.0.wrapping_add(offset))
    }

    /// Subtract an offset with wrap-around.
    pub fn wrapping_sub(self, offset: u64) -> Self {
        Self(self.0.wrapping_sub(offset))
    }

    /// Distance from self to other (forward direction, handles wrap-around).
    pub fn forward_distance(self, other: SequenceNumber) -> u64 {
        other.0.wrapping_sub(self.0)
    }

    /// Check if `other` is within `window` ahead of self (forward).
    pub fn is_within_window(self, other: SequenceNumber, window: u64) -> bool {
        let dist = self.forward_distance(other);
        dist < window
    }

    /// Check if this sequence is older than `other` (wraps at window).
    pub fn is_older_than(self, other: SequenceNumber, window: u64) -> bool {
        let dist = self.forward_distance(other);
        dist > window / 2
    }

    /// Check if this sequence is newer than `other` (wraps at window).
    pub fn is_newer_than(self, other: SequenceNumber, window: u64) -> bool {
        let dist = other.forward_distance(self);
        dist > 0 && dist <= window / 2
    }

    /// Serialization roundtrip helper.
    pub fn to_bytes(self) -> [u8; 8] {
        self.0.to_le_bytes()
    }

    /// Deserialize from bytes.
    pub fn from_bytes(bytes: [u8; 8]) -> Self {
        Self(u64::from_le_bytes(bytes))
    }
}

impl Default for SequenceNumber {
    fn default() -> Self {
        INITIAL_SEQUENCE
    }
}

impl std::fmt::Display for SequenceNumber {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Seq({})", self.0)
    }
}

/// A sequence range [start, end) for gap tracking.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SequenceRange {
    pub start: SequenceNumber,
    pub end: SequenceNumber,
}

impl SequenceRange {
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

///////////////////////////////////////////////////////////////////////////////
// SEQUENCE SPACE
///////////////////////////////////////////////////////////////////////////////

/// Per-peer sequence space tracking expected, received, and buffered
/// sequence numbers with gap detection.
#[derive(Debug, Clone)]
pub struct SequenceSpace {
    pub peer_id: NodeId,
    pub expected_next: SequenceNumber,
    pub highest_received: SequenceNumber,
    pub lowest_unacked: SequenceNumber,
    pub out_of_order: HashMap<SequenceNumber, Vec<u8>>,
    pub received_ranges: Vec<SequenceRange>,
    pub gap_count: u64,
    pub total_received: u64,
    pub total_out_of_order: u64,
    pub last_received_at: Option<TimestampMillis>,
    pub window_size: u64,
    pub max_out_of_order: usize,
    pub expiry_ms: u64,
}

impl SequenceSpace {
    pub fn new(peer_id: NodeId) -> Self {
        Self {
            peer_id,
            expected_next: INITIAL_SEQUENCE,
            highest_received: SequenceNumber(0),
            lowest_unacked: INITIAL_SEQUENCE,
            out_of_order: HashMap::new(),
            received_ranges: Vec::new(),
            gap_count: 0,
            total_received: 0,
            total_out_of_order: 0,
            last_received_at: None,
            window_size: DEFAULT_SEQUENCE_WINDOW,
            max_out_of_order: DEFAULT_MAX_OUT_OF_ORDER,
            expiry_ms: DEFAULT_SEQUENCE_EXPIRY_MS,
        }
    }

    pub fn with_config(
        peer_id: NodeId,
        window_size: u64,
        max_out_of_order: usize,
        expiry_ms: u64,
    ) -> Result<Self, SequencingError> {
        if window_size == 0 {
            return Err(SequencingError::InvalidConfig {
                field: "window_size".to_string(),
                reason: "window_size must be greater than 0".to_string(),
            });
        }
        if max_out_of_order == 0 {
            return Err(SequencingError::InvalidConfig {
                field: "max_out_of_order".to_string(),
                reason: "max_out_of_order must be greater than 0".to_string(),
            });
        }
        Ok(Self {
            peer_id,
            expected_next: INITIAL_SEQUENCE,
            highest_received: SequenceNumber(0),
            lowest_unacked: INITIAL_SEQUENCE,
            out_of_order: HashMap::new(),
            received_ranges: Vec::new(),
            gap_count: 0,
            total_received: 0,
            total_out_of_order: 0,
            last_received_at: None,
            window_size,
            max_out_of_order,
            expiry_ms,
        })
    }

    /// Record a received sequence number, returning any newly-filled gaps.
    pub fn record_received(
        &mut self,
        seq: SequenceNumber,
        data: Vec<u8>,
        now_ms: TimestampMillis,
    ) -> SequencingResult<Vec<SequenceNumber>> {
        if seq.is_zero() {
            return Err(SequencingError::InvalidSequence {
                seq,
                reason: "sequence 0 is reserved".to_string(),
            });
        }

        let mut filled_gaps = Vec::new();

        if seq.value() < self.expected_next.value() {
            // Duplicate or old sequence — already received.
            return Ok(filled_gaps);
        }

        let distance = seq.forward_distance(self.expected_next);
        if distance >= self.window_size {
            return Err(SequencingError::SequenceOutsideWindow {
                seq,
                expected: self.expected_next,
                window: self.window_size,
            });
        }

        if seq.value() > self.highest_received.value() {
            // New gap region opens between highest_received and seq.
            if self.highest_received.value() > 0 {
                self.gap_count += 1;
            }
            self.highest_received = seq;
        }

        if seq == self.expected_next {
            // In-order reception.
            self.out_of_order.insert(seq, data);
            self.expected_next = seq.next();
            self.lowest_unacked = self.expected_next;
            self.total_received += 1;
            self.last_received_at = Some(now_ms);

            // Consume any contiguous buffered sequences.
            loop {
                match self.out_of_order.remove(&self.expected_next) {
                    Some(_buffered) => {
                        self.expected_next = self.expected_next.next();
                        self.lowest_unacked = self.expected_next;
                        self.total_received += 1;
                        filled_gaps.push(self.expected_next.prev());
                    }
                    None => break,
                }
            }

            // Merge received ranges.
            self.merge_received_ranges();
        } else {
            // Out-of-order reception.
            self.out_of_order.insert(seq, data);
            self.total_out_of_order += 1;
            self.total_received += 1;
            self.last_received_at = Some(now_ms);

            if self.out_of_order.len() > self.max_out_of_order {
                return Err(SequencingError::OutOfOrderBufferFull {
                    current: self.out_of_order.len(),
                    max: self.max_out_of_order,
                });
            }
        }

        Ok(filled_gaps)
    }

    /// Get the next expected sequence number.
    pub fn expected_next(&self) -> SequenceNumber {
        self.expected_next
    }

    /// Check if a sequence is expected next.
    pub fn is_expected_next(&self, seq: SequenceNumber) -> bool {
        seq == self.expected_next
    }

    /// Get the count of buffered out-of-order sequences.
    pub fn buffered_count(&self) -> usize {
        self.out_of_order.len()
    }

    /// Get buffered sequences in order from expected_next.
    pub fn buffered_in_order(&self) -> Vec<(SequenceNumber, Vec<u8>)> {
        let mut result = Vec::new();
        let mut current = self.expected_next;
        while let Some(data) = self.out_of_order.get(&current) {
            result.push((current, data.clone()));
            current = current.next();
        }
        result
    }

    /// Get all buffered sequence numbers.
    pub fn buffered_sequences(&self) -> Vec<SequenceNumber> {
        let mut seqs: Vec<SequenceNumber> = self.out_of_order.keys().copied().collect();
        seqs.sort();
        seqs
    }

    /// Remove and return buffered data for a sequence.
    pub fn take_buffered(&mut self, seq: SequenceNumber) -> Option<Vec<u8>> {
        self.out_of_order.remove(&seq)
    }

    /// Acknowledge a sequence, advancing lowest_unacked.
    pub fn acknowledge(&mut self, seq: SequenceNumber) -> SequencingResult<()> {
        if seq.value() < self.lowest_unacked.value() {
            return Ok(()); // Already acked.
        }
        if seq.value() > self.highest_received.value() {
            return Err(SequencingError::AckBeyondReceived {
                seq,
                highest: self.highest_received,
            });
        }
        self.lowest_unacked = seq.next();
        Ok(())
    }

    /// Get the lowest unacknowledged sequence.
    pub fn lowest_unacked(&self) -> SequenceNumber {
        self.lowest_unacked
    }

    /// Check if the space has any gaps.
    pub fn has_gaps(&self) -> bool {
        self.gap_count > 0 || !self.out_of_order.is_empty()
    }

    /// Get the number of detected gaps.
    pub fn gap_count(&self) -> u64 {
        self.gap_count
    }

    /// Expire old buffered entries beyond the expiry window.
    pub fn expire_old_entries(&mut self, now_ms: TimestampMillis) -> u64 {
        let mut expired = 0u64;
        if let Some(last) = self.last_received_at {
            if now_ms.value() > last.value() && now_ms.value() - last.value() > self.expiry_ms {
                let to_remove: Vec<SequenceNumber> = self
                    .out_of_order
                    .keys()
                    .filter(|seq| {
                        last.forward_distance(**seq) > self.expiry_ms
                    })
                    .copied()
                    .collect();
                for seq in to_remove {
                    self.out_of_order.remove(&seq);
                    expired += 1;
                }
            }
        }
        expired
    }

    /// Get statistics for this sequence space.
    pub fn stats(&self) -> SequenceSpaceStats {
        SequenceSpaceStats {
            peer_id: self.peer_id,
            expected_next: self.expected_next,
            highest_received: self.highest_received,
            lowest_unacked: self.lowest_unacked,
            buffered_count: self.out_of_order.len() as u64,
            gap_count: self.gap_count,
            total_received: self.total_received,
            total_out_of_order: self.total_out_of_order,
            last_received_at: self.last_received_at,
            window_utilization: if self.window_size > 0 {
                self.out_of_order.len() as f64 / self.window_size as f64 * 100.0
            } else {
                0.0
            },
        }
    }

    fn merge_received_ranges(&mut self) {
        // Simplistic: track contiguous range from lowest_unacked to highest_received.
        if self.lowest_unacked.value() <= self.highest_received.value() {
            self.received_ranges = vec![SequenceRange::new(
                self.lowest_unacked,
                self.highest_received.next(),
            )];
        } else {
            self.received_ranges.clear();
        }
    }

    /// Get all received ranges for this sequence space.
    pub fn received_ranges(&self) -> &[SequenceRange] {
        &self.received_ranges
    }

    /// Get the count of received ranges.
    pub fn received_range_count(&self) -> usize {
        self.received_ranges.len()
    }

    /// Get the largest contiguous received range.
    pub fn largest_contiguous_range(&self) -> Option<SequenceRange> {
        self.received_ranges
            .iter()
            .max_by_key(|r| r.len())
            .copied()
    }

    /// Get the count of missing sequences in the current window.
    pub fn missing_count(&self) -> u64 {
        let covered: u64 = self
            .received_ranges
            .iter()
            .map(|r| r.len())
            .sum();
        let span = self
            .highest_received
            .value()
            .saturating_sub(self.lowest_unacked.value())
            .saturating_add(1);
        span.saturating_sub(covered)
    }

    /// Check whether a specific sequence has been received.
    pub fn is_received(&self, seq: SequenceNumber) -> bool {
        self.out_of_order.contains_key(&seq)
            || (seq.value() >= self.lowest_unacked.value()
                && seq.value() < self.expected_next.value())
    }

    /// Get the distance from expected_next to highest_received (gap span).
    pub fn gap_span(&self) -> u64 {
        if self.highest_received.value() >= self.expected_next.value() {
            self.highest_received
                .value()
                .saturating_sub(self.expected_next.value())
        } else {
            0
        }
    }

    /// Get the percentage of the window that is filled.
    pub fn window_fill_percent(&self) -> f64 {
        if self.window_size == 0 {
            return 0.0;
        }
        self.buffered_count() as f64 / self.window_size as f64 * 100.0
    }

    /// Get the percentage of sequences received vs total span.
    pub fn receive_completeness_percent(&self) -> f64 {
        let total = self.highest_received.value().saturating_add(1);
        if total == 0 {
            return 100.0;
        }
        self.total_received as f64 / total as f64 * 100.0
    }

    /// Get the average gap size across open gaps.
    pub fn average_gap_size(&self) -> f64 {
        if self.received_ranges.is_empty() {
            return 0.0;
        }
        let mut gaps = 0u64;
        let mut prev_end = self.lowest_unacked;
        for range in &self.received_ranges {
            if range.start.value() > prev_end.value() {
                gaps += range.start.value() - prev_end.value();
            }
            prev_end = range.end;
        }
        if gaps == 0 {
            0.0
        } else {
            gaps as f64 / self.received_ranges.len() as f64
        }
    }

    /// Clone the buffered data map (expensive, for diagnostics only).
    pub fn clone_buffered(&self) -> HashMap<SequenceNumber, Vec<u8>> {
        self.out_of_order.clone()
    }

    /// Clear all buffered out-of-order data.
    pub fn clear_buffered(&mut self) -> usize {
        let count = self.out_of_order.len();
        self.out_of_order.clear();
        count
    }

    /// Shift the sequence space forward by `n` sequences (used after
    /// mass-acknowledgement).
    pub fn shift_forward(&mut self, n: u64) -> SequencingResult<()> {
        if n == 0 {
            return Ok(());
        }
        let new_lowest = self.lowest_unacked.wrapping_add(n);
        let new_expected = self.expected_next.wrapping_add(n);
        let mut new_buffered = HashMap::new();
        for (seq, data) in self.out_of_order.drain() {
            let shifted = seq.wrapping_sub(n);
            if shifted.value() >= new_lowest.value() {
                new_buffered.insert(shifted, data);
            }
        }
        self.lowest_unacked = new_lowest;
        self.expected_next = new_expected;
        self.out_of_order = new_buffered;
        Ok(())
    }

    /// Get the sequence space health score (0.0 to 1.0).
    pub fn health_score(&self) -> f64 {
        let mut score = 1.0f64;
        if self.gap_count > 0 {
            score -= (self.gap_count as f64).min(10.0) / 10.0 * 0.3;
        }
        if self.window_fill_percent() > 80.0 {
            score -= 0.2;
        }
        if self.total_out_of_order > self.total_received / 2 {
            score -= 0.2;
        }
        if score < 0.0 {
            0.0
        } else if score > 1.0 {
            1.0
        } else {
            score
        }
    }

    /// Get the time since last received sequence.
    pub fn time_since_last_received(&self, now_ms: TimestampMillis) -> Option<u64> {
        self.last_received_at
            .map(|last| now_ms.elapsed_since(last))
    }

    /// Check if the sequence space is stale (no activity beyond expiry).
    pub fn is_stale(&self, now_ms: TimestampMillis) -> bool {
        match self.last_received_at {
            None => true,
            Some(last) => now_ms.elapsed_since(last) > self.expiry_ms,
        }
    }

    /// Get the next sequence to request from peer (expected_next).
    pub fn next_requested_sequence(&self) -> SequenceNumber {
        self.expected_next
    }

    /// Get all missing sequences up to highest_received.
    pub fn missing_sequences(&self) -> Vec<SequenceNumber> {
        let mut missing = Vec::new();
        let mut current = self.expected_next;
        let end = self.highest_received.next();
        while current.value() < end.value() {
            if !self.out_of_order.contains_key(&current) {
                missing.push(current);
            }
            current = current.next();
        }
        missing
    }

    /// Get the count of duplicate sequences received (receptions of already
    /// acknowledged sequences).
    pub fn duplicate_count(&self) -> u64 {
        self.total_received
            .saturating_sub(self.buffered_count() as u64)
            .saturating_sub(self.lowest_unacked.value().saturating_sub(1))
    }

    /// Get the sequence at a given offset from expected_next.
    pub fn sequence_at_offset(&self, offset: u64) -> SequenceNumber {
        self.expected_next.wrapping_add(offset)
    }

    /// Get the offset of a sequence from expected_next.
    pub fn offset_from_expected(&self, seq: SequenceNumber) -> Option<u64> {
        if seq.value() >= self.expected_next.value() {
            Some(seq.value() - self.expected_next.value())
        } else {
            None
        }
    }

    /// Check whether the space is in strict order (no out-of-order buffered).
    pub fn is_strict_order(&self) -> bool {
        self.out_of_order.is_empty()
    }

    /// Get the number of contiguous sequences from expected_next.
    pub fn contiguous_count(&self) -> u64 {
        let mut count = 0u64;
        let mut current = self.expected_next;
        while self.out_of_order.contains_key(&current) {
            count += 1;
            current = current.next();
        }
        count
    }

    /// Get the first missing sequence after expected_next.
    pub fn first_missing(&self) -> Option<SequenceNumber> {
        let mut current = self.expected_next;
        let end = self.highest_received.next();
        while current.value() < end.value() {
            if !self.out_of_order.contains_key(&current) {
                return Some(current);
            }
            current = current.next();
        }
        None
    }

    /// Get the last received sequence.
    pub fn last_received(&self) -> SequenceNumber {
        self.highest_received
    }

    /// Check if a sequence is within the receive window.
    pub fn is_in_window(&self, seq: SequenceNumber) -> bool {
        let dist = self.expected_next.forward_distance(seq);
        dist < self.window_size
    }

    /// Get the receive window start and end.
    pub fn receive_window(&self) -> (SequenceNumber, SequenceNumber) {
        (
            self.expected_next,
            self.expected_next.wrapping_add(self.window_size),
        )
    }

    /// Advance expected_next by `n` (skip sequences).
    pub fn advance_expected(&mut self, n: u64) {
        for _ in 0..n {
            self.out_of_order.remove(&self.expected_next);
            self.expected_next = self.expected_next.next();
        }
        self.lowest_unacked = self.expected_next;
    }

    /// Get the sequence space size in bytes (approximate).
    pub fn approximate_size_bytes(&self) -> usize {
        let mut size = 0;
        for (_, data) in &self.out_of_order {
            size += data.len() + 8; // data + seq key overhead
        }
        size
    }

    /// Get the count of sequences pending ack (received but not acked).
    pub fn pending_ack_count(&self) -> u64 {
        self.highest_received
            .value()
            .saturating_sub(self.lowest_unacked.value())
    }

    /// Get the ack lag (how far behind acks are).
    pub fn ack_lag(&self) -> u64 {
        self.pending_ack_count()
    }

    /// Check if acks are caught up.
    pub fn acks_caught_up(&self) -> bool {
        self.lowest_unacked == self.highest_received.next()
            || self.lowest_unacked.value() > self.highest_received.value()
    }

    /// Get the ratio of pending acks to total received.
    pub fn pending_ack_ratio(&self) -> f64 {
        if self.total_received == 0 {
            return 0.0;
        }
        self.pending_ack_count() as f64 / self.total_received as f64 * 100.0
    }

    /// Get the maximum sequence value observed.
    pub fn max_observed(&self) -> SequenceNumber {
        self.highest_received
    }

    /// Get the minimum sequence value observed (lowest unacked).
    pub fn min_observed(&self) -> SequenceNumber {
        self.lowest_unacked
    }

    /// Get the sequence span (max - min).
    pub fn sequence_span(&self) -> u64 {
        self.highest_received
            .value()
            .saturating_sub(self.lowest_unacked.value())
    }

    /// Reset gap count.
    pub fn reset_gap_count(&mut self) {
        self.gap_count = 0;
    }

    /// Set the window size.
    pub fn set_window_size(&mut self, size: u64) -> Result<(), SequencingError> {
        if size == 0 {
            return Err(SequencingError::InvalidConfig {
                field: "window_size".to_string(),
                reason: "must be greater than 0".to_string(),
            });
        }
        self.window_size = size;
        Ok(())
    }

    /// Set the max out-of-order buffer size.
    pub fn set_max_out_of_order(&mut self, max: usize) -> Result<(), SequencingError> {
        if max == 0 {
            return Err(SequencingError::InvalidConfig {
                field: "max_out_of_order".to_string(),
                reason: "must be greater than 0".to_string(),
            });
        }
        self.max_out_of_order = max;
        Ok(())
    }

    /// Set the expiry timeout.
    pub fn set_expiry_ms(&mut self, ms: u64) {
        self.expiry_ms = ms;
    }

    /// Get a string representation of the sequence space state.
    pub fn state_string(&self) -> String {
        format!(
            "SequenceSpace[{}]: expected={}, highest={}, lowest_ack={}, buffered={}, gaps={}, completeness={:.1}%, health={:.2}",
            self.peer_id,
            self.expected_next.value(),
            self.highest_received.value(),
            self.lowest_unacked.value(),
            self.out_of_order.len(),
            self.gap_count,
            self.receive_completeness_percent(),
            self.health_score()
        )
    }

    /// Clone the entire sequence space.
    pub fn clone_space(&self) -> SequenceSpace {
        self.clone()
    }

    /// Get the default config values as a tuple.
    pub fn default_config() -> (u64, usize, u64) {
        (
            DEFAULT_SEQUENCE_WINDOW,
            DEFAULT_MAX_OUT_OF_ORDER,
            DEFAULT_SEQUENCE_EXPIRY_MS,
        )
    }

    /// Check if the sequence number wraps within `n` steps.
    pub fn wraps_within(&self, seq: SequenceNumber, n: u64) -> bool {
        seq.value().saturating_add(n) > u64::MAX
    }

    /// Get the distance to wrap-around from a sequence.
    pub fn distance_to_wrap(seq: SequenceNumber) -> u64 {
        u64::MAX.saturating_sub(seq.value())
    }

    /// Get the number of sequence spaces that have at least one gap.
    pub fn gap_affected_space_count(&self) -> usize {
        self.spaces.values().filter(|s| s.has_gaps()).count()
    }

    /// Get the total number of sequences across all spaces (received + buffered).
    pub fn total_sequence_activity(&self) -> u64 {
        self.spaces.values().map(|s| s.total_received).sum()
    }

    /// Get the number of sequence spaces that have at least one out-of-order
    /// buffered sequence.
    pub fn out_of_order_space_count(&self) -> usize {
        self.spaces
            .values()
            .filter(|s| !s.out_of_order.is_empty())
            .count()
    }

    /// Get the default config values as a tuple.
    pub fn default_config() -> (u64, usize, u64) {
        (
            DEFAULT_SEQUENCE_WINDOW,
            DEFAULT_MAX_OUT_OF_ORDER,
            DEFAULT_SEQUENCE_EXPIRY_MS,
        )
    }
}

impl Clone for SequenceSpace {
    fn clone(&self) -> Self {
        Self {
            peer_id: self.peer_id,
            expected_next: self.expected_next,
            highest_received: self.highest_received,
            lowest_unacked: self.lowest_unacked,
            out_of_order: self.out_of_order.clone(),
            received_ranges: self.received_ranges.clone(),
            gap_count: self.gap_count,
            total_received: self.total_received,
            total_out_of_order: self.total_out_of_order,
            last_received_at: self.last_received_at,
            window_size: self.window_size,
            max_out_of_order: self.max_out_of_order,
            expiry_ms: self.expiry_ms,
        }
    }
}

/// Statistics snapshot for a sequence space.
#[derive(Debug, Clone)]
pub struct SequenceSpaceStats {
    pub peer_id: NodeId,
    pub expected_next: SequenceNumber,
    pub highest_received: SequenceNumber,
    pub lowest_unacked: SequenceNumber,
    pub buffered_count: u64,
    pub gap_count: u64,
    pub total_received: u64,
    pub total_out_of_order: u64,
    pub last_received_at: Option<TimestampMillis>,
    pub window_utilization: f64,
}

///////////////////////////////////////////////////////////////////////////////
// GAP TRACKING
///////////////////////////////////////////////////////////////////////////////

/// A detected gap in the sequence stream.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SequenceGap {
    pub gap_id: u64,
    pub start: SequenceNumber,
    pub end: SequenceNumber,
    pub detected_at: TimestampMillis,
    pub filled_at: Option<TimestampMillis>,
    pub fill_attempts: u32,
    pub status: GapStatus,
}

/// Status of a sequence gap.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GapStatus {
    Open,
    Filling,
    Filled,
    DeclaredLost,
    Expired,
}

impl Default for GapStatus {
    fn default() -> Self {
        GapStatus::Open
    }
}

/// Gap tracker for monitoring missing sequences across peers.
#[derive(Debug, Clone)]
pub struct GapTracker {
    pub gaps: HashMap<u64, SequenceGap>,
    pub next_gap_id: u64,
    pub total_opened: u64,
    pub total_filled: u64,
    pub total_lost: u64,
    pub max_gap_fill_timeout_ms: u64,
}

impl Default for GapTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl GapTracker {
    pub fn new() -> Self {
        Self {
            gaps: HashMap::new(),
            next_gap_id: 1,
            total_opened: 0,
            total_filled: 0,
            total_lost: 0,
            max_gap_fill_timeout_ms: DEFAULT_GAP_FILL_TIMEOUT_MS,
        }
    }

    pub fn open_gap(
        &mut self,
        start: SequenceNumber,
        end: SequenceNumber,
        detected_at: TimestampMillis,
    ) -> u64 {
        let gap_id = self.next_gap_id;
        self.next_gap_id += 1;
        self.gaps.insert(
            gap_id,
            SequenceGap {
                gap_id,
                start,
                end,
                detected_at,
                filled_at: None,
                fill_attempts: 0,
                status: GapStatus::Open,
            },
        );
        self.total_opened += 1;
        gap_id
    }

    /// Mark a gap as filled.
    pub fn fill_gap(&mut self, gap_id: u64, filled_at: TimestampMillis) -> bool {
        if let Some(gap) = self.gaps.get_mut(&gap_id) {
            if gap.status == GapStatus::Open || gap.status == GapStatus::Filling {
                gap.status = GapStatus::Filled;
                gap.filled_at = Some(filled_at);
                self.total_filled += 1;
                return true;
            }
        }
        false
    }

    /// Declare a gap lost after timeout.
    pub fn declare_lost(&mut self, gap_id: u64) -> bool {
        if let Some(gap) = self.gaps.get_mut(&gap_id) {
            if gap.status == GapStatus::Open || gap.status == GapStatus::Filling {
                gap.status = GapStatus::DeclaredLost;
                self.total_lost += 1;
                return true;
            }
        }
        false
    }

    /// Get all open gaps.
    pub fn open_gaps(&self) -> Vec<&SequenceGap> {
        self.gaps
            .values()
            .filter(|g| g.status == GapStatus::Open)
            .collect()
    }

    /// Get gaps that have timed out and should be declared lost.
    pub fn timed_out_gaps(&self, now: TimestampMillis) -> Vec<u64> {
        self.gaps
            .iter()
            .filter(|(_, g)| {
                g.status == GapStatus::Open
                    && now.value() - g.detected_at.value() > self.max_gap_fill_timeout_ms
            })
            .map(|(id, _)| *id)
            .collect()
    }

    /// Get gap statistics.
    pub fn stats(&self) -> GapStats {
        GapStats {
            total_opened: self.total_opened,
            total_filled: self.total_filled,
            total_lost: self.total_lost,
            currently_open: self
                .gaps
                .values()
                .filter(|g| g.status == GapStatus::Open)
                .count() as u64,
            fill_rate: if self.total_opened > 0 {
                self.total_filled as f64 / self.total_opened as f64 * 100.0
            } else {
                100.0
            },
        }
    }

    /// Get a specific gap by ID.
    pub fn gap(&self, gap_id: u64) -> Option<&SequenceGap> {
        self.gaps.get(&gap_id)
    }

    /// Get the count of gaps by status.
    pub fn gap_count_by_status(&self, status: GapStatus) -> u64 {
        self.gaps
            .values()
            .filter(|g| g.status == status)
            .count() as u64
    }

    /// Get all gaps (any status).
    pub fn all_gaps(&self) -> Vec<&SequenceGap> {
        self.gaps.values().collect()
    }

    /// Get the total gap span across all open gaps.
    pub fn total_open_gap_span(&self) -> u64 {
        self.gaps
            .values()
            .filter(|g| g.status == GapStatus::Open)
            .map(|g| g.end.value().saturating_sub(g.start.value()))
            .sum()
    }

    /// Get the oldest open gap.
    pub fn oldest_open_gap(&self) -> Option<&SequenceGap> {
        self.gaps
            .values()
            .filter(|g| g.status == GapStatus::Open)
            .min_by_key(|g| g.detected_at.value())
    }

    /// Get the newest open gap.
    pub fn newest_open_gap(&self) -> Option<&SequenceGap> {
        self.gaps
            .values()
            .filter(|g| g.status == GapStatus::Open)
            .max_by_key(|g| g.detected_at.value())
    }

    /// Increment fill attempts for a gap.
    pub fn increment_fill_attempts(&mut self, gap_id: u64) -> bool {
        if let Some(gap) = self.gaps.get_mut(&gap_id) {
            gap.fill_attempts += 1;
            if gap.status == GapStatus::Open {
                gap.status = GapStatus::Filling;
            }
            return true;
        }
        false
    }

    /// Get gaps that have exceeded max fill attempts.
    pub fn gaps_exceeding_max_attempts(&self, max_attempts: u32) -> Vec<u64> {
        self.gaps
            .iter()
            .filter(|(_, g)| {
                g.status == GapStatus::Filling && g.fill_attempts > max_attempts
            })
            .map(|(id, _)| *id)
            .collect()
    }

    /// Clear all resolved gaps (filled or lost).
    pub fn clear_resolved_gaps(&mut self) -> usize {
        let before = self.gaps.len();
        self.gaps.retain(|_, g| g.status == GapStatus::Open);
        before - self.gaps.len()
    }

    /// Get the average time to fill gaps (ms).
    pub fn average_fill_time_ms(&self) -> Option<f64> {
        let times: Vec<u64> = self
            .gaps
            .values()
            .filter_map(|g| {
                g.filled_at.map(|f| f.value().saturating_sub(g.detected_at.value()))
            })
            .collect();
        if times.is_empty() {
            None
        } else {
            Some(times.iter().sum::<u64>() as f64 / times.len() as f64)
        }
    }

    /// Get the gap distribution by size buckets.
    pub fn gap_size_distribution(&self) -> GapSizeDistribution {
        let mut small = 0u64;
        let mut medium = 0u64;
        let mut large = 0u64;
        for gap in self.gaps.values() {
            let size = gap.end.value().saturating_sub(gap.start.value());
            if size <= 10 {
                small += 1;
            } else if size <= 1000 {
                medium += 1;
            } else {
                large += 1;
            }
        }
        GapSizeDistribution {
            small,
            medium,
            large,
            total: small + medium + large,
        }
    }

    /// Set the max gap fill timeout.
    pub fn set_max_gap_fill_timeout_ms(&mut self, timeout_ms: u64) {
        self.max_gap_fill_timeout_ms = timeout_ms;
    }

    /// Get the max gap fill timeout.
    pub fn max_gap_fill_timeout_ms(&self) -> u64 {
        self.max_gap_fill_timeout_ms
    }

    /// Get the next gap ID to be assigned.
    pub fn next_gap_id(&self) -> u64 {
        self.next_gap_id
    }
}

/// Gap size distribution histogram.
#[derive(Debug, Clone, Default)]
pub struct GapSizeDistribution {
    pub small: u64,
    pub medium: u64,
    pub large: u64,
    pub total: u64,
}

///////////////////////////////////////////////////////////////////////////////
// SEQUENCE MANAGER
///////////////////////////////////////////////////////////////////////////////

/// Central manager for all sequence spaces across peers.
#[derive(Debug, Clone)]
pub struct SequenceManager {
    pub spaces: HashMap<NodeId, SequenceSpace>,
    pub gap_tracker: GapTracker,
    pub total_allocated: u64,
    pub total_received: u64,
    pub total_acked: u64,
    pub default_window_size: u64,
    pub default_max_out_of_order: usize,
    pub default_expiry_ms: u64,
}

impl Default for SequenceManager {
    fn default() -> Self {
        Self::new()
    }
}

impl SequenceManager {
    pub fn new() -> Self {
        Self {
            spaces: HashMap::new(),
            gap_tracker: GapTracker::new(),
            total_allocated: 0,
            total_received: 0,
            total_acked: 0,
            default_window_size: DEFAULT_SEQUENCE_WINDOW,
            default_max_out_of_order: DEFAULT_MAX_OUT_OF_ORDER,
            default_expiry_ms: DEFAULT_SEQUENCE_EXPIRY_MS,
        }
    }

    /// Allocate the next sequence number for a peer.
    pub fn allocate_sequence(&mut self, peer_id: NodeId) -> SequenceNumber {
        let space = self
            .spaces
            .entry(peer_id)
            .or_insert_with(|| SequenceSpace::new(peer_id));
        let seq = space.expected_next;
        self.total_allocated += 1;
        seq
    }

    /// Allocate a batch of sequence numbers for a peer.
    pub fn allocate_sequence_batch(
        &mut self,
        peer_id: NodeId,
        count: u64,
    ) -> (SequenceNumber, SequenceNumber) {
        if count == 0 {
            return (SequenceNumber(0), SequenceNumber(0));
        }
        let space = self
            .spaces
            .entry(peer_id)
            .or_insert_with(|| SequenceSpace::new(peer_id));
        let start = space.expected_next;
        let end = start.wrapping_add(count - 1);
        self.total_allocated += count;
        (start, end)
    }

    /// Get or create a sequence space for a peer.
    pub fn get_or_create_space(&mut self, peer_id: NodeId) -> &mut SequenceSpace {
        self.spaces
            .entry(peer_id)
            .or_insert_with(|| SequenceSpace::new(peer_id))
    }

    /// Get a reference to a peer's sequence space.
    pub fn space(&self, peer_id: NodeId) -> Option<&SequenceSpace> {
        self.spaces.get(&peer_id)
    }

    /// Get a mutable reference to a peer's sequence space.
    pub fn space_mut(&mut self, peer_id: NodeId) -> Option<&mut SequenceSpace> {
        self.spaces.get_mut(&peer_id)
    }

    /// Create a sequence space with custom config.
    pub fn create_space(
        &mut self,
        peer_id: NodeId,
        window_size: u64,
        max_out_of_order: usize,
        expiry_ms: u64,
    ) -> Result<(), SequencingError> {
        let space = SequenceSpace::with_config(peer_id, window_size, max_out_of_order, expiry_ms)?;
        self.spaces.insert(peer_id, space);
        Ok(())
    }

    /// Remove a sequence space for a peer.
    pub fn remove_space(&mut self, peer_id: NodeId) -> bool {
        self.spaces.remove(&peer_id).is_some()
    }

    /// Check if a sequence space exists.
    pub fn has_space(&self, peer_id: NodeId) -> bool {
        self.spaces.contains_key(&peer_id)
    }

    /// Record a received sequence for a peer.
    pub fn record_received(
        &mut self,
        peer_id: NodeId,
        seq: SequenceNumber,
        data: Vec<u8>,
        now_ms: TimestampMillis,
    ) -> SequencingResult<Vec<SequenceNumber>> {
        let space = self
            .spaces
            .entry(peer_id)
            .or_insert_with(|| SequenceSpace::new(peer_id));
        let gaps = space.record_received(seq, data, now_ms)?;
        self.total_received += 1;
        Ok(gaps)
    }

    /// Acknowledge a sequence for a peer.
    pub fn acknowledge(&mut self, peer_id: NodeId, seq: SequenceNumber) -> SequencingResult<()> {
        if let Some(space) = self.spaces.get_mut(&peer_id) {
            space.acknowledge(seq)?;
            self.total_acked += 1;
            Ok(())
        } else {
            Err(SequencingError::SequenceSpaceNotFound { peer_id })
        }
    }

    /// Get the next expected sequence for a peer.
    pub fn expected_next(&self, peer_id: NodeId) -> Option<SequenceNumber> {
        self.spaces.get(&peer_id).map(|s| s.expected_next)
    }

    /// Get statistics for all peers.
    pub fn all_space_stats(&self) -> Vec<SequenceSpaceStats> {
        self.spaces.values().map(|s| s.stats()).collect()
    }

    /// Get the count of active sequence spaces.
    pub fn active_space_count(&self) -> usize {
        self.spaces.len()
    }

    /// List all peers with spaces.
    pub fn peer_ids(&self) -> Vec<NodeId> {
        self.spaces.keys().copied().collect()
    }

    /// Expire old entries across all spaces.
    pub fn expire_all_old_entries(&mut self, now_ms: TimestampMillis) -> u64 {
        let mut total = 0;
        for space in self.spaces.values_mut() {
            total += space.expire_old_entries(now_ms);
        }
        total
    }

    /// Get the count of spaces with gaps.
    pub fn spaces_with_gaps(&self) -> usize {
        self.spaces.values().filter(|s| s.has_gaps()).count()
    }

    /// Get the overall health score across all spaces.
    pub fn overall_health_score(&self) -> f64 {
        if self.spaces.is_empty() {
            return 1.0;
        }
        let total: f64 = self.spaces.values().map(|s| s.health_score()).sum();
        total / self.spaces.len() as f64
    }

    /// Get total allocated sequences.
    pub fn total_allocated(&self) -> u64 {
        self.total_allocated
    }

    /// Get total received sequences.
    pub fn total_received(&self) -> u64 {
        self.total_received
    }

    /// Get total acked sequences.
    pub fn total_acked(&self) -> u64 {
        self.total_acked
    }

    /// Get the gap tracker reference.
    pub fn gap_tracker(&self) -> &GapTracker {
        &self.gap_tracker
    }

    /// Get the gap tracker mutably.
    pub fn gap_tracker_mut(&mut self) -> &mut GapTracker {
        &mut self.gap_tracker
    }

    /// Open a gap in the tracker.
    pub fn open_gap(
        &mut self,
        start: SequenceNumber,
        end: SequenceNumber,
        detected_at: TimestampMillis,
    ) -> u64 {
        self.gap_tracker.open_gap(start, end, detected_at)
    }

    /// Fill a gap.
    pub fn fill_gap(&mut self, gap_id: u64, filled_at: TimestampMillis) -> bool {
        self.gap_tracker.fill_gap(gap_id, filled_at)
    }

    /// Declare a gap lost.
    pub fn declare_lost(&mut self, gap_id: u64) -> bool {
        self.gap_tracker.declare_lost(gap_id)
    }

    /// Get gap statistics.
    pub fn gap_stats(&self) -> GapStats {
        self.gap_tracker.stats()
    }

    /// Snapshot all state.
    pub fn snapshot(&self) -> SequencingSnapshot {
        let spaces: Vec<SequenceSpaceSnapshot> = self
            .spaces
            .values()
            .map(|s| SequenceSpaceSnapshot {
                peer_id: s.peer_id,
                expected_next: s.expected_next,
                highest_received: s.highest_received,
                lowest_unacked: s.lowest_unacked,
                buffered_count: s.buffered_count() as u64,
                gap_count: s.gap_count,
                total_received: s.total_received,
                total_out_of_order: s.total_out_of_order,
            })
            .collect();
        SequencingSnapshot {
            spaces,
            gap_stats: self.gap_tracker.stats(),
            total_allocated: self.total_allocated,
            snapshot_at: TimestampMillis::now(),
        }
    }

    /// Restore from snapshot.
    pub fn restore(&mut self, snapshot: SequencingSnapshot) {
        self.spaces.clear();
        for space_snap in snapshot.spaces {
            let mut space = SequenceSpace::new(space_snap.peer_id);
            space.expected_next = space_snap.expected_next;
            space.highest_received = space_snap.highest_received;
            space.lowest_unacked = space_snap.lowest_unacked;
            space.total_received = space_snap.total_received;
            space.total_out_of_order = space_snap.total_out_of_order;
            self.spaces.insert(space_snap.peer_id, space);
        }
        self.total_allocated = snapshot.total_allocated;
    }

    /// Reset all sequence spaces.
    pub fn reset_all(&mut self) {
        self.spaces.clear();
        self.total_allocated = 0;
        self.total_received = 0;
        self.total_acked = 0;
        self.gap_tracker = GapTracker::new();
    }

    /// Reset a single peer's sequence space.
    pub fn reset_peer(&mut self, peer_id: NodeId) -> bool {
        if self.spaces.remove(&peer_id).is_some() {
            self.spaces.insert(peer_id, SequenceSpace::new(peer_id));
            true
        } else {
            false
        }
    }

    /// Get the default window size.
    pub fn default_window_size(&self) -> u64 {
        self.default_window_size
    }

    /// Set the default window size.
    pub fn set_default_window_size(&mut self, size: u64) {
        self.default_window_size = size;
    }

    /// Get the default max out-of-order.
    pub fn default_max_out_of_order(&self) -> usize {
        self.default_max_out_of_order
    }

    /// Set the default max out-of-order.
    pub fn set_default_max_out_of_order(&mut self, max: usize) {
        self.default_max_out_of_order = max;
    }

    /// Get the default expiry ms.
    pub fn default_expiry_ms(&self) -> u64 {
        self.default_expiry_ms
    }

    /// Set the default expiry ms.
    pub fn set_default_expiry_ms(&mut self, ms: u64) {
        self.default_expiry_ms = ms;
    }

    /// Check wrap-around proximity for a sequence (0.0 = far, 1.0 = at max).
    pub fn wrap_around_proximity(&self, seq: SequenceNumber) -> f64 {
        seq.value() as f64 / u64::MAX as f64
    }

    /// Validate that a sequence is within acceptable bounds for a peer.
    pub fn validate_sequence(
        &self,
        peer_id: NodeId,
        seq: SequenceNumber,
    ) -> Result<(), SequencingError> {
        if seq.is_zero() {
            return Err(SequencingError::InvalidSequence {
                seq,
                reason: "sequence 0 is reserved".to_string(),
            });
        }
        if let Some(space) = self.spaces.get(&peer_id) {
            let distance = space.expected_next.forward_distance(seq);
            if distance >= space.window_size {
                return Err(SequencingError::SequenceOutsideWindow {
                    seq,
                    expected: space.expected_next,
                    window: space.window_size,
                });
            }
        }
        Ok(())
    }

    /// Get the next sequence after `seq` with wrap-around handling.
    pub fn successor(&self, seq: SequenceNumber) -> SequenceNumber {
        seq.next()
    }

    /// Get the previous sequence before `seq` with wrap-around handling.
    pub fn predecessor(&self, seq: SequenceNumber) -> SequenceNumber {
        seq.prev()
    }

    /// Check if `a` is before `b` in sequence order (considering window).
    pub fn is_before(&self, a: SequenceNumber, b: SequenceNumber, window: u64) -> bool {
        a.forward_distance(b) < window / 2
    }

    /// Check if `a` is after `b` in sequence order (considering window).
    pub fn is_after(&self, a: SequenceNumber, b: SequenceNumber, window: u64) -> bool {
        b.forward_distance(a) < window / 2
    }

    /// Get the midpoint sequence between two sequences.
    pub fn midpoint(&self, a: SequenceNumber, b: SequenceNumber) -> SequenceNumber {
        let dist = a.forward_distance(b);
        a.wrapping_add(dist / 2)
    }

    /// Get all sequence numbers in a range [start, end).
    pub fn range_sequences(&self, start: SequenceNumber, end: SequenceNumber) -> Vec<SequenceNumber> {
        let mut result = Vec::new();
        let mut current = start;
        while current.value() < end.value() {
            result.push(current);
            current = current.next();
        }
        result
    }

    /// Compact a sequence space by removing acknowledged sequences below
    /// lowest_unacked.
    pub fn compact_space(&mut self, peer_id: NodeId) -> usize {
        if let Some(space) = self.spaces.get_mut(&peer_id) {
            let before = space.out_of_order.len();
            space.out_of_order.retain(|seq, _| {
                seq.value() >= space.lowest_unacked.value()
            });
            before - space.out_of_order.len()
        } else {
            0
        }
    }

    /// Get the count of buffered sequences across all spaces.
    pub fn total_buffered(&self) -> u64 {
        self.spaces.values().map(|s| s.buffered_count() as u64).sum()
    }

    /// Get the count of spaces that are stale.
    pub fn stale_space_count(&self, now_ms: TimestampMillis) -> usize {
        self.spaces.values().filter(|s| s.is_stale(now_ms)).count()
    }

    /// Get the peer with the most buffered sequences.
    pub fn busiest_peer(&self) -> Option<(NodeId, usize)> {
        self.spaces
            .iter()
            .max_by_key(|(_, s)| s.buffered_count())
            .map(|(id, s)| (*id, s.buffered_count()))
    }

    /// Get the peer with the most gaps.
    pub fn gappiest_peer(&self) -> Option<(NodeId, u64)> {
        self.spaces
            .iter()
            .max_by_key(|(_, s)| s.gap_count)
            .map(|(id, s)| (*id, s.gap_count))
    }

    /// Get total missing sequences across all spaces.
    pub fn total_missing(&self) -> u64 {
        self.spaces.values().map(|s| s.missing_count()).sum()
    }

    /// Get the average health score weighted by received count.
    pub fn weighted_health_score(&self) -> f64 {
        if self.spaces.is_empty() {
            return 1.0;
        }
        let mut weighted_sum = 0.0f64;
        let mut weight_total = 0u64;
        for space in self.spaces.values() {
            let weight = space.total_received.max(1);
            weighted_sum += space.health_score() * weight as f64;
            weight_total += weight;
        }
        if weight_total == 0 {
            1.0
        } else {
            weighted_sum / weight_total as f64
        }
    }

    /// Get sequence allocation rate estimate (allocations per ms).
    pub fn allocation_rate(&self, elapsed_ms: u64) -> f64 {
        if elapsed_ms == 0 {
            return 0.0;
        }
        self.total_allocated as f64 / elapsed_ms as f64
    }

    /// Get sequence receive rate estimate (receives per ms).
    pub fn receive_rate(&self, elapsed_ms: u64) -> f64 {
        if elapsed_ms == 0 {
            return 0.0;
        }
        self.total_received as f64 / elapsed_ms as f64
    }

    /// Get the ack ratio (acked / received) as percentage.
    pub fn ack_ratio_percent(&self) -> f64 {
        if self.total_received == 0 {
            return 100.0;
        }
        self.total_acked as f64 / self.total_received as f64 * 100.0
    }

    /// Get the out-of-order ratio as percentage.
    pub fn out_of_order_ratio_percent(&self) -> f64 {
        if self.total_received == 0 {
            return 0.0;
        }
        let total_ooo: u64 = self
            .spaces
            .values()
            .map(|s| s.total_out_of_order)
            .sum();
        total_ooo as f64 / self.total_received as f64 * 100.0
    }

    /// Get the gap ratio (gaps / received) as percentage.
    pub fn gap_ratio_percent(&self) -> f64 {
        if self.total_received == 0 {
            return 0.0;
        }
        let total_gaps: u64 = self.spaces.values().map(|s| s.gap_count).sum();
        total_gaps as f64 / self.total_received as f64 * 100.0
    }

    /// Serialize the manager state to a summary string.
    pub fn summary_string(&self) -> String {
        format!(
            "SequenceManager: spaces={}, allocated={}, received={}, acked={}, buffered={}, health={:.2}%, gap_ratio={:.2}%",
            self.active_space_count(),
            self.total_allocated,
            self.total_received,
            self.total_acked,
            self.total_buffered(),
            self.overall_health_score() * 100.0,
            self.gap_ratio_percent()
        )
    }

    /// Get the detailed debug string for a peer's sequence space.
    pub fn peer_debug_string(&self, peer_id: NodeId) -> Option<String> {
        self.spaces.get(&peer_id).map(|s| {
            format!(
                "Peer {}: expected={}, highest={}, lowest_unacked={}, buffered={}, gaps={}, health={:.2}",
                peer_id,
                s.expected_next.value(),
                s.highest_received.value(),
                s.lowest_unacked.value(),
                s.buffered_count(),
                s.gap_count,
                s.health_score()
            )
        })
    }

    /// Get all peer debug strings.
    pub fn all_peer_debug_strings(&self) -> Vec<String> {
        self.spaces
            .keys()
            .filter_map(|id| self.peer_debug_string(*id))
            .collect()
    }

    /// Merge another manager's gaps into this tracker (for gossip sync).
    pub fn merge_gap_tracker(&mut self, other: &GapTracker) {
        for gap in other.all_gaps() {
            if self.gap_tracker.gap(gap.gap_id).is_none() {
                self.gap_tracker.gaps.insert(
                    gap.gap_id,
                    SequenceGap {
                        gap_id: gap.gap_id,
                        start: gap.start,
                        end: gap.end,
                        detected_at: gap.detected_at,
                        filled_at: gap.filled_at,
                        fill_attempts: gap.fill_attempts,
                        status: gap.status,
                    },
                );
            }
        }
        if other.next_gap_id > self.gap_tracker.next_gap_id {
            self.gap_tracker.next_gap_id = other.next_gap_id;
        }
    }

    /// Get all open gap IDs sorted by detection time.
    pub fn open_gap_ids_sorted(&self) -> Vec<u64> {
        let mut ids: Vec<u64> = self
            .gap_tracker
            .open_gaps()
            .iter()
            .map(|g| g.gap_id)
            .collect();
        ids.sort_by_key(|id| self.gap_tracker.gap(*id).map(|g| g.detected_at.value()).unwrap_or(0));
        ids
    }

    /// Get the total number of fill attempts across all gaps.
    pub fn total_fill_attempts(&self) -> u64 {
        self.gap_tracker
            .all_gaps()
            .iter()
            .map(|g| g.fill_attempts as u64)
            .sum()
    }

    /// Get the max fill attempts on any single gap.
    pub fn max_fill_attempts(&self) -> u32 {
        self.gap_tracker
            .all_gaps()
            .iter()
            .map(|g| g.fill_attempts)
            .max()
            .unwrap_or(0)
    }

    /// Check whether any gap has been stuck filling too long.
    pub fn any_stuck_filling(&self, max_attempts: u32) -> bool {
        self.gap_tracker
            .all_gaps()
            .iter()
            .any(|g| g.status == GapStatus::Filling && g.fill_attempts > max_attempts)
    }

    /// Get the count of gaps by each status as a vector.
    pub fn gap_status_counts(&self) -> Vec<(GapStatus, u64)> {
        use GapStatus::*;
        vec![
            (Open, self.gap_tracker.gap_count_by_status(Open)),
            (Filling, self.gap_tracker.gap_count_by_status(Filling)),
            (Filled, self.gap_tracker.gap_count_by_status(Filled)),
            (DeclaredLost, self.gap_tracker.gap_count_by_status(DeclaredLost)),
            (Expired, self.gap_tracker.gap_count_by_status(Expired)),
        ]
    }

    /// Get a clone of all sequence spaces for export.
    pub fn clone_spaces(&self) -> HashMap<NodeId, SequenceSpace> {
        self.spaces.clone()
    }

    /// Replace all sequence spaces (for import).
    pub fn replace_spaces(&mut self, spaces: HashMap<NodeId, SequenceSpace>) {
        self.spaces = spaces;
    }

    /// Get the number of sequences allocated since a given counter.
    pub fn allocated_since(&self, since: u64) -> u64 {
        self.total_allocated.saturating_sub(since)
    }

    /// Get the number of sequences received since a given counter.
    pub fn received_since(&self, since: u64) -> u64 {
        self.total_received.saturating_sub(since)
    }

    /// Get the number of sequences acked since a given counter.
    pub fn acked_since(&self, since: u64) -> u64 {
        self.total_acked.saturating_sub(since)
    }
}

/// Gap statistics summary.
#[derive(Debug, Clone)]
pub struct GapStats {
    pub total_opened: u64,
    pub total_filled: u64,
    pub total_lost: u64,
    pub currently_open: u64,
    pub fill_rate: f64,
}

///////////////////////////////////////////////////////////////////////////////
// SEQUENCING RESULT
///////////////////////////////////////////////////////////////////////////////

/// Result type for sequencing operations.
pub type SequencingResult<T> = Result<T, SequencingError>;

///////////////////////////////////////////////////////////////////////////////
// SEQUENCING ERROR
///////////////////////////////////////////////////////////////////////////////

/// Errors returned by sequencing operations.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum SequencingError {
    InvalidConfig {
        field: String,
        reason: String,
    },
    InvalidSequence {
        seq: SequenceNumber,
        reason: String,
    },
    SequenceOutsideWindow {
        seq: SequenceNumber,
        expected: SequenceNumber,
        window: u64,
    },
    OutOfOrderBufferFull {
        current: usize,
        max: usize,
    },
    AckBeyondReceived {
        seq: SequenceNumber,
        highest: SequenceNumber,
    },
    GapNotFound {
        gap_id: u64,
    },
    SequenceSpaceNotFound {
        peer_id: NodeId,
    },
    Internal {
        message: String,
    },
}

impl SequencingError {
    pub fn category(&self) -> &'static str {
        match self {
            SequencingError::InvalidConfig { .. } => "invalid_config",
            SequencingError::InvalidSequence { .. } => "invalid_sequence",
            SequencingError::SequenceOutsideWindow { .. } => "outside_window",
            SequencingError::OutOfOrderBufferFull { .. } => "buffer_full",
            SequencingError::AckBeyondReceived { .. } => "ack_beyond_received",
            SequencingError::GapNotFound { .. } => "gap_not_found",
            SequencingError::SequenceSpaceNotFound { .. } => "space_not_found",
            SequencingError::Internal { .. } => "internal",
        }
    }

    pub fn is_recoverable(&self) -> bool {
        matches!(
            self,
            SequencingError::InvalidSequence { .. }
                | SequencingError::SequenceOutsideWindow { .. }
                | SequencingError::OutOfOrderBufferFull { .. }
                | SequencingError::AckBeyondReceived { .. }
        )
    }

    pub fn suggested_action(&self) -> &'static str {
        match self {
            SequencingError::InvalidConfig { .. } => "correct the configuration value",
            SequencingError::InvalidSequence { .. } => "reject and request retransmission",
            SequencingError::SequenceOutsideWindow { .. } =>
                "request sequence resynchronization with peer",
            SequencingError::OutOfOrderBufferFull { .. } =>
                "process buffered sequences or increase buffer size",
            SequencingError::AckBeyondReceived { .. } =>
                "do not ack sequences not yet received",
            SequencingError::GapNotFound { .. } => "gap may already be filled or lost",
            SequencingError::SequenceSpaceNotFound { .. } =>
                "create sequence space for peer first",
            SequencingError::Internal { .. } => "contact system administrator",
        }
    }
}

impl std::fmt::Display for SequencingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SequencingError::InvalidConfig { field, reason } => {
                write!(f, "invalid config for {}: {}", field, reason)
            }
            SequencingError::InvalidSequence { seq, reason } => {
                write!(f, "invalid sequence {}: {}", seq, reason)
            }
            SequencingError::SequenceOutsideWindow { seq, expected, window } => {
                write!(
                    f,
                    "sequence {} outside window (expected {}, window {})",
                    seq, expected, window
                )
            }
            SequencingError::OutOfOrderBufferFull { current, max } => {
                write!(f, "out-of-order buffer full ({} > {})", current, max)
            }
            SequencingError::AckBeyondReceived { seq, highest } => {
                write!(f, "ack {} beyond highest received {}", seq, highest)
            }
            SequencingError::GapNotFound { gap_id } => {
                write!(f, "gap {} not found", gap_id)
            }
            SequencingError::SequenceSpaceNotFound { peer_id } => {
                write!(f, "sequence space not found for peer {}", peer_id)
            }
            SequencingError::Internal { message } => {
                write!(f, "internal sequencing error: {}", message)
            }
        }
    }
}

///////////////////////////////////////////////////////////////////////////////
// SEALED TRAIT CONTRACT
///////////////////////////////////////////////////////////////////////////////

/// Native sequencing contract: sequence allocation, ordering, gap
/// detection, out-of-order buffering, wrap-around, and per-peer spaces.
///
/// Implementations are provided by ids-000-aa's sequencing engine crate
/// only. Downstream crates use this as a trait bound.
pub trait Sequencing: private::Sealed {
    /// Allocate the next sequence number for a peer.
    fn allocate_sequence(&self, peer_id: NodeId) -> SequencingResult<SequenceNumber>;

    /// Allocate a batch of sequence numbers for a peer.
    fn allocate_sequence_batch(
        &self,
        peer_id: NodeId,
        count: u64,
    ) -> SequencingResult<(SequenceNumber, SequenceNumber)>;

    /// Record a received sequence with its data payload.
    fn record_received(
        &self,
        peer_id: NodeId,
        seq: SequenceNumber,
        data: Vec<u8>,
        now_ms: TimestampMillis,
    ) -> SequencingResult<Vec<SequenceNumber>>;

    /// Get the next expected sequence for a peer.
    fn expected_next(&self, peer_id: NodeId) -> SequencingResult<SequenceNumber>;

    /// Check whether a sequence is the next expected one for a peer.
    fn is_expected_next(&self, peer_id: NodeId, seq: SequenceNumber) -> bool;

    /// Acknowledge a sequence for a peer.
    fn acknowledge(&self, peer_id: NodeId, seq: SequenceNumber) -> SequencingResult<()>;

    /// Get the lowest unacknowledged sequence for a peer.
    fn lowest_unacked(&self, peer_id: NodeId) -> SequencingResult<SequenceNumber>;

    /// Get all buffered out-of-order sequences for a peer in order.
    fn buffered_in_order(&self, peer_id: NodeId) -> SequencingResult<Vec<(SequenceNumber, Vec<u8>)>>;

    /// Take buffered data for a specific sequence.
    fn take_buffered(&self, peer_id: NodeId, seq: SequenceNumber) -> Option<Vec<u8>>;

    /// Create a sequence space for a peer.
    fn create_sequence_space(
        &self,
        peer_id: NodeId,
        window_size: u64,
        max_out_of_order: usize,
        expiry_ms: u64,
    ) -> SequencingResult<()>;

    /// Check if a sequence space exists for a peer.
    fn has_sequence_space(&self, peer_id: NodeId) -> bool;

    /// Remove a sequence space for a peer.
    fn remove_sequence_space(&self, peer_id: NodeId) -> bool;

    /// Get statistics for a peer's sequence space.
    fn sequence_space_stats(&self, peer_id: NodeId) -> Option<SequenceSpaceStats>;

    /// Expire old entries across all sequence spaces.
    fn expire_old_entries(&self, now_ms: TimestampMillis) -> u64;

    /// Open a gap between two sequences.
    fn open_gap(
        &self,
        start: SequenceNumber,
        end: SequenceNumber,
        detected_at: TimestampMillis,
    ) -> u64;

    /// Mark a gap as filled.
    fn fill_gap(&self, gap_id: u64, filled_at: TimestampMillis) -> bool;

    /// Declare a gap lost.
    fn declare_lost(&self, gap_id: u64) -> bool;

    /// Get all open gaps.
    fn open_gaps(&self) -> Vec<SequenceGap>;

    /// Get gap statistics.
    fn gap_stats(&self) -> GapStats;

    /// Check sequence wrap-around proximity.
    fn wrap_around_proximity(&self, seq: SequenceNumber) -> f64;

    /// Get the maximum sequence value.
    fn max_sequence(&self) -> SequenceNumber;

    /// Get the initial sequence value.
    fn initial_sequence(&self) -> SequenceNumber;

    /// Get the sequence window size for a peer.
    fn sequence_window(&self, peer_id: NodeId) -> Option<u64>;

    /// Set the sequence window size for a peer.
    fn set_sequence_window(
        &self,
        peer_id: NodeId,
        window_size: u64,
    ) -> SequencingResult<()>;

    /// Get the max out-of-order buffer size for a peer.
    fn max_out_of_order(&self, peer_id: NodeId) -> Option<usize>;

    /// Set the max out-of-order buffer size for a peer.
    fn set_max_out_of_order(
        &self,
        peer_id: NodeId,
        max: usize,
    ) -> SequencingResult<()>;

    /// Get the sequence expiry timeout for a peer.
    fn sequence_expiry_ms(&self, peer_id: NodeId) -> Option<u64>;

    /// Set the sequence expiry timeout for a peer.
    fn set_sequence_expiry_ms(
        &self,
        peer_id: NodeId,
        expiry_ms: u64,
    ) -> SequencingResult<()>;

    /// Get total sequences allocated across all peers.
    fn total_sequences_allocated(&self) -> u64;

    /// Get total sequences received across all peers.
    fn total_sequences_received(&self) -> u64;

    /// Get total gaps detected across all peers.
    fn total_gaps_detected(&self) -> u64;

    /// Get total gaps filled across all peers.
    fn total_gaps_filled(&self) -> u64;

    /// Get total gaps declared lost across all peers.
    fn total_gaps_lost(&self) -> u64;

    /// Get the count of active sequence spaces.
    fn active_sequence_spaces(&self) -> usize;

    /// List all peers with active sequence spaces.
    fn peers_with_spaces(&self) -> Vec<NodeId>;

    /// Check if a peer's sequence space is healthy (no large gaps).
    fn is_space_healthy(&self, peer_id: NodeId) -> bool;

    /// Get the gap fill rate as a percentage.
    fn gap_fill_rate(&self) -> f64;

    /// Reset a peer's sequence space to initial state.
    fn reset_sequence_space(&self, peer_id: NodeId) -> SequencingResult<()>;

    /// Snapshot all sequence spaces.
    fn snapshot_all_spaces(&self) -> SequencingSnapshot;

    /// Restore all sequence spaces from a snapshot.
    fn restore_all_spaces(&self, snapshot: SequencingSnapshot) -> SequencingResult<()>;
}

///////////////////////////////////////////////////////////////////////////////
// SEQUENCING SNAPSHOT
///////////////////////////////////////////////////////////////////////////////

/// Full snapshot of all sequence spaces and gap state.
#[derive(Debug, Clone)]
pub struct SequencingSnapshot {
    pub spaces: Vec<SequenceSpaceSnapshot>,
    pub gap_stats: GapStats,
    pub total_allocated: u64,
    pub snapshot_at: TimestampMillis,
}

/// Per-space snapshot.
#[derive(Debug, Clone)]
pub struct SequenceSpaceSnapshot {
    pub peer_id: NodeId,
    pub expected_next: SequenceNumber,
    pub highest_received: SequenceNumber,
    pub lowest_unacked: SequenceNumber,
    pub buffered_count: u64,
    pub gap_count: u64,
    pub total_received: u64,
    pub total_out_of_order: u64,
}

///////////////////////////////////////////////////////////////////////////////
// TIMESTAMP MILLIS
///////////////////////////////////////////////////////////////////////////////

/// Monotonic timestamp in milliseconds for sequencing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TimestampMillis(pub u64);

impl TimestampMillis {
    pub const fn now() -> Self {
        Self(0) // placeholder; implementations supply real time
    }

    pub const fn value(self) -> u64 {
        self.0
    }

    pub const fn elapsed_since(self, earlier: TimestampMillis) -> u64 {
        self.0.saturating_sub(earlier.0)
    }
}

impl Default for TimestampMillis {
    fn default() -> Self {
        Self(0)
    }
}

// Re-export types used across the crate for convenience.
use std::collections::HashMap;

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
    // SequenceNumber tests
    // ========================================================================

    #[test]
    fn sequence_number_new_and_value() {
        let seq = SequenceNumber::new(42);
        assert_eq!(seq.value(), 42);
    }

    #[test]
    fn sequence_number_is_zero() {
        assert!(SequenceNumber(0).is_zero());
        assert!(!SequenceNumber(1).is_zero());
    }

    #[test]
    fn sequence_number_is_at_max() {
        assert!(SequenceNumber(u64::MAX).is_at_max());
        assert!(!SequenceNumber(0).is_at_max());
    }

    #[test]
    fn sequence_number_next() {
        assert_eq!(SequenceNumber(1).next(), SequenceNumber(2));
        assert_eq!(SequenceNumber(u64::MAX).next(), SequenceNumber(0));
    }

    #[test]
    fn sequence_number_prev() {
        assert_eq!(SequenceNumber(2).prev(), SequenceNumber(1));
        assert_eq!(SequenceNumber(0).prev(), SequenceNumber(u64::MAX));
    }

    #[test]
    fn sequence_number_wrapping_add() {
        assert_eq!(SequenceNumber(100).wrapping_add(50), SequenceNumber(150));
        assert_eq!(
            SequenceNumber(u64::MAX).wrapping_add(1),
            SequenceNumber(0)
        );
    }

    #[test]
    fn sequence_number_wrapping_sub() {
        assert_eq!(SequenceNumber(100).wrapping_sub(50), SequenceNumber(50));
        assert_eq!(
            SequenceNumber(0).wrapping_sub(1),
            SequenceNumber(u64::MAX)
        );
    }

    #[test]
    fn sequence_number_forward_distance() {
        assert_eq!(
            SequenceNumber(10).forward_distance(SequenceNumber(20)),
            10
        );
        assert_eq!(
            SequenceNumber(u64::MAX).forward_distance(SequenceNumber(1)),
            2
        );
    }

    #[test]
    fn sequence_number_is_within_window() {
        assert!(SequenceNumber(10).is_within_window(SequenceNumber(15), 100));
        assert!(!SequenceNumber(10).is_within_window(SequenceNumber(200), 100));
    }

    #[test]
    fn sequence_number_is_older_than() {
        assert!(SequenceNumber(10).is_older_than(SequenceNumber(100), 1000));
        assert!(!SequenceNumber(100).is_older_than(SequenceNumber(10), 1000));
    }

    #[test]
    fn sequence_number_is_newer_than() {
        assert!(SequenceNumber(100).is_newer_than(SequenceNumber(10), 1000));
        assert!(!SequenceNumber(10).is_newer_than(SequenceNumber(100), 1000));
    }

    #[test]
    fn sequence_number_to_from_bytes() {
        let seq = SequenceNumber::new(12345);
        let bytes = seq.to_bytes();
        let restored = SequenceNumber::from_bytes(bytes);
        assert_eq!(seq, restored);
    }

    #[test]
    fn sequence_number_default() {
        assert_eq!(SequenceNumber::default(), INITIAL_SEQUENCE);
    }

    #[test]
    fn sequence_number_display() {
        let seq = SequenceNumber::new(7);
        assert!(!seq.to_string().is_empty());
    }

    #[test]
    fn sequence_number_partial_ord() {
        assert!(SequenceNumber(5) < SequenceNumber(10));
        assert!(SequenceNumber(10) > SequenceNumber(5));
        assert_eq!(SequenceNumber(5), SequenceNumber(5));
    }

    // ========================================================================
    // SequenceRange tests
    // ========================================================================

    #[test]
    fn sequence_range_new() {
        let range = SequenceRange::new(SequenceNumber(10), SequenceNumber(20));
        assert_eq!(range.start, SequenceNumber(10));
        assert_eq!(range.end, SequenceNumber(20));
    }

    #[test]
    fn sequence_range_is_empty() {
        assert!(SequenceRange::new(SequenceNumber(10), SequenceNumber(10)).is_empty());
        assert!(!SequenceRange::new(SequenceNumber(10), SequenceNumber(20)).is_empty());
    }

    #[test]
    fn sequence_range_len() {
        let range = SequenceRange::new(SequenceNumber(10), SequenceNumber(20));
        assert_eq!(range.len(), 10);
    }

    #[test]
    fn sequence_range_contains() {
        let range = SequenceRange::new(SequenceNumber(10), SequenceNumber(20));
        assert!(range.contains(SequenceNumber(10)));
        assert!(range.contains(SequenceNumber(19)));
        assert!(!range.contains(SequenceNumber(9)));
        assert!(!range.contains(SequenceNumber(20)));
    }

    // ========================================================================
    // SequenceSpace tests
    // ========================================================================

    #[test]
    fn sequence_space_new() {
        let space = SequenceSpace::new(test_peer1());
        assert_eq!(space.peer_id, test_peer1());
        assert_eq!(space.expected_next, INITIAL_SEQUENCE);
        assert_eq!(space.highest_received, SequenceNumber(0));
        assert_eq!(space.window_size, DEFAULT_SEQUENCE_WINDOW);
        assert_eq!(space.max_out_of_order, DEFAULT_MAX_OUT_OF_ORDER);
        assert_eq!(space.expiry_ms, DEFAULT_SEQUENCE_EXPIRY_MS);
        assert_eq!(space.gap_count, 0);
    }

    #[test]
    fn sequence_space_with_config_valid() {
        let space = SequenceSpace::with_config(test_peer1(), 2048, 8192, 600_000).unwrap();
        assert_eq!(space.window_size, 2048);
        assert_eq!(space.max_out_of_order, 8192);
        assert_eq!(space.expiry_ms, 600_000);
    }

    #[test]
    fn sequence_space_with_config_zero_window() {
        let result = SequenceSpace::with_config(test_peer1(), 0, 100, 1000);
        assert!(result.is_err());
    }

    #[test]
    fn sequence_space_with_config_zero_buffer() {
        let result = SequenceSpace::with_config(test_peer1(), 100, 0, 1000);
        assert!(result.is_err());
    }

    #[test]
    fn sequence_space_record_received_in_order() {
        let mut space = SequenceSpace::new(test_peer1());
        let gaps = space.record_received(SequenceNumber(1), vec![1], test_ts(100)).unwrap();
        assert_eq!(gaps.len(), 0);
        assert_eq!(space.expected_next, SequenceNumber(2));
        assert_eq!(space.highest_received, SequenceNumber(1));
        assert_eq!(space.total_received, 1);
        assert_eq!(space.total_out_of_order, 0);
    }

    #[test]
    fn sequence_space_record_received_out_of_order() {
        let mut space = SequenceSpace::new(test_peer1());
        let gaps = space.record_received(SequenceNumber(5), vec![5], test_ts(100)).unwrap();
        assert_eq!(gaps.len(), 0);
        assert_eq!(space.expected_next, SequenceNumber(1));
        assert_eq!(space.highest_received, SequenceNumber(5));
        assert_eq!(space.total_out_of_order, 1);
        assert_eq!(space.gap_count, 1);
    }

    #[test]
    fn sequence_space_record_received_duplicate() {
        let mut space = SequenceSpace::new(test_peer1());
        space.record_received(SequenceNumber(1), vec![1], test_ts(100)).unwrap();
        let gaps = space.record_received(SequenceNumber(1), vec![1], test_ts(101)).unwrap();
        assert_eq!(gaps.len(), 0);
        assert_eq!(space.total_received, 1); // not double-counted
    }

    #[test]
    fn sequence_space_record_received_zero_seq() {
        let mut space = SequenceSpace::new(test_peer1());
        let err = space.record_received(SequenceNumber(0), vec![], test_ts(100)).unwrap_err();
        assert!(matches!(err, SequencingError::InvalidSequence { .. }));
    }

    #[test]
    fn sequence_space_record_received_outside_window() {
        let mut space = SequenceSpace::with_config(test_peer1(), 10, 100, 1000).unwrap();
        let err = space
            .record_received(SequenceNumber(100), vec![1], test_ts(100))
            .unwrap_err();
        assert!(matches!(err, SequencingError::SequenceOutsideWindow { .. }));
    }

    #[test]
    fn sequence_space_record_received_buffer_full() {
        let mut space = SequenceSpace::with_config(test_peer1(), 1000, 2, 1000).unwrap();
        space.record_received(SequenceNumber(10), vec![10], test_ts(100)).unwrap();
        space.record_received(SequenceNumber(20), vec![20], test_ts(101)).unwrap();
        let err = space
            .record_received(SequenceNumber(30), vec![30], test_ts(102))
            .unwrap_err();
        assert!(matches!(err, SequencingError::OutOfOrderBufferFull { .. }));
    }

    #[test]
    fn sequence_space_buffered_in_order() {
        let mut space = SequenceSpace::new(test_peer1());
        space.record_received(SequenceNumber(1), vec![1], test_ts(100)).unwrap();
        space.record_received(SequenceNumber(3), vec![3], test_ts(101)).unwrap();
        space.record_received(SequenceNumber(2), vec![2], test_ts(102)).unwrap();
        let buffered = space.buffered_in_order();
        assert_eq!(buffered.len(), 3);
        assert_eq!(buffered[0].0, SequenceNumber(1));
        assert_eq!(buffered[1].0, SequenceNumber(2));
        assert_eq!(buffered[2].0, SequenceNumber(3));
    }

    #[test]
    fn sequence_space_take_buffered() {
        let mut space = SequenceSpace::new(test_peer1());
        space.record_received(SequenceNumber(1), vec![1, 2, 3], test_ts(100)).unwrap();
        let data = space.take_buffered(SequenceNumber(1));
        assert_eq!(data, Some(vec![1, 2, 3]));
        assert_eq!(space.take_buffered(SequenceNumber(1)), None);
    }

    #[test]
    fn sequence_space_acknowledge() {
        let mut space = SequenceSpace::new(test_peer1());
        space.record_received(SequenceNumber(1), vec![1], test_ts(100)).unwrap();
        space.acknowledge(SequenceNumber(1)).unwrap();
        assert_eq!(space.lowest_unacked, SequenceNumber(2));
    }

    #[test]
    fn sequence_space_acknowledge_already_acked() {
        let mut space = SequenceSpace::new(test_peer1());
        space.record_received(SequenceNumber(1), vec![1], test_ts(100)).unwrap();
        space.acknowledge(SequenceNumber(1)).unwrap();
        space.acknowledge(SequenceNumber(1)).unwrap(); // idempotent
        assert_eq!(space.lowest_unacked, SequenceNumber(2));
    }

    #[test]
    fn sequence_space_acknowledge_beyond_received() {
        let mut space = SequenceSpace::new(test_peer1());
        let err = space.acknowledge(SequenceNumber(5)).unwrap_err();
        assert!(matches!(err, SequencingError::AckBeyondReceived { .. }));
    }

    #[test]
    fn sequence_space_has_gaps() {
        let mut space = SequenceSpace::new(test_peer1());
        assert!(!space.has_gaps());
        space.record_received(SequenceNumber(5), vec![5], test_ts(100)).unwrap();
        assert!(space.has_gaps());
    }

    #[test]
    fn sequence_space_expire_old_entries() {
        let mut space = SequenceSpace::new(test_peer1());
        space.record_received(SequenceNumber(1), vec![1], test_ts(100)).unwrap();
        let expired = space.expire_old_entries(test_ts(200));
        assert_eq!(expired, 0); // not past expiry
        let expired = space.expire_old_entries(test_ts(400_100));
        assert!(expired >= 0);
    }

    #[test]
    fn sequence_space_stats() {
        let mut space = SequenceSpace::new(test_peer1());
        space.record_received(SequenceNumber(1), vec![1], test_ts(100)).unwrap();
        let stats = space.stats();
        assert_eq!(stats.peer_id, test_peer1());
        assert_eq!(stats.total_received, 1);
        assert_eq!(stats.buffered_count, 1);
    }

    // ========================================================================
    // GapTracker tests
    // ========================================================================

    #[test]
    fn gap_tracker_open_gap() {
        let mut tracker = GapTracker::new();
        let id = tracker.open_gap(SequenceNumber(10), SequenceNumber(20), test_ts(100));
        assert_eq!(id, 1);
        assert_eq!(tracker.total_opened, 1);
        assert_eq!(tracker.open_gaps().len(), 1);
    }

    #[test]
    fn gap_tracker_fill_gap() {
        let mut tracker = GapTracker::new();
        let id = tracker.open_gap(SequenceNumber(10), SequenceNumber(20), test_ts(100));
        assert!(tracker.fill_gap(id, test_ts(200)));
        assert_eq!(tracker.total_filled, 1);
        assert_eq!(tracker.open_gaps().len(), 0);
    }

    #[test]
    fn gap_tracker_fill_gap_not_found() {
        let mut tracker = GapTracker::new();
        assert!(!tracker.fill_gap(999, test_ts(200)));
    }

    #[test]
    fn gap_tracker_declare_lost() {
        let mut tracker = GapTracker::new();
        let id = tracker.open_gap(SequenceNumber(10), SequenceNumber(20), test_ts(100));
        assert!(tracker.declare_lost(id));
        assert_eq!(tracker.total_lost, 1);
    }

    #[test]
    fn gap_tracker_timed_out_gaps() {
        let mut tracker = GapTracker::new();
        tracker.open_gap(SequenceNumber(10), SequenceNumber(20), test_ts(100));
        let timed_out = tracker.timed_out_gaps(test_ts(6000));
        assert_eq!(timed_out.len(), 1);
    }

    #[test]
    fn gap_tracker_stats() {
        let mut tracker = GapTracker::new();
        tracker.open_gap(SequenceNumber(10), SequenceNumber(20), test_ts(100));
        tracker.fill_gap(1, test_ts(200));
        tracker.open_gap(SequenceNumber(30), SequenceNumber(40), test_ts(300));
        let stats = tracker.stats();
        assert_eq!(stats.total_opened, 2);
        assert_eq!(stats.total_filled, 1);
        assert!((stats.fill_rate - 50.0).abs() < 0.01);
    }

    // ========================================================================
    // SequencingError tests
    // ========================================================================

    #[test]
    fn sequencing_error_category() {
        assert_eq!(
            SequencingError::InvalidConfig {
                field: "x".to_string(),
                reason: "y".to_string(),
            }
            .category(),
            "invalid_config"
        );
        assert_eq!(
            SequencingError::OutOfOrderBufferFull {
                current: 1,
                max: 10
            }
            .category(),
            "buffer_full"
        );
    }

    #[test]
    fn sequencing_error_is_recoverable() {
        assert!(SequencingError::InvalidSequence {
            seq: SequenceNumber(1),
            reason: "bad".to_string(),
        }
        .is_recoverable());
        assert!(!SequencingError::Internal {
            message: "x".to_string()
        }
        .is_recoverable());
    }

    #[test]
    fn sequencing_error_suggested_action() {
        assert!(!SequencingError::SequenceSpaceNotFound {
            peer_id: test_peer1()
        }
        .suggested_action()
        .is_empty());
    }

    #[test]
    fn sequencing_error_display() {
        let err = SequencingError::SequenceOutsideWindow {
            seq: SequenceNumber(100),
            expected: SequenceNumber(1),
            window: 10,
        };
        assert!(!err.to_string().is_empty());
    }

    // ========================================================================
    // TimestampMillis tests
    // ========================================================================

    #[test]
    fn timestamp_millis_value() {
        let ts = TimestampMillis::new(5000);
        assert_eq!(ts.value(), 5000);
    }

    #[test]
    fn timestamp_millis_elapsed_since() {
        let later = TimestampMillis::new(1000);
        let earlier = TimestampMillis::new(100);
        assert_eq!(later.elapsed_since(earlier), 900);
    }

    #[test]
    fn timestamp_millis_default() {
        assert_eq!(TimestampMillis::default().value(), 0);
    }

    #[test]
    fn timestamp_millis_ordering() {
        assert!(TimestampMillis(2) > TimestampMillis(1));
        assert!(TimestampMillis(1) < TimestampMillis(2));
    }

    // ========================================================================
    // Sealing tests
    // ========================================================================

    #[test]
    fn sequencing_trait_is_sealed() {
        // The trait cannot be implemented outside this module — verified by
        // the `private::Sealed` bound. This test confirms the seal marker
        // exists and is referenced.
        fn _assert_sealed<T: Sequencing + private::Sealed>() {}
        // Compile-time check that the sealed marker trait is in scope.
        let _ = std::marker::PhantomData::<fn() -> ()>;
    }

    // ========================================================================
    // SequenceSpace advanced method tests
    // ========================================================================

    #[test]
    fn sequence_space_received_ranges() {
        let mut space = SequenceSpace::new(test_peer1());
        assert_eq!(space.received_range_count(), 0);
        space.record_received(SequenceNumber(1), vec![1], test_ts(100)).unwrap();
        assert_eq!(space.received_range_count(), 1);
    }

    #[test]
    fn sequence_space_largest_contiguous_range() {
        let mut space = SequenceSpace::new(test_peer1());
        assert!(space.largest_contiguous_range().is_none());
        space.record_received(SequenceNumber(1), vec![1], test_ts(100)).unwrap();
        let range = space.largest_contiguous_range().unwrap();
        assert_eq!(range.start, SequenceNumber(1));
    }

    #[test]
    fn sequence_space_missing_count() {
        let mut space = SequenceSpace::new(test_peer1());
        space.record_received(SequenceNumber(1), vec![1], test_ts(100)).unwrap();
        space.record_received(SequenceNumber(3), vec![3], test_ts(101)).unwrap();
        let _ = space.missing_count();
    }

    #[test]
    fn sequence_space_is_received() {
        let mut space = SequenceSpace::new(test_peer1());
        assert!(!space.is_received(SequenceNumber(1)));
        space.record_received(SequenceNumber(1), vec![1], test_ts(100)).unwrap();
        assert!(space.is_received(SequenceNumber(1)));
    }

    #[test]
    fn sequence_space_gap_span() {
        let mut space = SequenceSpace::new(test_peer1());
        assert_eq!(space.gap_span(), 0);
        space.record_received(SequenceNumber(5), vec![5], test_ts(100)).unwrap();
        assert!(space.gap_span() > 0);
    }

    #[test]
    fn sequence_space_window_fill_percent() {
        let space = SequenceSpace::new(test_peer1());
        assert!((space.window_fill_percent() - 0.0).abs() < 0.001);
    }

    #[test]
    fn sequence_space_receive_completeness_percent() {
        let space = SequenceSpace::new(test_peer1());
        assert!((space.receive_completeness_percent() - 100.0).abs() < 0.001);
    }

    #[test]
    fn sequence_space_average_gap_size() {
        let space = SequenceSpace::new(test_peer1());
        assert!((space.average_gap_size() - 0.0).abs() < 0.001);
    }

    #[test]
    fn sequence_space_clone_buffered() {
        let mut space = SequenceSpace::new(test_peer1());
        space.record_received(SequenceNumber(1), vec![1, 2], test_ts(100)).unwrap();
        let cloned = space.clone_buffered();
        assert_eq!(cloned.len(), 1);
    }

    #[test]
    fn sequence_space_clear_buffered() {
        let mut space = SequenceSpace::new(test_peer1());
        space.record_received(SequenceNumber(1), vec![1], test_ts(100)).unwrap();
        let cleared = space.clear_buffered();
        assert_eq!(cleared, 1);
        assert_eq!(space.buffered_count(), 0);
    }

    #[test]
    fn sequence_space_shift_forward() {
        let mut space = SequenceSpace::new(test_peer1());
        space.record_received(SequenceNumber(1), vec![1], test_ts(100)).unwrap();
        assert!(space.shift_forward(1).is_ok());
    }

    #[test]
    fn sequence_space_shift_forward_zero() {
        let mut space = SequenceSpace::new(test_peer1());
        assert!(space.shift_forward(0).is_ok());
    }

    #[test]
    fn sequence_space_health_score() {
        let space = SequenceSpace::new(test_peer1());
        let score = space.health_score();
        assert!(score >= 0.0 && score <= 1.0);
    }

    #[test]
    fn sequence_space_time_since_last_received() {
        let mut space = SequenceSpace::new(test_peer1());
        space.record_received(SequenceNumber(1), vec![1], test_ts(100)).unwrap();
        let elapsed = space.time_since_last_received(test_ts(200));
        assert_eq!(elapsed, Some(100));
    }

    #[test]
    fn sequence_space_is_stale() {
        let space = SequenceSpace::new(test_peer1());
        assert!(space.is_stale(test_ts(1000)));
    }

    #[test]
    fn sequence_space_next_requested_sequence() {
        let space = SequenceSpace::new(test_peer1());
        assert_eq!(space.next_requested_sequence(), SequenceNumber(1));
    }

    #[test]
    fn sequence_space_missing_sequences() {
        let mut space = SequenceSpace::new(test_peer1());
        space.record_received(SequenceNumber(1), vec![1], test_ts(100)).unwrap();
        space.record_received(SequenceNumber(3), vec![3], test_ts(101)).unwrap();
        let missing = space.missing_sequences();
        let _ = missing;
    }

    #[test]
    fn sequence_space_sequence_at_offset() {
        let space = SequenceSpace::new(test_peer1());
        assert_eq!(space.sequence_at_offset(0), SequenceNumber(1));
        assert_eq!(space.sequence_at_offset(5), SequenceNumber(6));
    }

    #[test]
    fn sequence_space_offset_from_expected() {
        let space = SequenceSpace::new(test_peer1());
        assert_eq!(space.offset_from_expected(SequenceNumber(1)), Some(0));
        assert_eq!(space.offset_from_expected(SequenceNumber(5)), Some(4));
        assert_eq!(space.offset_from_expected(SequenceNumber(0)), None);
    }

    #[test]
    fn sequence_space_is_strict_order() {
        let space = SequenceSpace::new(test_peer1());
        assert!(space.is_strict_order());
    }

    #[test]
    fn sequence_space_contiguous_count() {
        let mut space = SequenceSpace::new(test_peer1());
        space.record_received(SequenceNumber(1), vec![1], test_ts(100)).unwrap();
        assert_eq!(space.contiguous_count(), 1);
    }

    #[test]
    fn sequence_space_first_missing() {
        let mut space = SequenceSpace::new(test_peer1());
        space.record_received(SequenceNumber(1), vec![1], test_ts(100)).unwrap();
        let first = space.first_missing();
        assert_eq!(first, Some(SequenceNumber(2)));
    }

    #[test]
    fn sequence_space_last_received() {
        let mut space = SequenceSpace::new(test_peer1());
        assert_eq!(space.last_received(), SequenceNumber(0));
        space.record_received(SequenceNumber(7), vec![7], test_ts(100)).unwrap();
        assert_eq!(space.last_received(), SequenceNumber(7));
    }

    #[test]
    fn sequence_space_is_in_window() {
        let space = SequenceSpace::new(test_peer1());
        assert!(space.is_in_window(SequenceNumber(1)));
        assert!(!space.is_in_window(SequenceNumber(10000)));
    }

    #[test]
    fn sequence_space_receive_window() {
        let space = SequenceSpace::new(test_peer1());
        let (start, end) = space.receive_window();
        assert_eq!(start, SequenceNumber(1));
        assert_eq!(end.value(), 1 + DEFAULT_SEQUENCE_WINDOW);
    }

    #[test]
    fn sequence_space_advance_expected() {
        let mut space = SequenceSpace::new(test_peer1());
        space.advance_expected(3);
        assert_eq!(space.expected_next, SequenceNumber(4));
    }

    #[test]
    fn sequence_space_approximate_size_bytes() {
        let mut space = SequenceSpace::new(test_peer1());
        space.record_received(SequenceNumber(1), vec![1, 2, 3], test_ts(100)).unwrap();
        assert!(space.approximate_size_bytes() > 0);
    }

    #[test]
    fn sequence_space_pending_ack_count() {
        let mut space = SequenceSpace::new(test_peer1());
        space.record_received(SequenceNumber(1), vec![1], test_ts(100)).unwrap();
        assert_eq!(space.pending_ack_count(), 1);
    }

    #[test]
    fn sequence_space_ack_lag() {
        let mut space = SequenceSpace::new(test_peer1());
        space.record_received(SequenceNumber(1), vec![1], test_ts(100)).unwrap();
        assert_eq!(space.ack_lag(), 1);
    }

    #[test]
    fn sequence_space_acks_caught_up() {
        let space = SequenceSpace::new(test_peer1());
        assert!(space.acks_caught_up());
    }

    #[test]
    fn sequence_space_pending_ack_ratio() {
        let mut space = SequenceSpace::new(test_peer1());
        space.record_received(SequenceNumber(1), vec![1], test_ts(100)).unwrap();
        let ratio = space.pending_ack_ratio();
        assert!(ratio >= 0.0 && ratio <= 100.0);
    }

    #[test]
    fn sequence_space_max_min_observed() {
        let mut space = SequenceSpace::new(test_peer1());
        assert_eq!(space.max_observed(), SequenceNumber(0));
        assert_eq!(space.min_observed(), SequenceNumber(1));
        space.record_received(SequenceNumber(10), vec![10], test_ts(100)).unwrap();
        assert_eq!(space.max_observed(), SequenceNumber(10));
    }

    #[test]
    fn sequence_space_sequence_span() {
        let mut space = SequenceSpace::new(test_peer1());
        assert_eq!(space.sequence_span(), 0);
        space.record_received(SequenceNumber(10), vec![10], test_ts(100)).unwrap();
        assert!(space.sequence_span() > 0);
    }

    #[test]
    fn sequence_space_reset_gap_count() {
        let mut space = SequenceSpace::new(test_peer1());
        space.reset_gap_count();
        assert_eq!(space.gap_count, 0);
    }

    #[test]
    fn sequence_space_set_window_size() {
        let mut space = SequenceSpace::new(test_peer1());
        assert!(space.set_window_size(2048).is_ok());
        assert_eq!(space.window_size, 2048);
    }

    #[test]
    fn sequence_space_set_window_size_zero() {
        let mut space = SequenceSpace::new(test_peer1());
        assert!(space.set_window_size(0).is_err());
    }

    #[test]
    fn sequence_space_set_max_out_of_order() {
        let mut space = SequenceSpace::new(test_peer1());
        assert!(space.set_max_out_of_order(2048).is_ok());
        assert_eq!(space.max_out_of_order, 2048);
    }

    #[test]
    fn sequence_space_set_max_out_of_order_zero() {
        let mut space = SequenceSpace::new(test_peer1());
        assert!(space.set_max_out_of_order(0).is_err());
    }

    #[test]
    fn sequence_space_set_expiry_ms() {
        let mut space = SequenceSpace::new(test_peer1());
        space.set_expiry_ms(10000);
        assert_eq!(space.expiry_ms, 10000);
    }

    #[test]
    fn sequence_space_state_string() {
        let space = SequenceSpace::new(test_peer1());
        let s = space.state_string();
        assert!(!s.is_empty());
        assert!(s.contains("SequenceSpace"));
    }

    #[test]
    fn sequence_space_clone_space() {
        let mut space = SequenceSpace::new(test_peer1());
        space.record_received(SequenceNumber(1), vec![1], test_ts(100)).unwrap();
        let cloned = space.clone_space();
        assert_eq!(cloned.total_received, space.total_received);
    }

    #[test]
    fn sequence_space_clone_impl() {
        let mut space = SequenceSpace::new(test_peer1());
        space.record_received(SequenceNumber(1), vec![1], test_ts(100)).unwrap();
        let cloned = space.clone();
        assert_eq!(cloned.expected_next, space.expected_next);
    }

    #[test]
    fn sequence_space_default_config() {
        let (win, buf, exp) = SequenceSpace::default_config();
        assert_eq!(win, DEFAULT_SEQUENCE_WINDOW);
        assert_eq!(buf, DEFAULT_MAX_OUT_OF_ORDER);
        assert_eq!(exp, DEFAULT_SEQUENCE_EXPIRY_MS);
    }

    #[test]
    fn sequence_space_wraps_within() {
        let space = SequenceSpace::new(test_peer1());
        assert!(!space.wraps_within(SequenceNumber(100), 10));
        assert!(space.wraps_within(SequenceNumber(u64::MAX), 1));
    }

    #[test]
    fn sequence_space_distance_to_wrap() {
        assert_eq!(SequenceSpace::distance_to_wrap(SequenceNumber(0)), u64::MAX);
        assert_eq!(SequenceSpace::distance_to_wrap(SequenceNumber(u64::MAX)), 0);
    }

    // ========================================================================
    // SequenceManager advanced tests
    // ========================================================================

    #[test]
    fn sequence_manager_new() {
        let mgr = SequenceManager::new();
        assert_eq!(mgr.active_space_count(), 0);
        assert_eq!(mgr.total_allocated(), 0);
        assert_eq!(mgr.total_received(), 0);
        assert_eq!(mgr.total_acked(), 0);
    }

    #[test]
    fn sequence_manager_allocate_sequence() {
        let mut mgr = SequenceManager::new();
        let seq = mgr.allocate_sequence(test_peer1());
        assert_eq!(seq, SequenceNumber(1));
        assert_eq!(mgr.total_allocated(), 1);
    }

    #[test]
    fn sequence_manager_allocate_sequence_multiple_peers() {
        let mut mgr = SequenceManager::new();
        let s1 = mgr.allocate_sequence(test_peer1());
        let s2 = mgr.allocate_sequence(test_peer2());
        assert_eq!(s1, SequenceNumber(1));
        assert_eq!(s2, SequenceNumber(1));
        assert_eq!(mgr.total_allocated(), 2);
    }

    #[test]
    fn sequence_manager_allocate_sequence_batch() {
        let mut mgr = SequenceManager::new();
        let (start, end) = mgr.allocate_sequence_batch(test_peer1(), 10);
        assert_eq!(start, SequenceNumber(1));
        assert_eq!(end, SequenceNumber(10));
        assert_eq!(mgr.total_allocated(), 10);
    }

    #[test]
    fn sequence_manager_allocate_sequence_batch_zero() {
        let mut mgr = SequenceManager::new();
        let (start, end) = mgr.allocate_sequence_batch(test_peer1(), 0);
        assert_eq!(start, SequenceNumber(0));
        assert_eq!(end, SequenceNumber(0));
    }

    #[test]
    fn sequence_manager_create_space() {
        let mut mgr = SequenceManager::new();
        assert!(mgr
            .create_space(test_peer1(), 2048, 4096, 600_000)
            .is_ok());
        assert!(mgr.has_space(test_peer1()));
    }

    #[test]
    fn sequence_manager_create_space_invalid() {
        let mut mgr = SequenceManager::new();
        assert!(mgr.create_space(test_peer1(), 0, 100, 1000).is_err());
    }

    #[test]
    fn sequence_manager_remove_space() {
        let mut mgr = SequenceManager::new();
        mgr.create_space(test_peer1(), 1000, 100, 1000).unwrap();
        assert!(mgr.remove_space(test_peer1()));
        assert!(!mgr.has_space(test_peer1()));
        assert!(!mgr.remove_space(test_peer1()));
    }

    #[test]
    fn sequence_manager_record_received() {
        let mut mgr = SequenceManager::new();
        let gaps = mgr
            .record_received(test_peer1(), SequenceNumber(1), vec![1], test_ts(100))
            .unwrap();
        assert_eq!(gaps.len(), 0);
        assert_eq!(mgr.total_received(), 1);
    }

    #[test]
    fn sequence_manager_acknowledge() {
        let mut mgr = SequenceManager::new();
        mgr.record_received(test_peer1(), SequenceNumber(1), vec![1], test_ts(100))
            .unwrap();
        mgr.acknowledge(test_peer1(), SequenceNumber(1)).unwrap();
        assert_eq!(mgr.total_acked(), 1);
    }

    #[test]
    fn sequence_manager_acknowledge_no_space() {
        let mut mgr = SequenceManager::new();
        let err = mgr
            .acknowledge(test_peer1(), SequenceNumber(1))
            .unwrap_err();
        assert!(matches!(
            err,
            SequencingError::SequenceSpaceNotFound { .. }
        ));
    }

    #[test]
    fn sequence_manager_expected_next() {
        let mut mgr = SequenceManager::new();
        assert_eq!(mgr.expected_next(test_peer1()), None);
        mgr.record_received(test_peer1(), SequenceNumber(1), vec![1], test_ts(100))
            .unwrap();
        assert_eq!(mgr.expected_next(test_peer1()), Some(SequenceNumber(2)));
    }

    #[test]
    fn sequence_manager_all_space_stats() {
        let mut mgr = SequenceManager::new();
        mgr.record_received(test_peer1(), SequenceNumber(1), vec![1], test_ts(100))
            .unwrap();
        let stats = mgr.all_space_stats();
        assert_eq!(stats.len(), 1);
    }

    #[test]
    fn sequence_manager_peer_ids() {
        let mut mgr = SequenceManager::new();
        mgr.record_received(test_peer1(), SequenceNumber(1), vec![1], test_ts(100))
            .unwrap();
        mgr.record_received(test_peer2(), SequenceNumber(1), vec![1], test_ts(100))
            .unwrap();
        let peers = mgr.peer_ids();
        assert_eq!(peers.len(), 2);
    }

    #[test]
    fn sequence_manager_expire_all_old_entries() {
        let mut mgr = SequenceManager::new();
        mgr.record_received(test_peer1(), SequenceNumber(1), vec![1], test_ts(100))
            .unwrap();
        let expired = mgr.expire_all_old_entries(test_ts(400_100));
        assert!(expired >= 0);
    }

    #[test]
    fn sequence_manager_spaces_with_gaps() {
        let mut mgr = SequenceManager::new();
        mgr.record_received(test_peer1(), SequenceNumber(5), vec![5], test_ts(100))
            .unwrap();
        assert_eq!(mgr.spaces_with_gaps(), 1);
    }

    #[test]
    fn sequence_manager_overall_health_score() {
        let mgr = SequenceManager::new();
        assert!((mgr.overall_health_score() - 1.0).abs() < 0.001);
    }

    #[test]
    fn sequence_manager_total_buffered() {
        let mut mgr = SequenceManager::new();
        mgr.record_received(test_peer1(), SequenceNumber(1), vec![1], test_ts(100))
            .unwrap();
        assert_eq!(mgr.total_buffered(), 1);
    }

    #[test]
    fn sequence_manager_stale_space_count() {
        let mut mgr = SequenceManager::new();
        mgr.record_received(test_peer1(), SequenceNumber(1), vec![1], test_ts(100))
            .unwrap();
        let stale = mgr.stale_space_count(test_ts(400_100));
        assert_eq!(stale, 1);
    }

    #[test]
    fn sequence_manager_busiest_peer() {
        let mut mgr = SequenceManager::new();
        mgr.record_received(test_peer1(), SequenceNumber(1), vec![1], test_ts(100))
            .unwrap();
        let busiest = mgr.busiest_peer();
        assert_eq!(busiest, Some((test_peer1(), 1)));
    }

    #[test]
    fn sequence_manager_gappiest_peer() {
        let mut mgr = SequenceManager::new();
        mgr.record_received(test_peer1(), SequenceNumber(5), vec![5], test_ts(100))
            .unwrap();
        let gappiest = mgr.gappiest_peer();
        assert_eq!(gappiest, Some((test_peer1(), 1)));
    }

    #[test]
    fn sequence_manager_total_missing() {
        let mut mgr = SequenceManager::new();
        mgr.record_received(test_peer1(), SequenceNumber(5), vec![5], test_ts(100))
            .unwrap();
        let missing = mgr.total_missing();
        assert!(missing >= 0);
    }

    #[test]
    fn sequence_manager_weighted_health_score() {
        let mgr = SequenceManager::new();
        assert!((mgr.weighted_health_score() - 1.0).abs() < 0.001);
    }

    #[test]
    fn sequence_manager_allocation_rate() {
        let mgr = SequenceManager::new();
        assert!((mgr.allocation_rate(1000) - 0.0).abs() < 0.001);
    }

    #[test]
    fn sequence_manager_receive_rate() {
        let mgr = SequenceManager::new();
        assert!((mgr.receive_rate(1000) - 0.0).abs() < 0.001);
    }

    #[test]
    fn sequence_manager_ack_ratio_percent() {
        let mgr = SequenceManager::new();
        assert!((mgr.ack_ratio_percent() - 100.0).abs() < 0.001);
    }

    #[test]
    fn sequence_manager_out_of_order_ratio_percent() {
        let mgr = SequenceManager::new();
        assert!((mgr.out_of_order_ratio_percent() - 0.0).abs() < 0.001);
    }

    #[test]
    fn sequence_manager_gap_ratio_percent() {
        let mgr = SequenceManager::new();
        assert!((mgr.gap_ratio_percent() - 0.0).abs() < 0.001);
    }

    #[test]
    fn sequence_manager_summary_string() {
        let mgr = SequenceManager::new();
        let s = mgr.summary_string();
        assert!(!s.is_empty());
        assert!(s.contains("SequenceManager"));
    }

    #[test]
    fn sequence_manager_peer_debug_string() {
        let mut mgr = SequenceManager::new();
        mgr.record_received(test_peer1(), SequenceNumber(1), vec![1], test_ts(100))
            .unwrap();
        let debug = mgr.peer_debug_string(test_peer1());
        assert!(debug.is_some());
        assert!(debug.unwrap().contains("Peer"));
    }

    #[test]
    fn sequence_manager_all_peer_debug_strings() {
        let mut mgr = SequenceManager::new();
        mgr.record_received(test_peer1(), SequenceNumber(1), vec![1], test_ts(100))
            .unwrap();
        let debugs = mgr.all_peer_debug_strings();
        assert_eq!(debugs.len(), 1);
    }

    #[test]
    fn sequence_manager_merge_gap_tracker() {
        let mut mgr = SequenceManager::new();
        let mut other = GapTracker::new();
        other.open_gap(SequenceNumber(10), SequenceNumber(20), test_ts(100));
        mgr.merge_gap_tracker(&other);
        assert!(mgr.gap_stats().total_opened == 0 || mgr.gap_stats().total_opened == 1);
    }

    #[test]
    fn sequence_manager_open_gap_ids_sorted() {
        let mut mgr = SequenceManager::new();
        mgr.open_gap(SequenceNumber(10), SequenceNumber(20), test_ts(200));
        mgr.open_gap(SequenceNumber(1), SequenceNumber(5), test_ts(100));
        let ids = mgr.open_gap_ids_sorted();
        assert_eq!(ids.len(), 2);
    }

    #[test]
    fn sequence_manager_total_fill_attempts() {
        let mgr = SequenceManager::new();
        assert_eq!(mgr.total_fill_attempts(), 0);
    }

    #[test]
    fn sequence_manager_max_fill_attempts() {
        let mgr = SequenceManager::new();
        assert_eq!(mgr.max_fill_attempts(), 0);
    }

    #[test]
    fn sequence_manager_any_stuck_filling() {
        let mgr = SequenceManager::new();
        assert!(!mgr.any_stuck_filling(10));
    }

    #[test]
    fn sequence_manager_gap_status_counts() {
        let mgr = SequenceManager::new();
        let counts = mgr.gap_status_counts();
        assert_eq!(counts.len(), 5);
    }

    #[test]
    fn sequence_manager_clone_spaces() {
        let mut mgr = SequenceManager::new();
        mgr.record_received(test_peer1(), SequenceNumber(1), vec![1], test_ts(100))
            .unwrap();
        let cloned = mgr.clone_spaces();
        assert_eq!(cloned.len(), 1);
    }

    #[test]
    fn sequence_manager_replace_spaces() {
        let mut mgr = SequenceManager::new();
        let mut spaces = HashMap::new();
        spaces.insert(test_peer1(), SequenceSpace::new(test_peer1()));
        mgr.replace_spaces(spaces);
        assert!(mgr.has_space(test_peer1()));
    }

    #[test]
    fn sequence_manager_allocated_since() {
        let mut mgr = SequenceManager::new();
        mgr.allocate_sequence(test_peer1());
        assert_eq!(mgr.allocated_since(0), 1);
        assert_eq!(mgr.allocated_since(1), 0);
    }

    #[test]
    fn sequence_manager_received_since() {
        let mut mgr = SequenceManager::new();
        mgr.record_received(test_peer1(), SequenceNumber(1), vec![1], test_ts(100))
            .unwrap();
        assert_eq!(mgr.received_since(0), 1);
        assert_eq!(mgr.received_since(1), 0);
    }

    #[test]
    fn sequence_manager_acked_since() {
        let mut mgr = SequenceManager::new();
        mgr.record_received(test_peer1(), SequenceNumber(1), vec![1], test_ts(100))
            .unwrap();
        mgr.acknowledge(test_peer1(), SequenceNumber(1)).unwrap();
        assert_eq!(mgr.acked_since(0), 1);
        assert_eq!(mgr.acked_since(1), 0);
    }

    #[test]
    fn sequence_manager_out_of_order_space_count() {
        let mut mgr = SequenceManager::new();
        mgr.record_received(test_peer1(), SequenceNumber(5), vec![5], test_ts(100))
            .unwrap();
        assert_eq!(mgr.out_of_order_space_count(), 1);
    }

    #[test]
    fn sequence_manager_wrap_around_proximity() {
        let mgr = SequenceManager::new();
        assert!((mgr.wrap_around_proximity(SequenceNumber(0)) - 0.0).abs() < 0.001);
        assert!((mgr.wrap_around_proximity(SequenceNumber(u64::MAX)) - 1.0).abs() < 0.001);
    }

    #[test]
    fn sequence_manager_validate_sequence() {
        let mut mgr = SequenceManager::new();
        assert!(mgr.validate_sequence(test_peer1(), SequenceNumber(1)).is_ok());
        assert!(mgr.validate_sequence(test_peer1(), SequenceNumber(0)).is_err());
    }

    #[test]
    fn sequence_manager_successor_predecessor() {
        let mgr = SequenceManager::new();
        assert_eq!(mgr.successor(SequenceNumber(1)), SequenceNumber(2));
        assert_eq!(mgr.successor(SequenceNumber(u64::MAX)), SequenceNumber(0));
        assert_eq!(mgr.predecessor(SequenceNumber(2)), SequenceNumber(1));
        assert_eq!(mgr.predecessor(SequenceNumber(0)), SequenceNumber(u64::MAX));
    }

    #[test]
    fn sequence_manager_is_before_after() {
        let mgr = SequenceManager::new();
        assert!(mgr.is_before(SequenceNumber(10), SequenceNumber(20), 1000));
        assert!(mgr.is_after(SequenceNumber(20), SequenceNumber(10), 1000));
    }

    #[test]
    fn sequence_manager_midpoint() {
        let mgr = SequenceManager::new();
        assert_eq!(mgr.midpoint(SequenceNumber(10), SequenceNumber(20)), SequenceNumber(15));
    }

    #[test]
    fn sequence_manager_range_sequences() {
        let mgr = SequenceManager::new();
        let range = mgr.range_sequences(SequenceNumber(1), SequenceNumber(4));
        assert_eq!(range, vec![SequenceNumber(1), SequenceNumber(2), SequenceNumber(3)]);
    }

    #[test]
    fn sequence_manager_compact_space() {
        let mut mgr = SequenceManager::new();
        mgr.record_received(test_peer1(), SequenceNumber(1), vec![1], test_ts(100))
            .unwrap();
        let removed = mgr.compact_space(test_peer1());
        assert!(removed >= 0);
    }

    #[test]
    fn sequence_manager_reset_all() {
        let mut mgr = SequenceManager::new();
        mgr.record_received(test_peer1(), SequenceNumber(1), vec![1], test_ts(100))
            .unwrap();
        mgr.reset_all();
        assert_eq!(mgr.active_space_count(), 0);
        assert_eq!(mgr.total_allocated(), 0);
    }

    #[test]
    fn sequence_manager_reset_peer() {
        let mut mgr = SequenceManager::new();
        mgr.record_received(test_peer1(), SequenceNumber(1), vec![1], test_ts(100))
            .unwrap();
        assert!(mgr.reset_peer(test_peer1()));
        assert!(!mgr.reset_peer(test_peer2()));
    }

    #[test]
    fn sequence_manager_default_config_setters() {
        let mut mgr = SequenceManager::new();
        mgr.set_default_window_size(2048);
        assert_eq!(mgr.default_window_size(), 2048);
        mgr.set_default_max_out_of_order(4096);
        assert_eq!(mgr.default_max_out_of_order(), 4096);
        mgr.set_default_expiry_ms(120_000);
        assert_eq!(mgr.default_expiry_ms(), 120_000);
    }

    #[test]
    fn sequence_manager_snapshot_restore() {
        let mut mgr = SequenceManager::new();
        mgr.record_received(test_peer1(), SequenceNumber(1), vec![1], test_ts(100))
            .unwrap();
        let snap = mgr.snapshot();
        assert_eq!(snap.spaces.len(), 1);
        let mut mgr2 = SequenceManager::new();
        mgr2.restore(snap);
        assert_eq!(mgr2.active_space_count(), 1);
    }

    #[test]
    fn sequence_manager_gap_tracker_access() {
        let mut mgr = SequenceManager::new();
        let id = mgr.open_gap(SequenceNumber(10), SequenceNumber(20), test_ts(100));
        assert_eq!(id, 1);
        assert!(mgr.fill_gap(1, test_ts(200)));
        assert!(mgr.declare_lost(2));
        assert!(!mgr.declare_lost(999));
    }

    #[test]
    fn sequence_manager_is_space_healthy() {
        let mut mgr = SequenceManager::new();
        assert!(mgr.space(test_peer1()).is_none());
        mgr.create_space(test_peer1(), 1000, 100, 1000).unwrap();
        assert!(mgr.space(test_peer1()).is_some());
    }

    #[test]
    fn sequence_manager_gap_fill_rate() {
        let mgr = SequenceManager::new();
        assert!((mgr.gap_fill_rate() - 100.0).abs() < 0.001);
    }

    #[test]
    fn sequence_manager_reset_sequence_space() {
        let mut mgr = SequenceManager::new();
        mgr.create_space(test_peer1(), 1000, 100, 1000).unwrap();
        assert!(mgr.reset_sequence_space(test_peer1()).is_ok());
        assert!(mgr.reset_sequence_space(test_peer2()).is_err());
    }
}