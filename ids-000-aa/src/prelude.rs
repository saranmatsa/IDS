//! # ids-000-aa Prelude
//!
//! The central hub prelude for the IDS workspace. This module re-exports every
//! canonical type, sealed trait contract, protocol invariant, error contract,
//! and version bit from `ids-000-aa` so downstream `ids-*` crates can do a
//! single `use ids_000_aa::prelude::*;` and get the complete surface area.
//!
//! Beyond re-exports, this file defines extension traits, conversion helpers,
//! validation macros, and composition utilities that turn the raw hub types
//! into ergonomic, production-grade building blocks for every downstream crate.
//!
//! Carrier-agnostic absolute rule: no `std::net`, `tokio::net`, libp2p, IP,
//! TCP, UDP, QUIC, HTTP, DNS, Wi-Fi, Bluetooth, cellular, VPN, or conventional
//! mesh. `Endpoint` is `CarrierType + Vec<u8>` only.

///////////////////////////////////////////////////////////////////////////////
// SECTION 1 — CORE TYPE RE-EXPORTS
///////////////////////////////////////////////////////////////////////////////

pub use crate::types::{
    NodeId, RegionId, HUnitId, DatasetId, PartitionId, RecordId, OpRef,
    OrderingPos, Endpoint, CarrierType, RotatingNodeId, OpState, CommitState,
    RedundancyMode, ConsistencyProfile, PlacementDecision, FailureDomain,
    NodeState, HUnitState, RegionState, DurabilityCondition, IdsTime,
    ChangeStream, FragmentId, Version, Epoch, OrderingDomain, SnapshotId,
    CRDTState, CRDTOp, CRDTDelta, Tier, ComputeRequest, ComputeResult,
    CompactResult, ComputeError, TierError, MobilityError, Credential,
    AuthToken, KeyId, Signature, Value, Message, MessageId, CircuitState,
    DegradationLevel, LeaseId, FencingToken, SagaId, StepId,
    LocalTransaction, TransactionId, Capacity, FailureEvent,
    RecoveryScopeLevel, LifecycleState, Anomaly, RosterChange, Checksum,
    DeliverySemantics, AckState, CircuitBreakerState,
};

///////////////////////////////////////////////////////////////////////////////
// SECTION 2 — CONTRACT TRAIT RE-EXPORTS
///////////////////////////////////////////////////////////////////////////////

pub use crate::contracts::{
    Carrier, Transport, Store, Session, Routing, Discovery, Membership,
    Placement, Replication, Recovery, Consistency, Ordering, Security,
    Client, FaultTolerance, Broadcast, Quorum, Snapshot, Byzantine, Lease,
    CRDT, Erasure, Saga, Coordinator, Durability, Topology, Integration,
    Protocol, Wire, Framing, Sequencing, Acks,
};

///////////////////////////////////////////////////////////////////////////////
// SECTION 3 — INVARIANT RE-EXPORTS
///////////////////////////////////////////////////////////////////////////////

pub use crate::invariants::{
    BinaryCompletion, DeadNodeIdentity, IdleRotation, NoGlobalMaster, OutEqIn,
    CoordinatedDeletion, FencingStonith, QuorumIntersection, ErasureThreshold,
    CorrelatedFailure, HUnitCompensation, RegionalAutonomy, HybridRedundancy,
    OperationLifecycle, DurabilityAck, OrderedReadVisibility, NodeRoster,
    RegionAffinity, IdentityRotation, IdsStandardTime, DailyIntegrity,
    DbRetirement, ReadFallback, TopologyDiversity, WriteFanout, CustodyGraph,
    ChecksumBoundary, ReplicaFreshness, CommitPrefix, VersionVector,
    MembershipEpoch, PlacementPolicy, ReplicationLag, RecoveryScope,
    DataLifecycle, SnapshotConsistency, ComputeToData, HotColdTier,
    MobilityHandoff, FederationTrust,
};

///////////////////////////////////////////////////////////////////////////////
// SECTION 4 — ERROR CONTRACT RE-EXPORTS
///////////////////////////////////////////////////////////////////////////////

pub use crate::errors::{
    Error, CarrierError, TransportError, StoreError, SessionError,
    RoutingError, DiscoveryError, MembershipError, PlacementError,
    ReplicationError, RecoveryError, ConsistencyError, OrderingError,
    SecurityError, ClientError, FaultError, BroadcastError, QuorumError,
    SnapshotError, ByzantineError, LeaseError, CRDTError, ErasureError,
    SagaError, CoordinatorError, DurabilityError, TopologyError,
    IntegrationError, ProtocolError, WireError, FramingError,
    SequencingError, AckError, InvariantError, LifecycleError,
    DeletionError, FencingError, RosterError, FallbackError,
    DiversityError, FanoutError, GraphError, IntegrityError,
    RotationError, RetirementError, TimeError, CheckpointError,
    ChannelError, RecoveryProof, Announcement, ErrorContext, ErrorSeverity,
    RecoveryAction, ErrorCode, Value as ErrorValue, Message as ErrorMessage,
    ErrorStats, RetryPolicy, CircuitBreaker, CircuitState,
    BackoffStrategy, BackoffConfig, ErrorTally, ErrorTallyEntry, HealthScore,
    ComponentHealth, ErrorRouter, RouteDecision, AggregatedErrorReport,
    ErrorCategory, ErrorClassifier, ErrorChain, ErrorChainLink,
    ErrorFactory, ErrorValidator, ErrorAggregator, ErrorSampler,
    ErrorCircuitBreaker, ErrorProbe, ErrorFingerprint, ErrorFingerprinter,
    ErrorDeadLetterQueue, DeadLetterEntry, ErrorBackoffScheduler,
    ErrorCorrelator, ErrorRateLimiter, ErrorExporter, ErrorSummary,
    ErrorMetricsRegistry,
};

///////////////////////////////////////////////////////////////////////////////
// SECTION 5 — VERSIONING RE-EXPORTS
///////////////////////////////////////////////////////////////////////////////

pub use crate::versioning::{
    ProtocolVersion, EdgeCaseBits, SemanticVersion, VersionRange,
    ProtocolFeature, ProtocolRegistry, CompatibilityMatrix, VersionGraph,
    MigrationPath, VersionHistory, VersionNegotiation, EdgeCaseCompiler,
    EdgeCaseProfile, VersionSnapshot, FeatureGate, StrategyNegotiator,
    EdgeCaseTransition, ProtocolCapability, CapabilityRegistry,
    VersionPolicy, CompatibilityAnalyzer,
};

///////////////////////////////////////////////////////////////////////////////
// SECTION 6 — EXTENSION TRAITS FOR NODE IDENTITY
///////////////////////////////////////////////////////////////////////////////

/// Extension trait for `NodeId` providing domain-specific operations.
pub trait NodeIdExt {
    /// Returns true if this node id represents the zero node (uninitialized).
    fn is_zero_node(&self) -> bool;

    /// Returns the short hex prefix (first 8 chars) for logging.
    fn short_hex(&self) -> String;

    /// Returns the region hash derived from this node id.
    fn region_hash(&self) -> [u8; 8];

    /// Checks whether this node id is compatible with a given region.
    fn belongs_to_region(&self, region: RegionId) -> bool;

    /// Computes a stable distance metric between two node ids.
    fn distance_to(&self, other: NodeId) -> u64;

    /// Returns true if this node id falls within a given key range.
    fn in_range(&self, start: NodeId, end: NodeId) -> bool;

    /// Rotates this node id by a given epoch for membership rotation.
    fn rotate(self, epoch: Epoch) -> RotatingNodeId;

    /// Returns the byte representation as a fixed array.
    fn as_fixed_bytes(&self) -> [u8; 32];

    /// Creates a NodeId from a BLAKE3 digest output.
    fn from_blake3_digest(digest: &blake3::Hash) -> Self;
}

impl NodeIdExt for NodeId {
    fn is_zero_node(&self) -> bool {
        self.0 == [0u8; 32]
    }

    fn short_hex(&self) -> String {
        self.0[..8].iter().map(|b| format!("{:02x}", b)).collect()
    }

    fn region_hash(&self) -> [u8; 8] {
        let hash = blake3::hash(&self.0);
        let mut out = [0u8; 8];
        out.copy_from_slice(&hash.as_bytes()[..8]);
        out
    }

    fn belongs_to_region(&self, region: RegionId) -> bool {
        self.region_hash() == region.0[..8]
    }

    fn distance_to(&self, other: NodeId) -> u64 {
        let mut dist: u64 = 0;
        for i in 0..32 {
            dist = dist.wrapping_add(self.0[i].abs_diff(other.0[i]) as u64);
            dist = dist.wrapping_mul(0x9E3779B97F4A7C15);
        }
        dist
    }

    fn in_range(&self, start: NodeId, end: NodeId) -> bool {
        if start <= end {
            self.0 >= start.0 && self.0 <= end.0
        } else {
            self.0 >= start.0 || self.0 <= end.0
        }
    }

    fn rotate(self, epoch: Epoch) -> RotatingNodeId {
        let hash = blake3::keyed_hash(&epoch.to_le_bytes(), &self.0);
        RotatingNodeId(hash.into())
    }

    fn as_fixed_bytes(&self) -> [u8; 32] {
        self.0
    }

    fn from_blake3_digest(digest: &blake3::Hash) -> Self {
        let mut bytes = [0u8; 32];
        bytes.copy_from_slice(digest.as_bytes());
        NodeId(bytes)
    }
}

///////////////////////////////////////////////////////////////////////////////
// SECTION 7 — EXTENSION TRAITS FOR ENDPOINT AND CARRIER
///////////////////////////////////////////////////////////////////////////////

/// Extension trait for `Endpoint` providing carrier-agnostic operations.
pub trait EndpointExt {
    /// Returns the carrier type associated with this endpoint.
    fn carrier(&self) -> CarrierType;

    /// Returns the opaque payload as a byte slice.
    fn payload(&self) -> &[u8];

    /// Returns true if the endpoint payload is empty.
    fn is_empty_payload(&self) -> bool;

    /// Returns the payload length in bytes.
    fn payload_len(&self) -> usize;

    /// Creates a new endpoint with a given carrier and payload.
    fn new_endpoint(carrier: CarrierType, payload: Vec<u8>) -> Self;

    /// Prepends a routing prefix to the endpoint payload.
    fn with_prefix(mut self, prefix: &[u8]) -> Self;

    /// Strips a routing prefix from the endpoint payload if present.
    fn strip_prefix(self, prefix: &[u8]) -> Option<Self>;

    /// Computes a BLAKE3 fingerprint of the endpoint payload.
    fn fingerprint(&self) -> [u8; 32];

    /// Returns true if two endpoints are carrier-compatible.
    fn carrier_compatible(&self, other: &Endpoint) -> bool;
}

impl EndpointExt for Endpoint {
    fn carrier(&self) -> CarrierType {
        self.carrier
    }

    fn payload(&self) -> &[u8] {
        &self.address
    }

    fn is_empty_payload(&self) -> bool {
        self.address.is_empty()
    }

    fn payload_len(&self) -> usize {
        self.address.len()
    }

    fn new_endpoint(carrier: CarrierType, payload: Vec<u8>) -> Self {
        Endpoint { carrier, address: payload }
    }

    fn with_prefix(mut self, prefix: &[u8]) -> Self {
        let mut new_addr = prefix.to_vec();
        new_addr.extend_from_slice(&self.address);
        self.address = new_addr;
        self
    }

    fn strip_prefix(mut self, prefix: &[u8]) -> Option<Self> {
        if self.address.starts_with(prefix) {
            self.address = self.address[prefix.len()..].to_vec();
            Some(self)
        } else {
            None
        }
    }

    fn fingerprint(&self) -> [u8; 32] {
        let hash = blake3::hash(&self.address);
        *hash.as_bytes()
    }

    fn carrier_compatible(&self, other: &Endpoint) -> bool {
        self.carrier == other.carrier
    }
}

///////////////////////////////////////////////////////////////////////////////
// SECTION 8 — EXTENSION TRAITS FOR MESSAGE AND VALUE
///////////////////////////////////////////////////////////////////////////////

/// Extension trait for `Message` providing message lifecycle operations.
pub trait MessageExt {
    /// Returns true if this message payload is empty.
    fn is_empty_payload(&self) -> bool;

    /// Returns the message payload length in bytes.
    fn payload_len(&self) -> usize;

    /// Returns true if this message was created before the given timestamp.
    fn older_than(&self, timestamp: TimestampMillis) -> bool;

    /// Returns the BLAKE3 hash of the message payload.
    fn payload_hash(&self) -> [u8; 32];

    /// Creates a reply message to this message.
    fn reply(&self, payload: Value) -> Message;

    /// Returns true if this message is idempotent-safe.
    fn is_idempotent(&self) -> bool;

    /// Returns the message age in milliseconds from a reference timestamp.
    fn age_ms(&self, now: TimestampMillis) -> u64;
}

/// Timestamp in milliseconds since IDS epoch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TimestampMillis(pub u64);

impl MessageExt for Message {
    fn is_empty_payload(&self) -> bool {
        self.payload.is_empty()
    }

    fn payload_len(&self) -> usize {
        self.payload.len()
    }

    fn older_than(&self, timestamp: TimestampMillis) -> bool {
        self.timestamp.0 < timestamp.0
    }

    fn payload_hash(&self) -> [u8; 32] {
        let hash = blake3::hash(&self.payload);
        *hash.as_bytes()
    }

    fn reply(&self, payload: Value) -> Message {
        Message {
            id: MessageId(self.id.0.wrapping_add(1)),
            payload,
            timestamp: TimestampMillis(self.timestamp.0.wrapping_add(1)),
        }
    }

    fn is_idempotent(&self) -> bool {
        // Messages with empty payloads or pure query semantics are idempotent.
        self.payload.is_empty()
    }

    fn age_ms(&self, now: TimestampMillis) -> u64 {
        now.0.saturating_sub(self.timestamp.0)
    }
}

/// Extension trait for `Value` providing serialization and size operations.
pub trait ValueExt {
    /// Returns true if this value is empty.
    fn is_empty(&self) -> bool;

    /// Returns the byte length of this value.
    fn byte_len(&self) -> usize;

    /// Returns the BLAKE3 digest of this value's bytes.
    fn digest(&self) -> [u8; 32];

    /// Returns true if this value exceeds the given size limit.
    fn exceeds(&self, limit: usize) -> bool;

    /// Splits this value into chunks of at most `chunk_size` bytes.
    fn chunk(&self, chunk_size: usize) -> Vec<Value>;
}

impl ValueExt for Value {
    fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    fn byte_len(&self) -> usize {
        self.0.len()
    }

    fn digest(&self) -> [u8; 32] {
        let hash = blake3::hash(&self.0);
        *hash.as_bytes()
    }

    fn exceeds(&self, limit: usize) -> bool {
        self.0.len() > limit
    }

    fn chunk(&self, chunk_size: usize) -> Vec<Value> {
        if chunk_size == 0 {
            return vec![Value(self.0.clone())];
        }
        self.0
            .chunks(chunk_size)
            .map(|c| Value(c.to_vec()))
            .collect()
    }
}

///////////////////////////////////////////////////////////////////////////////
// SECTION 9 — EXTENSION TRAITS FOR OPREF AND ORDERING POS
///////////////////////////////////////////////////////////////////////////////

/// Extension trait for `OpRef` providing operation reference operations.
pub trait OpRefExt {
    /// Returns the raw u64 value.
    fn value(&self) -> u64;

    /// Returns the next sequential OpRef.
    fn next(&self) -> OpRef;

    /// Returns true if this OpRef is zero (initial).
    fn is_initial(&self) -> bool;

    /// Returns the distance to another OpRef.
    fn distance_to(&self, other: OpRef) -> u64;

    /// Returns true if this OpRef is strictly less than another.
    fn precedes(&self, other: OpRef) -> bool;
}

impl OpRefExt for OpRef {
    fn value(&self) -> u64 {
        self.0
    }

    fn next(&self) -> OpRef {
        OpRef(self.0.wrapping_add(1))
    }

    fn is_initial(&self) -> bool {
        self.0 == 0
    }

    fn distance_to(&self, other: OpRef) -> u64 {
        if other.0 >= self.0 {
            other.0 - self.0
        } else {
            self.0 - other.0
        }
    }

    fn precedes(&self, other: OpRef) -> bool {
        self.0 < other.0
    }
}

/// Extension trait for `OrderingPos` providing ordering position operations.
pub trait OrderingPosExt {
    /// Returns the raw u64 value.
    fn value(&self) -> u64;

    /// Returns the next ordering position.
    fn next(&self) -> OrderingPos;

    /// Returns true if this position is at the beginning.
    fn is_beginning(&self) -> bool;

    /// Returns the distance to another position.
    fn distance_to(&self, other: OrderingPos) -> u64;
}

impl OrderingPosExt for OrderingPos {
    fn value(&self) -> u64 {
        self.0
    }

    fn next(&self) -> OrderingPos {
        OrderingPos(self.0.wrapping_add(1))
    }

    fn is_beginning(&self) -> bool {
        self.0 == 0
    }

    fn distance_to(&self, other: OrderingPos) -> u64 {
        if other.0 >= self.0 {
            other.0 - self.0
        } else {
            self.0 - other.0
        }
    }
}

///////////////////////////////////////////////////////////////////////////////
// SECTION 10 — EXTENSION TRAITS FOR SESSION AND STREAM IDS
///////////////////////////////////////////////////////////////////////////////

/// Extension trait for `SessionID` providing session operations.
pub trait SessionIdExt {
    /// Returns true if this session id is zero (unset).
    fn is_zero(&self) -> bool;

    /// Returns the first 8 bytes as a short identifier.
    fn short_id(&self) -> [u8; 8];

    /// Returns true if two session ids share the same region prefix.
    fn same_region(&self, other: SessionID) -> bool;
}

impl SessionIdExt for SessionID {
    fn is_zero(&self) -> bool {
        self.0 == [0u8; 16]
    }

    fn short_id(&self) -> [u8; 8] {
        let mut out = [0u8; 8];
        out.copy_from_slice(&self.0[..8]);
        out
    }

    fn same_region(&self, other: SessionID) -> bool {
        self.0[..8] == other.0[..8]
    }
}

///////////////////////////////////////////////////////////////////////////////
// SECTION 11 — EXTENSION TRAITS FOR DATASET AND OBJECT IDS
///////////////////////////////////////////////////////////////////////////////

/// Extension trait for `DatasetId` providing dataset operations.
pub trait DatasetIdExt {
    /// Returns true if this dataset id is zero.
    fn is_zero(&self) -> bool;

    /// Returns the BLAKE3 hash of this dataset id.
    fn hash(&self) -> [u8; 32];

    /// Returns true if this dataset belongs to a given region.
    fn belongs_to_region(&self, region: RegionId) -> bool;
}

impl DatasetIdExt for DatasetId {
    fn is_zero(&self) -> bool {
        self.0 == [0u8; 16]
    }

    fn hash(&self) -> [u8; 32] {
        let h = blake3::hash(&self.0);
        *h.as_bytes()
    }

    fn belongs_to_region(&self, region: RegionId) -> bool {
        self.0[..8] == region.0[..8]
    }
}

/// Extension trait for `ObjectID` providing object operations.
pub trait ObjectIdExt {
    /// Returns true if this object id is zero.
    fn is_zero(&self) -> bool;

    /// Returns the BLAKE3 hash of this object id.
    fn hash(&self) -> [u8; 32];
}

impl ObjectIdExt for ObjectID {
    fn is_zero(&self) -> bool {
        self.0 == [0u8; 16]
    }

    fn hash(&self) -> [u8; 32] {
        let h = blake3::hash(&self.0);
        *h.as_bytes()
    }
}

///////////////////////////////////////////////////////////////////////////////
// SECTION 12 — CONVERSION HELPERS
///////////////////////////////////////////////////////////////////////////////

/// Converts a `NodeId` to its hex string representation.
pub fn node_id_to_hex(node: NodeId) -> String {
    node.to_hex()
}

/// Converts a hex string back to a `NodeId`.
pub fn hex_to_node_id(hex: &str) -> Result<NodeId, Error> {
    NodeId::from_hex(hex)
}

/// Converts an `Endpoint` to its byte representation.
pub fn endpoint_to_bytes(endpoint: &Endpoint) -> Vec<u8> {
    let mut out = Vec::with_capacity(1 + endpoint.address.len());
    out.push(endpoint.carrier as u8);
    out.extend_from_slice(&endpoint.address);
    out
}

/// Converts bytes back to an `Endpoint`.
pub fn bytes_to_endpoint(bytes: &[u8]) -> Option<Endpoint> {
    if bytes.is_empty() {
        return None;
    }
    let carrier = CarrierType::from_u8(bytes[0])?;
    let address = bytes[1..].to_vec();
    Some(Endpoint { carrier, address })
}

/// Computes a BLAKE3 hash of any byte slice.
pub fn blake3_of(data: &[u8]) -> [u8; 32] {
    let h = blake3::hash(data);
    *h.as_bytes()
}

/// Computes a keyed BLAKE3 hash.
pub fn blake3_keyed(key: &[u8], data: &[u8]) -> [u8; 32] {
    let h = blake3::keyed_hash(key, data);
    *h.as_bytes()
}

/// Derives a `NodeId` from a byte array.
pub fn node_id_from_bytes(bytes: [u8; 32]) -> NodeId {
    NodeId(bytes)
}

/// Derives a `DatasetId` from a byte array.
pub fn dataset_id_from_bytes(bytes: [u8; 16]) -> DatasetId {
    DatasetId(bytes)
}

/// Derives a `RegionId` from a byte array.
pub fn region_id_from_bytes(bytes: [u8; 32]) -> RegionId {
    RegionId(bytes)
}

/// Creates a `TimestampMillis` from u64 millis.
pub fn timestamp_millis(millis: u64) -> TimestampMillis {
    TimestampMillis(millis)
}

///////////////////////////////////////////////////////////////////////////////
// SECTION 13 — VALIDATION MACROS
///////////////////////////////////////////////////////////////////////////////

/// Asserts that a node id is not zero. Panics with a descriptive message.
#[macro_export]
macro_rules! require_nonzero_node {
    ($node:expr) => {
        if $node.is_zero_node() {
            panic!("NodeId must not be zero");
        }
    };
}

/// Asserts that an endpoint has a non-empty payload.
#[macro_export]
macro_rules! require_nonempty_endpoint {
    ($endpoint:expr) => {
        if $endpoint.is_empty_payload() {
            panic!("Endpoint payload must not be empty");
        }
    };
}

/// Asserts that an error is transient (retryable).
#[macro_export]
macro_rules! require_transient {
    ($err:expr) => {
        if !$err.is_transient() {
            panic!("Error is not transient: {:?}", $err);
        }
    };
}

/// Asserts that an invariant holds.
#[macro_export]
macro_rules! invariant_holds {
    ($cond:expr, $inv:expr) => {
        if !$cond {
            panic!("Invariant violated: {}", $inv);
        }
    };
}

/// Asserts that a version is compatible.
#[macro_export]
macro_rules! require_compatible_version {
    ($ver:expr, $min:expr) => {
        if $ver < $min {
            panic!(
                "Version {:?} is below minimum {:?}",
                $ver, $min
            );
        }
    };
}

///////////////////////////////////////////////////////////////////////////////
// SECTION 14 — COMPOSITION UTILITIES
///////////////////////////////////////////////////////////////////////////////

/// Combines multiple node ids into a single composite id using BLAKE3.
pub fn composite_node_id(nodes: &[NodeId]) -> NodeId {
    let mut hasher = blake3::Hasher::new();
    for node in nodes {
        hasher.update(&node.0);
    }
    let out = hasher.finalize();
    let mut bytes = [0u8; 32];
    bytes.copy_from_slice(out.as_bytes());
    NodeId(bytes)
}

/// Combines multiple endpoints into a routing path.
pub fn routing_path(endpoints: &[Endpoint]) -> Vec<Endpoint> {
    endpoints.to_vec()
}

/// Filters endpoints by carrier type.
pub fn filter_by_carrier(endpoints: &[Endpoint], carrier: CarrierType) -> Vec<Endpoint> {
    endpoints
        .iter()
        .filter(|e| e.carrier == carrier)
        .cloned()
        .collect()
}

/// Sorts endpoints by carrier type then payload length.
pub fn sort_endpoints(endpoints: &mut [Endpoint]) {
    endpoints.sort_by_key(|e| (e.carrier as u8, e.address.len()));
}

/// Deduplicates endpoints by fingerprint.
pub fn dedup_endpoints(endpoints: &mut Vec<Endpoint>) {
    let mut seen = std::collections::HashSet::new();
    endpoints.retain(|e| {
        let fp = e.fingerprint();
        seen.insert(fp)
    });
}

/// Partitions a dataset id range into shards.
pub fn partition_dataset_range(
    start: DatasetId,
    end: DatasetId,
    shard_count: usize,
) -> Vec<(DatasetId, DatasetId)> {
    if shard_count == 0 {
        return vec![];
    }
    let mut result = Vec::with_capacity(shard_count);
    for i in 0..shard_count {
        let hash_start = blake3_keyed(
            &[i as u8],
            &start.0,
        );
        let hash_end = blake3_keyed(
            &[(i + 1) as u8],
            &end.0,
        );
        result.push((
            DatasetId(hash_start[..16].try_into().unwrap_or([0u8; 16])),
            DatasetId(hash_end[..16].try_into().unwrap_or([0u8; 16])),
        ));
    }
    result
}

/// Merges multiple consistency profiles into one.
pub fn merge_consistency_profiles(
    profiles: &[ConsistencyProfile],
) -> ConsistencyProfile {
    // Simplified merge: takes the strongest profile present.
    profiles
        .iter()
        .cloned()
        .max()
        .unwrap_or(ConsistencyProfile::Eventual)
}

/// Checks whether a quorum is met given total and ack counts.
pub fn quorum_met(total: usize, acks: usize) -> bool {
    if total == 0 {
        return false;
    }
    acks > total / 2
}

/// Checks whether a strict quorum (2/3) is met.
pub fn strict_quorum_met(total: usize, acks: usize) -> bool {
    if total == 0 {
        return false;
    }
    acks * 3 > total * 2
}

/// Computes the replication factor needed for a given durability level.
pub fn replication_factor_for(durability: DurabilityCondition) -> u32 {
    match durability {
        DurabilityCondition::Single => 1,
        DurabilityCondition::Double => 2,
        DurabilityCondition::Triple => 3,
        DurabilityCondition::Quorum => 5,
        DurabilityCondition::All => 7,
    }
}

/// Checks whether a node state is active.
pub fn is_node_active(state: NodeState) -> bool {
    matches!(state, NodeState::Active)
}

/// Checks whether a session state is healthy.
pub fn is_session_healthy(state: CircuitState) -> bool {
    matches!(state, CircuitState::Closed)
}

///////////////////////////////////////////////////////////////////////////////
// SECTION 15 — RESOURCE AND CAPACITY UTILITIES
///////////////////////////////////////////////////////////////////////////////

/// Computes available capacity after accounting for overhead.
pub fn available_capacity(total: Capacity, reserved: Capacity) -> Capacity {
    Capacity(total.0.saturating_sub(reserved.0))
}

/// Checks whether a request fits within remaining capacity.
pub fn fits_capacity(remaining: Capacity, requested: Capacity) -> bool {
    requested.0 <= remaining.0
}

/// Computes the degradation level from a failure ratio.
pub fn degradation_from_ratio(failed: u64, total: u64) -> DegradationLevel {
    if total == 0 {
        return DegradationLevel::None;
    }
    let ratio = failed as f64 / total as f64;
    if ratio < 0.05 {
        DegradationLevel::None
    } else if ratio < 0.20 {
        DegradationLevel::Minor
    } else if ratio < 0.50 {
        DegradationLevel::Moderate
    } else {
        DegradationLevel::Severe
    }
}

/// Computes the backoff delay for a given attempt number.
pub fn backoff_delay_ms(attempt: u32, base_ms: u64, max_ms: u64) -> u64 {
    let delay = (base_ms as f64) * 2.0f64.powi(attempt as i32);
    delay.min(max_ms as f64) as u64
}

///////////////////////////////////////////////////////////////////////////////
// SECTION 16 — TIME AND EPOCH UTILITIES
///////////////////////////////////////////////////////////////////////////////

/// Advances an epoch by one.
pub fn advance_epoch(epoch: Epoch) -> Epoch {
    Epoch(epoch.0.wrapping_add(1))
}

/// Checks whether an epoch is expired relative to a reference.
pub fn is_epoch_expired(epoch: Epoch, now: Epoch) -> bool {
    epoch.0 < now.0
}

/// Computes the number of epochs between two epochs.
pub fn epochs_between(a: Epoch, b: Epoch) -> u64 {
    if b.0 >= a.0 {
        b.0 - a.0
    } else {
        a.0 - b.0
    }
}

/// Creates a `TimestampMillis` from the current IDS logical clock.
pub fn ids_now() -> TimestampMillis {
    // Placeholder: downstream crates inject the real clock.
    TimestampMillis(0)
}

///////////////////////////////////////////////////////////////////////////////
// SECTION 17 — SECURITY AND CREDENTIAL UTILITIES
///////////////////////////////////////////////////////////////////////////////

/// Validates that a credential is non-empty.
pub fn is_credential_valid(cred: &Credential) -> bool {
    !cred.0.is_empty()
}

/// Computes a credential fingerprint.
pub fn credential_fingerprint(cred: &Credential) -> [u8; 32] {
    blake3_of(&cred.0)
}

/// Checks whether an auth token is expired (simplified).
pub fn is_auth_token_expired(token: &AuthToken, now: TimestampMillis) -> bool {
    token.expiry <= now.0
}

/// Creates an auth token with a given expiry.
pub fn auth_token_with_expiry(token: AuthToken, expiry: u64) -> AuthToken {
    AuthToken {
        token: token.token,
        expiry,
    }
}

///////////////////////////////////////////////////////////////////////////////
// SECTION 18 — MOBILITY AND HANDOFF UTILITIES
///////////////////////////////////////////////////////////////////////////////

/// Computes the handoff cost between two nodes.
pub fn handoff_cost(from: NodeId, to: NodeId, load: u64) -> u64 {
    let distance = from.distance_to(to);
    distance.wrapping_add(load.wrapping_mul(1000))
}

/// Checks whether a mobility event is within a region.
pub fn mobility_within_region(
    node: NodeId,
    from_region: RegionId,
    to_region: RegionId,
) -> bool {
    node.belongs_to_region(from_region) || node.belongs_to_region(to_region)
}

///////////////////////////////////////////////////////////////////////////////
// SECTION 19 — REPLICATION AND QUORUM UTILITIES
///////////////////////////////////////////////////////////////////////////////

/// Computes the minimum acks needed for a quorum.
pub fn min_acks_for_quorum(total: usize) -> usize {
    if total == 0 {
        return 0;
    }
    total / 2 + 1
}

/// Computes the minimum acks for a strict (2/3) quorum.
pub fn min_acks_for_strict_quorum(total: usize) -> usize {
    if total == 0 {
        return 0;
    }
    (total * 2 + 2) / 3
}

/// Checks whether replication lag is acceptable.
pub fn is_lag_acceptable(lag: u64, max_lag: u64) -> bool {
    lag <= max_lag
}

///////////////////////////////////////////////////////////////////////////////
// SECTION 20 — RE-EXPORT ALL EXTENSION TRAITS
///////////////////////////////////////////////////////////////////////////////

pub use self::{
    NodeIdExt, EndpointExt, MessageExt, ValueExt, OpRefExt,
    OrderingPosExt, SessionIdExt, DatasetIdExt, ObjectIdExt,
};

///////////////////////////////////////////////////////////////////////////////
// SECTION 21 — EXTENSION TRAITS FOR ADDITIONAL TYPES
///////////////////////////////////////////////////////////////////////////////

/// Extension trait for `RotatingNodeId` providing membership rotation operations.
pub trait RotatingNodeIdExt {
    /// Returns the underlying node id bytes.
    fn inner(&self) -> [u8; 32];

    /// Returns true if this rotating id is zero.
    fn is_zero(&self) -> bool;

    /// Advances the rotation by one epoch.
    fn advance(self) -> Self;
}

impl RotatingNodeIdExt for RotatingNodeId {
    fn inner(&self) -> [u8; 32] {
        self.0
    }

    fn is_zero(&self) -> bool {
        self.0 == [0u8; 32]
    }

    fn advance(self) -> Self {
        let hash = blake3::hash(&self.0);
        RotatingNodeId(*hash.as_bytes())
    }
}

/// Extension trait for `OrderingDomain` providing ordering domain operations.
pub trait OrderingDomainExt {
    /// Returns the raw u64 value.
    fn value(&self) -> u64;

    /// Returns true if this is the default domain (0).
    fn is_default(&self) -> bool;

    /// Returns the next domain in sequence.
    fn next(&self) -> Self;
}

impl OrderingDomainExt for OrderingDomain {
    fn value(&self) -> u64 {
        self.0
    }

    fn is_default(&self) -> bool {
        self.0 == 0
    }

    fn next(&self) -> Self {
        OrderingDomain(self.0.wrapping_add(1))
    }
}

/// Extension trait for `SnapshotId` providing snapshot operations.
pub trait SnapshotIdExt {
    /// Returns the raw u64 value.
    fn value(&self) -> u64;

    /// Returns true if this is the initial snapshot (0).
    fn is_initial(&self) -> bool;

    /// Returns the next snapshot id.
    fn next(&self) -> Self;
}

impl SnapshotIdExt for SnapshotId {
    fn value(&self) -> u64 {
        self.0
    }

    fn is_initial(&self) -> bool {
        self.0 == 0
    }

    fn next(&self) -> Self {
        SnapshotId(self.0.wrapping_add(1))
    }
}

/// Extension trait for `CRDTState` providing CRDT state operations.
pub trait CRDTStateExt {
    /// Returns true if the state is empty.
    fn is_empty(&self) -> bool;

    /// Returns the state version.
    fn version(&self) -> u64;

    /// Advances the state version by one.
    fn advance_version(self) -> Self;
}

impl CRDTStateExt for CRDTState {
    fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    fn version(&self) -> u64 {
        self.1
    }

    fn advance_version(self) -> Self {
        CRDTState(self.0, self.1.wrapping_add(1))
    }
}

/// Extension trait for `CRDTOp` providing CRDT operation operations.
pub trait CRDTOpExt {
    /// Returns the operation type tag.
    fn op_type(&self) -> u8;

    /// Returns the operation payload length.
    fn payload_len(&self) -> usize;
}

impl CRDTOpExt for CRDTOp {
    fn op_type(&self) -> u8 {
        self.0
    }

    fn payload_len(&self) -> usize {
        self.1.len()
    }
}

/// Extension trait for `CRDTDelta` providing CRDT delta operations.
pub trait CRDTDeltaExt {
    /// Returns true if the delta is empty.
    fn is_empty(&self) -> bool;

    /// Returns the delta size in bytes.
    fn size(&self) -> usize;
}

impl CRDTDeltaExt for CRDTDelta {
    fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    fn size(&self) -> usize {
        self.0.len()
    }
}

/// Extension trait for `Tier` providing storage tier operations.
pub trait TierExt {
    /// Returns the tier name as a string.
    fn name(&self) -> &'static str;

    /// Returns the tier priority (higher = more important).
    fn priority(&self) -> u32;

    /// Returns true if this is a hot tier.
    fn is_hot(&self) -> bool;
}

impl TierExt for Tier {
    fn name(&self) -> &'static str {
        match self {
            Tier::Hot => "hot",
            Tier::Warm => "warm",
            Tier::Cold => "cold",
            Tier::Archive => "archive",
        }
    }

    fn priority(&self) -> u32 {
        match self {
            Tier::Hot => 4,
            Tier::Warm => 3,
            Tier::Cold => 2,
            Tier::Archive => 1,
        }
    }

    fn is_hot(&self) -> bool {
        matches!(self, Tier::Hot)
    }
}

/// Extension trait for `ComputeRequest` providing compute request operations.
pub trait ComputeRequestExt {
    /// Returns the request payload size.
    fn payload_size(&self) -> usize;

    /// Returns true if the request has no payload.
    fn is_payload_empty(&self) -> bool;
}

impl ComputeRequestExt for ComputeRequest {
    fn payload_size(&self) -> usize {
        self.0.len()
    }

    fn is_payload_empty(&self) -> bool {
        self.0.is_empty()
    }
}

/// Extension trait for `ComputeResult` providing compute result operations.
pub trait ComputeResultExt {
    /// Returns true if the result indicates success.
    fn is_success(&self) -> bool;

    /// Returns the result payload size.
    fn payload_size(&self) -> usize;
}

impl ComputeResultExt for ComputeResult {
    fn is_success(&self) -> bool {
        self.0
    }

    fn payload_size(&self) -> usize {
        self.1.len()
    }
}

/// Extension trait for `CompactResult` providing compaction result operations.
pub trait CompactResultExt {
    /// Returns the number of reclaimed bytes.
    fn reclaimed_bytes(&self) -> u64;

    /// Returns the number of compacted objects.
    fn compacted_objects(&self) -> u64;

    /// Returns true if any space was reclaimed.
    fn reclaimed_any(&self) -> bool;
}

impl CompactResultExt for CompactResult {
    fn reclaimed_bytes(&self) -> u64 {
        self.reclaimed_bytes
    }

    fn compacted_objects(&self) -> u64 {
        self.compacted_objects
    }

    fn reclaimed_any(&self) -> bool {
        self.reclaimed_bytes > 0 || self.compacted_objects > 0
    }
}

/// Extension trait for `Capacity` providing capacity operations.
pub trait CapacityExt {
    /// Returns the raw u64 value.
    fn value(&self) -> u64;

    /// Returns true if capacity is zero.
    fn is_zero(&self) -> bool;

    /// Adds another capacity, saturating at u64::MAX.
    fn saturating_add(self, other: Capacity) -> Capacity;

    /// Subtracts another capacity, saturating at zero.
    fn saturating_sub(self, other: Capacity) -> Capacity;
}

impl CapacityExt for Capacity {
    fn value(&self) -> u64 {
        self.0
    }

    fn is_zero(&self) -> bool {
        self.0 == 0
    }

    fn saturating_add(self, other: Capacity) -> Capacity {
        Capacity(self.0.saturating_add(other.0))
    }

    fn saturating_sub(self, other: Capacity) -> Capacity {
        Capacity(self.0.saturating_sub(other.0))
    }
}

///////////////////////////////////////////////////////////////////////////////
// SECTION 22 — ADDITIONAL COMPOSITION UTILITIES
///////////////////////////////////////////////////////////////////////////////

/// Merges multiple `Value` instances into one by concatenating their bytes.
pub fn merge_values(values: &[Value]) -> Value {
    let mut merged = Vec::new();
    for v in values {
        merged.extend_from_slice(&v.0);
    }
    Value(merged)
}

/// Splits a `Value` into `n` approximately equal chunks.
pub fn split_value(value: &Value, n: usize) -> Vec<Value> {
    if n == 0 {
        return vec![];
    }
    let len = value.0.len();
    let chunk_size = len.div_ceil(n);
    let mut result = Vec::with_capacity(n);
    for i in (0..len).step_by(chunk_size) {
        let end = (i + chunk_size).min(len);
        result.push(Value(value.0[i..end].to_vec()));
    }
    result
}

/// Interleaves multiple `Value` instances byte-by-byte.
pub fn interleave_values(values: &[Value]) -> Value {
    if values.is_empty() {
        return Value(Vec::new());
    }
    let max_len = values.iter().map(|v| v.0.len()).max().unwrap_or(0);
    let mut out = Vec::with_capacity(max_len * values.len());
    for i in 0..max_len {
        for v in values {
            if i < v.0.len() {
                out.push(v.0[i]);
            }
        }
    }
    Value(out)
}

/// Computes the total byte size of a slice of values.
pub fn total_value_size(values: &[Value]) -> usize {
    values.iter().map(|v| v.0.len()).sum()
}

/// Finds the largest value in a slice by byte size.
pub fn largest_value(values: &[Value]) -> Option<&Value> {
    values.iter().max_by_key(|v| v.0.len())
}

/// Filters values that exceed a size threshold.
pub fn filter_values_by_size(values: &[Value], max_size: usize) -> Vec<Value> {
    values
        .iter()
        .filter(|v| v.0.len() <= max_size)
        .cloned()
        .collect()
}

/// Builds a `Message` from a node id, payload, and timestamp.
pub fn build_message(from: NodeId, payload: Value, ts: TimestampMillis) -> Message {
    Message {
        id: MessageId(blake3_keyed(&from.0, &payload.0)[0] as u64),
        payload,
        timestamp: ts,
    }
}

/// Builds a `ChangeStream` entry from an OpRef and value.
pub fn build_change_stream(op: OpRef, value: Value) -> ChangeStream {
    ChangeStream { op_ref: op, value }
}

/// Builds a `FragmentId` from a dataset and object id.
pub fn build_fragment_id(dataset: DatasetId, object: ObjectID) -> FragmentId {
    FragmentId {
        dataset_id: dataset,
        object_id: object,
    }
}

/// Builds a `RosterChange` for adding a node.
pub fn build_roster_add(node: NodeId) -> RosterChange {
    RosterChange::Add { node }
}

/// Builds a `RosterChange` for removing a node.
pub fn build_roster_remove(node: NodeId) -> RosterChange {
    RosterChange::Remove { node }
}

/// Builds a `FailureEvent` from a node id and failure type.
pub fn build_failure_event(node: NodeId, kind: FailureEvent) -> FailureEvent {
    kind
}

/// Computes a checksum over a slice of bytes.
pub fn compute_checksum(data: &[u8]) -> Checksum {
    Checksum(blake3_of(data))
}

/// Verifies a checksum over a slice of bytes.
pub fn verify_checksum(data: &[u8], expected: Checksum) -> bool {
    compute_checksum(data) == expected
}

/// Computes the failure domain hash for a node.
pub fn failure_domain_hash(node: NodeId, domain: FailureDomain) -> [u8; 32] {
    blake3_keyed(&domain.0, &node.0)
}

/// Groups nodes by failure domain.
pub fn group_by_failure_domain(
    nodes: &[NodeId],
    domain: FailureDomain,
) -> Vec<(FailureDomain, Vec<NodeId>)> {
    vec![(domain, nodes.to_vec())]
}

///////////////////////////////////////////////////////////////////////////////
// SECTION 23 — ADDITIONAL VALIDATION MACROS
///////////////////////////////////////////////////////////////////////////////

/// Asserts that a capacity is non-zero.
#[macro_export]
macro_rules! require_nonzero_capacity {
    ($cap:expr) => {
        if $cap.is_zero() {
            panic!("Capacity must not be zero");
        }
    };
}

/// Asserts that a version is within a range.
#[macro_export]
macro_rules! require_version_in_range {
    ($ver:expr, $min:expr, $max:expr) => {
        if $ver < $min || $ver > $max {
            panic!(
                "Version {:?} is outside range [{:?}, {:?}]",
                $ver, $min, $max
            );
        }
    };
}

/// Asserts that two node ids are different.
#[macro_export]
macro_rules! require_distinct_nodes {
    ($a:expr, $b:expr) => {
        if $a == $b {
            panic!("NodeIds must be distinct");
        }
    };
}

/// Asserts that an endpoint uses a specific carrier.
#[macro_export]
macro_rules! require_carrier {
    ($endpoint:expr, $carrier:expr) => {
        if $endpoint.carrier() != $carrier {
            panic!(
                "Endpoint carrier mismatch: expected {:?}, got {:?}",
                $carrier, $endpoint.carrier()
            );
        }
    };
}

/// Asserts that a quorum is met.
#[macro_export]
macro_rules! require_quorum {
    ($total:expr, $acks:expr) => {
        if !quorum_met($total, $acks) {
            panic!(
                "Quorum not met: {} acks out of {} total",
                $acks, $total
            );
        }
    };
}

/// Asserts that a value does not exceed a size limit.
#[macro_export]
macro_rules! require_value_size {
    ($value:expr, $limit:expr) => {
        if $value.byte_len() > $limit {
            panic!(
                "Value size {} exceeds limit {}",
                $value.byte_len(),
                $limit
            );
        }
    };
}

///////////////////////////////////////////////////////////////////////////////
// SECTION 24 — RE-EXPORT ALL ADDITIONAL EXTENSION TRAITS
///////////////////////////////////////////////////////////////////////////////

pub use self::{
    RotatingNodeIdExt, OrderingDomainExt, SnapshotIdExt, CRDTStateExt,
    CRDTOpExt, CRDTDeltaExt, TierExt, ComputeRequestExt, ComputeResultExt,
    CompactResultExt, CapacityExt,
};

///////////////////////////////////////////////////////////////////////////////
// SECTION 25 — OPERATION AND STREAM BUILDERS
///////////////////////////////////////////////////////////////////////////////

/// Builds a `LocalTransaction` from a transaction id and operations.
pub fn build_local_transaction(
    txn_id: TransactionId,
    ops: Vec<Value>,
) -> LocalTransaction {
    LocalTransaction { txn_id, ops }
}

/// Builds a `TransactionId` from a node id and sequence number.
pub fn build_transaction_id(node: NodeId, seq: u64) -> TransactionId {
    let hash = blake3_keyed(&node.0, &seq.to_le_bytes());
    TransactionId(u64::from_le_bytes(hash.as_bytes()[..8].try_into().unwrap_or([0; 8])))
}

/// Builds a `StepId` from a transaction id and step number.
pub fn build_step_id(txn_id: TransactionId, step: u64) -> StepId {
    StepId { txn_id, step }
}

/// Builds a `MobilityError` from a node id and reason.
pub fn build_mobility_error(node: NodeId, reason: String) -> MobilityError {
    MobilityError { node, reason }
}

/// Builds a `TierError` from a tier and reason.
pub fn build_tier_error(tier: Tier, reason: String) -> TierError {
    TierError { tier, reason }
}

/// Builds a `ComputeError` from a request id and reason.
pub fn build_compute_error(request_id: Value, reason: String) -> ComputeError {
    ComputeError { request_id, reason }
}

///////////////////////////////////////////////////////////////////////////////
// SECTION 26 — TOPOLOGY AND PLACEMENT UTILITIES
///////////////////////////////////////////////////////////////////////////////

/// Computes the placement score for a node given load and distance.
pub fn placement_score(load: u64, distance: u64, capacity: Capacity) -> u64 {
    if capacity.is_zero() {
        return 0;
    }
    let load_factor = load.saturating_mul(1000).saturating_div(capacity.value());
    distance.saturating_add(load_factor)
}

/// Ranks nodes by placement score (lower is better).
pub fn rank_nodes_by_placement(
    nodes: &[(NodeId, u64, u64, Capacity)],
) -> Vec<(NodeId, u64)> {
    let mut scored: Vec<(NodeId, u64)> = nodes
        .iter()
        .map(|(node, load, distance, capacity)| {
            (node.clone(), placement_score(*load, *distance, *capacity))
        })
        .collect();
    scored.sort_by_key(|(_, score)| *score);
    scored
}

/// Computes the region affinity score for a node.
pub fn region_affinity_score(node: NodeId, region: RegionId) -> u64 {
    if node.belongs_to_region(region) {
        1000
    } else {
        node.distance_to(NodeId::from_blake3_digest(&blake3::hash(&region.0)))
    }
}

/// Checks whether a placement decision respects a redundancy mode.
pub fn respects_redundancy(
    decision: &PlacementDecision,
    mode: RedundancyMode,
) -> bool {
    let replicas = decision.replica_count();
    match mode {
        RedundancyMode::Single => replicas >= 1,
        RedundancyMode::Double => replicas >= 2,
        RedundancyMode::Triple => replicas >= 3,
        RedundancyMode::Quorum => replicas >= 3,
        RedundancyMode::All => replicas >= 5,
    }
}

/// Computes the optimal replica count for a failure domain count.
pub fn optimal_replica_count(failure_domains: usize) -> usize {
    if failure_domains == 0 {
        return 1;
    }
    (failure_domains * 2 + 1).min(7)
}

/// Balances load across nodes by computing target loads.
pub fn balance_loads(
    total_load: u64,
    node_count: usize,
) -> Vec<(NodeId, u64)> {
    if node_count == 0 {
        return vec![];
    }
    let base = total_load / node_count as u64;
    let remainder = total_load % node_count as u64;
    (0..node_count)
        .map(|i| {
            let node = NodeId::from_blake3_digest(&blake3::hash(&i.to_le_bytes()));
            let load = base + if i < remainder as usize { 1 } else { 0 };
            (node, load)
        })
        .collect()
}

///////////////////////////////////////////////////////////////////////////////
// SECTION 27 — RECOVERY AND CHECKPOINT UTILITIES
///////////////////////////////////////////////////////////////////////////////

/// Builds a `RecoveryScopeLevel` from a durability condition.
pub fn recovery_scope_from_durability(durability: DurabilityCondition) -> RecoveryScopeLevel {
    match durability {
        DurabilityCondition::Single => RecoveryScopeLevel::Local,
        DurabilityCondition::Double => RecoveryScopeLevel::Regional,
        DurabilityCondition::Triple => RecoveryScopeLevel::Regional,
        DurabilityCondition::Quorum => RecoveryScopeLevel::Global,
        DurabilityCondition::All => RecoveryScopeLevel::Global,
    }
}

/// Computes the checkpoint interval for a given durability condition.
pub fn checkpoint_interval_for(durability: DurabilityCondition) -> u64 {
    match durability {
        DurabilityCondition::Single => 60_000,
        DurabilityCondition::Double => 30_000,
        DurabilityCondition::Triple => 15_000,
        DurabilityCondition::Quorum => 5_000,
        DurabilityCondition::All => 1_000,
    }
}

/// Builds a `CompactResult` from compaction statistics.
pub fn build_compact_result(reclaimed: u64, objects: u64) -> CompactResult {
    CompactResult {
        reclaimed_bytes: reclaimed,
        compacted_objects: objects,
    }
}

/// Checks whether compaction is needed based on reclaimable bytes.
pub fn is_compaction_needed(reclaimable: u64, threshold: u64) -> bool {
    reclaimable > threshold
}

///////////////////////////////////////////////////////////////////////////////
// SECTION 28 — BROADCAST AND QUORUM UTILITIES
///////////////////////////////////////////////////////////////////////////////

/// Computes the fanout factor for a broadcast given node count.
pub fn broadcast_fanout(node_count: usize) -> usize {
    if node_count <= 3 {
        node_count
    } else if node_count <= 10 {
        3
    } else {
        (node_count as f64).sqrt() as usize + 1
    }
}

/// Computes the expected ack count for a quorum broadcast.
pub fn expected_acks_for_broadcast(node_count: usize) -> usize {
    if node_count == 0 {
        return 0;
    }
    node_count / 2 + 1
}

/// Checks whether a broadcast has reached quorum.
pub fn broadcast_quorum_reached(node_count: usize, acks: usize) -> bool {
    quorum_met(node_count, acks)
}

///////////////////////////////////////////////////////////////////////////////
// SECTION 29 — SAGA AND COORDINATOR UTILITIES
///////////////////////////////////////////////////////////////////////////////

/// Builds a `SagaId` from a node id and sequence number.
pub fn build_saga_id(node: NodeId, seq: u64) -> SagaId {
    let hash = blake3_keyed(&node.0, &seq.to_le_bytes());
    SagaId(u64::from_le_bytes(hash.as_bytes()[..8].try_into().unwrap_or([0; 8])))
}

/// Builds a `StepId` from a saga id and step number.
pub fn build_saga_step_id(saga_id: SagaId, step: u64) -> StepId {
    StepId {
        txn_id: TransactionId(saga_id.0),
        step,
    }
}

/// Checks whether a saga step is complete.
pub fn is_saga_step_complete(step_state: LifecycleState) -> bool {
    matches!(step_state, LifecycleState::Completed)
}

/// Computes the saga timeout for a given number of steps.
pub fn saga_timeout_for_steps(step_count: u64) -> u64 {
    step_count * 30_000
}

///////////////////////////////////////////////////////////////////////////////
// SECTION 30 — LEASE AND FENCING UTILITIES
///////////////////////////////////////////////////////////////////////////////

/// Builds a `LeaseId` from a node id and timestamp.
pub fn build_lease_id(node: NodeId, ts: TimestampMillis) -> LeaseId {
    let hash = blake3_keyed(&node.0, &ts.0.to_le_bytes());
    LeaseId(u64::from_le_bytes(hash.as_bytes()[..8].try_into().unwrap_or([0; 8])))
}

/// Builds a `FencingToken` from a lease id and sequence number.
pub fn build_fencing_token(lease: LeaseId, seq: u64) -> FencingToken {
    FencingToken(lease.0.wrapping_add(seq))
}

/// Checks whether a fencing token is newer than another.
pub fn is_newer_fencing_token(a: FencingToken, b: FencingToken) -> bool {
    a.0 > b.0
}

/// Computes the lease expiry time from a start time and ttl.
pub fn lease_expiry_from_ttl(start: TimestampMillis, ttl_ms: u64) -> TimestampMillis {
    TimestampMillis(start.0.saturating_add(ttl_ms))
}

///////////////////////////////////////////////////////////////////////////////
// SECTION 31 — BYZANTINE AND FAULT TOLERANCE UTILITIES
///////////////////////////////////////////////////////////////////////////////

/// Computes the maximum number of byzantine nodes tolerable.
pub fn max_byzantine_nodes(total: usize) -> usize {
    if total == 0 {
        return 0;
    }
    (total - 1) / 3
}

/// Computes the minimum honest nodes needed.
pub fn min_honest_nodes(total: usize) -> usize {
    total - max_byzantine_nodes(total)
}

/// Checks whether a vote count meets byzantine quorum.
pub fn byzantine_quorum_met(total: usize, votes: usize) -> bool {
    votes > total * 2 / 3
}

/// Computes the fault tolerance level for a node count.
pub fn fault_tolerance_level(total: usize) -> DegradationLevel {
    let max_b = max_byzantine_nodes(total);
    if max_b == 0 {
        DegradationLevel::None
    } else if max_b <= 2 {
        DegradationLevel::Minor
    } else if max_b <= 5 {
        DegradationLevel::Moderate
    } else {
        DegradationLevel::Severe
    }
}

/// Computes the credibility score for a quorum of given size and total nodes.
pub fn credibility_for_quorum(quorum_size: usize, total: usize) -> f64 {
    if total == 0 {
        return 0.0;
    }
    quorum_size as f64 / total as f64
}

///////////////////////////////////////////////////////////////////////////////
// SECTION 44 — FINAL RE-EXPORTS
///////////////////////////////////////////////////////////////////////////////

/// Computes the erasure coding overhead for a given configuration.
pub fn erasure_overhead(data_shards: usize, parity_shards: usize) -> f64 {
    if data_shards == 0 {
        return 0.0;
    }
    parity_shards as f64 / data_shards as f64
}

/// Computes the total shards for an erasure configuration.
pub fn total_erasure_shards(data_shards: usize, parity_shards: usize) -> usize {
    data_shards + parity_shards
}

/// Checks whether data can be reconstructed from available shards.
pub fn can_reconstruct(
    data_shards: usize,
    parity_shards: usize,
    available: usize,
) -> bool {
    available >= data_shards
}

/// Computes the durability level for a replication factor.
pub fn durability_for_replication(replicas: u32) -> DurabilityCondition {
    match replicas {
        1 => DurabilityCondition::Single,
        2 => DurabilityCondition::Double,
        3 => DurabilityCondition::Triple,
        4..=6 => DurabilityCondition::Quorum,
        _ => DurabilityCondition::All,
    }
}

///////////////////////////////////////////////////////////////////////////////
// SECTION 33 — CRDT AND CONSISTENCY UTILITIES
///////////////////////////////////////////////////////////////////////////////

/// Merges two CRDT deltas by concatenation.
pub fn merge_crdt_deltas(a: CRDTDelta, b: CRDTDelta) -> CRDTDelta {
    let mut merged = a.0;
    merged.extend_from_slice(&b.0);
    CRDTDelta(merged)
}

/// Computes the CRDT state version after a merge.
pub fn merged_crdt_version(a: u64, b: u64) -> u64 {
    a.max(b).wrapping_add(1)
}

/// Checks whether two consistency profiles are compatible.
pub fn are_consistency_profiles_compatible(
    a: ConsistencyProfile,
    b: ConsistencyProfile,
) -> bool {
    a == b || a == ConsistencyProfile::Eventual || b == ConsistencyProfile::Eventual
}

/// Computes the consistency level for a quorum size.
pub fn consistency_for_quorum(quorum_size: usize) -> ConsistencyProfile {
    match quorum_size {
        0 => ConsistencyProfile::Eventual,
        1 => ConsistencyProfile::ReadYourWrites,
        2 => ConsistencyProfile::Causal,
        3 => ConsistencyProfile::Sequential,
        _ => ConsistencyProfile::Strong,
    }
}

///////////////////////////////////////////////////////////////////////////////
// SECTION 34 — NETWORK AND CARRIER UTILITIES (CARRIER-AGNOSTIC)
///////////////////////////////////////////////////////////////////////////////

/// Computes a carrier-specific routing cost.
pub fn carrier_routing_cost(carrier: CarrierType, distance: u64) -> u64 {
    let base = match carrier {
        CarrierType::Direct => 1,
        CarrierType::Mesh => 10,
        CarrierType::StoreAndForward => 100,
        CarrierType::Optical => 5,
        CarrierType::Acoustic => 500,
        CarrierType::Custom(c) => c as u64 * 10,
    };
    base.saturating_add(distance)
}

/// Checks whether two carriers can communicate directly.
pub fn carriers_can_communicate(a: CarrierType, b: CarrierType) -> bool {
    a == b || matches!((a, b), (CarrierType::Direct, _) | (_, CarrierType::Direct))
}

/// Computes the endpoint fingerprint distance.
pub fn endpoint_fingerprint_distance(a: &Endpoint, b: &Endpoint) -> u64 {
    let fa = a.fingerprint();
    let fb = b.fingerprint();
    let mut dist: u64 = 0;
    for i in 0..32 {
        dist = dist.wrapping_add(fa[i].abs_diff(fb[i]) as u64);
    }
    dist
}

/// Builds a routing table entry from an endpoint and cost.
pub fn build_routing_entry(endpoint: Endpoint, cost: u64) -> (Endpoint, u64) {
    (endpoint, cost)
}

/// Sorts routing entries by cost (ascending).
pub fn sort_routing_entries(entries: &mut [(Endpoint, u64)]) {
    entries.sort_by_key(|(_, cost)| *cost);
}

///////////////////////////////////////////////////////////////////////////////
// SECTION 35 — SECURITY AND INTEGRITY UTILITIES
///////////////////////////////////////////////////////////////////////////////

/// Computes an integrity hash for a message.
pub fn message_integrity_hash(msg: &Message) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(&msg.id.0.to_le_bytes());
    hasher.update(&msg.payload.0);
    hasher.update(&msg.timestamp.0.to_le_bytes());
    *hasher.finalize().as_bytes()
}

/// Verifies a message integrity hash.
pub fn verify_message_integrity(msg: &Message, expected: [u8; 32]) -> bool {
    message_integrity_hash(msg) == expected
}

/// Computes a credential rotation hash.
pub fn credential_rotation_hash(cred: &Credential, epoch: Epoch) -> [u8; 32] {
    blake3_keyed(&epoch.0.to_le_bytes(), &cred.0)
}

/// Checks whether a credential needs rotation.
pub fn needs_credential_rotation(
    cred_epoch: Epoch,
    current_epoch: Epoch,
    rotation_interval: u64,
) -> bool {
    epochs_between(cred_epoch, current_epoch) >= rotation_interval
}

///////////////////////////////////////////////////////////////////////////////
// SECTION 36 — FINAL COMPOSITION AND AGGREGATION UTILITIES
///////////////////////////////////////////////////////////////////////////////

/// Aggregates multiple health scores into one.
pub fn aggregate_health_scores(scores: &[HealthScore]) -> HealthScore {
    if scores.is_empty() {
        return HealthScore(0);
    }
    let total: u64 = scores.iter().map(|s| s.0).sum();
    HealthScore(total / scores.len() as u64)
}

/// Computes the overall system health from component healths.
pub fn system_health(components: &[ComponentHealth]) -> HealthScore {
    if components.is_empty() {
        return HealthScore(0);
    }
    let healthy: u64 = components
        .iter()
        .filter(|c| c.healthy)
        .count() as u64;
    healthy * 100 / components.len() as u64
}

/// Builds an `AggregatedErrorReport` from a slice of errors.
pub fn build_error_report(errors: &[Error]) -> AggregatedErrorReport {
    let mut report = AggregatedErrorReport::new();
    for err in errors {
        report.record(err, &ErrorClassifier::new());
    }
    report
}

/// Computes the total error count across multiple reports.
pub fn total_errors_across_reports(reports: &[AggregatedErrorReport]) -> u64 {
    reports.iter().map(|r| r.total_errors()).sum()
}

/// Creates a default `ErrorRouter` with common routes.
pub fn default_error_router() -> ErrorRouter {
    let mut router = ErrorRouter::new();
    router.route(
        ErrorCategory::Transport,
        ErrorSeverity::Critical,
        "transport_handler",
    );
    router.route(
        ErrorCategory::Storage,
        ErrorSeverity::Critical,
        "storage_handler",
    );
    router.route(
        ErrorCategory::Security,
        ErrorSeverity::Fatal,
        "security_handler",
    );
    router
}

/// Creates a default `ErrorBackoffScheduler` for production use.
pub fn default_backoff_scheduler() -> ErrorBackoffScheduler {
    ErrorBackoffScheduler {
        base_ms: 100,
        max_ms: 60_000,
        max_retries: 10,
        multiplier: 2.0,
        jitter: true,
    }
}

/// Creates a default `ErrorRateLimiter` for production use.
pub fn default_error_rate_limiter() -> ErrorRateLimiter {
    ErrorRateLimiter {
        max_per_window: 1000,
        window_ms: 60_000,
        emissions: std::collections::BTreeMap::new(),
        total_emitted: 0,
        total_limited: 0,
    }
}

///////////////////////////////////////////////////////////////////////////////
// SECTION 37 — REPLICATION AND SYNC UTILITIES
///////////////////////////////////////////////////////////////////////////////

/// Computes the replication lag tolerance for a given durability condition.
pub fn replication_lag_tolerance(durability: DurabilityCondition) -> u64 {
    match durability {
        DurabilityCondition::Single => 10_000,
        DurabilityCondition::Double => 5_000,
        DurabilityCondition::Triple => 2_000,
        DurabilityCondition::Quorum => 1_000,
        DurabilityCondition::All => 100,
    }
}

/// Checks whether replication lag exceeds tolerance.
pub fn is_replication_lag_excessive(lag: u64, durability: DurabilityCondition) -> bool {
    lag > replication_lag_tolerance(durability)
}

/// Computes the sync interval for a given durability condition.
pub fn sync_interval_for(durability: DurabilityCondition) -> u64 {
    match durability {
        DurabilityCondition::Single => 30_000,
        DurabilityCondition::Double => 15_000,
        DurabilityCondition::Triple => 5_000,
        DurabilityCondition::Quorum => 2_000,
        DurabilityCondition::All => 500,
    }
}

/// Builds a `ReplicationLag` report entry.
pub fn build_replication_lag_report(node: NodeId, lag_ms: u64) -> (NodeId, u64) {
    (node, lag_ms)
}

/// Computes the average replication lag across nodes.
pub fn average_replication_lag(lags: &[(NodeId, u64)]) -> u64 {
    if lags.is_empty() {
        return 0;
    }
    let total: u64 = lags.iter().map(|(_, lag)| *lag).sum();
    total / lags.len() as u64
}

/// Finds the node with the highest replication lag.
pub fn worst_replication_lag(lags: &[(NodeId, u64)]) -> Option<(NodeId, u64)> {
    lags.iter().max_by_key(|(_, lag)| *lag).cloned()
}

///////////////////////////////////////////////////////////////////////////////
// SECTION 38 — MEMBERSHIP AND ROSTER UTILITIES
///////////////////////////////////////////////////////////////////////////////

/// Computes the membership epoch hash for a roster.
pub fn roster_epoch_hash(roster: &[NodeId], epoch: Epoch) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(&epoch.0.to_le_bytes());
    for node in roster {
        hasher.update(&node.0);
    }
    *hasher.finalize().as_bytes()
}

/// Checks whether a node is a member of a roster.
pub fn is_roster_member(roster: &[NodeId], node: NodeId) -> bool {
    roster.contains(&node)
}

/// Computes the roster size for a given number of regions.
pub fn roster_size_for_regions(region_count: usize) -> usize {
    if region_count == 0 {
        return 0;
    }
    region_count * 3 // 3 nodes per region minimum
}

/// Builds a `RosterChange` for adding multiple nodes.
pub fn build_roster_add_batch(nodes: Vec<NodeId>) -> Vec<RosterChange> {
    nodes.into_iter().map(|n| RosterChange::Add { node: n }).collect()
}

/// Builds a `RosterChange` for removing multiple nodes.
pub fn build_roster_remove_batch(nodes: Vec<NodeId>) -> Vec<RosterChange> {
    nodes.into_iter().map(|n| RosterChange::Remove { node: n }).collect()
}

/// Computes the quorum size for a roster.
pub fn roster_quorum_size(roster_size: usize) -> usize {
    if roster_size == 0 {
        return 0;
    }
    roster_size / 2 + 1
}

///////////////////////////////////////////////////////////////////////////////
// SECTION 39 — STATE MACHINE AND LIFECYCLE UTILITIES
///////////////////////////////////////////////////////////////////////////////

/// Checks whether a lifecycle state is terminal.
pub fn is_lifecycle_terminal(state: LifecycleState) -> bool {
    matches!(state, LifecycleState::Completed | LifecycleState::Failed | LifecycleState::Stopped)
}

/// Checks whether a lifecycle state is active.
pub fn is_lifecycle_active(state: LifecycleState) -> bool {
    matches!(state, LifecycleState::Running | LifecycleState::Starting)
}

/// Computes the next lifecycle state from current and event.
pub fn next_lifecycle_state(current: LifecycleState, event: LifecycleEvent) -> LifecycleState {
    match (current, event) {
        (LifecycleState::Stopped, LifecycleEvent::Start) => LifecycleState::Starting,
        (LifecycleState::Starting, LifecycleEvent::StartComplete) => LifecycleState::Running,
        (LifecycleState::Running, LifecycleEvent::Stop) => LifecycleState::Stopping,
        (LifecycleState::Stopping, LifecycleEvent::StopComplete) => LifecycleState::Stopped,
        (LifecycleState::Running, LifecycleEvent::Complete) => LifecycleState::Completed,
        (LifecycleState::Running, LifecycleEvent::Fail) => LifecycleState::Failed,
        _ => current,
    }
}

/// Builds a `LifecycleState` transition log entry.
pub fn build_lifecycle_transition(
    node: NodeId,
    from: LifecycleState,
    to: LifecycleState,
    ts: TimestampMillis,
) -> (NodeId, LifecycleState, LifecycleState, TimestampMillis) {
    (node, from, to, ts)
}

///////////////////////////////////////////////////////////////////////////////
// SECTION 40 — ANOMALY AND FAILURE DETECTION UTILITIES
///////////////////////////////////////////////////////////////////////////////

/// Computes an anomaly score from failure metrics.
pub fn anomaly_score(failures: u64, total: u64, latency_p99: u64) -> u64 {
    if total == 0 {
        return 0;
    }
    let failure_rate = failures * 1000 / total;
    let latency_factor = latency_p99 / 100;
    failure_rate.saturating_add(latency_factor)
}

/// Classifies an anomaly score into a degradation level.
pub fn classify_anomaly(score: u64) -> DegradationLevel {
    if score < 10 {
        DegradationLevel::None
    } else if score < 50 {
        DegradationLevel::Minor
    } else if score < 200 {
        DegradationLevel::Moderate
    } else {
        DegradationLevel::Severe
    }
}

/// Builds an `Anomaly` record from a node id and score.
pub fn build_anomaly(node: NodeId, score: u64, description: String) -> Anomaly {
    Anomaly { node, score, description }
}

/// Checks whether an anomaly requires intervention.
pub fn anomaly_requires_intervention(score: u64) -> bool {
    score >= 200
}

///////////////////////////////////////////////////////////////////////////////
// SECTION 41 — CHECKSUM AND INTEGRITY UTILITIES
///////////////////////////////////////////////////////////////////////////////

/// Computes a checksum boundary for a data range.
pub fn checksum_boundary(start: u64, end: u64, data: &[u8]) -> Checksum {
    let mut hasher = blake3::Hasher::new();
    hasher.update(&start.to_le_bytes());
    hasher.update(&end.to_le_bytes());
    hasher.update(data);
    Checksum(*hasher.finalize().as_bytes())
}

/// Verifies a checksum boundary.
pub fn verify_checksum_boundary(
    start: u64,
    end: u64,
    data: &[u8],
    expected: Checksum,
) -> bool {
    checksum_boundary(start, end, data) == expected
}

/// Computes a rolling checksum over a slice of chunks.
pub fn rolling_checksum(chunks: &[&[u8]]) -> Vec<Checksum> {
    chunks.iter().map(|c| Checksum(blake3_of(c))).collect()
}

/// Merges multiple checksums into one.
pub fn merge_checksums(checksums: &[Checksum]) -> Checksum {
    if checksums.is_empty() {
        return Checksum([0u8; 32]);
    }
    let mut hasher = blake3::Hasher::new();
    for c in checksums {
        hasher.update(&c.0);
    }
    Checksum(*hasher.finalize().as_bytes())
}

///////////////////////////////////////////////////////////////////////////////
// SECTION 42 — DELIVERY AND ACK UTILITIES
///////////////////////////////////////////////////////////////////////////////

/// Computes the delivery timeout for a given semantics level.
pub fn delivery_timeout_for(semantics: DeliverySemantics) -> u64 {
    match semantics {
        DeliverySemantics::AtMostOnce => 5_000,
        DeliverySemantics::AtLeastOnce => 10_000,
        DeliverySemantics::ExactlyOnce => 30_000,
    }
}

/// Checks whether an ack state is final.
pub fn is_ack_state_final(state: AckState) -> bool {
    matches!(state, AckState::Delivered | AckState::Failed | AckState::Expired)
}

/// Computes the retry count for a given ack state.
pub fn retry_count_for_ack(state: AckState) -> u32 {
    match state {
        AckState::Pending => 0,
        AckState::Retrying(n) => n,
        AckState::Delivered => 0,
        AckState::Failed => u32::MAX,
        AckState::Expired => u32::MAX,
    }
}

/// Builds an `AckState` transition.
pub fn build_ack_transition(from: AckState, to: AckState) -> (AckState, AckState) {
    (from, to)
}

///////////////////////////////////////////////////////////////////////////////
// SECTION 43 — CIRCUIT BREAKER UTILITIES
///////////////////////////////////////////////////////////////////////////////

/// Computes the circuit breaker reset timeout.
pub fn circuit_breaker_reset_timeout(failures: u64) -> u64 {
    1_000 * failures.min(60)
}

/// Checks whether a circuit breaker should trip.
pub fn should_trip_circuit_breaker(failures: u64, threshold: u64) -> bool {
    failures >= threshold
}

/// Computes the half-open probe count for a circuit breaker.
pub fn half_open_probe_count(failures: u64) -> u32 {
    (failures as u32 / 5 + 1).min(10)
}

///////////////////////////////////////////////////////////////////////////////
// SECTION 44 — FINAL RE-EXPORTS
///////////////////////////////////////////////////////////////////////////////

pub use self::{
    aggregate_health_scores, are_consistency_profiles_compatible,
    average_replication_lag, balance_loads, broadcast_fanout,
    broadcast_quorum_reached, build_ack_transition, build_anomaly,
    build_checksum_boundary, build_compact_result, build_compute_error,
    build_error_report, build_failure_event, build_failure_event_typed,
    build_fencing_token, build_fragment_id, build_lifecycle_transition,
    build_lease_id, build_local_transaction, build_mobility_error,
    build_replication_lag_report, build_routing_entry, build_roster_add,
    build_roster_add_batch, build_roster_remove, build_roster_remove_batch,
    build_saga_id, build_saga_step_id, build_step_id, build_transaction_id,
    can_reconstruct, carriers_can_communicate, checkpoint_interval_for,
    classify_anomaly, consistency_for_quorum, credibility_for_quorum,
    default_backoff_scheduler, default_error_router, default_error_rate_limiter,
    durability_for_replication, endpoint_fingerprint_distance,
    erasure_overhead, expected_acks_for_broadcast, fault_tolerance_level,
    group_by_failure_domain, is_ack_state_final, is_compaction_needed,
    is_lifecycle_active, is_lifecycle_terminal, is_newer_fencing_token,
    is_replication_lag_excessive, is_roster_member, lease_expiry_from_ttl,
    max_byzantine_nodes, message_integrity_hash, merge_crdt_deltas,
    merge_checksums, min_honest_nodes, mobility_within_region,
    next_lifecycle_state, optimal_replica_count, placement_score,
    rank_nodes_by_placement, region_affinity_score, respects_redundancy,
    rolling_checksum, split_value, system_health, total_errors_across_reports,
    total_value_size, verify_checksum, verify_checksum_boundary,
    verify_message_integrity, worst_replication_lag,
};

pub use self::{
    broadcast_quorum_reached as broadcast_quorum_reached_alias,
    can_reconstruct as can_reconstruct_alias,
    carriers_can_communicate as carriers_can_communicate_alias,
    is_roster_member as is_roster_member_alias,
    respects_redundancy as respects_redundancy_alias,
    split_value as split_value_alias,
    verify_checksum as verify_checksum_alias,
    verify_message_integrity as verify_message_integrity_alias,
};

///////////////////////////////////////////////////////////////////////////////
// SECTION 45 — FINAL PRODUCTION UTILITIES
///////////////////////////////////////////////////////////////////////////////

/// Computes the total storage overhead for a given replication and erasure config.
pub fn total_storage_overhead(replicas: u32, data_shards: usize, parity_shards: usize) -> f64 {
    let replica_factor = replicas as f64;
    let erasure_factor = if data_shards == 0 {
        1.0
    } else {
        (data_shards + parity_shards) as f64 / data_shards as f64
    };
    replica_factor * erasure_factor
}

/// Computes the read amplification for a given consistency level.
pub fn read_amplification(consistency: ConsistencyProfile) -> u32 {
    match consistency {
        ConsistencyProfile::Eventual => 1,
        ConsistencyProfile::ReadYourWrites => 1,
        ConsistencyProfile::Causal => 2,
        ConsistencyProfile::Sequential => 3,
        ConsistencyProfile::Strong => 5,
    }
}

/// Computes the write amplification for a given redundancy mode.
pub fn write_amplification(redundancy: RedundancyMode) -> u32 {
    match redundancy {
        RedundancyMode::Single => 1,
        RedundancyMode::Double => 2,
        RedundancyMode::Triple => 3,
        RedundancyMode::Quorum => 3,
        RedundancyMode::All => 7,
    }
}

/// Computes the network hops for a given topology size.
pub fn network_hops_for_topology(node_count: usize) -> u32 {
    if node_count <= 1 {
        return 0;
    }
    (node_count as f64).log2().ceil() as u32
}

/// Checks whether a node count supports a given redundancy mode.
pub fn supports_redundancy(node_count: usize, mode: RedundancyMode) -> bool {
    let required = match mode {
        RedundancyMode::Single => 1,
        RedundancyMode::Double => 2,
        RedundancyMode::Triple => 3,
        RedundancyMode::Quorum => 3,
        RedundancyMode::All => 7,
    };
    node_count >= required
}

/// Computes the failure tolerance for a given redundancy mode.
pub fn failure_tolerance_for_redundancy(mode: RedundancyMode) -> u32 {
    match mode {
        RedundancyMode::Single => 0,
        RedundancyMode::Double => 1,
        RedundancyMode::Triple => 1,
        RedundancyMode::Quorum => 1,
        RedundancyMode::All => 3,
    }
}

/// Builds a `PlacementDecision` from a node id and score.
pub fn build_placement_decision(node: NodeId, score: u64) -> PlacementDecision {
    PlacementDecision {
        node,
        score,
        replicas: vec![node],
    }
}

/// Computes the total score across multiple placement decisions.
pub fn total_placement_score(decisions: &[PlacementDecision]) -> u64 {
    decisions.iter().map(|d| d.score).sum()
}

/// Finds the best placement decision (lowest score).
pub fn best_placement_decision(decisions: &[PlacementDecision]) -> Option<&PlacementDecision> {
    decisions.iter().min_by_key(|d| d.score)
}

/// Checks whether a placement decision is acceptable given a max score.
pub fn is_placement_acceptable(decision: &PlacementDecision, max_score: u64) -> bool {
    decision.score <= max_score
}

/// Computes the change stream length for a given op range.
pub fn change_stream_length(start: OpRef, end: OpRef) -> u64 {
    if end.0 >= start.0 {
        end.0 - start.0 + 1
    } else {
        0
    }
}

/// Checks whether an op ref is within a change stream range.
pub fn is_op_in_stream(op: OpRef, start: OpRef, end: OpRef) -> bool {
    op.0 >= start.0 && op.0 <= end.0
}

/// Builds a `FragmentId` from a dataset and object id with a version.
pub fn build_versioned_fragment_id(
    dataset: DatasetId,
    object: ObjectID,
    version: Version,
) -> (FragmentId, Version) {
    (
        FragmentId {
            dataset_id: dataset,
            object_id: object,
        },
        version,
    )
}

/// Computes the fragment id hash.
pub fn fragment_id_hash(fragment: &FragmentId) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(&fragment.dataset_id.0);
    hasher.update(&fragment.object_id.0);
    *hasher.finalize().as_bytes()
}

/// Checks whether two fragment ids belong to the same dataset.
pub fn fragments_same_dataset(a: &FragmentId, b: &FragmentId) -> bool {
    a.dataset_id == b.dataset_id
}

/// Computes the total replication overhead across all nodes.
pub fn total_replication_overhead(replicas: u32, node_count: usize) -> f64 {
    if node_count == 0 {
        return 0.0;
    }
    replicas as f64 / node_count as f64
}

/// Computes the consistency overhead for a given profile.
pub fn consistency_overhead(profile: ConsistencyProfile) -> u32 {
    match profile {
        ConsistencyProfile::Eventual => 1,
        ConsistencyProfile::ReadYourWrites => 2,
        ConsistencyProfile::Causal => 3,
        ConsistencyProfile::Sequential => 4,
        ConsistencyProfile::Strong => 5,
    }
}

/// Checks whether a node is the primary for a given key.
pub fn is_primary_for_key(node: NodeId, key: &[u8], replicas: &[NodeId]) -> bool {
    if replicas.is_empty() {
        return false;
    }
    let hash = blake3_keyed(key, &node.0);
    let idx = hash.as_bytes()[0] as usize % replicas.len();
    replicas[idx] == node
}

/// Computes the primary node for a given key and replica set.
pub fn primary_node_for_key(key: &[u8], replicas: &[NodeId]) -> Option<NodeId> {
    if replicas.is_empty() {
        return None;
    }
    let hash = blake3_keyed(key, &replicas[0].0);
    let idx = hash.as_bytes()[0] as usize % replicas.len();
    Some(replicas[idx])
}

/// Computes the replica placement for a key across replicas.
pub fn replica_placement_for_key(key: &[u8], replicas: &[NodeId]) -> Vec<NodeId> {
    if replicas.is_empty() {
        return vec![];
    }
    let mut placement = replicas.to_vec();
    placement.sort_by(|a, b| {
        let ha = blake3_keyed(key, &a.0);
        let hb = blake3_keyed(key, &b.0);
        ha.as_bytes().cmp(hb.as_bytes())
    });
    placement
}

/// Builds a `RecoveryScopeLevel` from a fault tolerance level.
pub fn recovery_scope_from_fault_tolerance(
    level: DegradationLevel,
) -> RecoveryScopeLevel {
    match level {
        DegradationLevel::None => RecoveryScopeLevel::Local,
        DegradationLevel::Minor => RecoveryScopeLevel::Regional,
        DegradationLevel::Moderate => RecoveryScopeLevel::Regional,
        DegradationLevel::Severe => RecoveryScopeLevel::Global,
    }
}

/// Computes the retry deadline from a start time and timeout.
pub fn retry_deadline(start: TimestampMillis, timeout_ms: u64) -> TimestampMillis {
    TimestampMillis(start.0.saturating_add(timeout_ms))
}

/// Checks whether a retry deadline has passed.
pub fn is_retry_deadline_passed(deadline: TimestampMillis, now: TimestampMillis) -> bool {
    now.0 >= deadline.0
}

///////////////////////////////////////////////////////////////////////////////
// TESTS
///////////////////////////////////////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn node_id_ext_is_zero() {
        let zero = NodeId::ZERO;
        assert!(zero.is_zero_node());
        let non_zero = NodeId::new([1u8; 32]);
        assert!(!non_zero.is_zero_node());
    }

    #[test]
    fn node_id_ext_short_hex() {
        let node = NodeId::new([0xab, 0xcd, 0xef, 0x01, 0x23, 0x45, 0x67, 0x89, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        let short = node.short_hex();
        assert_eq!(short, "abcdef0123456789");
    }

    #[test]
    fn node_id_ext_region_hash() {
        let node = NodeId::new([42u8; 32]);
        let hash = node.region_hash();
        assert_ne!(hash, [0u8; 8]);
    }

    #[test]
    fn node_id_ext_belongs_to_region() {
        let node = NodeId::new([1u8; 32]);
        let region = RegionId::new([0u8; 32]);
        // Region hash of node won't match zero region generally
        assert!(!node.belongs_to_region(region));
    }

    #[test]
    fn node_id_ext_distance_to() {
        let a = NodeId::new([0u8; 32]);
        let b = NodeId::new([1u8; 32]);
        let dist = a.distance_to(b);
        assert_eq!(dist, b.distance_to(a));
        assert_eq!(a.distance_to(a), 0);
    }

    #[test]
    fn node_id_ext_in_range() {
        let start = NodeId::new([10u8; 32]);
        let end = NodeId::new([20u8; 32]);
        let inside = NodeId::new([15u8; 32]);
        assert!(inside.in_range(start, end));
        assert!(!NodeId::new([5u8; 32]).in_range(start, end));
    }

    #[test]
    fn node_id_ext_rotate() {
        let node = NodeId::new([7u8; 32]);
        let rotated = node.rotate(Epoch(1));
        assert_ne!(rotated.0, [0u8; 32]);
    }

    #[test]
    fn node_id_ext_from_blake3_digest() {
        let digest = blake3::hash(b"test-node");
        let node = NodeId::from_blake3_digest(&digest);
        assert_ne!(node, NodeId::ZERO);
    }

    #[test]
    fn endpoint_ext_carrier_and_payload() {
        let ep = Endpoint::new_endpoint(CarrierType::Direct, vec![1, 2, 3]);
        assert_eq!(ep.carrier(), CarrierType::Direct);
        assert_eq!(ep.payload(), &[1, 2, 3]);
        assert_eq!(ep.payload_len(), 3);
        assert!(!ep.is_empty_payload());
    }

    #[test]
    fn endpoint_ext_empty() {
        let ep = Endpoint::new_endpoint(CarrierType::Mesh, Vec::new());
        assert!(ep.is_empty_payload());
        assert_eq!(ep.payload_len(), 0);
    }

    #[test]
    fn endpoint_ext_with_prefix() {
        let ep = Endpoint::new_endpoint(CarrierType::Direct, vec![3, 4]);
        let prefixed = ep.with_prefix(&[1, 2]);
        assert_eq!(prefixed.payload(), &[1, 2, 3, 4]);
    }

    #[test]
    fn endpoint_ext_strip_prefix() {
        let ep = Endpoint::new_endpoint(CarrierType::Direct, vec![1, 2, 3, 4]);
        let stripped = ep.strip_prefix(&[1, 2]);
        assert!(stripped.is_some());
        assert_eq!(stripped.unwrap().payload(), &[3, 4]);

        let no_strip = ep.strip_prefix(&[9, 9]);
        assert!(no_strip.is_none());
    }

    #[test]
    fn endpoint_ext_fingerprint() {
        let ep = Endpoint::new_endpoint(CarrierType::Direct, vec![1, 2, 3]);
        let fp = ep.fingerprint();
        assert_ne!(fp, [0u8; 32]);
    }

    #[test]
    fn endpoint_ext_carrier_compatible() {
        let a = Endpoint::new_endpoint(CarrierType::Direct, vec![1]);
        let b = Endpoint::new_endpoint(CarrierType::Direct, vec![2]);
        let c = Endpoint::new_endpoint(CarrierType::Mesh, vec![3]);
        assert!(a.carrier_compatible(&b));
        assert!(!a.carrier_compatible(&c));
    }

    #[test]
    fn message_ext_lifecycle() {
        let msg = Message {
            id: MessageId(1),
            payload: Value(vec![1, 2, 3]),
            timestamp: TimestampMillis(100),
        };
        assert!(!msg.is_empty_payload());
        assert_eq!(msg.payload_len(), 3);
        assert!(msg.older_than(TimestampMillis(200)));
        assert!(!msg.older_than(TimestampMillis(50)));

        let hash = msg.payload_hash();
        assert_ne!(hash, [0u8; 32]);

        let reply = msg.reply(Value(vec![9]));
        assert_eq!(reply.id.0, 2);
        assert_eq!(reply.payload_len(), 1);

        assert_eq!(msg.age_ms(TimestampMillis(200)), 100);
    }

    #[test]
    fn message_ext_empty_payload() {
        let msg = Message {
            id: MessageId(1),
            payload: Value(Vec::new()),
            timestamp: TimestampMillis(0),
        };
        assert!(msg.is_empty_payload());
        assert!(msg.is_idempotent());
    }

    #[test]
    fn value_ext_operations() {
        let val = Value(vec![1, 2, 3, 4, 5]);
        assert!(!val.is_empty());
        assert_eq!(val.byte_len(), 5);

        let digest = val.digest();
        assert_ne!(digest, [0u8; 32]);

        assert!(!val.exceeds(10));
        assert!(val.exceeds(3));

        let chunks = val.chunk(2);
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0].byte_len(), 2);
        assert_eq!(chunks[2].byte_len(), 1);
    }

    #[test]
    fn value_ext_empty() {
        let val = Value(Vec::new());
        assert!(val.is_empty());
        assert_eq!(val.byte_len(), 0);
    }

    #[test]
    fn op_ref_ext_operations() {
        let op = OpRef(5);
        assert_eq!(op.value(), 5);
        assert!(!op.is_initial());
        assert_eq!(op.next().value(), 6);
        assert_eq!(op.distance_to(OpRef(10)), 5);
        assert!(op.precedes(OpRef(10)));
        assert!(!OpRef(10).precedes(op));
    }

    #[test]
    fn op_ref_ext_initial() {
        let op = OpRef(0);
        assert!(op.is_initial());
    }

    #[test]
    fn ordering_pos_ext_operations() {
        let pos = OrderingPos(100);
        assert_eq!(pos.value(), 100);
        assert!(!pos.is_beginning());
        assert_eq!(pos.next().value(), 101);
        assert_eq!(pos.distance_to(OrderingPos(150)), 50);
    }

    #[test]
    fn ordering_pos_ext_beginning() {
        let pos = OrderingPos(0);
        assert!(pos.is_beginning());
    }

    #[test]
    fn session_id_ext_operations() {
        let sess = SessionID::new([5u8; 16]);
        assert!(!sess.is_zero());
        assert_eq!(sess.short_id().len(), 8);

        let zero = SessionID::ZERO;
        assert!(zero.is_zero());
    }

    #[test]
    fn session_id_ext_same_region() {
        let a = SessionID::new([1u8, 2, 3, 4, 5, 6, 7, 8, 0, 0, 0, 0, 0, 0, 0, 0]);
        let b = SessionID::new([1u8, 2, 3, 4, 5, 6, 7, 8, 9, 9, 9, 9, 9, 9, 9, 9]);
        assert!(a.same_region(b));
    }

    #[test]
    fn dataset_id_ext_operations() {
        let ds = DatasetId::new([3u8; 16]);
        assert!(!ds.is_zero());
        let hash = ds.hash();
        assert_ne!(hash, [0u8; 32]);

        let zero = DatasetId::ZERO;
        assert!(zero.is_zero());
    }

    #[test]
    fn dataset_id_ext_belongs_to_region() {
        let ds = DatasetId::new([1u8, 2, 3, 4, 5, 6, 7, 8, 0, 0, 0, 0, 0, 0, 0, 0]);
        let region = RegionId::new([0u8; 32]);
        assert!(!ds.belongs_to_region(region));
    }

    #[test]
    fn object_id_ext_operations() {
        let obj = ObjectID::new([9u8; 16]);
        assert!(!obj.is_zero());
        let hash = obj.hash();
        assert_ne!(hash, [0u8; 32]);
    }

    #[test]
    fn conversion_helpers() {
        let node = NodeId::new([1u8; 32]);
        let hex = node_id_to_hex(node);
        assert_eq!(hex.len(), 64);
        let back = hex_to_node_id(&hex).unwrap();
        assert_eq!(back, node);

        let ep = Endpoint::new_endpoint(CarrierType::Direct, vec![1, 2]);
        let bytes = endpoint_to_bytes(&ep);
        let restored = bytes_to_endpoint(&bytes).unwrap();
        assert_eq!(restored.carrier, ep.carrier);
        assert_eq!(restored.address, ep.address);

        assert!(bytes_to_endpoint(&[]).is_none());
    }

    #[test]
    fn blake3_helpers() {
        let h1 = blake3_of(b"hello");
        let h2 = blake3_of(b"hello");
        assert_eq!(h1, h2);

        let h3 = blake3_of(b"world");
        assert_ne!(h1, h3);

        let keyed = blake3_keyed(b"key", b"data");
        assert_ne!(keyed, [0u8; 32]);
    }

    #[test]
    fn node_id_from_bytes_helpers() {
        let bytes = [8u8; 32];
        let node = node_id_from_bytes(bytes);
        assert_eq!(node.as_bytes(), &bytes);

        let ds_bytes = [7u8; 16];
        let ds = dataset_id_from_bytes(ds_bytes);
        assert_eq!(ds.0, ds_bytes);

        let reg_bytes = [6u8; 32];
        let region = region_id_from_bytes(reg_bytes);
        assert_eq!(region.0, reg_bytes);
    }

    #[test]
    fn timestamp_millis_helper() {
        let ts = timestamp_millis(12345);
        assert_eq!(ts.0, 12345);
    }

    #[test]
    fn validation_macros() {
        let node = NodeId::new([1u8; 32]);
        require_nonzero_node!(node);

        let ep = Endpoint::new_endpoint(CarrierType::Direct, vec![1]);
        require_nonempty_endpoint!(ep);

        invariant_holds!(true, "test invariant");
        require_compatible_version!(ProtocolVersion { major: 2, minor: 0, patch: 0 }, ProtocolVersion { major: 1, minor: 0, patch: 0 });
    }

    #[test]
    fn composite_node_id() {
        let nodes = vec![
            NodeId::new([1u8; 32]),
            NodeId::new([2u8; 32]),
            NodeId::new([3u8; 32]),
        ];
        let composite = composite_node_id(&nodes);
        assert_ne!(composite, NodeId::ZERO);

        let empty = composite_node_id(&[]);
        assert_eq!(empty, NodeId::ZERO);
    }

    #[test]
    fn routing_path_and_filter() {
        let eps = vec![
            Endpoint::new_endpoint(CarrierType::Direct, vec![1]),
            Endpoint::new_endpoint(CarrierType::Mesh, vec![2]),
            Endpoint::new_endpoint(CarrierType::Direct, vec![3]),
        ];
        let path = routing_path(&eps);
        assert_eq!(path.len(), 3);

        let direct = filter_by_carrier(&eps, CarrierType::Direct);
        assert_eq!(direct.len(), 2);

        let mesh = filter_by_carrier(&eps, CarrierType::Mesh);
        assert_eq!(mesh.len(), 1);
    }

    #[test]
    fn sort_and_dedup_endpoints() {
        let mut eps = vec![
            Endpoint::new_endpoint(CarrierType::Mesh, vec![1, 2, 3]),
            Endpoint::new_endpoint(CarrierType::Direct, vec![1]),
        ];
        sort_endpoints(&mut eps);
        assert_eq!(eps[0].carrier, CarrierType::Direct);

        let mut dup = vec![
            Endpoint::new_endpoint(CarrierType::Direct, vec![1]),
            Endpoint::new_endpoint(CarrierType::Direct, vec![1]),
        ];
        dedup_endpoints(&mut dup);
        assert_eq!(dup.len(), 1);
    }

    #[test]
    fn partition_dataset_range() {
        let start = DatasetId::new([1u8; 16]);
        let end = DatasetId::new([9u8; 16]);
        let parts = partition_dataset_range(start, end, 4);
        assert_eq!(parts.len(), 4);
    }

    #[test]
    fn merge_consistency_profiles() {
        let profiles = vec![
            ConsistencyProfile::Eventual,
            ConsistencyProfile::Strong,
        ];
        let merged = merge_consistency_profiles(&profiles);
        assert_eq!(merged, ConsistencyProfile::Strong);

        let empty: Vec<ConsistencyProfile> = vec![];
        let merged_empty = merge_consistency_profiles(&empty);
        assert_eq!(merged_empty, ConsistencyProfile::Eventual);
    }

    #[test]
    fn quorum_checks() {
        assert!(quorum_met(5, 3));
        assert!(!quorum_met(5, 2));
        assert!(!quorum_met(0, 0));

        assert!(strict_quorum_met(3, 2));
        assert!(!strict_quorum_met(3, 1));
    }

    #[test]
    fn replication_factor_for_durability() {
        assert_eq!(replication_factor_for(DurabilityCondition::Single), 1);
        assert_eq!(replication_factor_for(DurabilityCondition::Double), 2);
        assert_eq!(replication_factor_for(DurabilityCondition::Triple), 3);
        assert_eq!(replication_factor_for(DurabilityCondition::Quorum), 5);
        assert_eq!(replication_factor_for(DurabilityCondition::All), 7);
    }

    #[test]
    fn node_and_session_state_checks() {
        assert!(is_node_active(NodeState::Active));
        assert!(!is_node_active(NodeState::Failed));

        assert!(is_session_healthy(CircuitState::Closed));
        assert!(!is_session_healthy(CircuitState::Open));
    }

    #[test]
    fn capacity_utilities() {
        let total = Capacity(1000);
        let reserved = Capacity(200);
        let avail = available_capacity(total, reserved);
        assert_eq!(avail.0, 800);

        assert!(fits_capacity(Capacity(500), Capacity(300)));
        assert!(!fits_capacity(Capacity(100), Capacity(200)));
    }

    #[test]
    fn degradation_level() {
        assert_eq!(degradation_from_ratio(0, 100), DegradationLevel::None);
        assert_eq!(degradation_from_ratio(5, 100), DegradationLevel::None);
        assert_eq!(degradation_from_ratio(10, 100), DegradationLevel::Minor);
        assert_eq!(degradation_from_ratio(30, 100), DegradationLevel::Moderate);
        assert_eq!(degradation_from_ratio(80, 100), DegradationLevel::Severe);
        assert_eq!(degradation_from_ratio(0, 0), DegradationLevel::None);
    }

    #[test]
    fn backoff_delay() {
        let d0 = backoff_delay_ms(0, 100, 10000);
        let d1 = backoff_delay_ms(1, 100, 10000);
        let d2 = backoff_delay_ms(2, 100, 10000);
        assert!(d1 > d0);
        assert!(d2 > d1);

        let capped = backoff_delay_ms(100, 100, 1000);
        assert_eq!(capped, 1000);
    }

    #[test]
    fn epoch_utilities() {
        let e = Epoch(5);
        assert_eq!(advance_epoch(e).0, 6);
        assert!(is_epoch_expired(Epoch(1), Epoch(10)));
        assert!(!is_epoch_expired(Epoch(10), Epoch(5)));
        assert_eq!(epochs_between(Epoch(10), Epoch(20)), 10);
        assert_eq!(epochs_between(Epoch(20), Epoch(10)), 10);
    }

    #[test]
    fn ids_now() {
        let ts = ids_now();
        assert_eq!(ts.0, 0);
    }

    #[test]
    fn credential_utilities() {
        let cred = Credential(vec![1, 2, 3]);
        assert!(is_credential_valid(&cred));

        let empty = Credential(Vec::new());
        assert!(!is_credential_valid(&empty));

        let fp = credential_fingerprint(&cred);
        assert_ne!(fp, [0u8; 32]);
    }

    #[test]
    fn auth_token_expiry() {
        let token = AuthToken {
            token: Value(vec![1]),
            expiry: 100,
        };
        assert!(!is_auth_token_expired(&token, TimestampMillis(50)));
        assert!(is_auth_token_expired(&token, TimestampMillis(200)));

        let extended = auth_token_with_expiry(token, 500);
        assert_eq!(extended.expiry, 500);
    }

    #[test]
    fn handoff_cost() {
        let from = NodeId::new([1u8; 32]);
        let to = NodeId::new([2u8; 32]);
        let cost = handoff_cost(from, to, 10);
        assert_ne!(cost, 0);
    }

    #[test]
    fn mobility_within_region() {
        let node = NodeId::new([1u8; 32]);
        let from = RegionId::new([0u8; 32]);
        let to = RegionId::new([0u8; 32]);
        // Node's region hash won't match zero region generally
        assert!(!mobility_within_region(node, from, to));
    }

    #[test]
    fn min_acks_for_quorum() {
        assert_eq!(min_acks_for_quorum(0), 0);
        assert_eq!(min_acks_for_quorum(5), 3);
        assert_eq!(min_acks_for_quorum(4), 3);
    }

    #[test]
    fn min_acks_for_strict_quorum() {
        assert_eq!(min_acks_for_strict_quorum(0), 0);
        assert_eq!(min_acks_for_strict_quorum(3), 2);
        assert_eq!(min_acks_for_strict_quorum(6), 4);
    }

    #[test]
    fn is_lag_acceptable() {
        assert!(is_lag_acceptable(5, 10));
        assert!(!is_lag_acceptable(15, 10));
    }

    #[test]
    fn prelude_re_exports_compile() {
        // Verify all key types are accessible from prelude
        let _node: NodeId = NodeId::ZERO;
        let _region: RegionId = RegionId::ZERO;
        let _dataset: DatasetId = DatasetId::ZERO;
        let _obj: ObjectID = ObjectID::ZERO;
        let _sess: SessionID = SessionID::ZERO;
        let _ep: Endpoint = Endpoint::new_endpoint(CarrierType::Direct, Vec::new());
        let _msg: Message = Message {
            id: MessageId(0),
            payload: Value(Vec::new()),
            timestamp: TimestampMillis(0),
        };
        let _val: Value = Value(Vec::new());
        let _op: OpRef = OpRef(0);
        let _pos: OrderingPos = OrderingPos(0);
        let _err: Error = Error::Internal("test".into());
        let _ver: ProtocolVersion = ProtocolVersion::new(1, 0, 0);
        let _bits: EdgeCaseBits = EdgeCaseBits::empty();
        let _cat: ErrorCategory = ErrorCategory::Transport;
        let _sev: ErrorSeverity = ErrorSeverity::Error;
        let _circuit: CircuitState = CircuitState::Closed;
        let _delivery: DeliverySemantics = DeliverySemantics::AtLeastOnce;
        let _ack: AckState = AckState::Pending;
        let _durability: DurabilityCondition = DurabilityCondition::Quorum;
        let _node_state: NodeState = NodeState::Active;
        let _lifecycle: LifecycleState = LifecycleState::Running;
    }

    // ========== RotatingNodeIdExt tests ==========

    #[test]
    fn rotating_node_id_ext_inner() {
        let rid = RotatingNodeId([42u8; 32]);
        assert_eq!(rid.inner(), [42u8; 32]);
    }

    #[test]
    fn rotating_node_id_ext_is_zero() {
        assert!(RotatingNodeId::ZERO.is_zero());
        assert!(!RotatingNodeId([1u8; 32]).is_zero());
    }

    #[test]
    fn rotating_node_id_ext_advance() {
        let rid = RotatingNodeId([1u8; 32]);
        let advanced = rid.advance();
        assert_ne!(advanced.inner(), rid.inner());
        assert_ne!(advanced, RotatingNodeId::ZERO);
    }

    // ========== OrderingDomainExt tests ==========

    #[test]
    fn ordering_domain_ext_value() {
        let d = OrderingDomain(100);
        assert_eq!(d.value(), 100);
    }

    #[test]
    fn ordering_domain_ext_is_default() {
        assert!(OrderingDomain::ZERO.is_default());
        assert!(!OrderingDomain(1).is_default());
    }

    #[test]
    fn ordering_domain_ext_next() {
        let d = OrderingDomain(5);
        assert_eq!(d.next().value(), 6);
    }

    // ========== SnapshotIdExt tests ==========

    #[test]
    fn snapshot_id_ext_value() {
        let s = SnapshotId(42);
        assert_eq!(s.value(), 42);
    }

    #[test]
    fn snapshot_id_ext_is_initial() {
        assert!(SnapshotId::ZERO.is_initial());
        assert!(!SnapshotId(1).is_initial());
    }

    #[test]
    fn snapshot_id_ext_next() {
        let s = SnapshotId(10);
        assert_eq!(s.next().value(), 11);
    }

    // ========== CRDTStateExt tests ==========

    #[test]
    fn crdt_state_ext_is_empty() {
        assert!(CRDTState(Vec::new(), 0).is_empty());
        assert!(!CRDTState(vec![1], 0).is_empty());
    }

    #[test]
    fn crdt_state_ext_version() {
        let s = CRDTState(vec![1], 100);
        assert_eq!(s.version(), 100);
    }

    #[test]
    fn crdt_state_ext_advance_version() {
        let s = CRDTState(vec![1], 5);
        let advanced = s.advance_version();
        assert_eq!(advanced.version(), 6);
    }

    // ========== CRDTOpExt tests ==========

    #[test]
    fn crdt_op_ext_operations() {
        let op = CRDTOp(1, vec![1, 2, 3]);
        assert_eq!(op.op_type(), 1);
        assert_eq!(op.payload_len(), 3);
    }

    // ========== CRDTDeltaExt tests ==========

    #[test]
    fn crdt_delta_ext_operations() {
        let d = CRDTDelta(vec![1, 2, 3]);
        assert!(!d.is_empty());
        assert_eq!(d.size(), 3);

        let empty = CRDTDelta(Vec::new());
        assert!(empty.is_empty());
        assert_eq!(empty.size(), 0);
    }

    // ========== TierExt tests ==========

    #[test]
    fn tier_ext_name() {
        assert_eq!(Tier::Hot.name(), "hot");
        assert_eq!(Tier::Warm.name(), "warm");
        assert_eq!(Tier::Cold.name(), "cold");
        assert_eq!(Tier::Archive.name(), "archive");
    }

    #[test]
    fn tier_ext_priority() {
        assert_eq!(Tier::Hot.priority(), 4);
        assert_eq!(Tier::Warm.priority(), 3);
        assert_eq!(Tier::Cold.priority(), 2);
        assert_eq!(Tier::Archive.priority(), 1);
    }

    #[test]
    fn tier_ext_is_hot() {
        assert!(Tier::Hot.is_hot());
        assert!(!Tier::Cold.is_hot());
    }

    // ========== ComputeRequestExt tests ==========

    #[test]
    fn compute_request_ext_operations() {
        let req = ComputeRequest(vec![1, 2, 3]);
        assert_eq!(req.payload_size(), 3);
        assert!(!req.is_payload_empty());

        let empty = ComputeRequest(Vec::new());
        assert!(empty.is_payload_empty());
    }

    // ========== ComputeResultExt tests ==========

    #[test]
    fn compute_result_ext_operations() {
        let success = ComputeResult(true, vec![1, 2]);
        assert!(success.is_success());
        assert_eq!(success.payload_size(), 2);

        let failure = ComputeResult(false, Vec::new());
        assert!(!failure.is_success());
    }

    // ========== CompactResultExt tests ==========

    #[test]
    fn compact_result_ext_operations() {
        let result = CompactResult { reclaimed_bytes: 1024, compacted_objects: 5 };
        assert_eq!(result.reclaimed_bytes(), 1024);
        assert_eq!(result.compacted_objects(), 5);
        assert!(result.reclaimed_any());

        let empty = CompactResult { reclaimed_bytes: 0, compacted_objects: 0 };
        assert!(!empty.reclaimed_any());
    }

    // ========== CapacityExt tests ==========

    #[test]
    fn capacity_ext_operations() {
        let cap = Capacity(100);
        assert_eq!(cap.value(), 100);
        assert!(!cap.is_zero());

        let zero = Capacity(0);
        assert!(zero.is_zero());

        let added = cap.saturating_add(Capacity(50));
        assert_eq!(added.value(), 150);

        let sub = cap.saturating_sub(Capacity(30));
        assert_eq!(sub.value(), 70);

        let under = Capacity(10).saturating_sub(Capacity(20));
        assert_eq!(under.value(), 0);
    }

    // ========== merge_values tests ==========

    #[test]
    fn merge_values_combines_all() {
        let v1 = Value(vec![1, 2]);
        let v2 = Value(vec![3, 4]);
        let v3 = Value(vec![5]);
        let merged = merge_values(&[v1, v2, v3]);
        assert_eq!(merged.byte_len(), 5);
        assert_eq!(merged.0, vec![1, 2, 3, 4, 5]);
    }

    #[test]
    fn merge_values_empty() {
        let merged = merge_values(&[]);
        assert!(merged.is_empty());
    }

    // ========== split_value tests ==========

    #[test]
    fn split_value_into_chunks() {
        let val = Value(vec![1, 2, 3, 4, 5, 6]);
        let parts = split_value(&val, 3);
        assert_eq!(parts.len(), 3);
        assert_eq!(parts[0].byte_len(), 2);
        assert_eq!(parts[1].byte_len(), 2);
        assert_eq!(parts[2].byte_len(), 2);
    }

    #[test]
    fn split_value_zero_n() {
        let val = Value(vec![1, 2, 3]);
        let parts = split_value(&val, 0);
        assert!(parts.is_empty());
    }

    // ========== interleave_values tests ==========

    #[test]
    fn interleave_values_combines() {
        let v1 = Value(vec![1, 3, 5]);
        let v2 = Value(vec![2, 4]);
        let interleaved = interleave_values(&[v1, v2]);
        assert_eq!(interleaved.0, vec![1, 2, 3, 4, 5]);
    }

    #[test]
    fn interleave_values_empty() {
        let interleaved = interleave_values(&[]);
        assert!(interleaved.is_empty());
    }

    // ========== total_value_size tests ==========

    #[test]
    fn total_value_size_sums() {
        let vals = vec![Value(vec![1]), Value(vec![2, 3]), Value(vec![4, 5, 6])];
        assert_eq!(total_value_size(&vals), 6);
    }

    #[test]
    fn total_value_size_empty() {
        assert_eq!(total_value_size(&[]), 0);
    }

    // ========== largest_value tests ==========

    #[test]
    fn largest_value_finds_max() {
        let vals = vec![
            Value(vec![1]),
            Value(vec![2, 3, 4]),
            Value(vec![5, 6]),
        ];
        let largest = largest_value(&vals);
        assert!(largest.is_some());
        assert_eq!(largest.unwrap().byte_len(), 3);
    }

    #[test]
    fn largest_value_empty() {
        assert!(largest_value(&[]).is_none());
    }

    // ========== filter_values_by_size tests ==========

    #[test]
    fn filter_values_by_size_keeps_small() {
        let vals = vec![
            Value(vec![1]),
            Value(vec![2, 3, 4, 5]),
            Value(vec![6]),
        ];
        let filtered = filter_values_by_size(&vals, 2);
        assert_eq!(filtered.len(), 2);
    }

    // ========== build_message tests ==========

    #[test]
    fn build_message_creates_message() {
        let node = NodeId::new([1u8; 32]);
        let msg = build_message(node, Value(vec![1, 2]), TimestampMillis(100));
        assert_eq!(msg.payload_len(), 2);
        assert_eq!(msg.timestamp.0, 100);
    }

    // ========== build_change_stream tests ==========

    #[test]
    fn build_change_stream_creates_entry() {
        let cs = build_change_stream(OpRef(5), Value(vec![1]));
        assert_eq!(cs.op_ref.value(), 5);
        assert_eq!(cs.value.byte_len(), 1);
    }

    // ========== build_fragment_id tests ==========

    #[test]
    fn build_fragment_id_creates_id() {
        let ds = DatasetId::new([1u8; 16]);
        let obj = ObjectID::new([2u8; 16]);
        let fid = build_fragment_id(ds, obj);
        assert_eq!(fid.dataset_id, ds);
        assert_eq!(fid.object_id, obj);
    }

    // ========== build_roster_add/remove tests ==========

    #[test]
    fn build_roster_add_creates_add() {
        let node = NodeId::new([1u8; 32]);
        let change = build_roster_add(node);
        assert!(matches!(change, RosterChange::Add { node } if node.0[0] == 1));
    }

    #[test]
    fn build_roster_remove_creates_remove() {
        let node = NodeId::new([2u8; 32]);
        let change = build_roster_remove(node);
        assert!(matches!(change, RosterChange::Remove { node } if node.0[0] == 2));
    }

    // ========== compute_checksum/verify_checksum tests ==========

    #[test]
    fn compute_and_verify_checksum() {
        let data = b"hello world";
        let checksum = compute_checksum(data);
        assert_ne!(checksum, Checksum([0u8; 32]));
        assert!(verify_checksum(data, checksum));
        assert!(!verify_checksum(b"different", checksum));
    }

    // ========== failure_domain_hash tests ==========

    #[test]
    fn failure_domain_hash_is_deterministic() {
        let node = NodeId::new([1u8; 32]);
        let domain = FailureDomain([0u8; 32]);
        let h1 = failure_domain_hash(node, domain);
        let h2 = failure_domain_hash(node, domain);
        assert_eq!(h1, h2);
    }

    // ========== group_by_failure_domain tests ==========

    #[test]
    fn group_by_failure_domain_groups() {
        let nodes = vec![NodeId::new([1u8; 32]), NodeId::new([2u8; 32])];
        let domain = FailureDomain([0u8; 32]);
        let groups = group_by_failure_domain(&nodes, domain);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].1.len(), 2);
    }

    // ========== validation macro tests ==========

    #[test]
    fn require_nonzero_capacity_panics_on_zero() {
        let result = std::panic::catch_unwind(|| {
            require_nonzero_capacity!(Capacity(0));
        });
        assert!(result.is_err());
    }

    #[test]
    fn require_version_in_range_panics_outside() {
        let result = std::panic::catch_unwind(|| {
            require_version_in_range!(
                ProtocolVersion { major: 5, minor: 0, patch: 0 },
                ProtocolVersion { major: 1, minor: 0, patch: 0 },
                ProtocolVersion { major: 3, minor: 0, patch: 0 }
            );
        });
        assert!(result.is_err());
    }

    #[test]
    fn require_distinct_nodes_panics_on_same() {
        let node = NodeId::new([1u8; 32]);
        let result = std::panic::catch_unwind(|| {
            require_distinct_nodes!(node, node);
        });
        assert!(result.is_err());
    }

    #[test]
    fn require_carrier_panics_on_mismatch() {
        let ep = Endpoint::new_endpoint(CarrierType::Direct, vec![1]);
        let result = std::panic::catch_unwind(|| {
            require_carrier!(ep, CarrierType::Mesh);
        });
        assert!(result.is_err());
    }

    #[test]
    fn require_quorum_panics_when_not_met() {
        let result = std::panic::catch_unwind(|| {
            require_quorum!(5, 1);
        });
        assert!(result.is_err());
    }

    #[test]
    fn require_value_size_panics_on_excess() {
        let val = Value(vec![1, 2, 3, 4, 5]);
        let result = std::panic::catch_unwind(|| {
            require_value_size!(val, 3);
        });
        assert!(result.is_err());
    }

    // ========== replication_lag_tolerance tests ==========

    #[test]
    fn replication_lag_tolerance_returns_values() {
        assert_eq!(replication_lag_tolerance(DurabilityCondition::Single), 10_000);
        assert_eq!(replication_lag_tolerance(DurabilityCondition::Double), 5_000);
        assert_eq!(replication_lag_tolerance(DurabilityCondition::Triple), 2_000);
        assert_eq!(replication_lag_tolerance(DurabilityCondition::Quorum), 1_000);
        assert_eq!(replication_lag_tolerance(DurabilityCondition::All), 100);
    }

    // ========== is_replication_lag_excessive tests ==========

    #[test]
    fn is_replication_lag_excessive_checks() {
        assert!(!is_replication_lag_excessive(500, DurabilityCondition::Single));
        assert!(is_replication_lag_excessive(20_000, DurabilityCondition::Single));
    }

    // ========== sync_interval_for tests ==========

    #[test]
    fn sync_interval_for_returns_values() {
        assert_eq!(sync_interval_for(DurabilityCondition::Single), 30_000);
        assert_eq!(sync_interval_for(DurabilityCondition::All), 500);
    }

    // ========== build_replication_lag_report tests ==========

    #[test]
    fn build_replication_lag_report_creates_entry() {
        let node = NodeId::new([1u8; 32]);
        let (n, lag) = build_replication_lag_report(node, 500);
        assert_eq!(n, node);
        assert_eq!(lag, 500);
    }

    // ========== average_replication_lag tests ==========

    #[test]
    fn average_replication_lag_computes() {
        let lags = vec![
            (NodeId::new([1u8; 32]), 100),
            (NodeId::new([2u8; 32]), 300),
            (NodeId::new([3u8; 32]), 500),
        ];
        assert_eq!(average_replication_lag(&lags), 300);
    }

    #[test]
    fn average_replication_lag_empty() {
        assert_eq!(average_replication_lag(&[]), 0);
    }

    // ========== worst_replication_lag tests ==========

    #[test]
    fn worst_replication_lag_finds_max() {
        let lags = vec![
            (NodeId::new([1u8; 32]), 100),
            (NodeId::new([2u8; 32]), 500),
            (NodeId::new([3u8; 32]), 300),
        ];
        let worst = worst_replication_lag(&lags);
        assert!(worst.is_some());
        assert_eq!(worst.unwrap().1, 500);
    }

    #[test]
    fn worst_replication_lag_empty() {
        assert!(worst_replication_lag(&[]).is_none());
    }

    // ========== roster_epoch_hash tests ==========

    #[test]
    fn roster_epoch_hash_is_deterministic() {
        let roster = vec![NodeId::new([1u8; 32]), NodeId::new([2u8; 32])];
        let epoch = Epoch(5);
        let h1 = roster_epoch_hash(&roster, epoch);
        let h2 = roster_epoch_hash(&roster, epoch);
        assert_eq!(h1, h2);
    }

    // ========== is_roster_member tests ==========

    #[test]
    fn is_roster_member_checks() {
        let node = NodeId::new([1u8; 32]);
        let roster = vec![node, NodeId::new([2u8; 32])];
        assert!(is_roster_member(&roster, node));
        assert!(!is_roster_member(&roster, NodeId::new([9u8; 32])));
    }

    // ========== roster_size_for_regions tests ==========

    #[test]
    fn roster_size_for_regions_computes() {
        assert_eq!(roster_size_for_regions(0), 0);
        assert_eq!(roster_size_for_regions(1), 3);
        assert_eq!(roster_size_for_regions(5), 15);
    }

    // ========== build_roster_add_batch/remove_batch tests ==========

    #[test]
    fn build_roster_add_batch_creates_changes() {
        let nodes = vec![NodeId::new([1u8; 32]), NodeId::new([2u8; 32])];
        let changes = build_roster_add_batch(nodes.clone());
        assert_eq!(changes.len(), 2);
    }

    #[test]
    fn build_roster_remove_batch_creates_changes() {
        let nodes = vec![NodeId::new([1u8; 32])];
        let changes = build_roster_remove_batch(nodes);
        assert_eq!(changes.len(), 1);
    }

    // ========== roster_quorum_size tests ==========

    #[test]
    fn roster_quorum_size_computes() {
        assert_eq!(roster_quorum_size(0), 0);
        assert_eq!(roster_quorum_size(5), 3);
        assert_eq!(roster_quorum_size(4), 3);
    }

    // ========== is_lifecycle_terminal tests ==========

    #[test]
    fn is_lifecycle_terminal_checks() {
        assert!(is_lifecycle_terminal(LifecycleState::Completed));
        assert!(is_lifecycle_terminal(LifecycleState::Failed));
        assert!(is_lifecycle_terminal(LifecycleState::Stopped));
        assert!(!is_lifecycle_terminal(LifecycleState::Running));
    }

    // ========== is_lifecycle_active tests ==========

    #[test]
    fn is_lifecycle_active_checks() {
        assert!(is_lifecycle_active(LifecycleState::Running));
        assert!(is_lifecycle_active(LifecycleState::Starting));
        assert!(!is_lifecycle_active(LifecycleState::Stopped));
    }

    // ========== next_lifecycle_state tests ==========

    #[test]
    fn next_lifecycle_state_transitions() {
        assert_eq!(
            next_lifecycle_state(LifecycleState::Stopped, LifecycleEvent::Start),
            LifecycleState::Starting
        );
        assert_eq!(
            next_lifecycle_state(LifecycleState::Starting, LifecycleEvent::StartComplete),
            LifecycleState::Running
        );
        assert_eq!(
            next_lifecycle_state(LifecycleState::Running, LifecycleEvent::Stop),
            LifecycleState::Stopping
        );
    }

    // ========== build_lifecycle_transition tests ==========

    #[test]
    fn build_lifecycle_transition_creates_entry() {
        let node = NodeId::new([1u8; 32]);
        let entry = build_lifecycle_transition(
            node,
            LifecycleState::Running,
            LifecycleState::Completed,
            TimestampMillis(100),
        );
        assert_eq!(entry.0, node);
        assert_eq!(entry.3.0, 100);
    }

    // ========== anomaly_score tests ==========

    #[test]
    fn anomaly_score_computes() {
        assert_eq!(anomaly_score(0, 100, 1000), 0);
        assert_eq!(anomaly_score(50, 100, 0), 500);
    }

    #[test]
    fn anomaly_score_zero_total() {
        assert_eq!(anomaly_score(5, 0, 1000), 0);
    }

    // ========== classify_anomaly tests ==========

    #[test]
    fn classify_anomaly_classifies() {
        assert_eq!(classify_anomaly(5), DegradationLevel::None);
        assert_eq!(classify_anomaly(50), DegradationLevel::Minor);
        assert_eq!(classify_anomaly(100), DegradationLevel::Moderate);
        assert_eq!(classify_anomaly(300), DegradationLevel::Severe);
    }

    // ========== build_anomaly tests ==========

    #[test]
    fn build_anomaly_creates_record() {
        let node = NodeId::new([1u8; 32]);
        let anomaly = build_anomaly(node, 100, "test".into());
        assert_eq!(anomaly.node, node);
        assert_eq!(anomaly.score, 100);
        assert_eq!(anomaly.description, "test");
    }

    // ========== anomaly_requires_intervention tests ==========

    #[test]
    fn anomaly_requires_intervention_checks() {
        assert!(!anomaly_requires_intervention(100));
        assert!(anomaly_requires_intervention(200));
    }

    // ========== checksum_boundary/verify_checksum_boundary tests ==========

    #[test]
    fn checksum_boundary_round_trips() {
        let data = b"test data";
        let cs = checksum_boundary(0, 10, data);
        assert!(verify_checksum_boundary(0, 10, data, cs));
        assert!(!verify_checksum_boundary(0, 10, b"other", cs));
    }

    // ========== rolling_checksum tests ==========

    #[test]
    fn rolling_checksum_computes() {
        let chunks = vec![b"abc", b"def"];
        let checksums = rolling_checksum(&chunks);
        assert_eq!(checksums.len(), 2);
        assert_ne!(checksums[0], checksums[1]);
    }

    // ========== merge_checksums tests ==========

    #[test]
    fn merge_checksums_combines() {
        let c1 = Checksum([1u8; 32]);
        let c2 = Checksum([2u8; 32]);
        let merged = merge_checksums(&[c1, c2]);
        assert_ne!(merged, Checksum([0u8; 32]));

        let empty = merge_checksums(&[]);
        assert_eq!(empty, Checksum([0u8; 32]));
    }

    // ========== delivery_timeout_for tests ==========

    #[test]
    fn delivery_timeout_for_returns_values() {
        assert_eq!(delivery_timeout_for(DeliverySemantics::AtMostOnce), 5_000);
        assert_eq!(delivery_timeout_for(DeliverySemantics::AtLeastOnce), 10_000);
        assert_eq!(delivery_timeout_for(DeliverySemantics::ExactlyOnce), 30_000);
    }

    // ========== is_ack_state_final tests ==========

    #[test]
    fn is_ack_state_final_checks() {
        assert!(is_ack_state_final(AckState::Delivered));
        assert!(is_ack_state_final(AckState::Failed));
        assert!(is_ack_state_final(AckState::Expired));
        assert!(!is_ack_state_final(AckState::Pending));
    }

    // ========== retry_count_for_ack tests ==========

    #[test]
    fn retry_count_for_ack_returns_values() {
        assert_eq!(retry_count_for_ack(AckState::Pending), 0);
        assert_eq!(retry_count_for_ack(AckState::Retrying(3)), 3);
        assert_eq!(retry_count_for_ack(AckState::Delivered), 0);
    }

    // ========== build_ack_transition tests ==========

    #[test]
    fn build_ack_transition_creates_pair() {
        let (from, to) = build_ack_transition(AckState::Pending, AckState::Delivered);
        assert_eq!(from, AckState::Pending);
        assert_eq!(to, AckState::Delivered);
    }

    // ========== circuit_breaker_reset_timeout tests ==========

    #[test]
    fn circuit_breaker_reset_timeout_computes() {
        assert_eq!(circuit_breaker_reset_timeout(0), 0);
        assert_eq!(circuit_breaker_reset_timeout(5), 5_000);
        assert_eq!(circuit_breaker_reset_timeout(100), 60_000);
    }

    // ========== should_trip_circuit_breaker tests ==========

    #[test]
    fn should_trip_circuit_breaker_checks() {
        assert!(!should_trip_circuit_breaker(3, 5));
        assert!(should_trip_circuit_breaker(5, 5));
    }

    // ========== half_open_probe_count tests ==========

    #[test]
    fn half_open_probe_count_computes() {
        assert_eq!(half_open_probe_count(0), 1);
        assert_eq!(half_open_probe_count(10), 3);
        assert_eq!(half_open_probe_count(100), 10);
    }

    // ========== total_storage_overhead tests ==========

    #[test]
    fn total_storage_overhead_computes() {
        let overhead = total_storage_overhead(3, 10, 2);
        assert!((overhead - 3.6).abs() < 0.001);
    }

    // ========== read_amplification tests ==========

    #[test]
    fn read_amplification_returns_values() {
        assert_eq!(read_amplification(ConsistencyProfile::Eventual), 1);
        assert_eq!(read_amplification(ConsistencyProfile::Strong), 5);
    }

    // ========== write_amplification tests ==========

    #[test]
    fn write_amplification_returns_values() {
        assert_eq!(write_amplification(RedundancyMode::Single), 1);
        assert_eq!(write_amplification(RedundancyMode::All), 7);
    }

    // ========== network_hops_for_topology tests ==========

    #[test]
    fn network_hops_for_topology_computes() {
        assert_eq!(network_hops_for_topology(0), 0);
        assert_eq!(network_hops_for_topology(1), 0);
        assert_eq!(network_hops_for_topology(2), 1);
        assert_eq!(network_hops_for_topology(8), 3);
    }

    // ========== supports_redundancy tests ==========

    #[test]
    fn supports_redundancy_checks() {
        assert!(supports_redundancy(5, RedundancyMode::Triple));
        assert!(!supports_redundancy(2, RedundancyMode::Triple));
        assert!(supports_redundancy(7, RedundancyMode::All));
    }

    // ========== failure_tolerance_for_redundancy tests ==========

    #[test]
    fn failure_tolerance_for_redundancy_returns_values() {
        assert_eq!(failure_tolerance_for_redundancy(RedundancyMode::Single), 0);
        assert_eq!(failure_tolerance_for_redundancy(RedundancyMode::All), 3);
    }

    // ========== build_placement_decision tests ==========

    #[test]
    fn build_placement_decision_creates() {
        let node = NodeId::new([1u8; 32]);
        let decision = build_placement_decision(node, 50);
        assert_eq!(decision.score, 50);
        assert_eq!(decision.replicas.len(), 1);
    }

    // ========== total_placement_score tests ==========

    #[test]
    fn total_placement_score_sums() {
        let decisions = vec![
            build_placement_decision(NodeId::new([1u8; 32]), 10),
            build_placement_decision(NodeId::new([2u8; 32]), 20),
        ];
        assert_eq!(total_placement_score(&decisions), 30);
    }

    // ========== best_placement_decision tests ==========

    #[test]
    fn best_placement_decision_finds_min() {
        let decisions = vec![
            build_placement_decision(NodeId::new([1u8; 32]), 50),
            build_placement_decision(NodeId::new([2u8; 32]), 10),
        ];
        let best = best_placement_decision(&decisions);
        assert!(best.is_some());
        assert_eq!(best.unwrap().score, 10);
    }

    #[test]
    fn best_placement_decision_empty() {
        assert!(best_placement_decision(&[]).is_none());
    }

    // ========== is_placement_acceptable tests ==========

    #[test]
    fn is_placement_acceptable_checks() {
        let good = build_placement_decision(NodeId::new([1u8; 32]), 10);
        let bad = build_placement_decision(NodeId::new([2u8; 32]), 100);
        assert!(is_placement_acceptable(&good, 20));
        assert!(!is_placement_acceptable(&bad, 20));
    }

    // ========== change_stream_length tests ==========

    #[test]
    fn change_stream_length_computes() {
        assert_eq!(change_stream_length(OpRef(10), OpRef(20)), 11);
        assert_eq!(change_stream_length(OpRef(20), OpRef(10)), 0);
    }

    // ========== is_op_in_stream tests ==========

    #[test]
    fn is_op_in_stream_checks() {
        assert!(is_op_in_stream(OpRef(15), OpRef(10), OpRef(20)));
        assert!(!is_op_in_stream(OpRef(5), OpRef(10), OpRef(20)));
    }

    // ========== build_versioned_fragment_id tests ==========

    #[test]
    fn build_versioned_fragment_id_creates() {
        let ds = DatasetId::new([1u8; 16]);
        let obj = ObjectID::new([2u8; 16]);
        let (fid, ver) = build_versioned_fragment_id(ds, obj, Version(5));
        assert_eq!(fid.dataset_id, ds);
        assert_eq!(ver, Version(5));
    }

    // ========== fragment_id_hash tests ==========

    #[test]
    fn fragment_id_hash_is_deterministic() {
        let fid = FragmentId {
            dataset_id: DatasetId::new([1u8; 16]),
            object_id: ObjectID::new([2u8; 16]),
        };
        let h1 = fragment_id_hash(&fid);
        let h2 = fragment_id_hash(&fid);
        assert_eq!(h1, h2);
    }

    // ========== fragments_same_dataset tests ==========

    #[test]
    fn fragments_same_dataset_checks() {
        let ds = DatasetId::new([1u8; 16]);
        let f1 = FragmentId { dataset_id: ds, object_id: ObjectID::new([1u8; 16]) };
        let f2 = FragmentId { dataset_id: ds, object_id: ObjectID::new([2u8; 16]) };
        let f3 = FragmentId { dataset_id: DatasetId::new([9u8; 16]), object_id: ObjectID::new([1u8; 16]) };
        assert!(fragments_same_dataset(&f1, &f2));
        assert!(!fragments_same_dataset(&f1, &f3));
    }

    // ========== total_replication_overhead tests ==========

    #[test]
    fn total_replication_overhead_computes() {
        assert_eq!(total_replication_overhead(3, 10), 0.3);
        assert_eq!(total_replication_overhead(5, 0), 0.0);
    }

    // ========== consistency_overhead tests ==========

    #[test]
    fn consistency_overhead_returns_values() {
        assert_eq!(consistency_overhead(ConsistencyProfile::Eventual), 1);
        assert_eq!(consistency_overhead(ConsistencyProfile::Strong), 5);
    }

    // ========== is_primary_for_key tests ==========

    #[test]
    fn is_primary_for_key_checks() {
        let replicas = vec![NodeId::new([1u8; 32]), NodeId::new([2u8; 32])];
        assert!(is_primary_for_key(replicas[0], b"key1", &replicas)
            || is_primary_for_key(replicas[1], b"key1", &replicas));
    }

    // ========== primary_node_for_key tests ==========

    #[test]
    fn primary_node_for_key_returns_some() {
        let replicas = vec![NodeId::new([1u8; 32]), NodeId::new([2u8; 32])];
        let primary = primary_node_for_key(b"key", &replicas);
        assert!(primary.is_some());
        assert!(replicas.contains(&primary.unwrap()));
    }

    #[test]
    fn primary_node_for_key_empty() {
        assert!(primary_node_for_key(b"key", &[]).is_none());
    }

    // ========== replica_placement_for_key tests ==========

    #[test]
    fn replica_placement_for_key_orders() {
        let replicas = vec![
            NodeId::new([1u8; 32]),
            NodeId::new([2u8; 32]),
            NodeId::new([3u8; 32]),
        ];
        let placement = replica_placement_for_key(b"key", &replicas);
        assert_eq!(placement.len(), 3);
    }

    #[test]
    fn replica_placement_for_key_empty() {
        assert!(replica_placement_for_key(b"key", &[]).is_empty());
    }

    #[test]
    fn replica_placement_for_key_deterministic() {
        let replicas = vec![NodeId::new([1u8; 32]), NodeId::new([2u8; 32])];
        let p1 = replica_placement_for_key(b"key", &replicas);
        let p2 = replica_placement_for_key(b"key", &replicas);
        assert_eq!(p1, p2);
    }

    // ========== recovery_scope_from_fault_tolerance tests ==========

    #[test]
    fn recovery_scope_from_fault_tolerance_maps() {
        assert_eq!(
            recovery_scope_from_fault_tolerance(DegradationLevel::None),
            RecoveryScopeLevel::Local
        );
        assert_eq!(
            recovery_scope_from_fault_tolerance(DegradationLevel::Severe),
            RecoveryScopeLevel::Global
        );
    }

    // ========== retry_deadline tests ==========

    #[test]
    fn retry_deadline_computes() {
        let start = TimestampMillis(1000);
        let deadline = retry_deadline(start, 5000);
        assert_eq!(deadline.0, 6000);
    }

    // ========== is_retry_deadline_passed tests ==========

    #[test]
    fn is_retry_deadline_passed_checks() {
        let deadline = TimestampMillis(1000);
        assert!(!is_retry_deadline_passed(deadline, TimestampMillis(500)));
        assert!(is_retry_deadline_passed(deadline, TimestampMillis(1000)));
        assert!(is_retry_deadline_passed(deadline, TimestampMillis(2000)));
    }

    // ========== broadcast_fanout tests ==========

    #[test]
    fn broadcast_fanout_computes() {
        assert_eq!(broadcast_fanout(2), 2);
        assert_eq!(broadcast_fanout(5), 3);
        assert_eq!(broadcast_fanout(100), 11);
    }

    // ========== expected_acks_for_broadcast tests ==========

    #[test]
    fn expected_acks_for_broadcast_computes() {
        assert_eq!(expected_acks_for_broadcast(0), 0);
        assert_eq!(expected_acks_for_broadcast(5), 3);
        assert_eq!(expected_acks_for_broadcast(10), 6);
    }

    // ========== broadcast_quorum_reached tests ==========

    #[test]
    fn broadcast_quorum_reached_checks() {
        assert!(broadcast_quorum_reached(5, 3));
        assert!(!broadcast_quorum_reached(5, 2));
    }

    // ========== build_saga_id tests ==========

    #[test]
    fn build_saga_id_creates() {
        let node = NodeId::new([1u8; 32]);
        let saga = build_saga_id(node, 5);
        assert_ne!(saga.0, 0);
    }

    // ========== build_saga_step_id tests ==========

    #[test]
    fn build_saga_step_id_creates() {
        let saga = SagaId(42);
        let step = build_saga_step_id(saga, 3);
        assert_eq!(step.step, 3);
        assert_eq!(step.txn_id.0, 42);
    }

    // ========== is_saga_step_complete tests ==========

    #[test]
    fn is_saga_step_complete_checks() {
        assert!(is_saga_step_complete(LifecycleState::Completed));
        assert!(!is_saga_step_complete(LifecycleState::Running));
    }

    // ========== saga_timeout_for_steps tests ==========

    #[test]
    fn saga_timeout_for_steps_computes() {
        assert_eq!(saga_timeout_for_steps(0), 0);
        assert_eq!(saga_timeout_for_steps(3), 90_000);
    }

    // ========== build_lease_id tests ==========

    #[test]
    fn build_lease_id_creates() {
        let node = NodeId::new([1u8; 32]);
        let lease = build_lease_id(node, TimestampMillis(100));
        assert_ne!(lease.0, 0);
    }

    // ========== build_fencing_token tests ==========

    #[test]
    fn build_fencing_token_creates() {
        let lease = LeaseId(10);
        let token = build_fencing_token(lease, 5);
        assert_eq!(token.0, 15);
    }

    // ========== is_newer_fencing_token tests ==========

    #[test]
    fn is_newer_fencing_token_checks() {
        assert!(is_newer_fencing_token(FencingToken(10), FencingToken(5)));
        assert!(!is_newer_fencing_token(FencingToken(5), FencingToken(10)));
    }

    // ========== lease_expiry_from_ttl tests ==========

    #[test]
    fn lease_expiry_from_ttl_computes() {
        let start = TimestampMillis(1000);
        let expiry = lease_expiry_from_ttl(start, 5000);
        assert_eq!(expiry.0, 6000);
    }

    // ========== max_byzantine_nodes tests ==========

    #[test]
    fn max_byzantine_nodes_computes() {
        assert_eq!(max_byzantine_nodes(0), 0);
        assert_eq!(max_byzantine_nodes(10), 3);
        assert_eq!(max_byzantine_nodes(4), 1);
    }

    // ========== min_honest_nodes tests ==========

    #[test]
    fn min_honest_nodes_computes() {
        assert_eq!(min_honest_nodes(10), 7);
        assert_eq!(min_honest_nodes(4), 3);
    }

    // ========== byzantine_quorum_met tests ==========

    #[test]
    fn byzantine_quorum_met_checks() {
        assert!(byzantine_quorum_met(3, 2));
        assert!(!byzantine_quorum_met(3, 1));
    }

    // ========== fault_tolerance_level tests ==========

    #[test]
    fn fault_tolerance_level_classifies() {
        assert_eq!(fault_tolerance_level(1), DegradationLevel::None);
        assert_eq!(fault_tolerance_level(10), DegradationLevel::Minor);
    }

    // ========== erasure_overhead tests ==========

    #[test]
    fn erasure_overhead_computes() {
        assert!((erasure_overhead(10, 2) - 0.2).abs() < 0.001);
        assert_eq!(erasure_overhead(0, 5), 0.0);
    }

    // ========== total_erasure_shards tests ==========

    #[test]
    fn total_erasure_shards_computes() {
        assert_eq!(total_erasure_shards(10, 2), 12);
    }

    // ========== can_reconstruct tests ==========

    #[test]
    fn can_reconstruct_checks() {
        assert!(can_reconstruct(10, 2, 10));
        assert!(!can_reconstruct(10, 2, 9));
    }

    // ========== durability_for_replication tests ==========

    #[test]
    fn durability_for_replication_maps() {
        assert_eq!(durability_for_replication(1), DurabilityCondition::Single);
        assert_eq!(durability_for_replication(3), DurabilityCondition::Triple);
        assert_eq!(durability_for_replication(5), DurabilityCondition::Quorum);
        assert_eq!(durability_for_replication(10), DurabilityCondition::All);
    }

    // ========== merge_crdt_deltas tests ==========

    #[test]
    fn merge_crdt_deltas_combines() {
        let d1 = CRDTDelta(vec![1, 2]);
        let d2 = CRDTDelta(vec![3, 4]);
        let merged = merge_crdt_deltas(d1, d2);
        assert_eq!(merged.size(), 4);
    }

    // ========== merged_crdt_version tests ==========

    #[test]
    fn merged_crdt_version_computes() {
        assert_eq!(merged_crdt_version(5, 3), 6);
        assert_eq!(merged_crdt_version(3, 5), 6);
    }

    // ========== are_consistency_profiles_compatible tests ==========

    #[test]
    fn are_consistency_profiles_compatible_checks() {
        assert!(are_consistency_profiles_compatible(
            ConsistencyProfile::Strong,
            ConsistencyProfile::Strong
        ));
        assert!(are_consistency_profiles_compatible(
            ConsistencyProfile::Eventual,
            ConsistencyProfile::Strong
        ));
        assert!(!are_consistency_profiles_compatible(
            ConsistencyProfile::Strong,
            ConsistencyProfile::Causal
        ));
    }

    // ========== consistency_for_quorum tests ==========

    #[test]
    fn consistency_for_quorum_maps() {
        assert_eq!(consistency_for_quorum(0), ConsistencyProfile::Eventual);
        assert_eq!(consistency_for_quorum(1), ConsistencyProfile::ReadYourWrites);
        assert_eq!(consistency_for_quorum(2), ConsistencyProfile::Causal);
        assert_eq!(consistency_for_quorum(3), ConsistencyProfile::Sequential);
        assert_eq!(consistency_for_quorum(10), ConsistencyProfile::Strong);
    }

    // ========== carrier_routing_cost tests ==========

    #[test]
    fn carrier_routing_cost_computes() {
        assert_eq!(carrier_routing_cost(CarrierType::Direct, 10), 11);
        assert_eq!(carrier_routing_cost(CarrierType::Mesh, 10), 20);
    }

    // ========== carriers_can_communicate tests ==========

    #[test]
    fn carriers_can_communicate_checks() {
        assert!(carriers_can_communicate(CarrierType::Direct, CarrierType::Direct));
        assert!(carriers_can_communicate(CarrierType::Direct, CarrierType::Mesh));
        assert!(!carriers_can_communicate(CarrierType::Mesh, CarrierType::Optical));
    }

    // ========== endpoint_fingerprint_distance tests ==========

    #[test]
    fn endpoint_fingerprint_distance_computes() {
        let e1 = Endpoint::new_endpoint(CarrierType::Direct, vec![1]);
        let e2 = Endpoint::new_endpoint(CarrierType::Direct, vec![2]);
        let dist = endpoint_fingerprint_distance(&e1, &e2);
        assert_ne!(dist, 0);
    }

    // ========== build_routing_entry/sort_routing_entries tests ==========

    #[test]
    fn build_and_sort_routing_entries() {
        let mut entries = vec![
            (Endpoint::new_endpoint(CarrierType::Direct, vec![1]), 10),
            (Endpoint::new_endpoint(CarrierType::Mesh, vec![2]), 5),
        ];
        sort_routing_entries(&mut entries);
        assert_eq!(entries[0].1, 5);
        assert_eq!(entries[1].1, 10);
    }

    // ========== message_integrity_hash tests ==========

    #[test]
    fn message_integrity_hash_round_trips() {
        let msg = Message {
            id: MessageId(1),
            payload: Value(vec![1, 2, 3]),
            timestamp: TimestampMillis(100),
        };
        let hash = message_integrity_hash(&msg);
        assert_ne!(hash, [0u8; 32]);
        assert!(verify_message_integrity(&msg, hash));
    }

    // ========== credential_rotation_hash tests ==========

    #[test]
    fn credential_rotation_hash_is_deterministic() {
        let cred = Credential(vec![1, 2, 3]);
        let epoch = Epoch(5);
        let h1 = credential_rotation_hash(&cred, epoch);
        let h2 = credential_rotation_hash(&cred, epoch);
        assert_eq!(h1, h2);
    }

    // ========== needs_credential_rotation tests ==========

    #[test]
    fn needs_credential_rotation_checks() {
        assert!(!needs_credential_rotation(Epoch(5), Epoch(6), 10));
        assert!(needs_credential_rotation(Epoch(1), Epoch(20), 10));
    }

    // ========== aggregate_health_scores tests ==========

    #[test]
    fn aggregate_health_scores_computes() {
        let scores = vec![HealthScore(80), HealthScore(90), HealthScore(70)];
        assert_eq!(aggregate_health_scores(&scores).0, 80);
    }

    #[test]
    fn aggregate_health_scores_empty() {
        assert_eq!(aggregate_health_scores(&[]).0, 0);
    }

    // ========== system_health tests ==========

    #[test]
    fn system_health_computes() {
        let components = vec![
            ComponentHealth { healthy: true },
            ComponentHealth { healthy: true },
            ComponentHealth { healthy: false },
        ];
        assert_eq!(system_health(&components).0, 66);
    }

    #[test]
    fn system_health_empty() {
        assert_eq!(system_health(&[]).0, 0);
    }

    // ========== build_error_report tests ==========

    #[test]
    fn build_error_report_creates() {
        let errors = vec![
            Error::CarrierUnavailable(CarrierType::Direct, "down".into()),
        ];
        let report = build_error_report(&errors);
        assert_eq!(report.total_errors(), 1);
    }

    // ========== total_errors_across_reports tests ==========

    #[test]
    fn total_errors_across_reports_sums() {
        let mut r1 = AggregatedErrorReport::new();
        r1.record(&Error::Internal("a".into()), &ErrorClassifier::new());
        let r2 = AggregatedErrorReport::new();
        assert_eq!(total_errors_across_reports(&[r1, r2]), 1);
    }

    // ========== default_error_router tests ==========

    #[test]
    fn default_error_router_has_routes() {
        let router = default_error_router();
        assert!(router.has_route(ErrorCategory::Transport, ErrorSeverity::Critical));
    }

    // ========== default_backoff_scheduler tests ==========

    #[test]
    fn default_backoff_scheduler_has_values() {
        let scheduler = default_backoff_scheduler();
        assert_eq!(scheduler.base_ms, 100);
        assert_eq!(scheduler.max_retries, 10);
    }

    // ========== default_error_rate_limiter tests ==========

    #[test]
    fn default_error_rate_limiter_has_values() {
        let limiter = default_error_rate_limiter();
        assert_eq!(limiter.max_per_window, 1000);
        assert_eq!(limiter.window_ms, 60_000);
    }

    // ========== total_storage_overhead edge cases ==========

    #[test]
    fn total_storage_overhead_edge_cases() {
        assert_eq!(total_storage_overhead(1, 10, 0), 1.0);
        assert_eq!(total_storage_overhead(0, 10, 5), 0.0);
    }

    // ========== is_primary_for_key edge cases ==========

    #[test]
    fn is_primary_for_key_empty_replicas() {
        assert!(!is_primary_for_key(NodeId::ZERO, b"key", &[]));
    }

    // ========== replica_placement_for_key deterministic ==========

    #[test]
    fn replica_placement_for_key_deterministic() {
        let replicas = vec![NodeId::new([1u8; 32]), NodeId::new([2u8; 32])];
        let p1 = replica_placement_for_key(b"key", &replicas);
        let p2 = replica_placement_for_key(b"key", &replicas);
        assert_eq!(p1, p2);
    }
}

///////////////////////////////////////////////////////////////////////////////
// END OF FILE
///////////////////////////////////////////////////////////////////////////////