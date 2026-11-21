//! # ids-000-aa Canonical Types
//!
//! Every canonical type in the IDS workspace has exactly one definition here.
//! All downstream `ids-*` crates import these as `ids_000_aa::NodeId`, etc.
//! No type is duplicated anywhere else in the workspace.
//!
//! Carrier-agnostic: `Endpoint` uses `CarrierType + Vec<u8>` only — no IP, no
//! port, no `SocketAddr`, no `std::net`, no `tokio::net`.

use crate::errors::Error;
use serde::{Deserialize, Serialize};
use std::fmt;

///////////////////////////////////////////////////////////////////////////////
// CORE IDENTIFIERS
///////////////////////////////////////////////////////////////////////////////

/// Stable internal node identity — BLAKE3(ed25519 pubkey).
/// Never changes for the lifetime of a node. (IDS-B §3.1)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NodeId(pub [u8; 32]);

impl NodeId {
    pub const ZERO: NodeId = NodeId([0u8; 32]);

    pub fn new(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    pub fn is_zero(&self) -> bool {
        self.0 == [0u8; 32]
    }

    pub fn to_hex(&self) -> String {
        self.0.iter().map(|b| format!("{:02x}", b)).collect()
    }

    pub fn from_hex(hex: &str) -> Result<Self, Error> {
        let bytes: Vec<u8> = (0..hex.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&hex[i..i + 2], 16))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| Error::InvalidHex {
                value: hex.to_string(),
            })?;
        if bytes.len() != 32 {
            return Err(Error::InvalidNodeIdLength {
                len: bytes.len(),
            });
        }
        let mut arr = [0u8; 32];
        arr.copy_from_slice(&bytes);
        Ok(NodeId(arr))
    }
}

impl fmt::Display for NodeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "NodeId({})", self.to_hex())
    }
}

/// Region identifier — BLAKE3 hash of region genesis seed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RegionId(pub [u8; 32]);

impl RegionId {
    pub const ZERO: RegionId = RegionId([0u8; 32]);

    pub fn new(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    pub fn is_zero(&self) -> bool {
        self.0 == [0u8; 32]
    }

    pub fn to_hex(&self) -> String {
        self.0.iter().map(|b| format!("{:02x}", b)).collect()
    }

    pub fn from_hex(hex: &str) -> Result<Self, Error> {
        let bytes: Vec<u8> = (0..hex.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&hex[i..i + 2], 16))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| Error::InvalidHex {
                value: hex.to_string(),
            })?;
        if bytes.len() != 32 {
            return Err(Error::InvalidRegionIdLength {
                len: bytes.len(),
            });
        }
        let mut arr = [0u8; 32];
        arr.copy_from_slice(&bytes);
        Ok(RegionId(arr))
    }
}

impl fmt::Display for RegionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "RegionId({})", self.to_hex())
    }
}

/// H-unit identifier — a hardware unit grouping within a region.
/// (IDS-A §35, CAPABILITIES §3)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct HUnitId(pub [u8; 32]);

impl HUnitId {
    pub const ZERO: HUnitId = HUnitId([0u8; 32]);

    pub fn new(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    pub fn is_zero(&self) -> bool {
        self.0 == [0u8; 32]
    }

    pub fn to_hex(&self) -> String {
        self.0.iter().map(|b| format!("{:02x}", b)).collect()
    }

    pub fn from_hex(hex: &str) -> Result<Self, Error> {
        let bytes: Vec<u8> = (0..hex.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&hex[i..i + 2], 16))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| Error::InvalidHex {
                value: hex.to_string(),
            })?;
        if bytes.len() != 32 {
            return Err(Error::InvalidHUnitIdLength {
                len: bytes.len(),
            });
        }
        let mut arr = [0u8; 32];
        arr.copy_from_slice(&bytes);
        Ok(HUnitId(arr))
    }
}

impl fmt::Display for HUnitId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "HUnitId({})", self.to_hex())
    }
}

/// Dataset identifier — BLAKE3 hash of dataset creation transaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DatasetId(pub [u8; 32]);

impl DatasetId {
    pub const ZERO: DatasetId = DatasetId([0u8; 32]);

    pub fn new(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    pub fn is_zero(&self) -> bool {
        self.0 == [0u8; 32]
    }

    pub fn to_hex(&self) -> String {
        self.0.iter().map(|b| format!("{:02x}", b)).collect()
    }

    pub fn from_hex(hex: &str) -> Result<Self, Error> {
        let bytes: Vec<u8> = (0..hex.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&hex[i..i + 2], 16))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| Error::InvalidHex {
                value: hex.to_string(),
            })?;
        if bytes.len() != 32 {
            return Err(Error::InvalidDatasetIdLength {
                len: bytes.len(),
            });
        }
        let mut arr = [0u8; 32];
        arr.copy_from_slice(&bytes);
        Ok(DatasetId(arr))
    }
}

impl fmt::Display for DatasetId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "DatasetId({})", self.to_hex())
    }
}

/// Partition identifier within a dataset.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PartitionId(pub [u8; 32]);

impl PartitionId {
    pub const ZERO: PartitionId = PartitionId([0u8; 32]);

    pub fn new(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    pub fn is_zero(&self) -> bool {
        self.0 == [0u8; 32]
    }

    pub fn to_hex(&self) -> String {
        self.0.iter().map(|b| format!("{:02x}", b)).collect()
    }

    pub fn from_hex(hex: &str) -> Result<Self, Error> {
        let bytes: Vec<u8> = (0..hex.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&hex[i..i + 2], 16))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| Error::InvalidHex {
                value: hex.to_string(),
            })?;
        if bytes.len() != 32 {
            return Err(Error::InvalidPartitionIdLength {
                len: bytes.len(),
            });
        }
        let mut arr = [0u8; 32];
        arr.copy_from_slice(&bytes);
        Ok(PartitionId(arr))
    }
}

impl fmt::Display for PartitionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "PartitionId({})", self.to_hex())
    }
}

/// Record identifier within a partition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RecordId(pub [u8; 32]);

impl RecordId {
    pub const ZERO: RecordId = RecordId([0u8; 32]);

    pub fn new(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    pub fn is_zero(&self) -> bool {
        self.0 == [0u8; 32]
    }

    pub fn to_hex(&self) -> String {
        self.0.iter().map(|b| format!("{:02x}", b)).collect()
    }

    pub fn from_hex(hex: &str) -> Result<Self, Error> {
        let bytes: Vec<u8> = (0..hex.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&hex[i..i + 2], 16))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| Error::InvalidHex {
                value: hex.to_string(),
            })?;
        if bytes.len() != 32 {
            return Err(Error::InvalidRecordIdLength {
                len: bytes.len(),
            });
        }
        let mut arr = [0u8; 32];
        arr.copy_from_slice(&bytes);
        Ok(RecordId(arr))
    }
}

impl fmt::Display for RecordId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "RecordId({})", self.to_hex())
    }
}

/// Nine-character alphanumeric operation reference (36^9 namespace).
/// (IDS-A §38, 47)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct OpRef(pub [u8; 9]);

impl OpRef {
    pub const ZERO: OpRef = OpRef([0u8; 9]);

    pub fn new(bytes: [u8; 9]) -> Self {
        Self(bytes)
    }

    pub fn as_bytes(&self) -> &[u8; 9] {
        &self.0
    }

    pub fn is_zero(&self) -> bool {
        self.0 == [0u8; 9]
    }

    pub fn to_string_lossy(&self) -> String {
        String::from_utf8_lossy(&self.0).to_string()
    }

    pub fn from_str(s: &str) -> Result<Self, Error> {
        if s.len() != 9 {
            return Err(Error::InvalidOpRefLength {
                len: s.len(),
            });
        }
        let bytes = s.as_bytes().try_into().map_err(|_| Error::InvalidOpRefLength {
            len: s.len(),
        })?;
        Ok(OpRef(bytes))
    }
}

impl fmt::Display for OpRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "OpRef({})", self.to_string_lossy())
    }
}

/// Ordering position — monotonic per dataset domain, separate from OpRef.
/// (IDS-A §38.3)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct OrderingPos(pub u64);

impl OrderingPos {
    pub const ZERO: OrderingPos = OrderingPos(0);
    pub const MAX: OrderingPos = OrderingPos(u64::MAX);

    pub fn new(pos: u64) -> Self {
        Self(pos)
    }

    pub fn value(&self) -> u64 {
        self.0
    }

    pub fn next(&self) -> Self {
        Self(self.0.saturating_add(1))
    }

    pub fn prev(&self) -> Self {
        Self(self.0.saturating_sub(1))
    }

    pub fn is_zero(&self) -> bool {
        self.0 == 0
    }
}

impl fmt::Display for OrderingPos {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "OrderingPos({})", self.0)
    }
}

///////////////////////////////////////////////////////////////////////////////
// CARRIER-AGNOSTIC ADDRESS
///////////////////////////////////////////////////////////////////////////////

/// Carrier type for opaque endpoint addressing.
/// No IP, no port, no existing networking stack references.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CarrierType {
    Loopback,
    Serial,
    Optical,
    Radio,
    Custom(u16),
}

impl CarrierType {
    pub fn as_u16(&self) -> u16 {
        match self {
            CarrierType::Loopback => 0,
            CarrierType::Serial => 1,
            CarrierType::Optical => 2,
            CarrierType::Radio => 3,
            CarrierType::Custom(n) => *n,
        }
    }

    pub fn from_u16(n: u16) -> Self {
        match n {
            0 => CarrierType::Loopback,
            1 => CarrierType::Serial,
            2 => CarrierType::Optical,
            3 => CarrierType::Radio,
            _ => CarrierType::Custom(n),
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            CarrierType::Loopback => "loopback",
            CarrierType::Serial => "serial",
            CarrierType::Optical => "optical",
            CarrierType::Radio => "radio",
            CarrierType::Custom(_) => "custom",
        }
    }

    pub fn is_native(&self) -> bool {
        matches!(self, CarrierType::Loopback | CarrierType::Serial | CarrierType::Optical | CarrierType::Radio)
    }
}

impl fmt::Display for CarrierType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "CarrierType({})", self.name())
    }
}

/// Opaque carrier-agnostic endpoint address.
/// `CarrierType` + opaque `Vec<u8>` — no IP, no port, no SocketAddr.
/// This is the ONLY address type in the entire IDS workspace.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Endpoint {
    pub carrier_type: CarrierType,
    pub address: Vec<u8>,
}

impl Endpoint {
    pub const MAX_ADDRESS_SIZE: usize = 1024;

    pub fn new(carrier_type: CarrierType, address: Vec<u8>) -> Result<Self, Error> {
        if address.len() > Self::MAX_ADDRESS_SIZE {
            return Err(Error::EndpointAddressTooLarge {
                size: address.len(),
                max: Self::MAX_ADDRESS_SIZE,
            });
        }
        Ok(Endpoint { carrier_type, address })
    }

    pub fn carrier_type(&self) -> CarrierType {
        self.carrier_type
    }

    pub fn address(&self) -> &[u8] {
        &self.address
    }

    pub fn address_len(&self) -> usize {
        self.address.len()
    }

    pub fn is_loopback(&self) -> bool {
        self.carrier_type == CarrierType::Loopback
    }

    pub fn with_address(mut self, address: Vec<u8>) -> Result<Self, Error> {
        if address.len() > Self::MAX_ADDRESS_SIZE {
            return Err(Error::EndpointAddressTooLarge {
                size: address.len(),
                max: Self::MAX_ADDRESS_SIZE,
            });
        }
        self.address = address;
        Ok(self)
    }

    pub fn into_parts(self) -> (CarrierType, Vec<u8>) {
        (self.carrier_type, self.address)
    }
}

impl fmt::Display for Endpoint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let hex: String = self.address.iter().map(|b| format!("{:02x}", b)).collect();
        write!(f, "Endpoint({}:{})", self.carrier_type, hex)
    }
}

///////////////////////////////////////////////////////////////////////////////
// ROTATING NETWORK-FACING IDENTIFIER (RNI)
// (IDS-B §3.2)
///////////////////////////////////////////////////////////////////////////////

/// Rotating network-facing identifier — changes per rotation policy,
/// maps to stable `NodeId` through a protected mapping.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RotatingNodeId(pub [u8; 16]);

impl RotatingNodeId {
    pub const ZERO: RotatingNodeId = RotatingNodeId([0u8; 16]);

    pub fn new(bytes: [u8; 16]) -> Self {
        Self(bytes)
    }

    pub fn as_bytes(&self) -> &[u8; 16] {
        &self.0
    }

    pub fn is_zero(&self) -> bool {
        self.0 == [0u8; 16]
    }

    pub fn to_hex(&self) -> String {
        self.0.iter().map(|b| format!("{:02x}", b)).collect()
    }

    pub fn from_hex(hex: &str) -> Result<Self, Error> {
        let bytes: Vec<u8> = (0..hex.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&hex[i..i + 2], 16))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| Error::InvalidHex {
                value: hex.to_string(),
            })?;
        if bytes.len() != 16 {
            return Err(Error::InvalidRotatingNodeIdLength {
                len: bytes.len(),
            });
        }
        let mut arr = [0u8; 16];
        arr.copy_from_slice(&bytes);
        Ok(RotatingNodeId(arr))
    }
}

impl fmt::Display for RotatingNodeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "RotatingNodeId({})", self.to_hex())
    }
}

///////////////////////////////////////////////////////////////////////////////
// OPERATION STATES
///////////////////////////////////////////////////////////////////////////////

/// Operation state machine states.
/// received → ordered → executing → committed → synchronized → verified → failed
/// (IDS-A §42, CAPABILITIES §61)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum OpState {
    Received,
    Ordered,
    Executing,
    Committed,
    Synchronized,
    Verified,
    Failed,
}

impl OpState {
    pub fn is_terminal(&self) -> bool {
        matches!(self, OpState::Verified | OpState::Failed)
    }

    pub fn is_committed(&self) -> bool {
        matches!(self, OpState::Committed | OpState::Synchronized | OpState::Verified)
    }

    pub fn can_transition_to(&self, next: OpState) -> bool {
        match self {
            OpState::Received => matches!(next, OpState::Ordered | OpState::Failed),
            OpState::Ordered => matches!(next, OpState::Executing | OpState::Failed),
            OpState::Executing => matches!(next, OpState::Committed | OpState::Failed),
            OpState::Committed => matches!(next, OpState::Synchronized | OpState::Failed),
            OpState::Synchronized => matches!(next, OpState::Verified | OpState::Failed),
            OpState::Verified => false, // terminal
            OpState::Failed => false,   // terminal
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            OpState::Received => "received",
            OpState::Ordered => "ordered",
            OpState::Executing => "executing",
            OpState::Committed => "committed",
            OpState::Synchronized => "synchronized",
            OpState::Verified => "verified",
            OpState::Failed => "failed",
        }
    }
}

impl fmt::Display for OpState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Commit state machine states.
/// accepted → prepared → committed → aborted
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CommitState {
    Accepted,
    Prepared,
    Committed,
    Aborted,
}

impl CommitState {
    pub fn is_terminal(&self) -> bool {
        matches!(self, CommitState::Committed | CommitState::Aborted)
    }

    pub fn is_success(&self) -> bool {
        matches!(self, CommitState::Committed)
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            CommitState::Accepted => "accepted",
            CommitState::Prepared => "prepared",
            CommitState::Committed => "committed",
            CommitState::Aborted => "aborted",
        }
    }
}

impl fmt::Display for CommitState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

///////////////////////////////////////////////////////////////////////////////
// REDUNDANCY AND CONSISTENCY
///////////////////////////////////////////////////////////////////////////////

/// Redundancy mode for dataset placement.
/// (IDS-A §37, CAPABILITIES §41-42, 51)
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RedundancyMode {
    /// Full replication — every replica stores complete data
    FullReplica,
    /// Erasure coding — any k of n chunks reconstructs
    ErasureCoded { k: u32, n: u32 },
    /// Hybrid — both full replica and erasure coded copies coexist
    Hybrid,
}

impl RedundancyMode {
    pub fn is_full_replica(&self) -> bool {
        matches!(self, RedundancyMode::FullReplica)
    }

    pub fn is_erasure_coded(&self) -> bool {
        matches!(self, RedundancyMode::ErasureCoded { .. })
    }

    pub fn is_hybrid(&self) -> bool {
        matches!(self, RedundancyMode::Hybrid)
    }

    pub fn erasure_params(&self) -> Option<(u32, u32)> {
        match self {
            RedundancyMode::ErasureCoded { k, n } => Some((*k, *n)),
            _ => None,
        }
    }

    pub fn failure_tolerance(&self) -> Option<u32> {
        match self {
            RedundancyMode::FullReplica => Some(0), // any single replica loss tolerated via other copies
            RedundancyMode::ErasureCoded { k, n } => Some(n.saturating_sub(k)),
            RedundancyMode::Hybrid => Some(0),
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            RedundancyMode::FullReplica => "full_replica",
            RedundancyMode::ErasureCoded { .. } => "erasure_coded",
            RedundancyMode::Hybrid => "hybrid",
        }
    }
}

impl fmt::Display for RedundancyMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RedundancyMode::FullReplica => write!(f, "full_replica"),
            RedundancyMode::ErasureCoded { k, n } => write!(f, "erasure_coded({}/{})", k, n),
            RedundancyMode::Hybrid => write!(f, "hybrid"),
        }
    }
}

/// Consistency profile for read operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ConsistencyProfile {
    /// Strong consistency — linearizable reads
    Strong,
    /// Causal consistency — causally related operations seen in order
    Causal,
    /// Bounded staleness — reads within a time/version bound
    BoundedStaleness,
    /// Eventual consistency — converges eventually
    Eventual,
    /// Read-your-writes — a client always sees its own writes
    ReadYourWrites,
}

impl ConsistencyProfile {
    pub fn requires_quorum(&self) -> bool {
        matches!(self, ConsistencyProfile::Strong | ConsistencyProfile::BoundedStaleness)
    }

    pub fn allows_fallback(&self) -> bool {
        match self {
            ConsistencyProfile::Strong => false,   // no fallback for strong
            ConsistencyProfile::Causal => true,
            ConsistencyProfile::BoundedStaleness => true,
            ConsistencyProfile::Eventual => true,
            ConsistencyProfile::ReadYourWrites => true,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            ConsistencyProfile::Strong => "strong",
            ConsistencyProfile::Causal => "causal",
            ConsistencyProfile::BoundedStaleness => "bounded_staleness",
            ConsistencyProfile::Eventual => "eventual",
            ConsistencyProfile::ReadYourWrites => "read_your_writes",
        }
    }
}

impl fmt::Display for ConsistencyProfile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

///////////////////////////////////////////////////////////////////////////////
// PLACEMENT DECISION
///////////////////////////////////////////////////////////////////////////////

/// Placement decision — where data lives (capacity-aware, failure-domain).
/// (IDS §5, CAPABILITIES §21-28)
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PlacementDecision {
    pub region: RegionId,
    pub hunit: HUnitId,
    pub node: NodeId,
    pub redundancy: RedundancyMode,
    pub failure_domain: FailureDomain,
}

impl PlacementDecision {
    pub fn new(
        region: RegionId,
        hunit: HUnitId,
        node: NodeId,
        redundancy: RedundancyMode,
        failure_domain: FailureDomain,
    ) -> Self {
        Self { region, hunit, node, redundancy, failure_domain }
    }

    pub fn is_valid(&self) -> bool {
        !self.node.is_zero() && !self.region.is_zero() && !self.hunit.is_zero()
    }

    pub fn as_tuple(&self) -> (RegionId, HUnitId, NodeId) {
        (self.region, self.hunit, self.node)
    }
}

///////////////////////////////////////////////////////////////////////////////
// FAILURE DOMAIN
///////////////////////////////////////////////////////////////////////////////

/// Failure domain — replicas must not share power/facility/network/operator
/// domain to avoid correlated failures. (IDS-EDGE-CASES-RESPONSES #7)
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FailureDomain {
    pub power: Option<[u8; 16]>,
    pub facility: Option<[u8; 16]>,
    pub network_segment: Option<[u8; 16]>,
    pub operator: Option<[u8; 16]>,
    pub geographic: Option<[u8; 16]>,
}

impl FailureDomain {
    pub fn new() -> Self {
        Self {
            power: None,
            facility: None,
            network_segment: None,
            operator: None,
            geographic: None,
        }
    }

    pub fn with_power(mut self, power: [u8; 16]) -> Self {
        self.power = Some(power);
        self
    }

    pub fn with_facility(mut self, facility: [u8; 16]) -> Self {
        self.facility = Some(facility);
        self
    }

    pub fn with_network_segment(mut self, network_segment: [u8; 16]) -> Self {
        self.network_segment = Some(network_segment);
        self
    }

    pub fn with_operator(mut self, operator: [u8; 16]) -> Self {
        self.operator = Some(operator);
        self
    }

    pub fn with_geographic(mut self, geographic: [u8; 16]) -> Self {
        self.geographic = Some(geographic);
        self
    }

    pub fn is_empty(&self) -> bool {
        self.power.is_none()
            && self.facility.is_none()
            && self.network_segment.is_none()
            && self.operator.is_none()
            && self.geographic.is_none()
    }

    pub fn domain_count(&self) -> u32 {
        let mut count = 0;
        if self.power.is_some() { count += 1; }
        if self.facility.is_some() { count += 1; }
        if self.network_segment.is_some() { count += 1; }
        if self.operator.is_some() { count += 1; }
        if self.geographic.is_some() { count += 1; }
        count
    }

    pub fn shares_domain(&self, other: &FailureDomain) -> bool {
        (self.power.is_some() && self.power == other.power)
            || (self.facility.is_some() && self.facility == other.facility)
            || (self.network_segment.is_some() && self.network_segment == other.network_segment)
            || (self.operator.is_some() && self.operator == other.operator)
            || (self.geographic.is_some() && self.geographic == other.geographic)
    }
}

impl Default for FailureDomain {
    fn default() -> Self {
        Self::new()
    }
}

///////////////////////////////////////////////////////////////////////////////
// NODE, H-UNIT, REGION STATE
///////////////////////////////////////////////////////////////////////////////

/// Node lifecycle state.
/// (CAPABILITIES §67, IDS §13)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum NodeState {
    Candidate,
    Validating,
    Active,
    Degraded,
    Draining,
    Failed,
    Retired,
}

impl NodeState {
    pub fn is_active(&self) -> bool {
        matches!(self, NodeState::Active)
    }

    pub fn is_terminal(&self) -> bool {
        matches!(self, NodeState::Failed | NodeState::Retired)
    }

    pub fn can_serve_requests(&self) -> bool {
        matches!(self, NodeState::Active | NodeState::Degraded)
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            NodeState::Candidate => "candidate",
            NodeState::Validating => "validating",
            NodeState::Active => "active",
            NodeState::Degraded => "degraded",
            NodeState::Draining => "draining",
            NodeState::Failed => "failed",
            NodeState::Retired => "retired",
        }
    }
}

impl fmt::Display for NodeState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// H-unit state.
/// (IDS-A §35, CAPABILITIES §3)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum HUnitState {
    Forming,
    Operational,
    Degraded,
    Compensating,
    Retired,
}

impl HUnitState {
    pub fn is_operational(&self) -> bool {
        matches!(self, HUnitState::Operational)
    }

    pub fn is_terminal(&self) -> bool {
        matches!(self, HUnitState::Retired)
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            HUnitState::Forming => "forming",
            HUnitState::Operational => "operational",
            HUnitState::Degraded => "degraded",
            HUnitState::Compensating => "compensating",
            HUnitState::Retired => "retired",
        }
    }
}

impl fmt::Display for HUnitState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Region state.
/// (IDS §9, CAPABILITIES §87)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RegionState {
    Registering,
    Active,
    Isolated,
    Draining,
    Restored,
    Removed,
}

impl RegionState {
    pub fn is_active(&self) -> bool {
        matches!(self, RegionState::Active)
    }

    pub fn is_isolated(&self) -> bool {
        matches!(self, RegionState::Isolated)
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            RegionState::Registering => "registering",
            RegionState::Active => "active",
            RegionState::Isolated => "isolated",
            RegionState::Draining => "draining",
            RegionState::Restored => "restored",
            RegionState::Removed => "removed",
        }
    }
}

impl fmt::Display for RegionState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

///////////////////////////////////////////////////////////////////////////////
// DURABILITY CONDITION
///////////////////////////////////////////////////////////////////////////////

/// Durability acknowledgement condition.
/// (IDS-A §42)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DurabilityCondition {
    /// Operation received by node
    Received,
    /// Operation accepted into local log
    Accepted,
    /// Operation executed locally
    Executed,
    /// Operation committed locally
    LocalCommitted,
    /// Operation acknowledged by required replicas
    ReplicasAcked,
    /// Operation completed cross-region
    CrossRegionCompleted,
}

impl DurabilityCondition {
    pub fn level(&self) -> u32 {
        match self {
            DurabilityCondition::Received => 1,
            DurabilityCondition::Accepted => 2,
            DurabilityCondition::Executed => 3,
            DurabilityCondition::LocalCommitted => 4,
            DurabilityCondition::ReplicasAcked => 5,
            DurabilityCondition::CrossRegionCompleted => 6,
        }
    }

    pub fn meets(&self, required: DurabilityCondition) -> bool {
        self.level() >= required.level()
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            DurabilityCondition::Received => "received",
            DurabilityCondition::Accepted => "accepted",
            DurabilityCondition::Executed => "executed",
            DurabilityCondition::LocalCommitted => "local_committed",
            DurabilityCondition::ReplicasAcked => "replicas_acked",
            DurabilityCondition::CrossRegionCompleted => "cross_region_completed",
        }
    }
}

impl fmt::Display for DurabilityCondition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

///////////////////////////////////////////////////////////////////////////////
// IDS STANDARD TIME
///////////////////////////////////////////////////////////////////////////////

/// IDS standard time — proprietary time basis for ordering/commit,
/// not wall-clock. (IDS-EDGE-CASES-RESPONSES #2)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct IdsTime(pub u64);

impl IdsTime {
    pub const ZERO: IdsTime = IdsTime(0);

    pub fn new(ticks: u64) -> Self {
        Self(ticks)
    }

    pub fn value(&self) -> u64 {
        self.0
    }

    pub fn next(&self) -> Self {
        Self(self.0.saturating_add(1))
    }

    pub fn saturating_sub(&self, other: IdsTime) -> IdsTime {
        IdsTime(self.0.saturating_sub(other.0))
    }

    pub fn is_zero(&self) -> bool {
        self.0 == 0
    }

    pub fn elapsed_since(&self, earlier: IdsTime) -> u64 {
        self.0.saturating_sub(earlier.0)
    }
}

impl fmt::Display for IdsTime {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "IdsTime({})", self.0)
    }
}

///////////////////////////////////////////////////////////////////////////////
// ADDITIONAL CANONICAL TYPES REFERENCED BY CONTRACTS AND INVARIANTS
///////////////////////////////////////////////////////////////////////////////

/// Session identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SessionId(pub [u8; 16]);

impl SessionId {
    pub const ZERO: SessionId = SessionId([0u8; 16]);

    pub fn new(bytes: [u8; 16]) -> Self {
        Self(bytes)
    }

    pub fn is_zero(&self) -> bool {
        self.0 == [0u8; 16]
    }
}

/// Operation result returned by `ids_submit`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpResult {
    pub op_ref: OpRef,
    pub commit_state: CommitState,
    pub durability: DurabilityCondition,
    pub ordering_pos: OrderingPos,
    pub ids_time: IdsTime,
}

impl OpResult {
    pub fn new(
        op_ref: OpRef,
        commit_state: CommitState,
        durability: DurabilityCondition,
        ordering_pos: OrderingPos,
        ids_time: IdsTime,
    ) -> Self {
        Self { op_ref, commit_state, durability, ordering_pos, ids_time }
    }

    pub fn is_success(&self) -> bool {
        self.commit_state.is_success()
    }
}

/// Idempotency key for exactly-once semantics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct IdempotencyKey(pub [u8; 32]);

impl IdempotencyKey {
    pub const ZERO: IdempotencyKey = IdempotencyKey([0u8; 32]);

    pub fn new(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub fn is_zero(&self) -> bool {
        self.0 == [0u8; 32]
    }
}

/// Operation type for authorization checks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum OpType {
    Read,
    Write,
    Delete,
    Replicate,
    Recover,
    Admin,
}

impl OpType {
    pub fn as_str(&self) -> &'static str {
        match self {
            OpType::Read => "read",
            OpType::Write => "write",
            OpType::Delete => "delete",
            OpType::Replicate => "replicate",
            OpType::Recover => "recover",
            OpType::Admin => "admin",
        }
    }
}

/// Operation for submission via `ids_submit`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Operation {
    pub op_ref: OpRef,
    pub dataset_id: DatasetId,
    pub record_id: Option<RecordId>,
    pub op_type: OpType,
    pub payload: Vec<u8>,
    pub durability: DurabilityCondition,
    pub consistency: ConsistencyProfile,
    pub ids_time: IdsTime,
    pub idempotency_key: IdempotencyKey,
}

impl Operation {
    pub fn new(
        op_ref: OpRef,
        dataset_id: DatasetId,
        op_type: OpType,
        payload: Vec<u8>,
    ) -> Self {
        Self {
            op_ref,
            dataset_id,
            record_id: None,
            op_type,
            payload,
            durability: DurabilityCondition::Received,
            consistency: ConsistencyProfile::Strong,
            ids_time: IdsTime::ZERO,
            idempotency_key: IdempotencyKey::ZERO,
        }
    }

    pub fn payload_size(&self) -> usize {
        self.payload.len()
    }

    pub fn is_empty(&self) -> bool {
        self.payload.is_empty()
    }
}

/// Version for MVCC and version vectors.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Version(pub u64);

impl Version {
    pub const ZERO: Version = Version(0);

    pub fn new(v: u64) -> Self {
        Self(v)
    }

    pub fn value(&self) -> u64 {
        self.0
    }

    pub fn next(&self) -> Self {
        Self(self.0.saturating_add(1))
    }
}

/// Epoch for lease and coordinator terms.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Epoch(pub u64);

impl Epoch {
    pub const ZERO: Epoch = Epoch(0);

    pub fn new(e: u64) -> Self {
        Self(e)
    }

    pub fn value(&self) -> u64 {
        self.0
    }

    pub fn next(&self) -> Self {
        Self(self.0.saturating_add(1))
    }
}

/// Ordering domain identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct OrderingDomain(pub [u8; 32]);

impl OrderingDomain {
    pub const ZERO: OrderingDomain = OrderingDomain([0u8; 32]);

    pub fn new(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub fn is_zero(&self) -> bool {
        self.0 == [0u8; 32]
    }
}

/// Capacity for membership and placement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Capacity {
    pub storage_bytes: u64,
    pub memory_bytes: u64,
    pub cpu_cores: u32,
    pub network_bps: u64,
}

impl Capacity {
    pub const ZERO: Capacity = Capacity {
        storage_bytes: 0,
        memory_bytes: 0,
        cpu_cores: 0,
        network_bps: 0,
    };

    pub fn new(storage_bytes: u64, memory_bytes: u64, cpu_cores: u32, network_bps: u64) -> Self {
        Self { storage_bytes, memory_bytes, cpu_cores, network_bps }
    }

    pub fn is_zero(&self) -> bool {
        self.storage_bytes == 0 && self.memory_bytes == 0 && self.cpu_cores == 0 && self.network_bps == 0
    }

    pub fn total_bytes(&self) -> u64 {
        self.storage_bytes + self.memory_bytes
    }

    pub fn can_serve(&self, required: &Capacity) -> bool {
        self.storage_bytes >= required.storage_bytes
            && self.memory_bytes >= required.memory_bytes
            && self.cpu_cores >= required.cpu_cores
            && self.network_bps >= required.network_bps
    }
}

/// Snapshot identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SnapshotId(pub [u8; 16]);

impl SnapshotId {
    pub const ZERO: SnapshotId = SnapshotId([0u8; 16]);

    pub fn new(bytes: [u8; 16]) -> Self {
        Self(bytes)
    }

    pub fn is_zero(&self) -> bool {
        self.0 == [0u8; 16]
    }
}

/// Channel identifier for snapshot marker messages.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ChannelId(pub u64);

impl ChannelId {
    pub const ZERO: ChannelId = ChannelId(0);

    pub fn new(id: u64) -> Self {
        Self(id)
    }

    pub fn value(&self) -> u64 {
        self.0
    }
}

/// CRDT state (opaque type for CRDT operations).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CRDTState {
    pub inner: Vec<u8>,
}

impl CRDTState {
    pub fn new(data: Vec<u8>) -> Self {
        Self { inner: data }
    }

    pub fn empty() -> Self {
        Self { inner: Vec::new() }
    }

    pub fn len(&self) -> usize {
        self.inner.len()
    }

    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }
}

/// CRDT operation (opaque type for operation-based CRDTs).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CRDTOp {
    pub inner: Vec<u8>,
}

impl CRDTOp {
    pub fn new(data: Vec<u8>) -> Self {
        Self { inner: data }
    }

    pub fn len(&self) -> usize {
        self.inner.len()
    }
}

/// CRDT delta (opaque type for delta-CRDTs).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CRDTDelta {
    pub inner: Vec<u8>,
}

impl CRDTDelta {
    pub fn new(data: Vec<u8>) -> Self {
        Self { inner: data }
    }

    pub fn len(&self) -> usize {
        self.inner.len()
    }
}

/// Storage tier for hot/cold tiering.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Tier {
    Hot,
    Warm,
    Cold,
}

impl Tier {
    pub fn as_str(&self) -> &'static str {
        match self {
            Tier::Hot => "hot",
            Tier::Warm => "warm",
            Tier::Cold => "cold",
        }
    }

    pub fn rank(&self) -> u32 {
        match self {
            Tier::Hot => 0,
            Tier::Warm => 1,
            Tier::Cold => 2,
        }
    }

    pub fn is_hotter_than(&self, other: Tier) -> bool {
        self.rank() < other.rank()
    }
}

/// Credential for authentication.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Credential {
    pub identity: NodeId,
    pub token: Vec<u8>,
}

impl Credential {
    pub fn new(identity: NodeId, token: Vec<u8>) -> Self {
        Self { identity, token }
    }

    pub fn token_len(&self) -> usize {
        self.token.len()
    }
}

/// Authentication token.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AuthToken(pub [u8; 32]);

impl AuthToken {
    pub const ZERO: AuthToken = AuthToken([0u8; 32]);

    pub fn new(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub fn is_zero(&self) -> bool {
        self.0 == [0u8; 32]
    }
}

/// Key identifier for cryptographic keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct KeyId(pub [u8; 16]);

impl KeyId {
    pub const ZERO: KeyId = KeyId([0u8; 16]);

    pub fn new(bytes: [u8; 16]) -> Self {
        Self(bytes)
    }

    pub fn is_zero(&self) -> bool {
        self.0 == [0u8; 16]
    }
}

/// Signature for authenticated broadcast.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Signature(pub [u8; 64]);

impl Signature {
    pub const ZERO: Signature = Signature([0u8; 64]);

    pub fn new(bytes: [u8; 64]) -> Self {
        Self(bytes)
    }

    pub fn is_zero(&self) -> bool {
        self.0 == [0u8; 64]
    }
}

/// Value for Byzantine consensus.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Value {
    pub inner: Vec<u8>,
}

impl Value {
    pub fn new(data: Vec<u8>) -> Self {
        Self { inner: data }
    }

    pub fn len(&self) -> usize {
        self.inner.len()
    }

    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }
}

/// Message for broadcast protocols.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Message {
    pub id: MessageId,
    pub sender: NodeId,
    pub payload: Vec<u8>,
    pub ids_time: IdsTime,
}

impl Message {
    pub fn new(id: MessageId, sender: NodeId, payload: Vec<u8>, ids_time: IdsTime) -> Self {
        Self { id, sender, payload, ids_time }
    }

    pub fn len(&self) -> usize {
        self.payload.len()
    }
}

/// Message identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MessageId(pub [u8; 16]);

impl MessageId {
    pub const ZERO: MessageId = MessageId([0u8; 16]);

    pub fn new(bytes: [u8; 16]) -> Self {
        Self(bytes)
    }

    pub fn is_zero(&self) -> bool {
        self.0 == [0u8; 16]
    }
}

/// Circuit breaker state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CircuitState {
    Closed,
    Open,
    HalfOpen,
}

impl CircuitState {
    pub fn as_str(&self) -> &'static str {
        match self {
            CircuitState::Closed => "closed",
            CircuitState::Open => "open",
            CircuitState::HalfOpen => "half_open",
        }
    }
}

/// Degradation level for graceful degradation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum DegradationLevel {
    None,
    Minor,
    Moderate,
    Severe,
    Critical,
}

impl DegradationLevel {
    pub fn as_str(&self) -> &'static str {
        match self {
            DegradationLevel::None => "none",
            DegradationLevel::Minor => "minor",
            DegradationLevel::Moderate => "moderate",
            DegradationLevel::Severe => "severe",
            DegradationLevel::Critical => "critical",
        }
    }

    pub fn rank(&self) -> u32 {
        match self {
            DegradationLevel::None => 0,
            DegradationLevel::Minor => 1,
            DegradationLevel::Moderate => 2,
            DegradationLevel::Severe => 3,
            DegradationLevel::Critical => 4,
        }
    }
}

/// Lease identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct LeaseId(pub [u8; 16]);

impl LeaseId {
    pub const ZERO: LeaseId = LeaseId([0u8; 16]);

    pub fn new(bytes: [u8; 16]) -> Self {
        Self(bytes)
    }

    pub fn is_zero(&self) -> bool {
        self.0 == [0u8; 16]
    }
}

/// Fencing token for lease-based fencing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct FencingToken(pub u64);

impl FencingToken {
    pub const ZERO: FencingToken = FencingToken(0);

    pub fn new(v: u64) -> Self {
        Self(v)
    }

    pub fn value(&self) -> u64 {
        self.0
    }

    pub fn next(&self) -> Self {
        Self(self.0.saturating_add(1))
    }
}

/// Saga identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SagaId(pub [u8; 16]);

impl SagaId {
    pub const ZERO: SagaId = SagaId([0u8; 16]);

    pub fn new(bytes: [u8; 16]) -> Self {
        Self(bytes)
    }

    pub fn is_zero(&self) -> bool {
        self.0 == [0u8; 16]
    }
}

/// Step identifier within a saga.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct StepId(pub u64);

impl StepId {
    pub const ZERO: StepId = StepId(0);

    pub fn new(id: u64) -> Self {
        Self(id)
    }

    pub fn value(&self) -> u64 {
        self.0
    }
}

/// Local transaction for saga orchestration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalTransaction {
    pub step_id: StepId,
    pub dataset_id: DatasetId,
    pub payload: Vec<u8>,
    pub compensate_payload: Vec<u8>,
}

impl LocalTransaction {
    pub fn new(step_id: StepId, dataset_id: DatasetId, payload: Vec<u8>, compensate_payload: Vec<u8>) -> Self {
        Self { step_id, dataset_id, payload, compensate_payload }
    }
}

/// Transaction identifier for cross-domain transactions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TransactionId(pub [u8; 16]);

impl TransactionId {
    pub const ZERO: TransactionId = TransactionId([0u8; 16]);

    pub fn new(bytes: [u8; 16]) -> Self {
        Self(bytes)
    }

    pub fn is_zero(&self) -> bool {
        self.0 == [0u8; 16]
    }
}

/// Failure event for recovery scope classification.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FailureEvent {
    pub event_type: FailureEventType,
    pub affected_node: Option<NodeId>,
    pub affected_hunit: Option<HUnitId>,
    pub affected_region: Option<RegionId>,
    pub ids_time: IdsTime,
}

impl FailureEvent {
    pub fn new(
        event_type: FailureEventType,
        affected_node: Option<NodeId>,
        affected_hunit: Option<HUnitId>,
        affected_region: Option<RegionId>,
        ids_time: IdsTime,
    ) -> Self {
        Self { event_type, affected_node, affected_hunit, affected_region, ids_time }
    }

    pub fn is_node_failure(&self) -> bool {
        matches!(self.event_type, FailureEventType::NodeFailure)
    }

    pub fn is_hunit_failure(&self) -> bool {
        matches!(self.event_type, FailureEventType::HUnitFailure)
    }

    pub fn is_regional_failure(&self) -> bool {
        matches!(self.event_type, FailureEventType::RegionalFailure)
    }
}

/// Failure event type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FailureEventType {
    NodeFailure,
    HUnitFailure,
    RegionalFailure,
    NetworkPartition,
    DiskFailure,
    PowerFailure,
}

/// Recovery scope level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum RecoveryScopeLevel {
    Node,
    HUnit,
    Region,
}

impl RecoveryScopeLevel {
    pub fn as_str(&self) -> &'static str {
        match self {
            RecoveryScopeLevel::Node => "node",
            RecoveryScopeLevel::HUnit => "hunit",
            RecoveryScopeLevel::Region => "region",
        }
    }

    pub fn rank(&self) -> u32 {
        match self {
            RecoveryScopeLevel::Node => 1,
            RecoveryScopeLevel::HUnit => 2,
            RecoveryScopeLevel::Region => 3,
        }
    }
}

/// Data lifecycle state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum LifecycleState {
    Creating,
    Active,
    Archiving,
    Archived,
    Deleting,
    Deleted,
}

impl LifecycleState {
    pub fn as_str(&self) -> &'static str {
        match self {
            LifecycleState::Creating => "creating",
            LifecycleState::Active => "active",
            LifecycleState::Archiving => "archiving",
            LifecycleState::Archived => "archived",
            LifecycleState::Deleting => "deleting",
            LifecycleState::Deleted => "deleted",
        }
    }

    pub fn is_terminal(&self) -> bool {
        matches!(self, LifecycleState::Archived | LifecycleState::Deleted)
    }
}

/// Compute request for compute-to-data execution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComputeRequest {
    pub dataset_id: DatasetId,
    pub code_hash: [u8; 32],
    pub input: Vec<u8>,
}

impl ComputeRequest {
    pub fn new(dataset_id: DatasetId, code_hash: [u8; 32], input: Vec<u8>) -> Self {
        Self { dataset_id, code_hash, input }
    }
}

/// Compute result from compute-to-data execution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComputeResult {
    pub output: Vec<u8>,
    pub proof: Vec<u8>,
}

impl ComputeResult {
    pub fn new(output: Vec<u8>, proof: Vec<u8>) -> Self {
        Self { output, proof }
    }
}

/// Compacted result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompactResult {
    pub data: Vec<u8>,
    pub compression_ratio: f64,
}

impl CompactResult {
    pub fn new(data: Vec<u8>, compression_ratio: f64) -> Self {
        Self { data, compression_ratio }
    }
}

/// Announcement for gossip-based discovery.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Announcement {
    pub node_id: NodeId,
    pub endpoint: Endpoint,
    pub capabilities: Vec<u8>,
    pub ids_time: IdsTime,
}

impl Announcement {
    pub fn new(node_id: NodeId, endpoint: Endpoint, capabilities: Vec<u8>, ids_time: IdsTime) -> Self {
        Self { node_id, endpoint, capabilities, ids_time }
    }
}

/// Change stream for replication.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChangeStream {
    pub dataset_id: DatasetId,
    pub from_version: Version,
    pub to_version: Version,
    pub changes: Vec<Vec<u8>>,
}

impl ChangeStream {
    pub fn new(dataset_id: DatasetId, from_version: Version, to_version: Version, changes: Vec<Vec<u8>>) -> Self {
        Self { dataset_id, from_version, to_version, changes }
    }

    pub fn change_count(&self) -> usize {
        self.changes.len()
    }
}

/// Fragment identifier for erasure coding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FragmentId(pub [u8; 16]);

impl FragmentId {
    pub const ZERO: FragmentId = FragmentId([0u8; 16]);

    pub fn new(bytes: [u8; 16]) -> Self {
        Self(bytes)
    }

    pub fn is_zero(&self) -> bool {
        self.0 == [0u8; 16]
    }
}

/// Recovery proof for post-recovery verification.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecoveryProof {
    pub dataset_id: DatasetId,
    pub recovered_fragments: Vec<FragmentId>,
    pub checksum: Vec<u8>,
    pub ids_time: IdsTime,
}

impl RecoveryProof {
    pub fn new(dataset_id: DatasetId, recovered_fragments: Vec<FragmentId>, checksum: Vec<u8>, ids_time: IdsTime) -> Self {
        Self { dataset_id, recovered_fragments, checksum, ids_time }
    }
}

/// Checkpoint identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CheckpointId(pub [u8; 16]);

impl CheckpointId {
    pub const ZERO: CheckpointId = CheckpointId([0u8; 16]);

    pub fn new(bytes: [u8; 16]) -> Self {
        Self(bytes)
    }

    pub fn is_zero(&self) -> bool {
        self.0 == [0u8; 16]
    }
}

/// Anomaly detected by daily integrity checks.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Anomaly {
    pub node_id: NodeId,
    pub anomaly_type: AnomalyType,
    pub description: String,
    pub ids_time: IdsTime,
}

impl Anomaly {
    pub fn new(node_id: NodeId, anomaly_type: AnomalyType, description: String, ids_time: IdsTime) -> Self {
        Self { node_id, anomaly_type, description, ids_time }
    }
}

/// Anomaly type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AnomalyType {
    ByteMismatch,
    ReplicaDrift,
    ChecksumFailure,
    StateCorruption,
}

/// Roster change for node roster updates.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RosterChange {
    pub client_id: NodeId,
    pub added: Vec<NodeId>,
    pub removed: Vec<NodeId>,
    pub ids_time: IdsTime,
}

impl RosterChange {
    pub fn new(client_id: NodeId, added: Vec<NodeId>, removed: Vec<NodeId>, ids_time: IdsTime) -> Self {
        Self { client_id, added, removed, ids_time }
    }
}

/// Checksum for end-to-end integrity verification.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Checksum(pub [u8; 32]);

impl Checksum {
    pub const ZERO: Checksum = Checksum([0u8; 32]);

    pub fn new(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub fn is_zero(&self) -> bool {
        self.0 == [0u8; 32]
    }
}

/// Delivery semantics for transport.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DeliverySemantics {
    AtMostOnce,
    AtLeastOnce,
    ExactlyOnce,
}

/// Acknowledgement state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AckState {
    Pending,
    Acked,
    Nacked,
    Timeout,
}

/// Circuit breaker state (alias for fault module).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CircuitBreakerState {
    Closed,
    Open,
    HalfOpen,
}

/// Delivery semantics (alias for transport module).
pub use DeliverySemantics as TransportDeliverySemantics;

///////////////////////////////////////////////////////////////////////////////
// CONTROL PLANE TYPES REFERENCED BY CONTRACTS AND INVARIANTS
///////////////////////////////////////////////////////////////////////////////

/// Protocol version for wire format versioning.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ProtocolVersion {
    pub major: u16,
    pub minor: u16,
    pub patch: u16,
}

impl ProtocolVersion {
    pub const V0_1_0: ProtocolVersion = ProtocolVersion {
        major: 0,
        minor: 1,
        patch: 0,
    };

    pub fn new(major: u16, minor: u16, patch: u16) -> Self {
        Self { major, minor, patch }
    }

    pub fn as_u64(&self) -> u64 {
        ((self.major as u64) << 32) | ((self.minor as u64) << 16) | (self.patch as u64)
    }

    pub fn is_compatible_with(&self, other: ProtocolVersion) -> bool {
        self.major == other.major
    }

    pub fn to_string_compact(&self) -> String {
        format!("{}.{}.{}", self.major, self.minor, self.patch)
    }
}

impl Default for ProtocolVersion {
    fn default() -> Self {
        Self::V0_1_0
    }
}

impl fmt::Display for ProtocolVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

/// Edge case version bits as a compile-time bitfield (u128).
/// Each bit corresponds to one invariant in `src/invariants/`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EdgeCaseBits(pub u128);

impl EdgeCaseBits {
    pub const ZERO: EdgeCaseBits = EdgeCaseBits(0);
    pub const ALL: EdgeCaseBits = EdgeCaseBits(u128::MAX);

    pub const BINARY_COMPLETION: u128 = 1 << 0;
    pub const DEAD_NODE_IDENTITY: u128 = 1 << 1;
    pub const IDLE_ROTATION: u128 = 1 << 2;
    pub const NO_GLOBAL_MASTER: u128 = 1 << 3;
    pub const OUT_EQ_IN: u128 = 1 << 4;
    pub const COORDINATED_DELETION: u128 = 1 << 5;
    pub const FENCING_STONITH: u128 = 1 << 6;
    pub const QUORUM_INTERSECTION: u128 = 1 << 7;
    pub const ERASURE_THRESHOLD: u128 = 1 << 8;
    pub const CORRELATED_FAILURE: u128 = 1 << 9;
    pub const HUNIT_COMPENSATION: u128 = 1 << 10;
    pub const REGIONAL_AUTONOMY: u128 = 1 << 11;
    pub const HYBRID_REDUNDANCY: u128 = 1 << 12;
    pub const OPERATION_LIFECYCLE: u128 = 1 << 13;
    pub const DURABILITY_ACK: u128 = 1 << 14;
    pub const ORDERED_READ_VISIBILITY: u128 = 1 << 15;
    pub const NODE_ROSTER: u128 = 1 << 16;
    pub const REGION_AFFINITY: u128 = 1 << 17;
    pub const IDENTITY_ROTATION: u128 = 1 << 18;
    pub const IDS_STANDARD_TIME: u128 = 1 << 19;
    pub const DAILY_INTEGRITY: u128 = 1 << 20;
    pub const DB_RETIREMENT: u128 = 1 << 21;
    pub const READ_FALLBACK: u128 = 1 << 22;
    pub const TOPOLOGY_DIVERSITY: u128 = 1 << 23;
    pub const WRITE_FANOUT: u128 = 1 << 24;
    pub const CUSTODY_GRAPH: u128 = 1 << 25;
    pub const CHECKSUM_BOUNDARY: u128 = 1 << 26;
    pub const REPLICA_FRESHNESS: u128 = 1 << 27;
    pub const COMMIT_PREFIX: u128 = 1 << 28;
    pub const VERSION_VECTOR: u128 = 1 << 29;
    pub const MEMBERSHIP_EPOCH: u128 = 1 << 30;
    pub const PLACEMENT_POLICY: u128 = 1 << 31;

    pub fn new(bits: u128) -> Self {
        Self(bits)
    }

    pub fn value(&self) -> u128 {
        self.0
    }

    pub fn has(&self, bit: u128) -> bool {
        (self.0 & bit) != 0
    }

    pub fn has_all(&self, bits: u128) -> bool {
        (self.0 & bits) == bits
    }

    pub fn has_any(&self, bits: u128) -> bool {
        (self.0 & bits) != 0
    }

    pub fn enable(&mut self, bit: u128) {
        self.0 |= bit;
    }

    pub fn disable(&mut self, bit: u128) {
        self.0 &= !bit;
    }

    pub fn toggle(&mut self, bit: u128) {
        self.0 ^= bit;
    }

    pub fn count_enabled(&self) -> u32 {
        self.0.count_ones()
    }

    pub fn enabled_bits(&self) -> Vec<u128> {
        let mut result = Vec::new();
        let mut bits = self.0;
        let mut idx = 0u128;
        while bits != 0 {
            if bits & 1 != 0 {
                result.push(1 << idx);
            }
            bits >>= 1;
            idx += 1;
        }
        result
    }

    pub fn is_empty(&self) -> bool {
        self.0 == 0
    }
}

impl Default for EdgeCaseBits {
    fn default() -> Self {
        Self::ZERO
    }
}

impl fmt::Display for EdgeCaseBits {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "EdgeCaseBits(0x{:032x})", self.0)
    }
}

/// Configuration for ids-000-aa central hub.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HubConfig {
    pub protocol_version: ProtocolVersion,
    pub edge_case_bits: EdgeCaseBits,
    pub max_endpoint_address_size: usize,
    pub default_consistency: ConsistencyProfile,
    pub default_redundancy: RedundancyMode,
    pub default_durability: DurabilityCondition,
    pub enable_binary_completion: bool,
    pub enable_dead_node_identity: bool,
    pub enable_idle_rotation: bool,
    pub enable_coordinated_deletion: bool,
    pub enable_fencing_stonith: bool,
    pub enable_daily_integrity: bool,
    pub enable_read_fallback: bool,
    pub enable_topology_diversity: bool,
}

impl HubConfig {
    pub fn default_for_testing() -> Self {
        Self {
            protocol_version: ProtocolVersion::V0_1_0,
            edge_case_bits: EdgeCaseBits::ALL,
            max_endpoint_address_size: Endpoint::MAX_ADDRESS_SIZE,
            default_consistency: ConsistencyProfile::Strong,
            default_redundancy: RedundancyMode::FullReplica,
            default_durability: DurabilityCondition::ReplicasAcked,
            enable_binary_completion: true,
            enable_dead_node_identity: true,
            enable_idle_rotation: true,
            enable_coordinated_deletion: true,
            enable_fencing_stonith: true,
            enable_daily_integrity: true,
            enable_read_fallback: true,
            enable_topology_diversity: true,
        }
    }

    pub fn validate(&self) -> Result<(), Error> {
        if self.max_endpoint_address_size == 0 {
            return Err(Error::InvalidEndpointAddressSize {
                size: self.max_endpoint_address_size,
            });
        }
        if self.protocol_version.major == 0 && self.protocol_version.minor == 0 {
            return Err(Error::UnsupportedProtocolVersion {
                version: self.protocol_version.to_string_compact(),
            });
        }
        Ok(())
    }

    pub fn edge_case_enabled(&self, bit: u128) -> bool {
        self.edge_case_bits.has(bit)
    }
}

impl Default for HubConfig {
    fn default() -> Self {
        Self {
            protocol_version: ProtocolVersion::default(),
            edge_case_bits: EdgeCaseBits::ALL,
            max_endpoint_address_size: Endpoint::MAX_ADDRESS_SIZE,
            default_consistency: ConsistencyProfile::Strong,
            default_redundancy: RedundancyMode::FullReplica,
            default_durability: DurabilityCondition::ReplicasAcked,
            enable_binary_completion: true,
            enable_dead_node_identity: true,
            enable_idle_rotation: true,
            enable_coordinated_deletion: true,
            enable_fencing_stonith: true,
            enable_daily_integrity: true,
            enable_read_fallback: true,
            enable_topology_diversity: true,
        }
    }
}

/// Canonical error type for ids-000-aa (defined in errors.rs, re-exported here).
/// This type alias ensures all downstream crates reference the same Error.
pub type HubError = crate::errors::Error;

/// Type-safe wrapper for carrier-specific opaque data.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CarrierPayload {
    pub carrier_type: CarrierType,
    pub payload: Vec<u8>,
    pub ids_time: IdsTime,
}

impl CarrierPayload {
    pub fn new(carrier_type: CarrierType, payload: Vec<u8>, ids_time: IdsTime) -> Self {
        Self { carrier_type, payload, ids_time }
    }

    pub fn len(&self) -> usize {
        self.payload.len()
    }

    pub fn is_empty(&self) -> bool {
        self.payload.is_empty()
    }

    pub fn carrier_name(&self) -> &'static str {
        self.carrier_type.name()
    }
}

/// Registry entry for type IDs used by the internal type registry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TypeId(pub u64);

impl TypeId {
    pub const ZERO: TypeId = TypeId(0);
    pub const NODE_ID: TypeId = TypeId(1);
    pub const REGION_ID: TypeId = TypeId(2);
    pub const HUNIT_ID: TypeId = TypeId(3);
    pub const DATASET_ID: TypeId = TypeId(4);
    pub const PARTITION_ID: TypeId = TypeId(5);
    pub const RECORD_ID: TypeId = TypeId(6);
    pub const OP_REF: TypeId = TypeId(7);
    pub const ORDERING_POS: TypeId = TypeId(8);
    pub const ENDPOINT: TypeId = TypeId(9);
    pub const ROTATING_NODE_ID: TypeId = TypeId(10);

    pub fn new(id: u64) -> Self {
        Self(id)
    }

    pub fn value(&self) -> u64 {
        self.0
    }

    pub fn is_zero(&self) -> bool {
        self.0 == 0
    }

    pub fn name(&self) -> &'static str {
        match self.0 {
            1 => "NodeId",
            2 => "RegionId",
            3 => "HUnitId",
            4 => "DatasetId",
            5 => "PartitionId",
            6 => "RecordId",
            7 => "OpRef",
            8 => "OrderingPos",
            9 => "Endpoint",
            10 => "RotatingNodeId",
            _ => "unknown",
        }
    }
}

impl fmt::Display for TypeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "TypeId({}:{})", self.0, self.name())
    }
}

/// Nine-character operation reference allocator result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpRefAllocation {
    pub op_ref: OpRef,
    pub is_unique: bool,
    pub collision_count: u32,
}

impl OpRefAllocation {
    pub fn new(op_ref: OpRef, is_unique: bool, collision_count: u32) -> Self {
        Self { op_ref, is_unique, collision_count }
    }

    pub fn is_valid(&self) -> bool {
        self.is_unique && !self.op_ref.is_zero()
    }
}

/// Ordering position clock snapshot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrderingPosSnapshot {
    pub domain: OrderingDomain,
    pub position: OrderingPos,
    pub ids_time: IdsTime,
}

impl OrderingPosSnapshot {
    pub fn new(domain: OrderingDomain, position: OrderingPos, ids_time: IdsTime) -> Self {
        Self { domain, position, ids_time }
    }

    pub fn is_before(&self, other: &OrderingPosSnapshot) -> bool {
        self.position < other.position
    }

    pub fn is_after(&self, other: &OrderingPosSnapshot) -> bool {
        self.position > other.position
    }
}

/// Durability tracking state per operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DurabilityTrackState {
    NotStarted,
    InProgress,
    ReplicasAcked,
    CrossRegionCompleted,
    Failed,
}

impl DurabilityTrackState {
    pub fn as_str(&self) -> &'static str {
        match self {
            DurabilityTrackState::NotStarted => "not_started",
            DurabilityTrackState::InProgress => "in_progress",
            DurabilityTrackState::ReplicasAcked => "replicas_acked",
            DurabilityTrackState::CrossRegionCompleted => "cross_region_completed",
            DurabilityTrackState::Failed => "failed",
        }
    }

    pub fn is_terminal(&self) -> bool {
        matches!(self, DurabilityTrackState::CrossRegionCompleted | DurabilityTrackState::Failed)
    }

    pub fn is_success(&self) -> bool {
        matches!(self, DurabilityTrackState::CrossRegionCompleted)
    }
}

impl fmt::Display for DurabilityTrackState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Recovery plan step.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecoveryStep {
    pub step_id: u64,
    pub scope: RecoveryScopeLevel,
    pub target_node: Option<NodeId>,
    pub target_hunit: Option<HUnitId>,
    pub target_region: Option<RegionId>,
    pub action: RecoveryAction,
    pub completed: bool,
}

impl RecoveryStep {
    pub fn new(
        step_id: u64,
        scope: RecoveryScopeLevel,
        action: RecoveryAction,
    ) -> Self {
        Self {
            step_id,
            scope,
            target_node: None,
            target_hunit: None,
            target_region: None,
            action,
            completed: false,
        }
    }

    pub fn mark_completed(&mut self) {
        self.completed = true;
    }

    pub fn is_completed(&self) -> bool {
        self.completed
    }
}

/// Recovery action type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RecoveryAction {
    ReconstructErasure,
    CopyReplica,
    TransferOwnership,
    RestoreRedundancy,
    VerifyIntegrity,
    QuarantineNode,
}

impl RecoveryAction {
    pub fn as_str(&self) -> &'static str {
        match self {
            RecoveryAction::ReconstructErasure => "reconstruct_erasure",
            RecoveryAction::CopyReplica => "copy_replica",
            RecoveryAction::TransferOwnership => "transfer_ownership",
            RecoveryAction::RestoreRedundancy => "restore_redundancy",
            RecoveryAction::VerifyIntegrity => "verify_integrity",
            RecoveryAction::QuarantineNode => "quarantine_node",
        }
    }
}

/// Consistency profile selection result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConsistencySelection {
    pub dataset_id: DatasetId,
    pub selected: ConsistencyProfile,
    pub reason: ConsistencySelectionReason,
    pub ids_time: IdsTime,
}

impl ConsistencySelection {
    pub fn new(
        dataset_id: DatasetId,
        selected: ConsistencyProfile,
        reason: ConsistencySelectionReason,
        ids_time: IdsTime,
    ) -> Self {
        Self { dataset_id, selected, reason, ids_time }
    }
}

/// Reason for consistency profile selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ConsistencySelectionReason {
    DatasetDefault,
    ClientOverride,
    RegionPolicy,
    FailureDomainConstraint,
    AdminForced,
}

impl ConsistencySelectionReason {
    pub fn as_str(&self) -> &'static str {
        match self {
            ConsistencySelectionReason::DatasetDefault => "dataset_default",
            ConsistencySelectionReason::ClientOverride => "client_override",
            ConsistencySelectionReason::RegionPolicy => "region_policy",
            ConsistencySelectionReason::FailureDomainConstraint => "failure_domain_constraint",
            ConsistencySelectionReason::AdminForced => "admin_forced",
        }
    }
}

/// Broadcast topology configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BroadcastTopologyConfig {
    pub protocol: BroadcastProtocol,
    pub fanout_factor: u32,
    pub max_hops: u32,
    pub require_acks: bool,
}

impl BroadcastTopologyConfig {
    pub fn new(
        protocol: BroadcastProtocol,
        fanout_factor: u32,
        max_hops: u32,
        require_acks: bool,
    ) -> Self {
        Self { protocol, fanout_factor, max_hops, require_acks }
    }
}

/// Broadcast protocol type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BroadcastProtocol {
    Reliable,
    Causal,
    TotalOrder,
    Atomic,
}

impl BroadcastProtocol {
    pub fn as_str(&self) -> &'static str {
        match self {
            BroadcastProtocol::Reliable => "reliable",
            BroadcastProtocol::Causal => "causal",
            BroadcastProtocol::TotalOrder => "total_order",
            BroadcastProtocol::Atomic => "atomic",
        }
    }
}

/// Quorum calculation result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuorumResult {
    pub read_set: Vec<NodeId>,
    pub write_set: Vec<NodeId>,
    pub intersection: Vec<NodeId>,
    pub quorum_satisfied: bool,
}

impl QuorumResult {
    pub fn new(
        read_set: Vec<NodeId>,
        write_set: Vec<NodeId>,
        intersection: Vec<NodeId>,
        quorum_satisfied: bool,
    ) -> Self {
        Self { read_set, write_set, intersection, quorum_satisfied }
    }

    pub fn intersection_size(&self) -> usize {
        self.intersection.len()
    }

    pub fn is_valid(&self) -> bool {
        self.quorum_satisfied && !self.intersection.is_empty()
    }
}

/// Snapshot coordination result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnapshotResult {
    pub snapshot_id: SnapshotId,
    pub initiator: NodeId,
    pub participants: Vec<NodeId>,
    pub consistent: bool,
    pub ids_time: IdsTime,
}

impl SnapshotResult {
    pub fn new(
        snapshot_id: SnapshotId,
        initiator: NodeId,
        participants: Vec<NodeId>,
        consistent: bool,
        ids_time: IdsTime,
    ) -> Self {
        Self { snapshot_id, initiator, participants, consistent, ids_time }
    }

    pub fn participant_count(&self) -> usize {
        self.participants.len()
    }
}

/// Byzantine consensus result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ByzantineResult {
    pub agreed_value: Value,
    pub participating_replicas: Vec<NodeId>,
    pub faulty_replicas: Vec<NodeId>,
    pub consensus_reached: bool,
    pub ids_time: IdsTime,
}

impl ByzantineResult {
    pub fn new(
        agreed_value: Value,
        participating_replicas: Vec<NodeId>,
        faulty_replicas: Vec<NodeId>,
        consensus_reached: bool,
        ids_time: IdsTime,
    ) -> Self {
        Self { agreed_value, participating_replicas, faulty_replicas, consensus_reached, ids_time }
    }

    pub fn fault_count(&self) -> usize {
        self.faulty_replicas.len()
    }
}

/// Lease grant result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LeaseResult {
    pub lease_id: LeaseId,
    pub holder: NodeId,
    pub epoch: Epoch,
    pub fencing_token: FencingToken,
    pub expires_at: IdsTime,
    pub active: bool,
}

impl LeaseResult {
    pub fn new(
        lease_id: LeaseId,
        holder: NodeId,
        epoch: Epoch,
        fencing_token: FencingToken,
        expires_at: IdsTime,
        active: bool,
    ) -> Self {
        Self { lease_id, holder, epoch, fencing_token, expires_at, active }
    }

    pub fn is_expired(&self, current_time: IdsTime) -> bool {
        current_time > self.expires_at
    }
}

/// Saga transaction result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SagaResult {
    pub saga_id: SagaId,
    pub coordinator: Option<NodeId>,
    pub steps_completed: u64,
    pub steps_total: u64,
    pub compensated: bool,
    pub completed: bool,
}

impl SagaResult {
    pub fn new(
        saga_id: SagaId,
        coordinator: Option<NodeId>,
        steps_completed: u64,
        steps_total: u64,
        compensated: bool,
        completed: bool,
    ) -> Self {
        Self { saga_id, coordinator, steps_completed, steps_total, compensated, completed }
    }

    pub fn progress_ratio(&self) -> f64 {
        if self.steps_total == 0 {
            0.0
        } else {
            self.steps_completed as f64 / self.steps_total as f64
        }
    }

    pub fn is_fully_completed(&self) -> bool {
        self.completed && !self.compensated
    }
}

/// Erasure coding fragment placement.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FragmentPlacement {
    pub fragment_id: FragmentId,
    pub node_id: NodeId,
    pub fragment_index: u32,
    pub total_fragments: u32,
    pub stored: bool,
}

impl FragmentPlacement {
    pub fn new(
        fragment_id: FragmentId,
        node_id: NodeId,
        fragment_index: u32,
        total_fragments: u32,
        stored: bool,
    ) -> Self {
        Self { fragment_id, node_id, fragment_index, total_fragments, stored }
    }

    pub fn is_stored(&self) -> bool {
        self.stored
    }
}

/// Integrity verification result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IntegrityResult {
    pub node_id: NodeId,
    pub checksum: Checksum,
    pub verified: bool,
    pub anomaly: Option<AnomalyType>,
    pub ids_time: IdsTime,
}

impl IntegrityResult {
    pub fn new(
        node_id: NodeId,
        checksum: Checksum,
        verified: bool,
        anomaly: Option<AnomalyType>,
        ids_time: IdsTime,
    ) -> Self {
        Self { node_id, checksum, verified, anomaly, ids_time }
    }

    pub fn is_healthy(&self) -> bool {
        self.verified && self.anomaly.is_none()
    }
}

/// Coordinator election result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoordinatorResult {
    pub domain: OrderingDomain,
    pub leader: NodeId,
    pub epoch: Epoch,
    pub fencing_epoch: Epoch,
    pub elected_at: IdsTime,
}

impl CoordinatorResult {
    pub fn new(
        domain: OrderingDomain,
        leader: NodeId,
        epoch: Epoch,
        fencing_epoch: Epoch,
        elected_at: IdsTime,
    ) -> Self {
        Self { domain, leader, epoch, fencing_epoch, elected_at }
    }
}

/// Chaosphere experiment result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChaosResult {
    pub experiment_id: String,
    pub fault_type: ChaosFaultType,
    pub affected_nodes: Vec<NodeId>,
    pub duration_ms: u64,
    pub recovered: bool,
}

impl ChaosResult {
    pub fn new(
        experiment_id: String,
        fault_type: ChaosFaultType,
        affected_nodes: Vec<NodeId>,
        duration_ms: u64,
        recovered: bool,
    ) -> Self {
        Self { experiment_id, fault_type, affected_nodes, duration_ms, recovered }
    }

    pub fn affected_count(&self) -> usize {
        self.affected_nodes.len()
    }
}

/// Chaos fault type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ChaosFaultType {
    PacketLoss,
    Reordering,
    Duplication,
    Corruption,
    Partition,
    Churn,
    ResourceExhaustion,
    ByzantineBehavior,
}

impl ChaosFaultType {
    pub fn as_str(&self) -> &'static str {
        match self {
            ChaosFaultType::PacketLoss => "packet_loss",
            ChaosFaultType::Reordering => "reordering",
            ChaosFaultType::Duplication => "duplication",
            ChaosFaultType::Corruption => "corruption",
            ChaosFaultType::Partition => "partition",
            ChaosFaultType::Churn => "churn",
            ChaosFaultType::ResourceExhaustion => "resource_exhaustion",
            ChaosFaultType::ByzantineBehavior => "byzantine_behavior",
        }
    }
}

/// Observability metrics snapshot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MetricsSnapshot {
    pub node_id: NodeId,
    pub ops_received: u64,
    pub ops_committed: u64,
    pub ops_failed: u64,
    pub bytes_in: u64,
    pub bytes_out: u64,
    pub ids_time: IdsTime,
}

impl MetricsSnapshot {
    pub fn new(
        node_id: NodeId,
        ops_received: u64,
        ops_committed: u64,
        ops_failed: u64,
        bytes_in: u64,
        bytes_out: u64,
        ids_time: IdsTime,
    ) -> Self {
        Self { node_id, ops_received, ops_committed, ops_failed, bytes_in, bytes_out, ids_time }
    }

    pub fn success_rate(&self) -> f64 {
        let total = self.ops_received;
        if total == 0 {
            return 1.0;
        }
        self.ops_committed as f64 / total as f64
    }

    pub fn byte_accounting_balance(&self) -> i64 {
        self.bytes_out as i64 - self.bytes_in as i64
    }
}

/// Data lifecycle transition request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LifecycleTransition {
    pub dataset_id: DatasetId,
    pub from_state: LifecycleState,
    pub to_state: LifecycleState,
    pub ids_time: IdsTime,
    pub approved: bool,
}

impl LifecycleTransition {
    pub fn new(
        dataset_id: DatasetId,
        from_state: LifecycleState,
        to_state: LifecycleState,
        ids_time: IdsTime,
        approved: bool,
    ) -> Self {
        Self { dataset_id, from_state, to_state, ids_time, approved }
    }

    pub fn is_allowed(&self) -> bool {
        self.approved
    }
}

/// Federation trust domain.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederationDomain {
    pub region_id: RegionId,
    pub owner_id: NodeId,
    pub trust_level: TrustLevel,
    pub credentials_valid_until: IdsTime,
}

impl FederationDomain {
    pub fn new(
        region_id: RegionId,
        owner_id: NodeId,
        trust_level: TrustLevel,
        credentials_valid_until: IdsTime,
    ) -> Self {
        Self { region_id, owner_id, trust_level, credentials_valid_until }
    }

    pub fn is_trusted(&self, current_time: IdsTime) -> bool {
        current_time <= self.credentials_valid_until
    }
}

/// Trust level for federation domains.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum TrustLevel {
    None,
    Low,
    Medium,
    High,
    Full,
}

impl TrustLevel {
    pub fn as_str(&self) -> &'static str {
        match self {
            TrustLevel::None => "none",
            TrustLevel::Low => "low",
            TrustLevel::Medium => "medium",
            TrustLevel::High => "high",
            TrustLevel::Full => "full",
        }
    }

    pub fn rank(&self) -> u32 {
        match self {
            TrustLevel::None => 0,
            TrustLevel::Low => 1,
            TrustLevel::Medium => 2,
            TrustLevel::High => 3,
            TrustLevel::Full => 4,
        }
    }

    pub fn is_at_least(&self, minimum: TrustLevel) -> bool {
        self.rank() >= minimum.rank()
    }
}

/// Hot/cold tier transition request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TierTransition {
    pub dataset_id: DatasetId,
    pub from_tier: Tier,
    pub to_tier: Tier,
    pub identity_preserved: bool,
    pub ids_time: IdsTime,
}

impl TierTransition {
    pub fn new(
        dataset_id: DatasetId,
        from_tier: Tier,
        to_tier: Tier,
        identity_preserved: bool,
        ids_time: IdsTime,
    ) -> Self {
        Self { dataset_id, from_tier, to_tier, identity_preserved, ids_time }
    }

    pub fn is_demotion(&self) -> bool {
        self.from_tier.is_hotter_than(self.to_tier)
    }

    pub fn is_promotion(&self) -> bool {
        self.to_tier.is_hotter_than(self.from_tier)
    }
}

/// Mobility handoff request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MobilityHandoff {
    pub dataset_id: DatasetId,
    pub from_region: RegionId,
    pub to_region: RegionId,
    pub identity_preserved: bool,
    pub controlled: bool,
    pub ids_time: IdsTime,
}

impl MobilityHandoff {
    pub fn new(
        dataset_id: DatasetId,
        from_region: RegionId,
        to_region: RegionId,
        identity_preserved: bool,
        controlled: bool,
        ids_time: IdsTime,
    ) -> Self {
        Self { dataset_id, from_region, to_region, identity_preserved, controlled, ids_time }
    }

    pub fn is_controlled(&self) -> bool {
        self.controlled
    }
}

/// Compute-to-data execution boundary request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComputeBoundaryRequest {
    pub compute_request: ComputeRequest,
    pub dataset_id: DatasetId,
    pub execution_allowed: bool,
    pub ids_time: IdsTime,
}

impl ComputeBoundaryRequest {
    pub fn new(
        compute_request: ComputeRequest,
        dataset_id: DatasetId,
        execution_allowed: bool,
        ids_time: IdsTime,
    ) -> Self {
        Self { compute_request, dataset_id, execution_allowed, ids_time }
    }
}

/// Cross-crate integration binding result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IntegrationResult {
    pub bound_subsystems: Vec<String>,
    pub type_check_passed: bool,
    pub ids_time: IdsTime,
}

impl IntegrationResult {
    pub fn new(
        bound_subsystems: Vec<String>,
        type_check_passed: bool,
        ids_time: IdsTime,
    ) -> Self {
        Self { bound_subsystems, type_check_passed, ids_time }
    }

    pub fn subsystem_count(&self) -> usize {
        self.bound_subsystems.len()
    }

    pub fn is_fully_integrated(&self) -> bool {
        self.type_check_passed && !self.bound_subsystems.is_empty()
    }
}

///////////////////////////////////////////////////////////////////////////////
// TESTS
///////////////////////////////////////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn node_id_creation_and_zero() {
        let id = NodeId::new([42u8; 32]);
        assert!(!id.is_zero());
        assert_eq!(id.as_bytes(), &[42u8; 32]);
        let zero = NodeId::ZERO;
        assert!(zero.is_zero());
    }

    #[test]
    fn node_id_hex_roundtrip() {
        let bytes = [1u8; 32];
        let id = NodeId::new(bytes);
        let hex = id.to_hex();
        assert_eq!(hex.len(), 64);
        let restored = NodeId::from_hex(&hex).unwrap();
        assert_eq!(id, restored);
    }

    #[test]
    fn node_id_from_hex_invalid_length() {
        let result = NodeId::from_hex("deadbeef");
        assert!(result.is_err());
    }

    #[test]
    fn region_id_creation_and_zero() {
        let id = RegionId::new([7u8; 32]);
        assert!(!id.is_zero());
        assert!(RegionId::ZERO.is_zero());
    }

    #[test]
    fn hunit_id_creation_and_zero() {
        let id = HUnitId::new([9u8; 32]);
        assert!(!id.is_zero());
        assert!(HUnitId::ZERO.is_zero());
    }

    #[test]
    fn dataset_id_creation_and_zero() {
        let id = DatasetId::new([3u8; 32]);
        assert!(!id.is_zero());
        assert!(DatasetId::ZERO.is_zero());
    }

    #[test]
    fn partition_id_creation_and_zero() {
        let id = PartitionId::new([5u8; 32]);
        assert!(!id.is_zero());
        assert!(PartitionId::ZERO.is_zero());
    }

    #[test]
    fn record_id_creation_and_zero() {
        let id = RecordId::new([11u8; 32]);
        assert!(!id.is_zero());
        assert!(RecordId::ZERO.is_zero());
    }

    #[test]
    fn op_ref_creation_and_zero() {
        let id = OpRef::new([0x41u8; 9]);
        assert!(!id.is_zero());
        assert!(OpRef::ZERO.is_zero());
        assert_eq!(id.to_string_lossy(), "AAAAAAAAA");
    }

    #[test]
    fn op_ref_from_str_valid() {
        let op = OpRef::from_str("ABCDEFGHI").unwrap();
        assert!(!op.is_zero());
    }

    #[test]
    fn op_ref_from_str_invalid_length() {
        let result = OpRef::from_str("ABC");
        assert!(result.is_err());
        let result = OpRef::from_str("ABCDEFGHIJK");
        assert!(result.is_err());
    }

    #[test]
    fn ordering_pos_creation_and_ordering() {
        let a = OrderingPos::new(10);
        let b = OrderingPos::new(20);
        assert!(a < b);
        assert_eq!(a.next().value(), 11);
        assert_eq!(a.prev().value(), 9);
        assert!(OrderingPos::ZERO.is_zero());
    }

    #[test]
    fn carrier_type_serialization() {
        assert_eq!(CarrierType::Loopback.as_u16(), 0);
        assert_eq!(CarrierType::Serial.as_u16(), 1);
        assert_eq!(CarrierType::Optical.as_u16(), 2);
        assert_eq!(CarrierType::Radio.as_u16(), 3);
        assert_eq!(CarrierType::Custom(99).as_u16(), 99);
        assert_eq!(CarrierType::from_u16(0), CarrierType::Loopback);
        assert_eq!(CarrierType::from_u16(99), CarrierType::Custom(99));
        assert!(CarrierType::Loopback.is_native());
        assert!(CarrierType::Custom(9999).is_native() == false);
    }

    #[test]
    fn endpoint_creation_and_validation() {
        let ep = Endpoint::new(CarrierType::Loopback, vec![1, 2, 3]).unwrap();
        assert_eq!(ep.address_len(), 3);
        assert!(ep.is_loopback());
    }

    #[test]
    fn endpoint_address_too_large() {
        let large: Vec<u8> = vec![0u8; Endpoint::MAX_ADDRESS_SIZE + 1];
        let result = Endpoint::new(CarrierType::Loopback, large);
        assert!(result.is_err());
    }

    #[test]
    fn endpoint_at_max_size() {
        let max: Vec<u8> = vec![0u8; Endpoint::MAX_ADDRESS_SIZE];
        let ep = Endpoint::new(CarrierType::Loopback, max).unwrap();
        assert_eq!(ep.address_len(), Endpoint::MAX_ADDRESS_SIZE);
    }

    #[test]
    fn rotating_node_id_creation() {
        let rni = RotatingNodeId::new([0xFFu8; 16]);
        assert!(!rni.is_zero());
        assert!(RotatingNodeId::ZERO.is_zero());
    }

    #[test]
    fn op_state_transitions() {
        assert!(OpState::Received.can_transition_to(OpState::Ordered));
        assert!(!OpState::Received.can_transition_to(OpState::Committed));
        assert!(!OpState::Verified.can_transition_to(OpState::Failed));
        assert!(!OpState::Failed.can_transition_to(OpState::Received));
        assert!(OpState::Verified.is_terminal());
        assert!(OpState::Failed.is_terminal());
        assert!(!OpState::Received.is_terminal());
    }

    #[test]
    fn op_state_is_committed() {
        assert!(!OpState::Received.is_committed());
        assert!(!OpState::Ordered.is_committed());
        assert!(!OpState::Executing.is_committed());
        assert!(OpState::Committed.is_committed());
        assert!(OpState::Synchronized.is_committed());
        assert!(OpState::Verified.is_committed());
        assert!(!OpState::Failed.is_committed());
    }

    #[test]
    fn commit_state_terminal() {
        assert!(!CommitState::Accepted.is_terminal());
        assert!(!CommitState::Prepared.is_terminal());
        assert!(CommitState::Committed.is_terminal());
        assert!(CommitState::Aborted.is_terminal());
        assert!(CommitState::Committed.is_success());
        assert!(!CommitState::Aborted.is_success());
    }

    #[test]
    fn redundancy_mode_failure_tolerance() {
        let full = RedundancyMode::FullReplica;
        let ec = RedundancyMode::ErasureCoded { k: 4, n: 10 };
        assert_eq!(ec.failure_tolerance(), Some(6));
        assert!(full.is_full_replica());
        assert!(ec.is_erasure_coded());
        assert_eq!(ec.erasure_params(), Some((4, 10)));
    }

    #[test]
    fn consistency_profile_requires_quorum() {
        assert!(ConsistencyProfile::Strong.requires_quorum());
        assert!(ConsistencyProfile::BoundedStaleness.requires_quorum());
        assert!(!ConsistencyProfile::Eventual.requires_quorum());
        assert!(!ConsistencyProfile::Strong.allows_fallback());
        assert!(ConsistencyProfile::Eventual.allows_fallback());
    }

    #[test]
    fn placement_decision_is_valid() {
        let fd = FailureDomain::new();
        let pd = PlacementDecision::new(
            RegionId::new([1u8; 32]),
            HUnitId::new([2u8; 32]),
            NodeId::new([3u8; 32]),
            RedundancyMode::FullReplica,
            fd,
        );
        assert!(pd.is_valid());
    }

    #[test]
    fn placement_decision_invalid_zero_node() {
        let fd = FailureDomain::new();
        let pd = PlacementDecision::new(
            RegionId::new([1u8; 32]),
            HUnitId::new([2u8; 32]),
            NodeId::ZERO,
            RedundancyMode::FullReplica,
            fd,
        );
        assert!(!pd.is_valid());
    }

    #[test]
    fn failure_domain_shares_domain() {
        let mut a = FailureDomain::new();
        a = a.with_power([1u8; 16]);
        let mut b = FailureDomain::new();
        b = b.with_power([1u8; 16]);
        assert!(a.shares_domain(&b));
        let mut c = FailureDomain::new();
        c = c.with_power([2u8; 16]);
        assert!(!a.shares_domain(&c));
    }

    #[test]
    fn failure_domain_empty_and_count() {
        let empty = FailureDomain::new();
        assert!(empty.is_empty());
        assert_eq!(empty.domain_count(), 0);
        let mut fd = FailureDomain::new();
        fd = fd.with_power([1u8; 16]).with_facility([2u8; 16]);
        assert_eq!(fd.domain_count(), 2);
        assert!(!fd.is_empty());
    }

    #[test]
    fn node_state_terminal_and_active() {
        assert!(NodeState::Active.is_active());
        assert!(NodeState::Failed.is_terminal());
        assert!(NodeState::Retired.is_terminal());
        assert!(!NodeState::Candidate.is_terminal());
        assert!(NodeState::Active.can_serve_requests());
        assert!(NodeState::Degraded.can_serve_requests());
        assert!(!NodeState::Failed.can_serve_requests());
    }

    #[test]
    fn hunit_state_terminal() {
        assert!(HUnitState::Operational.is_operational());
        assert!(HUnitState::Retired.is_terminal());
    }

    #[test]
    fn region_state_active_and_isolated() {
        assert!(RegionState::Active.is_active());
        assert!(RegionState::Isolated.is_isolated());
    }

    #[test]
    fn durability_condition_levels() {
        assert_eq!(DurabilityCondition::Received.level(), 1);
        assert_eq!(DurabilityCondition::CrossRegionCompleted.level(), 6);
        assert!(DurabilityCondition::ReplicasAcked.meets(DurabilityCondition::Executed));
        assert!(!DurabilityCondition::Executed.meets(DurabilityCondition::ReplicasAcked));
    }

    #[test]
    fn ids_time_creation_and_comparison() {
        let a = IdsTime::new(100);
        let b = IdsTime::new(200);
        assert!(a < b);
        assert_eq!(a.elapsed_since(b), 0);
        assert_eq!(b.elapsed_since(a), 100);
        assert!(IdsTime::ZERO.is_zero());
    }

    #[test]
    fn edge_case_bits_has_and_enable() {
        let mut bits = EdgeCaseBits::ZERO;
        assert!(!bits.has(EdgeCaseBits::BINARY_COMPLETION));
        bits.enable(EdgeCaseBits::BINARY_COMPLETION);
        assert!(bits.has(EdgeCaseBits::BINARY_COMPLETION));
        bits.disable(EdgeCaseBits::BINARY_COMPLETION);
        assert!(!bits.has(EdgeCaseBits::BINARY_COMPLETION));
        bits.toggle(EdgeCaseBits::DEAD_NODE_IDENTITY);
        assert!(bits.has(EdgeCaseBits::DEAD_NODE_IDENTITY));
        bits.toggle(EdgeCaseBits::DEAD_NODE_IDENTITY);
        assert!(!bits.has(EdgeCaseBits::DEAD_NODE_IDENTITY));
    }

    #[test]
    fn edge_case_bits_count_and_all() {
        let bits = EdgeCaseBits::new(0b1011);
        assert_eq!(bits.count_enabled(), 3);
        let all = EdgeCaseBits::ALL;
        assert!(!all.is_empty());
        assert!(EdgeCaseBits::ZERO.is_empty());
    }

    #[test]
    fn edge_case_bits_enabled_bits() {
        let bits = EdgeCaseBits::new(0b101);
        let enabled = bits.enabled_bits();
        assert_eq!(enabled.len(), 2);
        assert!(enabled.contains(&1));
        assert!(enabled.contains(&4));
    }

    #[test]
    fn hub_config_default_and_validate() {
        let config = HubConfig::default_for_testing();
        assert!(config.validate().is_ok());
        assert!(config.edge_case_enabled(EdgeCaseBits::BINARY_COMPLETION));
    }

    #[test]
    fn hub_config_validate_invalid_address_size() {
        let mut config = HubConfig::default_for_testing();
        config.max_endpoint_address_size = 0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn carrier_payload_len_and_empty() {
        let cp = CarrierPayload::new(
            CarrierType::Serial,
            vec![1, 2, 3],
            IdsTime::new(5),
        );
        assert_eq!(cp.len(), 3);
        assert!(!cp.is_empty());
        assert_eq!(cp.carrier_name(), "serial");
    }

    #[test]
    fn type_id_constants_and_name() {
        assert_eq!(TypeId::NODE_ID.value(), 1);
        assert_eq!(TypeId::REGION_ID.value(), 2);
        assert_eq!(TypeId::ZERO.name(), "unknown");
        assert_eq!(TypeId::NODE_ID.name(), "NodeId");
        assert!(TypeId::ZERO.is_zero());
    }

    #[test]
    fn op_ref_allocation_is_valid() {
        let alloc = OpRefAllocation::new(OpRef::new([1u8; 9]), true, 0);
        assert!(alloc.is_valid());
        let alloc2 = OpRefAllocation::new(OpRef::ZERO, true, 0);
        assert!(!alloc2.is_valid());
        let alloc3 = OpRefAllocation::new(OpRef::new([1u8; 9]), false, 5);
        assert!(!alloc3.is_valid());
    }

    #[test]
    fn ordering_pos_snapshot_comparison() {
        let domain = OrderingDomain::new([1u8; 32]);
        let a = OrderingPosSnapshot::new(domain, OrderingPos::new(10), IdsTime::new(100));
        let b = OrderingPosSnapshot::new(domain, OrderingPos::new(20), IdsTime::new(200));
        assert!(a.is_before(&b));
        assert!(b.is_after(&a));
    }

    #[test]
    fn durability_track_state_terminal() {
        assert!(DurabilityTrackState::CrossRegionCompleted.is_terminal());
        assert!(DurabilityTrackState::Failed.is_terminal());
        assert!(!DurabilityTrackState::InProgress.is_terminal());
        assert!(DurabilityTrackState::CrossRegionCompleted.is_success());
        assert!(!DurabilityTrackState::Failed.is_success());
    }

    #[test]
    fn recovery_step_completion() {
        let mut step = RecoveryStep::new(1, RecoveryScopeLevel::Node, RecoveryAction::ReconstructErasure);
        assert!(!step.is_completed());
        step.mark_completed();
        assert!(step.is_completed());
    }

    #[test]
    fn consistency_selection_creation() {
        let sel = ConsistencySelection::new(
            DatasetId::new([1u8; 32]),
            ConsistencyProfile::Strong,
            ConsistencySelectionReason::ClientOverride,
            IdsTime::new(5),
        );
        assert_eq!(sel.selected, ConsistencyProfile::Strong);
    }

    #[test]
    fn broadcast_topology_config_creation() {
        let cfg = BroadcastTopologyConfig::new(
            BroadcastProtocol::Causal,
            3,
            7,
            true,
        );
        assert_eq!(cfg.fanout_factor, 3);
        assert!(cfg.require_acks);
    }

    #[test]
    fn quorum_result_is_valid() {
        let result = QuorumResult::new(
            vec![NodeId::new([1u8; 32])],
            vec![NodeId::new([1u8; 32])],
            vec![NodeId::new([1u8; 32])],
            true,
        );
        assert!(result.is_valid());
        assert_eq!(result.intersection_size(), 1);
        let invalid = QuorumResult::new(vec![], vec![], vec![], false);
        assert!(!invalid.is_valid());
    }

    #[test]
    fn snapshot_result_participant_count() {
        let result = SnapshotResult::new(
            SnapshotId::new([1u8; 16]),
            NodeId::new([2u8; 32]),
            vec![NodeId::new([3u8; 32]), NodeId::new([4u8; 32])],
            true,
            IdsTime::new(10),
        );
        assert_eq!(result.participant_count(), 2);
        assert!(result.consistent);
    }

    #[test]
    fn byzantine_result_fault_count() {
        let result = ByzantineResult::new(
            Value::new(vec![1, 2, 3]),
            vec![NodeId::new([1u8; 32])],
            vec![NodeId::new([2u8; 32]), NodeId::new([3u8; 32])],
            true,
            IdsTime::new(5),
        );
        assert_eq!(result.fault_count(), 2);
        assert!(result.consensus_reached);
    }

    #[test]
    fn lease_result_expiry() {
        let result = LeaseResult::new(
            LeaseId::new([1u8; 16]),
            NodeId::new([2u8; 32]),
            Epoch::new(1),
            FencingToken::new(100),
            IdsTime::new(50),
            true,
        );
        assert!(!result.is_expired(IdsTime::new(49)));
        assert!(result.is_expired(IdsTime::new(50)));
        assert!(result.is_expired(IdsTime::new(51)));
    }

    #[test]
    fn saga_result_progress() {
        let result = SagaResult::new(
            SagaId::new([1u8; 16]),
            None,
            3,
            10,
            false,
            false,
        );
        assert!((result.progress_ratio() - 0.3).abs() < f64::EPSILON);
        assert!(!result.is_fully_completed());
        let completed = SagaResult::new(
            SagaId::new([1u8; 16]),
            None,
            10,
            10,
            false,
            true,
        );
        assert!(completed.is_fully_completed());
    }

    #[test]
    fn fragment_placement_stored() {
        let placement = FragmentPlacement::new(
            FragmentId::new([1u8; 16]),
            NodeId::new([2u8; 32]),
            0,
            4,
            true,
        );
        assert!(placement.is_stored());
        let not_stored = FragmentPlacement::new(
            FragmentId::new([1u8; 16]),
            NodeId::new([2u8; 32]),
            0,
            4,
            false,
        );
        assert!(!not_stored.is_stored());
    }

    #[test]
    fn integrity_result_is_healthy() {
        let healthy = IntegrityResult::new(
            NodeId::new([1u8; 32]),
            Checksum::new([0u8; 32]),
            true,
            None,
            IdsTime::new(5),
        );
        assert!(healthy.is_healthy());
        let unhealthy = IntegrityResult::new(
            NodeId::new([1u8; 32]),
            Checksum::new([0u8; 32]),
            false,
            Some(AnomalyType::ChecksumFailure),
            IdsTime::new(5),
        );
        assert!(!unhealthy.is_healthy());
    }

    #[test]
    fn coordinator_result_creation() {
        let result = CoordinatorResult::new(
            OrderingDomain::new([1u8; 32]),
            NodeId::new([2u8; 32]),
            Epoch::new(1),
            Epoch::new(100),
            IdsTime::new(10),
        );
        assert_eq!(result.leader, NodeId::new([2u8; 32]));
    }

    #[test]
    fn chaos_result_affected_count() {
        let result = ChaosResult::new(
            "exp-1".to_string(),
            ChaosFaultType::PacketLoss,
            vec![NodeId::new([1u8; 32]), NodeId::new([2u8; 32])],
            5000,
            true,
        );
        assert_eq!(result.affected_count(), 2);
        assert!(result.recovered);
    }

    #[test]
    fn metrics_snapshot_success_rate() {
        let metrics = MetricsSnapshot::new(
            NodeId::new([1u8; 32]),
            100,
            90,
            10,
            1000,
            1000,
            IdsTime::new(5),
        );
        assert!((metrics.success_rate() - 0.9).abs() < f64::EPSILON);
        assert_eq!(metrics.byte_accounting_balance(), 0);
    }

    #[test]
    fn lifecycle_transition_is_allowed() {
        let t = LifecycleTransition::new(
            DatasetId::new([1u8; 32]),
            LifecycleState::Creating,
            LifecycleState::Active,
            IdsTime::new(1),
            true,
        );
        assert!(t.is_allowed());
        let not_allowed = LifecycleTransition::new(
            DatasetId::new([1u8; 32]),
            LifecycleState::Creating,
            LifecycleState::Active,
            IdsTime::new(1),
            false,
        );
        assert!(!not_allowed.is_allowed());
    }

    #[test]
    fn federation_domain_is_trusted() {
        let domain = FederationDomain::new(
            RegionId::new([1u8; 32]),
            NodeId::new([2u8; 32]),
            TrustLevel::High,
            IdsTime::new(100),
        );
        assert!(domain.is_trusted(IdsTime::new(99)));
        assert!(domain.is_trusted(IdsTime::new(100)));
        assert!(!domain.is_trusted(IdsTime::new(101)));
    }

    #[test]
    fn trust_level_ranking() {
        assert!(TrustLevel::Full.is_at_least(TrustLevel::High));
        assert!(!TrustLevel::Low.is_at_least(TrustLevel::Medium));
        assert_eq!(TrustLevel::None.rank(), 0);
        assert_eq!(TrustLevel::Full.rank(), 4);
    }

    #[test]
    fn tier_transition_demotion_and_promotion() {
        let demotion = TierTransition::new(
            DatasetId::new([1u8; 32]),
            Tier::Hot,
            Tier::Cold,
            true,
            IdsTime::new(1),
        );
        assert!(demotion.is_demotion());
        assert!(!demotion.is_promotion());
        let promotion = TierTransition::new(
            DatasetId::new([1u8; 32]),
            Tier::Cold,
            Tier::Hot,
            true,
            IdsTime::new(1),
        );
        assert!(promotion.is_promotion());
        assert!(!promotion.is_demotion());
    }

    #[test]
    fn mobility_handoff_is_controlled() {
        let handoff = MobilityHandoff::new(
            DatasetId::new([1u8; 32]),
            RegionId::new([2u8; 32]),
            RegionId::new([3u8; 32]),
            true,
            true,
            IdsTime::new(1),
        );
        assert!(handoff.is_controlled());
        assert!(handoff.identity_preserved);
    }

    #[test]
    fn compute_boundary_request_creation() {
        let req = ComputeBoundaryRequest::new(
            ComputeRequest::new(DatasetId::new([1u8; 32]), [0u8; 32], vec![]),
            DatasetId::new([1u8; 32]),
            true,
            IdsTime::new(1),
        );
        assert!(req.execution_allowed);
    }

    #[test]
    fn integration_result_fully_integrated() {
        let result = IntegrationResult::new(
            vec!["ids-carrier".to_string(), "ids-crypto".to_string()],
            true,
            IdsTime::new(1),
        );
        assert_eq!(result.subsystem_count(), 2);
        assert!(result.is_fully_integrated());
        let not_integrated = IntegrationResult::new(vec![], false, IdsTime::new(1));
        assert!(!not_integrated.is_fully_integrated());
    }

    // === ProtocolVersion tests ===
    #[test]
    fn protocol_version_creation_and_default() {
        let v = ProtocolVersion::new(1, 2, 3);
        assert_eq!(v.major, 1);
        assert_eq!(v.minor, 2);
        assert_eq!(v.patch, 3);
        let default = ProtocolVersion::default();
        assert_eq!(default, ProtocolVersion::V0_1_0);
    }

    #[test]
    fn protocol_version_as_u64_and_compatible() {
        let v1 = ProtocolVersion::new(2, 5, 0);
        let v2 = ProtocolVersion::new(2, 0, 9);
        let v3 = ProtocolVersion::new(3, 0, 0);
        assert!(v1.is_compatible_with(v2));
        assert!(!v1.is_compatible_with(v3));
        assert_eq!(v1.as_u64(), (2u64 << 32) | (5u64 << 16));
    }

    #[test]
    fn protocol_version_display() {
        let v = ProtocolVersion::new(0, 1, 0);
        assert_eq!(v.to_string_compact(), "0.1.0");
        let v2 = ProtocolVersion::new(10, 20, 30);
        assert_eq!(v2.to_string_compact(), "10.20.30");
    }

    // === EdgeCaseBits tests ===
    #[test]
    fn edge_case_bits_all_constants_defined() {
        assert_eq!(EdgeCaseBits::BINARY_COMPLETION, 1 << 0);
        assert_eq!(EdgeCaseBits::DEAD_NODE_IDENTITY, 1 << 1);
        assert_eq!(EdgeCaseBits::IDLE_ROTATION, 1 << 2);
        assert_eq!(EdgeCaseBits::NO_GLOBAL_MASTER, 1 << 3);
        assert_eq!(EdgeCaseBits::OUT_EQ_IN, 1 << 4);
        assert_eq!(EdgeCaseBits::COORDINATED_DELETION, 1 << 5);
        assert_eq!(EdgeCaseBits::FENCING_STONITH, 1 << 6);
        assert_eq!(EdgeCaseBits::QUORUM_INTERSECTION, 1 << 7);
        assert_eq!(EdgeCaseBits::ERASURE_THRESHOLD, 1 << 8);
        assert_eq!(EdgeCaseBits::CORRELATED_FAILURE, 1 << 9);
        assert_eq!(EdgeCaseBits::HUNIT_COMPENSATION, 1 << 10);
        assert_eq!(EdgeCaseBits::REGIONAL_AUTONOMY, 1 << 11);
        assert_eq!(EdgeCaseBits::HYBRID_REDUNDANCY, 1 << 12);
        assert_eq!(EdgeCaseBits::OPERATION_LIFECYCLE, 1 << 13);
        assert_eq!(EdgeCaseBits::DURABILITY_ACK, 1 << 14);
        assert_eq!(EdgeCaseBits::ORDERED_READ_VISIBILITY, 1 << 15);
        assert_eq!(EdgeCaseBits::NODE_ROSTER, 1 << 16);
        assert_eq!(EdgeCaseBits::REGION_AFFINITY, 1 << 17);
        assert_eq!(EdgeCaseBits::IDENTITY_ROTATION, 1 << 18);
        assert_eq!(EdgeCaseBits::IDS_STANDARD_TIME, 1 << 19);
        assert_eq!(EdgeCaseBits::DAILY_INTEGRITY, 1 << 20);
        assert_eq!(EdgeCaseBits::DB_RETIREMENT, 1 << 21);
        assert_eq!(EdgeCaseBits::READ_FALLBACK, 1 << 22);
        assert_eq!(EdgeCaseBits::TOPOLOGY_DIVERSITY, 1 << 23);
        assert_eq!(EdgeCaseBits::WRITE_FANOUT, 1 << 24);
        assert_eq!(EdgeCaseBits::CUSTODY_GRAPH, 1 << 25);
        assert_eq!(EdgeCaseBits::CHECKSUM_BOUNDARY, 1 << 26);
        assert_eq!(EdgeCaseBits::REPLICA_FRESHNESS, 1 << 27);
        assert_eq!(EdgeCaseBits::COMMIT_PREFIX, 1 << 28);
        assert_eq!(EdgeCaseBits::VERSION_VECTOR, 1 << 29);
        assert_eq!(EdgeCaseBits::MEMBERSHIP_EPOCH, 1 << 30);
        assert_eq!(EdgeCaseBits::PLACEMENT_POLICY, 1 << 31);
    }

    #[test]
    fn edge_case_bits_has_all_and_has_any() {
        let bits = EdgeCaseBits::new(EdgeCaseBits::BINARY_COMPLETION | EdgeCaseBits::DEAD_NODE_IDENTITY);
        assert!(bits.has_all(EdgeCaseBits::BINARY_COMPLETION));
        assert!(bits.has_all(EdgeCaseBits::DEAD_NODE_IDENTITY));
        assert!(!bits.has_all(EdgeCaseBits::BINARY_COMPLETION | EdgeCaseBits::IDLE_ROTATION));
        assert!(bits.has_any(EdgeCaseBits::BINARY_COMPLETION));
        assert!(bits.has_any(EdgeCaseBits::DEAD_NODE_IDENTITY));
        assert!(!bits.has_any(EdgeCaseBits::IDLE_ROTATION));
    }

    #[test]
    fn edge_case_bits_display() {
        let bits = EdgeCaseBits::new(0xDEADBEEF);
        let s = format!("{}", bits);
        assert!(s.contains("deadbeef"));
    }

    // === HubConfig tests ===
    #[test]
    fn hub_config_default_for_testing() {
        let config = HubConfig::default_for_testing();
        assert!(config.validate().is_ok());
        assert_eq!(config.max_endpoint_address_size, Endpoint::MAX_ADDRESS_SIZE);
        assert_eq!(config.default_consistency, ConsistencyProfile::Strong);
        assert_eq!(config.default_redundancy, RedundancyMode::FullReplica);
        assert_eq!(config.default_durability, DurabilityCondition::ReplicasAcked);
        assert!(config.enable_binary_completion);
        assert!(config.enable_dead_node_identity);
        assert!(config.enable_idle_rotation);
        assert!(config.enable_coordinated_deletion);
        assert!(config.enable_fencing_stonith);
        assert!(config.enable_daily_integrity);
        assert!(config.enable_read_fallback);
        assert!(config.enable_topology_diversity);
    }

    #[test]
    fn hub_config_validate_unsupported_version() {
        let mut config = HubConfig::default_for_testing();
        config.protocol_version = ProtocolVersion::new(0, 0, 0);
        assert!(config.validate().is_err());
    }

    #[test]
    fn hub_config_edge_case_enabled() {
        let mut config = HubConfig::default_for_testing();
        assert!(config.edge_case_enabled(EdgeCaseBits::BINARY_COMPLETION));
        config.edge_case_bits.disable(EdgeCaseBits::BINARY_COMPLETION);
        assert!(!config.edge_case_enabled(EdgeCaseBits::BINARY_COMPLETION));
    }

    // === TypeId tests ===
    #[test]
    fn type_id_all_constants() {
        assert_eq!(TypeId::ZERO.value(), 0);
        assert_eq!(TypeId::NODE_ID.value(), 1);
        assert_eq!(TypeId::REGION_ID.value(), 2);
        assert_eq!(TypeId::HUNIT_ID.value(), 3);
        assert_eq!(TypeId::DATASET_ID.value(), 4);
        assert_eq!(TypeId::PARTITION_ID.value(), 5);
        assert_eq!(TypeId::RECORD_ID.value(), 6);
        assert_eq!(TypeId::OP_REF.value(), 7);
        assert_eq!(TypeId::ORDERING_POS.value(), 8);
        assert_eq!(TypeId::ENDPOINT.value(), 9);
        assert_eq!(TypeId::ROTATING_NODE_ID.value(), 10);
        assert_eq!(TypeId::NODE_ID.name(), "NodeId");
        assert_eq!(TypeId::REGION_ID.name(), "RegionId");
        assert_eq!(TypeId::ENDPOINT.name(), "Endpoint");
        assert_eq!(TypeId::ZERO.name(), "unknown");
    }

    // === DurabilityTrackState tests ===
    #[test]
    fn durability_track_state_all_variants() {
        assert!(DurabilityTrackState::NotStarted.as_str() == "not_started");
        assert!(DurabilityTrackState::InProgress.as_str() == "in_progress");
        assert!(DurabilityTrackState::ReplicasAcked.as_str() == "replicas_acked");
        assert!(DurabilityTrackState::CrossRegionCompleted.as_str() == "cross_region_completed");
        assert!(DurabilityTrackState::Failed.as_str() == "failed");
        assert!(!DurabilityTrackState::NotStarted.is_terminal());
        assert!(!DurabilityTrackState::InProgress.is_terminal());
        assert!(DurabilityTrackState::CrossRegionCompleted.is_terminal());
        assert!(DurabilityTrackState::Failed.is_terminal());
    }

    // === RecoveryAction tests ===
    #[test]
    fn recovery_action_all_variants() {
        assert_eq!(RecoveryAction::ReconstructErasure.as_str(), "reconstruct_erasure");
        assert_eq!(RecoveryAction::CopyReplica.as_str(), "copy_replica");
        assert_eq!(RecoveryAction::TransferOwnership.as_str(), "transfer_ownership");
        assert_eq!(RecoveryAction::RestoreRedundancy.as_str(), "restore_redundancy");
        assert_eq!(RecoveryAction::VerifyIntegrity.as_str(), "verify_integrity");
        assert_eq!(RecoveryAction::QuarantineNode.as_str(), "quarantine_node");
    }

    // === ConsistencySelectionReason tests ===
    #[test]
    fn consistency_selection_reason_all_variants() {
        assert_eq!(ConsistencySelectionReason::DatasetDefault.as_str(), "dataset_default");
        assert_eq!(ConsistencySelectionReason::ClientOverride.as_str(), "client_override");
        assert_eq!(ConsistencySelectionReason::RegionPolicy.as_str(), "region_policy");
        assert_eq!(ConsistencySelectionReason::FailureDomainConstraint.as_str(), "failure_domain_constraint");
        assert_eq!(ConsistencySelectionReason::AdminForced.as_str(), "admin_forced");
    }

    // === BroadcastProtocol tests ===
    #[test]
    fn broadcast_protocol_all_variants() {
        assert_eq!(BroadcastProtocol::Reliable.as_str(), "reliable");
        assert_eq!(BroadcastProtocol::Causal.as_str(), "causal");
        assert_eq!(BroadcastProtocol::TotalOrder.as_str(), "total_order");
        assert_eq!(BroadcastProtocol::Atomic.as_str(), "atomic");
    }

    // === ChaosFaultType tests ===
    #[test]
    fn chaos_fault_type_all_variants() {
        assert_eq!(ChaosFaultType::PacketLoss.as_str(), "packet_loss");
        assert_eq!(ChaosFaultType::Reordering.as_str(), "reordering");
        assert_eq!(ChaosFaultType::Duplication.as_str(), "duplication");
        assert_eq!(ChaosFaultType::Corruption.as_str(), "corruption");
        assert_eq!(ChaosFaultType::Partition.as_str(), "partition");
        assert_eq!(ChaosFaultType::Churn.as_str(), "churn");
        assert_eq!(ChaosFaultType::ResourceExhaustion.as_str(), "resource_exhaustion");
        assert_eq!(ChaosFaultType::ByzantineBehavior.as_str(), "byzantine_behavior");
    }

    // === AnomalyType tests ===
    #[test]
    fn anomaly_type_all_variants() {
        assert_eq!(format!("{:?}", AnomalyType::ByteMismatch), "ByteMismatch");
        assert_eq!(format!("{:?}", AnomalyType::ReplicaDrift), "ReplicaDrift");
        assert_eq!(format!("{:?}", AnomalyType::ChecksumFailure), "ChecksumFailure");
        assert_eq!(format!("{:?}", AnomalyType::StateCorruption), "StateCorruption");
    }

    // === FailureEventType tests ===
    #[test]
    fn failure_event_type_all_variants() {
        assert_eq!(format!("{:?}", FailureEventType::NodeFailure), "NodeFailure");
        assert_eq!(format!("{:?}", FailureEventType::HUnitFailure), "HUnitFailure");
        assert_eq!(format!("{:?}", FailureEventType::RegionalFailure), "RegionalFailure");
        assert_eq!(format!("{:?}", FailureEventType::NetworkPartition), "NetworkPartition");
        assert_eq!(format!("{:?}", FailureEventType::DiskFailure), "DiskFailure");
        assert_eq!(format!("{:?}", FailureEventType::PowerFailure), "PowerFailure");
    }

    // === RecoveryScopeLevel tests ===
    #[test]
    fn recovery_scope_level_ranking() {
        assert_eq!(RecoveryScopeLevel::Node.rank(), 1);
        assert_eq!(RecoveryScopeLevel::HUnit.rank(), 2);
        assert_eq!(RecoveryScopeLevel::Region.rank(), 3);
        assert!(RecoveryScopeLevel::Region > RecoveryScopeLevel::Node);
        assert!(RecoveryScopeLevel::HUnit > RecoveryScopeLevel::Node);
    }

    // === LifecycleState tests ===
    #[test]
    fn lifecycle_state_terminal() {
        assert!(!LifecycleState::Creating.is_terminal());
        assert!(!LifecycleState::Active.is_terminal());
        assert!(!LifecycleState::Archiving.is_terminal());
        assert!(LifecycleState::Archived.is_terminal());
        assert!(!LifecycleState::Deleting.is_terminal());
        assert!(LifecycleState::Deleted.is_terminal());
    }

    // === OpType tests ===
    #[test]
    fn op_type_all_variants() {
        assert_eq!(OpType::Read.as_str(), "read");
        assert_eq!(OpType::Write.as_str(), "write");
        assert_eq!(OpType::Delete.as_str(), "delete");
        assert_eq!(OpType::Replicate.as_str(), "replicate");
        assert_eq!(OpType::Recover.as_str(), "recover");
        assert_eq!(OpType::Admin.as_str(), "admin");
    }

    // === CircuitState tests ===
    #[test]
    fn circuit_state_all_variants() {
        assert_eq!(CircuitState::Closed.as_str(), "closed");
        assert_eq!(CircuitState::Open.as_str(), "open");
        assert_eq!(CircuitState::HalfOpen.as_str(), "half_open");
    }

    // === DegradationLevel tests ===
    #[test]
    fn degradation_level_ranking() {
        assert_eq!(DegradationLevel::None.rank(), 0);
        assert_eq!(DegradationLevel::Minor.rank(), 1);
        assert_eq!(DegradationLevel::Moderate.rank(), 2);
        assert_eq!(DegradationLevel::Severe.rank(), 3);
        assert_eq!(DegradationLevel::Critical.rank(), 4);
        assert!(DegradationLevel::Critical > DegradationLevel::None);
    }

    // === Tier tests ===
    #[test]
    fn tier_ranking() {
        assert!(Tier::Hot.is_hotter_than(Tier::Warm));
        assert!(Tier::Warm.is_hotter_than(Tier::Cold));
        assert!(!Tier::Cold.is_hotter_than(Tier::Hot));
        assert_eq!(Tier::Hot.rank(), 0);
        assert_eq!(Tier::Cold.rank(), 2);
    }

    // === Endpoint roundtrip tests ===
    #[test]
    fn endpoint_into_parts_and_with_address() {
        let ep = Endpoint::new(CarrierType::Radio, vec![9, 9, 9]).unwrap();
        let (ct, addr) = ep.into_parts();
        assert_eq!(ct, CarrierType::Radio);
        assert_eq!(addr, vec![9, 9, 9]);
        let ep2 = Endpoint::new(CarrierType::Optical, vec![1, 2]).unwrap();
        let ep3 = ep2.with_address(vec![7, 8, 9]).unwrap();
        assert_eq!(ep3.address_len(), 3);
    }

    // === Operation tests ===
    #[test]
    fn operation_payload_size_and_empty() {
        let op = Operation::new(
            OpRef::new([1u8; 9]),
            DatasetId::new([2u8; 32]),
            OpType::Write,
            vec![1, 2, 3, 4, 5],
        );
        assert_eq!(op.payload_size(), 5);
        assert!(!op.is_empty());
        let empty_op = Operation::new(
            OpRef::new([1u8; 9]),
            DatasetId::new([2u8; 32]),
            OpType::Read,
            vec![],
        );
        assert!(empty_op.is_empty());
        assert_eq!(empty_op.payload_size(), 0);
    }

    // === Version and Epoch tests ===
    #[test]
    fn version_and_epoch_next() {
        let v = Version::new(100);
        assert_eq!(v.next().value(), 101);
        assert_eq!(Version::ZERO.value(), 0);
        let e = Epoch::new(50);
        assert_eq!(e.next().value(), 51);
        assert_eq!(Epoch::ZERO.value(), 0);
    }

    // === OrderingDomain, SessionId, SnapshotId, CheckpointId, LeaseId, SagaId, FragmentId, KeyId, AuthToken, Signature, MessageId, TransactionId tests ===
    #[test]
    fn fixed_size_id_types_zero_and_new() {
        assert!(SessionId::ZERO.is_zero());
        assert!(!SessionId::new([1u8; 16]).is_zero());
        assert!(SnapshotId::ZERO.is_zero());
        assert!(!SnapshotId::new([1u8; 16]).is_zero());
        assert!(CheckpointId::ZERO.is_zero());
        assert!(!CheckpointId::new([1u8; 16]).is_zero());
        assert!(LeaseId::ZERO.is_zero());
        assert!(!LeaseId::new([1u8; 16]).is_zero());
        assert!(SagaId::ZERO.is_zero());
        assert!(!SagaId::new([1u8; 16]).is_zero());
        assert!(FragmentId::ZERO.is_zero());
        assert!(!FragmentId::new([1u8; 16]).is_zero());
        assert!(KeyId::ZERO.is_zero());
        assert!(!KeyId::new([1u8; 16]).is_zero());
        assert!(AuthToken::ZERO.is_zero());
        assert!(!AuthToken::new([1u8; 32]).is_zero());
        assert!(Signature::ZERO.is_zero());
        assert!(!Signature::new([1u8; 64]).is_zero());
        assert!(MessageId::ZERO.is_zero());
        assert!(!MessageId::new([1u8; 16]).is_zero());
        assert!(TransactionId::ZERO.is_zero());
        assert!(!TransactionId::new([1u8; 16]).is_zero());
    }

    // === ChannelId, StepId, FencingToken tests ===
    #[test]
    fn numeric_id_types_next() {
        let c = ChannelId::new(42);
        assert_eq!(c.value(), 42);
        assert_eq!(ChannelId::ZERO.value(), 0);
        let s = StepId::new(7);
        assert_eq!(s.value(), 7);
        let f = FencingToken::new(1000);
        assert_eq!(f.next().value(), 1001);
        assert_eq!(FencingToken::ZERO.value(), 0);
    }

    // === Capacity tests ===
    #[test]
    fn capacity_can_serve_and_total_bytes() {
        let cap = Capacity::new(1000, 500, 4, 1000000);
        assert!(!cap.is_zero());
        assert_eq!(cap.total_bytes(), 1500);
        let required = Capacity::new(500, 250, 2, 500000);
        assert!(cap.can_serve(&required));
        let too_much = Capacity::new(2000, 1000, 8, 2000000);
        assert!(!cap.can_serve(&too_much));
        assert!(Capacity::ZERO.is_zero());
    }

    // === Credential tests ===
    #[test]
    fn credential_creation_and_token_len() {
        let cred = Credential::new(NodeId::new([1u8; 32]), vec![0u8; 32]);
        assert_eq!(cred.token_len(), 32);
        assert_eq!(cred.identity, NodeId::new([1u8; 32]));
    }

    // === Value and Message tests ===
    #[test]
    fn value_and_message_len() {
        let val = Value::new(vec![1, 2, 3]);
        assert_eq!(val.len(), 3);
        assert!(!val.is_empty());
        let empty_val = Value::new(vec![]);
        assert!(empty_val.is_empty());
        let msg = Message::new(
            MessageId::new([1u8; 16]),
            NodeId::new([2u8; 32]),
            vec![9, 9, 9],
            IdsTime::new(10),
        );
        assert_eq!(msg.len(), 3);
    }

    // === OpResult tests ===
    #[test]
    fn op_result_is_success() {
        let result = OpResult::new(
            OpRef::new([1u8; 9]),
            CommitState::Committed,
            DurabilityCondition::ReplicasAcked,
            OrderingPos::new(42),
            IdsTime::new(100),
        );
        assert!(result.is_success());
        let failed = OpResult::new(
            OpRef::new([1u8; 9]),
            CommitState::Aborted,
            DurabilityCondition::Received,
            OrderingPos::new(0),
            IdsTime::ZERO,
        );
        assert!(!failed.is_success());
    }

    // === IdempotencyKey tests ===
    #[test]
    fn idempotency_key_zero_and_new() {
        assert!(IdempotencyKey::ZERO.is_zero());
        assert!(!IdempotencyKey::new([1u8; 32]).is_zero());
    }

    // === DeliverySemantics and AckState tests ===
    #[test]
    fn delivery_semantics_and_ack_state_all_variants() {
        assert_eq!(format!("{:?}", DeliverySemantics::AtMostOnce), "AtMostOnce");
        assert_eq!(format!("{:?}", DeliverySemantics::AtLeastOnce), "AtLeastOnce");
        assert_eq!(format!("{:?}", DeliverySemantics::ExactlyOnce), "ExactlyOnce");
        assert_eq!(format!("{:?}", AckState::Pending), "Pending");
        assert_eq!(format!("{:?}", AckState::Acked), "Acked");
        assert_eq!(format!("{:?}", AckState::Nacked), "Nacked");
        assert_eq!(format!("{:?}", AckState::Timeout), "Timeout");
    }

    // === CircuitBreakerState tests ===
    #[test]
    fn circuit_breaker_state_all_variants() {
        assert_eq!(format!("{:?}", CircuitBreakerState::Closed), "Closed");
        assert_eq!(format!("{:?}", CircuitBreakerState::Open), "Open");
        assert_eq!(format!("{:?}", CircuitBreakerState::HalfOpen), "HalfOpen");
    }

    // === Error type usage tests ===
    #[test]
    fn error_variants_are_referenced() {
        // Verify that error types used in types.rs exist and are referenceable
        let _e_node_not_found = crate::errors::Error::NodeNotFound { node: NodeId::ZERO };
        let _e_quorum = crate::errors::Error::QuorumNotSatisfied { need: 3, have: 1 };
        let _e_invariant = crate::errors::Error::InvariantViolated { invariant: "test" };
        let _e_byte_mismatch = crate::errors::Error::ByteAccountingMismatch { out: 100, in_: 50 };
    }
}