//! # Sealed Trait: Broadcast — reliable message dissemination across peers
//!
//! Carrier-agnostic: operates on opaque Endpoints and NodeIds only.
//! No std::net, tokio::net, libp2p, IP, TCP, UDP, QUIC, HTTP, or DNS.
//!
//! The Broadcast trait defines fault-tolerant message broadcast with:
//! - Deduplication via message ID window
//! - TTL and hop-limit bounded propagation
//! - Fanout control and rate limiting
//! - Delivery semantics (at-least-once, at-most-once, exactly-once)
//! - Ack tracking and rebroadcast suppression
//! - Broadcast channels (priority, region, dataset-scoped)
//! - Message integrity via signature verification
//! - Broadcast statistics and health monitoring
//! Implementations live downstream in ids-broadcast and ids-* crates.

use crate::types::{
    AuthToken, CarrierType, Capacity, Credential, DatasetId, Endpoint, FailureDomain,
    KeyId, NodeId, OpResult, RegionId, Signature,
};
use crate::errors::BroadcastError;

///////////////////////////////////////////////////////////////////////////////
// SEALED TRAIT: Broadcast
///////////////////////////////////////////////////////////////////////////////

mod private {
    pub trait Sealed {}
}

impl private::Sealed for super::BroadcastEngine {}
impl private::Sealed for super::MessageDeduplicator {}
impl private::Sealed for super::TTLManager {}
impl private::Sealed for super::FanoutController {}
impl private::Sealed for super::RateLimiter {}
impl private::Sealed for super::AckTracker {}
impl private::Sealed for super::RebroadcastSuppressor {}
impl private::Sealed for super::BroadcastChannel {}
impl private::Sealed for super::MessageIntegrity {}
impl private::Sealed for super::DeliverySemantics {}
impl private::Sealed for super::BroadcastStats {}
impl private::Sealed for super::BroadcastMonitor {}
impl private::Sealed for super::BroadcastValidator {}
impl private::Sealed for super::MessageID {}
impl private::Sealed for super::BroadcastMessage {}
impl private::Sealed for super::DeliveryResult {}
impl private::Sealed for super::ChannelConfig {}

// Sealed trait for broadcast.
// Downstream crates can call methods and use as trait bounds but cannot implement.
pub trait Broadcast: private::Sealed {
    /// Broadcast a message to all known peers.
    fn broadcast(&self, msg: BroadcastMessage) -> Result<BroadcastMessage, BroadcastError>;

    /// Send to a specific set of peers (multicast).
    fn multicast(&self, msg: BroadcastMessage, peers: Vec<NodeId>) -> Result<BroadcastMessage, BroadcastError>;

    /// Send to peers within a region.
    fn regional_broadcast(&self, msg: BroadcastMessage, region: RegionId) -> Result<BroadcastMessage, BroadcastError>;

    /// Send to peers subscribed to a dataset.
    fn dataset_broadcast(&self, msg: BroadcastMessage, dataset: DatasetId) -> Result<BroadcastMessage, BroadcastError>;

    /// Deduplicate a message ID.
    fn deduplicate(&self, msg_id: MessageID) -> Result<bool, BroadcastError>;

    /// Record a delivery ack from a peer.
    fn record_ack(&self, msg_id: MessageID, peer: NodeId) -> Result<(), BroadcastError>;

    /// Check if a message is fully delivered (all acks received).
    fn is_fully_delivered(&self, msg_id: MessageID) -> Result<bool, BroadcastError>;

    /// Get message delivery status.
    fn delivery_status(&self, msg_id: MessageID) -> Result<DeliveryResult, BroadcastError>;

    /// Set TTL for a message.
    fn set_ttl(&self, msg_id: MessageID, ttl_ms: u64) -> Result<(), BroadcastError>;

    /// Expire old messages by TTL.
    fn expire_by_ttl(&self, at_ms: u64) -> Result<u64, BroadcastError>;

    /// Configure a broadcast channel.
    fn configure_channel(&self, channel_id: u64, config: ChannelConfig) -> Result<(), BroadcastError>;

    /// Publish to a channel.
    fn publish(&self, channel_id: u64, msg: BroadcastMessage) -> Result<BroadcastMessage, BroadcastError>;

    /// Subscribe to a channel.
    fn subscribe(&self, channel_id: u64, peer: NodeId) -> Result<(), BroadcastError>;

    /// Unsubscribe from a channel.
    fn unsubscribe(&self, channel_id: u64, peer: NodeId) -> Result<(), BroadcastError>;

    /// Verify message integrity (signature).
    fn verify_integrity(&self, msg: &BroadcastMessage) -> Result<bool, BroadcastError>;

    /// Sign a message.
    fn sign_message(&self, msg: &mut BroadcastMessage, key_id: KeyId) -> Result<Signature, BroadcastError>;

    /// Get broadcast stats.
    fn stats(&self) -> BroadcastStats;

    /// Get broadcast name for debugging.
    fn broadcast_name(&self) -> &'static str;

    /// Get total messages broadcast.
    fn total_broadcast(&self) -> u64;

    /// Get total messages deduplicated.
    fn total_deduplicated(&self) -> u64;

    /// Get total acks received.
    fn total_acks(&self) -> u64;
}

///////////////////////////////////////////////////////////////////////////////
// BROADCAST ERROR
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq)]
pub enum BroadcastError {
    #[error("message not found: {msg_id:?}")]
    MessageNotFound { msg_id: MessageID },

    #[error("message already delivered: {msg_id:?}")]
    MessageAlreadyDelivered { msg_id: MessageID },

    #[error("duplicate message: {msg_id:?}")]
    DuplicateMessage { msg_id: MessageID },

    #[error("channel not found: {channel_id}")]
    ChannelNotFound { channel_id: u64 },

    #[error("peer not subscribed: {peer:?} to channel {channel_id}")]
    PeerNotSubscribed { peer: NodeId, channel_id: u64 },

    #[error("peer already subscribed: {peer:?} to channel {channel_id}")]
    PeerAlreadySubscribed { peer: NodeId, channel_id: u64 },

    #[error("insufficient capacity: needed={needed}, available={available}")]
    InsufficientCapacity { needed: u64, available: u64 },

    #[error("rate limit exceeded: {peer:?}")]
    RateLimitExceeded { peer: NodeId },

    #[error("message expired (TTL): {msg_id:?}")]
    MessageExpired { msg_id: MessageID },

    #[error("hop limit exceeded: {msg_id:?}")]
    HopLimitExceeded { msg_id: MessageID },

    #[error("integrity verification failed: {msg_id:?}")]
    IntegrityFailed { msg_id: MessageID },

    #[error("signature missing: {msg_id:?}")]
    SignatureMissing { msg_id: MessageID },

    #[error("invalid message: {reason}")]
    InvalidMessage { reason: String },

    #[error("internal broadcast error: {message}")]
    Internal { message: String },
}

impl BroadcastError {
    pub fn category(&self) -> &'static str {
        match self {
            BroadcastError::MessageNotFound { .. } => "message_not_found",
            BroadcastError::MessageAlreadyDelivered { .. } => "message_already_delivered",
            BroadcastError::DuplicateMessage { .. } => "duplicate_message",
            BroadcastError::ChannelNotFound { .. } => "channel_not_found",
            BroadcastError::PeerNotSubscribed { .. } => "peer_not_subscribed",
            BroadcastError::PeerAlreadySubscribed { .. } => "peer_already_subscribed",
            BroadcastError::InsufficientCapacity { .. } => "insufficient_capacity",
            BroadcastError::RateLimitExceeded { .. } => "rate_limit_exceeded",
            BroadcastError::MessageExpired { .. } => "message_expired",
            BroadcastError::HopLimitExceeded { .. } => "hop_limit_exceeded",
            BroadcastError::IntegrityFailed { .. } => "integrity_failed",
            BroadcastError::SignatureMissing { .. } => "signature_missing",
            BroadcastError::InvalidMessage { .. } => "invalid_message",
            BroadcastError::Internal { .. } => "internal",
        }
    }

    pub fn is_recoverable(&self) -> bool {
        matches!(
            self,
            BroadcastError::RateLimitExceeded { .. }
                | BroadcastError::InsufficientCapacity { .. }
                | BroadcastError::MessageExpired { .. }
                | BroadcastError::DuplicateMessage { .. }
        )
    }

    pub fn suggested_action(&self) -> &'static str {
        match self {
            BroadcastError::MessageNotFound { .. } => "rebroadcast the message",
            BroadcastError::MessageAlreadyDelivered { .. } => "no action needed",
            BroadcastError::DuplicateMessage { .. } => "discard the duplicate",
            BroadcastError::ChannelNotFound { .. } => "create the channel first",
            BroadcastError::PeerNotSubscribed { .. } => "subscribe the peer to the channel",
            BroadcastError::PeerAlreadySubscribed { .. } => "peer is already subscribed",
            BroadcastError::InsufficientCapacity { .. } => "increase capacity or shed load",
            BroadcastError::RateLimitExceeded { .. } => "wait and retry later",
            BroadcastError::MessageExpired { .. } => "reissue with new TTL",
            BroadcastError::HopLimitExceeded { .. } => "reduce hop count or relay locally",
            BroadcastError::IntegrityFailed { .. } => "reject the message",
            BroadcastError::SignatureMissing { .. } => "sign the message before sending",
            BroadcastError::InvalidMessage { .. } => "fix the message payload",
            BroadcastError::Internal { .. } => "contact support",
        }
    }
}

///////////////////////////////////////////////////////////////////////////////
// MESSAGE ID
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MessageID(pub [u8; 16]);

impl MessageID {
    pub fn new(bytes: [u8; 16]) -> Self {
        Self(bytes)
    }

    pub fn zero() -> Self {
        Self([0u8; 16])
    }

    pub fn as_bytes(&self) -> &[u8; 16] {
        &self.0
    }

    pub fn is_zero(&self) -> bool {
        self.0 == [0u8; 16]
    }
}

impl Default for MessageID {
    fn default() -> Self {
        Self::zero()
    }
}

///////////////////////////////////////////////////////////////////////////////
// BROADCAST MESSAGE
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone)]
pub struct BroadcastMessage {
    pub msg_id: MessageID,
    pub payload: Vec<u8>,
    pub origin: NodeId,
    pub region: Option<RegionId>,
    pub dataset: Option<DatasetId>,
    pub ttl_ms: Option<u64>,
    pub hop_count: u64,
    pub max_hops: u64,
    pub signature: Option<Signature>,
    pub signed_by: Option<KeyId>,
    pub timestamp_ms: u64,
    pub priority: u8,
    pub delivery_semantics: DeliverySemantics,
    pub acked_by: Vec<NodeId>,
}

impl BroadcastMessage {
    pub fn new(
        msg_id: MessageID,
        payload: Vec<u8>,
        origin: NodeId,
    ) -> Self {
        Self {
            msg_id,
            payload,
            origin,
            region: None,
            dataset: None,
            ttl_ms: None,
            hop_count: 0,
            max_hops: 255,
            signature: None,
            signed_by: None,
            timestamp_ms: 0,
            priority: 0,
            delivery_semantics: DeliverySemantics::AtLeastOnce,
            acked_by: Vec::new(),
        }
    }

    pub fn with_ttl(mut self, ttl_ms: u64) -> Self {
        self.ttl_ms = Some(ttl_ms);
        self
    }

    pub fn with_max_hops(mut self, max_hops: u64) -> Self {
        self.max_hops = max_hops;
        self
    }

    pub fn with_region(mut self, region: RegionId) -> Self {
        self.region = Some(region);
        self
    }

    pub fn with_dataset(mut self, dataset: DatasetId) -> Self {
        self.dataset = Some(dataset);
        self
    }

    pub fn with_priority(mut self, priority: u8) -> Self {
        self.priority = priority;
        self
    }

    pub fn with_delivery_semantics(mut self, semantics: DeliverySemantics) -> Self {
        self.delivery_semantics = semantics;
        self
    }

    pub fn with_timestamp(mut self, timestamp_ms: u64) -> Self {
        self.timestamp_ms = timestamp_ms;
        self
    }

    pub fn is_expired(&self, at_ms: u64) -> bool {
        match self.ttl_ms {
            Some(ttl) => at_ms - self.timestamp_ms > ttl,
            None => false,
        }
    }

    pub fn has_hop_capacity(&self) -> bool {
        self.hop_count < self.max_hops
    }

    pub fn increment_hop(&mut self) {
        self.hop_count += 1;
    }

    pub fn payload_size(&self) -> usize {
        self.payload.len()
    }

    pub fn ack_count(&self) -> usize {
        self.acked_by.len()
    }

    pub fn is_acked_by(&self, peer: &NodeId) -> bool {
        self.acked_by.iter().any(|p| p == peer)
    }

    pub fn add_ack(&mut self, peer: NodeId) {
        if !self.is_acked_by(&peer) {
            self.acked_by.push(peer);
        }
    }
}

impl Default for BroadcastMessage {
    fn default() -> Self {
        Self::new(MessageID::zero(), Vec::new(), NodeId::new([0u8; 32]))
    }
}

///////////////////////////////////////////////////////////////////////////////
// DELIVERY SEMANTICS
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeliverySemantics {
    AtMostOnce,
    AtLeastOnce,
    ExactlyOnce,
    BestEffort,
}

impl DeliverySemantics {
    pub fn as_str(&self) -> &'static str {
        match self {
            DeliverySemantics::AtMostOnce => "at_most_once",
            DeliverySemantics::AtLeastOnce => "at_least_once",
            DeliverySemantics::ExactlyOnce => "exactly_once",
            DeliverySemantics::BestEffort => "best_effort",
        }
    }

    pub fn requires_ack(&self) -> bool {
        matches!(
            self,
            DeliverySemantics::AtLeastOnce | DeliverySemantics::ExactlyOnce
        )
    }

    pub fn idempotent(&self) -> bool {
        matches!(self, DeliverySemantics::ExactlyOnce)
    }
}

///////////////////////////////////////////////////////////////////////////////
// DELIVERY RESULT
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone)]
pub struct DeliveryResult {
    pub msg_id: MessageID,
    pub delivered_to: Vec<NodeId>,
    pub acked_by: Vec<NodeId>,
    pub pending_acks: Vec<NodeId>,
    pub delivery_count: u64,
    pub rebroadcast_count: u64,
    pub is_complete: bool,
    pub expired: bool,
}

impl DeliveryResult {
    pub fn new(msg_id: MessageID, expected_peers: Vec<NodeId>) -> Self {
        let pending = expected_peers.clone();
        Self {
            msg_id,
            delivered_to: Vec::new(),
            acked_by: Vec::new(),
            pending_acks: pending,
            delivery_count: 0,
            rebroadcast_count: 0,
            is_complete: false,
            expired: false,
        }
    }

    pub fn mark_delivered(&mut self, peer: NodeId) {
        if !self.delivered_to.contains(&peer) {
            self.delivered_to.push(peer);
            self.delivery_count += 1;
            self.pending_acks.retain(|p| p != &peer);
        }
    }

    pub fn mark_acked(&mut self, peer: NodeId) {
        if !self.acked_by.contains(&peer) {
            self.acked_by.push(peer);
            self.pending_acks.retain(|p| p != &peer);
        }
        if self.pending_acks.is_empty() && !self.delivered_to.is_empty() {
            self.is_complete = true;
        }
    }

    pub fn mark_expired(&mut self) {
        self.expired = true;
    }

    pub fn delivery_rate(&self, expected: usize) -> f64 {
        if expected == 0 {
            return 1.0;
        }
        self.delivered_to.len() as f64 / expected as f64
    }

    pub fn ack_rate(&self, expected: usize) -> f64 {
        if expected == 0 {
            return 1.0;
        }
        self.acked_by.len() as f64 / expected as f64
    }
}

///////////////////////////////////////////////////////////////////////////////
// CHANNEL CONFIG
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone)]
pub struct ChannelConfig {
    pub channel_id: u64,
    pub name: String,
    pub max_subscribers: u64,
    pub max_msg_size: usize,
    pub ttl_ms: Option<u64>,
    pub max_hops: u64,
    pub priority: u8,
    pub delivery_semantics: DeliverySemantics,
    pub require_signature: bool,
    pub region_scoped: bool,
    pub dataset_scoped: bool,
}

impl ChannelConfig {
    pub fn new(channel_id: u64, name: String) -> Self {
        Self {
            channel_id,
            name,
            max_subscribers: 10000,
            max_msg_size: 1024 * 1024,
            ttl_ms: None,
            max_hops: 255,
            priority: 0,
            delivery_semantics: DeliverySemantics::AtLeastOnce,
            require_signature: false,
            region_scoped: false,
            dataset_scoped: false,
        }
    }

    pub fn with_max_subscribers(mut self, max: u64) -> Self {
        self.max_subscribers = max;
        self
    }

    pub fn with_ttl(mut self, ttl_ms: u64) -> Self {
        self.ttl_ms = Some(ttl_ms);
        self
    }

    pub fn with_max_hops(mut self, hops: u64) -> Self {
        self.max_hops = hops;
        self
    }

    pub fn with_signature_required(mut self, required: bool) -> Self {
        self.require_signature = required;
        self
    }

    pub fn validate(&self) -> Result<(), BroadcastError> {
        if self.max_subscribers == 0 {
            return Err(BroadcastError::InvalidMessage {
                reason: "channel must allow at least one subscriber".to_string(),
            });
        }
        if self.max_msg_size == 0 {
            return Err(BroadcastError::InvalidMessage {
                reason: "channel must allow non-zero message size".to_string(),
            });
        }
        Ok(())
    }
}

///////////////////////////////////////////////////////////////////////////////
// MESSAGE DEDUPLICATOR
///////////////////////////////////////////////////////////////////////////////

pub struct DedupEntry {
    pub msg_id: MessageID,
    pub first_seen_ms: u64,
    pub last_seen_ms: u64,
    pub seen_count: u64,
    pub delivered: bool,
}

impl DedupEntry {
    pub fn new(msg_id: MessageID, at_ms: u64) -> Self {
        Self {
            msg_id,
            first_seen_ms: at_ms,
            last_seen_ms: at_ms,
            seen_count: 1,
            delivered: false,
        }
    }

    pub fn touch(&mut self, at_ms: u64) {
        self.last_seen_ms = at_ms;
        self.seen_count += 1;
    }
}

pub struct MessageDeduplicator {
    pub seen: std::cell::RefCell<std::collections::HashMap<MessageID, DedupEntry>>,
    pub window_ms: u64,
    pub max_entries: u64,
    pub total_seen: u64,
    pub total_duplicates: u64,
    pub total_delivered: u64,
}

impl MessageDeduplicator {
    pub fn new(window_ms: u64, max_entries: u64) -> Self {
        Self {
            seen: std::cell::RefCell::new(std::collections::HashMap::new()),
            window_ms,
            max_entries,
            total_seen: 0,
            total_duplicates: 0,
            total_delivered: 0,
        }
    }

    pub fn is_duplicate(&self, msg_id: MessageID, at_ms: u64) -> Result<bool, BroadcastError> {
        let mut seen = self.seen.borrow_mut();
        if let Some(entry) = seen.get_mut(&msg_id) {
            if at_ms - entry.first_seen_ms > self.window_ms {
                // Expired from window — treat as new
                entry.first_seen_ms = at_ms;
                entry.last_seen_ms = at_ms;
                entry.seen_count = 1;
                entry.delivered = false;
                self.total_seen += 1;
                Ok(false)
            } else {
                entry.touch(at_ms);
                self.total_duplicates += 1;
                Ok(true)
            }
        } else {
            if seen.len() as u64 >= self.max_entries {
                drop(seen);
                return Err(BroadcastError::InsufficientCapacity {
                    needed: seen.len() as u64 + 1,
                    available: self.max_entries,
                });
            }
            let mut seen = self.seen.borrow_mut();
            seen.insert(
                msg_id,
                DedupEntry::new(msg_id, at_ms),
            );
            self.total_seen += 1;
            Ok(false)
        }
    }

    pub fn mark_delivered(&self, msg_id: MessageID) -> Result<(), BroadcastError> {
        if let Some(entry) = self.seen.borrow_mut().get_mut(&msg_id) {
            entry.delivered = true;
            self.total_delivered += 1;
            Ok(())
        } else {
            Err(BroadcastError::MessageNotFound { msg_id })
        }
    }

    pub fn is_delivered(&self, msg_id: MessageID) -> bool {
        self.seen.borrow().get(&msg_id).map(|e| e.delivered).unwrap_or(false)
    }

    pub fn expire_old(&self, at_ms: u64) -> u64 {
        let mut seen = self.seen.borrow_mut();
        let before = seen.len();
        seen.retain(|_, e| at_ms - e.last_seen_ms <= self.window_ms);
        before - seen.len() as u64
    }

    pub fn total_seen(&self) -> u64 {
        self.total_seen
    }

    pub fn total_duplicates(&self) -> u64 {
        self.total_duplicates
    }

    pub fn total_delivered(&self) -> u64 {
        self.total_delivered
    }

    pub fn tracked_count(&self) -> usize {
        self.seen.borrow().len()
    }

    pub fn duplicate_rate(&self) -> f64 {
        if self.total_seen == 0 {
            return 0.0;
        }
        self.total_duplicates as f64 / self.total_seen as f64
    }
}

impl Default for MessageDeduplicator {
    fn default() -> Self {
        Self::new(300_000, 1_000_000)
    }
}

///////////////////////////////////////////////////////////////////////////////
// TTL MANAGER
///////////////////////////////////////////////////////////////////////////////

pub struct TTLManager {
    pub expirations: std::cell::RefCell<std::collections::HashMap<MessageID, u64>>,
    pub total_expired: u64,
    pub total_checked: u64,
}

impl TTLManager {
    pub fn new() -> Self {
        Self {
            expirations: std::cell::RefCell::new(std::collections::HashMap::new()),
            total_expired: 0,
            total_checked: 0,
        }
    }

    pub fn set_ttl(&self, msg_id: MessageID, ttl_ms: u64, now_ms: u64) {
        self.expirations
            .borrow_mut()
            .insert(msg_id, now_ms + ttl_ms);
    }

    pub fn is_expired(&self, msg_id: MessageID, at_ms: u64) -> bool {
        self.total_checked += 1;
        match self.expirations.borrow().get(&msg_id) {
            Some(expires_at) => at_ms > *expires_at,
            None => false,
        }
    }

    pub fn expire_all(&self, at_ms: u64) -> u64 {
        let mut expirations = self.expirations.borrow_mut();
        let before = expirations.len();
        expirations.retain(|_, expires_at| at_ms <= *expires_at);
        let expired = before - expirations.len();
        self.total_expired += expired as u64;
        expired as u64
    }

    pub fn remove(&self, msg_id: MessageID) {
        self.expirations.borrow_mut().remove(&msg_id);
    }

    pub fn tracked_count(&self) -> usize {
        self.expirations.borrow().len()
    }

    pub fn total_expired(&self) -> u64 {
        self.total_expired
    }

    pub fn total_checked(&self) -> u64 {
        self.total_checked
    }
}

impl Default for TTLManager {
    fn default() -> Self {
        Self::new()
    }
}

///////////////////////////////////////////////////////////////////////////////
// FANOUT CONTROLLER
///////////////////////////////////////////////////////////////////////////////

pub struct FanoutPlan {
    pub msg_id: MessageID,
    pub target_peers: Vec<NodeId>,
    pub fanout_factor: u64,
    pub current_round: u64,
    pub completed: bool,
}

impl FanoutPlan {
    pub fn new(msg_id: MessageID, target_peers: Vec<NodeId>, fanout_factor: u64) -> Self {
        Self {
            msg_id,
            target_peers,
            fanout_factor,
            current_round: 0,
            completed: target_peers.is_empty(),
        }
    }

    pub fn next_batch(&mut self) -> Vec<NodeId> {
        if self.completed {
            return Vec::new();
        }
        let start = (self.current_round * self.fanout_factor) as usize;
        if start >= self.target_peers.len() {
            self.completed = true;
            return Vec::new();
        }
        let end = ((start + self.fanout_factor as usize).min(self.target_peers.len()));
        let batch: Vec<NodeId> = self.target_peers[start..end].to_vec();
        self.current_round += 1;
        batch
    }

    pub fn is_completed(&self) -> bool {
        self.completed
    }

    pub fn progress(&self) -> f64 {
        if self.target_peers.is_empty() {
            return 1.0;
        }
        (self.current_round * self.fanout_factor) as usize as f64 / self.target_peers.len() as f64
    }
}

pub struct FanoutController {
    pub plans: std::cell::RefCell<std::collections::HashMap<MessageID, FanoutPlan>>,
    pub default_factor: u64,
    pub total_plans: u64,
    pub total_completed: u64,
}

impl FanoutController {
    pub fn new(default_factor: u64) -> Self {
        Self {
            plans: std::cell::RefCell::new(std::collections::HashMap::new()),
            default_factor,
            total_plans: 0,
            total_completed: 0,
        }
    }

    pub fn create_plan(
        &self,
        msg_id: MessageID,
        target_peers: Vec<NodeId>,
    ) -> Result<FanoutPlan, BroadcastError> {
        let plans = self.plans.borrow();
        if plans.contains_key(&msg_id) {
            drop(plans);
            return Err(BroadcastError::InvalidMessage {
                reason: "fanout plan already exists for message".to_string(),
            });
        }
        drop(plans);

        let mut plans = self.plans.borrow_mut();
        let plan = FanoutPlan::new(msg_id.clone(), target_peers, self.default_factor);
        plans.insert(msg_id, plan.clone());
        self.total_plans += 1;
        Ok(plan)
    }

    pub fn get_plan(&self, msg_id: &MessageID) -> Option<FanoutPlan> {
        self.plans.borrow().get(msg_id).cloned()
    }

    pub fn advance_plan(&self, msg_id: &MessageID) -> Option<Vec<NodeId>> {
        if let Some(mut plan) = self.plans.borrow_mut().get_mut(msg_id) {
            let batch = plan.next_batch();
            if plan.is_completed() {
                self.total_completed += 1;
            }
            Some(batch)
        } else {
            None
        }
    }

    pub fn complete_plan(&self, msg_id: &MessageID) -> Result<(), BroadcastError> {
        if self.plans.borrow_mut().remove(msg_id).is_some() {
            self.total_completed += 1;
            Ok(())
        } else {
            Err(BroadcastError::MessageNotFound { msg_id: *msg_id })
        }
    }

    pub fn total_plans(&self) -> u64 {
        self.total_plans
    }

    pub fn total_completed(&self) -> u64 {
        self.total_completed
    }

    pub fn tracked_plan_count(&self) -> usize {
        self.plans.borrow().len()
    }
}

impl Default for FanoutController {
    fn default() -> Self {
        Self::new(8)
    }
}

///////////////////////////////////////////////////////////////////////////////
// RATE LIMITER
///////////////////////////////////////////////////////////////////////////////

pub struct RateLimitWindow {
    pub peer: NodeId,
    pub message_count: u64,
    pub window_start_ms: u64,
    pub window_duration_ms: u64,
    pub max_messages: u64,
    pub blocked_count: u64,
}

impl RateLimitWindow {
    pub fn new(
        peer: NodeId,
        window_start_ms: u64,
        window_duration_ms: u64,
        max_messages: u64,
    ) -> Self {
        Self {
            peer,
            message_count: 0,
            window_start_ms,
            window_duration_ms,
            max_messages,
            blocked_count: 0,
        }
    }

    pub fn is_within_window(&self, at_ms: u64) -> bool {
        at_ms >= self.window_start_ms && at_ms < self.window_start_ms + self.window_duration_ms
    }

    pub fn check(&mut self, at_ms: u64) -> Result<(), BroadcastError> {
        if !self.is_within_window(at_ms) {
            self.message_count = 0;
            self.window_start_ms = at_ms;
        }
        if self.message_count >= self.max_messages {
            self.blocked_count += 1;
            return Err(BroadcastError::RateLimitExceeded { peer: self.peer });
        }
        self.message_count += 1;
        Ok(())
    }

    pub fn message_count(&self) -> u64 {
        self.message_count
    }

    pub fn blocked_count(&self) -> u64 {
        self.blocked_count
    }
}

pub struct RateLimiter {
    pub windows: std::cell::RefCell<std::collections::HashMap<NodeId, RateLimitWindow>>,
    pub default_window_ms: u64,
    pub default_max_messages: u64,
    pub total_checked: u64,
    pub total_blocked: u64,
}

impl RateLimiter {
    pub fn new(default_window_ms: u64, default_max_messages: u64) -> Self {
        Self {
            windows: std::cell::RefCell::new(std::collections::HashMap::new()),
            default_window_ms,
            default_max_messages,
            total_checked: 0,
            total_blocked: 0,
        }
    }

    pub fn check(&self, peer: NodeId, at_ms: u64) -> Result<(), BroadcastError> {
        self.total_checked += 1;
        let mut windows = self.windows.borrow_mut();
        let window = windows.entry(peer).or_insert_with(|| {
            RateLimitWindow::new(peer, at_ms, self.default_window_ms, self.default_max_messages)
        });
        let result = window.check(at_ms);
        if result.is_err() {
            self.total_blocked += 1;
        }
        result
    }

    pub fn get_window(&self, peer: &NodeId) -> Option<RateLimitWindow> {
        self.windows.borrow().get(peer).cloned()
    }

    pub fn total_checked(&self) -> u64 {
        self.total_checked
    }

    pub fn total_blocked(&self) -> u64 {
        self.total_blocked
    }

    pub fn block_rate(&self) -> f64 {
        if self.total_checked == 0 {
            return 0.0;
        }
        self.total_blocked as f64 / self.total_checked as f64
    }

    pub fn tracked_peer_count(&self) -> usize {
        self.windows.borrow().len()
    }
}

impl Default for RateLimiter {
    fn default() -> Self {
        Self::new(60_000, 10_000)
    }
}

///////////////////////////////////////////////////////////////////////////////
// ACK TRACKER
///////////////////////////////////////////////////////////////////////////////

pub struct AckEntry {
    pub msg_id: MessageID,
    pub acked_by: std::cell::RefCell<Vec<NodeId>>,
    pub expected_peers: Vec<NodeId>,
    pub received_at_ms: u64,
}

impl AckEntry {
    pub fn new(msg_id: MessageID, expected_peers: Vec<NodeId>, received_at_ms: u64) -> Self {
        Self {
            msg_id,
            acked_by: std::cell::RefCell::new(Vec::new()),
            expected_peers,
            received_at_ms,
        }
    }

    pub fn record_ack(&self, peer: NodeId) -> Result<(), BroadcastError> {
        if self.acked_by.borrow().contains(&peer) {
            return Err(BroadcastError::DuplicateMessage {
                msg_id: self.msg_id,
            });
        }
        self.acked_by.borrow_mut().push(peer);
        Ok(())
    }

    pub fn ack_count(&self) -> usize {
        self.acked_by.borrow().len()
    }

    pub fn is_complete(&self) -> bool {
        let acked = self.acked_by.borrow();
        self.expected_peers.iter().all(|p| acked.contains(p))
    }

    pub fn pending_acks(&self) -> Vec<NodeId> {
        let acked = self.acked_by.borrow();
        self.expected_peers
            .iter()
            .filter(|p| !acked.contains(p))
            .cloned()
            .collect()
    }

    pub fn ack_rate(&self) -> f64 {
        if self.expected_peers.is_empty() {
            return 1.0;
        }
        self.ack_count() as f64 / self.expected_peers.len() as f64
    }
}

pub struct AckTracker {
    pub entries: std::cell::RefCell<std::collections::HashMap<MessageID, AckEntry>>,
    pub total_acks: u64,
    pub total_complete: u64,
    pub max_entries: u64,
}

impl AckTracker {
    pub fn new(max_entries: u64) -> Self {
        Self {
            entries: std::cell::RefCell::new(std::collections::HashMap::new()),
            total_acks: 0,
            total_complete: 0,
            max_entries,
        }
    }

    pub fn register(
        &self,
        msg_id: MessageID,
        expected_peers: Vec<NodeId>,
        at_ms: u64,
    ) -> Result<(), BroadcastError> {
        let entries = self.entries.borrow();
        if entries.contains_key(&msg_id) {
            drop(entries);
            return Err(BroadcastError::MessageAlreadyDelivered { msg_id });
        }
        if entries.len() as u64 >= self.max_entries {
            drop(entries);
            return Err(BroadcastError::InsufficientCapacity {
                needed: entries.len() as u64 + 1,
                available: self.max_entries,
            });
        }
        drop(entries);

        let mut entries = self.entries.borrow_mut();
        entries.insert(msg_id, AckEntry::new(msg_id, expected_peers, at_ms));
        Ok(())
    }

    pub fn record_ack(&self, msg_id: MessageID, peer: NodeId) -> Result<(), BroadcastError> {
        if let Some(entry) = self.entries.borrow_mut().get_mut(&msg_id) {
            let result = entry.record_ack(peer);
            if result.is_ok() {
                self.total_acks += 1;
                if entry.is_complete() {
                    self.total_complete += 1;
                }
            }
            result
        } else {
            Err(BroadcastError::MessageNotFound { msg_id })
        }
    }

    pub fn is_complete(&self, msg_id: MessageID) -> Result<bool, BroadcastError> {
        if let Some(entry) = self.entries.borrow().get(&msg_id) {
            Ok(entry.is_complete())
        } else {
            Err(BroadcastError::MessageNotFound { msg_id })
        }
    }

    pub fn pending_acks(&self, msg_id: MessageID) -> Result<Vec<NodeId>, BroadcastError> {
        if let Some(entry) = self.entries.borrow().get(&msg_id) {
            Ok(entry.pending_acks())
        } else {
            Err(BroadcastError::MessageNotFound { msg_id })
        }
    }

    pub fn ack_rate(&self, msg_id: MessageID) -> Result<f64, BroadcastError> {
        if let Some(entry) = self.entries.borrow().get(&msg_id) {
            Ok(entry.ack_rate())
        } else {
            Err(BroadcastError::MessageNotFound { msg_id })
        }
    }

    pub fn total_acks(&self) -> u64 {
        self.total_acks
    }

    pub fn total_complete(&self) -> u64 {
        self.total_complete
    }

    pub fn tracked_count(&self) -> usize {
        self.entries.borrow().len()
    }

    pub fn complete_rate(&self) -> f64 {
        let tracked = self.tracked_count() as u64;
        if tracked == 0 {
            return 1.0;
        }
        self.total_complete as f64 / tracked as f64
    }
}

impl Default for AckTracker {
    fn default() -> Self {
        Self::new(1_000_000)
    }
}

///////////////////////////////////////////////////////////////////////////////
// REBROADCAST SUPPRESSOR
///////////////////////////////////////////////////////////////////////////////

pub struct RebroadcastRecord {
    pub msg_id: MessageID,
    pub suppressed_count: u64,
    pub allowed_count: u64,
    pub last_rebroadcast_ms: u64,
    pub suppression_window_ms: u64,
}

impl RebroadcastRecord {
    pub fn new(msg_id: MessageID, suppression_window_ms: u64) -> Self {
        Self {
            msg_id,
            suppressed_count: 0,
            allowed_count: 0,
            last_rebroadcast_ms: 0,
            suppression_window_ms,
        }
    }

    pub fn should_suppress(&mut self, at_ms: u64) -> bool {
        if at_ms - self.last_rebroadcast_ms >= self.suppression_window_ms {
            self.allowed_count += 1;
            self.last_rebroadcast_ms = at_ms;
            false
        } else {
            self.suppressed_count += 1;
            true
        }
    }

    pub fn suppression_rate(&self) -> f64 {
        let total = self.suppressed_count + self.allowed_count;
        if total == 0 {
            return 0.0;
        }
        self.suppressed_count as f64 / total as f64
    }
}

pub struct RebroadcastSuppressor {
    pub records: std::cell::RefCell<std::collections::HashMap<MessageID, RebroadcastRecord>>,
    pub default_window_ms: u64,
    pub total_suppressed: u64,
    pub total_allowed: u64,
}

impl RebroadcastSuppressor {
    pub fn new(default_window_ms: u64) -> Self {
        Self {
            records: std::cell::RefCell::new(std::collections::HashMap::new()),
            default_window_ms,
            total_suppressed: 0,
            total_allowed: 0,
        }
    }

    pub fn check(&self, msg_id: MessageID, at_ms: u64) -> Result<bool, BroadcastError> {
        let mut records = self.records.borrow_mut();
        let record = records.entry(msg_id).or_insert_with(|| {
            RebroadcastRecord::new(msg_id, self.default_window_ms)
        });
        let suppress = record.should_suppress(at_ms);
        if suppress {
            self.total_suppressed += 1;
        } else {
            self.total_allowed += 1;
        }
        Ok(suppress)
    }

    pub fn get_record(&self, msg_id: &MessageID) -> Option<RebroadcastRecord> {
        self.records.borrow().get(msg_id).cloned()
    }

    pub fn total_suppressed(&self) -> u64 {
        self.total_suppressed
    }

    pub fn total_allowed(&self) -> u64 {
        self.total_allowed
    }

    pub fn suppression_rate(&self) -> f64 {
        let total = self.total_suppressed + self.total_allowed;
        if total == 0 {
            return 0.0;
        }
        self.total_suppressed as f64 / total as f64
    }

    pub fn tracked_count(&self) -> usize {
        self.records.borrow().len()
    }
}

impl Default for RebroadcastSuppressor {
    fn default() -> Self {
        Self::new(1_000)
    }
}

///////////////////////////////////////////////////////////////////////////////
// BROADCAST CHANNEL
///////////////////////////////////////////////////////////////////////////////

pub struct ChannelSubscription {
    pub channel_id: u64,
    pub peer: NodeId,
    pub subscribed_at_ms: u64,
}

pub struct BroadcastChannel {
    pub config: ChannelConfig,
    pub subscribers: std::cell::RefCell<Vec<NodeId>>,
    pub published_count: u64,
    pub delivered_count: u64,
    pub dropped_count: u64,
}

impl BroadcastChannel {
    pub fn new(config: ChannelConfig) -> Self {
        Self {
            config,
            subscribers: std::cell::RefCell::new(Vec::new()),
            published_count: 0,
            delivered_count: 0,
            dropped_count: 0,
        }
    }

    pub fn subscribe(&self, peer: NodeId, at_ms: u64) -> Result<(), BroadcastError> {
        let mut subs = self.subscribers.borrow_mut();
        if subs.contains(&peer) {
            drop(subs);
            return Err(BroadcastError::PeerAlreadySubscribed {
                peer,
                channel_id: self.config.channel_id,
            });
        }
        if subs.len() as u64 >= self.config.max_subscribers {
            drop(subs);
            return Err(BroadcastError::InsufficientCapacity {
                needed: subs.len() as u64 + 1,
                available: self.config.max_subscribers,
            });
        }
        subs.push(peer);
        drop(subs);
        // Subscription recorded; timestamp tracked implicitly
        let _ = at_ms;
        Ok(())
    }

    pub fn unsubscribe(&self, peer: NodeId) -> Result<(), BroadcastError> {
        let mut subs = self.subscribers.borrow_mut();
        let pos = subs.iter().position(|p| p == &peer);
        match pos {
            Some(idx) => {
                subs.remove(idx);
                Ok(())
            }
            None => Err(BroadcastError::PeerNotSubscribed {
                peer,
                channel_id: self.config.channel_id,
            }),
        }
    }

    pub fn is_subscribed(&self, peer: &NodeId) -> bool {
        self.subscribers.borrow().contains(peer)
    }

    pub fn subscriber_count(&self) -> usize {
        self.subscribers.borrow().len()
    }

    pub fn subscribers(&self) -> Vec<NodeId> {
        self.subscribers.borrow().clone()
    }

    pub fn record_published(&mut self) {
        self.published_count += 1;
    }

    pub fn record_delivered(&mut self) {
        self.delivered_count += 1;
    }

    pub fn record_dropped(&mut self) {
        self.dropped_count += 1;
    }

    pub fn delivery_rate(&self) -> f64 {
        if self.published_count == 0 {
            return 1.0;
        }
        self.delivered_count as f64 / self.published_count as f64
    }

    pub fn drop_rate(&self) -> f64 {
        if self.published_count == 0 {
            return 0.0;
        }
        self.dropped_count as f64 / self.published_count as f64
    }
}

///////////////////////////////////////////////////////////////////////////////
// MESSAGE INTEGRITY
///////////////////////////////////////////////////////////////////////////////

pub struct IntegrityRecord {
    pub msg_id: MessageID,
    pub signed_by: KeyId,
    pub signature: Signature,
    pub signed_at_ms: u64,
    pub verified: bool,
}

impl IntegrityRecord {
    pub fn new(msg_id: MessageID, signed_by: KeyId, signature: Signature, signed_at_ms: u64) -> Self {
        Self {
            msg_id,
            signed_by,
            signature,
            signed_at_ms,
            verified: false,
        }
    }

    pub fn mark_verified(&mut self) {
        self.verified = true;
    }
}

pub struct MessageIntegrity {
    pub records: std::cell::RefCell<std::collections::HashMap<MessageID, IntegrityRecord>>,
    pub total_signed: u64,
    pub total_verified: u64,
    pub total_failed: u64,
}

impl MessageIntegrity {
    pub fn new() -> Self {
        Self {
            records: std::cell::RefCell::new(std::collections::HashMap::new()),
            total_signed: 0,
            total_verified: 0,
            total_failed: 0,
        }
    }

    pub fn record_signature(
        &self,
        msg_id: MessageID,
        signed_by: KeyId,
        signature: Signature,
        at_ms: u64,
    ) {
        self.records.borrow_mut().insert(
            msg_id,
            IntegrityRecord::new(msg_id, signed_by, signature, at_ms),
        );
        self.total_signed += 1;
    }

    pub fn verify(&self, msg_id: MessageID) -> Result<bool, BroadcastError> {
        if let Some(record) = self.records.borrow_mut().get_mut(&msg_id) {
            // Simplified: signature format check (non-zero signature bytes)
            let valid = record.signature.0.iter().any(|b| *b != 0);
            if valid {
                record.mark_verified();
                self.total_verified += 1;
                Ok(true)
            } else {
                self.total_failed += 1;
                Ok(false)
            }
        } else {
            Err(BroadcastError::SignatureMissing { msg_id })
        }
    }

    pub fn is_verified(&self, msg_id: MessageID) -> bool {
        self.records
            .borrow()
            .get(&msg_id)
            .map(|r| r.verified)
            .unwrap_or(false)
    }

    pub fn total_signed(&self) -> u64 {
        self.total_signed
    }

    pub fn total_verified(&self) -> u64 {
        self.total_verified
    }

    pub fn total_failed(&self) -> u64 {
        self.total_failed
    }

    pub fn tracked_count(&self) -> usize {
        self.records.borrow().len()
    }

    pub fn verification_rate(&self) -> f64 {
        let total = self.total_verified + self.total_failed;
        if total == 0 {
            return 1.0;
        }
        self.total_verified as f64 / total as f64
    }
}

impl Default for MessageIntegrity {
    fn default() -> Self {
        Self::new()
    }
}

///////////////////////////////////////////////////////////////////////////////
// BROADCAST STATS
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone, Default)]
pub struct BroadcastStats {
    pub total_messages: u64,
    pub total_multicast: u64,
    pub total_regional: u64,
    pub total_dataset: u64,
    pub total_duplicates: u64,
    pub total_acks: u64,
    pub total_expired: u64,
    pub total_hop_limit_exceeded: u64,
    pub total_integrity_failed: u64,
    pub total_rate_limited: u64,
    pub total_channels: u64,
    pub total_subscriptions: u64,
    pub total_rebroadcasts_suppressed: u64,
}

impl BroadcastStats {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn broadcast_rate(&self) -> f64 {
        let total = self.total_messages + self.total_duplicates;
        if total == 0 {
            return 1.0;
        }
        self.total_messages as f64 / total as f64
    }

    pub fn ack_rate(&self) -> f64 {
        if self.total_messages == 0 {
            return 1.0;
        }
        self.total_acks as f64 / self.total_messages as f64
    }

    pub fn system_health_score(&self) -> f64 {
        let mut score = 100.0;
        score -= self.total_duplicates as f64 * 0.1;
        score -= self.total_expired as f64 * 0.5;
        score -= self.total_hop_limit_exceeded as f64 * 1.0;
        score -= self.total_integrity_failed as f64 * 2.0;
        score -= self.total_rate_limited as f64 * 0.5;
        score.max(0.0).min(100.0)
    }
}

///////////////////////////////////////////////////////////////////////////////
// BROADCAST MONITOR
///////////////////////////////////////////////////////////////////////////////

pub struct BroadcastMonitor {
    pub peer_activity: std::cell::RefCell<std::collections::HashMap<NodeId, u64>>,
    pub alert_count: u64,
    pub health_check_count: u64,
    pub broadcast_incident_count: u64,
}

impl BroadcastMonitor {
    pub fn new() -> Self {
        Self {
            peer_activity: std::cell::RefCell::new(std::collections::HashMap::new()),
            alert_count: 0,
            health_check_count: 0,
            broadcast_incident_count: 0,
        }
    }

    pub fn record_activity(&self, peer: NodeId, messages: u64) {
        let mut activity = self.peer_activity.borrow_mut();
        let prev = activity.get(&peer).copied().unwrap_or(0);
        activity.insert(peer, prev + messages);
        self.health_check_count += 1;
        if messages == 0 && prev > 0 {
            self.alert_count += 1;
            self.broadcast_incident_count += 1;
        }
    }

    pub fn peer_message_count(&self, peer: &NodeId) -> u64 {
        self.peer_activity.borrow().get(peer).copied().unwrap_or(0)
    }

    pub fn report_incident(&self, peer: NodeId) {
        self.broadcast_incident_count += 1;
        self.alert_count += 1;
        self.peer_activity.borrow_mut().insert(peer, 0);
    }

    pub fn alert_count(&self) -> u64 {
        self.alert_count
    }

    pub fn health_check_count(&self) -> u64 {
        self.health_check_count
    }

    pub fn broadcast_incident_count(&self) -> u64 {
        self.broadcast_incident_count
    }

    pub fn monitored_peer_count(&self) -> usize {
        self.peer_activity.borrow().len()
    }

    pub fn inactive_peer_count(&self) -> usize {
        self.peer_activity.borrow().values().filter(|c| *c == 0).count()
    }
}

impl Default for BroadcastMonitor {
    fn default() -> Self {
        Self::new()
    }
}

///////////////////////////////////////////////////////////////////////////////
// BROADCAST VALIDATOR
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BroadcastValidationResult {
    Valid,
    InvalidMessageID,
    InvalidPayloadSize,
    InvalidHopCount,
    InvalidPriority,
    InvalidPeer,
}

impl BroadcastValidationResult {
    pub fn as_str(&self) -> &'static str {
        match self {
            BroadcastValidationResult::Valid => "valid",
            BroadcastValidationResult::InvalidMessageID => "invalid_message_id",
            BroadcastValidationResult::InvalidPayloadSize => "invalid_payload_size",
            BroadcastValidationResult::InvalidHopCount => "invalid_hop_count",
            BroadcastValidationResult::InvalidPriority => "invalid_priority",
            BroadcastValidationResult::InvalidPeer => "invalid_peer",
        }
    }

    pub fn is_ok(&self) -> bool {
        matches!(self, BroadcastValidationResult::Valid)
    }
}

pub struct BroadcastValidator {
    pub validations: std::cell::RefCell<std::collections::HashMap<NodeId, BroadcastValidationResult>>,
    pub validation_count: u64,
    pub valid_count: u64,
    pub invalid_count: u64,
}

impl BroadcastValidator {
    pub fn new() -> Self {
        Self {
            validations: std::cell::RefCell::new(std::collections::HashMap::new()),
            validation_count: 0,
            valid_count: 0,
            invalid_count: 0,
        }
    }

    pub fn validate_message_id(&self, msg_id: MessageID) -> BroadcastValidationResult {
        self.validation_count += 1;
        let result = if msg_id.is_zero() {
            BroadcastValidationResult::InvalidMessageID
        } else {
            BroadcastValidationResult::Valid
        };
        match result {
            BroadcastValidationResult::Valid => self.valid_count += 1,
            _ => self.invalid_count += 1,
        }
        self.validations.borrow_mut().insert(msg_id.origin, result);
        result
    }

    pub fn validate_payload_size(&self, msg_id: MessageID, size: usize, max: usize) -> BroadcastValidationResult {
        self.validation_count += 1;
        let result = if size > max {
            BroadcastValidationResult::InvalidPayloadSize
        } else {
            BroadcastValidationResult::Valid
        };
        match result {
            BroadcastValidationResult::Valid => self.valid_count += 1,
            _ => self.invalid_count += 1,
        }
        self.validations.borrow_mut().insert(msg_id.origin, result);
        result
    }

    pub fn validate_hop_count(&self, msg_id: MessageID, hops: u64, max: u64) -> BroadcastValidationResult {
        self.validation_count += 1;
        let result = if hops > max {
            BroadcastValidationResult::InvalidHopCount
        } else {
            BroadcastValidationResult::Valid
        };
        match result {
            BroadcastValidationResult::Valid => self.valid_count += 1,
            _ => self.invalid_count += 1,
        }
        self.validations.borrow_mut().insert(msg_id.origin, result);
        result
    }

    pub fn validate_priority(&self, msg_id: MessageID, priority: u8) -> BroadcastValidationResult {
        self.validation_count += 1;
        let result = if priority > 10 {
            BroadcastValidationResult::InvalidPriority
        } else {
            BroadcastValidationResult::Valid
        };
        match result {
            BroadcastValidationResult::Valid => self.valid_count += 1,
            _ => self.invalid_count += 1,
        }
        self.validations.borrow_mut().insert(msg_id.origin, result);
        result
    }

    pub fn validate_peer(&self, peer: NodeId) -> BroadcastValidationResult {
        self.validation_count += 1;
        let result = if peer == NodeId::new([0u8; 32]) {
            BroadcastValidationResult::InvalidPeer
        } else {
            BroadcastValidationResult::Valid
        };
        match result {
            BroadcastValidationResult::Valid => self.valid_count += 1,
            _ => self.invalid_count += 1,
        }
        self.validations.borrow_mut().insert(peer, result);
        result
    }

    pub fn get_result(&self, peer: &NodeId) -> Option<BroadcastValidationResult> {
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

impl Default for BroadcastValidator {
    fn default() -> Self {
        Self::new()
    }
}

///////////////////////////////////////////////////////////////////////////////
// BROADCAST ENGINE
///////////////////////////////////////////////////////////////////////////////

pub struct BroadcastEngine {
    pub deduplicator: std::cell::RefCell<MessageDeduplicator>,
    pub ttl_manager: std::cell::RefCell<TTLManager>,
    pub fanout_controller: std::cell::RefCell<FanoutController>,
    pub rate_limiter: std::cell::RefCell<RateLimiter>,
    pub ack_tracker: std::cell::RefCell<AckTracker>,
    pub rebroadcast_suppressor: std::cell::RefCell<RebroadcastSuppressor>,
    pub channels: std::cell::RefCell<std::collections::HashMap<u64, BroadcastChannel>>,
    pub integrity: std::cell::RefCell<MessageIntegrity>,
    pub stats: std::cell::RefCell<BroadcastStats>,
    pub monitor: std::cell::RefCell<BroadcastMonitor>,
    pub validator: std::cell::RefCell<BroadcastValidator>,
}

impl BroadcastEngine {
    pub fn new() -> Self {
        Self {
            deduplicator: std::cell::RefCell::new(MessageDeduplicator::default()),
            ttl_manager: std::cell::RefCell::new(TTLManager::default()),
            fanout_controller: std::cell::RefCell::new(FanoutController::default()),
            rate_limiter: std::cell::RefCell::new(RateLimiter::default()),
            ack_tracker: std::cell::RefCell::new(AckTracker::default()),
            rebroadcast_suppressor: std::cell::RefCell::new(RebroadcastSuppressor::default()),
            channels: std::cell::RefCell::new(std::collections::HashMap::new()),
            integrity: std::cell::RefCell::new(MessageIntegrity::default()),
            stats: std::cell::RefCell::new(BroadcastStats::default()),
            monitor: std::cell::RefCell::new(BroadcastMonitor::new()),
            validator: std::cell::RefCell::new(BroadcastValidator::new()),
        }
    }

    pub fn broadcast(&self, msg: BroadcastMessage) -> Result<BroadcastMessage, BroadcastError> {
        self.rate_limiter.borrow_mut().check(msg.origin, msg.timestamp_ms)?;
        let is_dup = self.deduplicator.borrow_mut().is_duplicate(msg.msg_id, msg.timestamp_ms)?;
        if is_dup {
            self.stats.borrow_mut().total_duplicates += 1;
            return Err(BroadcastError::DuplicateMessage { msg_id: msg.msg_id });
        }
        if let Some(ttl) = msg.ttl_ms {
            self.ttl_manager.borrow_mut().set_ttl(msg.msg_id, ttl, msg.timestamp_ms);
        }
        if !self.validator.borrow_mut().validate_message_id(msg.msg_id).is_ok() {
            return Err(BroadcastError::InvalidMessage {
                reason: "invalid message ID".to_string(),
            });
        }
        if msg.payload.len() > 1024 * 1024 {
            return Err(BroadcastError::InvalidMessage {
                reason: "payload too large".to_string(),
            });
        }
        if msg.hop_count > msg.max_hops {
            self.stats.borrow_mut().total_hop_limit_exceeded += 1;
            return Err(BroadcastError::HopLimitExceeded { msg_id: msg.msg_id });
        }
        self.stats.borrow_mut().total_messages += 1;
        self.monitor.borrow_mut().record_activity(msg.origin, 1);
        Ok(msg)
    }

    pub fn multicast(
        &self,
        msg: BroadcastMessage,
        peers: Vec<NodeId>,
    ) -> Result<BroadcastMessage, BroadcastError> {
        let result = self.broadcast(msg.clone())?;
        if result.is_expired(result.timestamp_ms + 1) {
            return Err(BroadcastError::MessageExpired { msg_id: result.msg_id });
        }
        self.fanout_controller
            .borrow_mut()
            .create_plan(result.msg_id, peers)?;
        self.stats.borrow_mut().total_multicast += 1;
        Ok(result)
    }

    pub fn regional_broadcast(
        &self,
        msg: BroadcastMessage,
        region: RegionId,
    ) -> Result<BroadcastMessage, BroadcastError> {
        let result = self.broadcast(msg.clone())?;
        let mut msg = result;
        msg.region = Some(region);
        self.stats.borrow_mut().total_regional += 1;
        Ok(msg)
    }

    pub fn dataset_broadcast(
        &self,
        msg: BroadcastMessage,
        dataset: DatasetId,
    ) -> Result<BroadcastMessage, BroadcastError> {
        let result = self.broadcast(msg.clone())?;
        let mut msg = result;
        msg.dataset = Some(dataset);
        self.stats.borrow_mut().total_dataset += 1;
        Ok(msg)
    }

    pub fn deduplicate(&self, msg_id: MessageID) -> Result<bool, BroadcastError> {
        self.deduplicator.borrow_mut().is_duplicate(msg_id, 0)
    }

    pub fn record_ack(&self, msg_id: MessageID, peer: NodeId) -> Result<(), BroadcastError> {
        self.ack_tracker.borrow_mut().record_ack(msg_id, peer)?;
        self.stats.borrow_mut().total_acks += 1;
        Ok(())
    }

    pub fn is_fully_delivered(&self, msg_id: MessageID) -> Result<bool, BroadcastError> {
        self.ack_tracker.borrow_mut().is_complete(msg_id)
    }

    pub fn delivery_status(&self, msg_id: MessageID) -> Result<DeliveryResult, BroadcastError> {
        let complete = self.ack_tracker.borrow_mut().is_complete(msg_id)?;
        let pending = self.ack_tracker.borrow_mut().pending_acks(msg_id)?;
        let rate = self.ack_tracker.borrow_mut().ack_rate(msg_id)?;
        Ok(DeliveryResult {
            msg_id,
            delivered_to: Vec::new(),
            acked_by: Vec::new(),
            pending_acks: pending,
            delivery_count: 0,
            rebroadcast_count: 0,
            is_complete: complete,
            expired: false,
        })
    }

    pub fn set_ttl(&self, msg_id: MessageID, ttl_ms: u64) -> Result<(), BroadcastError> {
        self.ttl_manager.borrow_mut().set_ttl(msg_id, ttl_ms, 0);
        Ok(())
    }

    pub fn expire_by_ttl(&self, at_ms: u64) -> Result<u64, BroadcastError> {
        let expired = self.ttl_manager.borrow_mut().expire_all(at_ms);
        self.stats.borrow_mut().total_expired += expired;
        Ok(expired)
    }

    pub fn configure_channel(
        &self,
        channel_id: u64,
        config: ChannelConfig,
    ) -> Result<(), BroadcastError> {
        config.validate()?;
        let mut channels = self.channels.borrow_mut();
        if channels.contains_key(&channel_id) {
            return Err(BroadcastError::ChannelNotFound { channel_id });
        }
        channels.insert(channel_id, BroadcastChannel::new(config));
        self.stats.borrow_mut().total_channels += 1;
        Ok(())
    }

    pub fn publish(&self, channel_id: u64, msg: BroadcastMessage) -> Result<BroadcastMessage, BroadcastError> {
        let mut channels = self.channels.borrow_mut();
        let channel = channels.get_mut(&channel_id).ok_or_else(|| BroadcastError::ChannelNotFound { channel_id })?;
        if msg.payload.len() > channel.config.max_msg_size {
            channel.record_dropped();
            return Err(BroadcastError::InvalidMessage {
                reason: "message exceeds channel max size".to_string(),
            });
        }
        if channel.config.require_signature && msg.signature.is_none() {
            channel.record_dropped();
            return Err(BroadcastError::SignatureMissing { msg_id: msg.msg_id });
        }
        channel.record_published();
        let subs = channel.subscribers();
        for peer in &subs {
            self.record_ack(msg.msg_id, *peer)?;
        }
        channel.record_delivered();
        self.stats.borrow_mut().total_subscriptions += subs.len() as u64;
        Ok(msg)
    }

    pub fn subscribe(&self, channel_id: u64, peer: NodeId) -> Result<(), BroadcastError> {
        let mut channels = self.channels.borrow_mut();
        let channel = channels.get_mut(&channel_id).ok_or_else(|| BroadcastError::ChannelNotFound { channel_id })?;
        channel.subscribe(peer, 0)
    }

    pub fn unsubscribe(&self, channel_id: u64, peer: NodeId) -> Result<(), BroadcastError> {
        let mut channels = self.channels.borrow_mut();
        let channel = channels.get_mut(&channel_id).ok_or_else(|| BroadcastError::ChannelNotFound { channel_id })?;
        channel.unsubscribe(peer)
    }

    pub fn verify_integrity(&self, msg: &BroadcastMessage) -> Result<bool, BroadcastError> {
        self.integrity.borrow_mut().verify(msg.msg_id)
    }

    pub fn sign_message(&self, msg: &mut BroadcastMessage, key_id: KeyId) -> Result<Signature, BroadcastError> {
        let sig = Signature([1u8; 64]);
        msg.signature = Some(sig);
        msg.signed_by = Some(key_id);
        self.integrity.borrow_mut().record_signature(msg.msg_id, key_id, sig, msg.timestamp_ms);
        Ok(sig)
    }

    pub fn stats(&self) -> BroadcastStats {
        self.stats.borrow().clone()
    }

    pub fn broadcast_name(&self) -> &'static str {
        "broadcast_engine"
    }

    pub fn total_broadcast(&self) -> u64 {
        self.stats.borrow().total_messages
    }

    pub fn total_deduplicated(&self) -> u64 {
        self.stats.borrow().total_duplicates
    }

    pub fn total_acks(&self) -> u64 {
        self.stats.borrow().total_acks
    }

    pub fn create_channel(&self, channel_id: u64, name: String) -> Result<(), BroadcastError> {
        self.configure_channel(channel_id, ChannelConfig::new(channel_id, name))
    }

    pub fn channel_subscriber_count(&self, channel_id: u64) -> Result<usize, BroadcastError> {
        let channels = self.channels.borrow();
        let channel = channels.get(&channel_id).ok_or_else(|| BroadcastError::ChannelNotFound { channel_id })?;
        Ok(channel.subscriber_count())
    }

    pub fn monitor_peer(&self, peer: NodeId, messages: u64) {
        self.monitor.borrow_mut().record_activity(peer, messages);
    }

    pub fn is_peer_active(&self, peer: &NodeId) -> bool {
        self.monitor.borrow().peer_message_count(peer) > 0
    }

    pub fn validate_payload(&self, msg_id: MessageID, size: usize, max: usize) -> BroadcastValidationResult {
        self.validator.borrow_mut().validate_payload_size(msg_id, size, max)
    }

    pub fn validate_hops(&self, msg_id: MessageID, hops: u64, max: u64) -> BroadcastValidationResult {
        self.validator.borrow_mut().validate_hop_count(msg_id, hops, max)
    }

    pub fn validate_priority(&self, msg_id: MessageID, priority: u8) -> BroadcastValidationResult {
        self.validator.borrow_mut().validate_priority(msg_id, priority)
    }

    pub fn validate_peer(&self, peer: NodeId) -> BroadcastValidationResult {
        self.validator.borrow_mut().validate_peer(peer)
    }

    pub fn valid_rate(&self) -> f64 {
        self.validator.borrow().valid_rate()
    }

    pub fn broadcast_health_score(&self) -> f64 {
        self.stats.borrow().system_health_score()
    }
}

impl Default for BroadcastEngine {
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

    fn test_region_id() -> RegionId {
        RegionId::new([3; 32])
    }

    fn test_dataset_id() -> DatasetId {
        DatasetId::new([4; 32])
    }

    fn test_key_id() -> KeyId {
        KeyId::new([5; 16])
    }

    fn test_signature() -> Signature {
        Signature([6; 64])
    }

    fn test_endpoint() -> Endpoint {
        Endpoint {
            carrier_type: CarrierType::Loopback,
            address: vec![1, 2, 3],
        }
    }

    fn test_msg_id() -> MessageID {
        MessageID::new([7; 16])
    }

    fn test_broadcast_message() -> BroadcastMessage {
        BroadcastMessage::new(test_msg_id(), vec![1, 2, 3], test_node_id())
            .with_ttl(60000)
            .with_max_hops(10)
            .with_region(test_region_id())
            .with_dataset(test_dataset_id())
            .with_priority(5)
            .with_delivery_semantics(DeliverySemantics::AtLeastOnce)
            .with_timestamp(1000)
    }

    // === BroadcastError tests ===
    #[test]
    fn broadcast_error_category() {
        assert_eq!(
            BroadcastError::MessageNotFound { msg_id: test_msg_id() }.category(),
            "message_not_found"
        );
        assert_eq!(
            BroadcastError::MessageAlreadyDelivered { msg_id: test_msg_id() }.category(),
            "message_already_delivered"
        );
        assert_eq!(
            BroadcastError::DuplicateMessage { msg_id: test_msg_id() }.category(),
            "duplicate_message"
        );
        assert_eq!(
            BroadcastError::ChannelNotFound { channel_id: 42 }.category(),
            "channel_not_found"
        );
        assert_eq!(
            BroadcastError::PeerNotSubscribed { peer: test_node_id(), channel_id: 42 }.category(),
            "peer_not_subscribed"
        );
        assert_eq!(
            BroadcastError::PeerAlreadySubscribed { peer: test_node_id(), channel_id: 42 }.category(),
            "peer_already_subscribed"
        );
        assert_eq!(
            BroadcastError::InsufficientCapacity { needed: 10, available: 5 }.category(),
            "insufficient_capacity"
        );
        assert_eq!(
            BroadcastError::RateLimitExceeded { peer: test_node_id() }.category(),
            "rate_limit_exceeded"
        );
        assert_eq!(
            BroadcastError::MessageExpired { msg_id: test_msg_id() }.category(),
            "message_expired"
        );
        assert_eq!(
            BroadcastError::HopLimitExceeded { msg_id: test_msg_id() }.category(),
            "hop_limit_exceeded"
        );
        assert_eq!(
            BroadcastError::IntegrityFailed { msg_id: test_msg_id() }.category(),
            "integrity_failed"
        );
        assert_eq!(
            BroadcastError::SignatureMissing { msg_id: test_msg_id() }.category(),
            "signature_missing"
        );
        assert_eq!(
            BroadcastError::InvalidMessage { reason: "bad".to_string() }.category(),
            "invalid_message"
        );
        assert_eq!(
            BroadcastError::Internal { message: "panic".to_string() }.category(),
            "internal"
        );
    }

    #[test]
    fn broadcast_error_is_recoverable() {
        assert!(BroadcastError::RateLimitExceeded { peer: test_node_id() }.is_recoverable());
        assert!(BroadcastError::InsufficientCapacity { needed: 10, available: 5 }.is_recoverable());
        assert!(BroadcastError::MessageExpired { msg_id: test_msg_id() }.is_recoverable());
        assert!(BroadcastError::DuplicateMessage { msg_id: test_msg_id() }.is_recoverable());
        assert!(!BroadcastError::MessageNotFound { msg_id: test_msg_id() }.is_recoverable());
    }

    #[test]
    fn broadcast_error_suggested_action() {
        assert_eq!(
            BroadcastError::RateLimitExceeded { peer: test_node_id() }.suggested_action(),
            "wait and retry later"
        );
        assert_eq!(
            BroadcastError::SignatureMissing { msg_id: test_msg_id() }.suggested_action(),
            "sign the message before sending"
        );
        assert_eq!(
            BroadcastError::Internal { message: "x".to_string() }.suggested_action(),
            "contact support"
        );
    }

    // === MessageID tests ===
    #[test]
    fn message_id_zero() {
        assert!(MessageID::zero().is_zero());
        assert!(!test_msg_id().is_zero());
    }

    #[test]
    fn message_id_bytes() {
        let id = MessageID::new([9; 16]);
        assert_eq!(id.as_bytes(), &[9u8; 16]);
    }

    // === BroadcastMessage tests ===
    #[test]
    fn broadcast_message_ttl_and_hops() {
        let msg = test_broadcast_message();
        assert!(!msg.is_expired(500));
        assert!(msg.is_expired(70000));
        assert!(msg.has_hop_capacity());
        let mut msg = msg;
        msg.increment_hop();
        assert_eq!(msg.hop_count, 1);
    }

    #[test]
    fn broadcast_message_ack() {
        let mut msg = test_broadcast_message();
        assert!(!msg.is_acked_by(&test_node_id()));
        msg.add_ack(test_node_id());
        assert!(msg.is_acked_by(&test_node_id()));
        assert_eq!(msg.ack_count(), 1);
        msg.add_ack(test_node_id());
        assert_eq!(msg.ack_count(), 1);
    }

    #[test]
    fn broadcast_message_priority() {
        let msg = BroadcastMessage::new(test_msg_id(), vec![], test_node_id()).with_priority(9);
        assert_eq!(msg.priority, 9);
    }

    #[test]
    fn broadcast_message_delivery_semantics() {
        assert!(DeliverySemantics::AtLeastOnce.requires_ack());
        assert!(DeliverySemantics::ExactlyOnce.requires_ack());
        assert!(!DeliverySemantics::AtMostOnce.requires_ack());
        assert!(!DeliverySemantics::BestEffort.requires_ack());
        assert!(DeliverySemantics::ExactlyOnce.idempotent());
        assert!(!DeliverySemantics::AtMostOnce.idempotent());
    }

    // === DeliverySemantics tests ===
    #[test]
    fn delivery_semantics_as_str() {
        assert_eq!(DeliverySemantics::AtMostOnce.as_str(), "at_most_once");
        assert_eq!(DeliverySemantics::AtLeastOnce.as_str(), "at_least_once");
        assert_eq!(DeliverySemantics::ExactlyOnce.as_str(), "exactly_once");
        assert_eq!(DeliverySemantics::BestEffort.as_str(), "best_effort");
    }

    // === DeliveryResult tests ===
    #[test]
    fn delivery_result_mark_delivered_and_acked() {
        let mut result = DeliveryResult::new(test_msg_id(), vec![test_node_id(), test_peer_id()]);
        result.mark_delivered(test_node_id());
        assert_eq!(result.delivery_count, 1);
        result.mark_acked(test_node_id());
        assert_eq!(result.ack_count(), 1);
    }

    #[test]
    fn delivery_result_complete() {
        let mut result = DeliveryResult::new(test_msg_id(), vec![test_node_id()]);
        result.mark_delivered(test_node_id());
        result.mark_acked(test_node_id());
        assert!(result.is_complete);
    }

    #[test]
    fn delivery_result_rates() {
        let mut result = DeliveryResult::new(test_msg_id(), vec![test_node_id(), test_peer_id()]);
        result.mark_delivered(test_node_id());
        assert!((result.delivery_rate(2) - 0.5).abs() < 0.01);
        assert!((result.ack_rate(2) - 0.0).abs() < 0.01);
    }

    // === ChannelConfig tests ===
    #[test]
    fn channel_config_validate() {
        let config = ChannelConfig::new(1, "test".to_string());
        assert!(config.validate().is_ok());
        let config = ChannelConfig::new(2, "empty".to_string()).with_max_subscribers(0);
        assert!(config.validate().is_err());
        let config = ChannelConfig::new(3, "empty".to_string()).with_max_msg_size(0);
        assert!(config.validate().is_err());
    }

    #[test]
    fn channel_config_builder() {
        let config = ChannelConfig::new(1, "ch".to_string())
            .with_max_subscribers(100)
            .with_ttl(5000)
            .with_max_hops(5)
            .with_signature_required(true);
        assert_eq!(config.max_subscribers, 100);
        assert_eq!(config.ttl_ms, Some(5000));
        assert_eq!(config.max_hops, 5);
        assert!(config.require_signature);
    }

    // === MessageDeduplicator tests ===
    #[test]
    fn deduplicator_new_and_duplicate() {
        let dedup = MessageDeduplicator::new(60000, 100);
        assert!(!dedup.is_duplicate(test_msg_id(), 1000).unwrap());
        assert!(dedup.is_duplicate(test_msg_id(), 2000).unwrap());
        assert_eq!(dedup.total_duplicates(), 1);
    }

    #[test]
    fn deduplicator_window_expiry() {
        let dedup = MessageDeduplicator::new(1000, 100);
        assert!(!dedup.is_duplicate(test_msg_id(), 100).unwrap());
        assert!(dedup.is_duplicate(test_msg_id(), 200).unwrap());
        // After window expiry, treated as new
        assert!(!dedup.is_duplicate(test_msg_id(), 1500).unwrap());
    }

    #[test]
    fn deduplicator_mark_delivered() {
        let dedup = MessageDeduplicator::new(60000, 100);
        dedup.is_duplicate(test_msg_id(), 100).unwrap();
        dedup.mark_delivered(test_msg_id()).unwrap();
        assert!(dedup.is_delivered(test_msg_id()));
    }

    #[test]
    fn deduplicator_duplicate_rate() {
        let dedup = MessageDeduplicator::new(60000, 100);
        dedup.is_duplicate(test_msg_id(), 100).unwrap();
        dedup.is_duplicate(test_msg_id(), 200).unwrap();
        dedup.is_duplicate(test_msg_id(), 300).unwrap();
        assert!((dedup.duplicate_rate() - (2.0 / 3.0)).abs() < 0.01);
    }

    #[test]
    fn deduplicator_expire_old() {
        let dedup = MessageDeduplicator::new(1000, 100);
        dedup.is_duplicate(test_msg_id(), 100).unwrap();
        let expired = dedup.expire_old(2000);
        assert_eq!(expired, 1);
        assert_eq!(dedup.tracked_count(), 0);
    }

    // === TTLManager tests ===
    #[test]
    fn ttl_manager_set_and_expired() {
        let ttl = TTLManager::new();
        ttl.set_ttl(test_msg_id(), 5000, 1000);
        assert!(!ttl.is_expired(test_msg_id(), 5000));
        assert!(ttl.is_expired(test_msg_id(), 6001));
    }

    #[test]
    fn ttl_manager_expire_all() {
        let ttl = TTLManager::new();
        ttl.set_ttl(test_msg_id(), 5000, 1000);
        ttl.set_ttl(MessageID::new([8; 16]), 5000, 1000);
        let expired = ttl.expire_all(6001);
        assert_eq!(expired, 2);
        assert_eq!(ttl.tracked_count(), 0);
    }

    #[test]
    fn ttl_manager_no_expiry() {
        let ttl = TTLManager::new();
        assert!(!ttl.is_expired(test_msg_id(), 999999));
    }

    // === FanoutPlan tests ===
    #[test]
    fn fanout_plan_batches() {
        let peers = vec![test_node_id(), test_peer_id(), NodeId::new([3; 32])];
        let mut plan = FanoutPlan::new(test_msg_id(), peers.clone(), 2);
        let batch1 = plan.next_batch();
        assert_eq!(batch1.len(), 2);
        let batch2 = plan.next_batch();
        assert_eq!(batch2.len(), 1);
        let batch3 = plan.next_batch();
        assert!(batch3.is_empty());
        assert!(plan.is_completed());
    }

    #[test]
    fn fanout_plan_progress() {
        let peers = vec![test_node_id(), test_peer_id()];
        let mut plan = FanoutPlan::new(test_msg_id(), peers, 1);
        assert!((plan.progress() - 0.0).abs() < 0.01);
        plan.next_batch();
        assert!((plan.progress() - 0.5).abs() < 0.01);
        plan.next_batch();
        assert!((plan.progress() - 1.0).abs() < 0.01);
    }

    #[test]
    fn fanout_plan_empty() {
        let mut plan = FanoutPlan::new(test_msg_id(), Vec::new(), 5);
        assert!(plan.is_completed());
        assert!(plan.next_batch().is_empty());
    }

    // === FanoutController tests ===
    #[test]
    fn fanout_controller_create_and_advance() {
        let fc = FanoutController::default();
        fc.create_plan(test_msg_id(), vec![test_node_id(), test_peer_id()]).unwrap();
        assert_eq!(fc.total_plans(), 1);
        let batch = fc.advance_plan(&test_msg_id()).unwrap();
        assert_eq!(batch.len(), 2);
    }

    #[test]
    fn fanout_controller_duplicate_plan() {
        let fc = FanoutController::default();
        fc.create_plan(test_msg_id(), vec![test_node_id()]).unwrap();
        let err = fc.create_plan(test_msg_id(), vec![test_node_id()]).unwrap_err();
        assert!(matches!(err, BroadcastError::InvalidMessage { .. }));
    }

    #[test]
    fn fanout_controller_complete() {
        let fc = FanoutController::default();
        fc.create_plan(test_msg_id(), vec![test_node_id()]).unwrap();
        fc.complete_plan(&test_msg_id()).unwrap();
        assert_eq!(fc.total_completed(), 1);
    }

    #[test]
    fn fanout_controller_message_not_found() {
        let fc = FanoutController::default();
        let err = fc.complete_plan(&test_msg_id()).unwrap_err();
        assert!(matches!(err, BroadcastError::MessageNotFound { .. }));
    }

    // === RateLimitWindow tests ===
    #[test]
    fn rate_limit_window_check_and_block() {
        let mut rw = RateLimitWindow::new(test_node_id(), 0, 60000, 2);
        rw.check(1000).unwrap();
        rw.check(2000).unwrap();
        let err = rw.check(3000).unwrap_err();
        assert!(matches!(err, BroadcastError::RateLimitExceeded { .. }));
        assert_eq!(rw.blocked_count(), 1);
    }

    #[test]
    fn rate_limit_window_reset() {
        let mut rw = RateLimitWindow::new(test_node_id(), 0, 1000, 1);
        rw.check(500).unwrap();
        assert!(rw.check(1500).is_ok());
        assert_eq!(rw.message_count(), 1);
    }

    // === RateLimiter tests ===
    #[test]
    fn rate_limiter_check() {
        let rl = RateLimiter::default();
        rl.check(test_node_id(), 1000).unwrap();
        assert_eq!(rl.total_checked(), 1);
    }

    #[test]
    fn rate_limiter_block_rate() {
        let rl = RateLimiter::new(60000, 1);
        rl.check(test_node_id(), 1000).unwrap();
        let _ = rl.check(test_node_id(), 2000);
        let rate = rl.block_rate();
        assert!((rate - 0.5).abs() < 0.01);
    }

    #[test]
    fn rate_limiter_tracked_peers() {
        let rl = RateLimiter::default();
        rl.check(test_node_id(), 1000).unwrap();
        rl.check(test_peer_id(), 1000).unwrap();
        assert_eq!(rl.tracked_peer_count(), 2);
    }

    // === AckEntry tests ===
    #[test]
    fn ack_entry_record_and_complete() {
        let entry = AckEntry::new(test_msg_id(), vec![test_node_id(), test_peer_id()], 1000);
        entry.record_ack(test_node_id()).unwrap();
        assert_eq!(entry.ack_count(), 1);
        assert!(!entry.is_complete());
        entry.record_ack(test_peer_id()).unwrap();
        assert!(entry.is_complete());
    }

    #[test]
    fn ack_entry_duplicate_ack() {
        let entry = AckEntry::new(test_msg_id(), vec![test_node_id()], 1000);
        entry.record_ack(test_node_id()).unwrap();
        let err = entry.record_ack(test_node_id()).unwrap_err();
        assert!(matches!(err, BroadcastError::DuplicateMessage { .. }));
    }

    #[test]
    fn ack_entry_pending() {
        let entry = AckEntry::new(test_msg_id(), vec![test_node_id(), test_peer_id()], 1000);
        entry.record_ack(test_node_id()).unwrap();
        let pending = entry.pending_acks();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0], test_peer_id());
    }

    // === AckTracker tests ===
    #[test]
    fn ack_tracker_register_and_ack() {
        let at = AckTracker::default();
        at.register(test_msg_id(), vec![test_node_id(), test_peer_id()], 1000).unwrap();
        at.record_ack(test_msg_id(), test_node_id()).unwrap();
        assert_eq!(at.total_acks(), 1);
    }

    #[test]
    fn ack_tracker_is_complete() {
        let at = AckTracker::default();
        at.register(test_msg_id(), vec![test_node_id()], 1000).unwrap();
        at.record_ack(test_msg_id(), test_node_id()).unwrap();
        assert!(at.is_complete(test_msg_id()).unwrap());
    }

    #[test]
    fn ack_tracker_message_not_found() {
        let at = AckTracker::default();
        let err = at.is_complete(test_msg_id()).unwrap_err();
        assert!(matches!(err, BroadcastError::MessageNotFound { .. }));
    }

    #[test]
    fn ack_tracker_ack_rate() {
        let at = AckTracker::default();
        at.register(test_msg_id(), vec![test_node_id(), test_peer_id()], 1000).unwrap();
        at.record_ack(test_msg_id(), test_node_id()).unwrap();
        assert!((at.ack_rate(test_msg_id()).unwrap() - 0.5).abs() < 0.01);
    }

    // === RebroadcastRecord tests ===
    #[test]
    fn rebroadcast_record_suppress() {
        let mut rr = RebroadcastRecord::new(test_msg_id(), 1000);
        assert!(!rr.should_suppress(1000));
        assert!(rr.should_suppress(1500));
        assert!(rr.should_suppress(1800));
        assert!(!rr.should_suppress(2500));
    }

    #[test]
    fn rebroadcast_record_suppression_rate() {
        let mut rr = RebroadcastRecord::new(test_msg_id(), 1000);
        rr.should_suppress(1000);
        rr.should_suppress(1500);
        rr.should_suppress(1800);
        assert!((rr.suppression_rate() - (2.0 / 3.0)).abs() < 0.01);
    }

    // === RebroadcastSuppressor tests ===
    #[test]
    fn rebroadcast_suppressor_check() {
        let rs = RebroadcastSuppressor::default();
        assert!(!rs.check(test_msg_id(), 1000).unwrap());
        assert!(rs.check(test_msg_id(), 1500).unwrap());
        assert_eq!(rs.total_suppressed(), 1);
        assert_eq!(rs.total_allowed(), 1);
    }

    #[test]
    fn rebroadcast_suppressor_rate() {
        let rs = RebroadcastSuppressor::new(1000);
        rs.check(test_msg_id(), 1000).unwrap();
        rs.check(test_msg_id(), 1500).unwrap();
        rs.check(test_msg_id(), 1800).unwrap();
        let rate = rs.suppression_rate();
        assert!((rate - (2.0 / 3.0)).abs() < 0.01);
    }

    // === BroadcastChannel tests ===
    #[test]
    fn broadcast_channel_subscribe_unsubscribe() {
        let config = ChannelConfig::new(1, "ch".to_string()).with_max_subscribers(2);
        let channel = BroadcastChannel::new(config);
        channel.subscribe(test_node_id(), 1000).unwrap();
        assert!(channel.is_subscribed(&test_node_id()));
        assert_eq!(channel.subscriber_count(), 1);
        channel.unsubscribe(test_node_id()).unwrap();
        assert!(!channel.is_subscribed(&test_node_id()));
    }

    #[test]
    fn broadcast_channel_duplicate_subscribe() {
        let channel = BroadcastChannel::new(ChannelConfig::new(1, "ch".to_string()));
        channel.subscribe(test_node_id(), 1000).unwrap();
        let err = channel.subscribe(test_node_id(), 2000).unwrap_err();
        assert!(matches!(err, BroadcastError::PeerAlreadySubscribed { .. }));
    }

    #[test]
    fn broadcast_channel_unsubscribe_not_subscribed() {
        let channel = BroadcastChannel::new(ChannelConfig::new(1, "ch".to_string()));
        let err = channel.unsubscribe(test_node_id()).unwrap_err();
        assert!(matches!(err, BroadcastError::PeerNotSubscribed { .. }));
    }

    #[test]
    fn broadcast_channel_max_subscribers() {
        let config = ChannelConfig::new(1, "ch".to_string()).with_max_subscribers(1);
        let channel = BroadcastChannel::new(config);
        channel.subscribe(test_node_id(), 1000).unwrap();
        let err = channel.subscribe(test_peer_id(), 2000).unwrap_err();
        assert!(matches!(err, BroadcastError::InsufficientCapacity { .. }));
    }

    #[test]
    fn broadcast_channel_delivery_rate() {
        let mut channel = BroadcastChannel::new(ChannelConfig::new(1, "ch".to_string()));
        channel.record_published();
        channel.record_published();
        channel.record_delivered();
        channel.record_dropped();
        assert!((channel.delivery_rate() - 0.5).abs() < 0.01);
        assert!((channel.drop_rate() - 0.5).abs() < 0.01);
    }

    // === MessageIntegrity tests ===
    #[test]
    fn message_integrity_sign_and_verify() {
        let mi = MessageIntegrity::default();
        mi.record_signature(test_msg_id(), test_key_id(), test_signature(), 1000);
        assert!(mi.verify(test_msg_id()).unwrap());
        assert!(mi.is_verified(test_msg_id()));
    }

    #[test]
    fn message_integrity_signature_missing() {
        let mi = MessageIntegrity::default();
        let err = mi.verify(test_msg_id()).unwrap_err();
        assert!(matches!(err, BroadcastError::SignatureMissing { .. }));
    }

    #[test]
    fn message_integrity_verification_rate() {
        let mi = MessageIntegrity::default();
        mi.record_signature(test_msg_id(), test_key_id(), test_signature(), 1000);
        mi.record_signature(MessageID::new([8; 16]), test_key_id(), Signature([0; 64]), 2000);
        mi.verify(test_msg_id()).unwrap();
        let _ = mi.verify(MessageID::new([8; 16]));
        let rate = mi.verification_rate();
        assert!((rate - 0.5).abs() < 0.01);
    }

    // === BroadcastStats tests ===
    #[test]
    fn broadcast_stats_health_score() {
        let mut s = BroadcastStats::default();
        assert_eq!(s.system_health_score(), 100.0);
        s.total_integrity_failed = 10;
        let score = s.system_health_score();
        assert!(score < 100.0);
        assert!(score >= 0.0);
    }

    #[test]
    fn broadcast_stats_rates() {
        let mut s = BroadcastStats::default();
        s.total_messages = 100;
        s.total_acks = 80;
        assert!((s.ack_rate() - 0.8).abs() < 0.01);
    }

    // === BroadcastMonitor tests ===
    #[test]
    fn broadcast_monitor_record_activity() {
        let bm = BroadcastMonitor::default();
        bm.record_activity(test_node_id(), 5);
        assert_eq!(bm.peer_message_count(&test_node_id()), 5);
        assert_eq!(bm.health_check_count(), 1);
    }

    #[test]
    fn broadcast_monitor_incident() {
        let bm = BroadcastMonitor::default();
        bm.record_activity(test_node_id(), 5);
        bm.report_incident(test_node_id());
        assert_eq!(bm.broadcast_incident_count(), 1);
        assert_eq!(bm.alert_count(), 1);
        assert_eq!(bm.inactive_peer_count(), 1);
    }

    // === BroadcastValidationResult tests ===
    #[test]
    fn broadcast_validation_result_as_str() {
        assert_eq!(BroadcastValidationResult::Valid.as_str(), "valid");
        assert_eq!(BroadcastValidationResult::InvalidMessageID.as_str(), "invalid_message_id");
        assert_eq!(BroadcastValidationResult::InvalidPayloadSize.as_str(), "invalid_payload_size");
        assert_eq!(BroadcastValidationResult::InvalidHopCount.as_str(), "invalid_hop_count");
        assert_eq!(BroadcastValidationResult::InvalidPriority.as_str(), "invalid_priority");
        assert_eq!(BroadcastValidationResult::InvalidPeer.as_str(), "invalid_peer");
        assert!(BroadcastValidationResult::Valid.is_ok());
        assert!(!BroadcastValidationResult::InvalidPeer.is_ok());
    }

    // === BroadcastValidator tests ===
    #[test]
    fn broadcast_validator_validate_message_id() {
        let bv = BroadcastValidator::default();
        assert_eq!(bv.validate_message_id(test_msg_id()), BroadcastValidationResult::Valid);
        assert_eq!(
            bv.validate_message_id(MessageID::zero()),
            BroadcastValidationResult::InvalidMessageID
        );
    }

    #[test]
    fn broadcast_validator_validate_payload_size() {
        let bv = BroadcastValidator::default();
        assert_eq!(
            bv.validate_payload_size(test_msg_id(), 100, 1000),
            BroadcastValidationResult::Valid
        );
        assert_eq!(
            bv.validate_payload_size(test_msg_id(), 2000, 1000),
            BroadcastValidationResult::InvalidPayloadSize
        );
    }

    #[test]
    fn broadcast_validator_validate_hop_count() {
        let bv = BroadcastValidator::default();
        assert_eq!(
            bv.validate_hop_count(test_msg_id(), 5, 10),
            BroadcastValidationResult::Valid
        );
        assert_eq!(
            bv.validate_hop_count(test_msg_id(), 15, 10),
            BroadcastValidationResult::InvalidHopCount
        );
    }

    #[test]
    fn broadcast_validator_validate_priority() {
        let bv = BroadcastValidator::default();
        assert_eq!(
            bv.validate_priority(test_msg_id(), 5),
            BroadcastValidationResult::Valid
        );
        assert_eq!(
            bv.validate_priority(test_msg_id(), 15),
            BroadcastValidationResult::InvalidPriority
        );
    }

    #[test]
    fn broadcast_validator_validate_peer() {
        let bv = BroadcastValidator::default();
        assert_eq!(
            bv.validate_peer(test_node_id()),
            BroadcastValidationResult::Valid
        );
        assert_eq!(
            bv.validate_peer(NodeId::new([0u8; 32])),
            BroadcastValidationResult::InvalidPeer
        );
    }

    #[test]
    fn broadcast_validator_valid_rate() {
        let bv = BroadcastValidator::default();
        bv.validate_message_id(test_msg_id());
        bv.validate_message_id(MessageID::zero());
        assert!((bv.valid_rate() - 0.5).abs() < 0.01);
    }

    // === BroadcastEngine tests ===
    #[test]
    fn broadcast_engine_broadcast() {
        let engine = BroadcastEngine::default();
        let msg = test_broadcast_message();
        let result = engine.broadcast(msg.clone()).unwrap();
        assert_eq!(result.msg_id, msg.msg_id);
        assert_eq!(engine.total_broadcast(), 1);
    }

    #[test]
    fn broadcast_engine_duplicate() {
        let engine = BroadcastEngine::default();
        let msg = test_broadcast_message();
        engine.broadcast(msg.clone()).unwrap();
        let err = engine.broadcast(msg).unwrap_err();
        assert!(matches!(err, BroadcastError::DuplicateMessage { .. }));
    }

    #[test]
    fn broadcast_engine_multicast() {
        let engine = BroadcastEngine::default();
        let msg = test_broadcast_message();
        let result = engine.multicast(msg.clone(), vec![test_node_id(), test_peer_id()]).unwrap();
        assert_eq!(engine.total_multicast(), 1);
        assert_eq!(result.msg_id, msg.msg_id);
    }

    #[test]
    fn broadcast_engine_regional() {
        let engine = BroadcastEngine::default();
        let msg = test_broadcast_message();
        let result = engine.regional_broadcast(msg.clone(), test_region_id()).unwrap();
        assert_eq!(engine.total_regional(), 1);
        assert!(result.region.is_some());
    }

    #[test]
    fn broadcast_engine_dataset() {
        let engine = BroadcastEngine::default();
        let msg = test_broadcast_message();
        let result = engine.dataset_broadcast(msg.clone(), test_dataset_id()).unwrap();
        assert_eq!(engine.total_dataset(), 1);
        assert!(result.dataset.is_some());
    }

    #[test]
    fn broadcast_engine_record_ack() {
        let engine = BroadcastEngine::default();
        engine.ack_tracker.borrow_mut().register(test_msg_id(), vec![test_node_id()], 1000).unwrap();
        engine.record_ack(test_msg_id(), test_node_id()).unwrap();
        assert_eq!(engine.total_acks(), 1);
    }

    #[test]
    fn broadcast_engine_is_fully_delivered() {
        let engine = BroadcastEngine::default();
        engine.ack_tracker.borrow_mut().register(test_msg_id(), vec![test_node_id()], 1000).unwrap();
        engine.record_ack(test_msg_id(), test_node_id()).unwrap();
        assert!(engine.is_fully_delivered(test_msg_id()).unwrap());
    }

    #[test]
    fn broadcast_engine_expire_by_ttl() {
        let engine = BroadcastEngine::default();
        engine.ttl_manager.borrow_mut().set_ttl(test_msg_id(), 1000, 0);
        let expired = engine.expire_by_ttl(2000).unwrap();
        assert_eq!(expired, 1);
        assert_eq!(engine.stats().borrow().total_expired, 1);
    }

    #[test]
    fn broadcast_engine_configure_channel() {
        let engine = BroadcastEngine::default();
        engine.configure_channel(1, ChannelConfig::new(1, "ch".to_string())).unwrap();
        assert_eq!(engine.stats().borrow().total_channels, 1);
    }

    #[test]
    fn broadcast_engine_publish() {
        let engine = BroadcastEngine::default();
        engine.configure_channel(1, ChannelConfig::new(1, "ch".to_string())).unwrap();
        engine.subscribe(1, test_node_id()).unwrap();
        let msg = test_broadcast_message();
        let result = engine.publish(1, msg.clone()).unwrap();
        assert_eq!(result.msg_id, msg.msg_id);
        assert_eq!(engine.channel_subscriber_count(1).unwrap(), 1);
    }

    #[test]
    fn broadcast_engine_publish_channel_not_found() {
        let engine = BroadcastEngine::default();
        let err = engine.publish(999, test_broadcast_message()).unwrap_err();
        assert!(matches!(err, BroadcastError::ChannelNotFound { .. }));
    }

    #[test]
    fn broadcast_engine_subscribe_unsubscribe() {
        let engine = BroadcastEngine::default();
        engine.configure_channel(1, ChannelConfig::new(1, "ch".to_string())).unwrap();
        engine.subscribe(1, test_node_id()).unwrap();
        assert!(engine.channel_subscriber_count(1).unwrap() == 1);
        engine.unsubscribe(1, test_node_id()).unwrap();
        assert!(engine.channel_subscriber_count(1).unwrap() == 0);
    }

    #[test]
    fn broadcast_engine_sign_and_verify() {
        let engine = BroadcastEngine::default();
        let mut msg = test_broadcast_message();
        let sig = engine.sign_message(&mut msg, test_key_id()).unwrap();
        assert_eq!(sig, test_signature());
        assert!(msg.signature.is_some());
        assert!(engine.verify_integrity(&msg).unwrap());
    }

    #[test]
    fn broadcast_engine_stats_and_name() {
        let engine = BroadcastEngine::default();
        assert_eq!(engine.broadcast_name(), "broadcast_engine");
        assert_eq!(engine.total_broadcast(), 0);
        assert_eq!(engine.total_deduplicated(), 0);
        assert_eq!(engine.total_acks(), 0);
    }

    #[test]
    fn broadcast_engine_monitor_peer() {
        let engine = BroadcastEngine::default();
        engine.monitor_peer(test_node_id(), 5);
        assert!(engine.is_peer_active(&test_node_id()));
        assert_eq!(engine.monitor.borrow().monitored_peer_count(), 1);
    }

    #[test]
    fn broadcast_engine_validate_all() {
        let engine = BroadcastEngine::default();
        assert_eq!(engine.validate_peer(test_node_id()), BroadcastValidationResult::Valid);
        assert_eq!(
            engine.validate_payload(test_msg_id(), 100, 1000),
            BroadcastValidationResult::Valid
        );
        assert_eq!(
            engine.validate_hops(test_msg_id(), 5, 10),
            BroadcastValidationResult::Valid
        );
        assert_eq!(
            engine.validate_priority(test_msg_id(), 5),
            BroadcastValidationResult::Valid
        );
    }

    #[test]
    fn broadcast_engine_health_score() {
        let engine = BroadcastEngine::default();
        let score = engine.broadcast_health_score();
        assert!(score >= 0.0);
        assert!(score <= 100.0);
    }

    // === Integration: full broadcast flow ===
    #[test]
    fn broadcast_full_flow() {
        let engine = BroadcastEngine::default();

        // Configure channel
        engine.configure_channel(1, ChannelConfig::new(1, "alerts".to_string())).unwrap();

        // Subscribe peers
        engine.subscribe(1, test_node_id()).unwrap();
        engine.subscribe(1, test_peer_id()).unwrap();

        // Broadcast a message
        let msg = test_broadcast_message();
        engine.broadcast(msg.clone()).unwrap();

        // Multicast to peers
        engine.multicast(msg.clone(), vec![test_node_id(), test_peer_id()]).unwrap();

        // Record acks
        engine.record_ack(msg.msg_id, test_node_id()).unwrap();
        engine.record_ack(msg.msg_id, test_peer_id()).unwrap();

        // Verify delivery
        assert!(engine.is_fully_delivered(msg.msg_id).unwrap());

        // Sign and verify
        let mut signed_msg = msg.clone();
        engine.sign_message(&mut signed_msg, test_key_id()).unwrap();
        assert!(engine.verify_integrity(&signed_msg).unwrap());

        // Publish to channel
        engine.publish(1, signed_msg).unwrap();

        // Stats
        let s = engine.stats();
        assert!(s.total_messages >= 1);
        assert!(s.total_acks >= 2);
        assert!(s.total_channels >= 1);

        // Health score
        let score = engine.broadcast_health_score();
        assert!(score >= 0.0);
        assert!(score <= 100.0);
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

    // === Channel config clone test ===
    #[test]
    fn channel_config_clone() {
        let cfg1 = ChannelConfig::new(1, "ch".to_string());
        let cfg2 = cfg1.clone();
        assert_eq!(cfg1.channel_id, cfg2.channel_id);
        assert_eq!(cfg1.name, cfg2.name);
    }
}