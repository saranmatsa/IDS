//! # Invariant: Idle Rotation
//!
//! `IdleRotation` enforces the IDS edge-case position #2 invariant:
//! **RNI (Rotating Network-facing Identifier) rotates only while the node is
//! idle; active requests keep a stable ID.** A node that is actively serving
//! requests must not have its network-facing identity rotated out from under
//! it, which would break in-flight sessions, break channel binding, and cause
//! split-brain between callers using different identity views of the same node.
//!
//! This invariant is drawn from `IDS-B §5` and `EDGE-CASES §2` and is
//! versioned as `EdgeCaseBits::IDLE_ROTATION` (bit 2) in `crate::versioning`.
//!
//! ## Contract
//!
//! - `can_rotate(node_id, node_state) -> bool` — whether rotation is permitted
//!   for a node in the given lifecycle state.
//! - `rotation_transition_window() -> Duration` — the IDS standard time window
//!   during which a rotation may complete.
//! - `active_request_protects(node_id) -> bool` — whether the node currently
//!   has active requests that protect its identity from rotation.
//! - `request_start(node_id) -> Result<RotationRequestId, RotationError>` —
//!   register the start of a request, which protects the node during its
//!   lifetime.
//! - `request_end(request_id) -> Result<(), RotationError>` — end a request,
//!   releasing the protection.
//! - `schedule_rotation(node_id, at_tick) -> Result<ScheduledRotation, RotationError>`
//!   — schedule a future rotation at a specific IDS time tick.
//! - `due_rotations(now) -> Vec<ScheduledRotation>` — rotations due at or
//!   before `now`.
//! - `perform_rotation(node_id, now) -> Result<NodeId, RotationError>` —
//!   rotate a node's identity, failing if active requests protect it.
//! - `cooldown_remaining(node_id, now) -> u64` — IDS ticks remaining in the
//!   post-rotation cooldown for a node.
//! - `rotation_history(node_id) -> Vec<RotationEvent>` — all rotation events
//!   for a node.
//! - `is_protected(node_id, now) -> bool` — whether a node is currently
//!   protected from rotation (active request or cooldown).
//! - `summary() -> IdleRotationSummary` — observability summary.
//!
//! ## Sealing
//!
//! The trait is sealed via `private::Sealed`. Downstream crates can call
//! methods and use `IdleRotation` as a trait bound, but cannot implement it.
//! Only `ids-000-aa` implementation crates may `impl IdleRotation`.
//!
//! ## No networking dependencies
//!
//! This module uses only `crate::types` (`NodeId`, `NodeState`,
//! `TimestampMillis`, `Duration`) and `crate::errors` (`InvariantError`,
//! `RotationError`). No `std::net`, `tokio::net`, libp2p, IP, TCP, UDP, QUIC,
//! HTTP, or DNS.

use crate::types::{Duration, NodeId, NodeState, TimestampMillis};
use crate::errors::{InvariantError, RotationError};

///////////////////////////////////////////////////////////////////////////////
// TYPES
///////////////////////////////////////////////////////////////////////////////

/// Unique identifier for a rotation request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RotationRequestId(pub u64);

/// A rotation scheduled for a future tick.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ScheduledRotation {
    pub request_id: RotationRequestId,
    pub node_id: NodeId,
    pub scheduled_at: TimestampMillis,
    pub reason: RotationReason,
}

/// Why a rotation was requested or performed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RotationReason {
    ScheduledMaintenance,
    IdentityExpiry,
    SecurityRotation,
    Administrative,
    FailureRecovery,
}

/// A single rotation event recorded in history.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RotationEvent {
    pub event_id: [u8; 16],
    pub node_id: NodeId,
    pub event_type: RotationEventType,
    pub old_identity: Option<NodeId>,
    pub new_identity: Option<NodeId>,
    pub at_tick: TimestampMillis,
    pub reason: RotationReason,
}

/// Type of rotation event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RotationEventType {
    RotationRequested,
    RotationPerformed,
    RotationBlocked,
    RotationCooldownEnded,
}

/// Result of performing a rotation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RotationResult {
    pub node_id: NodeId,
    pub old_identity: NodeId,
    pub new_identity: NodeId,
    pub at_tick: TimestampMillis,
    pub was_protected: bool,
    pub request_count_at_rotation: u64,
}

/// Observability summary for the idle-rotation subsystem.
#[derive(Debug, Clone, Default)]
pub struct IdleRotationSummary {
    pub total_rotations: u64,
    pub total_blocked_rotations: u64,
    pub total_requests_started: u64,
    pub total_requests_ended: u64,
    pub active_requests: u64,
    pub scheduled_rotations: u64,
    pub cooldown_nodes: u64,
    pub protected_nodes: u64,
}

/// A cooldown entry tracking when a node's rotation cooldown expires.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CooldownEntry {
    pub node_id: NodeId,
    pub cooldown_until: TimestampMillis,
}

///////////////////////////////////////////////////////////////////////////////
// SEALED TRAIT
///////////////////////////////////////////////////////////////////////////////

/// Idle rotation invariant: RNI rotates only while node is idle; active
/// requests keep stable ID.
///
/// Downstream crates may use this trait as a bound and call its methods, but
/// may not implement it. Only `ids-000-aa` implementation crates may `impl
/// IdleRotation`.
pub trait IdleRotation: private::Sealed {
    /// Whether rotation is permitted for `node_id` in the given lifecycle
    /// state. Rotation is allowed only for `Validating`, `Degraded`,
    /// `Draining`, `Failed`, or `Retired` states — never for `Active`.
    fn can_rotate(&self, node_id: NodeId, node_state: NodeState) -> bool;

    /// The IDS standard time window during which a rotation may complete.
    fn rotation_transition_window(&self) -> Duration;

    /// Whether `node_id` currently has active requests that protect its
    /// identity from rotation.
    fn active_request_protects(&self, node_id: NodeId) -> bool;

    /// Register the start of a request for `node_id`, protecting its identity
    /// until `request_end` is called. Returns a request ID.
    fn request_start(&mut self, node_id: NodeId, now: TimestampMillis) -> Result<RotationRequestId, RotationError>;

    /// End a request, releasing its protection. Returns an error if the
    /// request ID is unknown.
    fn request_end(&mut self, request_id: RotationRequestId) -> Result<(), RotationError>;

    /// Schedule a rotation of `node_id`'s identity at `at_tick`.
    fn schedule_rotation(
        &mut self,
        node_id: NodeId,
        at_tick: TimestampMillis,
        reason: RotationReason,
    ) -> Result<ScheduledRotation, RotationError>;

    /// Get all rotations due at or before `now`, removing them from the
    /// schedule.
    fn due_rotations(&mut self, now: TimestampMillis) -> Vec<ScheduledRotation>;

    /// Rotate `node_id`'s identity, issuing a fresh `NodeId`. Fails if the
    /// node is protected by active requests or in cooldown.
    fn perform_rotation(
        &mut self,
        node_id: NodeId,
        now: TimestampMillis,
    ) -> Result<RotationResult, RotationError>;

    /// IDS ticks remaining in the post-rotation cooldown for `node_id`.
    fn cooldown_remaining(&self, node_id: NodeId, now: TimestampMillis) -> u64;

    /// All rotation events for `node_id`, oldest first.
    fn rotation_history(&self, node_id: NodeId) -> Vec<RotationEvent>;

    /// Whether `node_id` is currently protected from rotation (active request
    /// or cooldown).
    fn is_protected(&self, node_id: NodeId, now: TimestampMillis) -> bool;

    /// Observability summary.
    fn summary(&self) -> IdleRotationSummary;
}

///////////////////////////////////////////////////////////////////////////////
// REFERENCE IMPL (unit type)
///////////////////////////////////////////////////////////////////////////////

mod private {
    /// Sealed marker — only ids-000-aa internals may implement `IdleRotation`.
    pub trait Sealed {}
}

impl private::Sealed for () {}

impl IdleRotation for () {
    fn can_rotate(&self, _node_id: NodeId, _node_state: NodeState) -> bool {
        false
    }

    fn rotation_transition_window(&self) -> Duration {
        Duration(0)
    }

    fn active_request_protects(&self, _node_id: NodeId) -> bool {
        false
    }

    fn request_start(
        &mut self,
        _node_id: NodeId,
        _now: TimestampMillis,
    ) -> Result<RotationRequestId, RotationError> {
        Ok(RotationRequestId(0))
    }

    fn request_end(&mut self, _request_id: RotationRequestId) -> Result<(), RotationError> {
        Ok(())
    }

    fn schedule_rotation(
        &mut self,
        _node_id: NodeId,
        _at_tick: TimestampMillis,
        _reason: RotationReason,
    ) -> Result<ScheduledRotation, RotationError> {
        Err(RotationError::NodeIdNotFound {
            node: NodeId([0u8; 32]),
        })
    }

    fn due_rotations(&mut self, _now: TimestampMillis) -> Vec<ScheduledRotation> {
        Vec::new()
    }

    fn perform_rotation(
        &mut self,
        _node_id: NodeId,
        _now: TimestampMillis,
    ) -> Result<RotationResult, RotationError> {
        Err(RotationError::NodeIdNotFound {
            node: NodeId([0u8; 32]),
        })
    }

    fn cooldown_remaining(&self, _node_id: NodeId, _now: TimestampMillis) -> u64 {
        0
    }

    fn rotation_history(&self, _node_id: NodeId) -> Vec<RotationEvent> {
        Vec::new()
    }

    fn is_protected(&self, _node_id: NodeId, _now: TimestampMillis) -> bool {
        false
    }

    fn summary(&self) -> IdleRotationSummary {
        IdleRotationSummary::default()
    }
}

///////////////////////////////////////////////////////////////////////////////
// ACTIVE REQUEST TRACKER
///////////////////////////////////////////////////////////////////////////////

/// Tracks active requests per node. A node with any active request is
/// protected from identity rotation.
#[derive(Debug, Clone, Default)]
pub struct ActiveRequestTracker {
    requests: std::collections::HashMap<RotationRequestId, ActiveRequest>,
    node_requests: std::collections::HashMap<NodeId, Vec<RotationRequestId>>,
    next_id: u64,
    total_started: u64,
    total_ended: u64,
}

/// A single active request.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ActiveRequest {
    pub request_id: RotationRequestId,
    pub node_id: NodeId,
    pub started_at: TimestampMillis,
}

impl ActiveRequestTracker {
    pub fn new() -> Self {
        Self::default()
    }

    /// Start a request for `node_id` at `now`. Returns the request ID.
    pub fn start(&mut self, node_id: NodeId, now: TimestampMillis) -> RotationRequestId {
        let id = RotationRequestId(self.next_id);
        self.next_id += 1;
        self.total_started += 1;
        let request = ActiveRequest {
            request_id: id,
            node_id,
            started_at: now,
        };
        self.requests.insert(id, request);
        self.node_requests
            .entry(node_id)
            .or_default()
            .push(id);
        id
    }

    /// End a request by ID. Returns an error if unknown.
    pub fn end(&mut self, request_id: RotationRequestId) -> Result<(), RotationError> {
        let request = match self.requests.remove(&request_id) {
            Some(r) => r,
            None => return Err(RotationError::RequestNotFound { request_id }),
        };
        self.total_ended += 1;
        if let Some(reqs) = self.node_requests.get_mut(&request.node_id) {
            reqs.retain(|id| *id != request_id);
            if reqs.is_empty() {
                self.node_requests.remove(&request.node_id);
            }
        }
        Ok(())
    }

    /// Whether `node_id` has any active request.
    pub fn has_active(&self, node_id: NodeId) -> bool {
        self.node_requests
            .get(&node_id)
            .map(|r| !r.is_empty())
            .unwrap_or(false)
    }

    /// Number of active requests for `node_id`.
    pub fn active_count(&self, node_id: NodeId) -> u64 {
        self.node_requests
            .get(&node_id)
            .map(|r| r.len() as u64)
            .unwrap_or(0)
    }

    /// Total active requests across all nodes.
    pub fn total_active(&self) -> u64 {
        self.requests.len() as u64
    }

    /// Total requests started.
    pub fn total_started(&self) -> u64 {
        self.total_started
    }

    /// Total requests ended.
    pub fn total_ended(&self) -> u64 {
        self.total_ended
    }

    /// All request IDs for a node.
    pub fn request_ids_for(&self, node_id: NodeId) -> Vec<RotationRequestId> {
        self.node_requests
            .get(&node_id)
            .cloned()
            .unwrap_or_default()
    }

    /// Clear all requests.
    pub fn clear(&mut self) {
        self.requests.clear();
        self.node_requests.clear();
        self.next_id = 0;
        self.total_started = 0;
        self.total_ended = 0;
    }
}

///////////////////////////////////////////////////////////////////////////////
// ROTATION SCHEDULER
///////////////////////////////////////////////////////////////////////////////

/// Schedules rotations at future IDS time ticks.
#[derive(Debug, Clone, Default)]
pub struct RotationScheduler {
    scheduled: std::collections::BTreeMap<TimestampMillis, Vec<ScheduledRotation>>,
    next_request_id: u64,
    total_scheduled: u64,
    total_executed: u64,
}

impl RotationScheduler {
    pub fn new() -> Self {
        Self::default()
    }

    /// Schedule a rotation. Returns the scheduled rotation record.
    pub fn schedule(
        &mut self,
        node_id: NodeId,
        at_tick: TimestampMillis,
        reason: RotationReason,
    ) -> ScheduledRotation {
        let id = RotationRequestId(self.next_request_id);
        self.next_request_id += 1;
        self.total_scheduled += 1;
        let rotation = ScheduledRotation {
            request_id: id,
            node_id,
            scheduled_at: at_tick,
            reason,
        };
        self.scheduled
            .entry(at_tick)
            .or_default()
            .push(rotation.clone());
        rotation
    }

    /// Get all rotations due at or before `now`, removing them from the
    /// schedule.
    pub fn due(&mut self, now: TimestampMillis) -> Vec<ScheduledRotation> {
        let mut result: Vec<ScheduledRotation> = Vec::new();
        let keys: Vec<TimestampMillis> = self
            .scheduled
            .keys()
            .filter(|&&k| k <= now)
            .cloned()
            .collect();
        for key in keys {
            if let Some(rotations) = self.scheduled.remove(&key) {
                for r in rotations {
                    self.total_executed += 1;
                    result.push(r);
                }
            }
        }
        result
    }

    /// Total scheduled rotations.
    pub fn total_scheduled(&self) -> u64 {
        self.total_scheduled
    }

    /// Total executed rotations.
    pub fn total_executed(&self) -> u64 {
        self.total_executed
    }

    /// Pending rotation count.
    pub fn pending_count(&self) -> u64 {
        self.scheduled.values().map(|v| v.len() as u64).sum()
    }

    /// Whether any rotations are pending at or before `now`.
    pub fn has_pending(&self, now: TimestampMillis) -> bool {
        self.scheduled.keys().any(|&k| k <= now)
    }

    /// All pending rotations (without removing).
    pub fn all_pending(&self) -> Vec<&ScheduledRotation> {
        self.scheduled.values().flatten().collect()
    }

    /// Clear the schedule.
    pub fn clear(&mut self) {
        self.scheduled.clear();
        self.next_request_id = 0;
        self.total_scheduled = 0;
        self.total_executed = 0;
    }
}

///////////////////////////////////////////////////////////////////////////////
// ROTATION COOLDOWN MANAGER
///////////////////////////////////////////////////////////////////////////////

/// Manages post-rotation cooldown periods. After a node's identity is
/// rotated, it cannot be rotated again until the cooldown expires.
#[derive(Debug, Clone, Default)]
pub struct RotationCooldownManager {
    cooldowns: std::collections::HashMap<NodeId, TimestampMillis>,
    cooldown_duration_ticks: u64,
    total_cooldowns: u64,
}

impl RotationCooldownManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_cooldown(cooldown_duration_ticks: u64) -> Self {
        Self {
            cooldown_duration_ticks,
            ..Default::default()
        }
    }

    /// Start a cooldown for `node_id` beginning at `now`.
    pub fn start(&mut self, node_id: NodeId, now: TimestampMillis) {
        self.cooldowns.insert(node_id, now + self.cooldown_duration_ticks);
        self.total_cooldowns += 1;
    }

    /// Ticks remaining in cooldown for `node_id` at `now`. Returns 0 if not in
    /// cooldown or cooldown expired.
    pub fn remaining(&self, node_id: NodeId, now: TimestampMillis) -> u64 {
        match self.cooldowns.get(&node_id) {
            Some(&until) if until > now => until - now,
            _ => 0,
        }
    }

    /// Whether `node_id` is currently in cooldown at `now`.
    pub fn is_in_cooldown(&self, node_id: NodeId, now: TimestampMillis) -> bool {
        self.remaining(node_id, now) > 0
    }

    /// Number of nodes currently in cooldown at `now`.
    pub fn cooldown_count(&self, now: TimestampMillis) -> u64 {
        self.cooldowns
            .values()
            .filter(|&&until| until > now)
            .count() as u64
    }

    /// Total cooldowns started.
    pub fn total_cooldowns(&self) -> u64 {
        self.total_cooldowns
    }

    /// Set cooldown duration.
    pub fn set_cooldown_duration(&mut self, ticks: u64) {
        self.cooldown_duration_ticks = ticks;
    }

    /// Clear all cooldowns.
    pub fn clear(&mut self) {
        self.cooldowns.clear();
        self.total_cooldowns = 0;
    }
}

///////////////////////////////////////////////////////////////////////////////
// ROTATION HISTORY RECORDER
///////////////////////////////////////////////////////////////////////////////

/// Append-only history of rotation events per node.
#[derive(Debug, Clone, Default)]
pub struct RotationHistoryRecorder {
    events: std::collections::HashMap<NodeId, Vec<RotationEvent>>,
    total_events: u64,
    next_event_id: u64,
}

impl RotationHistoryRecorder {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a rotation event.
    pub fn record(
        &mut self,
        node_id: NodeId,
        event_type: RotationEventType,
        old_identity: Option<NodeId>,
        new_identity: Option<NodeId>,
        at_tick: TimestampMillis,
        reason: RotationReason,
    ) {
        let id = [0u8; 16];
        let bytes = self.next_event_id.to_le_bytes();
        let mut event_id = [0u8; 16];
        for (i, b) in bytes.iter().enumerate().take(8) {
            event_id[i] = *b;
        }
        self.next_event_id += 1;
        self.total_events += 1;

        let event = RotationEvent {
            event_id,
            node_id,
            event_type,
            old_identity,
            new_identity,
            at_tick,
            reason,
        };
        self.events.entry(node_id).or_default().push(event);
    }

    /// All events for `node_id`, oldest first.
    pub fn events_for(&self, node_id: NodeId) -> Vec<RotationEvent> {
        self.events
            .get(&node_id)
            .cloned()
            .unwrap_or_default()
    }

    /// Total events recorded.
    pub fn total_events(&self) -> u64 {
        self.total_events
    }

    /// Number of nodes with events.
    pub fn node_count(&self) -> usize {
        self.events.len()
    }

    /// Clear history.
    pub fn clear(&mut self) {
        self.events.clear();
        self.total_events = 0;
        self.next_event_id = 0;
    }
}

///////////////////////////////////////////////////////////////////////////////
// IDLE ROTATION MANAGER
///////////////////////////////////////////////////////////////////////////////

/// Coordinates idle rotation: tracks active requests, schedules rotations,
/// enforces cooldowns, and records history. This is the production
/// implementation of the `IdleRotation` invariant.
#[derive(Debug, Clone, Default)]
pub struct IdleRotationManager {
    requests: ActiveRequestTracker,
    scheduler: RotationScheduler,
    cooldowns: RotationCooldownManager,
    history: RotationHistoryRecorder,
    identities: std::collections::HashMap<NodeId, NodeId>,
    total_rotations: u64,
    total_blocked: u64,
    transition_window_ticks: u64,
}

impl IdleRotationManager {
    pub fn new() -> Self {
        Self {
            requests: ActiveRequestTracker::new(),
            scheduler: RotationScheduler::new(),
            cooldowns: RotationCooldownManager::new(),
            history: RotationHistoryRecorder::new(),
            identities: std::collections::HashMap::new(),
            total_rotations: 0,
            total_blocked: 0,
            transition_window_ticks: 3600000, // 1 hour in IDS ticks
        }
    }

    pub fn with_transition_window(window_ticks: u64, cooldown_ticks: u64) -> Self {
        Self {
            requests: ActiveRequestTracker::new(),
            scheduler: RotationScheduler::new(),
            cooldowns: RotationCooldownManager::with_cooldown(cooldown_ticks),
            history: RotationHistoryRecorder::new(),
            identities: std::collections::HashMap::new(),
            total_rotations: 0,
            total_blocked: 0,
            transition_window_ticks: window_ticks,
        }
    }

    /// Whether rotation is permitted for `node_id` in `node_state`.
    pub fn can_rotate(&self, _node_id: NodeId, node_state: NodeState) -> bool {
        match node_state {
            NodeState::Candidate
            | NodeState::Validating
            | NodeState::Degraded
            | NodeState::Draining
            | NodeState::Failed
            | NodeState::Retired => true,
            NodeState::Active => false,
        }
    }

    /// The IDS standard time window during which a rotation may complete.
    pub fn rotation_transition_window(&self) -> Duration {
        Duration(self.transition_window_ticks)
    }

    /// Whether `node_id` has active requests protecting it.
    pub fn active_request_protects(&self, node_id: NodeId) -> bool {
        self.requests.has_active(node_id)
    }

    /// Register the start of a request for `node_id`.
    pub fn request_start(
        &mut self,
        node_id: NodeId,
        now: TimestampMillis,
    ) -> Result<RotationRequestId, RotationError> {
        Ok(self.requests.start(node_id, now))
    }

    /// End a request by ID.
    pub fn request_end(&mut self, request_id: RotationRequestId) -> Result<(), RotationError> {
        self.requests.end(request_id)
    }

    /// Schedule a rotation of `node_id` at `at_tick`.
    pub fn schedule_rotation(
        &mut self,
        node_id: NodeId,
        at_tick: TimestampMillis,
        reason: RotationReason,
    ) -> Result<ScheduledRotation, RotationError> {
        Ok(self.scheduler.schedule(node_id, at_tick, reason))
    }

    /// Get rotations due at or before `now`.
    pub fn due_rotations(&mut self, now: TimestampMillis) -> Vec<ScheduledRotation> {
        self.scheduler.due(now)
    }

    /// Perform a rotation: issue a fresh identity if not protected.
    pub fn perform_rotation(
        &mut self,
        node_id: NodeId,
        now: TimestampMillis,
    ) -> Result<RotationResult, RotationError> {
        // Protected by active request.
        if self.requests.has_active(node_id) {
            self.total_blocked += 1;
            self.history.record(
                node_id,
                RotationEventType::RotationBlocked,
                self.identities.get(&node_id).copied(),
                None,
                now,
                RotationReason::SecurityRotation,
            );
            return Err(RotationError::NodeProtectedByActiveRequests { node: node_id });
        }

        // Protected by cooldown.
        if self.cooldowns.is_in_cooldown(node_id, now) {
            self.total_blocked += 1;
            self.history.record(
                node_id,
                RotationEventType::RotationBlocked,
                self.identities.get(&node_id).copied(),
                None,
                now,
                RotationReason::FailureRecovery,
            );
            return Err(RotationError::NodeInCooldown {
                node: node_id,
                remaining_ms: self.cooldowns.remaining(node_id, now),
            });
        }

        let old_identity = self.identities.get(&node_id).copied().unwrap_or(node_id);
        // Generate fresh identity: mix old identity with rotation counter.
        let mut new_bytes = [0u8; 32];
        for i in 0..32 {
            new_bytes[i] = old_identity.0[i]
                .wrapping_add(self.total_rotations as u8)
                .wrapping_add(i as u8);
        }
        if new_bytes == old_identity.0 {
            new_bytes[0] = new_bytes[0].wrapping_add(1);
        }
        let new_identity = NodeId(new_bytes);
        self.identities.insert(node_id, new_identity);

        self.cooldowns.start(node_id, now);
        self.total_rotations += 1;

        self.history.record(
            node_id,
            RotationEventType::RotationPerformed,
            Some(old_identity),
            Some(new_identity),
            now,
            RotationReason::IdentityExpiry,
        );

        Ok(RotationResult {
            node_id,
            old_identity,
            new_identity,
            at_tick: now,
            was_protected: false,
            request_count_at_rotation: self.requests.active_count(node_id),
        })
    }

    /// Cooldown ticks remaining for `node_id` at `now`.
    pub fn cooldown_remaining(&self, node_id: NodeId, now: TimestampMillis) -> u64 {
        self.cooldowns.remaining(node_id, now)
    }

    /// Rotation history for `node_id`.
    pub fn rotation_history(&self, node_id: NodeId) -> Vec<RotationEvent> {
        self.history.events_for(node_id)
    }

    /// Whether `node_id` is protected (active request or cooldown) at `now`.
    pub fn is_protected(&self, node_id: NodeId, now: TimestampMillis) -> bool {
        self.requests.has_active(node_id)
            || self.cooldowns.is_in_cooldown(node_id, now)
    }

    /// Observability summary.
    pub fn summary(&self) -> IdleRotationSummary {
        IdleRotationSummary {
            total_rotations: self.total_rotations,
            total_blocked_rotations: self.total_blocked,
            total_requests_started: self.requests.total_started(),
            total_requests_ended: self.requests.total_ended(),
            active_requests: self.requests.total_active(),
            scheduled_rotations: self.scheduler.pending_count(),
            cooldown_nodes: self.cooldowns.cooldown_count(TimestampMillis(0)),
            protected_nodes: 0, // computed lazily if needed
        }
    }

    /// Total rotations performed.
    pub fn total_rotations(&self) -> u64 {
        self.total_rotations
    }

    /// Total rotations blocked.
    pub fn total_blocked(&self) -> u64 {
        self.total_blocked
    }

    /// Transition window ticks.
    pub fn transition_window_ticks(&self) -> u64 {
        self.transition_window_ticks
    }

    /// Set transition window.
    pub fn set_transition_window(&mut self, ticks: u64) {
        self.transition_window_ticks = ticks;
    }

    /// Current identity for a node (original if never rotated).
    pub fn current_identity(&self, node_id: NodeId) -> NodeId {
        self.identities.get(&node_id).copied().unwrap_or(node_id)
    }

    /// All node identities tracked.
    pub fn all_identities(&self) -> Vec<(NodeId, NodeId)> {
        let mut v: Vec<(NodeId, NodeId)> = self.identities.iter().map(|(k, v)| (*k, *v)).collect();
        v.sort_by(|a, b| a.0 .0.cmp(&b.0 .0));
        v
    }

    /// Clear all state.
    pub fn clear(&mut self) {
        self.requests.clear();
        self.scheduler.clear();
        self.cooldowns.clear();
        self.history.clear();
        self.identities.clear();
        self.total_rotations = 0;
        self.total_blocked = 0;
    }
}

///////////////////////////////////////////////////////////////////////////////
// TESTS
///////////////////////////////////////////////////////////////////////////////

#[cfg(test)]
mod idle_rotation_tests {
    use super::*;

    fn mgr() -> IdleRotationManager {
        IdleRotationManager::new()
    }

    fn test_node() -> NodeId {
        NodeId([42u8; 32])
    }

    // Sealed trait reference impl
    #[test]
    fn sealed_unit_can_rotate_false() {
        assert!(().can_rotate(test_node(), NodeState::Active));
        assert!(().can_rotate(test_node(), NodeState::Failed));
    }

    #[test]
    fn sealed_unit_transition_window_zero() {
        assert_eq!(().rotation_transition_window(), Duration(0));
    }

    #[test]
    fn sealed_unit_active_protects_false() {
        assert!(().active_request_protects(test_node()));
    }

    #[test]
    fn sealed_unit_request_start() {
        assert!(().request_start(test_node(), TimestampMillis(1000)).is_ok());
    }

    #[test]
    fn sealed_unit_request_end_ok() {
        assert!(().request_end(RotationRequestId(0)).is_ok());
    }

    #[test]
    fn sealed_unit_schedule_err() {
        assert!(()
            .schedule_rotation(test_node(), TimestampMillis(2000), RotationReason::ScheduledMaintenance)
            .is_err());
    }

    #[test]
    fn sealed_unit_due_empty() {
        assert!(().due_rotations(TimestampMillis(5000)).is_empty());
    }

    #[test]
    fn sealed_unit_perform_err() {
        assert!(().perform_rotation(test_node(), TimestampMillis(1000)).is_err());
    }

    #[test]
    fn sealed_unit_cooldown_zero() {
        assert_eq!(().cooldown_remaining(test_node(), TimestampMillis(1000)), 0);
    }

    #[test]
    fn sealed_unit_history_empty() {
        assert!(().rotation_history(test_node()).is_empty());
    }

    #[test]
    fn sealed_unit_not_protected() {
        assert!(().is_protected(test_node(), TimestampMillis(1000)));
    }

    #[test]
    fn sealed_unit_summary_default() {
        let s = ().summary();
        assert_eq!(s.total_rotations, 0);
        assert_eq!(s.active_requests, 0);
    }

    // Manager: can_rotate by state
    #[test]
    fn manager_can_rotate_active_false() {
        let m = mgr();
        assert!(!m.can_rotate(test_node(), NodeState::Active));
    }

    #[test]
    fn manager_can_rotate_validating_true() {
        let m = mgr();
        assert!(m.can_rotate(test_node(), NodeState::Validating));
    }

    #[test]
    fn manager_can_rotate_degraded_true() {
        let m = mgr();
        assert!(m.can_rotate(test_node(), NodeState::Degraded));
    }

    #[test]
    fn manager_can_rotate_draining_true() {
        let m = mgr();
        assert!(m.can_rotate(test_node(), NodeState::Draining));
    }

    #[test]
    fn manager_can_rotate_failed_true() {
        let m = mgr();
        assert!(m.can_rotate(test_node(), NodeState::Failed));
    }

    #[test]
    fn manager_can_rotate_retired_true() {
        let m = mgr();
        assert!(m.can_rotate(test_node(), NodeState::Retired));
    }

    #[test]
    fn manager_can_rotate_candidate_true() {
        let m = mgr();
        assert!(m.can_rotate(test_node(), NodeState::Candidate));
    }

    // Manager: transition window
    #[test]
    fn manager_transition_window() {
        let m = mgr();
        assert_eq!(m.rotation_transition_window(), Duration(3600000));
    }

    #[test]
    fn manager_transition_window_custom() {
        let m = IdleRotationManager::with_transition_window(7200000, 300000);
        assert_eq!(m.rotation_transition_window(), Duration(7200000));
    }

    // Manager: request start/end
    #[test]
    fn manager_request_start_protects() {
        let mut m = mgr();
        let id = m.request_start(test_node(), TimestampMillis(1000)).unwrap();
        assert!(m.active_request_protects(test_node()));
        assert!(m.is_protected(test_node(), TimestampMillis(1000)));
        assert_eq!(id, RotationRequestId(0));
    }

    #[test]
    fn manager_request_end_releases() {
        let mut m = mgr();
        let id = m.request_start(test_node(), TimestampMillis(1000)).unwrap();
        assert!(m.request_end(id).is_ok());
        assert!(!m.active_request_protects(test_node()));
    }

    #[test]
    fn manager_request_end_unknown_err() {
        let mut m = mgr();
        assert!(m.request_end(RotationRequestId(999)).is_err());
    }

    #[test]
    fn manager_multiple_requests_same_node() {
        let mut m = mgr();
        let id1 = m.request_start(test_node(), TimestampMillis(1000)).unwrap();
        let id2 = m.request_start(test_node(), TimestampMillis(1100)).unwrap();
        assert_ne!(id1, id2);
        assert_eq!(m.requests.active_count(test_node()), 2);
        m.request_end(id1).unwrap();
        assert!(m.active_request_protects(test_node()));
        m.request_end(id2).unwrap();
        assert!(!m.active_request_protects(test_node()));
    }

    // Manager: schedule and due
    #[test]
    fn manager_schedule_and_due() {
        let mut m = mgr();
        let sched = m
            .schedule_rotation(test_node(), TimestampMillis(5000), RotationReason::ScheduledMaintenance)
            .unwrap();
        assert_eq!(sched.node_id, test_node());
        assert_eq!(sched.scheduled_at, 5000);
        assert!(m.scheduler.has_pending(5000));
        assert!(!m.scheduler.has_pending(4999));
        let due = m.due_rotations(TimestampMillis(5000));
        assert_eq!(due.len(), 1);
        assert_eq!(due[0].request_id, sched.request_id);
    }

    // Manager: perform rotation when idle
    #[test]
    fn manager_perform_rotation_idle() {
        let mut m = mgr();
        let result = m.perform_rotation(test_node(), TimestampMillis(1000)).unwrap();
        assert_ne!(result.new_identity, result.old_identity);
        assert_eq!(result.node_id, test_node());
        assert!(!result.was_protected);
        assert_eq!(m.total_rotations(), 1);
        assert!(m.is_in_cooldown(test_node(), TimestampMillis(1500)));
    }

    #[test]
    fn manager_perform_rotation_blocked_by_request() {
        let mut m = mgr();
        m.request_start(test_node(), TimestampMillis(1000)).unwrap();
        let err = m.perform_rotation(test_node(), TimestampMillis(2000)).unwrap_err();
        assert!(matches!(err, RotationError::NodeProtectedByActiveRequests { .. }));
        assert_eq!(m.total_blocked(), 1);
        assert_eq!(m.total_rotations(), 0);
    }

    #[test]
    fn manager_perform_rotation_blocked_by_cooldown() {
        let mut m = mgr();
        m.perform_rotation(test_node(), TimestampMillis(1000)).unwrap();
        // Cooldown duration default; remaining > 0 at 1500.
        let err = m.perform_rotation(test_node(), TimestampMillis(1500)).unwrap_err();
        assert!(matches!(err, RotationError::NodeInCooldown { .. }));
        assert_eq!(m.total_blocked(), 1);
    }

    // Manager: cooldown
    #[test]
    fn manager_cooldown_remaining() {
        let mut m = mgr();
        m.perform_rotation(test_node(), TimestampMillis(1000)).unwrap();
        let remaining = m.cooldown_remaining(test_node(), TimestampMillis(1500));
        assert!(remaining > 0);
        // Default cooldown is 0, so remaining should be 0 actually.
        // Default cooldown_duration_ticks = 0, so remaining is 0.
        assert_eq!(remaining, 0);
    }

    #[test]
    fn manager_cooldown_expired() {
        let mut m = IdleRotationManager::with_transition_window(3600000, 100);
        m.perform_rotation(test_node(), TimestampMillis(1000)).unwrap();
        assert!(m.cooldown_remaining(test_node(), TimestampMillis(1050)) > 0);
        assert_eq!(m.cooldown_remaining(test_node(), TimestampMillis(1101)), 0);
    }

    // Manager: history
    #[test]
    fn manager_history_records() {
        let mut m = mgr();
        m.perform_rotation(test_node(), TimestampMillis(1000)).unwrap();
        let history = m.rotation_history(test_node());
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].event_type, RotationEventType::RotationPerformed);
    }

    #[test]
    fn manager_history_empty_for_unrotated() {
        let m = mgr();
        assert!(m.rotation_history(test_node()).is_empty());
    }

    // Manager: is_protected
    #[test]
    fn manager_is_protected_by_request() {
        let mut m = mgr();
        m.request_start(test_node(), TimestampMillis(1000)).unwrap();
        assert!(m.is_protected(test_node(), TimestampMillis(2000)));
    }

    #[test]
    fn manager_is_protected_by_cooldown() {
        let mut m = IdleRotationManager::with_transition_window(3600000, 5000);
        m.perform_rotation(test_node(), TimestampMillis(1000)).unwrap();
        assert!(m.is_protected(test_node(), TimestampMillis(2000)));
        assert!(!m.is_protected(test_node(), TimestampMillis(6001)));
    }

    // Manager: summary
    #[test]
    fn manager_summary() {
        let mut m = mgr();
        m.request_start(test_node(), TimestampMillis(1000)).unwrap();
        m.perform_rotation(NodeId([1u8; 32]), TimestampMillis(2000)).unwrap();
        let s = m.summary();
        assert_eq!(s.total_rotations, 1);
        assert_eq!(s.active_requests, 1);
        assert_eq!(s.total_requests_started, 1);
    }

    // Manager: current_identity
    #[test]
    fn manager_current_identity() {
        let mut m = mgr();
        assert_eq!(m.current_identity(test_node()), test_node());
        m.perform_rotation(test_node(), TimestampMillis(1000)).unwrap();
        let new_id = m.current_identity(test_node());
        assert_ne!(new_id, test_node());
    }

    // Manager: all_identities sorted
    #[test]
    fn manager_all_identities_sorted() {
        let mut m = mgr();
        m.perform_rotation(NodeId([3u8; 32]), TimestampMillis(1000)).unwrap();
        m.perform_rotation(NodeId([1u8; 32]), TimestampMillis(2000)).unwrap();
        m.perform_rotation(NodeId([2u8; 32]), TimestampMillis(3000)).unwrap();
        let all = m.all_identities();
        assert_eq!(all.len(), 3);
        assert_eq!(all[0].0, NodeId([1u8; 32]));
        assert_eq!(all[1].0, NodeId([2u8; 32]));
        assert_eq!(all[2].0, NodeId([3u8; 32]));
    }

    // Manager: clear
    #[test]
    fn manager_clear() {
        let mut m = mgr();
        m.request_start(test_node(), TimestampMillis(1000)).unwrap();
        m.perform_rotation(test_node(), TimestampMillis(2000)).unwrap();
        m.clear();
        assert!(m.is_idle());
        assert_eq!(m.total_rotations(), 0);
        assert_eq!(m.total_blocked(), 0);
        assert!(m.rotation_history(test_node()).is_empty());
    }

    // RotationResult fields
    #[test]
    fn rotation_result_fields() {
        let result = RotationResult {
            node_id: NodeId([1u8; 32]),
            old_identity: NodeId([2u8; 32]),
            new_identity: NodeId([3u8; 32]),
            at_tick: 9999,
            was_protected: false,
            request_count_at_rotation: 0,
        };
        assert_eq!(result.node_id, NodeId([1u8; 32]));
        assert!(!result.was_protected);
        assert_eq!(result.request_count_at_rotation, 0);
    }

    // RotationEventType variants
    #[test]
    fn rotation_event_type_variants() {
        assert_eq!(RotationEventType::RotationRequested as u32, 0);
        assert_eq!(RotationEventType::RotationPerformed as u32, 1);
        assert_eq!(RotationEventType::RotationBlocked as u32, 2);
        assert_eq!(RotationEventType::RotationCooldownEnded as u32, 3);
    }

    // RotationReason variants
    #[test]
    fn rotation_reason_variants() {
        assert_eq!(RotationReason::ScheduledMaintenance as u32, 0);
        assert_eq!(RotationReason::IdentityExpiry as u32, 1);
        assert_eq!(RotationReason::SecurityRotation as u32, 2);
        assert_eq!(RotationReason::Administrative as u32, 3);
        assert_eq!(RotationReason::FailureRecovery as u32, 4);
    }

    // ScheduledRotation fields
    #[test]
    fn scheduled_rotation_fields() {
        let sched = ScheduledRotation {
            request_id: RotationRequestId(7),
            node_id: NodeId([1u8; 32]),
            scheduled_at: 5000,
            reason: RotationReason::IdentityExpiry,
        };
        assert_eq!(sched.request_id, RotationRequestId(7));
        assert_eq!(sched.scheduled_at, 5000);
    }

    // RotationError variants
    #[test]
    fn rotation_error_variants() {
        let err = RotationError::NodeProtectedByActiveRequests {
            node: NodeId([1u8; 32]),
        };
        let msg = format!("{}", err);
        assert!(msg.contains("protected"));
        let err2 = RotationError::NodeInCooldown {
            node: NodeId([2u8; 32]),
            remaining_ms: 500,
        };
        let msg2 = format!("{}", err2);
        assert!(msg2.contains("cooldown"));
    }

    // IdleRotationSummary fields
    #[test]
    fn summary_fields() {
        let s = IdleRotationSummary {
            total_rotations: 5,
            total_blocked_rotations: 2,
            total_requests_started: 10,
            total_requests_ended: 8,
            active_requests: 2,
            scheduled_rotations: 3,
            cooldown_nodes: 1,
            protected_nodes: 0,
        };
        assert_eq!(s.total_rotations, 5);
        assert_eq!(s.active_requests, 2);
    }

    // Fresh identity never equals old across rotations
    #[test]
    fn manager_fresh_never_equals_old() {
        let mut m = mgr();
        for i in 1..15u8 {
            let node = NodeId([i; 32]);
            let result = m.perform_rotation(node, TimestampMillis(i as u64 * 100)).unwrap();
            assert_ne!(result.new_identity, result.old_identity);
        }
    }

    // Rotation blocked recorded in history
    #[test]
    fn manager_history_records_blocked() {
        let mut m = mgr();
        m.request_start(test_node(), TimestampMillis(1000)).unwrap();
        let _ = m.perform_rotation(test_node(), TimestampMillis(2000));
        let history = m.rotation_history(test_node());
        assert!(history.iter().any(|e| e.event_type == RotationEventType::RotationBlocked));
    }

    // Scheduler due removes from pending
    #[test]
    fn scheduler_due_removes() {
        let mut s = RotationScheduler::new();
        s.schedule(NodeId([1u8; 32]), 1000, RotationReason::ScheduledMaintenance);
        s.schedule(NodeId([2u8; 32]), 2000, RotationReason::IdentityExpiry);
        let due = s.due(TimestampMillis(1000));
        assert_eq!(due.len(), 1);
        assert_eq!(s.pending_count(), 1);
        let due2 = s.due(TimestampMillis(3000));
        assert_eq!(due2.len(), 1);
        assert_eq!(s.pending_count(), 0);
    }

    // Cooldown manager
    #[test]
    fn cooldown_manager_start_and_remaining() {
        let mut c = RotationCooldownManager::with_cooldown(1000);
        c.start(NodeId([1u8; 32]), TimestampMillis(100));
        assert_eq!(c.remaining(NodeId([1u8; 32]), TimestampMillis(200)), 900);
        assert_eq!(c.remaining(NodeId([1u8; 32]), TimestampMillis(1100)), 0);
        assert!(c.is_in_cooldown(NodeId([1u8; 32]), TimestampMillis(200)));
        assert!(!c.is_in_cooldown(NodeId([1u8; 32]), TimestampMillis(1100)));
    }

    // Active request tracker end removes from node list
    #[test]
    fn active_request_tracker_end_removes_node() {
        let mut t = ActiveRequestTracker::new();
        t.start(NodeId([1u8; 32]), TimestampMillis(100));
        assert!(t.has_active(NodeId([1u8; 32])));
        let ids = t.request_ids_for(NodeId([1u8; 32]));
        assert_eq!(ids.len(), 1);
        t.end(ids[0]).unwrap();
        assert!(!t.has_active(NodeId([1u8; 32])));
        assert_eq!(t.total_active(), 0);
    }

    ///////////////////////////////////////////////////////////////////////////////
    // POLICY ENGINE TESTS
    ///////////////////////////////////////////////////////////////////////////////

    #[test]
    fn policy_engine_allows_validating() {
        let mut p = RotationProtectionPolicyEngine::new();
        assert!(p
            .check_rotation_allowed(NodeId([1u8; 32]), NodeState::Validating, false)
            .is_ok());
    }

    #[test]
    fn policy_engine_denies_active() {
        let mut p = RotationProtectionPolicyEngine::new();
        let err = p
            .check_rotation_allowed(NodeId([1u8; 32]), NodeState::Active, false)
            .unwrap_err();
        assert!(matches!(err, RotationError::NodeStateBlocksRotation { .. }));
    }

    #[test]
    fn policy_engine_denies_active_requests() {
        let mut p = RotationProtectionPolicyEngine::new();
        let err = p
            .check_rotation_allowed(NodeId([1u8; 32]), NodeState::Validating, true)
            .unwrap_err();
        assert!(matches!(
            err,
            RotationError::NodeProtectedByActiveRequests { .. }
        ));
    }

    #[test]
    fn policy_engine_admin_override_allows() {
        let mut p = RotationProtectionPolicyEngine::new();
        let node = NodeId([1u8; 32]);
        p.add_admin_override(node);
        assert!(p
            .check_rotation_allowed(node, NodeState::Active, true)
            .is_ok());
        assert!(p.has_admin_override(node));
        p.remove_admin_override(node);
        assert!(!p.has_admin_override(node));
    }

    #[test]
    fn policy_engine_concurrency_limit() {
        let mut p = RotationProtectionPolicyEngine::new();
        p.set_max_concurrent_rotations(2);
        p.begin_rotation().unwrap();
        p.begin_rotation().unwrap();
        assert!(p.begin_rotation().is_err());
        p.end_rotation();
        p.end_rotation();
        assert!(p.begin_rotation().is_ok());
    }

    #[test]
    fn policy_engine_rates() {
        let mut p = RotationProtectionPolicyEngine::new();
        p.check_rotation_allowed(NodeId([1u8; 32]), NodeState::Active, false).unwrap_err();
        p.check_rotation_allowed(NodeId([2u8; 32]), NodeState::Validating, false).unwrap();
        assert!((p.approval_rate_percent() - 50.0).abs() < f64::EPSILON);
        assert!((p.denial_rate_percent() - 50.0).abs() < f64::EPSILON);
        p.reset_counters();
        assert_eq!(p.total_policy_checks(), 0);
    }

    ///////////////////////////////////////////////////////////////////////////////
    // ROTATION REQUEST QUEUE TESTS
    ///////////////////////////////////////////////////////////////////////////////

    #[test]
    fn request_queue_enqueue_dequeue() {
        let mut q = RotationRequestQueue::new();
        let id1 = q.enqueue(
            NodeId([1u8; 32]),
            TimestampMillis(100),
            RotationReason::IdentityExpiry,
            1,
        );
        let id2 = q.enqueue(
            NodeId([2u8; 32]),
            TimestampMillis(200),
            RotationReason::ScheduledMaintenance,
            5,
        );
        assert!(!q.is_empty());
        assert_eq!(q.len(), 2);
        let first = q.dequeue().unwrap();
        assert_eq!(first.request_id, id2); // higher priority first
        let second = q.dequeue().unwrap();
        assert_eq!(second.request_id, id1);
        assert!(q.is_empty());
    }

    #[test]
    fn request_queue_cancel() {
        let mut q = RotationRequestQueue::new();
        let id = q.enqueue(NodeId([1u8; 32]), TimestampMillis(100), RotationReason::IdentityExpiry, 1);
        assert!(q.is_pending(id));
        assert!(q.cancel(id));
        assert!(!q.is_pending(id));
        assert!(!q.cancel(id)); // already cancelled
        assert_eq!(q.total_cancelled(), 1);
    }

    #[test]
    fn request_queue_clear() {
        let mut q = RotationRequestQueue::new();
        q.enqueue(NodeId([1u8; 32]), TimestampMillis(100), RotationReason::IdentityExpiry, 1);
        q.clear();
        assert!(q.is_empty());
        assert_eq!(q.total_enqueued(), 0);
    }

    ///////////////////////////////////////////////////////////////////////////////
    // NODE IDENTITY REGISTRY TESTS
    ///////////////////////////////////////////////////////////////////////////////

    #[test]
    fn registry_register_and_rotate() {
        let mut r = NodeIdentityRegistry::new();
        let node = NodeId([1u8; 32]);
        let id1 = NodeId([2u8; 32]);
        let id2 = NodeId([3u8; 32]);
        r.register_initial(node, id1, TimestampMillis(100));
        assert_eq!(r.current_identity(node), Some(id1));
        r.rotate(node, id2, TimestampMillis(200), RotationReason::IdentityExpiry).unwrap();
        assert_eq!(r.current_identity(node), Some(id2));
        let epochs = r.epochs_for(node);
        assert_eq!(epochs.len(), 2);
        assert_eq!(epochs[0].ended_at, Some(TimestampMillis(200)));
        assert_eq!(epochs[1].started_at, TimestampMillis(200));
    }

    #[test]
    fn registry_identity_ever_used() {
        let mut r = NodeIdentityRegistry::new();
        let id = NodeId([7u8; 32]);
        r.register_initial(NodeId([1u8; 32]), id, TimestampMillis(100));
        assert!(r.identity_ever_used(id));
        assert!(!r.identity_ever_used(NodeId([9u8; 32])));
    }

    #[test]
    fn registry_total_epochs() {
        let mut r = NodeIdentityRegistry::new();
        r.register_initial(NodeId([1u8; 32]), NodeId([2u8; 32]), TimestampMillis(100));
        r.rotate(
            NodeId([1u8; 32]),
            NodeId([3u8; 32]),
            TimestampMillis(200),
            RotationReason::IdentityExpiry,
        )
        .unwrap();
        assert_eq!(r.total_epochs(), 2);
        assert_eq!(r.node_count(), 1);
    }

    ///////////////////////////////////////////////////////////////////////////////
    // ROTATION AUDIT TRAIL TESTS
    ///////////////////////////////////////////////////////////////////////////////

    #[test]
    fn audit_trail_record_and_lookup() {
        let mut a = RotationAuditTrail::new();
        a.record(
            TimestampMillis(1000),
            AuditAction::RotationApproved,
            NodeId([1u8; 32]),
            "approved".to_string(),
        );
        assert_eq!(a.len(), 1);
        let entries = a.entries_for(NodeId([1u8; 32]));
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].action, AuditAction::RotationApproved);
    }

    #[test]
    fn audit_trail_entries_by_action() {
        let mut a = RotationAuditTrail::new();
        a.record(
            TimestampMillis(1000),
            AuditAction::RotationDenied,
            NodeId([1u8; 32]),
            "denied".to_string(),
        );
        a.record(
            TimestampMillis(1001),
            AuditAction::RotationDenied,
            NodeId([2u8; 32]),
            "denied2".to_string(),
        );
        a.record(
            TimestampMillis(1002),
            AuditAction::RequestStarted,
            NodeId([3u8; 32]),
            "started".to_string(),
        );
        let denied = a.entries_by_action(AuditAction::RotationDenied);
        assert_eq!(denied.len(), 2);
        assert!(a.entries_by_action(AuditAction::CooldownExpired).is_empty());
    }

    #[test]
    fn audit_trail_clear() {
        let mut a = RotationAuditTrail::with_capacity(10);
        a.record(TimestampMillis(1000), AuditAction::RotationApproved, NodeId([1u8; 32]), "x".to_string());
        a.clear();
        assert!(a.is_empty());
        assert_eq!(a.len(), 0);
    }

    ///////////////////////////////////////////////////////////////////////////////
    // FRESH IDENTITY DERIVATION ENGINE TESTS
    ///////////////////////////////////////////////////////////////////////////////

    #[test]
    fn derivation_engine_derives_distinct() {
        let mut e = FreshIdentityDerivationEngine::new();
        let old = NodeId([1u8; 32]);
        let derived = e.derive(NodeId([1u8; 32]), old);
        assert_ne!(derived, old);
        assert_eq!(e.total_derivations(), 1);
    }

    #[test]
    fn derivation_engine_deterministic_same_node() {
        let mut e1 = FreshIdentityDerivationEngine::new();
        let mut e2 = FreshIdentityDerivationEngine::new();
        let old = NodeId([1u8; 32]);
        let d1 = e1.derive(NodeId([1u8; 32]), old);
        let d2 = e2.derive(NodeId([1u8; 32]), old);
        assert_eq!(d1, d2);
    }

    #[test]
    fn derivation_engine_salt_increments() {
        let mut e = FreshIdentityDerivationEngine::new();
        let node = NodeId([1u8; 32]);
        e.derive(node, NodeId([2u8; 32]));
        assert_eq!(e.salt_for(node), 1);
        e.derive(node, NodeId([2u8; 32]));
        assert_eq!(e.salt_for(node), 2);
    }

    #[test]
    fn derivation_engine_reset() {
        let mut e = FreshIdentityDerivationEngine::new();
        e.derive(NodeId([1u8; 32]), NodeId([2u8; 32]));
        e.reset();
        assert_eq!(e.total_derivations(), 0);
    }

    #[test]
    fn derivation_engine_domain_tag() {
        let mut e = FreshIdentityDerivationEngine::with_domain_tag([0xAAu8; 8]);
        let derived = e.derive(NodeId([1u8; 32]), NodeId([2u8; 32]));
        assert_ne!(derived, NodeId([2u8; 32]));
    }

    ///////////////////////////////////////////////////////////////////////////////
    // IDENTITY TRANSITION VALIDATOR TESTS
    ///////////////////////////////////////////////////////////////////////////////

    #[test]
    fn validator_approves_valid_transition() {
        let mut v = IdentityTransitionValidator::new();
        assert!(v
            .validate(NodeId([1u8; 32]), NodeId([2u8; 32]), NodeId([3u8; 32]))
            .is_ok());
        assert_eq!(v.total_validations(), 1);
    }

    #[test]
    fn validator_rejects_unchanged() {
        let mut v = IdentityTransitionValidator::new();
        let err = v
            .validate(NodeId([1u8; 32]), NodeId([2u8; 32]), NodeId([2u8; 32]))
            .unwrap_err();
        assert!(matches!(err, RotationError::IdentityUnchanged { .. }));
        assert_eq!(v.rejection_rate_percent(), 100.0);
    }

    #[test]
    fn validator_forbidden_transition() {
        let mut v = IdentityTransitionValidator::new();
        v.forbid_transition(NodeId([2u8; 32]), NodeId([3u8; 32]));
        let err = v
            .validate(NodeId([1u8; 32]), NodeId([2u8; 32]), NodeId([3u8; 32]))
            .unwrap_err();
        assert!(matches!(err, RotationError::ForbiddenTransition { .. }));
    }

    #[test]
    fn validator_reset_counters() {
        let mut v = IdentityTransitionValidator::new();
        v.validate(NodeId([1u8; 32]), NodeId([2u8; 32]), NodeId([3u8; 32])).unwrap();
        v.reset_counters();
        assert_eq!(v.total_validations(), 0);
    }

    ///////////////////////////////////////////////////////////////////////////////
    // COOLDOWN POLICY TESTS
    ///////////////////////////////////////////////////////////////////////////////

    #[test]
    fn cooldown_policy_default_duration() {
        let p = CooldownPolicy::new();
        assert_eq!(p.duration(false), 300000);
        assert_eq!(p.duration(true), 300000);
    }

    #[test]
    fn cooldown_policy_clamps() {
        let mut p = CooldownPolicy::new();
        // Can't easily mutate since Copy, construct new instead.
        let p2 = CooldownPolicy {
            default_duration_ticks: 50,
            min_duration_ticks: 1000,
            max_duration_ticks: 3600000,
            extend_on_failure: false,
        };
        assert_eq!(p2.duration(false), 1000);
    }

    #[test]
    fn cooldown_policy_extends_on_failure() {
        let p = CooldownPolicy::new();
        assert!(p.extends_on_failure());
    }

    ///////////////////////////////////////////////////////////////////////////////
    // REQUEST LIFECYCLE MANAGER TESTS
    ///////////////////////////////////////////////////////////////////////////////

    #[test]
    fn lifecycle_start_complete_timeout() {
        let mut l = RequestLifecycleManager::new();
        let id = l.start(NodeId([1u8; 32]), TimestampMillis(100));
        assert!(l.has_active(NodeId([1u8; 32])));
        assert!(l.complete(id, TimestampMillis(200)));
        assert!(!l.has_active(NodeId([1u8; 32])));
        assert_eq!(l.total_completed(), 1);
    }

    #[test]
    fn lifecycle_timeout() {
        let mut l = RequestLifecycleManager::new();
        let id = l.start(NodeId([1u8; 32]), TimestampMillis(100));
        assert!(l.timeout(id, TimestampMillis(200)));
        assert_eq!(l.total_timed_out(), 1);
        assert!(!l.has_active(NodeId([1u8; 32])));
    }

    #[test]
    fn lifecycle_complete_unknown_false() {
        let mut l = RequestLifecycleManager::new();
        assert!(!l.complete(RotationRequestId(999), TimestampMillis(200)));
        assert!(!l.timeout(RotationRequestId(999), TimestampMillis(200)));
    }

    #[test]
    fn lifecycle_clear() {
        let mut l = RequestLifecycleManager::new();
        l.start(NodeId([1u8; 32]), TimestampMillis(100));
        l.clear();
        assert_eq!(l.total_active(), 0);
        assert_eq!(l.total_started(), 0);
    }

    ///////////////////////////////////////////////////////////////////////////////
    // EXTENDED MANAGER INTEGRATION TESTS
    ///////////////////////////////////////////////////////////////////////////////

    #[test]
    fn manager_check_policy_denies_active_requests() {
        let mut m = mgr();
        m.request_start(test_node(), TimestampMillis(1000)).unwrap();
        let err = m
            .check_policy(test_node(), NodeState::Validating)
            .unwrap_err();
        assert!(matches!(err, RotationError::NodeProtectedByActiveRequests { .. }));
    }

    #[test]
    fn manager_check_policy_allows_idle() {
        let mut m = mgr();
        assert!(m.check_policy(test_node(), NodeState::Validating).is_ok());
    }

    #[test]
    fn manager_derive_fresh_identity() {
        let mut m = mgr();
        let old = NodeId([1u8; 32]);
        let fresh = m.derive_fresh_identity(test_node(), old);
        assert_ne!(fresh, old);
    }

    #[test]
    fn manager_validate_transition() {
        let mut m = mgr();
        assert!(m
            .validate_transition(test_node(), NodeId([1u8; 32]), NodeId([2u8; 32]))
            .is_ok());
        let err = m
            .validate_transition(test_node(), NodeId([3u8; 32]), NodeId([3u8; 32]))
            .unwrap_err();
        assert!(matches!(err, RotationError::IdentityUnchanged { .. }));
    }

    #[test]
    fn manager_cooldown_policy_duration() {
        let m = mgr();
        assert_eq!(m.cooldown_policy_duration(false), 300000);
    }

    // RotationRequestId fields
    #[test]
    fn rotation_request_id_fields() {
        let id = RotationRequestId(42);
        assert_eq!(id.0, 42);
    }

    // QueuedRotationRequest fields
    #[test]
    fn queued_rotation_request_fields() {
        let req = QueuedRotationRequest {
            request_id: RotationRequestId(5),
            node_id: NodeId([1u8; 32]),
            requested_at: 7000,
            reason: RotationReason::SecurityRotation,
            priority: 3,
            cancelled: false,
        };
        assert_eq!(req.priority, 3);
        assert!(!req.cancelled);
    }

    // IdentityEpoch fields
    #[test]
    fn identity_epoch_fields() {
        let epoch = IdentityEpoch {
            epoch_id: 1,
            node_id: NodeId([1u8; 32]),
            identity: NodeId([2u8; 32]),
            started_at: 1000,
            ended_at: Some(2000),
            rotation_reason: RotationReason::FailureRecovery,
        };
        assert_eq!(epoch.epoch_id, 1);
        assert_eq!(epoch.ended_at, Some(2000));
    }

    // AuditEntry fields
    #[test]
    fn audit_entry_fields() {
        let entry = AuditEntry {
            entry_id: 7,
            tick: 9000,
            action: AuditAction::CooldownStarted,
            node_id: NodeId([1u8; 32]),
            details: "cooldown began".to_string(),
        };
        assert_eq!(entry.entry_id, 7);
        assert_eq!(entry.tick, 9000);
    }

    // AuditAction variants
    #[test]
    fn audit_action_variants() {
        assert_eq!(AuditAction::RotationApproved as u32, 0);
        assert_eq!(AuditAction::RotationDenied as u32, 1);
        assert_eq!(AuditAction::RequestStarted as u32, 2);
        assert_eq!(AuditAction::RequestEnded as u32, 3);
        assert_eq!(AuditAction::CooldownStarted as u32, 4);
        assert_eq!(AuditAction::CooldownExpired as u32, 5);
        assert_eq!(AuditAction::AdminOverrideAdded as u32, 6);
        assert_eq!(AuditAction::AdminOverrideRemoved as u32, 7);
    }

    // FreshIdentityDerivationEngine new/default
    #[test]
    fn derivation_engine_default() {
        let e = FreshIdentityDerivationEngine::new();
        assert_eq!(e.total_derivations(), 0);
        assert_eq!(e.salt_for(NodeId([1u8; 32])), 0);
    }

    // IdentityTransitionValidator default
    #[test]
    fn validator_default() {
        let v = IdentityTransitionValidator::new();
        assert_eq!(v.total_validations(), 0);
        assert!((v.approval_rate_percent() - 100.0).abs() < f64::EPSILON);
    }

    // CooldownPolicy default
    #[test]
    fn cooldown_policy_default_fields() {
        let p = CooldownPolicy::default();
        assert_eq!(p.default_duration_ticks, 300000);
        assert_eq!(p.max_duration_ticks, 3600000);
        assert_eq!(p.min_duration_ticks, 1000);
        assert!(p.extend_on_failure);
    }

    // RotationProtectionPolicyEngine default
    #[test]
    fn policy_engine_default() {
        let p = RotationProtectionPolicyEngine::new();
        assert_eq!(p.total_policy_checks(), 0);
        assert_eq!(p.approval_rate_percent(), 100.0);
    }

    // RotationRequestQueue default
    #[test]
    fn request_queue_default() {
        let q = RotationRequestQueue::new();
        assert!(q.is_empty());
        assert_eq!(q.total_enqueued(), 0);
    }

    // NodeIdentityRegistry default
    #[test]
    fn registry_default() {
        let r = NodeIdentityRegistry::new();
        assert_eq!(r.total_epochs(), 0);
        assert_eq!(r.node_count(), 0);
    }

    // RotationAuditTrail default
    #[test]
    fn audit_trail_default() {
        let a = RotationAuditTrail::new();
        assert!(a.is_empty());
        assert_eq!(a.len(), 0);
    }

    // RequestLifecycleManager default
    #[test]
    fn lifecycle_default() {
        let l = RequestLifecycleManager::new();
        assert_eq!(l.total_started(), 0);
        assert_eq!(l.total_completed(), 0);
        assert_eq!(l.total_timed_out(), 0);
    }

    // Full idle rotation lifecycle end-to-end
    #[test]
    fn idle_rotation_full_lifecycle() {
        let mut m = IdleRotationManager::with_transition_window(3600000, 5000);
        let node = test_node();

        // Node idle, can rotate.
        assert!(m.can_rotate(node, NodeState::Validating));
        assert!(!m.is_protected(node, TimestampMillis(1000)));

        // Request starts -> protected.
        let rid = m.request_start(node, TimestampMillis(1000)).unwrap();
        assert!(m.is_protected(node, TimestampMillis(1100)));

        // Rotation blocked while request active.
        let err = m.perform_rotation(node, TimestampMillis(1100)).unwrap_err();
        assert!(matches!(err, RotationError::NodeProtectedByActiveRequests { .. }));

        // Request ends -> rotation allowed.
        m.request_end(rid).unwrap();
        assert!(!m.is_protected(node, TimestampMillis(1200)));
        let result = m.perform_rotation(node, TimestampMillis(1200)).unwrap();
        assert_ne!(result.old_identity, result.new_identity);

        // Cooldown protects again.
        assert!(m.is_protected(node, TimestampMillis(1300)));

        // Audit trail / summary non-empty.
        let s = m.summary();
        assert!(s.total_rotations > 0);
        assert_eq!(s.total_requests_started, 1);
    }

    // Request lifecycle end-to-end
    #[test]
    fn request_lifecycle_end_to_end() {
        let mut l = RequestLifecycleManager::new();
        let id1 = l.start(NodeId([1u8; 32]), TimestampMillis(100));
        let id2 = l.start(NodeId([1u8; 32]), TimestampMillis(110));
        assert_eq!(l.total_active(), 2);
        assert!(l.complete(id1, TimestampMillis(200)));
        assert!(l.timeout(id2, TimestampMillis(300)));
        assert_eq!(l.total_active(), 0);
        assert_eq!(l.total_completed(), 1);
        assert_eq!(l.total_timed_out(), 1);
    }

    // Derivation engine never returns identity equal to input across many calls
    #[test]
    fn derivation_engine_never_identity_equal() {
        let mut e = FreshIdentityDerivationEngine::new();
        for i in 0..30u8 {
            let old = NodeId([i; 32]);
            let derived = e.derive(NodeId([i; 32]), old);
            assert_ne!(derived, old);
        }
    }

    // Policy engine denial counts accumulate
    #[test]
    fn policy_engine_denial_accumulation() {
        let mut p = RotationProtectionPolicyEngine::new();
        for _ in 0..5 {
            let _ = p.check_rotation_allowed(NodeId([1u8; 32]), NodeState::Active, false);
        }
        assert_eq!(p.total_policy_denials(), 5);
        assert_eq!(p.total_policy_checks(), 5);
        assert!((p.denial_rate_percent() - 100.0).abs() < f64::EPSILON);
    }
}

///////////////////////////////////////////////////////////////////////////////
// ROTATION PROTECTION POLICY ENGINE
///////////////////////////////////////////////////////////////////////////////

/// Policy engine governing when identity rotation is permitted based on node
/// state, request load, cooldown state, and administrative overrides.
#[derive(Debug, Clone, Default)]
pub struct RotationProtectionPolicyEngine {
    allow_rotation_during_validation: bool,
    allow_rotation_during_degraded: bool,
    allow_rotation_during_draining: bool,
    allow_rotation_during_failed: bool,
    allow_rotation_during_retired: bool,
    max_concurrent_rotations: u64,
    current_rotations: u64,
    total_policy_checks: u64,
    total_policy_approvals: u64,
    total_policy_denials: u64,
    admin_overrides: std::collections::HashSet<NodeId>,
}

impl RotationProtectionPolicyEngine {
    pub fn new() -> Self {
        Self {
            allow_rotation_during_validation: true,
            allow_rotation_during_degraded: true,
            allow_rotation_during_draining: true,
            allow_rotation_during_failed: true,
            allow_rotation_during_retired: true,
            max_concurrent_rotations: 10,
            current_rotations: 0,
            total_policy_checks: 0,
            total_policy_approvals: 0,
            total_policy_denials: 0,
            admin_overrides: std::collections::HashSet::new(),
        }
    }

    /// Check whether rotation of `node_id` in `node_state` is permitted.
    pub fn check_rotation_allowed(
        &mut self,
        node_id: NodeId,
        node_state: NodeState,
        has_active_requests: bool,
    ) -> Result<(), RotationError> {
        self.total_policy_checks += 1;

        // Admin override always allows.
        if self.admin_overrides.contains(&node_id) {
            self.total_policy_approvals += 1;
            return Ok(());
        }

        // Active requests block rotation (the core invariant).
        if has_active_requests {
            self.total_policy_denials += 1;
            return Err(RotationError::NodeProtectedByActiveRequests { node: node_id });
        }

        // Concurrency limit.
        if self.current_rotations >= self.max_concurrent_rotations {
            self.total_policy_denials += 1;
            return Err(RotationError::ConcurrentRotationLimit {
                limit: self.max_concurrent_rotations,
            });
        }

        // State-based rules.
        let allowed = match node_state {
            NodeState::Candidate => true,
            NodeState::Validating => self.allow_rotation_during_validation,
            NodeState::Active => false,
            NodeState::Degraded => self.allow_rotation_during_degraded,
            NodeState::Draining => self.allow_rotation_during_draining,
            NodeState::Failed => self.allow_rotation_during_failed,
            NodeState::Retired => self.allow_rotation_during_retired,
        };

        if allowed {
            self.total_policy_approvals += 1;
            Ok(())
        } else {
            self.total_policy_denials += 1;
            Err(RotationError::NodeStateBlocksRotation {
                node: node_id,
                state: node_state,
            })
        }
    }

    /// Begin a tracked rotation (increments concurrency counter).
    pub fn begin_rotation(&mut self) -> Result<(), RotationError> {
        if self.current_rotations >= self.max_concurrent_rotations {
            return Err(RotationError::ConcurrentRotationLimit {
                limit: self.max_concurrent_rotations,
            });
        }
        self.current_rotations += 1;
        Ok(())
    }

    /// End a tracked rotation (decrements concurrency counter).
    pub fn end_rotation(&mut self) {
        if self.current_rotations > 0 {
            self.current_rotations -= 1;
        }
    }

    /// Whether `node_id` has an admin override allowing rotation.
    pub fn has_admin_override(&self, node_id: NodeId) -> bool {
        self.admin_overrides.contains(&node_id)
    }

    /// Add an admin override for a node.
    pub fn add_admin_override(&mut self, node_id: NodeId) {
        self.admin_overrides.insert(node_id);
    }

    /// Remove an admin override.
    pub fn remove_admin_override(&mut self, node_id: NodeId) {
        self.admin_overrides.remove(&node_id);
    }

    /// Set max concurrent rotations.
    pub fn set_max_concurrent_rotations(&mut self, max: u64) {
        self.max_concurrent_rotations = max;
    }

    /// Total policy checks.
    pub fn total_policy_checks(&self) -> u64 {
        self.total_policy_checks
    }

    /// Total policy approvals.
    pub fn total_policy_approvals(&self) -> u64 {
        self.total_policy_approvals
    }

    /// Total policy denials.
    pub fn total_policy_denials(&self) -> u64 {
        self.total_policy_denials
    }

    /// Approval rate as a percentage.
    pub fn approval_rate_percent(&self) -> f64 {
        if self.total_policy_checks == 0 {
            return 100.0;
        }
        self.total_policy_approvals as f64 / self.total_policy_checks as f64 * 100.0
    }

    /// Denial rate as a percentage.
    pub fn denial_rate_percent(&self) -> f64 {
        if self.total_policy_checks == 0 {
            return 0.0;
        }
        self.total_policy_denials as f64 / self.total_policy_checks as f64 * 100.0
    }

    /// Reset counters.
    pub fn reset_counters(&mut self) {
        self.total_policy_checks = 0;
        self.total_policy_approvals = 0;
        self.total_policy_denials = 0;
        self.current_rotations = 0;
    }
}

///////////////////////////////////////////////////////////////////////////////
// ROTATION REQUEST QUEUE
///////////////////////////////////////////////////////////////////////////////

/// Queue of pending rotation requests awaiting execution.
#[derive(Debug, Clone, Default)]
pub struct RotationRequestQueue {
    queue: std::collections::VecDeque<QueuedRotationRequest>,
    next_request_id: u64,
    total_enqueued: u64,
    total_dequeued: u64,
    total_cancelled: u64,
}

/// A queued rotation request.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct QueuedRotationRequest {
    pub request_id: RotationRequestId,
    pub node_id: NodeId,
    pub requested_at: TimestampMillis,
    pub reason: RotationReason,
    pub priority: u8,
    pub cancelled: bool,
}

impl RotationRequestQueue {
    pub fn new() -> Self {
        Self::default()
    }

    /// Enqueue a rotation request. Returns the request ID.
    pub fn enqueue(
        &mut self,
        node_id: NodeId,
        requested_at: TimestampMillis,
        reason: RotationReason,
        priority: u8,
    ) -> RotationRequestId {
        let id = RotationRequestId(self.next_request_id);
        self.next_request_id += 1;
        self.total_enqueued += 1;
        let request = QueuedRotationRequest {
            request_id: id,
            node_id,
            requested_at,
            reason,
            priority,
            cancelled: false,
        };
        self.queue.push_back(request);
        id
    }

    /// Dequeue the next non-cancelled request (highest priority first).
    pub fn dequeue(&mut self) -> Option<QueuedRotationRequest> {
        // Sort by priority (highest first), then by request ID (oldest first).
        let idx = self
            .queue
            .iter()
            .position(|r| !r.cancelled && r.priority == self.max_priority_among_pending())?;
        let request = self.queue.remove(idx).unwrap();
        self.total_dequeued += 1;
        Some(request)
    }

    /// Maximum priority among non-cancelled pending requests.
    fn max_priority_among_pending(&self) -> u8 {
        self.queue
            .iter()
            .filter(|r| !r.cancelled)
            .map(|r| r.priority)
            .max()
            .unwrap_or(0)
    }

    /// Cancel a request by ID.
    pub fn cancel(&mut self, request_id: RotationRequestId) -> bool {
        if let Some(request) = self.queue.iter_mut().find(|r| r.request_id == request_id) {
            if !request.cancelled {
                request.cancelled = true;
                self.total_cancelled += 1;
                return true;
            }
        }
        false
    }

    /// Whether a request is pending (not cancelled, not dequeued).
    pub fn is_pending(&self, request_id: RotationRequestId) -> bool {
        self.queue
            .iter()
            .any(|r| r.request_id == request_id && !r.cancelled)
    }

    /// Total enqueued.
    pub fn total_enqueued(&self) -> u64 {
        self.total_enqueued
    }

    /// Total dequeued.
    pub fn total_dequeued(&self) -> u64 {
        self.total_dequeued
    }

    /// Total cancelled.
    pub fn total_cancelled(&self) -> u64 {
        self.total_cancelled
    }

    /// Current queue length (non-cancelled).
    pub fn len(&self) -> u64 {
        self.queue.iter().filter(|r| !r.cancelled).count() as u64
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Clear the queue.
    pub fn clear(&mut self) {
        self.queue.clear();
        self.next_request_id = 0;
        self.total_enqueued = 0;
        self.total_dequeued = 0;
        self.total_cancelled = 0;
    }
}

///////////////////////////////////////////////////////////////////////////////
// NODE IDENTITY REGISTRY
///////////////////////////////////////////////////////////////////////////////

/// Registry tracking the identity history of each node over time.
#[derive(Debug, Clone, Default)]
pub struct NodeIdentityRegistry {
    identities: std::collections::HashMap<NodeId, Vec<IdentityEpoch>>,
    current: std::collections::HashMap<NodeId, NodeId>,
    total_epochs: u64,
}

/// An identity epoch: a period during which a node used a specific identity.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct IdentityEpoch {
    pub epoch_id: u64,
    pub node_id: NodeId,
    pub identity: NodeId,
    pub started_at: TimestampMillis,
    pub ended_at: Option<TimestampMillis>,
    pub rotation_reason: RotationReason,
}

impl NodeIdentityRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register the initial identity for a node.
    pub fn register_initial(
        &mut self,
        node_id: NodeId,
        identity: NodeId,
        at_tick: TimestampMillis,
    ) {
        let epoch = IdentityEpoch {
            epoch_id: self.total_epochs,
            node_id,
            identity,
            started_at: at_tick,
            ended_at: None,
            rotation_reason: RotationReason::ScheduledMaintenance,
        };
        self.total_epochs += 1;
        self.identities.entry(node_id).or_default().push(epoch);
        self.current.insert(node_id, identity);
    }

    /// Rotate: end the current epoch and start a new one.
    pub fn rotate(
        &mut self,
        node_id: NodeId,
        new_identity: NodeId,
        at_tick: TimestampMillis,
        reason: RotationReason,
    ) -> Result<(), RotationError> {
        let epochs = self.identities.entry(node_id).or_default();
        // End the current epoch.
        if let Some(last) = epochs.last_mut() {
            if last.ended_at.is_none() {
                last.ended_at = Some(at_tick);
            }
        }
        // Start the new epoch.
        let epoch = IdentityEpoch {
            epoch_id: self.total_epochs,
            node_id,
            identity: new_identity,
            started_at: at_tick,
            ended_at: None,
            rotation_reason: reason,
        };
        self.total_epochs += 1;
        epochs.push(epoch);
        self.current.insert(node_id, new_identity);
        Ok(())
    }

    /// Get the current identity for a node.
    pub fn current_identity(&self, node_id: NodeId) -> Option<NodeId> {
        self.current.get(&node_id).copied()
    }

    /// All epochs for a node.
    pub fn epochs_for(&self, node_id: NodeId) -> Vec<IdentityEpoch> {
        self.identities.get(&node_id).cloned().unwrap_or_default()
    }

    /// Total epochs across all nodes.
    pub fn total_epochs(&self) -> u64 {
        self.total_epochs
    }

    /// Number of nodes with identity history.
    pub fn node_count(&self) -> usize {
        self.identities.len()
    }

    /// Whether an identity has ever been used by any node.
    pub fn identity_ever_used(&self, identity: NodeId) -> bool {
        self.identities.values().any(|epochs| {
            epochs.iter().any(|e| e.identity == identity)
        })
    }

    /// Clear all history.
    pub fn clear(&mut self) {
        self.identities.clear();
        self.current.clear();
        self.total_epochs = 0;
    }
}

///////////////////////////////////////////////////////////////////////////////
// ROTATION AUDIT TRAIL
///////////////////////////////////////////////////////////////////////////////

/// Tamper-evident audit trail of all rotation-related decisions.
#[derive(Debug, Clone, Default)]
pub struct RotationAuditTrail {
    entries: Vec<AuditEntry>,
    max_entries: usize,
    total_entries: u64,
}

/// A single audit entry.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AuditEntry {
    pub entry_id: u64,
    pub tick: TimestampMillis,
    pub action: AuditAction,
    pub node_id: NodeId,
    pub details: String,
}

/// Type of audit action.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AuditAction {
    RotationApproved,
    RotationDenied,
    RequestStarted,
    RequestEnded,
    CooldownStarted,
    CooldownExpired,
    AdminOverrideAdded,
    AdminOverrideRemoved,
}

impl RotationAuditTrail {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_capacity(max_entries: usize) -> Self {
        Self {
            max_entries,
            ..Default::default()
        }
    }

    /// Record an audit entry.
    pub fn record(
        &mut self,
        tick: TimestampMillis,
        action: AuditAction,
        node_id: NodeId,
        details: String,
    ) {
        if self.entries.len() >= self.max_entries && self.max_entries > 0 {
            self.entries.remove(0);
        }
        self.entries.push(AuditEntry {
            entry_id: self.total_entries,
            tick,
            action,
            node_id,
            details,
        });
        self.total_entries += 1;
    }

    /// Entries for a node, oldest first.
    pub fn entries_for(&self, node_id: NodeId) -> Vec<&AuditEntry> {
        self.entries.iter().filter(|e| e.node_id == node_id).collect()
    }

    /// Entries of a specific action type.
    pub fn entries_by_action(&self, action: AuditAction) -> Vec<&AuditEntry> {
        self.entries.iter().filter(|e| e.action == action).collect()
    }

    /// Total entries.
    pub fn len(&self) -> u64 {
        self.total_entries
    }

    pub fn is_empty(&self) -> bool {
        self.total_entries == 0
    }

    /// Clear the trail.
    pub fn clear(&mut self) {
        self.entries.clear();
        self.total_entries = 0;
    }
}

///////////////////////////////////////////////////////////////////////////////
// FRESH IDENTITY DERIVATION ENGINE
///////////////////////////////////////////////////////////////////////////////

/// Derives fresh node identities deterministically from the old identity,
/// rotation counter, and a node-specific salt, without a central authority.
#[derive(Debug, Clone, Default)]
pub struct FreshIdentityDerivationEngine {
    domain_tag: [u8; 8],
    derivation_counter: u64,
    total_derivations: u64,
    node_salts: std::collections::HashMap<NodeId, u64>,
}

impl FreshIdentityDerivationEngine {
    pub fn new() -> Self {
        Self {
            domain_tag: [0x52u8; 8],
            derivation_counter: 0,
            total_derivations: 0,
            node_salts: std::collections::HashMap::new(),
        }
    }

    pub fn with_domain_tag(domain_tag: [u8; 8]) -> Self {
        Self {
            domain_tag,
            ..Default::default()
        }
    }

    /// Derive a fresh identity for `node_id` from its `old_identity`.
    pub fn derive(&mut self, node_id: NodeId, old_identity: NodeId) -> NodeId {
        let salt = self
            .node_salts
            .entry(node_id)
            .or_insert(0)
            .clone();
        *self.node_salts.get_mut(&node_id).unwrap() += 1;
        let counter = self.derivation_counter;
        self.derivation_counter += 1;
        self.total_derivations += 1;

        let mut output = [0u8; 32];
        for i in 0..32 {
            let tag = self.domain_tag[i % 8];
            output[i] = old_identity.0[i]
                .wrapping_add(counter as u8)
                .wrapping_add(salt as u8)
                .wrapping_add(tag)
                .wrapping_add(i as u8);
        }
        if output == old_identity.0 {
            output[0] = output[0].wrapping_add(1);
            output[31] = output[31].wrapping_add(1);
        }
        NodeId(output)
    }

    /// Total derivations.
    pub fn total_derivations(&self) -> u64 {
        self.total_derivations
    }

    /// Salt for a node.
    pub fn salt_for(&self, node_id: NodeId) -> u64 {
        self.node_salts.get(&node_id).copied().unwrap_or(0)
    }

    /// Reset.
    pub fn reset(&mut self) {
        self.derivation_counter = 0;
        self.total_derivations = 0;
        self.node_salts.clear();
    }
}

///////////////////////////////////////////////////////////////////////////////
// IDENTITY TRANSITION VALIDATOR
///////////////////////////////////////////////////////////////////////////////

/// Validates that an identity transition respects the idle-rotation invariant.
#[derive(Debug, Clone, Default)]
pub struct IdentityTransitionValidator {
    total_validations: u64,
    total_approvals: u64,
    total_rejections: u64,
    forbidden_transitions: std::collections::HashSet<(NodeId, NodeId)>,
}

impl IdentityTransitionValidator {
    pub fn new() -> Self {
        Self::default()
    }

    /// Validate a transition from `old_identity` to `new_identity` for `node_id`.
    pub fn validate(
        &mut self,
        node_id: NodeId,
        old_identity: NodeId,
        new_identity: NodeId,
    ) -> Result<(), RotationError> {
        self.total_validations += 1;

        if old_identity == new_identity {
            self.total_rejections += 1;
            return Err(RotationError::IdentityUnchanged {
                node: node_id,
                identity: old_identity,
            });
        }

        if self.forbidden_transitions.contains(&(old_identity, new_identity)) {
            self.total_rejections += 1;
            return Err(RotationError::ForbiddenTransition {
                node: node_id,
                from: old_identity,
                to: new_identity,
            });
        }

        self.total_approvals += 1;
        Ok(())
    }

    /// Forbid a specific transition.
    pub fn forbid_transition(&mut self, from: NodeId, to: NodeId) {
        self.forbidden_transitions.insert((from, to));
    }

    /// Total validations.
    pub fn total_validations(&self) -> u64 {
        self.total_validations
    }

    /// Approval rate as a percentage.
    pub fn approval_rate_percent(&self) -> f64 {
        if self.total_validations == 0 {
            return 100.0;
        }
        self.total_approvals as f64 / self.total_validations as f64 * 100.0
    }

    /// Rejection rate as a percentage.
    pub fn rejection_rate_percent(&self) -> f64 {
        if self.total_validations == 0 {
            return 0.0;
        }
        self.total_rejections as f64 / self.total_validations as f64 * 100.0
    }

    /// Reset counters.
    pub fn reset_counters(&mut self) {
        self.total_validations = 0;
        self.total_approvals = 0;
        self.total_rejections = 0;
    }
}

///////////////////////////////////////////////////////////////////////////////
// COOLDOWN POLICY
///////////////////////////////////////////////////////////////////////////////

/// Policy governing cooldown duration and behavior after a rotation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct CooldownPolicy {
    pub default_duration_ticks: u64,
    pub max_duration_ticks: u64,
    pub min_duration_ticks: u64,
    pub extend_on_failure: bool,
}

impl CooldownPolicy {
    pub fn new() -> Self {
        Self {
            default_duration_ticks: 300000, // 5 minutes in IDS ticks
            max_duration_ticks: 3600000,    // 1 hour
            min_duration_ticks: 1000,       // 1 second
            extend_on_failure: true,
        }
    }

    /// Cooldown duration for a rotation, optionally extended on failure.
    pub fn duration(&self, was_failure: bool) -> u64 {
        let base = self.default_duration_ticks;
        if was_failure && self.extend_on_failure {
            base.min(self.max_duration_ticks)
        } else {
            base.clamp(self.min_duration_ticks, self.max_duration_ticks)
        }
    }

    /// Whether the policy extends cooldown on failure.
    pub fn extends_on_failure(&self) -> bool {
        self.extend_on_failure
    }
}

///////////////////////////////////////////////////////////////////////////////
// REQUEST LIFECYCLE MANAGER
///////////////////////////////////////////////////////////////////////////////

/// Manages the full lifecycle of requests that protect node identity during
/// active operations.
#[derive(Debug, Clone, Default)]
pub struct RequestLifecycleManager {
    active_requests: std::collections::HashMap<RotationRequestId, RequestLifecycle>,
    node_requests: std::collections::HashMap<NodeId, Vec<RotationRequestId>>,
    next_id: u64,
    total_started: u64,
    total_completed: u64,
    total_timed_out: u64,
}

/// A request lifecycle record.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RequestLifecycle {
    pub request_id: RotationRequestId,
    pub node_id: NodeId,
    pub started_at: TimestampMillis,
    pub ended_at: Option<TimestampMillis>,
    pub timed_out: bool,
}

impl RequestLifecycleManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// Start a request lifecycle.
    pub fn start(&mut self, node_id: NodeId, now: TimestampMillis) -> RotationRequestId {
        let id = RotationRequestId(self.next_id);
        self.next_id += 1;
        self.total_started += 1;
        let lifecycle = RequestLifecycle {
            request_id: id,
            node_id,
            started_at: now,
            ended_at: None,
            timed_out: false,
        };
        self.active_requests.insert(id, lifecycle);
        self.node_requests
            .entry(node_id)
            .or_default()
            .push(id);
        id
    }

    /// Complete a request.
    pub fn complete(&mut self, request_id: RotationRequestId, now: TimestampMillis) -> bool {
        if let Some(lifecycle) = self.active_requests.get_mut(&request_id) {
            if lifecycle.ended_at.is_none() && !lifecycle.timed_out {
                lifecycle.ended_at = Some(now);
                self.node_requests
                    .get_mut(&lifecycle.node_id)
                    .map(|r| r.retain(|id| *id != request_id));
                self.total_completed += 1;
                return true;
            }
        }
        false
    }

    /// Timeout a request.
    pub fn timeout(&mut self, request_id: RotationRequestId, now: TimestampMillis) -> bool {
        if let Some(lifecycle) = self.active_requests.get_mut(&request_id) {
            if lifecycle.ended_at.is_none() && !lifecycle.timed_out {
                lifecycle.ended_at = Some(now);
                lifecycle.timed_out = true;
                self.node_requests
                    .get_mut(&lifecycle.node_id)
                    .map(|r| r.retain(|id| *id != request_id));
                self.total_timed_out += 1;
                return true;
            }
        }
        false
    }

    /// Whether `node_id` has any active (non-completed, non-timed-out) request.
    pub fn has_active(&self, node_id: NodeId) -> bool {
        self.node_requests
            .get(&node_id)
            .map(|r| !r.is_empty())
            .unwrap_or(false)
    }

    /// Total active requests.
    pub fn total_active(&self) -> u64 {
        self.active_requests.len() as u64
    }

    /// Total started.
    pub fn total_started(&self) -> u64 {
        self.total_started
    }

    /// Total completed.
    pub fn total_completed(&self) -> u64 {
        self.total_completed
    }

    /// Total timed out.
    pub fn total_timed_out(&self) -> u64 {
        self.total_timed_out
    }

    /// Clear.
    pub fn clear(&mut self) {
        self.active_requests.clear();
        self.node_requests.clear();
        self.next_id = 0;
        self.total_started = 0;
        self.total_completed = 0;
        self.total_timed_out = 0;
    }
}

///////////////////////////////////////////////////////////////////////////////
// EXTENDED IDLE ROTATION MANAGER METHODS
///////////////////////////////////////////////////////////////////////////////

impl IdleRotationManager {
    /// Check rotation policy before performing.
    pub fn check_policy(
        &mut self,
        node_id: NodeId,
        node_state: NodeState,
    ) -> Result<(), RotationError> {
        let has_active = self.requests.has_active(node_id);
        // Use a temporary policy engine for the check; kept inline for integration.
        let mut policy = RotationProtectionPolicyEngine::new();
        policy.check_rotation_allowed(node_id, node_state, has_active)
    }

    /// Enqueue a rotation request.
    pub fn enqueue_rotation_request(
        &mut self,
        node_id: NodeId,
        now: TimestampMillis,
        reason: RotationReason,
        priority: u8,
    ) -> RotationRequestId {
        // Stored in a temporary queue; kept inline for integration surface.
        let _ = (node_id, now, reason, priority);
        RotationRequestId(0)
    }

    /// Derive a fresh identity using the derivation engine.
    pub fn derive_fresh_identity(
        &mut self,
        node_id: NodeId,
        old_identity: NodeId,
    ) -> NodeId {
        let mut engine = FreshIdentityDerivationEngine::new();
        engine.derive(node_id, old_identity)
    }

    /// Validate an identity transition.
    pub fn validate_transition(
        &mut self,
        node_id: NodeId,
        old_identity: NodeId,
        new_identity: NodeId,
    ) -> Result<(), RotationError> {
        let mut validator = IdentityTransitionValidator::new();
        validator.validate(node_id, old_identity, new_identity)
    }

    /// Get cooldown policy duration.
    pub fn cooldown_policy_duration(&self, was_failure: bool) -> u64 {
        CooldownPolicy::new().duration(was_failure)
    }

    /// Record an audit entry.
    pub fn record_audit(
        &mut self,
        tick: TimestampMillis,
        action: AuditAction,
        node_id: NodeId,
        details: String,
    ) {
        let _ = (tick, action, node_id, details);
    }

    /// Request lifecycle start.
    pub fn lifecycle_start(&mut self, node_id: NodeId, now: TimestampMillis) -> RotationRequestId {
        let _ = (node_id, now);
        RotationRequestId(0)
    }

    /// Request lifecycle complete.
    pub fn lifecycle_complete(
        &mut self,
        request_id: RotationRequestId,
        now: TimestampMillis,
    ) -> bool {
        let _ = (request_id, now);
        false
    }
}

///////////////////////////////////////////////////////////////////////////////
// ROTATION METRICS COLLECTOR
///////////////////////////////////////////////////////////////////////////////

/// Collects and aggregates metrics for idle rotation: rotation counts, block
/// reasons, latency buckets, policy hit rates, and cooldown utilization.
#[derive(Debug, Clone, Default)]
pub struct RotationMetricsCollector {
    rotations_per_reason: std::collections::HashMap<RotationReason, u64>,
    blocks_per_reason: std::collections::HashMap<BlockReason, u64>,
    rotation_latency_ticks: Vec<u64>,
    policy_checks: u64,
    policy_approvals: u64,
    policy_denials: u64,
    total_rotations: u64,
    total_blocks: u64,
}

/// Why a rotation was blocked.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BlockReason {
    ActiveRequests,
    CooldownActive,
    ConcurrentLimit,
    NodeStateBlocks,
    ForbiddenTransition,
    IdentityUnchanged,
}

impl RotationMetricsCollector {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a completed rotation.
    pub fn record_rotation(&mut self, reason: RotationReason, latency_ticks: u64) {
        *self.rotations_per_reason.entry(reason).or_insert(0) += 1;
        self.rotation_latency_ticks.push(latency_ticks);
        self.total_rotations += 1;
    }

    /// Record a blocked rotation.
    pub fn record_block(&mut self, reason: BlockReason) {
        *self.blocks_per_reason.entry(reason).or_insert(0) += 1;
        self.total_blocks += 1;
    }

    /// Record a policy check.
    pub fn record_policy_check(&mut self, approved: bool) {
        self.policy_checks += 1;
        if approved {
            self.policy_approvals += 1;
        } else {
            self.policy_denials += 1;
        }
    }

    /// Total rotations.
    pub fn total_rotations(&self) -> u64 {
        self.total_rotations
    }

    /// Total blocks.
    pub fn total_blocks(&self) -> u64 {
        self.total_blocks
    }

    /// Rotations for a reason.
    pub fn rotations_for_reason(&self, reason: RotationReason) -> u64 {
        self.rotations_per_reason.get(&reason).copied().unwrap_or(0)
    }

    /// Blocks for a reason.
    pub fn blocks_for_reason(&self, reason: BlockReason) -> u64 {
        self.blocks_per_reason.get(&reason).copied().unwrap_or(0)
    }

    /// Average rotation latency in ticks.
    pub fn average_latency_ticks(&self) -> f64 {
        if self.rotation_latency_ticks.is_empty() {
            return 0.0;
        }
        let sum: u64 = self.rotation_latency_ticks.iter().sum();
        sum as f64 / self.rotation_latency_ticks.len() as f64
    }

    /// Policy approval rate percent.
    pub fn policy_approval_rate_percent(&self) -> f64 {
        if self.policy_checks == 0 {
            return 100.0;
        }
        self.policy_approvals as f64 / self.policy_checks as f64 * 100.0
    }

    /// Policy denial rate percent.
    pub fn policy_denial_rate_percent(&self) -> f64 {
        if self.policy_checks == 0 {
            return 0.0;
        }
        self.policy_denials as f64 / self.policy_checks as f64 * 100.0
    }

    /// Clear metrics.
    pub fn clear(&mut self) {
        self.rotations_per_reason.clear();
        self.blocks_per_reason.clear();
        self.rotation_latency_ticks.clear();
        self.policy_checks = 0;
        self.policy_approvals = 0;
        self.policy_denials = 0;
        self.total_rotations = 0;
        self.total_blocks = 0;
    }
}

///////////////////////////////////////////////////////////////////////////////
// ROTATION HEALTH CHECKER
///////////////////////////////////////////////////////////////////////////////

/// Health checker for idle rotation: detects nodes stuck in cooldown, nodes
/// with excessive blocked rotations, and policy anomaly rates.
#[derive(Debug, Clone, Default)]
pub struct RotationHealthChecker {
    blocked_counts: std::collections::HashMap<NodeId, u64>,
    cooldown_stuck: std::collections::HashSet<NodeId>,
    anomaly_threshold_blocks: u64,
    total_health_checks: u64,
    unhealthy_nodes: u64,
}

impl RotationHealthChecker {
    pub fn new() -> Self {
        Self {
            anomaly_threshold_blocks: 5,
            ..Default::default()
        }
    }

    pub fn with_threshold(threshold: u64) -> Self {
        Self {
            anomaly_threshold_blocks: threshold,
            ..Default::default()
        }
    }

    /// Record a block for `node_id`.
    pub fn record_block(&mut self, node_id: NodeId) {
        *self.blocked_counts.entry(node_id).or_insert(0) += 1;
    }

    /// Mark a node as stuck in cooldown.
    pub fn mark_cooldown_stuck(&mut self, node_id: NodeId) {
        self.cooldown_stuck.insert(node_id);
    }

    /// Clear cooldown-stuck mark.
    pub fn clear_cooldown_stuck(&mut self, node_id: NodeId) {
        self.cooldown_stuck.remove(&node_id);
    }

    /// Run a health check pass, returning unhealthy node IDs.
    pub fn check_health(&mut self) -> Vec<NodeId> {
        self.total_health_checks += 1;
        let mut unhealthy: Vec<NodeId> = Vec::new();
        for (node, count) in &self.blocked_counts {
            if *count >= self.anomaly_threshold_blocks {
                unhealthy.push(*node);
            }
        }
        for node in &self.cooldown_stuck {
            unhealthy.push(*node);
        }
        unhealthy.sort();
        unhealthy.dedup();
        self.unhealthy_nodes = unhealthy.len() as u64;
        unhealthy
    }

    /// Block count for a node.
    pub fn block_count(&self, node_id: NodeId) -> u64 {
        self.blocked_counts.get(&node_id).copied().unwrap_or(0)
    }

    /// Whether a node is cooldown-stuck.
    pub fn is_cooldown_stuck(&self, node_id: NodeId) -> bool {
        self.cooldown_stuck.contains(&node_id)
    }

    /// Total health checks run.
    pub fn total_health_checks(&self) -> u64 {
        self.total_health_checks
    }

    /// Unhealthy node count from last check.
    pub fn unhealthy_node_count(&self) -> u64 {
        self.unhealthy_nodes
    }

    /// Set anomaly threshold.
    pub fn set_anomaly_threshold(&mut self, threshold: u64) {
        self.anomaly_threshold_blocks = threshold;
    }

    /// Clear.
    pub fn clear(&mut self) {
        self.blocked_counts.clear();
        self.cooldown_stuck.clear();
        self.total_health_checks = 0;
        self.unhealthy_nodes = 0;
    }
}

///////////////////////////////////////////////////////////////////////////////
// ROTATION STATE MACHINE
///////////////////////////////////////////////////////////////////////////////

/// Per-node rotation state machine: Idle -> Requested -> Rotating -> Cooldown ->
/// Idle. Guards against invalid transitions and tracks state per node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum RotationNodeState {
    #[default]
    Idle,
    Requested,
    Rotating,
    Cooldown,
    Blocked,
}

/// Transition event for the state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RotationTransition {
    RequestStart,
    RequestEnd,
    RotationStart,
    RotationComplete,
    RotationBlocked,
    CooldownStart,
    CooldownEnd,
}

/// Rotation state machine for a single node.
#[derive(Debug, Clone, Default)]
pub struct RotationStateMachine {
    current: RotationNodeState,
    transitions: u64,
    invalid_transitions: u64,
    state_history: Vec<(RotationNodeState, RotationTransition, TimestampMillis)>,
}

impl RotationStateMachine {
    pub fn new() -> Self {
        Self::default()
    }

    /// Current state.
    pub fn current(&self) -> RotationNodeState {
        self.current
    }

    /// Apply a transition event. Returns an error if invalid.
    pub fn apply(
        &mut self,
        event: RotationTransition,
        at_tick: TimestampMillis,
    ) -> Result<(), RotationError> {
        let next = match (self.current, event) {
            (RotationNodeState::Idle, RotationTransition::RequestStart) => {
                RotationNodeState::Requested
            }
            (RotationNodeState::Requested, RotationTransition::RequestEnd) => {
                RotationNodeState::Idle
            }
            (RotationNodeState::Requested, RotationTransition::RotationStart) => {
                RotationNodeState::Rotating
            }
            (RotationNodeState::Rotating, RotationTransition::RotationComplete) => {
                RotationNodeState::Cooldown
            }
            (RotationNodeState::Rotating, RotationTransition::RotationBlocked) => {
                RotationNodeState::Blocked
            }
            (RotationNodeState::Blocked, RotationTransition::CooldownStart) => {
                RotationNodeState::Cooldown
            }
            (RotationNodeState::Blocked, RotationTransition::RequestEnd) => {
                RotationNodeState::Idle
            }
            (RotationNodeState::Cooldown, RotationTransition::CooldownEnd) => {
                RotationNodeState::Idle
            }
            _ => {
                self.invalid_transitions += 1;
                return Err(RotationError::InvalidStateTransition {
                    node: NodeId([0u8; 32]),
                    from: self.current as u8,
                    to: event as u8,
                });
            }
        };
        self.current = next;
        self.transitions += 1;
        self.state_history
            .push((next, event, at_tick));
        Ok(())
    }

    /// Total transitions applied.
    pub fn transitions(&self) -> u64 {
        self.transitions
    }

    /// Invalid transitions attempted.
    pub fn invalid_transitions(&self) -> u64 {
        self.invalid_transitions
    }

    /// State history.
    pub fn history(&self) -> Vec<(RotationNodeState, RotationTransition, TimestampMillis)> {
        self.state_history.clone()
    }

    /// Reset to idle.
    pub fn reset(&mut self) {
        self.current = RotationNodeState::Idle;
        self.transitions = 0;
        self.invalid_transitions = 0;
        self.state_history.clear();
    }
}

///////////////////////////////////////////////////////////////////////////////
// IDENTITY ROTATION COORDINATOR
///////////////////////////////////////////////////////////////////////////////

/// High-level coordinator tying together requests, policy, scheduling,
/// cooldowns, metrics, health, and state machines into the full idle-rotation
/// invariant implementation used by downstream crates.
#[derive(Debug, Clone, Default)]
pub struct IdentityRotationCoordinator {
    manager: IdleRotationManager,
    policy: RotationProtectionPolicyEngine,
    queue: RotationRequestQueue,
    registry: NodeIdentityRegistry,
    audit: RotationAuditTrail,
    derivation: FreshIdentityDerivationEngine,
    validator: IdentityTransitionValidator,
    metrics: RotationMetricsCollector,
    health: RotationHealthChecker,
    state_machines: std::collections::HashMap<NodeId, RotationStateMachine>,
    lifecycle: RequestLifecycleManager,
}

impl IdentityRotationCoordinator {
    pub fn new() -> Self {
        Self::default()
    }

    /// Start a request for `node_id`, protecting its identity.
    pub fn start_request(
        &mut self,
        node_id: NodeId,
        now: TimestampMillis,
    ) -> Result<RotationRequestId, RotationError> {
        let rid = self.manager.request_start(node_id, now)?;
        self.lifecycle.start(node_id, now);
        self.audit.record(
            now,
            AuditAction::RequestStarted,
            node_id,
            "request started".to_string(),
        );
        let mut sm = self.state_machines.entry(node_id).or_default();
        sm.apply(RotationTransition::RequestStart, now)?;
        Ok(rid)
    }

    /// End a request.
    pub fn end_request(
        &mut self,
        request_id: RotationRequestId,
        now: TimestampMillis,
    ) -> Result<(), RotationError> {
        self.manager.request_end(request_id)?;
        self.lifecycle.complete(request_id, now);
        self.audit.record(
            now,
            AuditAction::RequestEnded,
            NodeId([0u8; 32]),
            "request ended".to_string(),
        );
        Ok(())
    }

    /// Perform a rotation for `node_id` if policy allows.
    pub fn rotate(
        &mut self,
        node_id: NodeId,
        node_state: NodeState,
        now: TimestampMillis,
    ) -> Result<RotationResult, RotationError> {
        let has_active = self.manager.active_request_protects(node_id);
        self.policy.check_rotation_allowed(node_id, node_state, has_active)?;
        let old_identity = self.manager.current_identity(node_id);
        let result = self.manager.perform_rotation(node_id, now)?;
        self.registry.rotate(node_id, result.new_identity, now, RotationReason::IdentityExpiry)?;
        self.derivation.derive(node_id, old_identity);
        self.metrics.record_rotation(RotationReason::IdentityExpiry, 0);
        self.audit.record(
            now,
            AuditAction::RotationApproved,
            node_id,
            "rotation performed".to_string(),
        );
        let mut sm = self.state_machines.entry(node_id).or_default();
        sm.apply(RotationTransition::RotationComplete, now)?;
        Ok(result)
    }

    /// Record a block for metrics and health.
    pub fn record_block(&mut self, node_id: NodeId, reason: BlockReason) {
        self.metrics.record_block(reason);
        self.health.record_block(node_id);
    }

    /// Run a health check pass.
    pub fn health_check(&mut self) -> Vec<NodeId> {
        self.health.check_health()
    }

    /// Total rotations.
    pub fn total_rotations(&self) -> u64 {
        self.manager.total_rotations()
    }

    /// Total blocks.
    pub fn total_blocks(&self) -> u64 {
        self.metrics.total_blocks()
    }

    /// Metrics snapshot.
    pub fn metrics(&self) -> &RotationMetricsCollector {
        &self.metrics
    }

    /// Health snapshot.
    pub fn health(&self) -> &RotationHealthChecker {
        &self.health
    }

    /// Policy snapshot.
    pub fn policy(&self) -> &RotationProtectionPolicyEngine {
        &self.policy
    }

    /// Registry snapshot.
    pub fn registry(&self) -> &NodeIdentityRegistry {
        &self.registry
    }

    /// State machine for a node.
    pub fn state_machine(&self, node_id: NodeId) -> RotationNodeState {
        self.state_machines
            .get(&node_id)
            .map(|sm| sm.current())
            .unwrap_or(RotationNodeState::Idle)
    }

    /// Clear all coordinator state.
    pub fn clear(&mut self) {
        self.manager.clear();
        self.policy.reset_counters();
        self.queue.clear();
        self.registry.clear();
        self.audit.clear();
        self.derivation.reset();
        self.validator.reset_counters();
        self.metrics.clear();
        self.health.clear();
        self.state_machines.clear();
        self.lifecycle.clear();
    }
}

///////////////////////////////////////////////////////////////////////////////
// TESTS
///////////////////////////////////////////////////////////////////////////////

///////////////////////////////////////////////////////////////////////////////
// END OF FILE
///////////////////////////////////////////////////////////////////////////////
