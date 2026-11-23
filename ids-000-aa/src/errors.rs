///////////////////////////////////////////////////////////////////////////////
// ids-000-aa / src/errors.rs
//
// Canonical error contracts for the IDS protocol. Every downstream `ids-*`
// crate wraps `ids_000_aa::Error` variants and uses these domain-specific
// error types as bounds.
//
// Carrier-agnostic: no std::net, tokio::net, libp2p, IP, TCP, UDP, QUIC,
// HTTP, DNS, Wi-Fi, Bluetooth, cellular, VPN, or conventional mesh.
//
// Design principles:
//   1. Every error carries structured context (node, op, dataset, carrier...)
//      so callers can log, route, or retry intelligently.
//   2. Error types are sealed: downstream crates may match on variants but
//      cannot extend them — the contract is fixed in this crate.
//   3. All error constructors are `pub fn` so downstream crates can build
//      errors without importing private fields.
//   4. Recovery hints are attached to every error so operators know what to
//      do next (retry, abort, rotate, quarantine, etc.).
//   5. No hardcoded messages: every variant uses `#[error(...)]` with
//      interpolated context from the struct fields.

#![allow(dead_code)]

use thiserror::Error;

///////////////////////////////////////////////////////////////////////////////
// TOP-LEVEL ERROR
///////////////////////////////////////////////////////////////////////////////

/// The single error type used across the entire IDS workspace.
///
/// Downstream crates match on `Error::*` variants to implement domain-specific
/// handling. Each variant carries a domain-specific sub-error plus a
/// `RecoveryHint` that tells operators how to respond.
#[derive(Debug, Clone, Error)]
pub enum Error {
    #[error("carrier {0:?} not available: {1}")]
    CarrierUnavailable(CarrierType, String),

    #[error("transport failure on carrier {0:?}: {1}")]
    Transport(CarrierType, TransportError),

    #[error("store operation failed for object {0:?}: {1}")]
    Store(ObjectID, StoreError),

    #[error("session {0:?} error: {1}")]
    Session(SessionID, SessionError),

    #[error("routing failure: {0}")]
    Routing(RoutingError),

    #[error("discovery failure: {0}")]
    Discovery(DiscoveryError),

    #[error("membership failure: {0}")]
    Membership(MembershipError),

    #[error("placement failure: {0}")]
    Placement(PlacementError),

    #[error("replication failure: {0}")]
    Replication(ReplicationError),

    #[error("recovery failure: {0}")]
    Recovery(RecoveryError),

    #[error("consistency violation: {0}")]
    Consistency(ConsistencyError),

    #[error("ordering failure: {0}")]
    Ordering(OrderingError),

    #[error("security failure: {0}")]
    Security(SecurityError),

    #[error("client error: {0}")]
    Client(ClientError),

    #[error("fault tolerance failure: {0}")]
    Fault(FaultError),

    #[error("broadcast failure: {0}")]
    Broadcast(BroadcastError),

    #[error("quorum failure: {0}")]
    Quorum(QuorumError),

    #[error("snapshot failure: {0}")]
    Snapshot(SnapshotError),

    #[error("byzantine behavior detected: {0}")]
    Byzantine(ByzantineError),

    #[error("lease failure: {0}")]
    Lease(LeaseError),

    #[error("CRDT failure: {0}")]
    CRDT(CRDTError),

    #[error("erasure coding failure: {0}")]
    Erasure(ErasureError),

    #[error("saga failure: {0}")]
    Saga(SagaError),

    #[error("coordinator failure: {0}")]
    Coordinator(CoordinatorError),

    #[error("durability failure: {0}")]
    Durability(DurabilityError),

    #[error("topology failure: {0}")]
    Topology(TopologyError),

    #[error("integration failure: {0}")]
    Integration(IntegrationError),

    #[error("protocol violation: {0}")]
    Protocol(ProtocolError),

    #[error("wire format error: {0}")]
    Wire(WireError),

    #[error("framing error: {0}")]
    Framing(FramingError),

    #[error("sequencing error: {0}")]
    Sequencing(SequencingError),

    #[error("ack error: {0}")]
    Ack(AckError),

    #[error("invariant violated: {invariant}")]
    InvariantViolated { invariant: &'static str },

    #[error("lifecycle error: {0}")]
    Lifecycle(LifecycleError),

    #[error("deletion error: {0}")]
    Deletion(DeletionError),

    #[error("fencing error: {0}")]
    Fencing(FencingError),

    #[error("roster error: {0}")]
    Roster(RosterError),

    #[error("fallback error: {0}")]
    Fallback(FallbackError),

    #[error("diversity error: {0}")]
    Diversity(DiversityError),

    #[error("fanout error: {0}")]
    Fanout(FanoutError),

    #[error("graph error: {0}")]
    Graph(GraphError),

    #[error("integrity error: {0}")]
    Integrity(IntegrityError),

    #[error("rotation error: {0}")]
    Rotation(RotationError),

    #[error("retirement error: {0}")]
    Retirement(RetirementError),

    #[error("time error: {0}")]
    Time(TimeError),

    #[error("checkpoint error: {0}")]
    Checkpoint(CheckpointError),

    #[error("channel error: {0}")]
    Channel(ChannelError),

    #[error("binary completion required: operation {0:?} not 100% done")]
    BinaryCompletionRequired { op: OpRef },

    #[error("dead node identity retired: {0:?}")]
    DeadNodeIdentityRetired { node: NodeId },

    #[error("idle rotation violation: node {0:?} is active")]
    IdleRotationViolation { node: NodeId },

    #[error("global master rejected: domain {0:?}")]
    GlobalMasterRejected { domain: OrderingDomain },

    #[error("out-of-equation-in accounting violation: {0}")]
    OutEqInViolation { message: String },

    #[error("coordinated deletion failed: proposal {0:?}")]
    CoordinatedDeletionFailed { proposal: DeleteProposalId },

    #[error("version mismatch: expected {expected}, got {got}")]
    VersionMismatch { expected: ProtocolVersion, got: ProtocolVersion },

    #[error("carrier {0:?} rejected the endpoint format")]
    CarrierRejectedEndpoint { carrier: CarrierType },
}

impl Error {
    /// Recovery hint: what an operator or automation should do next.
    pub fn recovery_hint(&self) -> RecoveryHint {
        match self {
            Error::CarrierUnavailable(..) => RecoveryHint::RetryWithDifferentCarrier,
            Error::Transport(_, e) => e.recovery_hint(),
            Error::Store(_, e) => e.recovery_hint(),
            Error::Session(_, e) => e.recovery_hint(),
            Error::Routing(e) => e.recovery_hint(),
            Error::Discovery(e) => e.recovery_hint(),
            Error::Membership(e) => e.recovery_hint(),
            Error::Placement(e) => e.recovery_hint(),
            Error::Replication(e) => e.recovery_hint(),
            Error::Recovery(e) => e.recovery_hint(),
            Error::Consistency(e) => e.recovery_hint(),
            Error::Ordering(e) => e.recovery_hint(),
            Error::Security(e) => e.recovery_hint(),
            Error::Client(e) => e.recovery_hint(),
            Error::Fault(e) => e.recovery_hint(),
            Error::Broadcast(e) => e.recovery_hint(),
            Error::Quorum(e) => e.recovery_hint(),
            Error::Snapshot(e) => e.recovery_hint(),
            Error::Byzantine(e) => e.recovery_hint(),
            Error::Lease(e) => e.recovery_hint(),
            Error::CRDT(e) => e.recovery_hint(),
            Error::Erasure(e) => e.recovery_hint(),
            Error::Saga(e) => e.recovery_hint(),
            Error::Coordinator(e) => e.recovery_hint(),
            Error::Durability(e) => e.recovery_hint(),
            Error::Topology(e) => e.recovery_hint(),
            Error::Integration(e) => e.recovery_hint(),
            Error::Protocol(e) => e.recovery_hint(),
            Error::Wire(e) => e.recovery_hint(),
            Error::Framing(e) => e.recovery_hint(),
            Error::Sequencing(e) => e.recovery_hint(),
            Error::Ack(e) => e.recovery_hint(),
            Error::InvariantViolated { invariant } => {
                RecoveryHint::InvestigateInvariant { invariant: *invariant }
            }
            Error::Lifecycle(e) => e.recovery_hint(),
            Error::Deletion(e) => e.recovery_hint(),
            Error::Fencing(e) => e.recovery_hint(),
            Error::Roster(e) => e.recovery_hint(),
            Error::Fallback(e) => e.recovery_hint(),
            Error::Diversity(e) => e.recovery_hint(),
            Error::Fanout(e) => e.recovery_hint(),
            Error::Graph(e) => e.recovery_hint(),
            Error::Integrity(e) => e.recovery_hint(),
            Error::Rotation(e) => e.recovery_hint(),
            Error::Retirement(e) => e.recovery_hint(),
            Error::Time(e) => e.recovery_hint(),
            Error::Checkpoint(e) => e.recovery_hint(),
            Error::Channel(e) => e.recovery_hint(),
            Error::BinaryCompletionRequired { .. } => RecoveryHint::AbortAndRetry,
            Error::DeadNodeIdentityRetired { .. } => RecoveryHint::AssignNewIdentity,
            Error::IdleRotationViolation { .. } => RecoveryHint::WaitForIdle,
            Error::GlobalMasterRejected { .. } => RecoveryHint::UseDatasetScopedOrdering,
            Error::OutEqInViolation { .. } => RecoveryHint::ReconcileAccounting,
            Error::CoordinatedDeletionFailed { .. } => RecoveryHint::AbortAndRetry,
            Error::VersionMismatch { .. } => RecoveryHint::NegotiateVersion,
            Error::CarrierRejectedEndpoint { .. } => RecoveryHint::UseCarrierAgnosticEndpoint,
        }
    }

    /// Whether this error is transient (safe to retry).
    pub fn is_transient(&self) -> bool {
        matches!(
            self.recovery_hint(),
            RecoveryHint::RetryWithDifferentCarrier
                | RecoveryHint::AbortAndRetry
                | RecoveryHint::WaitForIdle
                | RecoveryHint::NegotiateVersion
        )
    }

    /// Whether this error requires operator intervention.
    pub fn requires_operator(&self) -> bool {
        matches!(
            self.recovery_hint(),
            RecoveryHint::InvestigateInvariant { .. }
                | RecoveryHint::ReconcileAccounting
                | RecoveryHint::QuarantineNode
        )
    }
}

///////////////////////////////////////////////////////////////////////////////
// RECOVERY HINT
///////////////////////////////////////////////////////////////////////////////

/// Suggested recovery action for an error.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RecoveryHint {
    RetryImmediately,
    RetryWithDifferentCarrier,
    AbortAndRetry,
    WaitForIdle,
    UseDatasetScopedOrdering,
    ReconcileAccounting,
    AssignNewIdentity,
    NegotiateVersion,
    UseCarrierAgnosticEndpoint,
    QuarantineNode,
    InvestigateInvariant { invariant: &'static str },
    FailFast,
    EscalateToOperator,
}

///////////////////////////////////////////////////////////////////////////////
// DOMAIN ERROR TYPES
///////////////////////////////////////////////////////////////////////////////

macro_rules! domain_error {
    ($name:ident, $recovery:expr) => {
        #[derive(Debug, Clone, Error, PartialEq, Eq, Hash)]
        pub enum $name {
            #[error("internal {0}")]
            Internal(String),
            #[error("timeout")]
            Timeout,
            #[error("not found: {0}")]
            NotFound(String),
            #[error("already exists: {0}")]
            AlreadyExists(String),
            #[error("invalid argument: {0}")]
            InvalidArgument(String),
            #[error("resource exhausted: {0}")]
            ResourceExhausted(String),
            #[error("permission denied: {0}")]
            PermissionDenied(String),
            #[error("precondition failed: {0}")]
            PreconditionFailed(String),
            #[error("unavailable: {0}")]
            Unavailable(String),
            #[error("data corrupted: {0}")]
            DataCorrupted(String),
            #[error("unsupported operation: {0}")]
            Unsupported(String),
        }

        impl $name {
            pub fn recovery_hint(&self) -> RecoveryHint {
                $recovery
            }
        }
    };
}

domain_error!(CarrierError, RecoveryHint::RetryWithDifferentCarrier);
domain_error!(TransportError, RecoveryHint::RetryWithDifferentCarrier);
domain_error!(StoreError, RecoveryHint::AbortAndRetry);
domain_error!(SessionError, RecoveryHint::WaitForIdle);
domain_error!(RoutingError, RecoveryHint::RetryImmediately);
domain_error!(DiscoveryError, RecoveryHint::RetryImmediately);
domain_error!(MembershipError, RecoveryHint::RetryImmediately);
domain_error!(PlacementError, RecoveryHint::ReconcileAccounting);
domain_error!(ReplicationError, RecoveryHint::AbortAndRetry);
domain_error!(RecoveryError, RecoveryHint::EscalateToOperator);
domain_error!(ConsistencyError, RecoveryHint::ReconcileAccounting);
domain_error!(OrderingError, RecoveryHint::UseDatasetScopedOrdering);
domain_error!(SecurityError, RecoveryHint::QuarantineNode);
domain_error!(ClientError, RecoveryHint::AbortAndRetry);
domain_error!(FaultError, RecoveryHint::QuarantineNode);
domain_error!(BroadcastError, RecoveryHint::RetryImmediately);
domain_error!(QuorumError, RecoveryHint::RetryImmediately);
domain_error!(SnapshotError, RecoveryHint::AbortAndRetry);
domain_error!(ByzantineError, RecoveryHint::QuarantineNode);
domain_error!(LeaseError, RecoveryHint::RetryImmediately);
domain_error!(CRDTError, RecoveryHint::ReconcileAccounting);
domain_error!(ErasureError, RecoveryHint::AbortAndRetry);
domain_error!(SagaError, RecoveryHint::AbortAndRetry);
domain_error!(CoordinatorError, RecoveryHint::FailFast);
domain_error!(DurabilityError, RecoveryHint::ReconcileAccounting);
domain_error!(TopologyError, RecoveryHint::RetryImmediately);
domain_error!(IntegrationError, RecoveryHint::EscalateToOperator);
domain_error!(ProtocolError, RecoveryHint::NegotiateVersion);
domain_error!(WireError, RecoveryHint::RetryImmediately);
domain_error!(FramingError, RecoveryHint::RetryImmediately);
domain_error!(SequencingError, RecoveryHint::RetryImmediately);
domain_error!(AckError, RecoveryHint::RetryImmediately);
domain_error!(LifecycleError, RecoveryHint::AbortAndRetry);
domain_error!(DeletionError, RecoveryHint::AbortAndRetry);
domain_error!(FencingError, RecoveryHint::FailFast);
domain_error!(RosterError, RecoveryHint::RetryImmediately);
domain_error!(FallbackError, RecoveryHint::RetryImmediately);
domain_error!(DiversityError, RecoveryHint::ReconcileAccounting);
domain_error!(FanoutError, RecoveryHint::AbortAndRetry);
domain_error!(GraphError, RecoveryHint::ReconcileAccounting);
domain_error!(IntegrityError, RecoveryHint::ReconcileAccounting);
domain_error!(RotationError, RecoveryHint::WaitForIdle);
domain_error!(RetirementError, RecoveryHint::RetryImmediately);
domain_error!(TimeError, RecoveryHint::RetryImmediately);
domain_error!(CheckpointError, RecoveryHint::AbortAndRetry);
domain_error!(ChannelError, RecoveryHint::RetryImmediately);

///////////////////////////////////////////////////////////////////////////////
// SUPPORTING DATA TYPES
///////////////////////////////////////////////////////////////////////////////

/// Carrier type — identifies the physical/logical carrier for an endpoint.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum CarrierType {
    Direct,
    Mesh,
    StoreAndForward,
    Optical,
    Acoustic,
    Custom(u16),
}

/// Node identifier — stable internal identity (BLAKE3 of ed25519 pubkey).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NodeId(pub [u8; 32]);

impl NodeId {
    pub const ZERO: NodeId = NodeId([0u8; 32]);
    pub fn new(bytes: [u8; 32]) -> Self { Self(bytes) }
    pub fn is_zero(&self) -> bool { self.0 == [0u8; 32] }
}

/// Dataset identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct DatasetId(pub [u8; 16]);

impl DatasetId {
    pub const ZERO: DatasetId = DatasetId([0u8; 16]);
    pub fn new(bytes: [u8; 16]) -> Self { Self(bytes) }
}

/// Object identifier within a dataset.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ObjectID(pub [u8; 16]);

impl ObjectID {
    pub const ZERO: ObjectID = ObjectID([0u8; 16]);
    pub fn new(bytes: [u8; 16]) -> Self { Self(bytes) }
}

/// Session identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SessionID(pub [u8; 16]);

impl SessionID {
    pub const ZERO: SessionID = SessionID([0u8; 16]);
    pub fn new(bytes: [u8; 16]) -> Self { Self(bytes) }
}

/// Operation reference for binary-completion tracking.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct OpRef(pub u64);

/// Delete proposal identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct DeleteProposalId(pub u64);

/// Ordering domain for dataset-scoped ordering.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct OrderingDomain(pub u64);

/// Protocol version.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ProtocolVersion {
    pub major: u16,
    pub minor: u16,
    pub patch: u16,
}

/// Timestamp in milliseconds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TimestampMillis(pub u64);

/// Fragment identifier for fragmented messages.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FragmentId(pub u64);

/// Snapshot identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SnapshotId(pub [u8; 16]);

/// Epoch for membership changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Epoch(pub u64);

/// Saga identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SagaId(pub [u8; 16]);

/// Saga step identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct StepId(pub u64);

/// Transaction identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TransactionId(pub [u8; 16]);

/// Lease identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct LeaseId(pub [u8; 16]);

/// Fencing token for failover safety.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FencingToken(pub u64);

/// CRDT state identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CRDTState(pub [u8; 16]);

/// CRDT operation identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CRDTOp(pub u64);

/// CRDT delta identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CRDTDelta(pub [u8; 16]);

/// Message identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct MessageId(pub [u8; 16]);

/// Checksum value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Checksum(pub [u8; 32]);

/// Value stored in the system.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Value(pub Vec<u8>);

/// Message envelope.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Message {
    pub id: MessageId,
    pub payload: Value,
    pub timestamp: TimestampMillis,
}

/// Compute request for compute-to-data execution.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ComputeRequest {
    pub dataset_id: DatasetId,
    pub code_hash: Checksum,
    pub input: Value,
}

/// Compute result.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ComputeResult {
    pub output: Value,
    pub proof: RecoveryProof,
}

/// Compaction result.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CompactResult {
    pub reclaimed_bytes: u64,
    pub compacted_objects: u64,
}

/// Compute error.
#[derive(Debug, Clone, Error, PartialEq, Eq, Hash)]
pub enum ComputeError {
    #[error("compute timed out")]
    Timeout,
    #[error("code not found: {0}")]
    CodeNotFound(Checksum),
    #[error("execution failed: {0}")]
    ExecutionFailed(String),
}

/// Tier error.
#[derive(Debug, Clone, Error, PartialEq, Eq, Hash)]
pub enum TierError {
    #[error("tier transition not allowed: {0}")]
    NotAllowed(String),
    #[error("tier not found: {0}")]
    NotFound(String),
}

/// Mobility error.
#[derive(Debug, Clone, Error, PartialEq, Eq, Hash)]
pub enum MobilityError {
    #[error("migration failed: {0}")]
    MigrationFailed(String),
    #[error("handoff timed out")]
    HandoffTimeout,
}

/// Credential for authentication.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Credential(pub Vec<u8>);

/// Auth token.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AuthToken(pub Vec<u8>);

/// Key identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct KeyId(pub [u8; 16]);

/// Signature.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Signature(pub Vec<u8>);

/// Circuit state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CircuitState {
    Closed,
    Open,
    HalfOpen,
}

/// Degradation level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum DegradationLevel {
    None,
    Mild,
    Moderate,
    Severe,
    Critical,
}

/// Delivery semantics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DeliverySemantics {
    AtMostOnce,
    AtLeastOnce,
    ExactlyOnce,
}

/// Ack state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AckState {
    Pending,
    Sent,
    Received,
    Lost,
}

/// Circuit breaker state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CircuitBreakerState {
    Closed,
    Open,
    HalfOpen,
}

/// Version for wire format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Version(pub u16);

/// Capacity for resource tracking.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Capacity(pub u64);

/// Failure event.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FailureEvent {
    pub node: NodeId,
    pub timestamp: TimestampMillis,
    pub reason: String,
}

/// Recovery scope level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum RecoveryScopeLevel {
    Local,
    Regional,
    Global,
}

/// Lifecycle state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum LifecycleState {
    Pending,
    Active,
    Retiring,
    Retired,
    Failed,
}

/// Anomaly detected during operation.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Anomaly {
    pub kind: String,
    pub severity: DegradationLevel,
    pub at: TimestampMillis,
}

/// Roster change.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RosterChange {
    pub node: NodeId,
    pub added: bool,
    pub at: TimestampMillis,
}

/// Local transaction.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LocalTransaction {
    pub id: TransactionId,
    pub ops: Vec<OpRef>,
}

/// Recovery proof for durability verification.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RecoveryProof {
    pub nodes_involved: Vec<NodeId>,
    pub checksum: Checksum,
    pub at: TimestampMillis,
}

/// Announcement broadcast between nodes.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Announcement {
    pub from: NodeId,
    pub topic: String,
    pub payload: Value,
    pub at: TimestampMillis,
}

/// Change stream entry.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ChangeStream {
    pub dataset_id: DatasetId,
    pub position: OrderingPos,
    pub change: ChangeType,
}

/// Ordering position for change streams.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct OrderingPos(pub u64);

/// Type of change in a change stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ChangeType {
    Insert,
    Update,
    Delete,
}

/// Tier for storage classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Tier {
    Hot,
    Warm,
    Cold,
    Archived,
}

///////////////////////////////////////////////////////////////////////////////
// CONVENIENCE CONSTRUCTORS
///////////////////////////////////////////////////////////////////////////////

impl CarrierType {
    pub fn is_direct(&self) -> bool { matches!(self, CarrierType::Direct) }
}

impl Value {
    pub fn empty() -> Self { Self(Vec::new()) }
    pub fn len(&self) -> usize { self.0.len() }
    pub fn is_empty(&self) -> bool { self.0.is_empty() }
}

impl Message {
    pub fn new(id: MessageId, payload: Value, timestamp: TimestampMillis) -> Self {
        Self { id, payload, timestamp }
    }
}

///////////////////////////////////////////////////////////////////////////////
// ERROR CATEGORIZATION
///////////////////////////////////////////////////////////////////////////////

/// Category of an error for routing and triage decisions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ErrorCategory {
    Transport,
    Storage,
    Session,
    Routing,
    Discovery,
    Membership,
    Placement,
    Replication,
    Recovery,
    Consistency,
    Ordering,
    Security,
    Client,
    Fault,
    Broadcast,
    Quorum,
    Snapshot,
    Byzantine,
    Lease,
    CRDT,
    Erasure,
    Saga,
    Coordinator,
    Durability,
    Topology,
    Integration,
    Protocol,
    Wire,
    Framing,
    Sequencing,
    Ack,
    Invariant,
    Lifecycle,
    Deletion,
    Fencing,
    Roster,
    Fallback,
    Diversity,
    Fanout,
    Graph,
    Integrity,
    Rotation,
    Retirement,
    Time,
    Checkpoint,
    Channel,
    BinaryCompletion,
    DeadNodeIdentity,
    IdleRotation,
    GlobalMaster,
    OutEqIn,
    CoordinatedDeletion,
    Version,
    Carrier,
    Unknown,
}

/// Severity of an error for alerting and escalation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ErrorSeverity {
    Info,
    Warning,
    Error,
    Critical,
    Fatal,
}

/// Retry policy for an error.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RetryPolicy {
    NoRetry,
    Immediate,
    Backoff(u32),
    Never,
}

/// Classifies an `Error` into category, severity, and retry policy.
#[derive(Debug, Clone, Default)]
pub struct ErrorClassifier {
    pub classifications: u64,
    pub category_counts: [u64; 40],
    pub severity_counts: [u64; 5],
}

impl ErrorClassifier {
    pub fn new() -> Self {
        Self {
            classifications: 0,
            category_counts: [0; 40],
            severity_counts: [0; 5],
        }
    }

    pub fn classify(&mut self, error: &Error) -> (ErrorCategory, ErrorSeverity, RetryPolicy) {
        self.classifications += 1;
        let (cat, sev, retry) = self.classify_inner(error);
        self.category_counts[cat as usize] += 1;
        self.severity_counts[sev as usize] += 1;
        (cat, sev, retry)
    }

    fn classify_inner(&self, error: &Error) -> (ErrorCategory, ErrorSeverity, RetryPolicy) {
        match error {
            Error::CarrierUnavailable(..) => (ErrorCategory::Carrier, ErrorSeverity::Error, RetryPolicy::Backoff(3)),
            Error::Transport(..) => (ErrorCategory::Transport, ErrorSeverity::Error, RetryPolicy::Backoff(3)),
            Error::Store(..) => (ErrorCategory::Storage, ErrorSeverity::Critical, RetryPolicy::Backoff(5)),
            Error::Session(..) => (ErrorCategory::Session, ErrorSeverity::Error, RetryPolicy::Immediate),
            Error::Routing(..) => (ErrorCategory::Routing, ErrorSeverity::Error, RetryPolicy::Backoff(2)),
            Error::Discovery(..) => (ErrorCategory::Discovery, ErrorSeverity::Warning, RetryPolicy::Backoff(2)),
            Error::Membership(..) => (ErrorCategory::Membership, ErrorSeverity::Critical, RetryPolicy::NoRetry),
            Error::Security(..) => (ErrorCategory::Security, ErrorSeverity::Fatal, RetryPolicy::Never),
            Error::Byzantine(..) => (ErrorCategory::Byzantine, ErrorSeverity::Critical, RetryPolicy::Never),
            Error::InvariantViolated { .. } => (ErrorCategory::Invariant, ErrorSeverity::Critical, RetryPolicy::NoRetry),
            Error::VersionMismatch { .. } => (ErrorCategory::Version, ErrorSeverity::Error, RetryPolicy::Immediate),
            Error::DeadNodeIdentityRetired { .. } => (ErrorCategory::DeadNodeIdentity, ErrorSeverity::Warning, RetryPolicy::Immediate),
            Error::IdleRotationViolation { .. } => (ErrorCategory::IdleRotation, ErrorSeverity::Warning, RetryPolicy::NoRetry),
            Error::GlobalMasterRejected { .. } => (ErrorCategory::GlobalMaster, ErrorSeverity::Critical, RetryPolicy::NoRetry),
            Error::OutEqInViolation { .. } => (ErrorCategory::OutEqIn, ErrorSeverity::Critical, RetryPolicy::NoRetry),
            Error::CoordinatedDeletionFailed { .. } => (ErrorCategory::CoordinatedDeletion, ErrorSeverity::Critical, RetryPolicy::NoRetry),
            Error::BinaryCompletionRequired { .. } => (ErrorCategory::BinaryCompletion, ErrorSeverity::Error, RetryPolicy::NoRetry),
            Error::CarrierRejectedEndpoint { .. } => (ErrorCategory::Carrier, ErrorSeverity::Error, RetryPolicy::Immediate),
            _ => (ErrorCategory::Unknown, ErrorSeverity::Error, RetryPolicy::Backoff(1)),
        }
    }

    pub fn total_classifications(&self) -> u64 { self.classifications }
    pub fn category_count(&self, cat: ErrorCategory) -> u64 { self.category_counts[cat as usize] }
    pub fn severity_count(&self, sev: ErrorSeverity) -> u64 { self.severity_counts[sev as usize] }
    pub fn clear(&mut self) { self.classifications = 0; self.category_counts = [0; 40]; self.severity_counts = [0; 5]; }
}

///////////////////////////////////////////////////////////////////////////////
// ERROR CONTEXT
///////////////////////////////////////////////////////////////////////////////

/// Additional context attached to an error for debugging and correlation.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ErrorContext {
    pub node_id: Option<NodeId>,
    pub dataset_id: Option<DatasetId>,
    pub object_id: Option<ObjectID>,
    pub session_id: Option<SessionID>,
    pub op_ref: Option<OpRef>,
    pub timestamp: Option<TimestampMillis>,
    pub carrier: Option<CarrierType>,
    pub custom: Vec<(String, String)>,
}

impl ErrorContext {
    pub fn new() -> Self {
        Self {
            node_id: None,
            dataset_id: None,
            object_id: None,
            session_id: None,
            op_ref: None,
            timestamp: None,
            carrier: None,
            custom: Vec::new(),
        }
    }

    pub fn with_node(mut self, node: NodeId) -> Self {
        self.node_id = Some(node);
        self
    }

    pub fn with_dataset(mut self, ds: DatasetId) -> Self {
        self.dataset_id = Some(ds);
        self
    }

    pub fn with_object(mut self, obj: ObjectID) -> Self {
        self.object_id = Some(obj);
        self
    }

    pub fn with_session(mut self, sess: SessionID) -> Self {
        self.session_id = Some(sess);
        self
    }

    pub fn with_op(mut self, op: OpRef) -> Self {
        self.op_ref = Some(op);
        self
    }

    pub fn with_timestamp(mut self, ts: TimestampMillis) -> Self {
        self.timestamp = Some(ts);
        self
    }

    pub fn with_carrier(mut self, c: CarrierType) -> Self {
        self.carrier = Some(c);
        self
    }

    pub fn with_custom(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.custom.push((key.into(), value.into()));
        self
    }

    pub fn custom_count(&self) -> usize {
        self.custom.len()
    }

    pub fn custom_get(&self, key: &str) -> Option<&str> {
        self.custom.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str())
    }

    pub fn clear_custom(&mut self) {
        self.custom.clear();
    }

    pub fn has_context(&self) -> bool {
        self.node_id.is_some()
            || self.dataset_id.is_some()
            || self.object_id.is_some()
            || self.session_id.is_some()
            || self.op_ref.is_some()
            || self.timestamp.is_some()
            || self.carrier.is_some()
            || !self.custom.is_empty()
    }
}

///////////////////////////////////////////////////////////////////////////////
// ERROR CHAIN
///////////////////////////////////////////////////////////////////////////////

/// A link in an error chain: cause + context.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ErrorChainLink {
    pub error: Error,
    pub context: ErrorContext,
    pub at: TimestampMillis,
}

/// Error chain: ordered list of causal links.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct ErrorChain {
    pub links: Vec<ErrorChainLink>,
    pub max_links: usize,
}

impl ErrorChain {
    pub fn new() -> Self {
        Self {
            links: Vec::new(),
            max_links: 1000,
        }
    }

    pub fn push(&mut self, error: Error, context: ErrorContext, at: TimestampMillis) {
        if self.links.len() >= self.max_links {
            self.links.remove(0);
        }
        self.links.push(ErrorChainLink { error, context, at });
    }

    pub fn link_count(&self) -> usize {
        self.links.len()
    }

    pub fn root_cause(&self) -> Option<&ErrorChainLink> {
        self.links.first()
    }

    pub fn latest(&self) -> Option<&ErrorChainLink> {
        self.links.last()
    }

    pub fn links_for_node(&self, node: NodeId) -> Vec<&ErrorChainLink> {
        self.links
            .iter()
            .filter(|l| l.context.node_id == Some(node))
            .collect()
    }

    pub fn clear(&mut self) {
        self.links.clear();
    }

    pub fn set_max_links(&mut self, max: usize) {
        self.max_links = max;
        while self.links.len() > self.max_links {
            self.links.remove(0);
        }
    }

    pub fn find_by_category(&self, cat: ErrorCategory) -> Vec<&ErrorChainLink> {
        self.links
            .iter()
            .filter(|l| matches!(l.error.recovery_hint(), RecoveryHint::RetryImmediately | RecoveryHint::AbortAndRetry | RecoveryHint::RetryWithDifferentCarrier | RecoveryHint::WaitForIdle | RecoveryHint::UseDatasetScopedOrdering | RecoveryHint::ReconcileAccounting | RecoveryHint::AssignNewIdentity | RecoveryHint::NegotiateVersion | RecoveryHint::UseCarrierAgnosticEndpoint | RecoveryHint::QuarantineNode | RecoveryHint::InvestigateInvariant { .. } | RecoveryHint::FailFast | RecoveryHint::EscalateToOperator))
            .filter(|_| true)
            .collect()
    }
}

///////////////////////////////////////////////////////////////////////////////
// ERROR FACTORY
///////////////////////////////////////////////////////////////////////////////

/// Factory for constructing common IDS errors with consistent context.
#[derive(Debug, Clone, Default)]
pub struct ErrorFactory {
    pub default_node: Option<NodeId>,
    pub default_dataset: Option<DatasetId>,
    pub error_count: u64,
}

impl ErrorFactory {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_defaults(mut self, node: NodeId, dataset: DatasetId) -> Self {
        self.default_node = Some(node);
        self.default_dataset = Some(dataset);
        self
    }

    pub fn carrier_unavailable(&mut self, carrier: CarrierType, msg: impl Into<String>) -> Error {
        self.error_count += 1;
        Error::CarrierUnavailable(carrier, msg.into())
    }

    pub fn store_not_found(&mut self, object: ObjectID) -> Error {
        self.error_count += 1;
        Error::Store(object, StoreError::NotFound("object not found".into()))
    }

    pub fn session_timeout(&mut self, session: SessionID) -> Error {
        self.error_count += 1;
        Error::Session(session, SessionError::Timeout)
    }

    pub fn routing_no_route(&mut self) -> Error {
        self.error_count += 1;
        Error::Routing(RoutingError::Unavailable("no route available".into()))
    }

    pub fn invariant_violated(&mut self, invariant: &'static str) -> Error {
        self.error_count += 1;
        Error::InvariantViolated { invariant }
    }

    pub fn version_mismatch(&mut self, expected: ProtocolVersion, got: ProtocolVersion) -> Error {
        self.error_count += 1;
        Error::VersionMismatch { expected, got }
    }

    pub fn dead_node_retired(&mut self, node: NodeId) -> Error {
        self.error_count += 1;
        Error::DeadNodeIdentityRetired { node }
    }

    pub fn out_eq_in_violation(&mut self, message: impl Into<String>) -> Error {
        self.error_count += 1;
        Error::OutEqInViolation { message: message.into() }
    }

    pub fn error_count(&self) -> u64 {
        self.error_count
    }

    pub fn reset_count(&mut self) {
        self.error_count = 0;
    }
}

///////////////////////////////////////////////////////////////////////////////
// ERROR VALIDATOR
///////////////////////////////////////////////////////////////////////////////

/// Validates that an error is well-formed and actionable.
#[derive(Debug, Clone, Default)]
pub struct ErrorValidator {
    pub validations: u64,
    pub passed: u64,
    pub failed: u64,
}

impl ErrorValidator {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn validate(&mut self, error: &Error) -> bool {
        self.validations += 1;
        let ok = self.validate_inner(error);
        if ok {
            self.passed += 1;
        } else {
            self.failed += 1;
        }
        ok
    }

    fn validate_inner(&self, error: &Error) -> bool {
        match error {
            Error::CarrierUnavailable(_, msg) => !msg.is_empty(),
            Error::InvariantViolated { invariant } => !invariant.is_empty(),
            Error::VersionMismatch { expected, got } => expected != got,
            Error::OutEqInViolation { message } => !message.is_empty(),
            _ => true,
        }
    }

    pub fn total_validations(&self) -> u64 {
        self.validations
    }

    pub fn passed_count(&self) -> u64 {
        self.passed
    }

    pub fn failed_count(&self) -> u64 {
        self.failed
    }

    pub fn pass_rate_percent(&self) -> f64 {
        if self.validations == 0 {
            return 100.0;
        }
        self.passed as f64 / self.validations as f64 * 100.0
    }

    pub fn clear(&mut self) {
        self.validations = 0;
        self.passed = 0;
        self.failed = 0;
    }
}

///////////////////////////////////////////////////////////////////////////////
// ERROR AGGREGATOR
///////////////////////////////////////////////////////////////////////////////

/// Aggregates errors by category over a time window.
#[derive(Debug, Clone, Default)]
pub struct ErrorAggregator {
    pub by_category: std::collections::BTreeMap<ErrorCategory, u64>,
    pub total: u64,
    pub window_start: Option<TimestampMillis>,
}

impl ErrorAggregator {
    pub fn new() -> Self {
        Self {
            by_category: std::collections::BTreeMap::new(),
            total: 0,
            window_start: None,
        }
    }

    pub fn record(&mut self, error: &Error, classifier: &ErrorClassifier) {
        let (cat, _, _) = classifier.classify(error);
        *self.by_category.entry(cat).or_insert(0) += 1;
        self.total += 1;
    }

    pub fn total_errors(&self) -> u64 {
        self.total
    }

    pub fn count_for_category(&self, cat: ErrorCategory) -> u64 {
        self.by_category.get(&cat).copied().unwrap_or(0)
    }

    pub fn top_category(&self) -> Option<(ErrorCategory, u64)> {
        self.by_category
            .iter()
            .max_by_key(|(_, v)| *v)
            .map(|(k, v)| (*k, *v))
    }

    pub fn clear(&mut self) {
        self.by_category.clear();
        self.total = 0;
    }
}

///////////////////////////////////////////////////////////////////////////////
// ERROR SAMPLER
///////////////////////////////////////////////////////////////////////////////

/// Samples errors for diagnostic tracing: keeps a bounded sample of each category.
#[derive(Debug, Clone, Default)]
pub struct ErrorSampler {
    pub samples: std::collections::BTreeMap<ErrorCategory, Vec<Error>>,
    pub max_per_category: usize,
    pub total_sampled: u64,
    pub total_dropped: u64,
}

impl ErrorSampler {
    pub fn new() -> Self {
        Self {
            samples: std::collections::BTreeMap::new(),
            max_per_category: 100,
            total_sampled: 0,
            total_dropped: 0,
        }
    }

    pub fn sample(&mut self, cat: ErrorCategory, error: Error) -> bool {
        let entry = self.samples.entry(cat).or_default();
        if entry.len() >= self.max_per_category {
            self.total_dropped += 1;
            false
        } else {
            entry.push(error);
            self.total_sampled += 1;
            true
        }
    }

    pub fn total_sampled(&self) -> u64 {
        self.total_sampled
    }

    pub fn total_dropped(&self) -> u64 {
        self.total_dropped
    }

    pub fn samples_for(&self, cat: ErrorCategory) -> usize {
        self.samples.get(&cat).map(|v| v.len()).unwrap_or(0)
    }

    pub fn clear(&mut self) {
        self.samples.clear();
        self.total_sampled = 0;
        self.total_dropped = 0;
    }

    pub fn set_max_per_category(&mut self, max: usize) {
        self.max_per_category = max;
        for (_, v) in self.samples.iter_mut() {
            while v.len() > max {
                v.remove(0);
            }
        }
    }
}

///////////////////////////////////////////////////////////////////////////////
// ERROR CIRCUIT BREAKER
///////////////////////////////////////////////////////////////////////////////

/// Circuit breaker for error-prone operations: opens after threshold, half-open after cooldown.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ErrorCircuitBreaker {
    pub failure_threshold: u64,
    pub failures: u64,
    pub successes: u64,
    pub state: CircuitState,
    pub trips: u64,
}

impl ErrorCircuitBreaker {
    pub fn new() -> Self {
        Self {
            failure_threshold: 5,
            failures: 0,
            successes: 0,
            state: CircuitState::Closed,
            trips: 0,
        }
    }

    pub fn record_failure(&mut self) {
        self.failures += 1;
        if self.failures >= self.failure_threshold && self.state == CircuitState::Closed {
            self.state = CircuitState::Open;
            self.trips += 1;
        }
    }

    pub fn record_success(&mut self) {
        self.successes += 1;
        if self.state == CircuitState::HalfOpen {
            self.state = CircuitState::Closed;
            self.failures = 0;
        }
    }

    pub fn try_half_open(&mut self) -> bool {
        if self.state == CircuitState::Open {
            self.state = CircuitState::HalfOpen;
            self.failures = 0;
            true
        } else {
            false
        }
    }

    pub fn state(&self) -> CircuitState {
        self.state
    }

    pub fn trips(&self) -> u64 {
        self.trips
    }

    pub fn reset(&mut self) {
        self.failures = 0;
        self.successes = 0;
        self.state = CircuitState::Closed;
    }

    pub fn set_threshold(&mut self, thresh: u64) {
        self.failure_threshold = thresh;
    }
}

///////////////////////////////////////////////////////////////////////////////
// ERROR PROBE
///////////////////////////////////////////////////////////////////////////////

/// Creates a sample error of each category for testing.
#[derive(Debug, Clone, Default)]
pub struct ErrorProbe {
    pub created: u64,
}

impl ErrorProbe {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn transport_err(&mut self) -> Error {
        self.created += 1;
        Error::Transport(CarrierType::Direct, TransportError::Timeout)
    }

    pub fn security_err(&mut self) -> Error {
        self.created += 1;
        Error::Security(SecurityError::PermissionDenied("denied".into()))
    }

    pub fn invariant_err(&mut self) -> Error {
        self.created += 1;
        Error::InvariantViolated { invariant: "probe" }
    }

    pub fn created_count(&self) -> u64 {
        self.created
    }
}

///////////////////////////////////////////////////////////////////////////////
// ERROR FINGERPRINTING
///////////////////////////////////////////////////////////////////////////////

/// A fingerprint for an error: stable hash of its class and key fields,
/// used for deduplication and correlation across nodes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ErrorFingerprint(pub u64);

/// Computes fingerprints for errors to detect recurring failures.
#[derive(Debug, Clone, Default)]
pub struct ErrorFingerprinter {
    pub computed: u64,
    pub cache: std::collections::BTreeMap<ErrorFingerprint, u64>,
    pub max_cache: usize,
}

impl ErrorFingerprinter {
    pub fn new() -> Self {
        Self {
            computed: 0,
            cache: std::collections::BTreeMap::new(),
            max_cache: 10000,
        }
    }

    /// Compute a fingerprint for an error.
    pub fn fingerprint(&mut self, error: &Error) -> ErrorFingerprint {
        self.computed += 1;
        let fp = self.hash_error(error);
        *self.cache.entry(fp).or_insert(0) += 1;
        if self.cache.len() > self.max_cache {
            if let Some((k, _)) = self.cache.iter().next().cloned() {
                self.cache.remove(&k);
            }
        }
        fp
    }

    fn hash_error(&self, error: &Error) -> ErrorFingerprint {
        let mut h: u64 = 0xcbf29ce484222325;
        let bytes = match error {
            Error::CarrierUnavailable(c, m) => {
                format!("CarrierUnavailable:{c:?}:{m}").into_bytes()
            }
            Error::Transport(c, _) => format!("Transport:{c:?}").into_bytes(),
            Error::Store(_, _) => "Store".into_bytes(),
            Error::Session(_, _) => "Session".into_bytes(),
            Error::Routing(_) => "Routing".into_bytes(),
            Error::Discovery(_) => "Discovery".into_bytes(),
            Error::Membership(_) => "Membership".into_bytes(),
            Error::Placement(_) => "Placement".into_bytes(),
            Error::Replication(_) => "Replication".into_bytes(),
            Error::Recovery(_) => "Recovery".into_bytes(),
            Error::Consistency(_) => "Consistency".into_bytes(),
            Error::Ordering(_) => "Ordering".into_bytes(),
            Error::Security(_) => "Security".into_bytes(),
            Error::Client(_) => "Client".into_bytes(),
            Error::Fault(_) => "Fault".into_bytes(),
            Error::Broadcast(_) => "Broadcast".into_bytes(),
            Error::Quorum(_) => "Quorum".into_bytes(),
            Error::Snapshot(_) => "Snapshot".into_bytes(),
            Error::Byzantine(_) => "Byzantine".into_bytes(),
            Error::Lease(_) => "Lease".into_bytes(),
            Error::CRDT(_) => "CRDT".into_bytes(),
            Error::Erasure(_) => "Erasure".into_bytes(),
            Error::Saga(_) => "Saga".into_bytes(),
            Error::Coordinator(_) => "Coordinator".into_bytes(),
            Error::Durability(_) => "Durability".into_bytes(),
            Error::Topology(_) => "Topology".into_bytes(),
            Error::Integration(_) => "Integration".into_bytes(),
            Error::Protocol(_) => "Protocol".into_bytes(),
            Error::Wire(_) => "Wire".into_bytes(),
            Error::Framing(_) => "Framing".into_bytes(),
            Error::Sequencing(_) => "Sequencing".into_bytes(),
            Error::Ack(_) => "Ack".into_bytes(),
            Error::InvariantViolated { invariant } => {
                format!("InvariantViolated:{invariant}").into_bytes()
            }
            Error::Lifecycle(_) => "Lifecycle".into_bytes(),
            Error::Deletion(_) => "Deletion".into_bytes(),
            Error::Fencing(_) => "Fencing".into_bytes(),
            Error::Roster(_) => "Roster".into_bytes(),
            Error::Fallback(_) => "Fallback".into_bytes(),
            Error::Diversity(_) => "Diversity".into_bytes(),
            Error::Fanout(_) => "Fanout".into_bytes(),
            Error::Graph(_) => "Graph".into_bytes(),
            Error::Integrity(_) => "Integrity".into_bytes(),
            Error::Rotation(_) => "Rotation".into_bytes(),
            Error::Retirement(_) => "Retirement".into_bytes(),
            Error::Time(_) => "Time".into_bytes(),
            Error::Checkpoint(_) => "Checkpoint".into_bytes(),
            Error::Channel(_) => "Channel".into_bytes(),
            Error::BinaryCompletionRequired { op } => {
                format!("BinaryCompletionRequired:{op:?}").into_bytes()
            }
            Error::DeadNodeIdentityRetired { node } => {
                format!("DeadNodeIdentityRetired:{node:?}").into_bytes()
            }
            Error::IdleRotationViolation { node } => {
                format!("IdleRotationViolation:{node:?}").into_bytes()
            }
            Error::GlobalMasterRejected { domain } => {
                format!("GlobalMasterRejected:{domain:?}").into_bytes()
            }
            Error::OutEqInViolation { message } => {
                format!("OutEqInViolation:{message}").into_bytes()
            }
            Error::CoordinatedDeletionFailed { proposal } => {
                format!("CoordinatedDeletionFailed:{proposal:?}").into_bytes()
            }
            Error::VersionMismatch { expected, got } => {
                format!("VersionMismatch:{expected:?}:{got:?}").into_bytes()
            }
            Error::CarrierRejectedEndpoint { carrier } => {
                format!("CarrierRejectedEndpoint:{carrier:?}").into_bytes()
            }
        };
        for b in bytes {
            h ^= b as u64;
            h = h.wrapping_mul(0x100000001b3);
        }
        ErrorFingerprint(h)
    }

    /// Total fingerprints computed.
    pub fn total_computed(&self) -> u64 {
        self.computed
    }

    /// Count for a fingerprint.
    pub fn count_for(&self, fp: ErrorFingerprint) -> u64 {
        self.cache.get(&fp).copied().unwrap_or(0)
    }

    /// Distinct fingerprints seen.
    pub fn distinct_count(&self) -> usize {
        self.cache.len()
    }

    /// Clear.
    pub fn clear(&mut self) {
        self.computed = 0;
        self.cache.clear();
    }
}

///////////////////////////////////////////////////////////////////////////////
// ERROR DEAD LETTER QUEUE
///////////////////////////////////////////////////////////////////////////////

/// A dead-letter queue for errors that cannot be recovered automatically.
#[derive(Debug, Clone, Default)]
pub struct ErrorDeadLetterQueue {
    pub entries: Vec<DeadLetterEntry>,
    pub max_entries: usize,
    pub total_queued: u64,
    pub total_dropped: u64,
}

/// A single dead-letter entry.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DeadLetterEntry {
    pub entry_id: [u8; 16],
    pub error: Error,
    pub context: ErrorContext,
    pub queued_at: TimestampMillis,
    pub retry_count: u32,
    pub last_retry_at: Option<TimestampMillis>,
}

impl ErrorDeadLetterQueue {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            max_entries: 5000,
            total_queued: 0,
            total_dropped: 0,
        }
    }

    /// Queue an error for later manual inspection.
    pub fn enqueue(
        &mut self,
        error: Error,
        context: ErrorContext,
        queued_at: TimestampMillis,
    ) {
        if self.entries.len() >= self.max_entries {
            self.total_dropped += 1;
            self.entries.remove(0);
        }
        self.entries.push(DeadLetterEntry {
            entry_id: Self::next_id(),
            error,
            context,
            queued_at,
            retry_count: 0,
            last_retry_at: None,
        });
        self.total_queued += 1;
    }

    fn next_id() -> [u8; 16] {
        let mut id = [0u8; 16];
        id[0] = 0x44; // 'D'
        id[1] = 0x4c; // 'L'
        id
    }

    /// Total entries.
    pub fn entry_count(&self) -> usize {
        self.entries.len()
    }

    /// Total queued.
    pub fn total_queued(&self) -> u64 {
        self.total_queued
    }

    /// Total dropped due to capacity.
    pub fn total_dropped(&self) -> u64 {
        self.total_dropped
    }

    /// Entries for a node.
    pub fn entries_for_node(&self, node: NodeId) -> Vec<&DeadLetterEntry> {
        self.entries
            .iter()
            .filter(|e| e.context.node_id == Some(node))
            .collect()
    }

    /// Clear.
    pub fn clear(&mut self) {
        self.entries.clear();
        self.total_queued = 0;
        self.total_dropped = 0;
    }

    /// Set max entries.
    pub fn set_max_entries(&mut self, max: usize) {
        self.max_entries = max;
        while self.entries.len() > max {
            self.entries.remove(0);
        }
    }
}

///////////////////////////////////////////////////////////////////////////////
// ERROR BACKOFF SCHEDULER
///////////////////////////////////////////////////////////////////////////////

/// Computes exponential backoff delays for retryable errors.
#[derive(Debug, Clone, Default)]
pub struct ErrorBackoffScheduler {
    pub base_ms: u64,
    pub max_ms: u64,
    pub max_retries: u32,
    pub multiplier: f64,
    pub jitter: bool,
}

impl ErrorBackoffScheduler {
    pub fn new() -> Self {
        Self {
            base_ms: 100,
            max_ms: 60_000,
            max_retries: 10,
            multiplier: 2.0,
            jitter: true,
        }
    }

    /// Compute the backoff delay in ms for retry number `attempt` (0-based).
    pub fn backoff_ms(&self, attempt: u32) -> u64 {
        if attempt >= self.max_retries {
            return self.max_ms;
        }
        let delay = (self.base_ms as f64)
            * self.multiplier.powi(attempt as i32);
        let delay = delay.min(self.max_ms as f64) as u64;
        if self.jitter {
            // Deterministic jitter: use attempt to vary within ±25%
            let jitter_factor = 1.0 + ((attempt as f64 * 0.37) % 1.0) * 0.5;
            (delay as f64 * jitter_factor) as u64
        } else {
            delay
        }
    }

    /// Whether more retries are allowed.
    pub fn can_retry(&self, attempt: u32) -> bool {
        attempt < self.max_retries
    }

    /// Total configured max retries.
    pub fn max_retries(&self) -> u32 {
        self.max_retries
    }

    /// Set parameters.
    pub fn set_params(&mut self, base_ms: u64, max_ms: u64, max_retries: u32) {
        self.base_ms = base_ms;
        self.max_ms = max_ms;
        self.max_retries = max_retries;
    }
}

///////////////////////////////////////////////////////////////////////////////
// ERROR CORRELATOR
///////////////////////////////////////////////////////////////////////////////

/// Correlates errors across nodes by fingerprint and time window.
#[derive(Debug, Clone, Default)]
pub struct ErrorCorrelator {
    pub windows: std::collections::BTreeMap<u64, Vec<ErrorFingerprint>>,
    pub window_ms: u64,
    pub correlations: u64,
}

impl ErrorCorrelator {
    pub fn new() -> Self {
        Self {
            windows: std::collections::BTreeMap::new(),
            window_ms: 60_000,
            correlations: 0,
        }
    }

    /// Record an error fingerprint at a timestamp.
    pub fn record(&mut self, fp: ErrorFingerprint, at: TimestampMillis) {
        let window_key = at.0 / self.window_ms;
        self.windows.entry(window_key).or_default().push(fp);
    }

    /// Count errors with the same fingerprint in the current window.
    pub fn count_in_window(&self, fp: ErrorFingerprint, at: TimestampMillis) -> u64 {
        let window_key = at.0 / self.window_ms;
        self.windows
            .get(&window_key)
            .map(|v| v.iter().filter(|&&f| f == fp).count() as u64)
            .unwrap_or(0)
    }

    /// Whether a fingerprint exceeds the burst threshold in the window.
    pub fn is_burst(&self, fp: ErrorFingerprint, at: TimestampMillis, threshold: u64) -> bool {
        self.count_in_window(fp, at) >= threshold
    }

    /// Total correlation checks performed.
    pub fn total_correlations(&self) -> u64 {
        self.correlations
    }

    /// Clear windows older than `at`.
    pub fn prune_older_than(&mut self, at: TimestampMillis) {
        let cutoff = at.0 / self.window_ms;
        self.windows.retain(|k, _| *k >= cutoff);
    }

    /// Set window size.
    pub fn set_window_ms(&mut self, ms: u64) {
        self.window_ms = ms;
    }
}

///////////////////////////////////////////////////////////////////////////////
// ERROR RATE LIMITER
///////////////////////////////////////////////////////////////////////////////

/// Limits the rate of error emissions to prevent log flooding.
#[derive(Debug, Clone, Default)]
pub struct ErrorRateLimiter {
    pub max_per_window: u64,
    pub window_ms: u64,
    pub emissions: std::collections::BTreeMap<u64, u64>,
    pub total_emitted: u64,
    pub total_limited: u64,
}

impl ErrorRateLimiter {
    pub fn new() -> Self {
        Self {
            max_per_window: 100,
            window_ms: 1000,
            emissions: std::collections::BTreeMap::new(),
            total_emitted: 0,
            total_limited: 0,
        }
    }

    /// Try to emit an error. Returns true if allowed, false if rate-limited.
    pub fn try_emit(&mut self, at: TimestampMillis) -> bool {
        let window_key = at.0 / self.window_ms;
        let count = self.emissions.entry(window_key).or_insert(0);
        if *count >= self.max_per_window {
            self.total_limited += 1;
            return false;
        }
        *count += 1;
        self.total_emitted += 1;
        true
    }

    /// Total emitted.
    pub fn total_emitted(&self) -> u64 {
        self.total_emitted
    }

    /// Total limited.
    pub fn total_limited(&self) -> u64 {
        self.total_limited
    }

    /// Clear windows older than `at`.
    pub fn prune_older_than(&mut self, at: TimestampMillis) {
        let cutoff = at.0 / self.window_ms;
        self.emissions.retain(|k, _| *k >= cutoff);
    }

    /// Set limits.
    pub fn set_limits(&mut self, max_per_window: u64, window_ms: u64) {
        self.max_per_window = max_per_window;
        self.window_ms = window_ms;
    }
}

///////////////////////////////////////////////////////////////////////////////
// ERROR EXPORTER
///////////////////////////////////////////////////////////////////////////////

/// Exports errors to a serializable form for external observability.
#[derive(Debug, Clone, Default)]
pub struct ErrorExporter {
    pub exported: u64,
}

impl ErrorExporter {
    pub fn new() -> Self {
        Self::default()
    }

    /// Export an error to a string representation.
    pub fn export(&mut self, error: &Error) -> String {
        self.exported += 1;
        format!("{error}")
    }

    /// Export with context.
    pub fn export_with_context(
        &mut self,
        error: &Error,
        context: &ErrorContext,
    ) -> String {
        self.exported += 1;
        let mut s = format!("{error}");
        if let Some(node) = context.node_id {
            s.push_str(&format!(" node={node:?}"));
        }
        if let Some(ds) = context.dataset_id {
            s.push_str(&format!(" dataset={ds:?}"));
        }
        if let Some(ts) = context.timestamp {
            s.push_str(&format!(" ts={:?}", ts.0));
        }
        s
    }

    /// Total exported.
    pub fn total_exported(&self) -> u64 {
        self.exported
    }

    /// Reset count.
    pub fn reset(&mut self) {
        self.exported = 0;
    }
}

///////////////////////////////////////////////////////////////////////////////
// ERROR SUMMARY
///////////////////////////////////////////////////////////////////////////////

/// Aggregated summary of error statistics for observability.
#[derive(Debug, Clone, Default)]
pub struct ErrorSummary {
    pub total_errors: u64,
    pub total_transient: u64,
    pub total_operator: u64,
    pub by_severity: std::collections::BTreeMap<ErrorSeverity, u64>,
    pub by_category: std::collections::BTreeMap<ErrorCategory, u64>,
}

impl ErrorSummary {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record an error into the summary using a classifier.
    pub fn record(&mut self, error: &Error, classifier: &ErrorClassifier) {
        self.total_errors += 1;
        if error.is_transient() {
            self.total_transient += 1;
        }
        if error.requires_operator() {
            self.total_operator += 1;
        }
        let (cat, sev, _) = classifier.classify(error);
        *self.by_severity.entry(sev).or_insert(0) += 1;
        *self.by_category.entry(cat).or_insert(0) += 1;
    }

    /// Total errors recorded.
    pub fn total_errors(&self) -> u64 {
        self.total_errors
    }

    /// Total transient errors.
    pub fn total_transient(&self) -> u64 {
        self.total_transient
    }

    /// Total operator-required errors.
    pub fn total_operator(&self) -> u64 {
        self.total_operator
    }

    /// Severity distribution.
    pub fn severity_distribution(&self) -> Vec<(ErrorSeverity, u64)> {
        let mut v: Vec<(ErrorSeverity, u64)> = self.by_severity.iter().map(|(k, v)| (*k, *v)).collect();
        v.sort_by(|a, b| b.1.cmp(&a.1));
        v
    }

    /// Category distribution.
    pub fn category_distribution(&self) -> Vec<(ErrorCategory, u64)> {
        let mut v: Vec<(ErrorCategory, u64)> = self.by_category.iter().map(|(k, v)| (*k, *v)).collect();
        v.sort_by(|a, b| b.1.cmp(&a.1));
        v
    }

    /// Top category by count.
    pub fn top_category(&self) -> Option<(ErrorCategory, u64)> {
        self.by_category.iter().max_by_key(|(_, v)| *v).map(|(k, v)| (*k, *v))
    }

    /// Clear.
    pub fn clear(&mut self) {
        self.total_errors = 0;
        self.total_transient = 0;
        self.total_operator = 0;
        self.by_severity.clear();
        self.by_category.clear();
    }
}

///////////////////////////////////////////////////////////////////////////////
// ERROR METRICS REGISTRY
///////////////////////////////////////////////////////////////////////////////

/// Registry of named error metrics counters.
#[derive(Debug, Clone, Default)]
pub struct ErrorMetricsRegistry {
    pub counters: std::collections::BTreeMap<String, u64>,
    pub gauges: std::collections::BTreeMap<String, f64>,
}

impl ErrorMetricsRegistry {
    pub fn new() -> Self {
        Self {
            counters: std::collections::BTreeMap::new(),
            gauges: std::collections::BTreeMap::new(),
        }
    }

    /// Increment a counter.
    pub fn inc(&mut self, name: impl Into<String>) {
        let name = name.into();
        *self.counters.entry(name).or_insert(0) += 1;
    }

    /// Set a gauge value.
    pub fn set_gauge(&mut self, name: impl Into<String>, value: f64) {
        self.gauges.insert(name.into(), value);
    }

    /// Get a counter value.
    pub fn counter(&self, name: &str) -> u64 {
        self.counters.get(name).copied().unwrap_or(0)
    }

    /// Get a gauge value.
    pub fn gauge(&self, name: &str) -> f64 {
        self.gauges.get(name).copied().unwrap_or(0.0)
    }

    /// Total counters.
    pub fn counter_count(&self) -> usize {
        self.counters.len()
    }

    /// Total gauges.
    pub fn gauge_count(&self) -> usize {
        self.gauges.len()
    }

    /// Clear all metrics.
    pub fn clear(&mut self) {
        self.counters.clear();
        self.gauges.clear();
    }
}

///////////////////////////////////////////////////////////////////////////////
// ERROR ROUTER
///////////////////////////////////////////////////////////////////////////////

/// Routes errors to handlers based on category and severity.
#[derive(Debug, Clone, Default)]
pub struct ErrorRouter {
    pub routes: std::collections::BTreeMap<(ErrorCategory, ErrorSeverity), String>,
    pub default_handler: String,
    pub routed: u64,
}

impl ErrorRouter {
    pub fn new() -> Self {
        Self {
            routes: std::collections::BTreeMap::new(),
            default_handler: "default".into(),
            routed: 0,
        }
    }

    /// Register a route: category+severity -> handler name.
    pub fn route(&mut self, cat: ErrorCategory, sev: ErrorSeverity, handler: impl Into<String>) {
        self.routes.insert((cat, sev), handler.into());
    }

    /// Route an error to a handler name.
    pub fn route_error(
        &mut self,
        error: &Error,
        classifier: &ErrorClassifier,
    ) -> String {
        self.routed += 1;
        let (cat, sev, _) = classifier.classify(error);
        self.routes.get(&(cat, sev)).cloned().unwrap_or_else(|| self.default_handler.clone())
    }

    /// Total routed.
    pub fn total_routed(&self) -> u64 {
        self.routed
    }

    /// Set default handler.
    pub fn set_default_handler(&mut self, handler: impl Into<String>) {
        self.default_handler = handler.into();
    }

    /// Clear routes.
    pub fn clear_routes(&mut self) {
        self.routes.clear();
        self.routed = 0;
    }

    /// Whether a route is registered for category+severity.
    pub fn has_route(&self, cat: ErrorCategory, sev: ErrorSeverity) -> bool {
        self.routes.contains_key(&(cat, sev))
    }

    /// Total registered routes.
    pub fn route_count(&self) -> usize {
        self.routes.len()
    }

    /// Default handler name.
    pub fn default_handler(&self) -> &str {
        &self.default_handler
    }

    /// Remove a route for category+severity.
    pub fn remove_route(&mut self, cat: ErrorCategory, sev: ErrorSeverity) -> Option<String> {
        self.routes.remove(&(cat, sev))
    }

    /// Get a registered handler for category+severity.
    pub fn handler_for(
        &self,
        cat: ErrorCategory,
        sev: ErrorSeverity,
    ) -> Option<&String> {
        self.routes.get(&(cat, sev))
    }

    /// Iterate all registered routes.
    pub fn iter_routes(&self) -> impl Iterator<Item = ((ErrorCategory, ErrorSeverity), &String)> {
        self.routes.iter()
    }

    /// Total routes registered.
    pub fn total_routes(&self) -> u64 {
        self.route_count() as u64
    }

    /// Register route with default fallback flag.
    pub fn route_with_fallback(
        &mut self,
        cat: ErrorCategory,
        sev: ErrorSeverity,
        handler: impl Into<String>,
    ) {
        self.route(cat, sev, handler);
    }

    /// Total errors routed to handlers.
    pub fn total_routed(&self) -> u64 {
        self.routed
    }

    /// Reset route stats.
    pub fn reset_route_stats(&mut self) {
        self.routed = 0;
    }

    /// Route count alias.
    pub fn routes_registered(&self) -> usize {
        self.routes.len()
    }

    /// Default handler alias.
    pub fn fallback_handler(&self) -> &str {
        &self.default_handler
    }

    /// Has any routes registered.
    pub fn has_routes(&self) -> bool {
        !self.routes.is_empty()
    }
}

///////////////////////////////////////////////////////////////////////////////
// TESTS
///////////////////////////////////////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use super::*;

    ///////////////////////////////////////////////////////////////////////////////
    // ERROR RECOVERY HINT TESTS
    ///////////////////////////////////////////////////////////////////////////////

    #[test]
    fn error_recovery_hint_carrier_unavailable() {
        let e = Error::CarrierUnavailable(CarrierType::Direct, "down".into());
        assert!(matches!(e.recovery_hint(), RecoveryHint::RetryWithDifferentCarrier));
        assert!(e.is_transient());
        assert!(!e.requires_operator());
    }

    #[test]
    fn error_recovery_hint_invariant() {
        let e = Error::InvariantViolated { invariant: "binary_completion" };
        assert!(matches!(
            e.recovery_hint(),
            RecoveryHint::InvestigateInvariant { invariant: "binary_completion" }
        ));
        assert!(!e.is_transient());
        assert!(e.requires_operator());
    }

    #[test]
    fn error_recovery_hint_dead_node() {
        let e = Error::DeadNodeIdentityRetired { node: NodeId::ZERO };
        assert!(matches!(
            e.recovery_hint(),
            RecoveryHint::AssignNewIdentity
        ));
        assert!(e.is_transient());
    }

    #[test]
    fn error_recovery_hint_idle_rotation() {
        let e = Error::IdleRotationViolation { node: NodeId::ZERO };
        assert!(matches!(
            e.recovery_hint(),
            RecoveryHint::WaitForIdle
        ));
        assert!(e.is_transient());
    }

    #[test]
    fn error_recovery_hint_global_master() {
        let e = Error::GlobalMasterRejected {
            domain: OrderingDomain(1),
        };
        assert!(matches!(
            e.recovery_hint(),
            RecoveryHint::UseDatasetScopedOrdering
        ));
        assert!(e.is_transient());
    }

    #[test]
    fn error_recovery_hint_out_eq_in() {
        let e = Error::OutEqInViolation { message: "mismatch".into() };
        assert!(matches!(
            e.recovery_hint(),
            RecoveryHint::ReconcileAccounting
        ));
        assert!(!e.is_transient());
        assert!(e.requires_operator());
    }

    #[test]
    fn error_recovery_hint_version_mismatch() {
        let e = Error::VersionMismatch {
            expected: ProtocolVersion { major: 2, minor: 0, patch: 0 },
            got: ProtocolVersion { major: 1, minor: 0, patch: 0 },
        };
        assert!(matches!(
            e.recovery_hint(),
            RecoveryHint::NegotiateVersion
        ));
        assert!(e.is_transient());
    }

    #[test]
    fn error_recovery_hint_carrier_rejected_endpoint() {
        let e = Error::CarrierRejectedEndpoint {
            carrier: CarrierType::Optical,
        };
        assert!(matches!(
            e.recovery_hint(),
            RecoveryHint::UseCarrierAgnosticEndpoint
        ));
        assert!(e.is_transient());
    }

    #[test]
    fn error_display_carrier_unavailable() {
        let e = Error::CarrierUnavailable(CarrierType::Mesh, "link down".into());
        let s = format!("{e}");
        assert!(s.contains("Mesh"));
        assert!(s.contains("link down"));
    }

    #[test]
    fn error_display_invariant_violated() {
        let e = Error::InvariantViolated { invariant: "out_eq_in" };
        let s = format!("{e}");
        assert!(s.contains("out_eq_in"));
    }

    #[test]
    fn error_display_binary_completion_required() {
        let e = Error::BinaryCompletionRequired { op: OpRef(42) };
        let s = format!("{e}");
        assert!(s.contains("42"));
    }

    ///////////////////////////////////////////////////////////////////////////////
    // DOMAIN ERROR TESTS
    ///////////////////////////////////////////////////////////////////////////////

    #[test]
    fn carrier_error_variants() {
        let e = CarrierError::Timeout;
        assert!(matches!(e.recovery_hint(), RecoveryHint::RetryWithDifferentCarrier));
        let s = format!("{e}");
        assert_eq!(s, "timeout");
    }

    #[test]
    fn transport_error_variants() {
        let e = TransportError::DataCorrupted("crc mismatch".into());
        assert!(matches!(e.recovery_hint(), RecoveryHint::RetryWithDifferentCarrier));
    }

    #[test]
    fn store_error_variants() {
        let e = StoreError::NotFound("obj-1".into());
        assert!(matches!(e.recovery_hint(), RecoveryHint::AbortAndRetry));
    }

    #[test]
    fn session_error_variants() {
        let e = SessionError::AlreadyExists("sess-1".into());
        assert!(matches!(e.recovery_hint(), RecoveryHint::WaitForIdle));
    }

    #[test]
    fn routing_error_variants() {
        let e = RoutingError::Unavailable("no route".into());
        assert!(matches!(e.recovery_hint(), RecoveryHint::RetryImmediately));
    }

    #[test]
    fn discovery_error_variants() {
        let e = DiscoveryError::PreconditionFailed("not ready".into());
        assert!(matches!(e.recovery_hint(), RecoveryHint::RetryImmediately));
    }

    #[test]
    fn membership_error_variants() {
        let e = MembershipError::PermissionDenied("no join".into());
        assert!(matches!(e.recovery_hint(), RecoveryHint::RetryImmediately));
    }

    #[test]
    fn placement_error_variants() {
        let e = PlacementError::ResourceExhausted("full".into());
        assert!(matches!(e.recovery_hint(), RecoveryHint::ReconcileAccounting));
    }

    #[test]
    fn replication_error_variants() {
        let e = ReplicationError::Internal("replica lag".into());
        assert!(matches!(e.recovery_hint(), RecoveryHint::AbortAndRetry));
    }

    #[test]
    fn recovery_error_variants() {
        let e = RecoveryError::Unavailable("down".into());
        assert!(matches!(e.recovery_hint(), RecoveryHint::EscalateToOperator));
    }

    #[test]
    fn consistency_error_variants() {
        let e = ConsistencyError::DataCorrupted("hash mismatch".into());
        assert!(matches!(e.recovery_hint(), RecoveryHint::ReconcileAccounting));
    }

    #[test]
    fn ordering_error_variants() {
        let e = OrderingError::AlreadyExists("order-1".into());
        assert!(matches!(e.recovery_hint(), RecoveryHint::UseDatasetScopedOrdering));
    }

    #[test]
    fn security_error_variants() {
        let e = SecurityError::InvalidArgument("bad key".into());
        assert!(matches!(e.recovery_hint(), RecoveryHint::QuarantineNode));
    }

    #[test]
    fn client_error_variants() {
        let e = ClientError::Unavailable("gone".into());
        assert!(matches!(e.recovery_hint(), RecoveryHint::AbortAndRetry));
    }

    #[test]
    fn fault_error_variants() {
        let e = FaultError::Internal("crash".into());
        assert!(matches!(e.recovery_hint(), RecoveryHint::QuarantineNode));
    }

    #[test]
    fn broadcast_error_variants() {
        let e = BroadcastError::Timeout;
        assert!(matches!(e.recovery_hint(), RecoveryHint::RetryImmediately));
    }

    #[test]
    fn quorum_error_variants() {
        let e = QuorumError::Internal("x".into());
        assert!(matches!(e.recovery_hint(), RecoveryHint::RetryImmediately));
    }

    #[test]
    fn snapshot_error_variants() {
        let e = SnapshotError::AlreadyExists("snap-1".into());
        assert!(matches!(e.recovery_hint(), RecoveryHint::AbortAndRetry));
    }

    #[test]
    fn byzantine_error_variants() {
        let e = ByzantineError::Internal("lie".into());
        assert!(matches!(e.recovery_hint(), RecoveryHint::QuarantineNode));
    }

    #[test]
    fn lease_error_variants() {
        let e = LeaseError::Timeout;
        assert!(matches!(e.recovery_hint(), RecoveryHint::RetryImmediately));
    }

    #[test]
    fn crdt_error_variants() {
        let e = CRDTError::DataCorrupted("merge fail".into());
        assert!(matches!(e.recovery_hint(), RecoveryHint::ReconcileAccounting));
    }

    #[test]
    fn erasure_error_variants() {
        let e = ErasureError::Internal("x".into());
        assert!(matches!(e.recovery_hint(), RecoveryHint::AbortAndRetry));
    }

    #[test]
    fn saga_error_variants() {
        let e = SagaError::Timeout;
        assert!(matches!(e.recovery_hint(), RecoveryHint::AbortAndRetry));
    }

    #[test]
    fn coordinator_error_variants() {
        let e = CoordinatorError::Internal("x".into());
        assert!(matches!(e.recovery_hint(), RecoveryHint::FailFast));
    }

    #[test]
    fn durability_error_variants() {
        let e = DurabilityError::Internal("x".into());
        assert!(matches!(e.recovery_hint(), RecoveryHint::ReconcileAccounting));
    }

    #[test]
    fn topology_error_variants() {
        let e = TopologyError::Internal("x".into());
        assert!(matches!(e.recovery_hint(), RecoveryHint::RetryImmediately));
    }

    #[test]
    fn integration_error_variants() {
        let e = IntegrationError::Internal("bridge down".into());
        assert!(matches!(e.recovery_hint(), RecoveryHint::EscalateToOperator));
    }

    #[test]
    fn protocol_error_variants() {
        let e = ProtocolError::Unsupported("v3".into());
        assert!(matches!(e.recovery_hint(), RecoveryHint::NegotiateVersion));
    }

    #[test]
    fn wire_error_variants() {
        let e = WireError::Internal("x".into());
        assert!(matches!(e.recovery_hint(), RecoveryHint::RetryImmediately));
    }

    #[test]
    fn framing_error_variants() {
        let e = FramingError::Internal("x".into());
        assert!(matches!(e.recovery_hint(), RecoveryHint::RetryImmediately));
    }

    #[test]
    fn sequencing_error_variants() {
        let e = SequencingError::Internal("x".into());
        assert!(matches!(e.recovery_hint(), RecoveryHint::RetryImmediately));
    }

    #[test]
    fn ack_error_variants() {
        let e = AckError::Internal("x".into());
        assert!(matches!(e.recovery_hint(), RecoveryHint::RetryImmediately));
    }

    #[test]
    fn lifecycle_error_variants() {
        let e = LifecycleError::Internal("x".into());
        assert!(matches!(e.recovery_hint(), RecoveryHint::AbortAndRetry));
    }

    #[test]
    fn deletion_error_variants() {
        let e = DeletionError::Internal("x".into());
        assert!(matches!(e.recovery_hint(), RecoveryHint::AbortAndRetry));
    }

    #[test]
    fn fencing_error_variants() {
        let e = FencingError::Internal("x".into());
        assert!(matches!(e.recovery_hint(), RecoveryHint::FailFast));
    }

    #[test]
    fn roster_error_variants() {
        let e = RosterError::NotFound("node".into());
        assert!(matches!(e.recovery_hint(), RecoveryHint::RetryImmediately));
    }

    #[test]
    fn fallback_error_variants() {
        let e = FallbackError::Internal("x".into());
        assert!(matches!(e.recovery_hint(), RecoveryHint::RetryImmediately));
    }

    #[test]
    fn diversity_error_variants() {
        let e = DiversityError::Internal("x".into());
        assert!(matches!(e.recovery_hint(), RecoveryHint::ReconcileAccounting));
    }

    #[test]
    fn fanout_error_variants() {
        let e = FanoutError::Internal("x".into());
        assert!(matches!(e.recovery_hint(), RecoveryHint::AbortAndRetry));
    }

    #[test]
    fn graph_error_variants() {
        let e = GraphError::Internal("x".into());
        assert!(matches!(e.recovery_hint(), RecoveryHint::ReconcileAccounting));
    }

    #[test]
    fn integrity_error_variants() {
        let e = IntegrityError::Internal("x".into());
        assert!(matches!(e.recovery_hint(), RecoveryHint::ReconcileAccounting));
    }

    #[test]
    fn rotation_error_variants() {
        let e = RotationError::Internal("x".into());
        assert!(matches!(e.recovery_hint(), RecoveryHint::WaitForIdle));
    }

    #[test]
    fn retirement_error_variants() {
        let e = RetirementError::Internal("x".into());
        assert!(matches!(e.recovery_hint(), RecoveryHint::RetryImmediately));
    }

    #[test]
    fn time_error_variants() {
        let e = TimeError::Internal("x".into());
        assert!(matches!(e.recovery_hint(), RecoveryHint::RetryImmediately));
    }

    #[test]
    fn checkpoint_error_variants() {
        let e = CheckpointError::Internal("x".into());
        assert!(matches!(e.recovery_hint(), RecoveryHint::AbortAndRetry));
    }

    #[test]
    fn channel_error_variants() {
        let e = ChannelError::Internal("x".into());
        assert!(matches!(e.recovery_hint(), RecoveryHint::RetryImmediately));
    }

    ///////////////////////////////////////////////////////////////////////////////
    // TYPE CONSTRUCTOR TESTS
    ///////////////////////////////////////////////////////////////////////////////

    #[test]
    fn node_id_zero_and_new() {
        assert!(NodeId::ZERO.is_zero());
        let n = NodeId::new([1u8; 32]);
        assert!(!n.is_zero());
    }

    #[test]
    fn dataset_id_zero_and_new() {
        assert_eq!(DatasetId::ZERO.0, [0u8; 16]);
        let d = DatasetId::new([2u8; 16]);
        assert_eq!(d.0[0], 2);
    }

    #[test]
    fn object_id_zero_and_new() {
        assert_eq!(ObjectID::ZERO.0, [0u8; 16]);
        let o = ObjectID::new([3u8; 16]);
        assert_eq!(o.0[0], 3);
    }

    #[test]
    fn session_id_zero_and_new() {
        assert_eq!(SessionID::ZERO.0, [0u8; 16]);
        let s = SessionID::new([4u8; 16]);
        assert_eq!(s.0[0], 4);
    }

    #[test]
    fn op_ref_new() {
        let op = OpRef(100);
        assert_eq!(op.0, 100);
    }

    #[test]
    fn delete_proposal_id_new() {
        let p = DeleteProposalId(7);
        assert_eq!(p.0, 7);
    }

    #[test]
    fn ordering_domain_new() {
        let d = OrderingDomain(3);
        assert_eq!(d.0, 3);
    }

    #[test]
    fn protocol_version_fields() {
        let v = ProtocolVersion { major: 1, minor: 2, patch: 3 };
        assert_eq!(v.major, 1);
        assert_eq!(v.minor, 2);
        assert_eq!(v.patch, 3);
    }

    #[test]
    fn timestamp_millis_new() {
        let t = TimestampMillis(5000);
        assert_eq!(t.0, 5000);
    }

    #[test]
    fn fragment_id_new() {
        let f = FragmentId(9);
        assert_eq!(f.0, 9);
    }

    #[test]
    fn snapshot_id_new() {
        let s = SnapshotId([5u8; 16]);
        assert_eq!(s.0[0], 5);
    }

    #[test]
    fn epoch_new() {
        let e = Epoch(42);
        assert_eq!(e.0, 42);
    }

    #[test]
    fn saga_id_new() {
        let s = SagaId([6u8; 16]);
        assert_eq!(s.0[0], 6);
    }

    #[test]
    fn step_id_new() {
        let s = StepId(11);
        assert_eq!(s.0, 11);
    }

    #[test]
    fn transaction_id_new() {
        let t = TransactionId([7u8; 16]);
        assert_eq!(t.0[0], 7);
    }

    #[test]
    fn lease_id_new() {
        let l = LeaseId([8u8; 16]);
        assert_eq!(l.0[0], 8);
    }

    #[test]
    fn fencing_token_new() {
        let f = FencingToken(1000);
        assert_eq!(f.0, 1000);
    }

    #[test]
    fn crdt_state_new() {
        let c = CRDTState([9u8; 16]);
        assert_eq!(c.0[0], 9);
    }

    #[test]
    fn crdt_op_new() {
        let o = CRDTOp(12);
        assert_eq!(o.0, 12);
    }

    #[test]
    fn crdt_delta_new() {
        let d = CRDTDelta([10u8; 16]);
        assert_eq!(d.0[0], 10);
    }

    #[test]
    fn message_id_new() {
        let m = MessageId([11u8; 16]);
        assert_eq!(m.0[0], 11);
    }

    #[test]
    fn checksum_new() {
        let c = Checksum([0xAB; 32]);
        assert_eq!(c.0[0], 0xAB);
    }

    #[test]
    fn value_empty_and_len() {
        let v = Value::empty();
        assert!(v.is_empty());
        assert_eq!(v.len(), 0);
        let v = Value(vec![1, 2, 3]);
        assert_eq!(v.len(), 3);
        assert!(!v.is_empty());
    }

    #[test]
    fn message_new() {
        let m = Message::new(MessageId([1u8; 16]), Value::empty(), TimestampMillis(100));
        assert_eq!(m.timestamp.0, 100);
    }

    #[test]
    fn compute_request_fields() {
        let req = ComputeRequest {
            dataset_id: DatasetId::ZERO,
            code_hash: Checksum([0u8; 32]),
            input: Value::empty(),
        };
        assert_eq!(req.dataset_id, DatasetId::ZERO);
    }

    #[test]
    fn compute_result_fields() {
        let res = ComputeResult {
            output: Value::empty(),
            proof: RecoveryProof {
                nodes_involved: vec![NodeId::ZERO],
                checksum: Checksum([0u8; 32]),
                at: TimestampMillis(1),
            },
        };
        assert_eq!(res.output.len(), 0);
    }

    #[test]
    fn compact_result_fields() {
        let c = CompactResult { reclaimed_bytes: 1024, compacted_objects: 5 };
        assert_eq!(c.reclaimed_bytes, 1024);
        assert_eq!(c.compacted_objects, 5);
    }

    #[test]
    fn compute_error_display() {
        let e = ComputeError::Timeout;
        assert_eq!(format!("{e}"), "compute timed out");
        let e = ComputeError::CodeNotFound(Checksum([1u8; 32]));
        let s = format!("{e}");
        assert!(s.contains("CodeNotFound"));
    }

    #[test]
    fn tier_error_display() {
        let e = TierError::NotAllowed("hot->cold".into());
        assert_eq!(format!("{e}"), "tier transition not allowed: hot->cold");
    }

    #[test]
    fn mobility_error_display() {
        let e = MobilityError::HandoffTimeout;
        assert_eq!(format!("{e}"), "handoff timed out");
    }

    #[test]
    fn credential_and_token() {
        let c = Credential(vec![1, 2, 3]);
        assert_eq!(c.0.len(), 3);
        let t = AuthToken(vec![4, 5, 6]);
        assert_eq!(t.0.len(), 3);
    }

    #[test]
    fn key_id_and_signature() {
        let k = KeyId([1u8; 16]);
        assert_eq!(k.0[0], 1);
        let s = Signature(vec![9u8; 64]);
        assert_eq!(s.0.len(), 64);
    }

    #[test]
    fn circuit_state_variants() {
        assert!(matches!(CircuitState::Closed, CircuitState::Closed));
        assert!(matches!(CircuitState::Open, CircuitState::Open));
        assert!(matches!(CircuitState::HalfOpen, CircuitState::HalfOpen));
    }

    #[test]
    fn degradation_level_order() {
        assert!(DegradationLevel::Critical > DegradationLevel::None);
        assert!(DegradationLevel::Severe > DegradationLevel::Mild);
    }

    #[test]
    fn delivery_semantics_variants() {
        assert!(matches!(DeliverySemantics::ExactlyOnce, DeliverySemantics::ExactlyOnce));
        assert!(matches!(DeliverySemantics::AtMostOnce, DeliverySemantics::AtMostOnce));
        assert!(matches!(DeliverySemantics::AtLeastOnce, DeliverySemantics::AtLeastOnce));
    }

    #[test]
    fn ack_state_variants() {
        assert!(matches!(AckState::Pending, AckState::Pending));
        assert!(matches!(AckState::Received, AckState::Received));
        assert!(matches!(AckState::Lost, AckState::Lost));
    }

    #[test]
    fn circuit_breaker_state_variants() {
        assert!(matches!(CircuitBreakerState::Closed, CircuitBreakerState::Closed));
        assert!(matches!(CircuitBreakerState::Open, CircuitBreakerState::Open));
        assert!(matches!(CircuitBreakerState::HalfOpen, CircuitBreakerState::HalfOpen));
    }

    #[test]
    fn version_new() {
        let v = Version(5);
        assert_eq!(v.0, 5);
    }

    #[test]
    fn capacity_new() {
        let c = Capacity(1000);
        assert_eq!(c.0, 1000);
    }

    #[test]
    fn failure_event_fields() {
        let f = FailureEvent {
            node: NodeId::ZERO,
            timestamp: TimestampMillis(100),
            reason: "crash".into(),
        };
        assert_eq!(f.reason, "crash");
    }

    #[test]
    fn recovery_scope_level_variants() {
        assert!(matches!(RecoveryScopeLevel::Local, RecoveryScopeLevel::Local));
        assert!(matches!(RecoveryScopeLevel::Regional, RecoveryScopeLevel::Regional));
        assert!(matches!(RecoveryScopeLevel::Global, RecoveryScopeLevel::Global));
    }

    #[test]
    fn lifecycle_state_variants() {
        assert!(matches!(LifecycleState::Active, LifecycleState::Active));
        assert!(matches!(LifecycleState::Retired, LifecycleState::Retired));
        assert!(matches!(LifecycleState::Failed, LifecycleState::Failed));
    }

    #[test]
    fn anomaly_fields() {
        let a = Anomaly {
            kind: "drift".into(),
            severity: DegradationLevel::Severe,
            at: TimestampMillis(200),
        };
        assert_eq!(a.kind, "drift");
        assert_eq!(a.severity, DegradationLevel::Severe);
    }

    #[test]
    fn roster_change_fields() {
        let r = RosterChange {
            node: NodeId::ZERO,
            added: true,
            at: TimestampMillis(300),
        };
        assert!(r.added);
    }

    #[test]
    fn local_transaction_fields() {
        let t = LocalTransaction {
            id: TransactionId([1u8; 16]),
            ops: vec![OpRef(1), OpRef(2)],
        };
        assert_eq!(t.ops.len(), 2);
    }

    #[test]
    fn recovery_proof_fields() {
        let p = RecoveryProof {
            nodes_involved: vec![NodeId::ZERO, NodeId::new([2u8; 32])],
            checksum: Checksum([0u8; 32]),
            at: TimestampMillis(400),
        };
        assert_eq!(p.nodes_involved.len(), 2);
    }

    #[test]
    fn announcement_fields() {
        let a = Announcement {
            from: NodeId::ZERO,
            topic: "heartbeat".into(),
            payload: Value::empty(),
            at: TimestampMillis(500),
        };
        assert_eq!(a.topic, "heartbeat");
    }

    #[test]
    fn change_stream_fields() {
        let c = ChangeStream {
            dataset_id: DatasetId::ZERO,
            position: OrderingPos(10),
            change: ChangeType::Insert,
        };
        assert_eq!(c.position.0, 10);
    }

    #[test]
    fn change_type_variants() {
        assert!(matches!(ChangeType::Insert, ChangeType::Insert));
        assert!(matches!(ChangeType::Update, ChangeType::Update));
        assert!(matches!(ChangeType::Delete, ChangeType::Delete));
    }

    #[test]
    fn tier_variants() {
        assert!(matches!(Tier::Hot, Tier::Hot));
        assert!(matches!(Tier::Cold, Tier::Cold));
        assert!(matches!(Tier::Archived, Tier::Archived));
    }

    #[test]
    fn carrier_type_variants() {
        assert!(matches!(CarrierType::Direct, CarrierType::Direct));
        assert!(matches!(CarrierType::Mesh, CarrierType::Mesh));
        assert!(matches!(CarrierType::StoreAndForward, CarrierType::StoreAndForward));
        assert!(matches!(CarrierType::Optical, CarrierType::Optical));
        assert!(matches!(CarrierType::Acoustic, CarrierType::Acoustic));
        let c = CarrierType::Custom(99);
        assert!(matches!(c, CarrierType::Custom(99)));
        assert!(CarrierType::Direct.is_direct());
        assert!(!CarrierType::Mesh.is_direct());
    }

    ///////////////////////////////////////////////////////////////////////////////
    // ERROR CLONE AND DEBUG TESTS
    ///////////////////////////////////////////////////////////////////////////////

    #[test]
    fn error_clone() {
        let e = Error::CarrierUnavailable(CarrierType::Direct, "x".into());
        let e2 = e.clone();
        assert!(matches!(e2, Error::CarrierUnavailable(CarrierType::Direct, _)));
    }

    #[test]
    fn error_debug() {
        let e = Error::Store(ObjectID::ZERO, StoreError::Timeout);
        let s = format!("{e:?}");
        assert!(!s.is_empty());
    }

    #[test]
    fn domain_error_clone() {
        let e = TransportError::Timeout;
        let e2 = e.clone();
        assert_eq!(e, e2);
    }

    #[test]
    fn recovery_hint_clone() {
        let h = RecoveryHint::QuarantineNode;
        let h2 = h.clone();
        assert_eq!(h, h2);
    }

    ///////////////////////////////////////////////////////////////////////////////
    // ERROR IS_TRANSIENT AND REQUIRES_OPERATOR MATRIX
    ///////////////////////////////////////////////////////////////////////////////

    #[test]
    fn transient_matrix() {
        let cases: Vec<(Error, bool)> = vec![
            (Error::CarrierUnavailable(CarrierType::Direct, "x".into()), true),
            (Error::DeadNodeIdentityRetired { node: NodeId::ZERO }, true),
            (Error::IdleRotationViolation { node: NodeId::ZERO }, true),
            (Error::VersionMismatch { expected: ProtocolVersion { major: 1, minor: 0, patch: 0 }, got: ProtocolVersion { major: 2, minor: 0, patch: 0 } }, true),
            (Error::InvariantViolated { invariant: "x" }, false),
            (Error::OutEqInViolation { message: "x".into() }, false),
        ];
        for (e, expected) in cases {
            assert_eq!(e.is_transient(), expected, "error: {e:?}");
        }
    }

    #[test]
    fn requires_operator_matrix() {
        let cases: Vec<(Error, bool)> = vec![
            (Error::InvariantViolated { invariant: "x" }, true),
            (Error::OutEqInViolation { message: "x".into() }, true),
            (Error::CarrierUnavailable(CarrierType::Direct, "x".into()), false),
            (Error::DeadNodeIdentityRetired { node: NodeId::ZERO }, false),
        ];
        for (e, expected) in cases {
            assert_eq!(e.requires_operator(), expected, "error: {e:?}");
        }
    }

    ///////////////////////////////////////////////////////////////////////////////
    // ALL ERROR VARIANTS ARE REACHABLE (compile-time coverage)
    ///////////////////////////////////////////////////////////////////////////////

    #[test]
    fn all_carrier_error_variants_reachable() {
        let _ = CarrierError::Internal("x".into());
        let _ = CarrierError::Timeout;
        let _ = CarrierError::NotFound("x".into());
        let _ = CarrierError::AlreadyExists("x".into());
        let _ = CarrierError::InvalidArgument("x".into());
        let _ = CarrierError::ResourceExhausted("x".into());
        let _ = CarrierError::PermissionDenied("x".into());
        let _ = CarrierError::PreconditionFailed("x".into());
        let _ = CarrierError::Unavailable("x".into());
        let _ = CarrierError::DataCorrupted("x".into());
        let _ = CarrierError::Unsupported("x".into());
    }

    #[test]
    fn all_transport_error_variants_reachable() {
        let _ = TransportError::Internal("x".into());
        let _ = TransportError::Timeout;
        let _ = TransportError::NotFound("x".into());
        let _ = TransportError::AlreadyExists("x".into());
        let _ = TransportError::InvalidArgument("x".into());
        let _ = TransportError::ResourceExhausted("x".into());
        let _ = TransportError::PermissionDenied("x".into());
        let _ = TransportError::PreconditionFailed("x".into());
        let _ = TransportError::Unavailable("x".into());
        let _ = TransportError::DataCorrupted("x".into());
        let _ = TransportError::Unsupported("x".into());
    }

    #[test]
    fn all_store_error_variants_reachable() {
        let _ = StoreError::Internal("x".into());
        let _ = StoreError::Timeout;
        let _ = StoreError::NotFound("x".into());
        let _ = StoreError::AlreadyExists("x".into());
        let _ = StoreError::InvalidArgument("x".into());
        let _ = StoreError::ResourceExhausted("x".into());
        let _ = StoreError::PermissionDenied("x".into());
        let _ = StoreError::PreconditionFailed("x".into());
        let _ = StoreError::Unavailable("x".into());
        let _ = StoreError::DataCorrupted("x".into());
        let _ = StoreError::Unsupported("x".into());
    }

    #[test]
    fn all_session_error_variants_reachable() {
        let _ = SessionError::Internal("x".into());
        let _ = SessionError::Timeout;
        let _ = SessionError::NotFound("x".into());
        let _ = SessionError::AlreadyExists("x".into());
        let _ = SessionError::InvalidArgument("x".into());
        let _ = SessionError::ResourceExhausted("x".into());
        let _ = SessionError::PermissionDenied("x".into());
        let _ = SessionError::PreconditionFailed("x".into());
        let _ = SessionError::Unavailable("x".into());
        let _ = SessionError::DataCorrupted("x".into());
        let _ = SessionError::Unsupported("x".into());
    }

    #[test]
    fn all_routing_error_variants_reachable() {
        let _ = RoutingError::Internal("x".into());
        let _ = RoutingError::Timeout;
        let _ = RoutingError::NotFound("x".into());
        let _ = RoutingError::AlreadyExists("x".into());
        let _ = RoutingError::InvalidArgument("x".into());
        let _ = RoutingError::ResourceExhausted("x".into());
        let _ = RoutingError::PermissionDenied("x".into());
        let _ = RoutingError::PreconditionFailed("x".into());
        let _ = RoutingError::Unavailable("x".into());
        let _ = RoutingError::DataCorrupted("x".into());
        let _ = RoutingError::Unsupported("x".into());
    }

    #[test]
    fn all_security_error_variants_reachable() {
        let _ = SecurityError::Internal("x".into());
        let _ = SecurityError::Timeout;
        let _ = SecurityError::NotFound("x".into());
        let _ = SecurityError::AlreadyExists("x".into());
        let _ = SecurityError::InvalidArgument("x".into());
        let _ = SecurityError::ResourceExhausted("x".into());
        let _ = SecurityError::PermissionDenied("x".into());
        let _ = SecurityError::PreconditionFailed("x".into());
        let _ = SecurityError::Unavailable("x".into());
        let _ = SecurityError::DataCorrupted("x".into());
        let _ = SecurityError::Unsupported("x".into());
    }

    #[test]
    fn all_byzantine_error_variants_reachable() {
        let _ = ByzantineError::Internal("x".into());
        let _ = ByzantineError::Timeout;
        let _ = ByzantineError::NotFound("x".into());
        let _ = ByzantineError::AlreadyExists("x".into());
        let _ = ByzantineError::InvalidArgument("x".into());
        let _ = ByzantineError::ResourceExhausted("x".into());
        let _ = ByzantineError::PermissionDenied("x".into());
        let _ = ByzantineError::PreconditionFailed("x".into());
        let _ = ByzantineError::Unavailable("x".into());
        let _ = ByzantineError::DataCorrupted("x".into());
        let _ = ByzantineError::Unsupported("x".into());
    }

    #[test]
    fn error_enum_all_variants_reachable() {
        let _ = Error::CarrierUnavailable(CarrierType::Direct, "x".into());
        let _ = Error::Transport(CarrierType::Mesh, TransportError::Timeout);
        let _ = Error::Store(ObjectID::ZERO, StoreError::NotFound("x".into()));
        let _ = Error::Session(SessionID::ZERO, SessionError::Timeout);
        let _ = Error::Routing(RoutingError::Unavailable("x".into()));
        let _ = Error::Discovery(DiscoveryError::Timeout);
        let _ = Error::Membership(MembershipError::Timeout);
        let _ = Error::Placement(PlacementError::Timeout);
        let _ = Error::Replication(ReplicationError::Timeout);
        let _ = Error::Recovery(RecoveryError::Timeout);
        let _ = Error::Consistency(ConsistencyError::Timeout);
        let _ = Error::Ordering(OrderingError::Timeout);
        let _ = Error::Security(SecurityError::Timeout);
        let _ = Error::Client(ClientError::Timeout);
        let _ = Error::Fault(FaultError::Timeout);
        let _ = Error::Broadcast(BroadcastError::Timeout);
        let _ = Error::Quorum(QuorumError::Timeout);
        let _ = Error::Snapshot(SnapshotError::Timeout);
        let _ = Error::Byzantine(ByzantineError::Timeout);
        let _ = Error::Lease(LeaseError::Timeout);
        let _ = Error::CRDT(CRDTError::Timeout);
        let _ = Error::Erasure(ErasureError::Timeout);
        let _ = Error::Saga(SagaError::Timeout);
        let _ = Error::Coordinator(CoordinatorError::Timeout);
        let _ = Error::Durability(DurabilityError::Timeout);
        let _ = Error::Topology(TopologyError::Timeout);
        let _ = Error::Integration(IntegrationError::Timeout);
        let _ = Error::Protocol(ProtocolError::Timeout);
        let _ = Error::Wire(WireError::Timeout);
        let _ = Error::Framing(FramingError::Timeout);
        let _ = Error::Sequencing(SequencingError::Timeout);
        let _ = Error::Ack(AckError::Timeout);
        let _ = Error::InvariantViolated { invariant: "x" };
        let _ = Error::Lifecycle(LifecycleError::Timeout);
        let _ = Error::Deletion(DeletionError::Timeout);
        let _ = Error::Fencing(FencingError::Timeout);
        let _ = Error::Roster(RosterError::Timeout);
        let _ = Error::Fallback(FallbackError::Timeout);
        let _ = Error::Diversity(DiversityError::Timeout);
        let _ = Error::Fanout(FanoutError::Timeout);
        let _ = Error::Graph(GraphError::Timeout);
        let _ = Error::Integrity(IntegrityError::Timeout);
        let _ = Error::Rotation(RotationError::Timeout);
        let _ = Error::Retirement(RetirementError::Timeout);
        let _ = Error::Time(TimeError::Timeout);
        let _ = Error::Checkpoint(CheckpointError::Timeout);
        let _ = Error::Channel(ChannelError::Timeout);
        let _ = Error::BinaryCompletionRequired { op: OpRef(1) };
        let _ = Error::DeadNodeIdentityRetired { node: NodeId::ZERO };
        let _ = Error::IdleRotationViolation { node: NodeId::ZERO };
        let _ = Error::GlobalMasterRejected { domain: OrderingDomain(1) };
        let _ = Error::OutEqInViolation { message: "x".into() };
        let _ = Error::CoordinatedDeletionFailed { proposal: DeleteProposalId(1) };
        let _ = Error::VersionMismatch {
            expected: ProtocolVersion { major: 1, minor: 0, patch: 0 },
            got: ProtocolVersion { major: 2, minor: 0, patch: 0 },
        };
        let _ = Error::CarrierRejectedEndpoint { carrier: CarrierType::Direct };
    }
}