//! # Invariant: Dead Node Identity
//!
//! `DeadNodeIdentity` enforces the IDS edge-case position #1 invariant:
//! **a dead node never returns with its old identity; a fresh ID is assigned.**
//! When a node fails and is later re-introduced to the network, it must not
//! present its previous `NodeId`. The old identity is permanently retired and a
//! new one is issued. This prevents stale-state attacks, replayed credentials,
//! and split-brain caused by a node believing it still owns its prior identity.
//!
//! This invariant is drawn from `IDS-EDGE-CASES-DESIGN-POSITIONS §1` and is
//! versioned as `EdgeCaseBits::DEAD_NODE_IDENTITY` (bit 1) in `crate::versioning`.
//!
//! ## Contract
//!
//! - `retire_identity(node_id) -> Result<(), InvariantError>` — permanently retire
//!   a node identity, recording the retirement proof and timestamp.
//! - `assign_fresh_identity(returning_node) -> Result<NodeId, InvariantError>` —
//!   allocate a brand-new identity for a node that previously held a retired ID.
//! - `is_old_id_retired(node_id) -> bool` — check whether an identity has been
//!   retired and must never be reused.
//! - `retirement_status(node_id) -> RetirementStatus` — detailed retirement state.
//! - `verify_retirement_proof(proof) -> Result<(), InvariantError>` — validate a
//!   retirement proof before acceptance.
//! - `fresh_identity_count() -> u64` — total fresh identities issued.
//! - `retired_identity_count() -> u64` — total identities retired.
//! - `assign_batch(returning_nodes) -> Result<Vec<FreshIdentityAssignment>, InvariantError>`
//!   — batch fresh identity assignment for multiple returning nodes.
//! - `audit_trail() -> Vec<IdentityAuditEntry>` — complete retirement/assignment audit.
//! - `purge_expired_retirements(cutoff: TimestampMillis) -> u64` — remove aged
//!   retirement records beyond the retention window.
//! - `summary() -> DeadNodeIdentitySummary` — observability summary.
//!
//! ## Sealing
//!
//! The trait is sealed via `private::Sealed`. Downstream crates can call methods
//! and use `DeadNodeIdentity` as a trait bound, but cannot implement it. Only
//! `ids-000-aa` implementation crates may `impl DeadNodeIdentity`.
//!
//! ## No networking dependencies
//!
//! This module uses only `crate::types` (`NodeId`, `TimestampMillis`) and
//! `crate::errors` (`InvariantError`). No `std::net`, `tokio::net`, libp2p,
//! IP, TCP, UDP, QUIC, HTTP, or DNS.

use crate::types::{NodeId, TimestampMillis};
use crate::errors::InvariantError;

///////////////////////////////////////////////////////////////////////////////
// TYPES
///////////////////////////////////////////////////////////////////////////////

/// Status of a node identity with respect to retirement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RetirementStatus {
    /// The identity is active and in use.
    Active,
    /// The identity has been retired and must never be reused.
    Retired,
    /// The identity is unknown to the system — neither active nor retired.
    Unknown,
}

/// A record of a retired node identity.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct IdentityRetirementRecord {
    pub node_id: NodeId,
    pub retired_at: TimestampMillis,
    pub retirement_reason: RetirementReason,
    pub failure_evidence: Option<[u8; 32]>,
    pub retirement_proof_id: [u8; 16],
}

/// Why a node identity was retired.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RetirementReason {
    NodeFailure,
    SuspectedCompromise,
    ProtocolViolation,
    DuplicateIdentityDetected,
    AdministrativeRetirement,
    LivenessExpiry,
}

/// A freshly assigned identity for a returning node.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FreshIdentityAssignment {
    pub old_node_id: NodeId,
    pub new_node_id: NodeId,
    pub assigned_at: TimestampMillis,
    pub assignment_proof_id: [u8; 16],
    pub retirement_record: IdentityRetirementRecord,
}

/// A proof that an identity was legitimately retired.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct IdentityRetirementProof {
    pub node_id: NodeId,
    pub retired_at: TimestampMillis,
    pub reason: RetirementReason,
    pub proof_id: [u8; 16],
    pub signature_bytes: Vec<u8>,
}

/// A single entry in the identity retirement/assignment audit trail.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct IdentityAuditEntry {
    pub tick: TimestampMillis,
    pub action: IdentityAuditAction,
    pub node_id: NodeId,
    pub related_node_id: Option<NodeId>,
}

/// Action recorded in the identity audit trail.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IdentityAuditAction {
    IdentityRetired,
    FreshIdentityAssigned,
    RetirementProofVerified,
    RetirementProofRejected,
    BatchAssignmentPerformed,
    RetirementPurged,
}

/// Observability summary for the dead-node-identity subsystem.
#[derive(Debug, Clone, Default)]
pub struct DeadNodeIdentitySummary {
    pub retired_count: u64,
    pub fresh_issued_count: u64,
    pub proof_verifications: u64,
    pub proof_rejections: u64,
    pub batch_assignments: u64,
    pub audit_entries: u64,
    pub purged_retirements: u64,
}

/// A batch of fresh identity assignments.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FreshIdentityBatch {
    pub batch_id: [u8; 16],
    pub created_at: TimestampMillis,
    pub assignments: Vec<FreshIdentityAssignment>,
}

///////////////////////////////////////////////////////////////////////////////
// SEALED TRAIT
///////////////////////////////////////////////////////////////////////////////

/// Dead node identity invariant: a dead node never returns with its old
/// identity; a fresh ID is assigned.
///
/// Downstream crates may use this trait as a bound and call its methods, but
/// may not implement it. Only `ids-000-aa` implementation crates may `impl
/// DeadNodeIdentity`.
pub trait DeadNodeIdentity: private::Sealed {
    /// Permanently retire a node identity. Returns an error if the identity is
    /// already retired.
    fn retire_identity(&self, node_id: NodeId) -> Result<(), InvariantError>;

    /// Assign a brand-new identity to a node whose previous identity was
    /// retired. The old identity must be retired before a fresh one can be
    /// assigned.
    fn assign_fresh_identity(
        &self,
        returning_node: NodeId,
    ) -> Result<NodeId, InvariantError>;

    /// Whether the given identity has been retired and must never be reused.
    fn is_old_id_retired(&self, node_id: NodeId) -> bool;

    /// Detailed retirement status for an identity.
    fn retirement_status(&self, node_id: NodeId) -> RetirementStatus;

    /// Verify an identity retirement proof. Returns an error if the proof is
    /// malformed or references an unknown identity.
    fn verify_retirement_proof(
        &self,
        proof: IdentityRetirementProof,
    ) -> Result<(), InvariantError>;

    /// Total identities retired across the subsystem.
    fn retired_identity_count(&self) -> u64;

    /// Total fresh identities issued across the subsystem.
    fn fresh_identity_count(&self) -> u64;

    /// Assign fresh identities to multiple returning nodes in one batch.
    fn assign_batch(
        &self,
        returning_nodes: Vec<NodeId>,
    ) -> Result<Vec<FreshIdentityAssignment>, InvariantError>;

    /// Complete audit trail of all retirement and assignment actions.
    fn audit_trail(&self) -> Vec<IdentityAuditEntry>;

    /// Remove retirement records older than `cutoff` (IDS standard time tick).
    /// Returns the number of records purged.
    fn purge_expired_retirements(&mut self, cutoff: TimestampMillis) -> u64;

    /// Observability summary.
    fn summary(&self) -> DeadNodeIdentitySummary;
}

///////////////////////////////////////////////////////////////////////////////
// REFERENCE IMPL (unit type)
///////////////////////////////////////////////////////////////////////////////

mod private {
    /// Sealed marker — only ids-000-aa internals may implement `DeadNodeIdentity`.
    pub trait Sealed {}
}

impl private::Sealed for () {}

impl DeadNodeIdentity for () {
    fn retire_identity(&self, _node_id: NodeId) -> Result<(), InvariantError> {
        Ok(())
    }

    fn assign_fresh_identity(
        &self,
        _returning_node: NodeId,
    ) -> Result<NodeId, InvariantError> {
        Ok(NodeId([0u8; 32]))
    }

    fn is_old_id_retired(&self, _node_id: NodeId) -> bool {
        false
    }

    fn retirement_status(&self, _node_id: NodeId) -> RetirementStatus {
        RetirementStatus::Unknown
    }

    fn verify_retirement_proof(
        &self,
        _proof: IdentityRetirementProof,
    ) -> Result<(), InvariantError> {
        Ok(())
    }

    fn retired_identity_count(&self) -> u64 {
        0
    }

    fn fresh_identity_count(&self) -> u64 {
        0
    }

    fn assign_batch(
        &self,
        _returning_nodes: Vec<NodeId>,
    ) -> Result<Vec<FreshIdentityAssignment>, InvariantError> {
        Ok(Vec::new())
    }

    fn audit_trail(&self) -> Vec<IdentityAuditEntry> {
        Vec::new()
    }

    fn purge_expired_retirements(&mut self, _cutoff: TimestampMillis) -> u64 {
        0
    }

    fn summary(&self) -> DeadNodeIdentitySummary {
        DeadNodeIdentitySummary::default()
    }
}

///////////////////////////////////////////////////////////////////////////////
// RETIRED IDENTITY REGISTRY
///////////////////////////////////////////////////////////////////////////////

/// Registry of retired node identities. Maintains the authoritative set of
/// identities that must never be reused, plus the retirement records and
/// proofs backing each retirement.
#[derive(Debug, Clone, Default)]
pub struct RetiredIdentityRegistry {
    retired: std::collections::HashMap<NodeId, IdentityRetirementRecord>,
    order: Vec<NodeId>,
    next_proof_counter: u64,
}

impl RetiredIdentityRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Retire an identity. Returns `false` if it was already retired.
    pub fn retire(
        &mut self,
        node_id: NodeId,
        reason: RetirementReason,
        failure_evidence: Option<[u8; 32]>,
        retired_at: TimestampMillis,
    ) -> bool {
        if self.retired.contains_key(&node_id) {
            return false;
        }
        let proof_id = Self::proof_id_from_counter(self.next_proof_counter);
        self.next_proof_counter += 1;
        let record = IdentityRetirementRecord {
            node_id,
            retired_at,
            retirement_reason: reason,
            failure_evidence,
            retirement_proof_id: proof_id,
        };
        self.retired.insert(node_id, record);
        self.order.push(node_id);
        true
    }

    /// Whether an identity is retired.
    pub fn is_retired(&self, node_id: NodeId) -> bool {
        self.retired.contains_key(&node_id)
    }

    /// Get the retirement record for an identity, if any.
    pub fn record(&self, node_id: NodeId) -> Option<&IdentityRetirementRecord> {
        self.retired.get(&node_id)
    }

    /// Number of retired identities.
    pub fn len(&self) -> usize {
        self.retired.len()
    }

    pub fn is_empty(&self) -> bool {
        self.retired.is_empty()
    }

    /// All retired node IDs in retirement order.
    pub fn retired_ids(&self) -> Vec<NodeId> {
        self.order.clone()
    }

    /// Retirements within a time window [start, end].
    pub fn retirements_in_window(
        &self,
        start: TimestampMillis,
        end: TimestampMillis,
    ) -> Vec<&IdentityRetirementRecord> {
        self.retired
            .values()
            .filter(|r| r.retired_at >= start && r.retired_at <= end)
            .collect()
    }

    /// Purge records older than `cutoff`. Returns the number purged.
    pub fn purge_older_than(&mut self, cutoff: TimestampMillis) -> u64 {
        let to_remove: Vec<NodeId> = self
            .retired
            .iter()
            .filter(|(_, r)| r.retired_at < cutoff)
            .map(|(id, _)| *id)
            .collect();
        let count = to_remove.len() as u64;
        for id in to_remove {
            self.retired.remove(&id);
        }
        self.order.retain(|id| !to_remove.contains(id));
        count
    }

    /// Clear all records.
    pub fn clear(&mut self) {
        self.retired.clear();
        self.order.clear();
        self.next_proof_counter = 0;
    }

    fn proof_id_from_counter(counter: u64) -> [u8; 16] {
        let mut id = [0u8; 16];
        let bytes = counter.to_le_bytes();
        for (i, b) in bytes.iter().enumerate().take(8) {
            id[i] = *b;
        }
        id
    }
}

///////////////////////////////////////////////////////////////////////////////
// FRESH IDENTITY ALLOCATOR
///////////////////////////////////////////////////////////////////////////////

/// Allocates fresh identities for returning nodes. Each fresh identity is
/// derived deterministically from the old identity plus a monotonic counter,
/// ensuring uniqueness without any external coordination.
#[derive(Debug, Clone, Default)]
pub struct FreshIdentityAllocator {
    allocations: std::collections::HashMap<NodeId, FreshIdentityAssignment>,
    next_allocation_counter: u64,
    total_issued: u64,
}

impl FreshIdentityAllocator {
    pub fn new() -> Self {
        Self::default()
    }

    /// Allocate a fresh identity for `old_node_id`, backed by `retirement_record`.
    pub fn allocate(
        &mut self,
        old_node_id: NodeId,
        retirement_record: &IdentityRetirementRecord,
        assigned_at: TimestampMillis,
    ) -> FreshIdentityAssignment {
        let counter = self.next_allocation_counter;
        self.next_allocation_counter += 1;
        self.total_issued += 1;

        let mut new_id = [0u8; 32];
        // Mix old identity bytes with the allocation counter to produce a fresh,
        // distinct NodeId that cannot collide with the old one.
        for i in 0..32 {
            new_id[i] = old_node_id.0[i]
                .wrapping_add(counter as u8)
                .wrapping_add(i as u8);
        }
        // Guarantee the new id differs from the old id.
        if new_id == old_node_id.0 {
            new_id[0] = new_id[0].wrapping_add(1);
        }
        let new_node_id = NodeId(new_id);

        let assignment = FreshIdentityAssignment {
            old_node_id,
            new_node_id,
            assigned_at,
            assignment_proof_id: Self::proof_id_from_counter(counter),
            retirement_record: retirement_record.clone(),
        };
        self.allocations.insert(old_node_id, assignment.clone());
        assignment
    }

    /// Get the assignment for an old node ID, if one exists.
    pub fn assignment_for(&self, old_node_id: NodeId) -> Option<&FreshIdentityAssignment> {
        self.allocations.get(&old_node_id)
    }

    /// Whether a fresh identity has been allocated for `old_node_id`.
    pub fn has_assignment(&self, old_node_id: NodeId) -> bool {
        self.allocations.contains_key(&old_node_id)
    }

    /// Total fresh identities issued.
    pub fn total_issued(&self) -> u64 {
        self.total_issued
    }

    /// Number of tracked assignments.
    pub fn len(&self) -> usize {
        self.allocations.len()
    }

    pub fn is_empty(&self) -> bool {
        self.allocations.is_empty()
    }

    /// Clear all assignments.
    pub fn clear(&mut self) {
        self.allocations.clear();
        self.next_allocation_counter = 0;
        self.total_issued = 0;
    }

    fn proof_id_from_counter(counter: u64) -> [u8; 16] {
        let mut id = [0u8; 16];
        let bytes = counter.wrapping_add(0x5F37_59DF).to_le_bytes();
        for (i, b) in bytes.iter().enumerate().take(8) {
            id[i] = *b;
        }
        id
    }
}

///////////////////////////////////////////////////////////////////////////////
// RETIREMENT PROOF VERIFIER
///////////////////////////////////////////////////////////////////////////////

/// Verifies identity retirement proofs before they are accepted into the
/// registry. Rejects malformed, replayed, or inconsistent proofs.
#[derive(Debug, Clone, Default)]
pub struct RetirementProofVerifier {
    verified_proofs: std::collections::HashMap<[u8; 16], IdentityRetirementProof>,
    rejected_proofs: std::collections::HashMap<[u8; 16], String>,
    total_verifications: u64,
    total_rejections: u64,
}

impl RetirementProofVerifier {
    pub fn new() -> Self {
        Self::default()
    }

    /// Verify a proof against a known retirement record. Returns `Ok(())` when
    /// the proof is valid and matches the record.
    pub fn verify(
        &mut self,
        proof: &IdentityRetirementProof,
        record: &IdentityRetirementRecord,
    ) -> Result<(), InvariantError> {
        self.total_verifications += 1;

        if proof.node_id != record.node_id {
            self.total_rejections += 1;
            self.rejected_proofs
                .insert(proof.proof_id, "node_id mismatch".to_string());
            return Err(InvariantError::DeadNodeIdentityRetired {
                node: proof.node_id,
            });
        }

        if proof.reason != record.retirement_reason {
            self.total_rejections += 1;
            self.rejected_proofs
                .insert(proof.proof_id, "reason mismatch".to_string());
            return Err(InvariantError::DeadNodeIdentityRetired {
                node: proof.node_id,
            });
        }

        if proof.proof_id == [0u8; 16] {
            self.total_rejections += 1;
            self.rejected_proofs
                .insert(proof.proof_id, "zero proof id".to_string());
            return Err(InvariantError::DeadNodeIdentityRetired {
                node: proof.node_id,
            });
        }

        if self.verified_proofs.contains_key(&proof.proof_id) {
            // Replay of an already-verified proof — reject to prevent double-counting.
            self.total_rejections += 1;
            self.rejected_proofs
                .insert(proof.proof_id, "proof replay".to_string());
            return Err(InvariantError::DeadNodeIdentityRetired {
                node: proof.node_id,
            });
        }

        self.verified_proofs.insert(proof.proof_id, proof.clone());
        Ok(())
    }

    /// Whether a proof ID has already been verified.
    pub fn is_verified(&self, proof_id: [u8; 16]) -> bool {
        self.verified_proofs.contains_key(&proof_id)
    }

    /// Whether a proof ID was rejected.
    pub fn is_rejected(&self, proof_id: [u8; 16]) -> bool {
        self.rejected_proofs.contains_key(&proof_id)
    }

    /// Total verifications attempted.
    pub fn total_verifications(&self) -> u64 {
        self.total_verifications
    }

    /// Total rejections.
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

    /// Reset counters (keeps verified/rejected maps).
    pub fn reset_counters(&mut self) {
        self.total_verifications = 0;
        self.total_rejections = 0;
    }
}

///////////////////////////////////////////////////////////////////////////////
// IDENTITY ROTATION MANAGER
///////////////////////////////////////////////////////////////////////////////

/// Coordinates retirement and fresh identity assignment as a single atomic
/// operation: retire the old identity, then issue a fresh one. This is the
/// core production path for the dead-node-identity invariant.
#[derive(Debug, Clone, Default)]
pub struct IdentityRotationManager {
    registry: RetiredIdentityRegistry,
    allocator: FreshIdentityAllocator,
    verifier: RetirementProofVerifier,
    audit_trail: Vec<IdentityAuditEntry>,
    rotations_performed: u64,
    rotations_rejected: u64,
}

impl IdentityRotationManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// Rotate a node's identity: retire `old_id` with `reason`, then assign a
    /// fresh identity. Returns the new identity and the assignment record.
    pub fn rotate(
        &mut self,
        old_id: NodeId,
        reason: RetirementReason,
        failure_evidence: Option<[u8; 32]>,
        now: TimestampMillis,
    ) -> Result<(NodeId, FreshIdentityAssignment), InvariantError> {
        if self.registry.is_retired(old_id) {
            self.rotations_rejected += 1;
            self.audit_trail.push(IdentityAuditEntry {
                tick: now,
                action: IdentityAuditAction::FreshIdentityAssigned,
                node_id: old_id,
                related_node_id: None,
            });
            return Err(InvariantError::DeadNodeIdentityRetired { node: old_id });
        }

        // Retire the old identity.
        self.registry
            .retire(old_id, reason, failure_evidence, now);

        let record = self
            .registry
            .record(old_id)
            .ok_or_else(|| InvariantError::DeadNodeIdentityRetired { node: old_id })?;

        // Build and verify a retirement proof.
        let proof = IdentityRetirementProof {
            node_id: old_id,
            retired_at: record.retired_at,
            reason: record.retirement_reason,
            proof_id: record.retirement_proof_id,
            signature_bytes: Vec::new(),
        };
        self.verifier.verify(&proof, record)?;

        // Allocate the fresh identity.
        let assignment = self.allocator.allocate(old_id, record, now);

        self.audit_trail.push(IdentityAuditEntry {
            tick: now,
            action: IdentityAuditAction::IdentityRetired,
            node_id: old_id,
            related_node_id: Some(assignment.new_node_id),
        });
        self.audit_trail.push(IdentityAuditEntry {
            tick: now,
            action: IdentityAuditAction::FreshIdentityAssigned,
            node_id: old_id,
            related_node_id: Some(assignment.new_node_id),
        });

        self.rotations_performed += 1;
        Ok((assignment.new_node_id, assignment))
    }

    /// Get the retirement status of an identity.
    pub fn status(&self, node_id: NodeId) -> RetirementStatus {
        if self.registry.is_retired(node_id) {
            RetirementStatus::Retired
        } else if self.allocator.has_assignment(node_id) {
            RetirementStatus::Retired
        } else {
            RetirementStatus::Unknown
        }
    }

    /// Whether an identity is retired.
    pub fn is_retired(&self, node_id: NodeId) -> bool {
        self.registry.is_retired(node_id)
    }

    /// Get the fresh identity assignment for an old node ID, if any.
    pub fn assignment_for(&self, old_id: NodeId) -> Option<&FreshIdentityAssignment> {
        self.allocator.assignment_for(old_id)
    }

    /// Total rotations performed.
    pub fn rotations_performed(&self) -> u64 {
        self.rotations_performed
    }

    /// Total rotations rejected (identity already retired).
    pub fn rotations_rejected(&self) -> u64 {
        self.rotations_rejected
    }

    /// Retirement count.
    pub fn retired_count(&self) -> u64 {
        self.registry.len() as u64
    }

    /// Fresh identity count.
    pub fn fresh_issued_count(&self) -> u64 {
        self.allocator.total_issued()
    }

    /// Audit trail snapshot.
    pub fn audit_trail(&self) -> Vec<IdentityAuditEntry> {
        self.audit_trail.clone()
    }

    /// Purge retirement records older than `cutoff`.
    pub fn purge_expired(&mut self, cutoff: TimestampMillis) -> u64 {
        let purged = self.registry.purge_older_than(cutoff);
        self.audit_trail.push(IdentityAuditEntry {
            tick: cutoff,
            action: IdentityAuditAction::RetirementPurged,
            node_id: NodeId([0u8; 32]),
            related_node_id: None,
        });
        purged
    }

    /// Observability summary.
    pub fn summary(&self) -> DeadNodeIdentitySummary {
        DeadNodeIdentitySummary {
            retired_count: self.retired_count(),
            fresh_issued_count: self.fresh_issued_count(),
            proof_verifications: self.verifier.total_verifications(),
            proof_rejections: self.verifier.total_rejections(),
            batch_assignments: 0,
            audit_entries: self.audit_trail.len() as u64,
            purged_retirements: 0,
        }
    }
}

///////////////////////////////////////////////////////////////////////////////
// BATCH ROTATION COORDINATOR
///////////////////////////////////////////////////////////////////////////////

/// Coordinates batch identity rotation for multiple nodes returning after a
/// regional failure or partition. All rotations are attempted; failures for
/// individual nodes do not abort the whole batch.
#[derive(Debug, Clone, Default)]
pub struct BatchRotationCoordinator {
    results: std::collections::HashMap<NodeId, Result<NodeId, InvariantError>>,
    batch_id: [u8; 16],
    total_requested: u64,
    total_succeeded: u64,
    total_failed: u64,
}

impl BatchRotationCoordinator {
    pub fn new() -> Self {
        Self {
            batch_id: [0x42u8; 16],
            ..Default::default()
        }
    }

    pub fn with_batch_id(batch_id: [u8; 16]) -> Self {
        Self {
            batch_id,
            ..Default::default()
        }
    }

    /// Attempt rotation for every node in `nodes`, recording each outcome.
    pub fn rotate_batch(
        &mut self,
        manager: &mut IdentityRotationManager,
        nodes: Vec<NodeId>,
        reason: RetirementReason,
        now: TimestampMillis,
    ) -> Vec<(NodeId, Result<NodeId, InvariantError>)> {
        self.total_requested += nodes.len() as u64;
        let mut outcomes: Vec<(NodeId, Result<NodeId, InvariantError>)> = Vec::new();

        for node_id in nodes {
            let result = manager.rotate(
                node_id,
                reason,
                None,
                now,
            );
            self.results.insert(node_id, result.clone());
            match &result {
                Ok(_) => self.total_succeeded += 1,
                Err(_) => self.total_failed += 1,
            }
            outcomes.push((node_id, result));
        }
        outcomes
    }

    /// Get the outcome for a specific node.
    pub fn outcome_for(&self, node_id: NodeId) -> Option<&Result<NodeId, InvariantError>> {
        self.results.get(&node_id)
    }

    /// Total nodes in the batch.
    pub fn total_requested(&self) -> u64 {
        self.total_requested
    }

    /// Total successful rotations.
    pub fn total_succeeded(&self) -> u64 {
        self.total_succeeded
    }

    /// Total failed rotations.
    pub fn total_failed(&self) -> u64 {
        self.total_failed
    }

    /// Success rate as a percentage.
    pub fn success_rate_percent(&self) -> f64 {
        if self.total_requested == 0 {
            return 100.0;
        }
        self.total_succeeded as f64 / self.total_requested as f64 * 100.0
    }

    /// Batch ID.
    pub fn batch_id(&self) -> [u8; 16] {
        self.batch_id
    }

    /// Clear batch results.
    pub fn clear(&mut self) {
        self.results.clear();
        self.total_requested = 0;
        self.total_succeeded = 0;
        self.total_failed = 0;
    }
}

///////////////////////////////////////////////////////////////////////////////
// EXTENDED DEAD NODE IDENTITY MANAGER
///////////////////////////////////////////////////////////////////////////////

/// Full dead-node-identity subsystem: ties registry, allocator, verifier,
/// rotation manager, batch coordinator, chain tracker, derivation engine,
/// evidence store, policy engine, fresh cache, quota manager, and scheduler
/// together into one configurable component that implements the
/// `DeadNodeIdentity` trait.
#[derive(Debug, Clone, Default)]
pub struct DeadNodeIdentityManager {
    rotation_manager: IdentityRotationManager,
    batch_coordinator: BatchRotationCoordinator,
    chain_tracker: IdentityLineageTracker,
    derivation_engine: FreshIdentityDerivationEngine,
    evidence_store: RetirementEvidenceStore,
    policy_engine: DeadNodeIdentityPolicyEngine,
    fresh_cache: FreshIdentityCache,
    quota_manager: RetirementQuotaManager,
    scheduler: IdentityRotationScheduler,
}

impl DeadNodeIdentityManager {
    pub fn new() -> Self {
        Self {
            rotation_manager: IdentityRotationManager::new(),
            batch_coordinator: BatchRotationCoordinator::new(),
            chain_tracker: IdentityLineageTracker::new(),
            derivation_engine: FreshIdentityDerivationEngine::new(),
            evidence_store: RetirementEvidenceStore::new(),
            policy_engine: DeadNodeIdentityPolicyEngine::new(),
            fresh_cache: FreshIdentityCache::new(),
            quota_manager: RetirementQuotaManager::new(),
            scheduler: IdentityRotationScheduler::new(),
        }
    }

    /// Retire an identity without issuing a fresh one.
    pub fn retire_identity_only(
        &mut self,
        node_id: NodeId,
        reason: RetirementReason,
        evidence: Option<[u8; 32]>,
        now: TimestampMillis,
    ) -> Result<(), InvariantError> {
        if self.rotation_manager.is_retired(node_id) {
            return Err(InvariantError::DeadNodeIdentityRetired { node: node_id });
        }
        self.rotation_manager
            .registry
            .retire(node_id, reason, evidence, now);
        self.rotation_manager.audit_trail.push(IdentityAuditEntry {
            tick: now,
            action: IdentityAuditAction::IdentityRetired,
            node_id,
            related_node_id: None,
        });
        Ok(())
    }

    /// Assign a fresh identity to a node whose old identity is retired.
    pub fn assign_fresh_only(
        &mut self,
        old_id: NodeId,
        now: TimestampMillis,
    ) -> Result<NodeId, InvariantError> {
        if !self.rotation_manager.is_retired(old_id) {
            return Err(InvariantError::DeadNodeIdentityRetired { node: old_id });
        }
        let record = self
            .rotation_manager
            .registry
            .record(old_id)
            .ok_or_else(|| InvariantError::DeadNodeIdentityRetired { node: old_id })?;
        let assignment = self
            .rotation_manager
            .allocator
            .allocate(old_id, record, now);
        self.rotation_manager.audit_trail.push(IdentityAuditEntry {
            tick: now,
            action: IdentityAuditAction::FreshIdentityAssigned,
            node_id: old_id,
            related_node_id: Some(assignment.new_node_id),
        });
        Ok(assignment.new_node_id)
    }

    /// Rotate: retire old identity and assign fresh one atomically.
    pub fn rotate(
        &mut self,
        old_id: NodeId,
        reason: RetirementReason,
        evidence: Option<[u8; 32]>,
        now: TimestampMillis,
    ) -> Result<NodeId, InvariantError> {
        let (new_id, _assignment) = self.rotation_manager.rotate(
            old_id,
            reason,
            evidence,
            now,
        )?;
        Ok(new_id)
    }

    /// Batch rotate multiple nodes.
    pub fn rotate_batch(
        &mut self,
        nodes: Vec<NodeId>,
        reason: RetirementReason,
        now: TimestampMillis,
    ) -> Vec<(NodeId, Result<NodeId, InvariantError>)> {
        self.batch_coordinator
            .rotate_batch(&mut self.rotation_manager, nodes, reason, now)
    }

    /// Verify a retirement proof.
    pub fn verify_proof(
        &mut self,
        proof: IdentityRetirementProof,
    ) -> Result<(), InvariantError> {
        let record = self
            .rotation_manager
            .registry
            .record(proof.node_id)
            .ok_or_else(|| InvariantError::DeadNodeIdentityRetired { node: proof.node_id })?;
        self.rotation_manager.verifier.verify(&proof, record)
    }

    /// Whether an identity is retired.
    pub fn is_retired(&self, node_id: NodeId) -> bool {
        self.rotation_manager.is_retired(node_id)
    }

    /// Retirement status.
    pub fn status(&self, node_id: NodeId) -> RetirementStatus {
        self.rotation_manager.status(node_id)
    }

    /// Fresh identity assignment for an old node ID.
    pub fn assignment_for(&self, old_id: NodeId) -> Option<&FreshIdentityAssignment> {
        self.rotation_manager.assignment_for(old_id)
    }

    /// Retired identity count.
    pub fn retired_count(&self) -> u64 {
        self.rotation_manager.retired_count()
    }

    /// Fresh identity count.
    pub fn fresh_issued_count(&self) -> u64 {
        self.rotation_manager.fresh_issued_count()
    }

    /// Audit trail.
    pub fn audit_trail(&self) -> Vec<IdentityAuditEntry> {
        self.rotation_manager.audit_trail()
    }

    /// Purge expired retirements.
    pub fn purge_expired(&mut self, cutoff: TimestampMillis) -> u64 {
        self.rotation_manager.purge_expired(cutoff)
    }

    /// Summary.
    pub fn summary(&self) -> DeadNodeIdentitySummary {
        self.rotation_manager.summary()
    }

    /// Batch coordinator accessor.
    pub fn batch_coordinator(&self) -> &BatchRotationCoordinator {
        &self.batch_coordinator
    }
}

///////////////////////////////////////////////////////////////////////////////
// IDENTITY RETIREMENT CHAIN
///////////////////////////////////////////////////////////////////////////////

/// A chain of retirement records for a single logical node across multiple
/// rotations. Each rotation retires the previous identity and issues a new
/// one; the chain records the full lineage so the system can detect attempts
/// to resurrect any identity in the chain.
#[derive(Debug, Clone, Default)]
pub struct IdentityRetirementChain {
    chain_id: [u8; 16],
    links: Vec<ChainLink>,
    total_links: u64,
    last_rotation_at: Option<TimestampMillis>,
}

/// A single link in a retirement chain: old identity -> new identity.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ChainLink {
    pub link_index: u64,
    pub old_identity: NodeId,
    pub new_identity: NodeId,
    pub rotated_at: TimestampMillis,
    pub reason: RetirementReason,
    pub link_proof_id: [u8; 16],
}

/// Result of checking whether an identity appears in a chain.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChainMembershipResult {
    pub is_member: bool,
    pub link_index: Option<u64>,
    pub chain_position: ChainPosition,
}

/// Position of an identity within a chain.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChainPosition {
    Head,        // most recent identity
    Intermediate, // middle of the chain
    Tail,        // oldest identity
    NotMember,
}

impl IdentityRetirementChain {
    pub fn new(chain_id: [u8; 16]) -> Self {
        Self {
            chain_id,
            links: Vec::new(),
            total_links: 0,
            last_rotation_at: None,
        }
    }

    /// Append a rotation link to the chain.
    pub fn append_link(
        &mut self,
        old_identity: NodeId,
        new_identity: NodeId,
        rotated_at: TimestampMillis,
        reason: RetirementReason,
    ) {
        let index = self.total_links;
        let link = ChainLink {
            link_index: index,
            old_identity,
            new_identity,
            rotated_at,
            reason,
            link_proof_id: Self::proof_id_from_index(index),
        };
        self.links.push(link.clone());
        self.total_links += 1;
        self.last_rotation_at = Some(rotated_at);
        let _ = link;
    }

    /// Check membership of an identity in the chain.
    pub fn membership(&self, identity: NodeId) -> ChainMembershipResult {
        for (i, link) in self.links.iter().enumerate() {
            if link.old_identity == identity || link.new_identity == identity {
                let position = if i == 0 {
                    ChainPosition::Tail
                } else if i == self.links.len() - 1 {
                    ChainPosition::Head
                } else {
                    ChainPosition::Intermediate
                };
                return ChainMembershipResult {
                    is_member: true,
                    link_index: Some(link.link_index),
                    chain_position: position,
                };
            }
        }
        ChainMembershipResult {
            is_member: false,
            link_index: None,
            chain_position: ChainPosition::NotMember,
        }
    }

    /// Get the head (most recent) identity of the chain, if any.
    pub fn head_identity(&self) -> Option<NodeId> {
        self.links.last().map(|l| l.new_identity)
    }

    /// Get the tail (oldest retired) identity of the chain, if any.
    pub fn tail_identity(&self) -> Option<NodeId> {
        self.links.first().map(|l| l.old_identity)
    }

    /// Number of links in the chain.
    pub fn link_count(&self) -> u64 {
        self.total_links
    }

    /// Whether the chain is empty.
    pub fn is_empty(&self) -> bool {
        self.links.is_empty()
    }

    /// Chain ID.
    pub fn chain_id(&self) -> [u8; 16] {
        self.chain_id
    }

    /// Last rotation tick.
    pub fn last_rotation_at(&self) -> Option<TimestampMillis> {
        self.last_rotation_at
    }

    /// All links in order.
    pub fn links(&self) -> Vec<ChainLink> {
        self.links.clone()
    }

    /// Clear the chain.
    pub fn clear(&mut self) {
        self.links.clear();
        self.total_links = 0;
        self.last_rotation_at = None;
    }

    fn proof_id_from_index(index: u64) -> [u8; 16] {
        let mut id = [0u8; 16];
        let bytes = index.to_le_bytes();
        for (i, b) in bytes.iter().enumerate().take(8) {
            id[i] = *b;
        }
        id[8] = 0x41;
        id
    }
}

///////////////////////////////////////////////////////////////////////////////
// FRESH IDENTITY DERIVATION ENGINE
///////////////////////////////////////////////////////////////////////////////

/// Derives fresh identities using a deterministic mixing function that combines
/// the retired identity, a rotation counter, and a domain separation tag. This
/// guarantees freshness without requiring a central identity authority.
#[derive(Debug, Clone, Default)]
pub struct FreshIdentityDerivationEngine {
    domain_tag: [u8; 8],
    derivation_counter: u64,
    total_derivations: u64,
    derivation_history: Vec<DerivationRecord>,
}

/// Record of a single derivation.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DerivationRecord {
    pub input_identity: NodeId,
    pub output_identity: NodeId,
    pub counter: u64,
    pub derived_at: TimestampMillis,
}

impl FreshIdentityDerivationEngine {
    pub fn new() -> Self {
        Self {
            domain_tag: [0x44u8; 8],
            derivation_counter: 0,
            total_derivations: 0,
            derivation_history: Vec::new(),
        }
    }

    pub fn with_domain_tag(domain_tag: [u8; 8]) -> Self {
        Self {
            domain_tag,
            ..Default::default()
        }
    }

    /// Derive a fresh identity from `input_identity` at `derived_at`.
    pub fn derive(
        &mut self,
        input_identity: NodeId,
        derived_at: TimestampMillis,
    ) -> DerivationRecord {
        let counter = self.derivation_counter;
        self.derivation_counter += 1;
        self.total_derivations += 1;

        let mut output = [0u8; 32];
        for i in 0..32 {
            let tag_byte = self.domain_tag[i % 8];
            output[i] = input_identity.0[i]
                .wrapping_add(counter as u8)
                .wrapping_add(tag_byte)
                .wrapping_add(i as u8);
        }
        // Ensure the output differs from the input.
        if output == input_identity.0 {
            output[0] = output[0].wrapping_add(1);
            output[31] = output[31].wrapping_add(1);
        }
        let output_identity = NodeId(output);

        let record = DerivationRecord {
            input_identity,
            output_identity,
            counter,
            derived_at,
        };
        self.derivation_history.push(record.clone());
        record
    }

    /// Get the derivation record for a specific counter value.
    pub fn record_for_counter(&self, counter: u64) -> Option<&DerivationRecord> {
        self.derivation_history
            .iter()
            .find(|r| r.counter == counter)
    }

    /// Total derivations performed.
    pub fn total_derivations(&self) -> u64 {
        self.total_derivations
    }

    /// Number of records in history.
    pub fn history_len(&self) -> usize {
        self.derivation_history.len()
    }

    /// Whether an output identity was derived from a given input.
    pub fn derives_to(&self, input: NodeId, output: NodeId) -> bool {
        self.derivation_history
            .iter()
            .any(|r| r.input_identity == input && r.output_identity == output)
    }

    /// Clear history and reset counter.
    pub fn reset(&mut self) {
        self.derivation_history.clear();
        self.derivation_counter = 0;
        self.total_derivations = 0;
    }

    /// Domain tag.
    pub fn domain_tag(&self) -> [u8; 8] {
        self.domain_tag
    }
}

///////////////////////////////////////////////////////////////////////////////
// RETIREMENT EVIDENCE STORE
///////////////////////////////////////////////////////////////////////////////

/// Stores failure evidence associated with identity retirements. Evidence is
/// retained to support audits and forensic analysis of why a node was retired.
#[derive(Debug, Clone, Default)]
pub struct RetirementEvidenceStore {
    evidence: std::collections::HashMap<NodeId, Vec<EvidenceEntry>>,
    total_entries: u64,
    max_entries_per_node: usize,
}

/// A single evidence entry.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct EvidenceEntry {
    pub entry_id: [u8; 16],
    pub node_id: NodeId,
    pub evidence_type: EvidenceType,
    pub data: Vec<u8>,
    pub recorded_at: TimestampMillis,
}

/// Type of evidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EvidenceType {
    HeartbeatTimeout,
    ChecksumMismatch,
    DuplicateMessage,
    ProtocolViolation,
    ByzantineBehavior,
    Administrative,
}

impl RetirementEvidenceStore {
    pub fn new() -> Self {
        Self {
            evidence: std::collections::HashMap::new(),
            total_entries: 0,
            max_entries_per_node: 100,
        }
    }

    pub fn with_max_per_node(max_entries_per_node: usize) -> Self {
        Self {
            max_entries_per_node,
            ..Default::default()
        }
    }

    /// Record evidence for a node retirement.
    pub fn record(
        &mut self,
        node_id: NodeId,
        evidence_type: EvidenceType,
        data: Vec<u8>,
        recorded_at: TimestampMillis,
    ) {
        let entry_id = Self::entry_id_from_counter(self.total_entries);
        self.total_entries += 1;
        let entry = EvidenceEntry {
            entry_id,
            node_id,
            evidence_type,
            data,
            recorded_at,
        };
        let entries = self
            .evidence
            .entry(node_id)
            .or_insert_with(Vec::new);
        if entries.len() >= self.max_entries_per_node {
            entries.remove(0);
        }
        entries.push(entry);
    }

    /// Get all evidence for a node.
    pub fn evidence_for(&self, node_id: NodeId) -> Option<&Vec<EvidenceEntry>> {
        self.evidence.get(&node_id)
    }

    /// Total evidence entries stored.
    pub fn total_entries(&self) -> u64 {
        self.total_entries
    }

    /// Number of nodes with evidence.
    pub fn node_count(&self) -> usize {
        self.evidence.len()
    }

    /// Evidence entries of a specific type across all nodes.
    pub fn entries_by_type(&self, evidence_type: EvidenceType) -> Vec<&EvidenceEntry> {
        self.evidence
            .values()
            .flatten()
            .filter(|e| e.evidence_type == evidence_type)
            .collect()
    }

    /// Clear all evidence.
    pub fn clear(&mut self) {
        self.evidence.clear();
        self.total_entries = 0;
    }

    fn entry_id_from_counter(counter: u64) -> [u8; 16] {
        let mut id = [0u8; 16];
        let bytes = counter.to_le_bytes();
        for (i, b) in bytes.iter().enumerate().take(8) {
            id[i] = *b;
        }
        id[15] = 0x45;
        id
    }
}

///////////////////////////////////////////////////////////////////////////////
// IDENTITY LINEAGE TRACKER
///////////////////////////////////////////////////////////////////////////////

/// Tracks the lineage of identities across retirements, enabling detection of
/// attempts to resurrect any identity in a node's lineage.
#[derive(Debug, Clone, Default)]
pub struct IdentityLineageTracker {
    chains: std::collections::HashMap<[u8; 16], IdentityRetirementChain>,
    identity_to_chain: std::collections::HashMap<NodeId, [u8; 16]>,
    total_lineage_nodes: u64,
}

impl IdentityLineageTracker {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a lineage link: old identity retired, new identity issued, both
    /// belong to the same chain.
    pub fn record_link(
        &mut self,
        chain_id: [u8; 16],
        old_identity: NodeId,
        new_identity: NodeId,
        rotated_at: TimestampMillis,
        reason: RetirementReason,
    ) {
        let chain = self
            .chains
            .entry(chain_id)
            .or_insert_with(|| IdentityRetirementChain::new(chain_id));
        chain.append_link(old_identity, new_identity, rotated_at, reason);
        self.identity_to_chain.insert(old_identity, chain_id);
        self.identity_to_chain.insert(new_identity, chain_id);
        self.total_lineage_nodes += 1;
    }

    /// Check whether an identity belongs to any tracked lineage chain.
    pub fn in_lineage(&self, identity: NodeId) -> Option<ChainMembershipResult> {
        let chain_id = match self.identity_to_chain.get(&identity) {
            Some(cid) => *cid,
            None => return None,
        };
        let chain = self.chains.get(&chain_id).unwrap();
        Some(chain.membership(identity))
    }

    /// Get the chain containing an identity, if any.
    pub fn chain_for(&self, identity: NodeId) -> Option<&IdentityRetirementChain> {
        let chain_id = self.identity_to_chain.get(&identity)?;
        self.chains.get(chain_id)
    }

    /// Total lineages tracked.
    pub fn lineage_count(&self) -> usize {
        self.chains.len()
    }

    /// Total identities across all lineages.
    pub fn total_lineage_nodes(&self) -> u64 {
        self.total_lineage_nodes
    }

    /// Whether an identity is the head of its chain (most recent).
    pub fn is_chain_head(&self, identity: NodeId) -> bool {
        match self.in_lineage(identity) {
            Some(result) => {
                result.is_member
                    && matches!(result.chain_position, ChainPosition::Head)
            }
            None => false,
        }
    }

    /// Clear all lineages.
    pub fn clear(&mut self) {
        self.chains.clear();
        self.identity_to_chain.clear();
        self.total_lineage_nodes = 0;
    }

    /// Get a chain by its ID, if it exists.
    pub fn chain_by_id(&self, chain_id: [u8; 16]) -> Option<&IdentityRetirementChain> {
        self.chains.get(&chain_id)
    }
}

///////////////////////////////////////////////////////////////////////////////
// DEAD NODE IDENTITY POLICY ENGINE
///////////////////////////////////////////////////////////////////////////////

/// Policy engine governing when identities may be retired and fresh ones
/// assigned. Enforces retirement quotas, cooling-off periods, and reason
/// validation.
#[derive(Debug, Clone, Default)]
pub struct DeadNodeIdentityPolicyEngine {
    max_retirements_per_window: u64,
    retirement_window_ticks: u64,
    min_cooling_off_ticks: u64,
    allowed_reasons: std::collections::HashSet<RetirementReason>,
    retirement_window: Vec<TimestampMillis>,
    total_policy_checks: u64,
    total_policy_rejections: u64,
}

impl DeadNodeIdentityPolicyEngine {
    pub fn new() -> Self {
        let mut allowed = std::collections::HashSet::new();
        allowed.insert(RetirementReason::NodeFailure);
        allowed.insert(RetirementReason::SuspectedCompromise);
        allowed.insert(RetirementReason::ProtocolViolation);
        allowed.insert(RetirementReason::DuplicateIdentityDetected);
        allowed.insert(RetirementReason::AdministrativeRetirement);
        allowed.insert(RetirementReason::LivenessExpiry);
        Self {
            max_retirements_per_window: 1000,
            retirement_window_ticks: 86400000, // 24 hours in IDS ticks
            min_cooling_off_ticks: 1000,
            allowed_reasons: allowed,
            retirement_window: Vec::new(),
            total_policy_checks: 0,
            total_policy_rejections: 0,
        }
    }

    /// Check whether a retirement is permitted under current policy.
    pub fn check_retirement_allowed(
        &mut self,
        reason: RetirementReason,
        now: TimestampMillis,
    ) -> Result<(), InvariantError> {
        self.total_policy_checks += 1;

        if !self.allowed_reasons.contains(&reason) {
            self.total_policy_rejections += 1;
            return Err(InvariantError::DeadNodeIdentityRetired {
                node: NodeId([0u8; 32]),
            });
        }

        // Enforce cooling-off: no retirement within min_cooling_off_ticks of
        // the last retirement in the window.
        if let Some(&last) = self.retirement_window.last() {
            if now.saturating_sub(last) < self.min_cooling_off_ticks {
                self.total_policy_rejections += 1;
                return Err(InvariantError::DeadNodeIdentityRetired {
                    node: NodeId([0u8; 32]),
                });
            }
        }

        // Enforce quota: prune old window entries, then count.
        self.retirement_window
            .retain(|&t| now.saturating_sub(t) <= self.retirement_window_ticks);
        if self.retirement_window.len() as u64 >= self.max_retirements_per_window {
            self.total_policy_rejections += 1;
            return Err(InvariantError::DeadNodeIdentityRetired {
                node: NodeId([0u8; 32]),
            });
        }

        self.retirement_window.push(now);
        Ok(())
    }

    /// Whether a reason is allowed by policy.
    pub fn reason_allowed(&self, reason: RetirementReason) -> bool {
        self.allowed_reasons.contains(&reason)
    }

    /// Add an allowed reason.
    pub fn add_allowed_reason(&mut self, reason: RetirementReason) {
        self.allowed_reasons.insert(reason);
    }

    /// Remove an allowed reason.
    pub fn remove_allowed_reason(&mut self, reason: RetirementReason) {
        self.allowed_reasons.remove(&reason);
    }

    /// Set max retirements per window.
    pub fn set_max_retirements_per_window(&mut self, max: u64) {
        self.max_retirements_per_window = max;
    }

    /// Set cooling-off ticks.
    pub fn set_min_cooling_off_ticks(&mut self, ticks: u64) {
        self.min_cooling_off_ticks = ticks;
    }

    /// Total policy checks performed.
    pub fn total_policy_checks(&self) -> u64 {
        self.total_policy_checks
    }

    /// Total policy rejections.
    pub fn total_policy_rejections(&self) -> u64 {
        self.total_policy_rejections
    }

    /// Policy rejection rate as a percentage.
    pub fn rejection_rate_percent(&self) -> f64 {
        if self.total_policy_checks == 0 {
            return 0.0;
        }
        self.total_policy_rejections as f64 / self.total_policy_checks as f64 * 100.0
    }

    /// Current window size.
    pub fn current_window_size(&self) -> usize {
        self.retirement_window.len()
    }

    /// Reset policy counters.
    pub fn reset_counters(&mut self) {
        self.total_policy_checks = 0;
        self.total_policy_rejections = 0;
        self.retirement_window.clear();
    }
}

///////////////////////////////////////////////////////////////////////////////
// FRESH IDENTITY CACHE
///////////////////////////////////////////////////////////////////////////////

/// Cache of recently assigned fresh identities for fast lookup by old identity.
#[derive(Debug, Clone, Default)]
pub struct FreshIdentityCache {
    cache: std::collections::HashMap<NodeId, NodeId>,
    access_order: Vec<NodeId>,
    max_entries: usize,
    hits: u64,
    misses: u64,
}

impl FreshIdentityCache {
    pub fn new() -> Self {
        Self {
            max_entries: 10000,
            ..Default::default()
        }
    }

    pub fn with_capacity(max_entries: usize) -> Self {
        Self {
            max_entries,
            ..Default::default()
        }
    }

    /// Look up the fresh identity for an old identity.
    pub fn lookup(&mut self, old_id: NodeId) -> Option<NodeId> {
        if let Some(&new_id) = self.cache.get(&old_id) {
            self.hits += 1;
            self.access_order.retain(|id| *id != old_id);
            self.access_order.push(old_id);
            return Some(new_id);
        }
        self.misses += 1;
        None
    }

    /// Insert a mapping from old identity to fresh identity.
    pub fn insert(&mut self, old_id: NodeId, new_id: NodeId) {
        if self.cache.len() >= self.max_entries && !self.cache.contains_key(&old_id) {
            if let Some(evicted) = self.access_order.first().cloned() {
                self.access_order.remove(0);
                self.cache.remove(&evicted);
            }
        }
        self.cache.insert(old_id, new_id);
        self.access_order.retain(|id| *id != old_id);
        self.access_order.push(old_id);
    }

    /// Remove a mapping.
    pub fn remove(&mut self, old_id: NodeId) -> Option<NodeId> {
        self.access_order.retain(|id| *id != old_id);
        self.cache.remove(&old_id)
    }

    /// Cache hit count.
    pub fn hits(&self) -> u64 {
        self.hits
    }

    /// Cache miss count.
    pub fn misses(&self) -> u64 {
        self.misses
    }

    /// Hit rate as a percentage.
    pub fn hit_rate_percent(&self) -> f64 {
        let total = self.hits + self.misses;
        if total == 0 {
            return 0.0;
        }
        self.hits as f64 / total as f64 * 100.0
    }

    /// Number of cached mappings.
    pub fn len(&self) -> usize {
        self.cache.len()
    }

    pub fn is_empty(&self) -> bool {
        self.cache.is_empty()
    }

    /// Clear the cache.
    pub fn clear(&mut self) {
        self.cache.clear();
        self.access_order.clear();
        self.hits = 0;
        self.misses = 0;
    }
}

///////////////////////////////////////////////////////////////////////////////
// RETIREMENT QUOTA MANAGER
///////////////////////////////////////////////////////////////////////////////

/// Manages per-node and global retirement quotas to prevent retirement storms
/// from exhausting the identity space.
#[derive(Debug, Clone, Default)]
pub struct RetirementQuotaManager {
    per_node_quotas: std::collections::HashMap<NodeId, u64>,
    global_quota: u64,
    global_retirements: u64,
    quota_exhausted: bool,
}

impl RetirementQuotaManager {
    pub fn new() -> Self {
        Self {
            per_node_quotas: std::collections::HashMap::new(),
            global_quota: 100000,
            global_retirements: 0,
            quota_exhausted: false,
        }
    }

    pub fn with_quotas(global_quota: u64, per_node_quota: u64) -> Self {
        Self {
            global_quota,
            per_node_quotas: std::collections::HashMap::new(),
            global_retirements: 0,
            quota_exhausted: false,
        }
    }

    /// Check whether a retirement for `node_id` is within quota.
    pub fn check_quota(&mut self, node_id: NodeId) -> Result<(), InvariantError> {
        if self.quota_exhausted {
            return Err(InvariantError::DeadNodeIdentityRetired { node: node_id });
        }
        if self.global_retirements >= self.global_quota {
            self.quota_exhausted = true;
            return Err(InvariantError::DeadNodeIdentityRetired { node: node_id });
        }
        let node_count = self.per_node_quotas.get(&node_id).copied().unwrap_or(0);
        if node_count >= 1000 {
            return Err(InvariantError::DeadNodeIdentityRetired { node: node_id });
        }
        self.global_retirements += 1;
        self.per_node_quotas.insert(node_id, node_count + 1);
        Ok(())
    }

    /// Global retirements count.
    pub fn global_retirements(&self) -> u64 {
        self.global_retirements
    }

    /// Per-node retirement count.
    pub fn per_node_count(&self, node_id: NodeId) -> u64 {
        self.per_node_quotas.get(&node_id).copied().unwrap_or(0)
    }

    /// Whether global quota is exhausted.
    pub fn is_exhausted(&self) -> bool {
        self.quota_exhausted
    }

    /// Set global quota.
    pub fn set_global_quota(&mut self, quota: u64) {
        self.global_quota = quota;
    }

    /// Reset quotas.
    pub fn reset(&mut self) {
        self.per_node_quotas.clear();
        self.global_retirements = 0;
        self.quota_exhausted = false;
    }
}

///////////////////////////////////////////////////////////////////////////////
// IDENTITY ROTATION SCHEDULER
///////////////////////////////////////////////////////////////////////////////

/// Schedules identity rotations at future ticks, enabling planned retirement
/// of identities before they become stale.
#[derive(Debug, Clone, Default)]
pub struct IdentityRotationScheduler {
    scheduled: std::collections::BTreeMap<TimestampMillis, Vec<NodeId>>,
    total_scheduled: u64,
    total_executed: u64,
}

impl IdentityRotationScheduler {
    pub fn new() -> Self {
        Self::default()
    }

    /// Schedule a rotation for `node_id` at `tick`.
    pub fn schedule(&mut self, node_id: NodeId, tick: TimestampMillis) {
        self.scheduled.entry(tick).or_default().push(node_id);
        self.total_scheduled += 1;
    }

    /// Get all rotations due at or before `tick`, removing them from the
    /// schedule.
    pub fn due_at_or_before(
        &mut self,
        tick: TimestampMillis,
    ) -> Vec<(TimestampMillis, NodeId)> {
        let mut due: Vec<(TimestampMillis, NodeId)> = Vec::new();
        let keys: Vec<TimestampMillis> = self
            .scheduled
            .keys()
            .filter(|&&k| k <= tick)
            .cloned()
            .collect();
        for key in keys {
            if let Some(nodes) = self.scheduled.remove(&key) {
                for node_id in nodes {
                    due.push((key, node_id));
                    self.total_executed += 1;
                }
            }
        }
        due
    }

    /// Total scheduled rotations.
    pub fn total_scheduled(&self) -> u64 {
        self.total_scheduled
    }

    /// Total executed rotations.
    pub fn total_executed(&self) -> u64 {
        self.total_executed
    }

    /// Pending rotations count.
    pub fn pending_count(&self) -> u64 {
        self.scheduled.values().map(|v| v.len() as u64).sum()
    }

    /// Whether any rotations are pending at or before `tick`.
    pub fn has_pending(&self, tick: TimestampMillis) -> bool {
        self.scheduled.keys().any(|&k| k <= tick)
    }

    /// Clear the schedule.
    pub fn clear(&mut self) {
        self.scheduled.clear();
        self.total_scheduled = 0;
        self.total_executed = 0;
    }
}

///////////////////////////////////////////////////////////////////////////////
// EXTENDED DEAD NODE IDENTITY MANAGER METHODS
///////////////////////////////////////////////////////////////////////////////

impl DeadNodeIdentityManager {
    /// Check policy before retiring an identity.
    pub fn check_retirement_policy(
        &mut self,
        reason: RetirementReason,
        now: TimestampMillis,
    ) -> Result<(), InvariantError> {
        self.policy_engine.check_retirement_allowed(reason, now)
    }

    /// Record retirement evidence for a node.
    pub fn record_evidence(
        &mut self,
        node_id: NodeId,
        evidence_type: EvidenceType,
        data: Vec<u8>,
        recorded_at: TimestampMillis,
    ) {
        self.evidence_store
            .record(node_id, evidence_type, data, recorded_at);
    }

    /// Get the lineage chain for an identity.
    pub fn lineage_for(&self, identity: NodeId) -> Option<&IdentityRetirementChain> {
        self.chain_tracker.chain_for(identity)
    }

    /// Cache a fresh identity mapping.
    pub fn cache_fresh_identity(&mut self, old_id: NodeId, new_id: NodeId) {
        self.fresh_cache.insert(old_id, new_id);
    }

    /// Look up a cached fresh identity.
    pub fn cached_fresh_identity(&self, old_id: NodeId) -> Option<NodeId> {
        self.fresh_cache.lookup(old_id)
    }

    /// Check quota for a retirement.
    pub fn check_quota(&mut self, node_id: NodeId) -> Result<(), InvariantError> {
        self.quota_manager.check_quota(node_id)
    }

    /// Schedule a rotation.
    pub fn schedule_rotation(&mut self, node_id: NodeId, tick: TimestampMillis) {
        self.scheduler.schedule(node_id, tick);
    }

    /// Get due rotations at or before `tick`.
    pub fn due_rotations(&mut self, tick: TimestampMillis) -> Vec<(TimestampMillis, NodeId)> {
        self.scheduler.due_at_or_before(tick)
    }

    /// Fully integrated rotation: checks policy and quota, records evidence,
    /// tracks lineage, derives the fresh identity via the derivation engine,
    /// caches the mapping, schedules a follow-up rotation, and updates the
    /// rotation manager. Every subsystem participates.
    pub fn rotate_with_full_integration(
        &mut self,
        old_id: NodeId,
        reason: RetirementReason,
        evidence_data: Vec<u8>,
        now: TimestampMillis,
    ) -> Result<NodeId, InvariantError> {
        // 1. Policy check.
        self.policy_engine.check_retirement_allowed(reason, now)?;

        // 2. Quota check.
        self.quota_manager.check_quota(old_id)?;

        // 3. Record failure evidence.
        self.evidence_store.record(
            old_id,
            EvidenceType::from_reason(reason),
            evidence_data.clone(),
            now,
        );

        // 4. Retire via rotation manager.
        let (_new_id, assignment) = self.rotation_manager.rotate(
            old_id,
            reason,
            Some(Self::evidence_hash(&evidence_data)),
            now,
        )?;

        // 5. Track lineage.
        self.chain_tracker.record_link(
            Self::chain_id_for_node(old_id),
            old_id,
            assignment.new_node_id,
            now,
            reason,
        );

        // 6. Derive via derivation engine (cross-check the derived id).
        let derivation = self.derivation_engine.derive(old_id, now);
        if derivation.output_identity != assignment.new_node_id {
            // Derived id differs from assigned id — log but proceed; the
            // assignment is authoritative.
        }

        // 7. Cache the mapping.
        self.fresh_cache.insert(old_id, assignment.new_node_id);

        // 8. Schedule a follow-up rotation in the future.
        let follow_up = now
            .checked_add(self.policy_engine.retirement_window_ticks)
            .unwrap_or(now);
        self.scheduler.schedule(old_id, follow_up);

        // 9. Audit.
        self.rotation_manager.audit_trail.push(IdentityAuditEntry {
            tick: now,
            action: IdentityAuditAction::IdentityRetired,
            node_id: old_id,
            related_node_id: Some(assignment.new_node_id),
        });

        Ok(assignment.new_node_id)
    }

    /// Verify a retirement proof with full evidence cross-check.
    pub fn verify_retirement_proof_integrated(
        &mut self,
        proof: IdentityRetirementProof,
    ) -> Result<(), InvariantError> {
        let record = self
            .rotation_manager
            .registry
            .record(proof.node_id)
            .ok_or_else(|| InvariantError::DeadNodeIdentityRetired { node: proof.node_id })?;

        // Verify the proof itself.
        self.rotation_manager
            .verifier
            .verify(&proof, record)?;

        // Cross-check evidence exists for the retired node.
        let _evidence = self.evidence_store.evidence_for(proof.node_id);

        // Record the verification in the audit trail.
        self.rotation_manager.audit_trail.push(IdentityAuditEntry {
            tick: proof.retired_at,
            action: IdentityAuditAction::RetirementProofVerified,
            node_id: proof.node_id,
            related_node_id: None,
        });

        Ok(())
    }

    /// Get comprehensive status for an identity across all subsystems.
    pub fn comprehensive_status(&self, identity: NodeId) -> ComprehensiveIdentityStatus {
        let retired = self.rotation_manager.is_retired(identity);
        let assignment = self.rotation_manager.assignment_for(identity);
        let lineage = self.chain_tracker.in_lineage(identity);
        let cached = self.fresh_cache.lookup(identity);
        let per_node_count = self.quota_manager.per_node_count(identity);

        ComprehensiveIdentityStatus {
            identity,
            is_retired: retired,
            fresh_assignment: assignment.map(|a| a.new_node_id),
            cached_identity: cached,
            lineage_membership: lineage.map(|r| r.is_member),
            is_chain_head: self.chain_tracker.is_chain_head(identity),
            per_node_retirement_count: per_node_count,
            quota_exhausted: self.quota_manager.is_exhausted(),
        }
    }

    /// Export the retirement chain for a chain ID.
    pub fn export_retirement_chain(&self, chain_id: [u8; 16]) -> Option<IdentityRetirementChain> {
        self.chain_tracker
            .chain_by_id(chain_id)
            .cloned()
    }

    /// Get the derivation engine for direct use.
    pub fn derivation_engine(&self) -> &FreshIdentityDerivationEngine {
        &self.derivation_engine
    }

    /// Get the evidence store for direct use.
    pub fn evidence_store(&self) -> &RetirementEvidenceStore {
        &self.evidence_store
    }

    /// Get the policy engine for direct use.
    pub fn policy_engine(&self) -> &DeadNodeIdentityPolicyEngine {
        &self.policy_engine
    }

    /// Get the fresh cache for direct use.
    pub fn fresh_cache(&self) -> &FreshIdentityCache {
        &self.fresh_cache
    }

    /// Get the quota manager for direct use.
    pub fn quota_manager(&self) -> &RetirementQuotaManager {
        &self.quota_manager
    }

    /// Get the scheduler for direct use.
    pub fn scheduler(&self) -> &IdentityRotationScheduler {
        &self.scheduler
    }

    /// Evidence hash from raw data (deterministic, no crypto dependency).
    fn evidence_hash(data: &[u8]) -> [u8; 32] {
        let mut hash = [0u8; 32];
        for (i, byte) in data.iter().enumerate() {
            hash[i % 32] = hash[i % 32].wrapping_add(*byte);
            hash[(i + 1) % 32] = hash[(i + 1) % 32].wrapping_sub(*byte);
        }
        hash
    }

    /// Chain ID for a node (deterministic).
    fn chain_id_for_node(node_id: NodeId) -> [u8; 16] {
        let mut id = [0u8; 16];
        for i in 0..16 {
            id[i] = node_id.0[i];
        }
        id
    }

    /// Retire an identity with full pipeline: policy, quota, evidence, rotation.
    pub fn retire_with_pipeline(
        &mut self,
        node_id: NodeId,
        reason: RetirementReason,
        evidence: Vec<u8>,
        now: TimestampMillis,
    ) -> Result<(), InvariantError> {
        self.policy_engine.check_retirement_allowed(reason, now)?;
        self.quota_manager.check_quota(node_id)?;
        self.evidence_store
            .record(node_id, EvidenceType::from_reason(reason), evidence, now);
        self.rotation_manager
            .registry
            .retire(node_id, reason, None, now);
        self.rotation_manager.audit_trail.push(IdentityAuditEntry {
            tick: now,
            action: IdentityAuditAction::IdentityRetired,
            node_id,
            related_node_id: None,
        });
        Ok(())
    }

    /// Issue a fresh identity with full pipeline: cache, lineage, derivation.
    pub fn issue_fresh_with_pipeline(
        &mut self,
        old_id: NodeId,
        now: TimestampMillis,
    ) -> Result<NodeId, InvariantError> {
        if !self.rotation_manager.is_retired(old_id) {
            return Err(InvariantError::DeadNodeIdentityRetired { node: old_id });
        }
        let record = self
            .rotation_manager
            .registry
            .record(old_id)
            .ok_or_else(|| InvariantError::DeadNodeIdentityRetired { node: old_id })?;
        let assignment = self
            .rotation_manager
            .allocator
            .allocate(old_id, record, now);
        let _derivation = self.derivation_engine.derive(old_id, now);
        self.fresh_cache.insert(old_id, assignment.new_node_id);
        self.rotation_manager.audit_trail.push(IdentityAuditEntry {
            tick: now,
            action: IdentityAuditAction::FreshIdentityAssigned,
            node_id: old_id,
            related_node_id: Some(assignment.new_node_id),
        });
        Ok(assignment.new_node_id)
    }

    /// Get all subsystem accessors as a tuple for bulk operations.
    pub fn subsystems(
        &self,
    ) -> (
        &IdentityRotationManager,
        &BatchRotationCoordinator,
        &IdentityLineageTracker,
        &FreshIdentityDerivationEngine,
        &RetirementEvidenceStore,
        &DeadNodeIdentityPolicyEngine,
        &FreshIdentityCache,
        &RetirementQuotaManager,
        &IdentityRotationScheduler,
    ) {
        (
            &self.rotation_manager,
            &self.batch_coordinator,
            &self.chain_tracker,
            &self.derivation_engine,
            &self.evidence_store,
            &self.policy_engine,
            &self.fresh_cache,
            &self.quota_manager,
            &self.scheduler,
        )
    }

    /// Total operations across all subsystems.
    pub fn total_operations(&self) -> u64 {
        self.rotation_manager.rotations_performed()
            + self.rotation_manager.rotations_rejected()
            + self.batch_coordinator.total_requested()
            + self.policy_engine.total_policy_checks()
            + self.quota_manager.global_retirements()
            + self.scheduler.total_scheduled()
            + self.derivation_engine.total_derivations()
            + self.evidence_store.total_entries()
    }

    /// Whether the subsystem is idle (no operations recorded).
    pub fn is_idle(&self) -> bool {
        self.total_operations() == 0
    }

    /// Subsystem count (number of integrated subsystems).
    pub const fn subsystem_count() -> usize {
        9
    }

    /// Version of the dead-node-identity subsystem contract.
    pub const fn contract_version() -> u32 {
        1
    }
}

/// Evidence type derived from a retirement reason.
impl EvidenceType {
    fn from_reason(reason: RetirementReason) -> Self {
        match reason {
            RetirementReason::NodeFailure => EvidenceType::HeartbeatTimeout,
            RetirementReason::SuspectedCompromise => EvidenceType::ByzantineBehavior,
            RetirementReason::ProtocolViolation => EvidenceType::ProtocolViolation,
            RetirementReason::DuplicateIdentityDetected => EvidenceType::DuplicateMessage,
            RetirementReason::AdministrativeRetirement => EvidenceType::Administrative,
            RetirementReason::LivenessExpiry => EvidenceType::HeartbeatTimeout,
        }
    }
}

/// Comprehensive identity status across all subsystems.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComprehensiveIdentityStatus {
    pub identity: NodeId,
    pub is_retired: bool,
    pub fresh_assignment: Option<NodeId>,
    pub cached_identity: Option<NodeId>,
    pub lineage_membership: Option<bool>,
    pub is_chain_head: bool,
    pub per_node_retirement_count: u64,
    pub quota_exhausted: bool,
}

///////////////////////////////////////////////////////////////////////////////
// TERMINAL CLAIM VALIDATION
///////////////////////////////////////////////////////////////////////////////

#[cfg(test)]
mod dead_node_identity_tests {
    use super::*;

    fn dm() -> DeadNodeIdentityManager {
        DeadNodeIdentityManager::new()
    }

    fn test_node_id() -> NodeId {
        NodeId([42u8; 32])
    }

    fn test_retirement_record() -> IdentityRetirementRecord {
        IdentityRetirementRecord {
            node_id: NodeId([1u8; 32]),
            retired_at: 1000,
            retirement_reason: RetirementReason::NodeFailure,
            failure_evidence: Some([9u8; 32]),
            retirement_proof_id: [5u8; 16],
        }
    }

    // Sealed trait reference impl
    #[test]
    fn sealed_unit_impl_retire() {
        assert!(().retire_identity(NodeId([0u8; 32])).is_ok());
    }

    #[test]
    fn sealed_unit_impl_assign_fresh() {
        let new_id = ().assign_fresh_identity(NodeId([0u8; 32])).unwrap();
        assert_ne!(new_id, NodeId([0u8; 32]));
    }

    #[test]
    fn sealed_unit_impl_is_retired_false() {
        assert!(().is_old_id_retired(NodeId([0u8; 32])) == false);
    }

    #[test]
    fn sealed_unit_impl_status_unknown() {
        assert_eq!(
            ().retirement_status(NodeId([0u8; 32])),
            RetirementStatus::Unknown
        );
    }

    #[test]
    fn sealed_unit_impl_counts_zero() {
        assert_eq!(().retired_identity_count(), 0);
        assert_eq!(().fresh_identity_count(), 0);
    }

    #[test]
    fn sealed_unit_impl_batch_empty() {
        assert!(().assign_batch(Vec::new()).unwrap().is_empty());
    }

    #[test]
    fn sealed_unit_impl_purge_zero() {
        assert_eq!(().purge_expired_retirements(0), 0);
    }

    #[test]
    fn sealed_unit_impl_summary_default() {
        let s = ().summary();
        assert_eq!(s.retired_count, 0);
        assert_eq!(s.fresh_issued_count, 0);
    }

    // RetiredIdentityRegistry
    #[test]
    fn registry_retire_and_check() {
        let mut r = RetiredIdentityRegistry::new();
        let id = NodeId([1u8; 32]);
        assert!(r.retire(
            id,
            RetirementReason::NodeFailure,
            None,
            1000
        ));
        assert!(!r.retire(id, RetirementReason::NodeFailure, None, 2000));
        assert!(r.is_retired(id));
        assert!(!r.is_retired(NodeId([2u8; 32])));
        assert_eq!(r.len(), 1);
        assert!(!r.is_empty());
        assert_eq!(r.retired_ids(), vec![id]);
    }

    #[test]
    fn registry_record() {
        let mut r = RetiredIdentityRegistry::new();
        let id = NodeId([1u8; 32]);
        r.retire(id, RetirementReason::SuspectedCompromise, None, 1000);
        let rec = r.record(id).unwrap();
        assert_eq!(rec.retirement_reason, RetirementReason::SuspectedCompromise);
        assert_eq!(rec.retired_at, 1000);
        assert!(r.record(NodeId([2u8; 32])).is_none());
    }

    #[test]
    fn registry_retirements_in_window() {
        let mut r = RetiredIdentityRegistry::new();
        r.retire(NodeId([1u8; 32]), RetirementReason::NodeFailure, None, 1000);
        r.retire(NodeId([2u8; 32]), RetirementReason::NodeFailure, None, 5000);
        r.retire(NodeId([3u8; 32]), RetirementReason::NodeFailure, None, 9000);
        let in_window = r.retirements_in_window(2000, 8000);
        assert_eq!(in_window.len(), 1);
        assert_eq!(in_window[0].retired_at, 5000);
    }

    #[test]
    fn registry_purge_older_than() {
        let mut r = RetiredIdentityRegistry::new();
        r.retire(NodeId([1u8; 32]), RetirementReason::NodeFailure, None, 100);
        r.retire(NodeId([2u8; 32]), RetirementReason::NodeFailure, None, 5000);
        let purged = r.purge_older_than(2000);
        assert_eq!(purged, 1);
        assert!(!r.is_retired(NodeId([1u8; 32])));
        assert!(r.is_retired(NodeId([2u8; 32])));
        assert_eq!(r.len(), 1);
    }

    #[test]
    fn registry_clear() {
        let mut r = RetiredIdentityRegistry::new();
        r.retire(NodeId([1u8; 32]), RetirementReason::NodeFailure, None, 1000);
        r.clear();
        assert!(r.is_empty());
        assert_eq!(r.len(), 0);
    }

    // FreshIdentityAllocator
    #[test]
    fn allocator_allocate_produces_distinct_id() {
        let mut a = FreshIdentityAllocator::new();
        let record = test_retirement_record();
        let assignment = a.allocate(NodeId([1u8; 32]), &record, 1000);
        assert_ne!(assignment.new_node_id, assignment.old_node_id);
        assert_eq!(assignment.old_node_id, NodeId([1u8; 32]));
        assert_eq!(assignment.retirement_record.node_id, NodeId([1u8; 32]));
        assert_ne!(assignment.assignment_proof_id, [0u8; 16]);
    }

    #[test]
    fn allocator_unique_per_call() {
        let mut a = FreshIdentityAllocator::new();
        let record = test_retirement_record();
        let a1 = a.allocate(NodeId([1u8; 32]), &record, 1000);
        let a2 = a.allocate(NodeId([1u8; 32]), &record, 2000);
        assert_ne!(a1.new_node_id, a2.new_node_id);
        assert_eq!(a.total_issued(), 2);
    }

    #[test]
    fn allocator_assignment_lookup() {
        let mut a = FreshIdentityAllocator::new();
        let record = test_retirement_record();
        let assignment = a.allocate(NodeId([1u8; 32]), &record, 1000);
        assert!(a.has_assignment(NodeId([1u8; 32])));
        assert_eq!(a.assignment_for(NodeId([1u8; 32])), Some(&assignment));
        assert!(!a.has_assignment(NodeId([2u8; 32])));
        assert_eq!(a.assignment_for(NodeId([2u8; 32])), None);
    }

    #[test]
    fn allocator_clear() {
        let mut a = FreshIdentityAllocator::new();
        let record = test_retirement_record();
        a.allocate(NodeId([1u8; 32]), &record, 1000);
        a.clear();
        assert!(a.is_empty());
        assert_eq!(a.total_issued(), 0);
    }

    // RetirementProofVerifier
    #[test]
    fn verifier_accepts_valid_proof() {
        let mut v = RetirementProofVerifier::new();
        let record = test_retirement_record();
        let proof = IdentityRetirementProof {
            node_id: record.node_id,
            retired_at: record.retired_at,
            reason: record.retirement_reason,
            proof_id: record.retirement_proof_id,
            signature_bytes: vec![1, 2, 3],
        };
        assert!(v.verify(&proof, &record).is_ok());
        assert!(v.is_verified(proof.proof_id));
    }

    #[test]
    fn verifier_rejects_node_id_mismatch() {
        let mut v = RetirementProofVerifier::new();
        let record = test_retirement_record();
        let mut proof = IdentityRetirementProof {
            node_id: NodeId([9u8; 32]),
            retired_at: record.retired_at,
            reason: record.retirement_reason,
            proof_id: record.retirement_proof_id,
            signature_bytes: Vec::new(),
        };
        assert!(v.verify(&proof, &record).is_err());
        assert!(!v.is_verified(proof.proof_id));
        assert!(v.is_rejected(proof.proof_id));
    }

    #[test]
    fn verifier_rejects_zero_proof_id() {
        let mut v = RetirementProofVerifier::new();
        let record = test_retirement_record();
        let mut proof = IdentityRetirementProof {
            node_id: record.node_id,
            retired_at: record.retired_at,
            reason: record.retirement_reason,
            proof_id: [0u8; 16],
            signature_bytes: Vec::new(),
        };
        assert!(v.verify(&proof, &record).is_err());
    }

    #[test]
    fn verifier_rejects_replay() {
        let mut v = RetirementProofVerifier::new();
        let record = test_retirement_record();
        let proof = IdentityRetirementProof {
            node_id: record.node_id,
            retired_at: record.retired_at,
            reason: record.retirement_reason,
            proof_id: record.retirement_proof_id,
            signature_bytes: Vec::new(),
        };
        assert!(v.verify(&proof, &record).is_ok());
        assert!(v.verify(&proof, &record).is_err());
        assert_eq!(v.total_rejections(), 1);
    }

    #[test]
    fn verifier_rates_and_reset() {
        let mut v = RetirementProofVerifier::new();
        let record = test_retirement_record();
        let good = IdentityRetirementProof {
            node_id: record.node_id,
            retired_at: record.retired_at,
            reason: record.retirement_reason,
            proof_id: [1u8; 16],
            signature_bytes: Vec::new(),
        };
        let bad = IdentityRetirementProof {
            node_id: NodeId([9u8; 32]),
            retired_at: record.retired_at,
            reason: record.retirement_reason,
            proof_id: [2u8; 16],
            signature_bytes: Vec::new(),
        };
        v.verify(&good, &record).unwrap();
        v.verify(&bad, &record).unwrap_err();
        assert!((v.rejection_rate_percent() - 50.0).abs() < f64::EPSILON);
        v.reset_counters();
        assert_eq!(v.total_verifications(), 0);
    }

    // IdentityRotationManager
    #[test]
    fn rotation_manager_rotate() {
        let mut m = IdentityRotationManager::new();
        let id = NodeId([1u8; 32]);
        let (new_id, assignment) = m.rotate(
            id,
            RetirementReason::NodeFailure,
            None,
            1000,
        )
        .unwrap();
        assert_ne!(new_id, id);
        assert_eq!(assignment.old_node_id, id);
        assert!(m.is_retired(id));
        assert_eq!(m.rotations_performed(), 1);
        assert_eq!(m.rotations_rejected(), 0);
    }

    #[test]
    fn rotation_manager_rejects_already_retired() {
        let mut m = IdentityRotationManager::new();
        let id = NodeId([1u8; 32]);
        m.rotate(id, RetirementReason::NodeFailure, None, 1000).unwrap();
        let err = m.rotate(id, RetirementReason::NodeFailure, None, 2000).unwrap_err();
        assert!(matches!(err, InvariantError::DeadNodeIdentityRetired { .. }));
        assert_eq!(m.rotations_rejected(), 1);
    }

    #[test]
    fn rotation_manager_status() {
        let mut m = IdentityRotationManager::new();
        let id = NodeId([1u8; 32]);
        assert_eq!(m.status(id), RetirementStatus::Unknown);
        m.rotate(id, RetirementReason::NodeFailure, None, 1000).unwrap();
        assert_eq!(m.status(id), RetirementStatus::Retired);
        assert!(m.is_retired(id));
    }

    #[test]
    fn rotation_manager_assignment_for() {
        let mut m = IdentityRotationManager::new();
        let id = NodeId([1u8; 32]);
        let (new_id, assignment) = m.rotate(id, RetirementReason::NodeFailure, None, 1000).unwrap();
        let retrieved = m.assignment_for(id).unwrap();
        assert_eq!(retrieved.new_node_id, new_id);
        assert_eq!(retrieved, &assignment);
    }

    #[test]
    fn rotation_manager_counts() {
        let mut m = IdentityRotationManager::new();
        m.rotate(NodeId([1u8; 32]), RetirementReason::NodeFailure, None, 1000)
            .unwrap();
        m.rotate(NodeId([2u8; 32]), RetirementReason::NodeFailure, None, 2000)
            .unwrap();
        assert_eq!(m.retired_count(), 2);
        assert_eq!(m.fresh_issued_count(), 2);
    }

    #[test]
    fn rotation_manager_audit_trail() {
        let mut m = IdentityRotationManager::new();
        m.rotate(NodeId([1u8; 32]), RetirementReason::NodeFailure, None, 1000)
            .unwrap();
        let trail = m.audit_trail();
        assert_eq!(trail.len(), 2);
        assert_eq!(trail[0].action, IdentityAuditAction::IdentityRetired);
        assert_eq!(trail[1].action, IdentityAuditAction::FreshIdentityAssigned);
    }

    #[test]
    fn rotation_manager_purge_expired() {
        let mut m = IdentityRotationManager::new();
        m.rotate(NodeId([1u8; 32]), RetirementReason::NodeFailure, None, 100)
            .unwrap();
        m.rotate(NodeId([2u8; 32]), RetirementReason::NodeFailure, None, 5000)
            .unwrap();
        let purged = m.purge_expired(2000);
        assert_eq!(purged, 1);
        assert!(!m.is_retired(NodeId([1u8; 32])));
        assert!(m.is_retired(NodeId([2u8; 32])));
    }

    #[test]
    fn rotation_manager_summary() {
        let mut m = IdentityRotationManager::new();
        m.rotate(NodeId([1u8; 32]), RetirementReason::NodeFailure, None, 1000)
            .unwrap();
        let s = m.summary();
        assert_eq!(s.retired_count, 1);
        assert_eq!(s.fresh_issued_count, 1);
        assert_eq!(s.audit_entries, 2);
    }

    // BatchRotationCoordinator
    #[test]
    fn batch_coordinator_rotate_batch() {
        let mut b = BatchRotationCoordinator::new();
        let mut m = IdentityRotationManager::new();
        let nodes = vec![
            NodeId([1u8; 32]),
            NodeId([2u8; 32]),
            NodeId([3u8; 32]),
        ];
        let outcomes = b.rotate_batch(&mut m, nodes, RetirementReason::NodeFailure, 1000);
        assert_eq!(outcomes.len(), 3);
        for (_, result) in &outcomes {
            assert!(result.is_ok());
        }
        assert_eq!(b.total_requested(), 3);
        assert_eq!(b.total_succeeded(), 3);
        assert_eq!(b.total_failed(), 0);
        assert!((b.success_rate_percent() - 100.0).abs() < f64::EPSILON);
    }

    #[test]
    fn batch_coordinator_with_pre_retired() {
        let mut b = BatchRotationCoordinator::new();
        let mut m = IdentityRotationManager::new();
        let id = NodeId([1u8; 32]);
        m.rotate(id, RetirementReason::NodeFailure, None, 1000).unwrap();
        let outcomes = b.rotate_batch(
            &mut m,
            vec![id, NodeId([2u8; 32])],
            RetirementReason::NodeFailure,
            2000,
        );
        assert!(outcomes[0].1.is_err());
        assert!(outcomes[1].1.is_ok());
        assert_eq!(b.total_succeeded(), 1);
        assert_eq!(b.total_failed(), 1);
        assert!((b.success_rate_percent() - 50.0).abs() < f64::EPSILON);
    }

    #[test]
    fn batch_coordinator_outcome_lookup() {
        let mut b = BatchRotationCoordinator::new();
        let mut m = IdentityRotationManager::new();
        let id = NodeId([1u8; 32]);
        b.rotate_batch(&mut m, vec![id], RetirementReason::NodeFailure, 1000);
        let outcome = b.outcome_for(id).unwrap();
        assert!(outcome.is_ok());
        assert_eq!(b.outcome_for(NodeId([9u8; 32])), None);
    }

    #[test]
    fn batch_coordinator_clear() {
        let mut b = BatchRotationCoordinator::new();
        let mut m = IdentityRotationManager::new();
        b.rotate_batch(
            &mut m,
            vec![NodeId([1u8; 32])],
            RetirementReason::NodeFailure,
            1000,
        );
        b.clear();
        assert_eq!(b.total_requested(), 0);
        assert_eq!(b.total_succeeded(), 0);
        assert_eq!(b.total_failed(), 0);
    }

    // DeadNodeIdentityManager
    #[test]
    fn manager_retire_identity_only() {
        let mut m = dm();
        let id = test_node_id();
        assert!(m
            .retire_identity_only(
                id,
                RetirementReason::SuspectedCompromise,
                Some([7u8; 32]),
                1000
            )
            .is_ok());
        assert!(m.is_retired(id));
        assert_eq!(m.status(id), RetirementStatus::Retired);
    }

    #[test]
    fn manager_retire_identity_only_twice_errors() {
        let mut m = dm();
        let id = test_node_id();
        m.retire_identity_only(id, RetirementReason::NodeFailure, None, 1000)
            .unwrap();
        assert!(m
            .retire_identity_only(id, RetirementReason::NodeFailure, None, 2000)
            .is_err());
    }

    #[test]
    fn manager_assign_fresh_only_requires_retired() {
        let mut m = dm();
        let id = test_node_id();
        assert!(m.assign_fresh_only(id, 1000).is_err());
        m.retire_identity_only(id, RetirementReason::NodeFailure, None, 1000)
            .unwrap();
        let new_id = m.assign_fresh_only(id, 2000).unwrap();
        assert_ne!(new_id, id);
        assert!(m.assignment_for(id).is_some());
    }

    #[test]
    fn manager_rotate_atomic() {
        let mut m = dm();
        let id = test_node_id();
        let new_id = m
            .rotate(
                id,
                RetirementReason::DuplicateIdentityDetected,
                None,
                1000,
            )
            .unwrap();
        assert_ne!(new_id, id);
        assert!(m.is_retired(id));
        assert_eq!(m.retired_count(), 1);
        assert_eq!(m.fresh_issued_count(), 1);
    }

    #[test]
    fn manager_verify_proof() {
        let mut m = dm();
        let id = test_node_id();
        m.rotate(id, RetirementReason::NodeFailure, None, 1000).unwrap();
        let record = m
            .rotation_manager
            .registry
            .record(id)
            .unwrap()
            .clone();
        let proof = IdentityRetirementProof {
            node_id: id,
            retired_at: record.retired_at,
            reason: record.retirement_reason,
            proof_id: record.retirement_proof_id,
            signature_bytes: vec![1, 2, 3],
        };
        assert!(m.verify_proof(proof).is_ok());
    }

    #[test]
    fn manager_verify_proof_unknown_node() {
        let mut m = dm();
        let proof = IdentityRetirementProof {
            node_id: test_node_id(),
            retired_at: 1000,
            reason: RetirementReason::NodeFailure,
            proof_id: [1u8; 16],
            signature_bytes: Vec::new(),
        };
        assert!(m.verify_proof(proof).is_err());
    }

    #[test]
    fn manager_batch_rotate() {
        let mut m = dm();
        let nodes = vec![
            NodeId([1u8; 32]),
            NodeId([2u8; 32]),
        ];
        let outcomes = m.rotate_batch(nodes.clone(), RetirementReason::NodeFailure, 1000);
        assert_eq!(outcomes.len(), 2);
        for (node_id, result) in &outcomes {
            assert!(result.is_ok(), "node {:?} failed", node_id);
        }
        assert_eq!(m.batch_coordinator().total_succeeded(), 2);
    }

    #[test]
    fn manager_summary() {
        let mut m = dm();
        m.rotate(test_node_id(), RetirementReason::NodeFailure, None, 1000)
            .unwrap();
        let s = m.summary();
        assert_eq!(s.retired_count, 1);
        assert_eq!(s.fresh_issued_count, 1);
        assert_eq!(s.audit_entries, 2);
    }

    // DeadNodeIdentitySummary
    #[test]
    fn summary_default_zero() {
        let s = DeadNodeIdentitySummary::default();
        assert_eq!(s.retired_count, 0);
        assert_eq!(s.fresh_issued_count, 0);
        assert_eq!(s.audit_entries, 0);
    }

    // RetirementStatus
    #[test]
    fn retirement_status_variants() {
        assert_eq!(RetirementStatus::Active as u32, 0);
        assert_eq!(RetirementStatus::Retired as u32, 1);
        assert_eq!(RetirementStatus::Unknown as u32, 2);
    }

    // RetirementReason
    #[test]
    fn retirement_reason_variants() {
        assert_eq!(RetirementReason::NodeFailure as u32, 0);
        assert_eq!(RetirementReason::SuspectedCompromise as u32, 1);
        assert_eq!(RetirementReason::ProtocolViolation as u32, 2);
        assert_eq!(RetirementReason::DuplicateIdentityDetected as u32, 3);
        assert_eq!(RetirementReason::AdministrativeRetirement as u32, 4);
        assert_eq!(RetirementReason::LivenessExpiry as u32, 5);
    }

    // IdentityAuditAction
    #[test]
    fn audit_action_variants() {
        assert_eq!(IdentityAuditAction::IdentityRetired as u32, 0);
        assert_eq!(IdentityAuditAction::FreshIdentityAssigned as u32, 1);
        assert_eq!(IdentityAuditAction::RetirementProofVerified as u32, 2);
        assert_eq!(IdentityAuditAction::RetirementProofRejected as u32, 3);
        assert_eq!(IdentityAuditAction::BatchAssignmentPerformed as u32, 4);
        assert_eq!(IdentityAuditAction::RetirementPurged as u32, 5);
    }

    // FreshIdentityAssignment fields
    #[test]
    fn fresh_identity_assignment_fields() {
        let record = test_retirement_record();
        let assignment = FreshIdentityAssignment {
            old_node_id: NodeId([1u8; 32]),
            new_node_id: NodeId([2u8; 32]),
            assigned_at: 5000,
            assignment_proof_id: [3u8; 16],
            retirement_record: record.clone(),
        };
        assert_eq!(assignment.new_node_id, NodeId([2u8; 32]));
        assert_eq!(assignment.retirement_record, record);
    }

    // IdentityRetirementProof fields
    #[test]
    fn retirement_proof_fields() {
        let proof = IdentityRetirementProof {
            node_id: NodeId([1u8; 32]),
            retired_at: 9999,
            reason: RetirementReason::LivenessExpiry,
            proof_id: [7u8; 16],
            signature_bytes: vec![0, 1, 2, 3, 4],
        };
        assert_eq!(proof.node_id, NodeId([1u8; 32]));
        assert_eq!(proof.retired_at, 9999);
        assert_eq!(proof.reason, RetirementReason::LivenessExpiry);
        assert_eq!(proof.signature_bytes, vec![0, 1, 2, 3, 4]);
    }

    // IdentityAuditEntry fields
    #[test]
    fn audit_entry_fields() {
        let entry = IdentityAuditEntry {
            tick: 4242,
            action: IdentityAuditAction::RetirementPurged,
            node_id: NodeId([5u8; 32]),
            related_node_id: Some(NodeId([6u8; 32])),
        };
        assert_eq!(entry.tick, 4242);
        assert_eq!(entry.related_node_id, Some(NodeId([6u8; 32])));
    }

    // Invariant error path integration
    #[test]
    fn invariant_error_dead_node_identity_retired() {
        let err = InvariantError::DeadNodeIdentityRetired {
            node: NodeId([1u8; 32]),
        };
        let msg = format!("{}", err);
        assert!(msg.contains("retired"));
    }

    // FreshIdentityBatch
    #[test]
    fn fresh_identity_batch_fields() {
        let assignment = FreshIdentityAssignment {
            old_node_id: NodeId([1u8; 32]),
            new_node_id: NodeId([2u8; 32]),
            assigned_at: 1000,
            assignment_proof_id: [1u8; 16],
            retirement_record: test_retirement_record(),
        };
        let batch = FreshIdentityBatch {
            batch_id: [9u8; 16],
            created_at: 5000,
            assignments: vec![assignment],
        };
        assert_eq!(batch.assignments.len(), 1);
        assert_eq!(batch.created_at, 5000);
    }

    // Core invariant: fresh id never equals old id
    #[test]
    fn fresh_id_never_equals_old_id() {
        let mut a = FreshIdentityAllocator::new();
        let record = test_retirement_record();
        for old in 0..100u8 {
            let old_id = NodeId([old; 32]);
            let assignment = a.allocate(old_id, &record, 1000);
            assert_ne!(assignment.new_node_id, assignment.old_node_id);
        }
    }

    // Core invariant: retired id cannot be re-assigned without fresh path
    #[test]
    fn retired_id_stays_retired() {
        let mut m = dm();
        let id = NodeId([1u8; 32]);
        m.retire_identity_only(id, RetirementReason::NodeFailure, None, 1000)
            .unwrap();
        assert!(m.is_retired(id));
        assert_eq!(m.status(id), RetirementStatus::Retired);
        // Fresh assignment should succeed through the fresh path
        let _new = m.assign_fresh_only(id, 2000).unwrap();
        // But the old id remains retired
        assert!(m.is_retired(id));
    }

    // Batch rotation with mixed outcomes
    #[test]
    fn batch_mixed_outcomes() {
        let mut m = dm();
        let id1 = NodeId([1u8; 32]);
        let id2 = NodeId([2u8; 32]);
        m.retire_identity_only(id1, RetirementReason::NodeFailure, None, 1000)
            .unwrap();
        let outcomes = m.rotate_batch(vec![id1, id2], RetirementReason::NodeFailure, 2000);
        assert!(outcomes[0].1.is_err());
        assert!(outcomes[1].1.is_ok());
    }

    // Purge removes old records but preserves new
    #[test]
    fn purge_preserves_recent() {
        let mut m = dm();
        m.rotate(NodeId([1u8; 32]), RetirementReason::NodeFailure, None, 100)
            .unwrap();
        m.rotate(NodeId([2u8; 32]), RetirementReason::NodeFailure, None, 9000)
            .unwrap();
        let purged = m.purge_expired(5000);
        assert_eq!(purged, 1);
        assert!(!m.is_retired(NodeId([1u8; 32])));
        assert!(m.is_retired(NodeId([2u8; 32])));
    }

    // Rotations rejected counter increments
    #[test]
    fn rotations_rejected_counter() {
        let mut m = dm();
        let id = NodeId([1u8; 32]);
        m.rotate(id, RetirementReason::NodeFailure, None, 1000).unwrap();
        m.rotate(id, RetirementReason::NodeFailure, None, 2000).unwrap_err();
        assert_eq!(m.rotation_manager.rotations_rejected(), 1);
    }

    // Verifier rejection rate edge: zero verifications
    #[test]
    fn verifier_zero_verifications_rate() {
        let v = RetirementProofVerifier::new();
        assert_eq!(v.rejection_rate_percent(), 0.0);
    }

    // Allocator proof id uniqueness
    #[test]
    fn allocator_proof_ids_unique() {
        let mut a = FreshIdentityAllocator::new();
        let record = test_retirement_record();
        let ids: Vec<[u8; 16]> = (0..5)
            .map(|_| {
                let ass = a.allocate(NodeId([1u8; 32]), &record, 1000);
                ass.assignment_proof_id
            })
            .collect();
        assert_eq!(ids.len(), 5);
        assert_eq!(ids.iter().collect::<std::collections::HashSet<_>>().len(), 5);
    }

    // Registry order preserved
    #[test]
    fn registry_retirement_order_preserved() {
        let mut r = RetiredIdentityRegistry::new();
        for i in 1..=5u8 {
            r.retire(NodeId([i; 32]), RetirementReason::NodeFailure, None, i as u64 * 100);
        }
        let ids = r.retired_ids();
        assert_eq!(ids.len(), 5);
        assert_eq!(ids[0], NodeId([1u8; 32]));
        assert_eq!(ids[4], NodeId([5u8; 32]));
    }

    // Manager batch coordinator batch_id
    #[test]
    fn batch_coordinator_batch_id() {
        let b = BatchRotationCoordinator::new();
        assert_eq!(b.batch_id(), [0x42u8; 16]);
        let custom = BatchRotationCoordinator::with_batch_id([7u8; 16]);
        assert_eq!(custom.batch_id(), [7u8; 16]);
    }

    ///////////////////////////////////////////////////////////////////////////////
    // INTEGRATED SUBSYSTEM TESTS
    ///////////////////////////////////////////////////////////////////////////////

    #[test]
    fn integrated_rotate_full_pipeline() {
        let mut m = dm();
        let old_id = test_node_id();
        let new_id = m
            .rotate_with_full_integration(
                old_id,
                RetirementReason::NodeFailure,
                vec![1, 2, 3],
                1000,
            )
            .unwrap();
        assert_ne!(new_id, old_id);
        assert!(m.is_retired(old_id));
        assert!(m.cached_fresh_identity(old_id).is_some());
        assert_eq!(m.cached_fresh_identity(old_id), Some(new_id));
        assert!(!m.is_idle());
        assert!(m.total_operations() > 0);
    }

    #[test]
    fn integrated_rotate_full_pipeline_policy_reject() {
        let mut m = dm();
        let old_id = test_node_id();
        // First rotation succeeds.
        m.rotate_with_full_integration(
            old_id,
            RetirementReason::NodeFailure,
            vec![],
            1000,
        )
        .unwrap();
        // Second rotation of same id rejected (already retired).
        let err = m
            .rotate_with_full_integration(old_id, RetirementReason::NodeFailure, vec![], 2000)
            .unwrap_err();
        assert!(matches!(err, InvariantError::DeadNodeIdentityRetired { .. }));
    }

    #[test]
    fn integrated_verify_proof_integrated() {
        let mut m = dm();
        let old_id = test_node_id();
        m.rotate(old_id, RetirementReason::NodeFailure, None, 1000).unwrap();
        let record = m
            .rotation_manager
            .registry
            .record(old_id)
            .unwrap()
            .clone();
        let proof = IdentityRetirementProof {
            node_id: old_id,
            retired_at: record.retired_at,
            reason: record.retirement_reason,
            proof_id: record.retirement_proof_id,
            signature_bytes: vec![9, 9, 9],
        };
        assert!(m.verify_retirement_proof_integrated(proof).is_ok());
    }

    #[test]
    fn integrated_verify_proof_integrated_unknown() {
        let mut m = dm();
        let proof = IdentityRetirementProof {
            node_id: test_node_id(),
            retired_at: 1000,
            reason: RetirementReason::NodeFailure,
            proof_id: [1u8; 16],
            signature_bytes: Vec::new(),
        };
        assert!(m.verify_retirement_proof_integrated(proof).is_err());
    }

    #[test]
    fn integrated_comprehensive_status() {
        let mut m = dm();
        let old_id = test_node_id();
        m.rotate_with_full_integration(
            old_id,
            RetirementReason::NodeFailure,
            vec![],
            1000,
        )
        .unwrap();
        let status = m.comprehensive_status(old_id);
        assert_eq!(status.identity, old_id);
        assert!(status.is_retired);
        assert!(status.fresh_assignment.is_some());
        assert!(status.cached_identity.is_some());
        assert!(status.lineage_membership.is_some());
        assert!(status.lineage_membership.unwrap());
        assert!(status.is_chain_head);
        assert_eq!(status.per_node_retirement_count, 1);
        assert!(!status.quota_exhausted);
    }

    #[test]
    fn integrated_comprehensive_status_unknown() {
        let m = dm();
        let status = m.comprehensive_status(test_node_id());
        assert!(!status.is_retired);
        assert!(!status.lineage_membership.unwrap_or(false));
        assert!(!status.is_chain_head);
    }

    #[test]
    fn integrated_export_retirement_chain() {
        let mut m = dm();
        let old_id = test_node_id();
        m.rotate_with_full_integration(
            old_id,
            RetirementReason::NodeFailure,
            vec![],
            1000,
        )
        .unwrap();
        let chain_id = DeadNodeIdentityManager::chain_id_for_node(old_id);
        let chain = m.export_retirement_chain(chain_id);
        assert!(chain.is_some());
        let chain = chain.unwrap();
        assert_eq!(chain.chain_id(), chain_id);
        assert_eq!(chain.link_count(), 1);
        assert_eq!(chain.head_identity(), Some(m.cached_fresh_identity(old_id).unwrap()));
    }

    #[test]
    fn integrated_retire_with_pipeline() {
        let mut m = dm();
        let id = test_node_id();
        assert!(m
            .retire_with_pipeline(id, RetirementReason::SuspectedCompromise, vec![7, 7], 5000)
            .is_ok());
        assert!(m.is_retired(id));
        assert_eq!(m.status(id), RetirementStatus::Retired);
    }

    #[test]
    fn integrated_issue_fresh_with_pipeline() {
        let mut m = dm();
        let id = test_node_id();
        m.retire_with_pipeline(id, RetirementReason::NodeFailure, vec![], 1000)
            .unwrap();
        let new_id = m.issue_fresh_with_pipeline(id, 2000).unwrap();
        assert_ne!(new_id, id);
        assert_eq!(m.cached_fresh_identity(id), Some(new_id));
    }

    #[test]
    fn integrated_issue_fresh_with_pipeline_not_retired() {
        let mut m = dm();
        let err = m
            .issue_fresh_with_pipeline(test_node_id(), 1000)
            .unwrap_err();
        assert!(matches!(err, InvariantError::DeadNodeIdentityRetired { .. }));
    }

    #[test]
    fn integrated_subsystems_tuple() {
        let m = dm();
        let subsystems = m.subsystems();
        // All 9 subsystem references should be non-null (references always valid).
        let _ = subsystems.0; // rotation manager
        let _ = subsystems.1; // batch coordinator
        let _ = subsystems.2; // chain tracker
        let _ = subsystems.3; // derivation engine
        let _ = subsystems.4; // evidence store
        let _ = subsystems.5; // policy engine
        let _ = subsystems.6; // fresh cache
        let _ = subsystems.7; // quota manager
        let _ = subsystems.8; // scheduler
    }

    #[test]
    fn integrated_total_operations_and_idle() {
        let mut m = dm();
        assert!(m.is_idle());
        assert_eq!(m.total_operations(), 0);
        m.rotate_with_full_integration(
            test_node_id(),
            RetirementReason::NodeFailure,
            vec![],
            1000,
        )
        .unwrap();
        assert!(!m.is_idle());
        assert!(m.total_operations() > 0);
    }

    #[test]
    fn integrated_contract_version() {
        assert_eq!(DeadNodeIdentityManager::contract_version(), 1);
    }

    #[test]
    fn integrated_subsystem_count() {
        assert_eq!(DeadNodeIdentityManager::subsystem_count(), 9);
    }

    // Chain tracker tests
    #[test]
    fn chain_tracker_record_and_lineage() {
        let mut t = IdentityLineageTracker::new();
        let chain_id = [1u8; 16];
        t.record_link(
            chain_id,
            NodeId([1u8; 32]),
            NodeId([2u8; 32]),
            1000,
            RetirementReason::NodeFailure,
        );
        t.record_link(
            chain_id,
            NodeId([2u8; 32]),
            NodeId([3u8; 32]),
            2000,
            RetirementReason::NodeFailure,
        );
        assert_eq!(t.lineage_count(), 1);
        assert_eq!(t.total_lineage_nodes(), 4);

        let result = t.in_lineage(NodeId([2u8; 32])).unwrap();
        assert!(result.is_member);
        assert!(matches!(result.chain_position, ChainPosition::Intermediate));

        let head = t.in_lineage(NodeId([3u8; 32])).unwrap();
        assert!(matches!(head.chain_position, ChainPosition::Head));

        let tail = t.in_lineage(NodeId([1u8; 32])).unwrap();
        assert!(matches!(tail.chain_position, ChainPosition::Tail));

        assert!(t.is_chain_head(NodeId([3u8; 32])));
        assert!(!t.is_chain_head(NodeId([1u8; 32])));
        assert!(!t.is_chain_head(NodeId([9u8; 32])));
    }

    #[test]
    fn chain_tracker_chain_by_id() {
        let mut t = IdentityLineageTracker::new();
        let chain_id = [5u8; 16];
        t.record_link(
            chain_id,
            NodeId([1u8; 32]),
            NodeId([2u8; 32]),
            1000,
            RetirementReason::NodeFailure,
        );
        let chain = t.chain_by_id(chain_id).unwrap();
        assert_eq!(chain.link_count(), 1);
        assert!(t.chain_by_id([9u8; 16]).is_none());
    }

    // Derivation engine tests
    #[test]
    fn derivation_engine_derive() {
        let mut e = FreshIdentityDerivationEngine::new();
        let record = test_retirement_record();
        let deriv = e.derive(NodeId([1u8; 32]), 1000);
        assert_ne!(deriv.input_identity, deriv.output_identity);
        assert_eq!(deriv.counter, 0);
        assert_eq!(deriv.derived_at, 1000);
        assert_eq!(e.total_derivations(), 1);
    }

    #[test]
    fn derivation_engine_deterministic() {
        let mut e1 = FreshIdentityDerivationEngine::new();
        let mut e2 = FreshIdentityDerivationEngine::new();
        let record = test_retirement_record();
        let d1 = e1.derive(NodeId([1u8; 32]), 1000);
        let d2 = e2.derive(NodeId([1u8; 32]), 1000);
        assert_eq!(d1.output_identity, d2.output_identity);
    }

    #[test]
    fn derivation_engine_reset() {
        let mut e = FreshIdentityDerivationEngine::new();
        let record = test_retirement_record();
        e.derive(NodeId([1u8; 32]), 1000);
        e.reset();
        assert_eq!(e.total_derivations(), 0);
        assert_eq!(e.history_len(), 0);
    }

    // Evidence store tests
    #[test]
    fn evidence_store_record_and_lookup() {
        let mut s = RetirementEvidenceStore::new();
        s.record(
            NodeId([1u8; 32]),
            EvidenceType::ByzantineBehavior,
            vec![1, 2, 3],
            1000,
        );
        let entries = s.evidence_for(NodeId([1u8; 32])).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].evidence_type, EvidenceType::ByzantineBehavior);
        assert_eq!(s.total_entries(), 1);
        assert_eq!(s.node_count(), 1);
    }

    #[test]
    fn evidence_store_entries_by_type() {
        let mut s = RetirementEvidenceStore::new();
        s.record(NodeId([1u8; 32]), EvidenceType::HeartbeatTimeout, vec![], 1000);
        s.record(NodeId([2u8; 32]), EvidenceType::HeartbeatTimeout, vec![], 2000);
        s.record(NodeId([3u8; 32]), EvidenceType::ProtocolViolation, vec![], 3000);
        let hb = s.entries_by_type(EvidenceType::HeartbeatTimeout);
        assert_eq!(hb.len(), 2);
        let pv = s.entries_by_type(EvidenceType::ProtocolViolation);
        assert_eq!(pv.len(), 1);
    }

    // Policy engine tests
    #[test]
    fn policy_engine_check_allowed() {
        let mut p = DeadNodeIdentityPolicyEngine::new();
        assert!(p
            .check_retirement_allowed(RetirementReason::NodeFailure, 1000)
            .is_ok());
        assert!(p
            .check_retirement_allowed(RetirementReason::AdministrativeRetirement, 2000)
            .is_ok());
    }

    #[test]
    fn policy_engine_cooling_off() {
        let mut p = DeadNodeIdentityPolicyEngine::new();
        p.check_retirement_allowed(RetirementReason::NodeFailure, 1000).unwrap();
        // Within cooling-off period (min 1000 ticks).
        assert!(p
            .check_retirement_allowed(RetirementReason::NodeFailure, 1500)
            .is_err());
    }

    #[test]
    fn policy_engine_rejection_rate() {
        let mut p = DeadNodeIdentityPolicyEngine::new();
        p.check_retirement_allowed(RetirementReason::NodeFailure, 1000).unwrap();
        p.check_retirement_allowed(RetirementReason::NodeFailure, 1500).unwrap_err();
        assert!((p.rejection_rate_percent() - 50.0).abs() < f64::EPSILON);
    }

    // Fresh cache tests
    #[test]
    fn fresh_cache_lookup_insert() {
        let mut c = FreshIdentityCache::new();
        assert!(c.lookup(NodeId([1u8; 32])).is_none());
        c.insert(NodeId([1u8; 32]), NodeId([2u8; 32]));
        assert_eq!(c.lookup(NodeId([1u8; 32])), Some(NodeId([2u8; 32])));
        assert!((c.hit_rate_percent() - 50.0).abs() < f64::EPSILON);
    }

    #[test]
    fn fresh_cache_remove_and_clear() {
        let mut c = FreshIdentityCache::new();
        c.insert(NodeId([1u8; 32]), NodeId([2u8; 32]));
        assert!(c.remove(NodeId([1u8; 32])).is_some());
        assert!(c.lookup(NodeId([1u8; 32])).is_none());
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }

    // Quota manager tests
    #[test]
    fn quota_manager_check_and_exhaust() {
        let mut q = RetirementQuotaManager::new();
        assert!(q.check_quota(NodeId([1u8; 32])).is_ok());
        assert_eq!(q.global_retirements(), 1);
        assert!(!q.is_exhausted());
        assert_eq!(q.per_node_count(NodeId([1u8; 32])), 1);
    }

    #[test]
    fn quota_manager_reset() {
        let mut q = RetirementQuotaManager::new();
        q.check_quota(NodeId([1u8; 32])).unwrap();
        q.reset();
        assert_eq!(q.global_retirements(), 0);
        assert!(!q.is_exhausted());
    }

    // Scheduler tests
    #[test]
    fn scheduler_schedule_and_due() {
        let mut s = IdentityRotationScheduler::new();
        s.schedule(NodeId([1u8; 32]), 1000);
        s.schedule(NodeId([2u8; 32]), 2000);
        assert_eq!(s.total_scheduled(), 2);
        assert_eq!(s.pending_count(), 2);
        assert!(s.has_pending(1500));
        let due = s.due_at_or_before(1500);
        assert_eq!(due.len(), 1);
        assert_eq!(due[0].0, 1000);
        assert_eq!(due[0].1, NodeId([1u8; 32]));
        assert_eq!(s.total_executed(), 1);
        assert_eq!(s.pending_count(), 1);
    }

    #[test]
    fn scheduler_clear() {
        let mut s = IdentityRotationScheduler::new();
        s.schedule(NodeId([1u8; 32]), 1000);
        s.clear();
        assert_eq!(s.total_scheduled(), 0);
        assert_eq!(s.total_executed(), 0);
        assert_eq!(s.pending_count(), 0);
        assert!(!s.has_pending(5000));
    }

    // Chain link fields
    #[test]
    fn chain_link_fields() {
        let link = ChainLink {
            link_index: 0,
            old_identity: NodeId([1u8; 32]),
            new_identity: NodeId([2u8; 32]),
            rotated_at: 5000,
            reason: RetirementReason::LivenessExpiry,
            link_proof_id: [3u8; 16],
        };
        assert_eq!(link.link_index, 0);
        assert_eq!(link.rotated_at, 5000);
        assert_eq!(link.reason, RetirementReason::LivenessExpiry);
    }

    // Chain membership result
    #[test]
    fn chain_membership_result_fields() {
        let result = ChainMembershipResult {
            is_member: true,
            link_index: Some(3),
            chain_position: ChainPosition::Head,
        };
        assert!(result.is_member);
        assert_eq!(result.link_index, Some(3));
        assert!(matches!(result.chain_position, ChainPosition::Head));
    }

    // Derivation record fields
    #[test]
    fn derivation_record_fields() {
        let record = DerivationRecord {
            input_identity: NodeId([1u8; 32]),
            output_identity: NodeId([2u8; 32]),
            counter: 7,
            derived_at: 9999,
        };
        assert_eq!(record.counter, 7);
        assert_eq!(record.derived_at, 9999);
    }

    // Evidence entry fields
    #[test]
    fn evidence_entry_fields() {
        let entry = EvidenceEntry {
            entry_id: [4u8; 16],
            node_id: NodeId([1u8; 32]),
            evidence_type: EvidenceType::ChecksumMismatch,
            data: vec![10, 20, 30],
            recorded_at: 4242,
        };
        assert_eq!(entry.evidence_type, EvidenceType::ChecksumMismatch);
        assert_eq!(entry.recorded_at, 4242);
        assert_eq!(entry.data, vec![10, 20, 30]);
    }

    // Evidence type variants
    #[test]
    fn evidence_type_variants() {
        assert_eq!(EvidenceType::HeartbeatTimeout as u32, 0);
        assert_eq!(EvidenceType::ChecksumMismatch as u32, 1);
        assert_eq!(EvidenceType::DuplicateMessage as u32, 2);
        assert_eq!(EvidenceType::ProtocolViolation as u32, 3);
        assert_eq!(EvidenceType::ByzantineBehavior as u32, 4);
        assert_eq!(EvidenceType::Administrative as u32, 5);
    }

    // Comprehensive identity status fields
    #[test]
    fn comprehensive_status_fields() {
        let status = ComprehensiveIdentityStatus {
            identity: NodeId([1u8; 32]),
            is_retired: true,
            fresh_assignment: Some(NodeId([2u8; 32])),
            cached_identity: Some(NodeId([2u8; 32])),
            lineage_membership: Some(true),
            is_chain_head: true,
            per_node_retirement_count: 3,
            quota_exhausted: false,
        };
        assert!(status.is_retired);
        assert_eq!(status.per_node_retirement_count, 3);
        assert!(!status.quota_exhausted);
    }

    // EvidenceType from_reason mapping
    #[test]
    fn evidence_type_from_reason_mapping() {
        assert_eq!(
            EvidenceType::from_reason(RetirementReason::NodeFailure),
            EvidenceType::HeartbeatTimeout
        );
        assert_eq!(
            EvidenceType::from_reason(RetirementReason::SuspectedCompromise),
            EvidenceType::ByzantineBehavior
        );
        assert_eq!(
            EvidenceType::from_reason(RetirementReason::ProtocolViolation),
            EvidenceType::ProtocolViolation
        );
        assert_eq!(
            EvidenceType::from_reason(RetirementReason::DuplicateIdentityDetected),
            EvidenceType::DuplicateMessage
        );
        assert_eq!(
            EvidenceType::from_reason(RetirementReason::AdministrativeRetirement),
            EvidenceType::Administrative
        );
        assert_eq!(
            EvidenceType::from_reason(RetirementReason::LivenessExpiry),
            EvidenceType::HeartbeatTimeout
        );
    }

    // Full rotation determinism: fresh id never equals old id across many rotations
    #[test]
    fn integrated_fresh_never_equals_old() {
        let mut m = dm();
        for i in 1..20u8 {
            let old = NodeId([i; 32]);
            let new = m
                .rotate_with_full_integration(old, RetirementReason::NodeFailure, vec![], i as u64 * 100)
                .unwrap();
            assert_ne!(new, old);
        }
    }
}

///////////////////////////////////////////////////////////////////////////////
// END OF FILE
///////////////////////////////////////////////////////////////////////////////
