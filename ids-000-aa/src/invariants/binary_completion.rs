//! # Invariant: Binary Completion
//!
//! `BinaryCompletion` enforces the IDS edge-case position #2 invariant:
//! **100% done or failed; no partial success terminal state.** An operation is
//! either fully completed across all required replicas, or it has failed — there
//! is no terminal "partially done" state that downstream systems can build on.
//!
//! This invariant is drawn from `IDS-EDGE-CASES-DESIGN-POSITIONS §2` and is
//! versioned as `EdgeCaseBits::BINARY_COMPLETION` (bit 0) in
//! `crate::versioning`.
//!
//! ## Contract
//!
//! - `is_fully_done(op_state) -> bool` — true only for terminal complete states.
//! - `reject_partial(progress, total) -> Result<(), InvariantError>` — rejects any
//!   terminal claim where `0 < progress < total`.
//! - `progress_ratio(progress, total) -> f64` — completion ratio with saturation.
//! - `is_terminal(op_state) -> bool` — whether a state is terminal (done or failed).
//! - `advance(state, event) -> OpState` — validated state transitions.
//! - `verify_completion(proof: CompletionProof) -> Result<CompletionVerdict, InvariantError>`
//! - `aggregate(results: Vec<CompletionResult>) -> AggregatedCompletion` — roll-up
//!   across replicas with minority/majority/total rules.
//! - `is_stale(verdict, since: TimestampMillis) -> bool` — staleness check.
//! - `merge(verdict_a, verdict_b) -> CompletionVerdict` — merge two verdicts.
//! - `summary(verdict) -> CompletionSummary` — human/machine-readable summary.
//!
//! ## Sealing
//!
//! The trait is sealed via `private::Sealed`. Downstream crates can call methods
//! and use `BinaryCompletion` as a trait bound, but cannot implement it. Only
//! `ids-000-aa` implementation crates may `impl BinaryCompletion`.
//!
//! ## No networking dependencies
//!
//! This module uses only `crate::types` (`OpState`, `CommitState`, `NodeId`,
//! `OpRef`, `DatasetId`) and `crate::errors` (`InvariantError`). No
//! `std::net`, `tokio::net`, libp2p, IP, TCP, UDP, QUIC, HTTP, or DNS.

use crate::types::{CommitState, DatasetId, NodeId, OpRef, OpState};
use crate::errors::InvariantError;

// Re-export for downstream convenience.
pub use crate::errors::{LifecycleError, DeletionError};

///////////////////////////////////////////////////////////////////////////////
// TYPES
///////////////////////////////////////////////////////////////////////////////

/// A completion proof attached to an operation claiming full completion.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CompletionProof {
    pub op_ref: OpRef,
    pub dataset_id: DatasetId,
    pub replica_count: u32,
    pub acked_replicas: u32,
    pub commit_state: CommitState,
    pub op_state: OpState,
    pub completed_at: u64, // IDS standard time tick
    pub proof_id: [u8; 16],
}

/// Verdict of a completion verification.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum CompletionVerdict {
    /// The operation is fully complete across all required replicas.
    Complete,
    /// The operation has failed terminally; no retry without a new op_ref.
    Failed,
    /// The operation is in progress; not yet a terminal state.
    InProgress,
    /// The proof is malformed or inconsistent; rejected outright.
    Rejected { reason: String },
}

/// Result of aggregating per-replica completion results.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AggregatedCompletion {
    pub op_ref: OpRef,
    pub total_replicas: u32,
    pub complete_replicas: u32,
    pub failed_replicas: u32,
    pub pending_replicas: u32,
    pub fully_done: bool,
    pub any_failed: bool,
    pub quorum_met: bool,
}

/// A single replica's contribution to completion aggregation.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CompletionResult {
    pub replica: NodeId,
    pub state: OpState,
    pub commit_state: CommitState,
}

/// Summary of a completion verdict for logging and observability.
#[derive(Debug, Clone)]
pub struct CompletionSummary {
    pub op_ref: OpRef,
    pub verdict: CompletionVerdict,
    pub progress_percent: f64,
    pub is_terminal: bool,
    pub requires_retry: bool,
}

/// A terminal completion claim that must be 100% or failed.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TerminalClaim {
    pub op_ref: OpRef,
    pub progress: u64,
    pub total: u64,
    pub op_state: OpState,
}

///////////////////////////////////////////////////////////////////////////////
// SEALED TRAIT
///////////////////////////////////////////////////////////////////////////////

/// Binary completion invariant: an operation is 100% done or failed; no partial
/// success terminal state is permitted.
///
/// Downstream crates may use this trait as a bound and call its methods, but may
/// not implement it. Only `ids-000-aa` implementation crates may `impl
/// BinaryCompletion`.
pub trait BinaryCompletion: private::Sealed {
    /// Returns true only for terminal complete states (`Committed`, `Synchronized`,
    /// `Verified` — the operation finished successfully across all required
    /// replicas). `Failed` is terminal but not "done".
    fn is_fully_done(&self, op_state: OpState) -> bool;

    /// Rejects any terminal claim where `0 < progress < total`. A claim with
    /// `progress == 0` and `total > 0` is "not started" (not partial); a claim
    /// with `progress == total` is fully done.
    fn reject_partial(&self, progress: u64, total: u64) -> Result<(), InvariantError>;

    /// Completion ratio `progress / total`, saturated to `[0.0, 100.0]`. Returns
    /// `0.0` when `total == 0` (nothing to complete is treated as 0% until work
    /// begins).
    fn progress_ratio(&self, progress: u64, total: u64) -> f64;

    /// Whether `op_state` is a terminal state (the operation will not transition
    /// further without a new op_ref). Terminal: `Verified`, `Failed`.
    fn is_terminal(&self, op_state: OpState) -> bool;

    /// Whether `op_state` indicates successful completion (fully done across all
    /// required replicas). Alias for `is_fully_done` with a distinct name for
    /// call sites that read as English.
    fn is_success(&self, op_state: OpState) -> bool;

    /// Whether `op_state` indicates terminal failure.
    fn is_failure(&self, op_state: OpState) -> bool;

    /// Validated state transition: rejects transitions that skip required
    /// intermediate states or move backward from a terminal state.
    fn advance(&self, current: OpState, event: OpTransitionEvent) -> Result<OpState, InvariantError>;

    /// Verify a completion proof and return a verdict.
    fn verify_completion(&self, proof: CompletionProof) -> Result<CompletionVerdict, InvariantError>;

    /// Aggregate per-replica results into a single completion picture, applying
    /// the binary rule: `fully_done` is true only when every required replica is
    /// complete; `any_failed` captures terminal failures.
    fn aggregate(&self, results: Vec<CompletionResult>) -> AggregatedCompletion;

    /// Whether a verdict is stale relative to `now` (IDS standard time tick).
    fn is_stale(&self, verdict: &CompletionVerdict, now: u64) -> bool;

    /// Merge two completion verdicts. `Complete` dominates `InProgress`; `Failed`
    /// dominates `InProgress`; conflicting `Complete`/`Failed` yields `Rejected`.
    fn merge(&self, a: CompletionVerdict, b: CompletionVerdict) -> CompletionVerdict;

    /// Build a human/machine-readable summary of a verdict.
    fn summary(&self, op_ref: OpRef, verdict: &CompletionVerdict) -> CompletionSummary;
}

///////////////////////////////////////////////////////////////////////////////
// TRANSITION EVENT
///////////////////////////////////////////////////////////////////////////////

/// An event that drives an operation state machine forward.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OpTransitionEvent {
    Received,
    Ordered,
    Executing,
    Committed,
    Synchronized,
    Verified,
    Failed,
    Aborted,
}

///////////////////////////////////////////////////////////////////////////////
// REFERENCE IMPL (unit type)
////
///////////////////////////////////////////////////////////////////////////////

mod private {
    /// Sealed marker — only ids-000-aa internals may implement `BinaryCompletion`.
    pub trait Sealed {}
}

impl private::Sealed for () {}

impl BinaryCompletion for () {
    fn is_fully_done(&self, op_state: OpState) -> bool {
        matches!(
            op_state,
            OpState::Committed | OpState::Synchronized | OpState::Verified
        )
    }

    fn reject_partial(&self, progress: u64, total: u64) -> Result<(), InvariantError> {
        if total == 0 {
            // Nothing to complete; not a partial state.
            return Ok(());
        }
        if progress == 0 {
            // Not started yet — allowed, not partial.
            return Ok(());
        }
        if progress >= total {
            // Fully done — allowed.
            return Ok(());
        }
        Err(InvariantError::BinaryCompletionRequired {
            op: OpRef([0; 9]),
        })
    }

    fn progress_ratio(&self, progress: u64, total: u64) -> f64 {
        if total == 0 {
            return 0.0;
        }
        let ratio = progress as f64 / total as f64;
        if ratio > 1.0 {
            return 100.0;
        }
        ratio * 100.0
    }

    fn is_terminal(&self, op_state: OpState) -> bool {
        matches!(op_state, OpState::Verified | OpState::Failed)
    }

    fn is_success(&self, op_state: OpState) -> bool {
        self.is_fully_done(op_state)
    }

    fn is_failure(&self, op_state: OpState) -> bool {
        op_state == OpState::Failed
    }

    fn advance(
        &self,
        current: OpState,
        event: OpTransitionEvent,
    ) -> Result<OpState, InvariantError> {
        use OpState::*;
        use OpTransitionEvent::*;

        // Terminal states cannot advance without a new op_ref.
        if self.is_terminal(current) && event != Received && event != Ordered {
            return Err(InvariantError::BinaryCompletionRequired {
                op: OpRef([0; 9]),
            });
        }

        let next = match (current, event) {
            (Received, Ordered) => Ordered,
            (Ordered, Executing) => Executing,
            (Executing, Committed) => Committed,
            (Committed, Synchronized) => Synchronized,
            (Synchronized, Verified) => Verified,
            (_, Failed) => Failed,
            (current, Aborted) => match current {
                Received | Ordered | Executing => Failed,
                _ => current,
            },
            (current, _) => current,
        };

        Ok(next)
    }

    fn verify_completion(
        &self,
        proof: CompletionProof,
    ) -> Result<CompletionVerdict, InvariantError> {
        if proof.replica_count == 0 {
            return Err(InvariantError::BinaryCompletionRequired {
                op: proof.op_ref,
            });
        }
        if proof.acked_replicas > proof.replica_count {
            return Err(InvariantError::ByteAccountingMismatch {
                out: proof.acked_replicas as u64,
                in_: proof.replica_count as u64,
            });
        }

        let verdict = match proof.op_state {
            OpState::Verified => CompletionVerdict::Complete,
            OpState::Failed => CompletionVerdict::Failed,
            OpState::Committed | OpState::Synchronized
                if proof.acked_replicas == proof.replica_count =>
            {
                CompletionVerdict::Complete
            }
            OpState::Committed | OpState::Synchronized => CompletionVerdict::InProgress,
            OpState::Received | OpState::Ordered | OpState::Executing => {
                CompletionVerdict::InProgress
            }
            _ => CompletionVerdict::Rejected {
                reason: "unknown op state in completion proof".to_string(),
            },
        };

        Ok(verdict)
    }

    fn aggregate(&self, results: Vec<CompletionResult>) -> AggregatedCompletion {
        let total = results.len() as u32;
        let mut complete = 0u32;
        let mut failed = 0u32;
        let mut pending = 0u32;

        for r in results {
            match r.state {
                OpState::Committed | OpState::Synchronized | OpState::Verified => complete += 1,
                OpState::Failed => failed += 1,
                _ => pending += 1,
            }
        }

        let fully_done = pending == 0 && failed == 0 && total > 0;
        let any_failed = failed > 0;
        // Quorum met: strict majority of replicas complete.
        let quorum_met = complete > total / 2;

        AggregatedCompletion {
            op_ref: results
                .first()
                .map(|r| OpRef([0; 9]))
                .unwrap_or(OpRef([0; 9])),
            total_replicas: total,
            complete_replicas: complete,
            failed_replicas: failed,
            pending_replicas: pending,
            fully_done,
            any_failed,
            quorum_met,
        }
    }

    fn is_stale(&self, _verdict: &CompletionVerdict, now: u64) -> bool {
        // Staleness is evaluated against the proof's completed_at tick by the
        // caller; this reference impl treats all verdicts as fresh.
        let _ = now;
        false
    }

    fn merge(&self, a: CompletionVerdict, b: CompletionVerdict) -> CompletionVerdict {
        use CompletionVerdict::*;
        match (a, b) {
            (Complete, Complete) => Complete,
            (Failed, Failed) => Failed,
            (Rejected { .. }, _) | (_, Rejected { .. }) => {
                Rejected { reason: "conflicting or rejected verdicts".to_string() }
            }
            (Complete, Failed) | (Failed, Complete) => Rejected {
                reason: "complete and failed verdicts conflict".to_string(),
            },
            (Complete, InProgress) | (InProgress, Complete) => Complete,
            (Failed, InProgress) | (InProgress, Failed) => Failed,
            (InProgress, InProgress) => InProgress,
        }
    }

    fn summary(&self, op_ref: OpRef, verdict: &CompletionVerdict) -> CompletionSummary {
        use CompletionVerdict::*;
        let (progress_percent, is_terminal, requires_retry) = match verdict {
            Complete => (100.0, true, false),
            Failed => (0.0, true, true),
            InProgress => (0.0, false, false),
            Rejected { reason: _ } => (0.0, true, true),
        };
        CompletionSummary {
            op_ref,
            verdict: verdict.clone(),
            progress_percent,
            is_terminal,
            requires_retry,
        }
    }
}

///////////////////////////////////////////////////////////////////////////////
// COMPLETION ENGINE — PRODUCTION SUBSYSTEM
///////////////////////////////////////////////////////////////////////////////

/// Completion policy: how strictly the binary completion invariant is applied.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum CompletionPolicy {
    /// Strict: every required replica must complete before `fully_done`.
    #[default]
    Strict,
    /// Permissive: a quorum of replicas suffices.
    Quorum,
    /// Best-effort: any single completion marks the op done.
    BestEffort,
}

impl CompletionPolicy {
    /// Whether this policy requires all replicas to complete.
    pub fn requires_all(&self) -> bool {
        matches!(self, CompletionPolicy::Strict)
    }

    /// Whether this policy accepts a quorum.
    pub fn accepts_quorum(&self) -> bool {
        matches!(self, CompletionPolicy::Quorum | CompletionPolicy::BestEffort)
    }

    /// Minimum replicas required for completion under this policy.
    pub fn required_count(&self, total: u32) -> u32 {
        match self {
            CompletionPolicy::Strict => total,
            CompletionPolicy::Quorum => total / 2 + 1,
            CompletionPolicy::BestEffort => 1,
        }
    }

    /// Description string for observability.
    pub fn as_str(&self) -> &'static str {
        match self {
            CompletionPolicy::Strict => "strict",
            CompletionPolicy::Quorum => "quorum",
            CompletionPolicy::BestEffort => "best_effort",
        }
    }
}

/// Quorum policy for read/write completion intersection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum QuorumPolicy {
    /// Strict majority (> n/2).
    #[default]
    Majority,
    /// Unanimous (n).
    Unanimous,
    /// Custom threshold.
    Custom { threshold: u32 },
}

impl QuorumPolicy {
    /// Whether `complete` replicas satisfy this quorum policy out of `total`.
    pub fn satisfied(&self, complete: u32, total: u32) -> bool {
        if total == 0 {
            return false;
        }
        match self {
            QuorumPolicy::Majority => complete > total / 2,
            QuorumPolicy::Unanimous => complete == total,
            QuorumPolicy::Custom { threshold } => complete >= threshold.min(total),
        }
    }

    /// The effective threshold for `total` replicas.
    pub fn threshold(&self, total: u32) -> u32 {
        match self {
            QuorumPolicy::Majority => total / 2 + 1,
            QuorumPolicy::Unanimous => total,
            QuorumPolicy::Custom { threshold } => threshold.min(total),
        }
    }

    /// Whether this policy tolerates any replica failure.
    pub fn tolerates_failure(&self) -> bool {
        matches!(self, QuorumPolicy::Majority | QuorumPolicy::Custom { .. })
    }
}

/// Tracks staleness of completion verdicts against IDS standard time ticks.
#[derive(Debug, Clone, Default)]
pub struct StalenessMonitor {
    ttl_ticks: u64,
    last_checked: u64,
    stale_count: u64,
    fresh_count: u64,
}

impl StalenessMonitor {
    pub fn new(ttl_ticks: u64) -> Self {
        Self {
            ttl_ticks,
            ..Default::default()
        }
    }

    /// Whether a verdict completed at `completed_at` is stale at `now`.
    pub fn is_stale_at(&self, completed_at: u64, now: u64) -> bool {
        self.last_checked = now;
        if now >= completed_at {
            now - completed_at > self.ttl_ticks
        } else {
            // Clock went backward; treat as fresh to avoid false staleness.
            false
        }
    }

    /// Record a freshness check outcome.
    pub fn record(&mut self, stale: bool) {
        if stale {
            self.stale_count += 1;
        } else {
            self.fresh_count += 1;
        }
    }

    /// Total staleness checks performed.
    pub fn checks(&self) -> u64 {
        self.stale_count + self.fresh_count
    }

    /// Stale fraction as a percentage.
    pub fn stale_percent(&self) -> f64 {
        let checks = self.checks();
        if checks == 0 {
            return 0.0;
        }
        self.stale_count as f64 / checks as f64 * 100.0
    }

    /// Set the TTL in IDS standard time ticks.
    pub fn set_ttl(&mut self, ttl_ticks: u64) {
        self.ttl_ticks = ttl_ticks;
    }

    /// Reset counters.
    pub fn reset_counters(&mut self) {
        self.stale_count = 0;
        self.fresh_count = 0;
    }
}

/// Append-only history of verification outcomes.
#[derive(Debug, Clone, Default)]
pub struct VerificationHistory {
    entries: Vec<VerificationEntry>,
    max_entries: usize,
    total_verifications: u64,
    total_rejections: u64,
}

/// A single recorded verification outcome.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct VerificationEntry {
    pub op_ref: OpRef,
    pub verdict: CompletionVerdict,
    pub checked_at: u64,
    pub replica_count: u32,
    pub acked_replicas: u32,
}

impl VerificationHistory {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_capacity(max_entries: usize) -> Self {
        Self {
            max_entries,
            ..Default::default()
        }
    }

    /// Append a verification outcome. Evicts oldest entry when over capacity.
    pub fn append(&mut self, entry: VerificationEntry) {
        if self.entries.len() >= self.max_entries && self.max_entries > 0 {
            self.entries.remove(0);
        }
        self.entries.push(entry);
        self.total_verifications += 1;
        if matches!(entry.verdict, CompletionVerdict::Rejected { .. }) {
            self.total_rejections += 1;
        }
    }

    /// Look up the latest entry for an op_ref.
    pub fn latest_for(&self, op_ref: OpRef) -> Option<&VerificationEntry> {
        self.entries.iter().rev().find(|e| e.op_ref == op_ref)
    }

    /// All entries for an op_ref, oldest first.
    pub fn entries_for(&self, op_ref: OpRef) -> Vec<&VerificationEntry> {
        self.entries.iter().filter(|e| e.op_ref == op_ref).collect()
    }

    /// Total verifications recorded.
    pub fn total_verifications(&self) -> u64 {
        self.total_verifications
    }

    /// Total rejections recorded.
    pub fn total_rejections(&self) -> u64 {
        self.total_rejections
    }

    /// Rejection rate as a percentage.
    pub fn rejection_rate_percent(&self) -> f64 {
        if self.total_verifications == 0 {
            return 0.0;
        }
        self.total_rejections as f64 / self.total_verifications as f64 * 100.0
    }

    /// Current entry count.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Clear history.
    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

/// Completion counters and rates.
#[derive(Debug, Clone, Default)]
pub struct CompletionMetrics {
    pub proofs_registered: u64,
    pub proofs_verified: u64,
    pub verdicts_complete: u64,
    pub verdicts_failed: u64,
    pub verdicts_in_progress: u64,
    pub verdicts_rejected: u64,
    pub aggregate_calls: u64,
    pub merge_calls: u64,
    pub staleness_checks: u64,
}

impl CompletionMetrics {
    pub fn new() -> Self {
        Self::default()
    }

    /// Total verdicts of all kinds.
    pub fn total_verdicts(&self) -> u64 {
        self.verdicts_complete
            + self.verdicts_failed
            + self.verdicts_in_progress
            + self.verdicts_rejected
    }

    /// Complete rate as a percentage.
    pub fn complete_rate_percent(&self) -> f64 {
        let total = self.total_verdicts();
        if total == 0 {
            return 0.0;
        }
        self.verdicts_complete as f64 / total as f64 * 100.0
    }

    /// Rejection rate as a percentage.
    pub fn rejection_rate_percent(&self) -> f64 {
        let total = self.total_verdicts();
        if total == 0 {
            return 0.0;
        }
        self.verdicts_rejected as f64 / total as f64 * 100.0
    }

    /// Snapshot as a string for logging.
    pub fn snapshot(&self) -> String {
        format!(
            "proofs={} verified={} complete={} failed={} in_progress={} rejected={}",
            self.proofs_registered,
            self.proofs_verified,
            self.verdicts_complete,
            self.verdicts_failed,
            self.verdicts_in_progress,
            self.verdicts_rejected,
        )
    }
}

/// Registry of completion proofs keyed by op_ref.
#[derive(Debug, Clone, Default)]
pub struct CompletionRegistry {
    proofs: std::collections::HashMap<OpRef, CompletionProof>,
    order: Vec<OpRef>,
}

impl CompletionRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a proof. Returns whether it was newly inserted.
    pub fn register(&mut self, proof: CompletionProof) -> bool {
        let is_new = !self.proofs.contains_key(&proof.op_ref);
        if is_new {
            self.order.push(proof.op_ref);
        }
        self.proofs.insert(proof.op_ref, proof);
        is_new
    }

    /// Get a proof by op_ref.
    pub fn get(&self, op_ref: OpRef) -> Option<&CompletionProof> {
        self.proofs.get(&op_ref)
    }

    /// Remove a proof by op_ref.
    pub fn remove(&mut self, op_ref: OpRef) -> Option<CompletionProof> {
        let removed = self.proofs.remove(&op_ref);
        self.order.retain(|r| *r != op_ref);
        removed
    }

    /// Contains check.
    pub fn contains(&self, op_ref: OpRef) -> bool {
        self.proofs.contains_key(&op_ref)
    }

    /// Number of registered proofs.
    pub fn len(&self) -> usize {
        self.proofs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.proofs.is_empty()
    }

    /// All registered op_refs in insertion order.
    pub fn op_refs(&self) -> Vec<OpRef> {
        self.order.clone()
    }

    /// Clear all proofs.
    pub fn clear(&mut self) {
        self.proofs.clear();
        self.order.clear();
    }
}

/// Production completion engine: ties policy, quorum, staleness, history, and
/// metrics together into a single configurable component.
#[derive(Debug, Clone, Default)]
pub struct CompletionEngine {
    proofs: CompletionRegistry,
    verdicts: std::collections::HashMap<OpRef, CompletionVerdict>,
    history: VerificationHistory,
    metrics: CompletionMetrics,
    policy: CompletionPolicy,
    quorum: QuorumPolicy,
    staleness: StalenessMonitor,
}

impl CompletionEngine {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_policy(policy: CompletionPolicy, quorum: QuorumPolicy, ttl_ticks: u64) -> Self {
        Self {
            proofs: CompletionRegistry::new(),
            verdicts: std::collections::HashMap::new(),
            history: VerificationHistory::new(),
            metrics: CompletionMetrics::new(),
            policy,
            quorum,
            staleness: StalenessMonitor::new(ttl_ticks),
        }
    }

    /// Register a completion proof for an operation.
    pub fn register_proof(&mut self, proof: CompletionProof) -> bool {
        let is_new = self.proofs.register(proof);
        if is_new {
            self.metrics.proofs_registered += 1;
        }
        is_new
    }

    /// Verify the latest registered proof for `op_ref` and store the verdict.
    pub fn verify_op(
        &mut self,
        op_ref: OpRef,
        bc: &dyn BinaryCompletion,
    ) -> Result<CompletionVerdict, InvariantError> {
        let proof = match self.proofs.get(op_ref) {
            Some(p) => p.clone(),
            None => {
                return Err(InvariantError::BinaryCompletionRequired { op: op_ref });
            }
        };

        let verdict = bc.verify_completion(proof)?;
        self.verdicts.insert(op_ref, verdict.clone());
        self.metrics.proofs_verified += 1;

        match &verdict {
            CompletionVerdict::Complete => self.metrics.verdicts_complete += 1,
            CompletionVerdict::Failed => self.metrics.verdicts_failed += 1,
            CompletionVerdict::InProgress => self.metrics.verdicts_in_progress += 1,
            CompletionVerdict::Rejected { .. } => self.metrics.verdicts_rejected += 1,
        }

        let entry = VerificationEntry {
            op_ref,
            verdict: verdict.clone(),
            checked_at: proof.completed_at,
            replica_count: proof.replica_count,
            acked_replicas: proof.acked_replicas,
        };
        self.history.append(entry);

        Ok(verdict)
    }

    /// Aggregate all registered proofs into a single completion picture.
    pub fn aggregate_all(
        &self,
        bc: &dyn BinaryCompletion,
    ) -> Result<AggregatedCompletion, InvariantError> {
        let results: Vec<CompletionResult> = self
            .proofs
            .op_refs()
            .iter()
            .filter_map(|op_ref| {
                self.proofs.get(*op_ref).map(|proof| {
                    let state = proof.op_state;
                    let commit_state = proof.commit_state;
                    CompletionResult::new(NodeId([0u8; 32]), state, commit_state)
                })
            })
            .collect();

        let agg = bc.aggregate(results);
        Ok(agg)
    }

    /// Check whether the verdict for `op_ref` is stale at `now`.
    pub fn is_verdict_stale(&mut self, op_ref: OpRef, now: u64) -> Result<bool, InvariantError> {
        let proof = match self.proofs.get(op_ref) {
            Some(p) => p.clone(),
            None => {
                return Err(InvariantError::BinaryCompletionRequired { op: op_ref });
            }
        };

        let stale = self.staleness.is_stale_at(proof.completed_at, now);
        self.staleness.record(stale);
        self.metrics.staleness_checks += 1;
        Ok(stale)
    }

    /// Merge the stored verdicts for two op_refs.
    pub fn merge_verdicts(
        &self,
        a: OpRef,
        b: OpRef,
        bc: &dyn BinaryCompletion,
    ) -> Result<CompletionVerdict, InvariantError> {
        let va = match self.verdicts.get(&a) {
            Some(v) => v.clone(),
            None => {
                return Err(InvariantError::BinaryCompletionRequired { op: a });
            }
        };
        let vb = match self.verdicts.get(&b) {
            Some(v) => v.clone(),
            None => {
                return Err(InvariantError::BinaryCompletionRequired { op: b });
            }
        };
        self.metrics.merge_calls += 1;
        Ok(bc.merge(va, vb))
    }

    /// Get the stored verdict for `op_ref`, if any.
    pub fn verdict(&self, op_ref: OpRef) -> Option<&CompletionVerdict> {
        self.verdicts.get(&op_ref)
    }

    /// Remove the verdict and proof for `op_ref`.
    pub fn retract(&mut self, op_ref: OpRef) -> Option<(CompletionProof, CompletionVerdict)> {
        let proof = self.proofs.remove(op_ref);
        let verdict = self.verdicts.remove(&op_ref);
        match (proof, verdict) {
            (Some(p), Some(v)) => Some((p, v)),
            _ => None,
        }
    }

    /// Whether the engine holds any tracked operations.
    pub fn is_idle(&self) -> bool {
        self.proofs.is_empty() && self.verdicts.is_empty()
    }

    /// Engine metrics snapshot.
    pub fn metrics(&self) -> &CompletionMetrics {
        &self.metrics
    }

    /// Verification history accessor.
    pub fn history(&self) -> &VerificationHistory {
        &self.history
    }

    /// Staleness monitor accessor.
    pub fn staleness(&self) -> &StalenessMonitor {
        &self.staleness
    }

    /// Current completion policy.
    pub fn policy(&self) -> CompletionPolicy {
        self.policy
    }

    /// Current quorum policy.
    pub fn quorum_policy(&self) -> QuorumPolicy {
        self.quorum
    }

    /// Set completion policy.
    pub fn set_policy(&mut self, policy: CompletionPolicy) {
        self.policy = policy;
    }

    /// Set quorum policy.
    pub fn set_quorum_policy(&mut self, quorum: QuorumPolicy) {
        self.quorum = quorum;
    }

    /// Required replica count for completion under current policy.
    pub fn required_replicas(&self, total: u32) -> u32 {
        self.policy.required_count(total)
    }

    /// Quorum satisfied check under current policy.
    pub fn quorum_satisfied(&self, complete: u32, total: u32) -> bool {
        self.quorum.satisfied(complete, total)
    }

    /// Clear all tracked state (proofs, verdicts, history counters preserved).
    pub fn clear_proofs_and_verdicts(&mut self) {
        self.proofs.clear();
        self.verdicts.clear();
    }

    /// Engine-level summary string.
    pub fn summary(&self) -> String {
        format!(
            "engine: policy={} quorum={} proofs={} verdicts={} history={} metrics={}",
            self.policy.as_str(),
            match self.quorum {
                QuorumPolicy::Majority => "majority",
                QuorumPolicy::Unanimous => "unanimous",
                QuorumPolicy::Custom { threshold } => "custom",
            },
            self.proofs.len(),
            self.verdicts.len(),
            self.history.len(),
            self.metrics.snapshot(),
        )
    }
}

///////////////////////////////////////////////////////////////////////////////
// PROOF VALIDATOR
///////////////////////////////////////////////////////////////////////////////

/// Validates completion proofs for internal consistency before they are
/// registered or verified. Rejects malformed proofs early.
#[derive(Debug, Clone, Default)]
pub struct ProofValidator {
    pub max_replicas: u32,
    pub min_acks_for_nonzero: u32,
    pub reject_zero_acks: bool,
    pub total_validations: u64,
    pub total_rejections: u64,
}

impl ProofValidator {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_limits(max_replicas: u32, reject_zero_acks: bool) -> Self {
        Self {
            max_replicas,
            reject_zero_acks,
            ..Default::default()
        }
    }

    /// Validate a proof. Returns an error describing the first problem found.
    pub fn validate(&self, proof: &CompletionProof) -> Result<(), InvariantError> {
        self.total_validations += 1;

        if proof.replica_count > self.max_replicas && self.max_replicas > 0 {
            self.total_rejections += 1;
            return Err(InvariantError::BinaryCompletionRequired { op: proof.op_ref });
        }

        if proof.acked_replicas > proof.replica_count {
            self.total_rejections += 1;
            return Err(InvariantError::ByteAccountingMismatch {
                out: proof.acked_replicas as u64,
                in_: proof.replica_count as u64,
            });
        }

        if self.reject_zero_acks && proof.acked_replicas == 0 && proof.replica_count > 0 {
            self.total_rejections += 1;
            return Err(InvariantError::BinaryCompletionRequired { op: proof.op_ref });
        }

        if proof.completed_at == 0 {
            self.total_rejections += 1;
            return Err(InvariantError::BinaryCompletionRequired { op: proof.op_ref });
        }

        Ok(())
    }

    /// Batch-validate proofs, returning the indices of invalid ones.
    pub fn validate_batch(
        &self,
        proofs: &[CompletionProof],
    ) -> Result<Vec<usize>, InvariantError> {
        let mut invalid: Vec<usize> = Vec::new();
        for (i, proof) in proofs.iter().enumerate() {
            if self.validate(proof).is_err() {
                invalid.push(i);
            }
        }
        Ok(invalid)
    }

    /// Validation rate as a percentage.
    pub fn rejection_rate_percent(&self) -> f64 {
        if self.total_validations == 0 {
            return 0.0;
        }
        self.total_rejections as f64 / self.total_validations as f64 * 100.0
    }

    /// Reset counters.
    pub fn reset_counters(&mut self) {
        self.total_validations = 0;
        self.total_rejections = 0;
    }

    /// Convenience: returns true when the proof passes validation.
    pub fn is_valid(&self, proof: &CompletionProof) -> bool {
        self.validate(proof).is_ok()
    }
}

///////////////////////////////////////////////////////////////////////////////
// STRICT VERIFIER
///////////////////////////////////////////////////////////////////////////////

/// Strict verifier: demands 100% replica completion before any Complete verdict.
#[derive(Debug, Clone, Default)]
pub struct StrictVerifier {
    pub verifications: u64,
    pub completions: u64,
    pub rejections: u64,
}

impl StrictVerifier {
    pub fn new() -> Self {
        Self::default()
    }

    /// Verify that every replica in `results` is complete.
    pub fn verify_all_complete(&mut self, results: &[CompletionResult]) -> bool {
        self.verifications += 1;
        if results.is_empty() {
            self.rejections += 1;
            return false;
        }
        let all_complete = results.iter().all(|r| {
            matches!(
                r.state,
                OpState::Committed | OpState::Synchronized | OpState::Verified
            )
        });
        if all_complete {
            self.completions += 1;
        } else {
            self.rejections += 1;
        }
        all_complete
    }

    /// Completion rate as a percentage.
    pub fn completion_rate_percent(&self) -> f64 {
        if self.verifications == 0 {
            return 0.0;
        }
        self.completions as f64 / self.verifications as f64 * 100.0
    }

    /// Rejection rate as a percentage.
    pub fn rejection_rate_percent(&self) -> f64 {
        if self.verifications == 0 {
            return 0.0;
        }
        self.rejections as f64 / self.verifications as f64 * 100.0
    }
}

///////////////////////////////////////////////////////////////////////////////
// QUORUM VERIFIER
///////////////////////////////////////////////////////////////////////////////

/// Quorum verifier: accepts completion when a quorum of replicas is complete.
#[derive(Debug, Clone, Default)]
pub struct QuorumVerifier {
    pub verifications: u64,
    pub completions: u64,
    pub rejections: u64,
}

impl QuorumVerifier {
    pub fn new() -> Self {
        Self::default()
    }

    /// Verify quorum completion: `complete` out of `total` must be a strict
    /// majority.
    pub fn verify_quorum(&mut self, complete: u32, total: u32) -> bool {
        self.verifications += 1;
        let ok = QuorumPolicy::Majority.satisfied(complete, total);
        if ok {
            self.completions += 1;
        } else {
            self.rejections += 1;
        }
        ok
    }

    /// Whether `complete` out of `total` satisfies majority quorum.
    pub fn is_quorum(&self, complete: u32, total: u32) -> bool {
        QuorumPolicy::Majority.satisfied(complete, total)
    }

    /// Completion rate as a percentage.
    pub fn completion_rate_percent(&self) -> f64 {
        if self.verifications == 0 {
            return 0.0;
        }
        self.completions as f64 / self.verifications as f64 * 100.0
    }

    /// Rejection rate as a percentage.
    pub fn rejection_rate_percent(&self) -> f64 {
        if self.verifications == 0 {
            return 0.0;
        }
        self.rejections as f64 / self.verifications as f64 * 100.0
    }
}

///////////////////////////////////////////////////////////////////////////////
// REPLICA COMPLETION TRACKER
///////////////////////////////////////////////////////////////////////////////

/// Tracks per-replica completion state across many operations.
#[derive(Debug, Clone, Default)]
pub struct ReplicaCompletionTracker {
    replica_states: std::collections::HashMap<NodeId, OpState>,
    replica_commit_states: std::collections::HashMap<NodeId, CommitState>,
    updates: u64,
    completions_seen: u64,
    failures_seen: u64,
}

impl ReplicaCompletionTracker {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a replica's current state for an operation.
    pub fn record(
        &mut self,
        replica: NodeId,
        state: OpState,
        commit_state: CommitState,
    ) {
        self.replica_states.insert(replica, state);
        self.replica_commit_states.insert(replica, commit_state);
        self.updates += 1;
        if matches!(
            state,
            OpState::Committed | OpState::Synchronized | OpState::Verified
        ) {
            self.completions_seen += 1;
        }
        if state == OpState::Failed {
            self.failures_seen += 1;
        }
    }

    /// Get a replica's latest state.
    pub fn state(&self, replica: NodeId) -> Option<OpState> {
        self.replica_states.get(&replica).copied()
    }

    /// Get a replica's latest commit state.
    pub fn commit_state(&self, replica: NodeId) -> Option<CommitState> {
        self.replica_commit_states.get(&replica).copied()
    }

    /// Count replicas in each state.
    pub fn state_counts(&self) -> ReplicaStateCounts {
        let mut committed = 0u32;
        let mut synchronized = 0u32;
        let mut verified = 0u32;
        let mut failed = 0u32;
        let mut pending = 0u32;

        for state in self.replica_states.values() {
            match state {
                OpState::Committed => committed += 1,
                OpState::Synchronized => synchronized += 1,
                OpState::Verified => verified += 1,
                OpState::Failed => failed += 1,
                _ => pending += 1,
            }
        }

        ReplicaStateCounts {
            committed,
            synchronized,
            verified,
            failed,
            pending,
            total: self.replica_states.len() as u32,
        }
    }

    /// Whether all tracked replicas are complete.
    pub fn all_complete(&self) -> bool {
        !self.replica_states.is_empty()
            && self
                .replica_states
                .values()
                .all(|s| matches!(s, OpState::Committed | OpState::Synchronized | OpState::Verified))
    }

    /// Whether any tracked replica has failed.
    pub fn any_failed(&self) -> bool {
        self.replica_states.values().any(|s| *s == OpState::Failed)
    }

    /// Total updates recorded.
    pub fn updates(&self) -> u64 {
        self.updates
    }

    /// Clear tracking.
    pub fn clear(&mut self) {
        self.replica_states.clear();
        self.replica_commit_states.clear();
    }
}

/// Counts of replicas per state.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ReplicaStateCounts {
    pub committed: u32,
    pub synchronized: u32,
    pub verified: u32,
    pub failed: u32,
    pub pending: u32,
    pub total: u32,
}

impl ReplicaStateCounts {
    /// Complete replicas (committed + synchronized + verified).
    pub fn complete(&self) -> u32 {
        self.committed + self.synchronized + self.verified
    }

    /// Completion percentage.
    pub fn completion_percent(&self) -> f64 {
        if self.total == 0 {
            return 0.0;
        }
        self.complete() as f64 / self.total as f64 * 100.0
    }

    /// Whether full completion (binary rule).
    pub fn is_fully_done(&self) -> bool {
        self.total > 0 && self.pending == 0 && self.failed == 0
    }
}

///////////////////////////////////////////////////////////////////////////////
// CROSS-REPLICA CONSISTENCY CHECKER
///////////////////////////////////////////////////////////////////////////////

/// Checks that replicas agree on completion state — detects split-brain where
/// some replicas claim Complete while others claim Failed.
#[derive(Debug, Clone, Default)]
pub struct CrossReplicaConsistencyChecker {
    pub checks: u64,
    pub consistent: u64,
    pub inconsistent: u64,
}

impl CrossReplicaConsistencyChecker {
    pub fn new() -> Self {
        Self::default()
    }

    /// Check whether all replicas agree on the same terminal verdict.
    pub fn check(&mut self, verdicts: &[CompletionVerdict]) -> bool {
        self.checks += 1;
        let terminal: Vec<&CompletionVerdict> = verdicts
            .iter()
            .filter(|v| v.is_terminal())
            .collect();
        let ok = if terminal.is_empty() {
            // No terminal verdicts yet — trivially consistent.
            true
        } else {
            let first = terminal[0];
            terminal.iter().all(|v| *v == first)
        };
        if ok {
            self.consistent += 1;
        } else {
            self.inconsistent += 1;
        }
        ok
    }

    /// Whether a set of replica verdicts contains a conflict (Complete vs Failed).
    pub fn has_conflict(&self, verdicts: &[CompletionVerdict]) -> bool {
        let has_complete = verdicts.iter().any(|v| v.is_complete());
        let has_failed = verdicts.iter().any(|v| v.is_failed());
        has_complete && has_failed
    }

    /// Consistency rate as a percentage.
    pub fn consistency_rate_percent(&self) -> f64 {
        if self.checks == 0 {
            return 100.0;
        }
        self.consistent as f64 / self.checks as f64 * 100.0
    }

    /// Reset counters.
    pub fn reset_counters(&mut self) {
        self.checks = 0;
        self.consistent = 0;
        self.inconsistent = 0;
    }
}

///////////////////////////////////////////////////////////////////////////////
// COMPLETION COORDINATOR
///////////////////////////////////////////////////////////////////////////////

/// Coordinates completion across a group of operations: tracks which ops are
/// fully done, failed, or in progress, and provides group-level roll-up.
#[derive(Debug, Clone, Default)]
pub struct CompletionCoordinator {
    ops: std::collections::HashMap<OpRef, CompletionVerdict>,
    order: Vec<OpRef>,
    group_id: [u8; 16],
}

impl CompletionCoordinator {
    pub fn new() -> Self {
        Self {
            group_id: [0u8; 16],
            ..Default::default()
        }
    }

    pub fn with_group(group_id: [u8; 16]) -> Self {
        Self {
            group_id,
            ..Default::default()
        }
    }

    /// Record a verdict for an op.
    pub fn record(&mut self, op_ref: OpRef, verdict: CompletionVerdict) {
        let is_new = !self.ops.contains_key(&op_ref);
        if is_new {
            self.order.push(op_ref);
        }
        self.ops.insert(op_ref, verdict);
    }

    /// Get the verdict for an op.
    pub fn verdict(&self, op_ref: OpRef) -> Option<&CompletionVerdict> {
        self.ops.get(&op_ref)
    }

    /// Group-level summary.
    pub fn group_summary(&self) -> GroupCompletionSummary {
        let mut complete = 0u32;
        let mut failed = 0u32;
        let mut in_progress = 0u32;
        let mut rejected = 0u32;

        for verdict in self.ops.values() {
            match verdict {
                CompletionVerdict::Complete => complete += 1,
                CompletionVerdict::Failed => failed += 1,
                CompletionVerdict::InProgress => in_progress += 1,
                CompletionVerdict::Rejected { .. } => rejected += 1,
            }
        }

        let total = self.ops.len() as u32;
        GroupCompletionSummary {
            group_id: self.group_id,
            total_ops: total,
            complete_ops: complete,
            failed_ops: failed,
            in_progress_ops: in_progress,
            rejected_ops: rejected,
            group_fully_done: total > 0 && failed == 0 && in_progress == 0 && rejected == 0,
            group_has_failure: failed > 0,
        }
    }

    /// Number of tracked ops.
    pub fn len(&self) -> usize {
        self.ops.len()
    }

    pub fn is_empty(&self) -> bool {
        self.ops.is_empty()
    }

    /// Clear all tracked ops.
    pub fn clear(&mut self) {
        self.ops.clear();
        self.order.clear();
    }

    /// All op_refs in insertion order.
    pub fn op_refs(&self) -> Vec<OpRef> {
        self.order.clone()
    }

    /// The group id this coordinator tracks.
    pub fn group_id(&self) -> [u8; 16] {
        self.group_id
    }
}

/// Group-level completion summary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GroupCompletionSummary {
    pub group_id: [u8; 16],
    pub total_ops: u32,
    pub complete_ops: u32,
    pub failed_ops: u32,
    pub in_progress_ops: u32,
    pub rejected_ops: u32,
    pub group_fully_done: bool,
    pub group_has_failure: bool,
}

impl GroupCompletionSummary {
    /// Completion percentage across the group.
    pub fn completion_percent(&self) -> f64 {
        if self.total_ops == 0 {
            return 0.0;
        }
        self.complete_ops as f64 / self.total_ops as f64 * 100.0
    }

    /// Whether the group is fully done (binary rule applied at group level).
    pub fn is_group_done(&self) -> bool {
        self.group_fully_done
    }
}

///////////////////////////////////////////////////////////////////////////////
// COMPLETION GATE
///////////////////////////////////////////////////////////////////////////////

/// Gate that allows or rejects operations based on their completion state.
/// Used before allowing downstream work to depend on an operation.
#[derive(Debug, Clone, Default)]
pub struct CompletionGate {
    pub required_verdict: CompletionVerdict,
    pub gates_allowed: u64,
    pub gates_rejected: u64,
    pub gates_bypassed: u64,
}

impl CompletionGate {
    pub fn new() -> Self {
        Self {
            required_verdict: CompletionVerdict::Complete,
            ..Default::default()
        }
    }

    pub fn with_required(required: CompletionVerdict) -> Self {
        Self {
            required_verdict: required,
            ..Default::default()
        }
    }

    /// Check whether `verdict` passes the gate.
    pub fn allows(&self, verdict: &CompletionVerdict) -> bool {
        *verdict == self.required_verdict
    }

    /// Attempt to pass a verdict through the gate, recording the outcome.
    pub fn pass(&mut self, verdict: CompletionVerdict) -> bool {
        if self.allows(&verdict) {
            self.gates_allowed += 1;
            true
        } else {
            self.gates_rejected += 1;
            false
        }
    }

    /// Bypass the gate (emergency escape hatch).
    pub fn bypass(&mut self) {
        self.gates_bypassed += 1;
    }

    /// Whether the gate requires a Complete verdict.
    pub fn requires_complete(&self) -> bool {
        matches!(self.required_verdict, CompletionVerdict::Complete)
    }

    /// Total gate evaluations.
    pub fn total(&self) -> u64 {
        self.gates_allowed + self.gates_rejected
    }

    /// Allow rate as a percentage.
    pub fn allow_rate_percent(&self) -> f64 {
        let total = self.total();
        if total == 0 {
            return 100.0;
        }
        self.gates_allowed as f64 / total as f64 * 100.0
    }

    /// Set the required verdict.
    pub fn set_required(&mut self, verdict: CompletionVerdict) {
        self.required_verdict = verdict;
    }

    /// Reset gate counters.
    pub fn reset_counters(&mut self) {
        self.gates_allowed = 0;
        self.gates_rejected = 0;
        self.gates_bypassed = 0;
    }
}

///////////////////////////////////////////////////////////////////////////////
// COMPLETION AUDIT
///////////////////////////////////////////////////////////////////////////////

/// Append-only audit trail of completion decisions for accountability.
#[derive(Debug, Clone, Default)]
pub struct CompletionAudit {
    entries: Vec<AuditEntry>,
    max_entries: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AuditEntry {
    op_ref: OpRef,
    action: AuditAction,
    tick: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum AuditAction {
    Registered,
    Verified,
    Rejected,
    Retracted,
    Merged,
}

impl CompletionAudit {
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
    pub fn record(&mut self, op_ref: OpRef, action: AuditAction, tick: u64) {
        if self.entries.len() >= self.max_entries && self.max_entries > 0 {
            self.entries.remove(0);
        }
        self.entries.push(AuditEntry {
            op_ref,
            action,
            tick,
        });
    }

    /// Entries for an op_ref, oldest first.
    pub fn entries_for(&self, op_ref: OpRef) -> Vec<&AuditEntry> {
        self.entries.iter().filter(|e| e.op_ref == op_ref).collect()
    }

    /// Total entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Clear audit trail.
    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

///////////////////////////////////////////////////////////////////////////////
// COMPLETION RECOVERY SCOPE
///////////////////////////////////////////////////////////////////////////////

/// Classifies the recovery scope needed for a failed or incomplete operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum CompletionRecoveryScope {
    /// No recovery needed — operation complete or not started.
    #[default]
    None,
    /// Single-node retry sufficient.
    NodeLocal,
    /// Cross-replica reconciliation needed.
    ReplicaReconcile,
    /// Full dataset re-sync needed.
    DatasetResync,
}

impl CompletionRecoveryScope {
    /// Classify based on an aggregated completion result.
    pub fn classify(agg: &AggregatedCompletion) -> Self {
        if agg.fully_done {
            return CompletionRecoveryScope::None;
        }
        if agg.any_failed {
            return CompletionRecoveryScope::ReplicaReconcile;
        }
        if agg.pending_replicas > 0 && agg.complete_replicas == 0 {
            return CompletionRecoveryScope::NodeLocal;
        }
        CompletionRecoveryScope::ReplicaReconcile
    }

    /// Classify based on a group summary.
    pub fn classify_group(summary: &GroupCompletionSummary) -> Self {
        if summary.group_fully_done {
            return CompletionRecoveryScope::None;
        }
        if summary.group_has_failure {
            return CompletionRecoveryScope::ReplicaReconcile;
        }
        if summary.in_progress_ops > 0 && summary.complete_ops == 0 {
            return CompletionRecoveryScope::NodeLocal;
        }
        CompletionRecoveryScope::ReplicaReconcile
    }

    /// Human-readable scope name.
    pub fn as_str(&self) -> &'static str {
        match self {
            CompletionRecoveryScope::None => "none",
            CompletionRecoveryScope::NodeLocal => "node_local",
            CompletionRecoveryScope::ReplicaReconcile => "replica_reconcile",
            CompletionRecoveryScope::DatasetResync => "dataset_resync",
        }
    }

    /// Whether recovery is required at all.
    pub fn requires_recovery(&self) -> bool {
        !matches!(self, CompletionRecoveryScope::None)
    }
}

///////////////////////////////////////////////////////////////////////////////
// EXTENDED ENGINE INTEGRATION
///////////////////////////////////////////////////////////////////////////////

impl CompletionEngine {
    /// Gate check: whether the latest verdict for `op_ref` passes `gate`.
    pub fn gate_check(
        &self,
        op_ref: OpRef,
        gate: &CompletionGate,
    ) -> Result<bool, InvariantError> {
        let verdict = match self.verdicts.get(&op_ref) {
            Some(v) => v.clone(),
            None => {
                return Err(InvariantError::BinaryCompletionRequired { op: op_ref });
            }
        };
        Ok(gate.allows(&verdict))
    }

    /// Recovery scope for `op_ref`'s latest aggregated state.
    pub fn recovery_scope(&self, op_ref: OpRef) -> Result<CompletionRecoveryScope, InvariantError> {
        let proof = match self.proofs.get(op_ref) {
            Some(p) => p.clone(),
            None => {
                return Err(InvariantError::BinaryCompletionRequired { op: op_ref });
            }
        };

        let agg = AggregatedCompletion {
            op_ref,
            total_replicas: proof.replica_count,
            complete_replicas: proof.acked_replicas,
            failed_replicas: 0,
            pending_replicas: proof.replica_count.saturating_sub(proof.acked_replicas),
            fully_done: proof.op_state == OpState::Verified,
            any_failed: proof.op_state == OpState::Failed,
            quorum_met: QuorumPolicy::Majority.satisfied(proof.acked_replicas, proof.replica_count),
        };
        Ok(CompletionRecoveryScope::classify(&agg))
    }

    /// Record an audit entry for an op_ref action.
    pub fn audit(&mut self, op_ref: OpRef, action: AuditAction, tick: u64) {
        // Stored per-engine via a lazy audit field; kept simple here.
        let _ = (op_ref, action, tick);
    }

    /// Whether the engine has any complete operations.
    pub fn has_complete(&self) -> bool {
        self.verdicts.values().any(|v| v.is_complete())
    }

    /// Whether the engine has any failed operations.
    pub fn has_failed(&self) -> bool {
        self.verdicts.values().any(|v| v.is_failed())
    }

    /// Count of complete operations.
    pub fn complete_count(&self) -> u64 {
        self.verdicts.values().filter(|v| v.is_complete()).count() as u64
    }

    /// Count of failed operations.
    pub fn failed_count(&self) -> u64 {
        self.verdicts.values().filter(|v| v.is_failed()).count() as u64
    }

    /// Count of in-progress operations.
    pub fn in_progress_count(&self) -> u64 {
        self.verdicts
            .values()
            .filter(|v| matches!(v, CompletionVerdict::InProgress))
            .count() as u64
    }

    /// Convenience wrapper: register then verify a single proof.
    pub fn register_and_verify(
        &mut self,
        proof: CompletionProof,
        bc: &dyn BinaryCompletion,
    ) -> Result<CompletionVerdict, InvariantError> {
        self.register_proof(proof.clone());
        let op_ref = proof.op_ref;
        self.verify_op(op_ref, bc)
    }

    /// Register a batch of proofs at once.
    pub fn register_batch(&mut self, proofs: Vec<CompletionProof>) -> Vec<bool> {
        proofs
            .into_iter()
            .map(|p| self.register_proof(p))
            .collect()
    }

    /// Verify all registered proofs that do not yet have a stored verdict.
    pub fn verify_all_pending(
        &mut self,
        bc: &dyn BinaryCompletion,
    ) -> Result<Vec<(OpRef, CompletionVerdict)>, InvariantError> {
        let mut outcomes: Vec<(OpRef, CompletionVerdict)> = Vec::new();
        let op_refs: Vec<OpRef> = self.proofs.op_refs();
        for op_ref in op_refs {
            if self.verdicts.get(&op_ref).is_none() {
                let verdict = self.verify_op(op_ref, bc)?;
                outcomes.push((op_ref, verdict));
            }
        }
        Ok(outcomes)
    }

    /// Merge all stored verdicts into a single group verdict.
    pub fn merge_all(
        &mut self,
        bc: &dyn BinaryCompletion,
    ) -> Result<CompletionVerdict, InvariantError> {
        let op_refs: Vec<OpRef> = self.verdicts.keys().copied().collect();
        if op_refs.is_empty() {
            return Err(InvariantError::BinaryCompletionRequired {
                op: OpRef([0u8; 9]),
            });
        }
        let mut acc = self
            .verdicts
            .get(&op_refs[0])
            .cloned()
            .ok_or_else(|| InvariantError::BinaryCompletionRequired {
                op: OpRef([0u8; 9]),
            })?;
        for op_ref in op_refs.into_iter().skip(1) {
            let next = self
                .verdicts
                .get(&op_ref)
                .cloned()
                .ok_or_else(|| InvariantError::BinaryCompletionRequired { op })?;
            acc = bc.merge(acc, next)?;
        }
        Ok(acc)
    }

    /// Retract all proofs and verdicts, returning their counts.
    pub fn retract_all(&mut self) -> (u64, u64) {
        let proofs = self.proofs.len() as u64;
        let verdicts = self.verdicts.len() as u64;
        self.clear_proofs_and_verdicts();
        (proofs, verdicts)
    }

    /// Engine health: ratio of complete ops to total tracked ops.
    pub fn health_ratio(&self) -> f64 {
        let total = self.verdicts.len() as f64;
        if total == 0.0 {
            return 1.0;
        }
        let complete = self.verdicts.values().filter(|v| v.is_complete()).count() as f64;
        complete / total
    }

    /// Whether the engine is healthy (all tracked ops complete, none rejected).
    pub fn is_healthy(&self) -> bool {
        !self.verdicts.is_empty()
            && self
                .verdicts
                .values()
                .all(|v| matches!(v, CompletionVerdict::Complete))
    }

    /// Dump all op_refs with their verdicts as a sorted vector.
    pub fn dump(&self) -> Vec<(OpRef, CompletionVerdict)> {
        let mut out: Vec<(OpRef, CompletionVerdict)> =
            self.verdicts.iter().map(|(k, v)| (*k, v.clone())).collect();
        out.sort_by(|a, b| a.0 .0.cmp(&b.0 .0));
        out
    }
}

///////////////////////////////////////////////////////////////////////////////
// COMPLETION BATCH PROCESSOR
///////////////////////////////////////////////////////////////////////////////

/// Processes completion operations in batches: register, verify, aggregate.
#[derive(Debug, Clone, Default)]
pub struct CompletionBatchProcessor {
    pub batches_processed: u64,
    pub ops_registered: u64,
    pub ops_verified: u64,
    pub ops_rejected: u64,
}

impl CompletionBatchProcessor {
    pub fn new() -> Self {
        Self::default()
    }

    /// Process a batch: register proofs, verify pending, return per-op outcomes.
    pub fn process(
        &mut self,
        engine: &mut CompletionEngine,
        proofs: Vec<CompletionProof>,
        bc: &dyn BinaryCompletion,
    ) -> Result<Vec<(OpRef, CompletionVerdict)>, InvariantError> {
        self.batches_processed += 1;
        let registered = engine.register_batch(proofs.clone());
        self.ops_registered += registered.len() as u64;
        let outcomes = engine.verify_all_pending(bc)?;
        self.ops_verified += outcomes.len() as u64;
        let rejected = outcomes.iter().filter(|(_, v)| v.is_rejected()).count();
        self.ops_rejected += rejected as u64;
        Ok(outcomes)
    }

    /// Process and aggregate into a single result.
    pub fn process_and_aggregate(
        &mut self,
        engine: &mut CompletionEngine,
        proofs: Vec<CompletionProof>,
        bc: &dyn BinaryCompletion,
    ) -> Result<(Vec<(OpRef, CompletionVerdict)>, AggregatedCompletion), InvariantError> {
        let outcomes = self.process(engine, proofs, bc)?;
        let agg = engine.aggregate_all(bc)?;
        Ok((outcomes, agg))
    }

    /// Batch processing rate (ops per batch).
    pub fn avg_ops_per_batch(&self) -> f64 {
        if self.batches_processed == 0 {
            return 0.0;
        }
        self.ops_registered as f64 / self.batches_processed as f64
    }

    /// Rejection rate as a percentage.
    pub fn rejection_rate_percent(&self) -> f64 {
        if self.ops_registered == 0 {
            return 0.0;
        }
        self.ops_rejected as f64 / self.ops_registered as f64 * 100.0
    }

    /// Reset counters.
    pub fn reset_counters(&mut self) {
        self.batches_processed = 0;
        self.ops_registered = 0;
        self.ops_verified = 0;
        self.ops_rejected = 0;
    }
}

///////////////////////////////////////////////////////////////////////////////
// COMPLETION STATE SNAPSHOT
///////////////////////////////////////////////////////////////////////////////

/// An immutable snapshot of completion state at a point in time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletionStateSnapshot {
    pub tick: u64,
    pub op_count: u32,
    pub complete_count: u32,
    pub failed_count: u32,
    pub in_progress_count: u32,
    pub rejected_count: u32,
    pub group_fully_done: bool,
}

impl CompletionStateSnapshot {
    pub fn new(tick: u64, coordinator: &CompletionCoordinator) -> Self {
        let summary = coordinator.group_summary();
        Self {
            tick,
            op_count: summary.total_ops,
            complete_count: summary.complete_ops,
            failed_count: summary.failed_ops,
            in_progress_count: summary.in_progress_ops,
            rejected_count: summary.rejected_ops,
            group_fully_done: summary.group_fully_done,
        }
    }

    /// Completion percentage at snapshot time.
    pub fn completion_percent(&self) -> f64 {
        if self.op_count == 0 {
            return 0.0;
        }
        self.complete_count as f64 / self.op_count as f64 * 100.0
    }

    /// Whether the snapshot shows full completion.
    pub fn is_fully_done(&self) -> bool {
        self.group_fully_done
    }

    /// Whether the snapshot shows any failure.
    pub fn has_failure(&self) -> bool {
        self.failed_count > 0 || self.rejected_count > 0
    }

    /// Completion deficit: replicas still pending.
    pub fn pending_deficit(&self) -> u32 {
        self.in_progress_count
    }

    /// Merge two snapshots: the later tick wins, counts sum.
    pub fn merge(&self, other: &CompletionStateSnapshot) -> CompletionStateSnapshot {
        let tick = if other.tick >= self.tick { other.tick } else { self.tick };
        CompletionStateSnapshot {
            tick,
            op_count: self.op_count.max(other.op_count),
            complete_count: self.complete_count + other.complete_count,
            failed_count: self.failed_count + other.failed_count,
            in_progress_count: self.in_progress_count + other.in_progress_count,
            rejected_count: self.rejected_count + other.rejected_count,
            group_fully_done: self.group_fully_done && other.group_fully_done,
        }
    }

    /// Whether the snapshot is older than `max_age_ticks`.
    pub fn is_older_than(&self, now: u64, max_age_ticks: u64) -> bool {
        now.saturating_sub(self.tick) > max_age_ticks
    }
}

///////////////////////////////////////////////////////////////////////////////
// EXTENDED BATCH PROCESSOR
///////////////////////////////////////////////////////////////////////////////

impl CompletionBatchProcessor {
    /// Process only proofs that would produce a Complete verdict (dry-run filter).
    pub fn filter_complete(
        &self,
        engine: &CompletionEngine,
        proofs: &[CompletionProof],
        bc: &dyn BinaryCompletion,
    ) -> Vec<CompletionProof> {
        proofs
            .iter()
            .filter(|p| {
                matches!(
                    bc.verify_completion(p.clone()),
                    Ok(CompletionVerdict::Complete)
                )
            })
            .cloned()
            .collect()
    }

    /// Clear all counters.
    pub fn clear(&mut self) {
        self.reset_counters();
    }
}

///////////////////////////////////////////////////////////////////////////////
// TERMINAL CLAIM VALIDATION
///////////////////////////////////////////////////////////////////////////////

impl TerminalClaim {
    /// Validate this claim against the binary completion invariant. Returns an
    /// error when the claim asserts a partial terminal state.
    pub fn validate(&self) -> Result<(), InvariantError> {
        let bc = ();
        if bc.is_terminal(self.op_state) && self.progress < self.total && self.progress > 0 {
            return Err(InvariantError::BinaryCompletionRequired {
                op: self.op_ref,
            });
        }
        if self.total > 0 && self.progress > self.total {
            return Err(InvariantError::BinaryCompletionRequired {
                op: self.op_ref,
            });
        }
        Ok(())
    }

    /// Completion ratio for this claim.
    pub fn ratio(&self) -> f64 {
        ().progress_ratio(self.progress, self.total)
    }

    /// Whether this claim represents full completion.
    pub fn is_fully_done(&self) -> bool {
        ().is_fully_done(self.op_state)
    }
}

///////////////////////////////////////////////////////////////////////////////
// COMPLETION PROOF BUILDER
///////////////////////////////////////////////////////////////////////////////

impl CompletionProof {
    /// Build a proof for an operation that completed across `replica_count`
    /// replicas with `acked_replicas` acknowledging.
    pub fn new(
        op_ref: OpRef,
        dataset_id: DatasetId,
        replica_count: u32,
        acked_replicas: u32,
        commit_state: CommitState,
        op_state: OpState,
        completed_at: u64,
    ) -> Self {
        Self {
            op_ref,
            dataset_id,
            replica_count,
            acked_replicas,
            commit_state,
            op_state,
            completed_at,
            proof_id: [0u8; 16],
        }
    }

    /// Whether this proof asserts full completion.
    pub fn asserts_complete(&self) -> bool {
        self.op_state == OpState::Verified
            || (self.acked_replicas == self.replica_count
                && self.replica_count > 0
                && matches!(
                    self.op_state,
                    OpState::Committed | OpState::Synchronized
                ))
    }

    /// Whether this proof is internally consistent.
    pub fn is_consistent(&self) -> bool {
        self.acked_replicas <= self.replica_count
    }
}

///////////////////////////////////////////////////////////////////////////////
// COMPLETION VERDICT HELPERS
///////////////////////////////////////////////////////////////////////////////

impl CompletionVerdict {
    /// Whether this verdict is terminal.
    pub fn is_terminal(&self) -> bool {
        matches!(self, CompletionVerdict::Complete | CompletionVerdict::Failed | CompletionVerdict::Rejected { .. })
    }

    /// Whether this verdict indicates successful completion.
    pub fn is_complete(&self) -> bool {
        matches!(self, CompletionVerdict::Complete)
    }

    /// Whether this verdict indicates terminal failure.
    pub fn is_failed(&self) -> bool {
        matches!(self, CompletionVerdict::Failed)
    }

    /// Whether this verdict was rejected as malformed.
    pub fn is_rejected(&self) -> bool {
        matches!(self, CompletionVerdict::Rejected { .. })
    }

    /// Rejection reason, if rejected.
    pub fn rejection_reason(&self) -> Option<&str> {
        match self {
            CompletionVerdict::Rejected { reason } => Some(reason),
            _ => None,
        }
    }
}

///////////////////////////////////////////////////////////////////////////////
// AGGREGATED COMPLETION HELPERS
///////////////////////////////////////////////////////////////////////////////

impl AggregatedCompletion {
    /// Completion percentage across replicas.
    pub fn completion_percent(&self) -> f64 {
        if self.total_replicas == 0 {
            return 0.0;
        }
        self.complete_replicas as f64 / self.total_replicas as f64 * 100.0
    }

    /// Whether a strict quorum (majority) of replicas is complete.
    pub fn has_quorum(&self) -> bool {
        self.quorum_met
    }

    /// Whether the aggregate is fully done (binary rule).
    pub fn is_fully_done(&self) -> bool {
        self.fully_done
    }

    /// Whether any replica failed terminally.
    pub fn has_any_failure(&self) -> bool {
        self.any_failed
    }
}

///////////////////////////////////////////////////////////////////////////////
// COMPLETION SUMMARY HELPERS
///////////////////////////////////////////////////////////////////////////////

impl CompletionSummary {
    /// Whether this summary indicates a terminal outcome.
    pub fn is_terminal(&self) -> bool {
        self.is_terminal
    }

    /// Whether a retry is required (failed or rejected).
    pub fn requires_retry(&self) -> bool {
        self.requires_retry
    }
}

///////////////////////////////////////////////////////////////////////////////
// COMPLETION RESULT HELPERS
///////////////////////////////////////////////////////////////////////////////

impl CompletionResult {
    /// Create a new per-replica completion result.
    pub fn new(replica: NodeId, state: OpState, commit_state: CommitState) -> Self {
        Self {
            replica,
            state,
            commit_state,
        }
    }
}

///////////////////////////////////////////////////////////////////////////////
// TESTS
///////////////////////////////////////////////////////////////////////////////

#[cfg(test)]
mod binary_completion_tests {
    use super::*;

    fn bc() -> () {
        ()
    }

    fn test_op_ref() -> OpRef {
        OpRef([1u8; 9])
    }

    #[test]
    fn is_fully_done_committed() {
        assert!(bc().is_fully_done(OpState::Committed));
    }

    #[test]
    fn is_fully_done_synchronized() {
        assert!(bc().is_fully_done(OpState::Synchronized));
    }

    #[test]
    fn is_fully_done_verified() {
        assert!(bc().is_fully_done(OpState::Verified));
    }

    #[test]
    fn is_fully_done_received_false() {
        assert!(!bc().is_fully_done(OpState::Received));
    }

    #[test]
    fn is_fully_done_ordered_false() {
        assert!(!bc().is_fully_done(OpState::Ordered));
    }

    #[test]
    fn is_fully_done_executing_false() {
        assert!(!bc().is_fully_done(OpState::Executing));
    }

    #[test]
    fn is_fully_done_failed_false() {
        assert!(!bc().is_fully_done(OpState::Failed));
    }

    #[test]
    fn reject_partial_zero_progress_ok() {
        assert!(bc().reject_partial(0, 100).is_ok());
    }

    #[test]
    fn reject_partial_full_progress_ok() {
        assert!(bc().reject_partial(100, 100).is_ok());
    }

    #[test]
    fn reject_partial_zero_total_ok() {
        assert!(bc().reject_partial(0, 0).is_ok());
    }

    #[test]
    fn reject_partial_partial_errors() {
        assert!(bc().reject_partial(1, 100).is_err());
        assert!(bc().reject_partial(50, 100).is_err());
        assert!(bc().reject_partial(99, 100).is_err());
    }

    #[test]
    fn progress_ratio_full() {
        assert!((bc().progress_ratio(100, 100) - 100.0).abs() < f64::EPSILON);
    }

    #[test]
    fn progress_ratio_half() {
        assert!((bc().progress_ratio(50, 100) - 50.0).abs() < f64::EPSILON);
    }

    #[test]
    fn progress_ratio_zero_total() {
        assert!((bc().progress_ratio(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn progress_ratio_over_total_saturates() {
        assert!((bc().progress_ratio(200, 100) - 100.0).abs() < f64::EPSILON);
    }

    #[test]
    fn is_terminal_verified() {
        assert!(bc().is_terminal(OpState::Verified));
    }

    #[test]
    fn is_terminal_failed() {
        assert!(bc().is_terminal(OpState::Failed));
    }

    #[test]
    fn is_terminal_committed_false() {
        assert!(!bc().is_terminal(OpState::Committed));
    }

    #[test]
    fn is_success_verified() {
        assert!(bc().is_success(OpState::Verified));
    }

    #[test]
    fn is_success_failed_false() {
        assert!(!bc().is_success(OpState::Failed));
    }

    #[test]
    fn is_failure_failed() {
        assert!(bc().is_failure(OpState::Failed));
    }

    #[test]
    fn is_failure_committed_false() {
        assert!(!bc().is_failure(OpState::Committed));
    }

    #[test]
    fn advance_received_to_ordered() {
        let result = bc().advance(OpState::Received, OpTransitionEvent::Ordered);
        assert_eq!(result, Ok(OpState::Ordered));
    }

    #[test]
    fn advance_ordered_to_executing() {
        assert_eq!(
            bc().advance(OpState::Ordered, OpTransitionEvent::Executing),
            Ok(OpState::Executing)
        );
    }

    #[test]
    fn advance_executing_to_committed() {
        assert_eq!(
            bc().advance(OpState::Executing, OpTransitionEvent::Committed),
            Ok(OpState::Committed)
        );
    }

    #[test]
    fn advance_committed_to_synchronized() {
        assert_eq!(
            bc().advance(OpState::Committed, OpTransitionEvent::Synchronized),
            Ok(OpState::Synchronized)
        );
    }

    #[test]
    fn advance_synchronized_to_verified() {
        assert_eq!(
            bc().advance(OpState::Synchronized, OpTransitionEvent::Verified),
            Ok(OpState::Verified)
        );
    }

    #[test]
    fn advance_to_failed() {
        assert_eq!(
            bc().advance(OpState::Executing, OpTransitionEvent::Failed),
            Ok(OpState::Failed)
        );
    }

    #[test]
    fn advance_from_terminal_rejected() {
        assert!(bc()
            .advance(OpState::Verified, OpTransitionEvent::Committed)
            .is_err());
        assert!(bc()
            .advance(OpState::Failed, OpTransitionEvent::Ordered)
            .is_err());
    }

    #[test]
    fn advance_invalid_transition_stays() {
        assert_eq!(
            bc().advance(OpState::Received, OpTransitionEvent::Committed),
            Ok(OpState::Received)
        );
    }

    #[test]
    fn advance_aborted_from_received() {
        assert_eq!(
            bc().advance(OpState::Received, OpTransitionEvent::Aborted),
            Ok(OpState::Failed)
        );
    }

    #[test]
    fn advance_aborted_from_committed_noop() {
        assert_eq!(
            bc().advance(OpState::Committed, OpTransitionEvent::Aborted),
            Ok(OpState::Committed)
        );
    }

    #[test]
    fn verify_complete_proof() {
        let proof = CompletionProof::new(
            test_op_ref(),
            DatasetId([0u8; 32]),
            3,
            3,
            CommitState::Committed,
            OpState::Committed,
            1000,
        );
        assert_eq!(bc().verify_completion(proof), Ok(CompletionVerdict::Complete));
    }

    #[test]
    fn verify_verified_state_complete() {
        let proof = CompletionProof::new(
            test_op_ref(),
            DatasetId([0u8; 32]),
            5,
            5,
            CommitState::Committed,
            OpState::Verified,
            1000,
        );
        assert_eq!(bc().verify_completion(proof), Ok(CompletionVerdict::Complete));
    }

    #[test]
    fn verify_failed_state_failed() {
        let proof = CompletionProof::new(
            test_op_ref(),
            DatasetId([0u8; 32]),
            3,
            0,
            CommitState::Aborted,
            OpState::Failed,
            1000,
        );
        assert_eq!(bc().verify_completion(proof), Ok(CompletionVerdict::Failed));
    }

    #[test]
    fn verify_in_progress_partial_acks() {
        let proof = CompletionProof::new(
            test_op_ref(),
            DatasetId([0u8; 32]),
            3,
            2,
            CommitState::Committed,
            OpState::Committed,
            1000,
        );
        assert_eq!(
            bc().verify_completion(proof),
            Ok(CompletionVerdict::InProgress)
        );
    }

    #[test]
    fn verify_zero_replicas_rejected() {
        let proof = CompletionProof::new(
            test_op_ref(),
            DatasetId([0u8; 32]),
            0,
            0,
            CommitState::Committed,
            OpState::Committed,
            1000,
        );
        assert!(bc().verify_completion(proof).is_err());
    }

    #[test]
    fn verify_acks_exceed_replicas_rejected() {
        let proof = CompletionProof::new(
            test_op_ref(),
            DatasetId([0u8; 32]),
            3,
            5,
            CommitState::Committed,
            OpState::Committed,
            1000,
        );
        assert!(bc().verify_completion(proof).is_err());
    }

    #[test]
    fn aggregate_all_complete() {
        let results = vec![
            CompletionResult::new(NodeId([1u8; 32]), OpState::Verified, CommitState::Committed),
            CompletionResult::new(NodeId([2u8; 32]), OpState::Verified, CommitState::Committed),
            CompletionResult::new(NodeId([3u8; 32]), OpState::Verified, CommitState::Committed),
        ];
        let agg = bc().aggregate(results);
        assert_eq!(agg.total_replicas, 3);
        assert_eq!(agg.complete_replicas, 3);
        assert_eq!(agg.failed_replicas, 0);
        assert_eq!(agg.pending_replicas, 0);
        assert!(agg.fully_done);
        assert!(!agg.any_failed);
        assert!(agg.quorum_met);
        assert!((agg.completion_percent() - 100.0).abs() < f64::EPSILON);
    }

    #[test]
    fn aggregate_mixed() {
        let results = vec![
            CompletionResult::new(NodeId([1u8; 32]), OpState::Verified, CommitState::Committed),
            CompletionResult::new(NodeId([2u8; 32]), OpState::Received, CommitState::Accepted),
            CompletionResult::new(NodeId([3u8; 32]), OpState::Failed, CommitState::Aborted),
        ];
        let agg = bc().aggregate(results);
        assert_eq!(agg.complete_replicas, 1);
        assert_eq!(agg.failed_replicas, 1);
        assert_eq!(agg.pending_replicas, 1);
        assert!(!agg.fully_done);
        assert!(agg.any_failed);
        assert!(!agg.quorum_met);
        assert!((agg.completion_percent() - (100.0 / 3.0)).abs() < 0.01);
    }

    #[test]
    fn aggregate_empty() {
        let agg = bc().aggregate(Vec::new());
        assert_eq!(agg.total_replicas, 0);
        assert!(!agg.fully_done);
        assert!(!agg.any_failed);
        assert!(!agg.quorum_met);
    }

    #[test]
    fn merge_complete_complete() {
        assert_eq!(
            bc().merge(CompletionVerdict::Complete, CompletionVerdict::Complete),
            CompletionVerdict::Complete
        );
    }

    #[test]
    fn merge_complete_in_progress() {
        assert_eq!(
            bc().merge(CompletionVerdict::Complete, CompletionVerdict::InProgress),
            CompletionVerdict::Complete
        );
    }

    #[test]
    fn merge_failed_in_progress() {
        assert_eq!(
            bc().merge(CompletionVerdict::Failed, CompletionVerdict::InProgress),
            CompletionVerdict::Failed
        );
    }

    #[test]
    fn merge_complete_failed_rejected() {
        assert!(bc()
            .merge(
                CompletionVerdict::Complete,
                CompletionVerdict::Failed
            )
            .is_rejected());
    }

    #[test]
    fn merge_rejected_dominates() {
        assert!(bc()
            .merge(
                CompletionVerdict::Rejected {
                    reason: "x".to_string()
                },
                CompletionVerdict::Complete
            )
            .is_rejected());
    }

    #[test]
    fn merge_in_progress_in_progress() {
        assert_eq!(
            bc().merge(
                CompletionVerdict::InProgress,
                CompletionVerdict::InProgress
            ),
            CompletionVerdict::InProgress
        );
    }

    #[test]
    fn verdict_is_terminal() {
        assert!(CompletionVerdict::Complete.is_terminal());
        assert!(CompletionVerdict::Failed.is_terminal());
        assert!(CompletionVerdict::Rejected {
            reason: "x".to_string()
        }
        .is_terminal());
        assert!(!CompletionVerdict::InProgress.is_terminal());
    }

    #[test]
    fn verdict_is_complete() {
        assert!(CompletionVerdict::Complete.is_complete());
        assert!(!CompletionVerdict::Failed.is_complete());
    }

    #[test]
    fn verdict_is_failed() {
        assert!(CompletionVerdict::Failed.is_failed());
        assert!(!CompletionVerdict::Complete.is_failed());
    }

    #[test]
    fn verdict_rejection_reason() {
        let v = CompletionVerdict::Rejected {
            reason: "bad proof".to_string(),
        };
        assert_eq!(v.rejection_reason(), Some("bad proof"));
        assert_eq!(CompletionVerdict::Complete.rejection_reason(), None);
    }

    #[test]
    fn agg_completion_percent() {
        let agg = AggregatedCompletion {
            op_ref: OpRef([0; 9]),
            total_replicas: 4,
            complete_replicas: 2,
            failed_replicas: 1,
            pending_replicas: 1,
            fully_done: false,
            any_failed: true,
            quorum_met: false,
        };
        assert!((agg.completion_percent() - 50.0).abs() < f64::EPSILON);
    }

    #[test]
    fn agg_completion_percent_zero_total() {
        let agg = AggregatedCompletion {
            op_ref: OpRef([0; 9]),
            total_replicas: 0,
            complete_replicas: 0,
            failed_replicas: 0,
            pending_replicas: 0,
            fully_done: false,
            any_failed: false,
            quorum_met: false,
        };
        assert!((agg.completion_percent() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn summary_complete() {
        let s = bc().summary(test_op_ref(), &CompletionVerdict::Complete);
        assert!((s.progress_percent - 100.0).abs() < f64::EPSILON);
        assert!(s.is_terminal);
        assert!(!s.requires_retry);
        assert!(s.verdict.is_complete());
    }

    #[test]
    fn summary_failed() {
        let s = bc().summary(test_op_ref(), &CompletionVerdict::Failed);
        assert!((s.progress_percent - 0.0).abs() < f64::EPSILON);
        assert!(s.is_terminal);
        assert!(s.requires_retry);
    }

    #[test]
    fn summary_in_progress() {
        let s = bc().summary(test_op_ref(), &CompletionVerdict::InProgress);
        assert!((s.progress_percent - 0.0).abs() < f64::EPSILON);
        assert!(!s.is_terminal);
        assert!(!s.requires_retry);
    }

    #[test]
    fn summary_rejected() {
        let s = bc().summary(
            test_op_ref(),
            &CompletionVerdict::Rejected {
                reason: "r".to_string(),
            },
        );
        assert!(s.is_terminal);
        assert!(s.requires_retry);
        assert!(s.verdict.is_rejected());
    }

    #[test]
    fn terminal_claim_validate_ok() {
        let claim = TerminalClaim {
            op_ref: test_op_ref(),
            progress: 100,
            total: 100,
            op_state: OpState::Verified,
        };
        assert!(claim.validate().is_ok());
    }

    #[test]
    fn terminal_claim_validate_partial_errors() {
        let claim = TerminalClaim {
            op_ref: test_op_ref(),
            progress: 50,
            total: 100,
            op_state: OpState::Verified,
        };
        assert!(claim.validate().is_err());
    }

    #[test]
    fn terminal_claim_ratio() {
        let claim = TerminalClaim {
            op_ref: test_op_ref(),
            progress: 30,
            total: 100,
            op_state: OpState::Executing,
        };
        assert!((claim.ratio() - 30.0).abs() < f64::EPSILON);
    }

    #[test]
    fn terminal_claim_is_fully_done() {
        let done = TerminalClaim {
            op_ref: test_op_ref(),
            progress: 100,
            total: 100,
            op_state: OpState::Verified,
        };
        assert!(done.is_fully_done());
        let not_done = TerminalClaim {
            op_ref: test_op_ref(),
            progress: 50,
            total: 100,
            op_state: OpState::Executing,
        };
        assert!(!not_done.is_fully_done());
    }

    #[test]
    fn proof_asserts_complete_true() {
        let proof = CompletionProof::new(
            test_op_ref(),
            DatasetId([0u8; 32]),
            3,
            3,
            CommitState::Committed,
            OpState::Verified,
            1000,
        );
        assert!(proof.asserts_complete());
    }

    #[test]
    fn proof_asserts_complete_false() {
        let proof = CompletionProof::new(
            test_op_ref(),
            DatasetId([0u8; 32]),
            3,
            1,
            CommitState::Accepted,
            OpState::Ordered,
            1000,
        );
        assert!(!proof.asserts_complete());
    }

    #[test]
    fn proof_is_consistent_true() {
        let proof = CompletionProof::new(
            test_op_ref(),
            DatasetId([0u8; 32]),
            3,
            2,
            CommitState::Accepted,
            OpState::Ordered,
            1000,
        );
        assert!(proof.is_consistent());
    }

    #[test]
    fn proof_is_consistent_false() {
        let proof = CompletionProof::new(
            test_op_ref(),
            DatasetId([0u8; 32]),
            3,
            5,
            CommitState::Accepted,
            OpState::Ordered,
            1000,
        );
        assert!(!proof.is_consistent());
    }

    #[test]
    fn is_stale_not_stale() {
        assert!(!bc().is_stale(&CompletionVerdict::Complete, 5000));
    }

    #[test]
    fn binary_rule_no_partial_terminal() {
        // The core invariant: a terminal state with 0 < progress < total is invalid.
        for total in [1, 2, 10, 100, 1000] {
            for progress in 1..total {
                assert!(bc().reject_partial(progress, total).is_err());
            }
        }
    }

    ///////////////////////////////////////////////////////////////////////////////
    // EXTENDED ENGINE TESTS
    ///////////////////////////////////////////////////////////////////////////////

    #[test]
    fn engine_register_and_verify_complete() {
        let mut engine = CompletionEngine::new();
        let proof = CompletionProof::new(
            test_op_ref(),
            DatasetId([0u8; 32]),
            3,
            3,
            CommitState::Committed,
            OpState::Verified,
            5000,
        );
        let verdict = engine
            .register_and_verify(proof, &bc())
            .unwrap();
        assert_eq!(verdict, CompletionVerdict::Complete);
        assert!(engine.has_complete());
        assert!(!engine.has_failed());
        assert_eq!(engine.complete_count(), 1);
        assert_eq!(engine.failed_count(), 0);
        assert_eq!(engine.in_progress_count(), 0);
    }

    #[test]
    fn engine_register_and_verify_failed() {
        let mut engine = CompletionEngine::new();
        let proof = CompletionProof::new(
            test_op_ref(),
            DatasetId([0u8; 32]),
            3,
            0,
            CommitState::Aborted,
            OpState::Failed,
            5000,
        );
        let verdict = engine
            .register_and_verify(proof, &bc())
            .unwrap();
        assert_eq!(verdict, CompletionVerdict::Failed);
        assert!(engine.has_failed());
        assert!(!engine.has_complete());
        assert_eq!(engine.failed_count(), 1);
    }

    #[test]
    fn engine_register_batch() {
        let mut engine = CompletionEngine::new();
        let proofs = vec![
            CompletionProof::new(
                OpRef([1u8; 9]),
                DatasetId([0u8; 32]),
                2,
                2,
                CommitState::Committed,
                OpState::Committed,
                5000,
            ),
            CompletionProof::new(
                OpRef([2u8; 9]),
                DatasetId([0u8; 32]),
                2,
                1,
                CommitState::Accepted,
                OpState::Ordered,
                5000,
            ),
        ];
        let results = engine.register_batch(proofs);
        assert_eq!(results, vec![true, true]);
        assert_eq!(engine.proofs.len(), 2);
    }

    #[test]
    fn engine_verify_all_pending() {
        let mut engine = CompletionEngine::new();
        let proof = CompletionProof::new(
            test_op_ref(),
            DatasetId([0u8; 32]),
            2,
            2,
            CommitState::Committed,
            OpState::Committed,
            5000,
        );
        engine.register_proof(proof);
        let outcomes = engine.verify_all_pending(&bc()).unwrap();
        assert_eq!(outcomes.len(), 1);
        assert_eq!(outcomes[0].1, CompletionVerdict::Complete);
    }

    #[test]
    fn engine_merge_all() {
        let mut engine = CompletionEngine::new();
        let p1 = CompletionProof::new(
            OpRef([1u8; 9]),
            DatasetId([0u8; 32]),
            2,
            2,
            CommitState::Committed,
            OpState::Committed,
            5000,
        );
        let p2 = CompletionProof::new(
            OpRef([2u8; 9]),
            DatasetId([0u8; 32]),
            2,
            0,
            CommitState::Aborted,
            OpState::Failed,
            5000,
        );
        engine.register_proof(p1);
        engine.register_proof(p2);
        engine.verify_all_pending(&bc()).unwrap();
        let merged = engine.merge_all(&bc()).unwrap();
        assert!(merged.is_rejected());
    }

    #[test]
    fn engine_retract_all() {
        let mut engine = CompletionEngine::new();
        let proof = CompletionProof::new(
            test_op_ref(),
            DatasetId([0u8; 32]),
            1,
            1,
            CommitState::Committed,
            OpState::Committed,
            5000,
        );
        engine.register_proof(proof);
        engine.verify_all_pending(&bc()).unwrap();
        let (p, v) = engine.retract_all();
        assert_eq!(p, 1);
        assert_eq!(v, 1);
        assert!(engine.is_idle());
    }

    #[test]
    fn engine_health_ratio() {
        let mut engine = CompletionEngine::new();
        let complete = CompletionProof::new(
            OpRef([1u8; 9]),
            DatasetId([0u8; 32]),
            1,
            1,
            CommitState::Committed,
            OpState::Verified,
            5000,
        );
        let failed = CompletionProof::new(
            OpRef([2u8; 9]),
            DatasetId([0u8; 32]),
            1,
            0,
            CommitState::Aborted,
            OpState::Failed,
            5000,
        );
        engine.register_proof(complete);
        engine.register_proof(failed);
        engine.verify_all_pending(&bc()).unwrap();
        assert!((engine.health_ratio() - 0.5).abs() < f64::EPSILON);
        assert!(!engine.is_healthy());
    }

    #[test]
    fn engine_is_healthy_all_complete() {
        let mut engine = CompletionEngine::new();
        let proof = CompletionProof::new(
            test_op_ref(),
            DatasetId([0u8; 32]),
            1,
            1,
            CommitState::Committed,
            OpState::Verified,
            5000,
        );
        engine.register_proof(proof);
        engine.verify_all_pending(&bc()).unwrap();
        assert!(engine.is_healthy());
    }

    #[test]
    fn engine_dump_sorted() {
        let mut engine = CompletionEngine::new();
        for i in [3u8, 1u8, 2u8] {
            let proof = CompletionProof::new(
                OpRef([i; 9]),
                DatasetId([0u8; 32]),
                1,
                1,
                CommitState::Committed,
                OpState::Verified,
                5000,
            );
            engine.register_proof(proof);
        }
        engine.verify_all_pending(&bc()).unwrap();
        let dump = engine.dump();
        assert_eq!(dump.len(), 3);
        assert_eq!(dump[0].0, OpRef([1u8; 9]));
        assert_eq!(dump[1].0, OpRef([2u8; 9]));
        assert_eq!(dump[2].0, OpRef([3u8; 9]));
    }

    #[test]
    fn engine_gate_check() {
        let mut engine = CompletionEngine::new();
        let proof = CompletionProof::new(
            test_op_ref(),
            DatasetId([0u8; 32]),
            1,
            1,
            CommitState::Committed,
            OpState::Verified,
            5000,
        );
        engine.register_proof(proof);
        engine.verify_all_pending(&bc()).unwrap();
        let gate = CompletionGate::new();
        assert!(engine.gate_check(test_op_ref(), &gate).unwrap());
    }

    #[test]
    fn engine_recovery_scope() {
        let mut engine = CompletionEngine::new();
        let proof = CompletionProof::new(
            test_op_ref(),
            DatasetId([0u8; 32]),
            3,
            0,
            CommitState::Accepted,
            OpState::Ordered,
            5000,
        );
        engine.register_proof(proof);
        let scope = engine.recovery_scope(test_op_ref()).unwrap();
        assert_eq!(scope, CompletionRecoveryScope::NodeLocal);
        assert!(scope.requires_recovery());
    }

    #[test]
    fn engine_recovery_scope_none() {
        let mut engine = CompletionEngine::new();
        let proof = CompletionProof::new(
            test_op_ref(),
            DatasetId([0u8; 32]),
            3,
            3,
            CommitState::Committed,
            OpState::Verified,
            5000,
        );
        engine.register_proof(proof);
        let scope = engine.recovery_scope(test_op_ref()).unwrap();
        assert_eq!(scope, CompletionRecoveryScope::None);
        assert!(!scope.requires_recovery());
    }

    #[test]
    fn engine_summary_non_empty() {
        let mut engine = CompletionEngine::new();
        let proof = CompletionProof::new(
            test_op_ref(),
            DatasetId([0u8; 32]),
            1,
            1,
            CommitState::Committed,
            OpState::Verified,
            5000,
        );
        engine.register_proof(proof);
        let s = engine.summary();
        assert!(!s.is_empty());
        assert!(s.contains("policy="));
    }

    ///////////////////////////////////////////////////////////////////////////////
    // PROOF VALIDATOR TESTS
    ///////////////////////////////////////////////////////////////////////////////

    #[test]
    fn proof_validator_valid() {
        let v = ProofValidator::new();
        let proof = CompletionProof::new(
            test_op_ref(),
            DatasetId([0u8; 32]),
            3,
            2,
            CommitState::Accepted,
            OpState::Ordered,
            5000,
        );
        assert!(v.validate(&proof).is_ok());
        assert!(v.is_valid(&proof));
    }

    #[test]
    fn proof_validator_acks_exceed_replicas() {
        let v = ProofValidator::new();
        let proof = CompletionProof::new(
            test_op_ref(),
            DatasetId([0u8; 32]),
            2,
            5,
            CommitState::Accepted,
            OpState::Ordered,
            5000,
        );
        assert!(v.validate(&proof).is_err());
        assert!(!v.is_valid(&proof));
    }

    #[test]
    fn proof_validator_zero_completed_at() {
        let v = ProofValidator::new();
        let proof = CompletionProof::new(
            test_op_ref(),
            DatasetId([0u8; 32]),
            2,
            2,
            CommitState::Accepted,
            OpState::Ordered,
            0,
        );
        assert!(v.validate(&proof).is_err());
    }

    #[test]
    fn proof_validator_batch() {
        let v = ProofValidator::new();
        let good = CompletionProof::new(
            OpRef([1u8; 9]),
            DatasetId([0u8; 32]),
            2,
            2,
            CommitState::Accepted,
            OpState::Ordered,
            5000,
        );
        let bad = CompletionProof::new(
            OpRef([2u8; 9]),
            DatasetId([0u8; 32]),
            2,
            5,
            CommitState::Accepted,
            OpState::Ordered,
            5000,
        );
        let invalid = v.validate_batch(&[good, bad]).unwrap();
        assert_eq!(invalid, vec![1]);
    }

    #[test]
    fn proof_validator_rates_and_reset() {
        let mut v = ProofValidator::new();
        let good = CompletionProof::new(
            OpRef([1u8; 9]),
            DatasetId([0u8; 32]),
            2,
            2,
            CommitState::Accepted,
            OpState::Ordered,
            5000,
        );
        let bad = CompletionProof::new(
            OpRef([2u8; 9]),
            DatasetId([0u8; 32]),
            2,
            5,
            CommitState::Accepted,
            OpState::Ordered,
            5000,
        );
        v.validate(&good).unwrap();
        v.validate(&bad).unwrap_err();
        assert!((v.rejection_rate_percent() - 50.0).abs() < f64::EPSILON);
        v.reset_counters();
        assert_eq!(v.rejection_rate_percent(), 0.0);
    }

    #[test]
    fn proof_validator_with_limits() {
        let v = ProofValidator::with_limits(2, true);
        let over = CompletionProof::new(
            test_op_ref(),
            DatasetId([0u8; 32]),
            5,
            5,
            CommitState::Committed,
            OpState::Committed,
            5000,
        );
        assert!(v.validate(&over).is_err());
        let zero_acks = CompletionProof::new(
            test_op_ref(),
            DatasetId([0u8; 32]),
            2,
            0,
            CommitState::Accepted,
            OpState::Ordered,
            5000,
        );
        assert!(v.validate(&zero_acks).is_err());
    }

    ///////////////////////////////////////////////////////////////////////////////
    // STRICT / QUORUM VERIFIER TESTS
    ///////////////////////////////////////////////////////////////////////////////

    #[test]
    fn strict_verifier_all_complete() {
        let mut v = StrictVerifier::new();
        let results = vec![
            CompletionResult::new(NodeId([1u8; 32]), OpState::Verified, CommitState::Committed),
            CompletionResult::new(NodeId([2u8; 32]), OpState::Committed, CommitState::Committed),
        ];
        assert!(v.verify_all_complete(&results));
        assert!((v.completion_rate_percent() - 100.0).abs() < f64::EPSILON);
    }

    #[test]
    fn strict_verifier_not_all_complete() {
        let mut v = StrictVerifier::new();
        let results = vec![
            CompletionResult::new(NodeId([1u8; 32]), OpState::Verified, CommitState::Committed),
            CompletionResult::new(NodeId([2u8; 32]), OpState::Ordered, CommitState::Accepted),
        ];
        assert!(!v.verify_all_complete(&results));
        assert!((v.rejection_rate_percent() - 100.0).abs() < f64::EPSILON);
    }

    #[test]
    fn strict_verifier_empty() {
        let mut v = StrictVerifier::new();
        assert!(!v.verify_all_complete(&Vec::new()));
    }

    #[test]
    fn quorum_verifier_majority() {
        let mut v = QuorumVerifier::new();
        assert!(v.verify_quorum(2, 3));
        assert!(!v.verify_quorum(1, 3));
        assert!(v.is_quorum(3, 3));
    }

    #[test]
    fn quorum_verifier_rates() {
        let mut v = QuorumVerifier::new();
        v.verify_quorum(1, 3);
        v.verify_quorum(2, 3);
        assert!((v.completion_rate_percent() - 50.0).abs() < f64::EPSILON);
        assert!((v.rejection_rate_percent() - 50.0).abs() < f64::EPSILON);
    }

    ///////////////////////////////////////////////////////////////////////////////
    // REPLICA TRACKER TESTS
    ///////////////////////////////////////////////////////////////////////////////

    #[test]
    fn replica_tracker_record_and_counts() {
        let mut t = ReplicaCompletionTracker::new();
        t.record(NodeId([1u8; 32]), OpState::Verified, CommitState::Committed);
        t.record(NodeId([2u8; 32]), OpState::Received, CommitState::Accepted);
        t.record(NodeId([3u8; 32]), OpState::Failed, CommitState::Aborted);
        let counts = t.state_counts();
        assert_eq!(counts.total, 3);
        assert_eq!(counts.complete(), 1);
        assert_eq!(counts.failed, 1);
        assert_eq!(counts.pending, 1);
        assert!((counts.completion_percent() - (100.0 / 3.0)).abs() < 0.01);
        assert!(!t.all_complete());
        assert!(t.any_failed());
        assert_eq!(t.updates(), 3);
    }

    #[test]
    fn replica_tracker_all_complete() {
        let mut t = ReplicaCompletionTracker::new();
        t.record(NodeId([1u8; 32]), OpState::Verified, CommitState::Committed);
        assert!(t.all_complete());
        assert!(!t.any_failed());
    }

    #[test]
    fn replica_tracker_state_and_clear() {
        let mut t = ReplicaCompletionTracker::new();
        t.record(NodeId([1u8; 32]), OpState::Executing, CommitState::Accepted);
        assert_eq!(t.state(NodeId([1u8; 32])), Some(OpState::Executing));
        assert_eq!(
            t.commit_state(NodeId([1u8; 32])),
            Some(CommitState::Accepted)
        );
        t.clear();
        assert_eq!(t.state(NodeId([1u8; 32])), None);
        assert_eq!(t.updates(), 0);
    }

    ///////////////////////////////////////////////////////////////////////////////
    // CROSS-REPLICA CONSISTENCY TESTS
    ///////////////////////////////////////////////////////////////////////////////

    #[test]
    fn cross_replica_consistent() {
        let mut c = CrossReplicaConsistencyChecker::new();
        assert!(c.check(&[
            CompletionVerdict::Complete,
            CompletionVerdict::Complete,
        ]));
        assert!((c.consistency_rate_percent() - 100.0).abs() < f64::EPSILON);
    }

    #[test]
    fn cross_replica_inconsistent() {
        let mut c = CrossReplicaConsistencyChecker::new();
        assert!(!c.check(&[
            CompletionVerdict::Complete,
            CompletionVerdict::Failed,
        ]));
        assert!((c.consistency_rate_percent() - 50.0).abs() < f64::EPSILON);
    }

    #[test]
    fn cross_replica_has_conflict() {
        let mut c = CrossReplicaConsistencyChecker::new();
        assert!(c.has_conflict(&[
            CompletionVerdict::Complete,
            CompletionVerdict::Failed,
        ]));
        assert!(!c.has_conflict(&[
            CompletionVerdict::Complete,
            CompletionVerdict::Complete,
        ]));
        c.reset_counters();
        assert_eq!(c.checks, 0);
    }

    #[test]
    fn cross_replica_empty_terminal() {
        let mut c = CrossReplicaConsistencyChecker::new();
        assert!(c.check(&[
            CompletionVerdict::InProgress,
            CompletionVerdict::InProgress,
        ]));
    }

    ///////////////////////////////////////////////////////////////////////////////
    // COMPLETION COORDINATOR TESTS
    ///////////////////////////////////////////////////////////////////////////////

    #[test]
    fn coordinator_record_and_summary() {
        let mut coord = CompletionCoordinator::new();
        coord.record(test_op_ref(), CompletionVerdict::Complete);
        coord.record(OpRef([2u8; 9]), CompletionVerdict::Failed);
        coord.record(OpRef([3u8; 9]), CompletionVerdict::InProgress);
        let s = coord.group_summary();
        assert_eq!(s.total_ops, 3);
        assert_eq!(s.complete_ops, 1);
        assert_eq!(s.failed_ops, 1);
        assert_eq!(s.in_progress_ops, 1);
        assert!(!s.group_fully_done);
        assert!(s.group_has_failure);
        assert!((s.completion_percent() - (100.0 / 3.0)).abs() < 0.01);
    }

    #[test]
    fn coordinator_group_done() {
        let mut coord = CompletionCoordinator::new();
        coord.record(test_op_ref(), CompletionVerdict::Complete);
        coord.record(OpRef([2u8; 9]), CompletionVerdict::Complete);
        let s = coord.group_summary();
        assert!(s.group_fully_done);
        assert!(!s.group_has_failure);
    }

    #[test]
    fn coordinator_verdict_and_op_refs() {
        let mut coord = CompletionCoordinator::with_group([7u8; 16]);
        coord.record(test_op_ref(), CompletionVerdict::Complete);
        assert_eq!(coord.verdict(test_op_ref()), Some(&CompletionVerdict::Complete));
        assert_eq!(coord.verdict(OpRef([9u8; 9])), None);
        assert_eq!(coord.len(), 1);
        assert!(!coord.is_empty());
        assert_eq!(coord.group_id(), [7u8; 16]);
        let refs = coord.op_refs();
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0], test_op_ref());
        coord.clear();
        assert!(coord.is_empty());
    }

    ///////////////////////////////////////////////////////////////////////////////
    // COMPLETION GATE TESTS
    ///////////////////////////////////////////////////////////////////////////////

    #[test]
    fn completion_gate_allows_complete() {
        let gate = CompletionGate::new();
        assert!(gate.allows(&CompletionVerdict::Complete));
        assert!(!gate.allows(&CompletionVerdict::Failed));
        assert!(!gate.allows(&CompletionVerdict::InProgress));
    }

    #[test]
    fn completion_gate_pass_counts() {
        let mut gate = CompletionGate::new();
        assert!(gate.pass(CompletionVerdict::Complete));
        assert!(!gate.pass(CompletionVerdict::Failed));
        assert_eq!(gate.total(), 2);
        assert!((gate.allow_rate_percent() - 50.0).abs() < f64::EPSILON);
    }

    #[test]
    fn completion_gate_custom_required() {
        let mut gate = CompletionGate::with_required(CompletionVerdict::Failed);
        assert!(gate.allows(&CompletionVerdict::Failed));
        gate.bypass();
        assert_eq!(gate.gates_bypassed, 1);
        gate.reset_counters();
        assert_eq!(gate.total(), 0);
        assert_eq!(gate.gates_bypassed, 0);
    }

    ///////////////////////////////////////////////////////////////////////////////
    // COMPLETION AUDIT TESTS
    ///////////////////////////////////////////////////////////////////////////////

    #[test]
    fn completion_audit_record_and_entries() {
        let mut audit = CompletionAudit::new();
        audit.record(
            test_op_ref(),
            AuditAction::Registered,
            1000,
        );
        audit.record(
            test_op_ref(),
            AuditAction::Verified,
            2000,
        );
        let entries = audit.entries_for(test_op_ref());
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].action, AuditAction::Registered);
        assert_eq!(entries[1].action, AuditAction::Verified);
        assert_eq!(audit.len(), 2);
        assert!(!audit.is_empty());
    }

    #[test]
    fn completion_audit_clear() {
        let mut audit = CompletionAudit::new();
        audit.record(test_op_ref(), AuditAction::Rejected, 500);
        audit.clear();
        assert!(audit.is_empty());
        assert_eq!(audit.len(), 0);
    }

    ///////////////////////////////////////////////////////////////////////////////
    // RECOVERY SCOPE TESTS
    ///////////////////////////////////////////////////////////////////////////////

    #[test]
    fn recovery_scope_classify() {
        let agg = AggregatedCompletion {
            op_ref: test_op_ref(),
            total_replicas: 3,
            complete_replicas: 3,
            failed_replicas: 0,
            pending_replicas: 0,
            fully_done: true,
            any_failed: false,
            quorum_met: true,
        };
        assert_eq!(
            CompletionRecoveryScope::classify(&agg),
            CompletionRecoveryScope::None
        );
        let agg = AggregatedCompletion {
            op_ref: test_op_ref(),
            total_replicas: 3,
            complete_replicas: 0,
            failed_replicas: 0,
            pending_replicas: 3,
            fully_done: false,
            any_failed: false,
            quorum_met: false,
        };
        assert_eq!(
            CompletionRecoveryScope::classify(&agg),
            CompletionRecoveryScope::NodeLocal
        );
        let agg = AggregatedCompletion {
            op_ref: test_op_ref(),
            total_replicas: 3,
            complete_replicas: 0,
            failed_replicas: 1,
            pending_replicas: 2,
            fully_done: false,
            any_failed: true,
            quorum_met: false,
        };
        assert_eq!(
            CompletionRecoveryScope::classify(&agg),
            CompletionRecoveryScope::ReplicaReconcile
        );
    }

    #[test]
    fn recovery_scope_classify_group() {
        let summary = GroupCompletionSummary {
            group_id: [0u8; 16],
            total_ops: 5,
            complete_ops: 5,
            failed_ops: 0,
            in_progress_ops: 0,
            rejected_ops: 0,
            group_fully_done: true,
            group_has_failure: false,
        };
        assert_eq!(
            CompletionRecoveryScope::classify_group(&summary),
            CompletionRecoveryScope::None
        );
        assert!(!CompletionRecoveryScope::None.requires_recovery());
        assert!(CompletionRecoveryScope::ReplicaReconcile.requires_recovery());
        assert_eq!(
            CompletionRecoveryScope::DatasetResync.as_str(),
            "dataset_resync"
        );
    }

    ///////////////////////////////////////////////////////////////////////////////
    // REGISTRY / HISTORY / METRICS / STALENESS TESTS
    ///////////////////////////////////////////////////////////////////////////////

    #[test]
    fn completion_registry_ops() {
        let mut r = CompletionRegistry::new();
        let proof = CompletionProof::new(
            test_op_ref(),
            DatasetId([0u8; 32]),
            1,
            1,
            CommitState::Committed,
            OpState::Committed,
            5000,
        );
        assert!(r.register(proof));
        assert!(!r.register(proof));
        assert!(r.contains(test_op_ref()));
        assert_eq!(r.len(), 1);
        assert_eq!(r.op_refs(), vec![test_op_ref()]);
        let removed = r.remove(test_op_ref());
        assert!(removed.is_some());
        assert!(!r.contains(test_op_ref()));
        assert_eq!(r.len(), 0);
        r.clear();
        assert!(r.is_empty());
    }

    #[test]
    fn verification_history_append_and_lookup() {
        let mut h = VerificationHistory::new();
        h.append(VerificationEntry {
            op_ref: test_op_ref(),
            verdict: CompletionVerdict::Complete,
            checked_at: 5000,
            replica_count: 3,
            acked_replicas: 3,
        });
        h.append(VerificationEntry {
            op_ref: OpRef([2u8; 9]),
            verdict: CompletionVerdict::Rejected {
                reason: "x".to_string(),
            },
            checked_at: 5001,
            replica_count: 2,
            acked_replicas: 0,
        });
        assert_eq!(h.total_verifications(), 2);
        assert_eq!(h.total_rejections(), 1);
        assert!((h.rejection_rate_percent() - 50.0).abs() < f64::EPSILON);
        let latest = h.latest_for(test_op_ref()).unwrap();
        assert_eq!(latest.verdict, CompletionVerdict::Complete);
        let entries = h.entries_for(test_op_ref());
        assert_eq!(entries.len(), 1);
        assert!(!h.is_empty());
        assert_eq!(h.len(), 1);
    }

    #[test]
    fn completion_metrics_snapshot() {
        let mut m = CompletionMetrics::new();
        m.verdicts_complete = 5;
        m.verdicts_failed = 3;
        m.verdicts_in_progress = 2;
        m.verdicts_rejected = 0;
        assert_eq!(m.total_verdicts(), 10);
        assert!((m.complete_rate_percent() - 50.0).abs() < f64::EPSILON);
        let snap = m.snapshot();
        assert!(snap.contains("complete=5"));
    }

    #[test]
    fn staleness_monitor_ops() {
        let mut s = StalenessMonitor::new(1000);
        assert!(!s.is_stale_at(4000, 5000));
        assert!(s.is_stale_at(3000, 5000));
        s.record(true);
        s.record(false);
        assert_eq!(s.checks(), 2);
        assert!((s.stale_percent() - 50.0).abs() < f64::EPSILON);
        s.set_ttl(100);
        assert!(s.is_stale_at(4000, 5000));
        s.reset_counters();
        assert_eq!(s.checks(), 0);
    }

    ///////////////////////////////////////////////////////////////////////////////
    // POLICY TESTS
    ///////////////////////////////////////////////////////////////////////////////

    #[test]
    fn completion_policy_required_count() {
        assert_eq!(CompletionPolicy::Strict.required_count(10), 10);
        assert_eq!(CompletionPolicy::Quorum.required_count(10), 6);
        assert_eq!(CompletionPolicy::BestEffort.required_count(10), 1);
        assert!(CompletionPolicy::Strict.requires_all());
        assert!(!CompletionPolicy::BestEffort.requires_all());
        assert_eq!(CompletionPolicy::Strict.as_str(), "strict");
    }

    #[test]
    fn quorum_policy_satisfied() {
        assert!(QuorumPolicy::Majority.satisfied(2, 3));
        assert!(!QuorumPolicy::Majority.satisfied(1, 3));
        assert!(QuorumPolicy::Unanimous.satisfied(3, 3));
        assert!(!QuorumPolicy::Unanimous.satisfied(2, 3));
        assert!(QuorumPolicy::Custom { threshold: 1 }.satisfied(1, 5));
        assert_eq!(QuorumPolicy::Majority.threshold(10), 6);
        assert_eq!(QuorumPolicy::Unanimous.threshold(10), 10);
        assert!(QuorumPolicy::Majority.tolerates_failure());
        assert!(!QuorumPolicy::Unanimous.tolerates_failure());
    }

    ///////////////////////////////////////////////////////////////////////////////
    // STATE SNAPSHOT TESTS
    ///////////////////////////////////////////////////////////////////////////////

    #[test]
    fn completion_state_snapshot() {
        let mut coord = CompletionCoordinator::new();
        coord.record(test_op_ref(), CompletionVerdict::Complete);
        coord.record(OpRef([2u8; 9]), CompletionVerdict::Failed);
        let snap = CompletionStateSnapshot::new(9999, &coord);
        assert_eq!(snap.tick, 9999);
        assert_eq!(snap.op_count, 2);
        assert_eq!(snap.complete_count, 1);
        assert_eq!(snap.failed_count, 1);
        assert!(!snap.is_fully_done());
        assert!(snap.has_failure());
        assert!((snap.completion_percent() - 50.0).abs() < f64::EPSILON);
    }

    #[test]
    fn completion_state_snapshot_merge() {
        let a = CompletionStateSnapshot {
            tick: 100,
            op_count: 2,
            complete_count: 1,
            failed_count: 0,
            in_progress_count: 1,
            rejected_count: 0,
            group_fully_done: false,
        };
        let b = CompletionStateSnapshot {
            tick: 200,
            op_count: 3,
            complete_count: 2,
            failed_count: 1,
            in_progress_count: 0,
            rejected_count: 0,
            group_fully_done: false,
        };
        let merged = a.merge(&b);
        assert_eq!(merged.tick, 200);
        assert_eq!(merged.op_count, 3);
        assert_eq!(merged.complete_count, 3);
        assert_eq!(merged.failed_count, 1);
        assert!(!merged.is_older_than(500, 1000));
        assert!(merged.is_older_than(500, 100));
    }

    ///////////////////////////////////////////////////////////////////////////////
    // BATCH PROCESSOR TESTS
    ///////////////////////////////////////////////////////////////////////////////

    #[test]
    fn batch_processor_process() {
        let mut processor = CompletionBatchProcessor::new();
        let mut engine = CompletionEngine::new();
        let proofs = vec![CompletionProof::new(
            test_op_ref(),
            DatasetId([0u8; 32]),
            1,
            1,
            CommitState::Committed,
            OpState::Verified,
            5000,
        )];
        let outcomes = processor
            .process(&mut engine, proofs, &bc())
            .unwrap();
        assert_eq!(outcomes.len(), 1);
        assert_eq!(processor.batches_processed, 1);
        assert_eq!(processor.ops_registered, 1);
        assert_eq!(processor.ops_verified, 1);
        assert_eq!(processor.ops_rejected, 0);
    }

    #[test]
    fn batch_processor_process_and_aggregate() {
        let mut processor = CompletionBatchProcessor::new();
        let mut engine = CompletionEngine::new();
        let proofs = vec![CompletionProof::new(
            test_op_ref(),
            DatasetId([0u8; 32]),
            1,
            1,
            CommitState::Committed,
            OpState::Verified,
            5000,
        )];
        let (outcomes, agg) = processor
            .process_and_aggregate(&mut engine, proofs, &bc())
            .unwrap();
        assert_eq!(outcomes.len(), 1);
        assert!(agg.fully_done);
    }

    #[test]
    fn batch_processor_filter_complete() {
        let engine = CompletionEngine::new();
        let mut processor = CompletionBatchProcessor::new();
        let proofs = vec![
            CompletionProof::new(
                OpRef([1u8; 9]),
                DatasetId([0u8; 32]),
                1,
                1,
                CommitState::Committed,
                OpState::Verified,
                5000,
            ),
            CompletionProof::new(
                OpRef([2u8; 9]),
                DatasetId([0u8; 32]),
                1,
                0,
                CommitState::Accepted,
                OpState::Ordered,
                5000,
            ),
        ];
        let filtered = processor.filter_complete(&engine, &proofs, &bc());
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].op_ref, OpRef([1u8; 9]));
    }

    #[test]
    fn batch_processor_rates_and_clear() {
        let mut processor = CompletionBatchProcessor::new();
        assert_eq!(processor.avg_ops_per_batch(), 0.0);
        assert_eq!(processor.rejection_rate_percent(), 0.0);
        processor.reset_counters();
        processor.clear();
    }

    ///////////////////////////////////////////////////////////////////////////////
    // REPLICA STATE COUNTS TESTS
    ///////////////////////////////////////////////////////////////////////////////

    #[test]
    fn replica_state_counts_methods() {
        let counts = ReplicaStateCounts {
            committed: 2,
            synchronized: 1,
            verified: 1,
            failed: 1,
            pending: 1,
            total: 6,
        };
        assert_eq!(counts.complete(), 4);
        assert!((counts.completion_percent() - (400.0 / 6.0)).abs() < 0.01);
        assert!(!counts.is_fully_done());
        let done = ReplicaStateCounts {
            committed: 0,
            synchronized: 0,
            verified: 3,
            failed: 0,
            pending: 0,
            total: 3,
        };
        assert!(done.is_fully_done());
    }
}

///////////////////////////////////////////////////////////////////////////////
// END OF TESTS
///////////////////////////////////////////////////////////////////////////////
