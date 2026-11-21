//! # ids-000-aa — IDS Central Hub / Father Crate
//!
//! Every canonical type, sealed trait contract, protocol invariant, error contract,
//! and version bit lives here. All downstream `ids-*` crates depend on `ids-000-aa`
//! only. Adding one edge case = edit exactly one file here, then implement the new
//! trait method in one dependent crate at a time.
//!
//! Carrier-agnostic absolute rule: no `std::net`, `tokio::net`, libp2p, IP, TCP,
//! UDP, QUIC, HTTP, DNS, Wi-Fi, Bluetooth, cellular, VPN, or conventional mesh.
//! `Endpoint` is `CarrierType + Vec<u8>` only.

pub mod contracts;
pub mod invariants;
pub mod internals;
pub mod codec;
pub mod state;
pub mod validation;
pub mod recovery;
pub mod placement;
pub mod routing;
pub mod security;
pub mod time;
pub mod identity;
pub mod quorum;
pub mod broadcast;
pub mod crdt;
pub mod erasure;
pub mod snapshot;
pub mod fault;
pub mod byzantine;
pub mod saga;
pub mod lease;
pub mod chaos;
pub mod observability;

pub mod types;
pub mod versioning;
pub mod errors;
pub mod prelude;

// Re-export canonical types at crate root
pub use types::{
    NodeId, RegionId, HUnitId, DatasetId, PartitionId, RecordId, OpRef, OrderingPos,
    Endpoint, CarrierType, RotatingNodeId, OpState, CommitState, RedundancyMode,
    ConsistencyProfile, PlacementDecision, FailureDomain, NodeState, HUnitState,
    RegionState, DurabilityCondition, IdsTime,
};

// Re-export contracts at crate root
pub use contracts::{
    Carrier, Transport, Store, Session, Routing, Discovery, Membership, Placement,
    Replication, Recovery, Consistency, Ordering, Security, Client, FaultTolerance,
    Broadcast, Quorum, Snapshot, Byzantine, Lease, CRDT, Erasure, Saga, Coordinator,
    Durability, Topology, Integration, Protocol, Wire, Framing, Sequencing, Acks,
};

// Re-export invariants at crate root
pub use invariants::{
    BinaryCompletion, DeadNodeIdentity, IdleRotation, NoGlobalMaster, OutEqIn,
    CoordinatedDeletion, FencingStonith, QuorumIntersection, ErasureThreshold,
    CorrelatedFailure, HUnitCompensation, RegionalAutonomy, HybridRedundancy,
    OperationLifecycle, DurabilityAck, OrderedReadVisibility, NodeRoster,
    RegionAffinity, IdentityRotation, IdsStandardTime, DailyIntegrity, DbRetirement,
    ReadFallback, TopologyDiversity, WriteFanout, CustodyGraph, ChecksumBoundary,
    ReplicaFreshness, CommitPrefix, VersionVector, MembershipEpoch, PlacementPolicy,
    ReplicationLag, RecoveryScope, DataLifecycle, SnapshotConsistency, ComputeToData,
    HotColdTier, MobilityHandoff, FederationTrust,
};

// Re-export error types at crate root
pub use errors::{Error, CarrierError, TransportError, StoreError, SessionError,
    RoutingError, DiscoveryError, MembershipError, PlacementError, ReplicationError,
    RecoveryError, ConsistencyError, OrderingError, SecurityError, ClientError,
    FaultError, BroadcastError, QuorumError, SnapshotError, ByzantineError,
    LeaseError, CRDTError, ErasureError, SagaError, CoordinatorError,
    DurabilityError, TopologyError, IntegrationError, ProtocolError, WireError,
    FramingError, SequencingError, AckError, InvariantError, LifecycleError,
    DeletionError, FencingError, RosterError, FallbackError, DiversityError,
    FanoutError, GraphError, IntegrityError, RotationError, RetirementError,
    TimeError, CheckpointError, ChannelError, RecoveryProof, Announcement,
    ChangeStream, FragmentId, Version, Epoch, OrderingDomain, SnapshotId, CRDTState,
    CRDTOp, CRDTDelta, Tier, ComputeRequest, ComputeResult, CompactResult,
    ComputeError, TierError, MobilityError, Credential, AuthToken, KeyId, Signature,
    Value, Message, MessageId, CircuitState, DegradationLevel, LeaseId, FencingToken,
    SagaId, StepId, LocalTransaction, TransactionId, Capacity, FailureEvent,
    RecoveryScopeLevel, LifecycleState, Anomaly, RosterChange, Checksum,
    DeliverySemantics, AckState, CircuitBreakerState,
};

// Re-export versioning at crate root
pub use versioning::{ProtocolVersion, EdgeCaseBits};

// Re-export internals (not re-exported downstream — internal only)
pub use internals::{
    TypeRegistry, VersionGraph, ContractValidator, InvariantChecker,
    EdgeCaseCompiler, OpRefAllocator, OrderingPositionClock, DurabilityTracker,
    ReplicationStreamMgr, SyncCheckpointMgr, RecoveryPlanBuilder,
    ConsistencyProfileMgr, BroadcastTopology, QuorumCalculator, SnapshotCoordinator,
    ByzantineVoteMgr, LeaseTracker, CRDTMergeEngine, ErasureCodec, SagaCoordinator,
    CoordinatorEpoch,
};

// Re-export prelude at crate root
pub use prelude::*;

// Re-export codec types at crate root
pub use codec::{
    FrameHeader, FrameBody, WireFormat, Datagram, EndpointCodec, OpRefCodec,
    OrderingPosCodec, NodeIdCodec, RegionIdCodec, HUnitIdCodec, DatasetIdCodec,
    PartitionIdCodec, RecordIdCodec, CarrierTypeCodec,
};

// Re-export state machine types at crate root
pub use state::{
    OpStateMachine, CommitStateMachine, NodeStateMachine, HUnitStateMachine,
    RegionStateMachine, SessionStateMachine, MembershipStateMachine,
    PlacementStateMachine, ReplicationStateMachine, RecoveryStateMachine,
    ConsistencyStateMachine, OrderingStateMachine, BroadcastStateMachine,
    QuorumStateMachine, SnapshotStateMachine, ByzantineStateMachine,
    LeaseStateMachine, CRDTStateMachine, ErasureStateMachine, SagaStateMachine,
    CoordinatorStateMachine,
};