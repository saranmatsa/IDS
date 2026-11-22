//! # ids-000-aa Sealed Trait Contract: Protocol
//!
//! Protocol contract for IDS.
//! Provides native protocol versioning, capability negotiation, protocol
//! selection, handshake, upgrade/downgrade, and protocol framing integration.
//! All types are canonical and sealed — only ids-000-aa and its designated
//! implementation crates can implement this trait.
//!
//! Carrier-agnostic: no std::net, tokio::net, libp2p, IP, TCP, UDP, QUIC,
//! HTTP, DNS, or any existing networking stack. Protocol messages are opaque
//! byte blobs carried on CarrierType + Vec<u8> Endpoints only.

use crate::types::{
    Endpoint, NodeId, Version,
};
use std::collections::{HashMap, HashSet};

///////////////////////////////////////////////////////////////////////////////
// SEALED TRAIT: Protocol
///////////////////////////////////////////////////////////////////////////////

/// Sealed marker — only ids-000-aa and designated crates can implement.
pub mod private {
    pub trait Sealed {}
}

/// Sealed protocol management trait.
/// Implementations live downstream in ids-protocol and ids-* crates.
pub trait Protocol: private::Sealed {
    /// Configure protocol parameters.
    fn configure_protocol(&self, config: ProtocolConfig) -> Result<(), ProtocolError>;

    /// Get the current protocol version.
    fn protocol_version(&self) -> ProtocolVersion;

    /// Set the protocol version.
    fn set_protocol_version(&self, version: ProtocolVersion) -> Result<(), ProtocolError>;

    /// Get all supported protocol versions.
    fn supported_versions(&self) -> Vec<ProtocolVersion>;

    /// Negotiate protocol version with a peer.
    fn negotiate_version(
        &self,
        peer_id: NodeId,
        offered_versions: Vec<ProtocolVersion>,
    ) -> Result<ProtocolVersion, ProtocolError>;

    /// Get protocol capabilities.
    fn capabilities(&self) -> ProtocolCapabilities;

    /// Add a protocol capability.
    fn add_capability(
        &self,
        capability: ProtocolCapability,
    ) -> Result<(), ProtocolError>;

    /// Remove a protocol capability.
    fn remove_capability(
        &self,
        capability: ProtocolCapability,
    ) -> Result<(), ProtocolError>;

    /// Check if a capability is supported.
    fn has_capability(&self, capability: ProtocolCapability) -> bool;

    /// Perform protocol handshake with a peer.
    fn handshake(
        &self,
        peer_id: NodeId,
        endpoint: Endpoint,
    ) -> Result<ProtocolHandshakeResult, ProtocolError>;

    /// Complete a protocol handshake.
    fn complete_handshake(
        &self,
        peer_id: NodeId,
        handshake_id: u64,
    ) -> Result<ProtocolHandshakeResult, ProtocolError>;

    /// Get pending handshakes.
    fn pending_handshakes(&self) -> Vec<ProtocolHandshakeRecord>;

    /// Upgrade protocol to a higher version.
    fn upgrade_protocol(
        &self,
        target_version: ProtocolVersion,
    ) -> Result<ProtocolUpgradeResult, ProtocolError>;

    /// Downgrade protocol to a lower version.
    fn downgrade_protocol(
        &self,
        target_version: ProtocolVersion,
    ) -> Result<ProtocolDowngradeResult, ProtocolError>;

    /// Get protocol upgrade history.
    fn upgrade_history(&self) -> Vec<ProtocolUpgradeRecord>;

    /// Get protocol downgrade history.
    fn downgrade_history(&self) -> Vec<ProtocolDowngradeRecord>;

    /// Register a protocol extension.
    fn register_extension(
        &self,
        extension: ProtocolExtension,
    ) -> Result<ProtocolExtensionId, ProtocolError>;

    /// Get registered protocol extensions.
    fn registered_extensions(&self) -> Vec<ProtocolExtension>;

    /// Get extension by ID.
    fn extension_by_id(
        &self,
        extension_id: ProtocolExtensionId,
    ) -> Option<ProtocolExtension>;

    /// Check if protocol is compatible with a version.
    fn is_compatible(&self, version: ProtocolVersion) -> bool;

    /// Get protocol compatibility matrix.
    fn compatibility_matrix(&self) -> ProtocolCompatibilityMatrix;

    /// Negotiate protocol features with a peer.
    fn negotiate_features(
        &self,
        peer_id: NodeId,
        offered_features: Vec<ProtocolFeature>,
    ) -> Result<Vec<ProtocolFeature>, ProtocolError>;

    /// Get protocol statistics.
    fn stats(&self) -> ProtocolStats;

    /// Check if protocol is healthy.
    fn is_healthy(&self) -> bool;

    /// Validate protocol configuration.
    fn validate_config(&self, config: &ProtocolConfig) -> Result<(), ProtocolError>;

    /// Get protocol snapshot.
    fn snapshot(&self) -> ProtocolSnapshot;

    /// Restore protocol from snapshot.
    fn restore_from_snapshot(
        &self,
        snapshot: ProtocolSnapshot,
    ) -> Result<(), ProtocolError>;

    /// Get peers by protocol version.
    fn peers_by_version(&self, version: ProtocolVersion) -> Vec<NodeId>;

    /// Get peer protocol version.
    fn peer_version(&self, peer_id: NodeId) -> Option<ProtocolVersion>;

    /// Set peer protocol version.
    fn set_peer_version(
        &self,
        peer_id: NodeId,
        version: ProtocolVersion,
    ) -> Result<(), ProtocolError>;

    /// Get protocol negotiation timeout.
    fn negotiation_timeout_ms(&self) -> u64;

    /// Set protocol negotiation timeout.
    fn set_negotiation_timeout_ms(&self, timeout_ms: u64) -> Result<(), ProtocolError>;
}

///////////////////////////////////////////////////////////////////////////////
// CONFIGURATION
///////////////////////////////////////////////////////////////////////////////

/// Protocol configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProtocolConfig {
    /// Current protocol version.
    pub current_version: ProtocolVersion,
    /// Minimum supported version.
    pub min_supported_version: ProtocolVersion,
    /// Maximum supported version.
    pub max_supported_version: ProtocolVersion,
    /// Enable automatic version negotiation.
    pub enable_auto_negotiation: bool,
    /// Enable protocol upgrades.
    pub enable_upgrades: bool,
    /// Enable protocol downgrades.
    pub enable_downgrades: bool,
    /// Negotiation timeout (ms).
    pub negotiation_timeout_ms: u64,
    /// Handshake timeout (ms).
    pub handshake_timeout_ms: u64,
    /// Maximum pending handshakes.
    pub max_pending_handshakes: u32,
    /// Enable capability negotiation.
    pub enable_capability_negotiation: bool,
    /// Maximum capabilities.
    pub max_capabilities: u32,
    /// Maximum extensions.
    pub max_extensions: u32,
    /// Strict version matching.
    pub strict_version_matching: bool,
}

impl ProtocolConfig {
    pub fn new(current_version: ProtocolVersion) -> Result<Self, ProtocolError> {
        let config = Self {
            current_version,
            min_supported_version: ProtocolVersion::new(1),
            max_supported_version: ProtocolVersion::new(100),
            enable_auto_negotiation: true,
            enable_upgrades: true,
            enable_downgrades: true,
            negotiation_timeout_ms: 10000,
            handshake_timeout_ms: 5000,
            max_pending_handshakes: 100,
            enable_capability_negotiation: true,
            max_capabilities: 50,
            max_extensions: 20,
            strict_version_matching: false,
        };
        config.validate()?;
        Ok(config)
    }

    pub fn validate(&self) -> Result<(), ProtocolError> {
        if self.current_version.0 == 0 {
            return Err(ProtocolError::InvalidConfig {
                field: "current_version".to_string(),
                reason: "current_version must be greater than 0".to_string(),
            });
        }
        if self.min_supported_version.0 == 0 {
            return Err(ProtocolError::InvalidConfig {
                field: "min_supported_version".to_string(),
                reason: "min_supported_version must be greater than 0".to_string(),
            });
        }
        if self.max_supported_version.0 == 0 {
            return Err(ProtocolError::InvalidConfig {
                field: "max_supported_version".to_string(),
                reason: "max_supported_version must be greater than 0".to_string(),
            });
        }
        if self.min_supported_version > self.max_supported_version {
            return Err(ProtocolError::InvalidConfig {
                field: "version_range".to_string(),
                reason: "min_supported_version must be <= max_supported_version".to_string(),
            });
        }
        if self.current_version < self.min_supported_version
            || self.current_version > self.max_supported_version
        {
            return Err(ProtocolError::InvalidConfig {
                field: "current_version".to_string(),
                reason: "current_version must be within supported range".to_string(),
            });
        }
        if self.negotiation_timeout_ms == 0 {
            return Err(ProtocolError::InvalidConfig {
                field: "negotiation_timeout_ms".to_string(),
                reason: "negotiation_timeout_ms must be greater than 0".to_string(),
            });
        }
        if self.handshake_timeout_ms == 0 {
            return Err(ProtocolError::InvalidConfig {
                field: "handshake_timeout_ms".to_string(),
                reason: "handshake_timeout_ms must be greater than 0".to_string(),
            });
        }
        if self.max_pending_handshakes == 0 {
            return Err(ProtocolError::InvalidConfig {
                field: "max_pending_handshakes".to_string(),
                reason: "max_pending_handshakes must be greater than 0".to_string(),
            });
        }
        if self.max_capabilities == 0 {
            return Err(ProtocolError::InvalidConfig {
                field: "max_capabilities".to_string(),
                reason: "max_capabilities must be greater than 0".to_string(),
            });
        }
        if self.max_extensions == 0 {
            return Err(ProtocolError::InvalidConfig {
                field: "max_extensions".to_string(),
                reason: "max_extensions must be greater than 0".to_string(),
            });
        }
        Ok(())
    }

    pub fn default() -> Self {
        Self {
            current_version: ProtocolVersion::new(1),
            min_supported_version: ProtocolVersion::new(1),
            max_supported_version: ProtocolVersion::new(100),
            enable_auto_negotiation: true,
            enable_upgrades: true,
            enable_downgrades: true,
            negotiation_timeout_ms: 10000,
            handshake_timeout_ms: 5000,
            max_pending_handshakes: 100,
            enable_capability_negotiation: true,
            max_capabilities: 50,
            max_extensions: 20,
            strict_version_matching: false,
        }
    }

    pub fn strict() -> Self {
        Self {
            current_version: ProtocolVersion::new(1),
            min_supported_version: ProtocolVersion::new(1),
            max_supported_version: ProtocolVersion::new(5),
            enable_auto_negotiation: false,
            enable_upgrades: false,
            enable_downgrades: false,
            negotiation_timeout_ms: 5000,
            handshake_timeout_ms: 2000,
            max_pending_handshakes: 10,
            enable_capability_negotiation: true,
            max_capabilities: 10,
            max_extensions: 5,
            strict_version_matching: true,
        }
    }

    pub fn lenient() -> Self {
        Self {
            current_version: ProtocolVersion::new(1),
            min_supported_version: ProtocolVersion::new(1),
            max_supported_version: ProtocolVersion::new(1000),
            enable_auto_negotiation: true,
            enable_upgrades: true,
            enable_downgrades: true,
            negotiation_timeout_ms: 60000,
            handshake_timeout_ms: 30000,
            max_pending_handshakes: 1000,
            enable_capability_negotiation: true,
            max_capabilities: 500,
            max_extensions: 200,
            strict_version_matching: false,
        }
    }

    pub fn version_range(&self) -> (ProtocolVersion, ProtocolVersion) {
        (self.min_supported_version, self.max_supported_version)
    }

    pub fn is_version_supported(&self, version: ProtocolVersion) -> bool {
        version >= self.min_supported_version && version <= self.max_supported_version
    }
}

impl Default for ProtocolConfig {
    fn default() -> Self {
        Self::default()
    }
}

///////////////////////////////////////////////////////////////////////////////
// PROTOCOL VERSION
///////////////////////////////////////////////////////////////////////////////

/// Protocol version.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ProtocolVersion(pub u64);

impl ProtocolVersion {
    pub fn new(version: u64) -> Self {
        Self(version)
    }

    pub fn major(&self) -> u64 {
        self.0 / 1000
    }

    pub fn minor(&self) -> u64 {
        (self.0 % 1000) / 100
    }

    pub fn patch(&self) -> u64 {
        self.0 % 100
    }

    pub fn increment_major(&self) -> Self {
        Self(self.0 + 1000)
    }

    pub fn increment_minor(&self) -> Self {
        Self(self.0 + 100)
    }

    pub fn increment_patch(&self) -> Self {
        Self(self.0 + 1)
    }

    pub fn display_string(&self) -> String {
        format!("{}.{}.{}", self.major(), self.minor(), self.patch())
    }
}

///////////////////////////////////////////////////////////////////////////////
// PROTOCOL CAPABILITIES
///////////////////////////////////////////////////////////////////////////////

/// Protocol capabilities.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProtocolCapabilities {
    pub compression: bool,
    pub encryption: bool,
    pub authentication: bool,
    pub fragmentation: bool,
    pub sequencing: bool,
    pub acknowledgments: bool,
    pub flow_control: bool,
    pub congestion_control: bool,
    pub multicast: bool,
    pub broadcast: bool,
    pub custom_capabilities: HashMap<String, bool>,
}

impl ProtocolCapabilities {
    pub fn new() -> Self {
        Self {
            compression: false,
            encryption: false,
            authentication: false,
            fragmentation: false,
            sequencing: false,
            acknowledgments: false,
            flow_control: false,
            congestion_control: false,
            multicast: false,
            broadcast: false,
            custom_capabilities: HashMap::new(),
        }
    }

    pub fn enabled_count(&self) -> u32 {
        let mut count = 0u32;
        if self.compression { count += 1; }
        if self.encryption { count += 1; }
        if self.authentication { count += 1; }
        if self.fragmentation { count += 1; }
        if self.sequencing { count += 1; }
        if self.acknowledgments { count += 1; }
        if self.flow_control { count += 1; }
        if self.congestion_control { count += 1; }
        if self.multicast { count += 1; }
        if self.broadcast { count += 1; }
        count + self.custom_capabilities.len() as u32
    }

    pub fn has_capability(&self, name: &str) -> bool {
        match name {
            "compression" => self.compression,
            "encryption" => self.encryption,
            "authentication" => self.authentication,
            "fragmentation" => self.fragmentation,
            "sequencing" => self.sequencing,
            "acknowledgments" => self.acknowledgments,
            "flow_control" => self.flow_control,
            "congestion_control" => self.congestion_control,
            "multicast" => self.multicast,
            "broadcast" => self.broadcast,
            _ => self.custom_capabilities.get(name).copied().unwrap_or(false),
        }
    }

    pub fn set_capability(&mut self, name: &str, enabled: bool) {
        match name {
            "compression" => self.compression = enabled,
            "encryption" => self.encryption = enabled,
            "authentication" => self.authentication = enabled,
            "fragmentation" => self.fragmentation = enabled,
            "sequencing" => self.sequencing = enabled,
            "acknowledgments" => self.acknowledgments = enabled,
            "flow_control" => self.flow_control = enabled,
            "congestion_control" => self.congestion_control = enabled,
            "multicast" => self.multicast = enabled,
            "broadcast" => self.broadcast = enabled,
            _ => {
                self.custom_capabilities.insert(name.to_string(), enabled);
            }
        }
    }
}

impl Default for ProtocolCapabilities {
    fn default() -> Self {
        Self::new()
    }
}

/// Protocol capability enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ProtocolCapability {
    Compression,
    Encryption,
    Authentication,
    Fragmentation,
    Sequencing,
    Acknowledgments,
    FlowControl,
    CongestionControl,
    Multicast,
    Broadcast,
    Custom(u8),
}

impl ProtocolCapability {
    pub fn as_str(&self) -> &'static str {
        match self {
            ProtocolCapability::Compression => "compression",
            ProtocolCapability::Encryption => "encryption",
            ProtocolCapability::Authentication => "authentication",
            ProtocolCapability::Fragmentation => "fragmentation",
            ProtocolCapability::Sequencing => "sequencing",
            ProtocolCapability::Acknowledgments => "acknowledgments",
            ProtocolCapability::FlowControl => "flow_control",
            ProtocolCapability::CongestionControl => "congestion_control",
            ProtocolCapability::Multicast => "multicast",
            ProtocolCapability::Broadcast => "broadcast",
            ProtocolCapability::Custom(_) => "custom",
        }
    }

    pub fn from_str(name: &str) -> Option<Self> {
        match name {
            "compression" => Some(ProtocolCapability::Compression),
            "encryption" => Some(ProtocolCapability::Encryption),
            "authentication" => Some(ProtocolCapability::Authentication),
            "fragmentation" => Some(ProtocolCapability::Fragmentation),
            "sequencing" => Some(ProtocolCapability::Sequencing),
            "acknowledgments" => Some(ProtocolCapability::Acknowledgments),
            "flow_control" => Some(ProtocolCapability::FlowControl),
            "congestion_control" => Some(ProtocolCapability::CongestionControl),
            "multicast" => Some(ProtocolCapability::Multicast),
            "broadcast" => Some(ProtocolCapability::Broadcast),
            _ => None,
        }
    }
}

///////////////////////////////////////////////////////////////////////////////
// PROTOCOL HANDSHAKE
///////////////////////////////////////////////////////////////////////////////

/// Protocol handshake result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProtocolHandshakeResult {
    pub handshake_id: u64,
    pub peer_id: NodeId,
    pub negotiated_version: ProtocolVersion,
    pub negotiated_capabilities: ProtocolCapabilities,
    pub completed: bool,
    pub completed_at: u64,
}

/// Protocol handshake record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProtocolHandshakeRecord {
    pub handshake_id: u64,
    pub peer_id: NodeId,
    pub endpoint: Endpoint,
    pub offered_version: ProtocolVersion,
    pub negotiated_version: Option<ProtocolVersion>,
    pub state: HandshakeState,
    pub created_at: u64,
    pub completed_at: Option<u64>,
}

/// Handshake state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum HandshakeState {
    Initiated,
    OfferSent,
    OfferReceived,
    Accepted,
    Rejected,
    Completed,
    Failed,
    TimedOut,
}

impl HandshakeState {
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            HandshakeState::Completed
                | HandshakeState::Failed
                | HandshakeState::TimedOut
                | HandshakeState::Rejected
        )
    }

    pub fn is_pending(&self) -> bool {
        !self.is_termial()
    }

    fn is_termial(&self) -> bool {
        matches!(
            self,
            HandshakeState::Completed
                | HandshakeState::Failed
                | HandshakeState::TimedOut
                | HandshakeState::Rejected
        )
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            HandshakeState::Initiated => "initiated",
            HandshakeState::OfferSent => "offer_sent",
            HandshakeState::OfferReceived => "offer_received",
            HandshakeState::Accepted => "accepted",
            HandshakeState::Rejected => "rejected",
            HandshakeState::Completed => "completed",
            HandshakeState::Failed => "failed",
            HandshakeState::TimedOut => "timed_out",
        }
    }
}

///////////////////////////////////////////////////////////////////////////////
// PROTOCOL UPGRADE / DOWNGRADE
///////////////////////////////////////////////////////////////////////////////

/// Protocol upgrade result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProtocolUpgradeResult {
    pub from_version: ProtocolVersion,
    pub to_version: ProtocolVersion,
    pub success: bool,
    pub upgraded_at: u64,
    pub duration_ms: u64,
}

/// Protocol downgrade result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProtocolDowngradeResult {
    pub from_version: ProtocolVersion,
    pub to_version: ProtocolVersion,
    pub success: bool,
    pub downgraded_at: u64,
    pub duration_ms: u64,
}

/// Protocol upgrade record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProtocolUpgradeRecord {
    pub upgrade_id: u64,
    pub from_version: ProtocolVersion,
    pub to_version: ProtocolVersion,
    pub success: bool,
    pub upgraded_at: u64,
    pub duration_ms: u64,
}

/// Protocol downgrade record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProtocolDowngradeRecord {
    pub downgrade_id: u64,
    pub from_version: ProtocolVersion,
    pub to_version: ProtocolVersion,
    pub success: bool,
    pub downgraded_at: u64,
    pub duration_ms: u64,
}

///////////////////////////////////////////////////////////////////////////////
// PROTOCOL EXTENSION
///////////////////////////////////////////////////////////////////////////////

/// Protocol extension ID.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ProtocolExtensionId(pub u64);

/// Protocol extension.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProtocolExtension {
    pub extension_id: ProtocolExtensionId,
    pub name: String,
    pub version: ProtocolVersion,
    pub description: Vec<u8>,
    pub enabled: bool,
}

impl ProtocolExtension {
    pub fn new(name: String, version: ProtocolVersion) -> Self {
        Self {
            extension_id: ProtocolExtensionId(0),
            name,
            version,
            description: Vec::new(),
            enabled: true,
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }
}

///////////////////////////////////////////////////////////////////////////////
// PROTOCOL FEATURE
///////////////////////////////////////////////////////////////////////////////

/// Protocol feature.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ProtocolFeature {
    LargeMessages,
    ZeroCopy,
    Backpressure,
    Prioritization,
    Batching,
    Compression,
    Encryption,
    Authentication,
}

impl ProtocolFeature {
    pub fn as_str(&self) -> &'static str {
        match self {
            ProtocolFeature::LargeMessages => "large_messages",
            ProtocolFeature::ZeroCopy => "zero_copy",
            ProtocolFeature::Backpressure => "backpressure",
            ProtocolFeature::Prioritization => "prioritization",
            ProtocolFeature::Batching => "batching",
            ProtocolFeature::Compression => "compression",
            ProtocolFeature::Encryption => "encryption",
            ProtocolFeature::Authentication => "authentication",
        }
    }
}

///////////////////////////////////////////////////////////////////////////////
// PROTOCOL COMPATIBILITY
///////////////////////////////////////////////////////////////////////////////

/// Protocol compatibility matrix.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProtocolCompatibilityMatrix {
    pub version_pairs: HashMap<(ProtocolVersion, ProtocolVersion), bool>,
}

impl ProtocolCompatibilityMatrix {
    pub fn new() -> Self {
        Self {
            version_pairs: HashMap::new(),
        }
    }

    pub fn is_compatible(
        &self,
        v1: ProtocolVersion,
        v2: ProtocolVersion,
    ) -> bool {
        self.version_pairs
            .get(&(v1, v2))
            .copied()
            .unwrap_or(v1 == v2)
    }

    pub fn set_compatibility(
        &mut self,
        v1: ProtocolVersion,
        v2: ProtocolVersion,
        compatible: bool,
    ) {
        self.version_pairs.insert((v1, v2), compatible);
        self.version_pairs.insert((v2, v1), compatible);
    }
}

///////////////////////////////////////////////////////////////////////////////
// PROTOCOL STATS
///////////////////////////////////////////////////////////////////////////////

/// Protocol statistics.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProtocolStats {
    pub current_version: ProtocolVersion,
    pub total_handshakes: u64,
    pub completed_handshakes: u64,
    pub failed_handshakes: u64,
    pub pending_handshakes: u64,
    pub total_upgrades: u64,
    pub successful_upgrades: u64,
    pub failed_upgrades: u64,
    pub total_downgrades: u64,
    pub successful_downgrades: u64,
    pub failed_downgrades: u64,
    pub total_extensions: u32,
    pub active_extensions: u32,
    pub total_negotiations: u64,
    pub successful_negotiations: u64,
    pub failed_negotiations: u64,
}

impl ProtocolStats {
    pub fn new(current_version: ProtocolVersion) -> Self {
        Self {
            current_version,
            total_handshakes: 0,
            completed_handshakes: 0,
            failed_handshakes: 0,
            pending_handshakes: 0,
            total_upgrades: 0,
            successful_upgrades: 0,
            failed_upgrades: 0,
            total_downgrades: 0,
            successful_downgrades: 0,
            failed_downgrades: 0,
            total_extensions: 0,
            active_extensions: 0,
            total_negotiations: 0,
            successful_negotiations: 0,
            failed_negotiations: 0,
        }
    }

    pub fn handshake_success_rate(&self) -> f64 {
        if self.total_handshakes == 0 {
            return 100.0;
        }
        self.completed_handshakes as f64 / self.total_handshakes as f64 * 100.0
    }

    pub fn upgrade_success_rate(&self) -> f64 {
        if self.total_upgrades == 0 {
            return 100.0;
        }
        self.successful_upgrades as f64 / self.total_upgrades as f64 * 100.0
    }

    pub fn downgrade_success_rate(&self) -> f64 {
        if self.total_downgrades == 0 {
            return 100.0;
        }
        self.successful_downgrades as f64 / self.total_downgrades as f64 * 100.0
    }

    pub fn negotiation_success_rate(&self) -> f64 {
        if self.total_negotiations == 0 {
            return 100.0;
        }
        self.successful_negotiations as f64 / self.total_negotiations as f64 * 100.0
    }
}

///////////////////////////////////////////////////////////////////////////////
// PROTOCOL SNAPSHOT
///////////////////////////////////////////////////////////////////////////////

/// Protocol snapshot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProtocolSnapshot {
    pub version: u64,
    pub current_protocol_version: ProtocolVersion,
    pub capabilities: ProtocolCapabilities,
    pub extensions: Vec<ProtocolExtension>,
    pub peer_versions: HashMap<NodeId, ProtocolVersion>,
    pub stats: ProtocolStats,
}

impl ProtocolSnapshot {
    pub fn peer_count(&self) -> usize {
        self.peer_versions.len()
    }

    pub fn extension_count(&self) -> usize {
        self.extensions.len()
    }
}

///////////////////////////////////////////////////////////////////////////////
// PROTOCOL ERROR
///////////////////////////////////////////////////////////////////////////////

/// Protocol management errors.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
pub enum ProtocolError {
    #[error("invalid config: {field} - {reason}")]
    InvalidConfig { field: String, reason: String },

    #[error("version not supported: {version:?}")]
    VersionNotSupported { version: ProtocolVersion },

    #[error("version out of range: {version:?}")]
    VersionOutOfRange { version: ProtocolVersion },

    #[error("peer not found: {peer_id:?}")]
    PeerNotFound { peer_id: NodeId },

    #[error("handshake not found: {handshake_id}")]
    HandshakeNotFound { handshake_id: u64 },

    #[error("handshake already exists: {handshake_id}")]
    HandshakeAlreadyExists { handshake_id: u64 },

    #[error("handshake already terminal: {handshake_id}")]
    HandshakeTerminal { handshake_id: u64 },

    #[error("upgrade not supported: {from:?} -> {to:?}")]
    UpgradeNotSupported {
        from: ProtocolVersion,
        to: ProtocolVersion,
    },

    #[error("downgrade not supported: {from:?} -> {to:?}")]
    DowngradeNotSupported {
        from: ProtocolVersion,
        to: ProtocolVersion,
    },

    #[error("extension not found: {extension_id:?}")]
    ExtensionNotFound { extension_id: ProtocolExtensionId },

    #[error("extension already exists: {extension_id:?}")]
    ExtensionAlreadyExists { extension_id: ProtocolExtensionId },

    #[error("maximum extensions exceeded: {max_extensions}")]
    MaxExtensionsExceeded { max_extensions: u32 },

    #[error("maximum capabilities exceeded: {max_capabilities}")]
    MaxCapabilitiesExceeded { max_capabilities: u32 },

    #[error("maximum pending handshakes exceeded: {max_pending_handshakes}")]
    MaxPendingHandshakesExceeded { max_pending_handshakes: u32 },

    #[error("capability not supported: {capability:?}")]
    CapabilityNotSupported { capability: ProtocolCapability },

    #[error("incompatible versions: {v1:?} and {v2:?}")]
    IncompatibleVersions {
        v1: ProtocolVersion,
        v2: ProtocolVersion,
    },

    #[error("negotiation timeout: {peer_id:?}")]
    NegotiationTimeout { peer_id: NodeId },

    #[error("handshake timeout: {peer_id:?}")]
    HandshakeTimeout { peer_id: NodeId },

    #[error("internal error: {message}")]
    Internal { message: String },
}

impl ProtocolError {
    pub fn category(&self) -> &'static str {
        match self {
            ProtocolError::InvalidConfig { .. } => "invalid_config",
            ProtocolError::VersionNotSupported { .. } => "version_not_supported",
            ProtocolError::VersionOutOfRange { .. } => "version_out_of_range",
            ProtocolError::PeerNotFound { .. } => "peer_not_found",
            ProtocolError::HandshakeNotFound { .. } => "handshake_not_found",
            ProtocolError::HandshakeAlreadyExists { .. } => "handshake_already_exists",
            ProtocolError::HandshakeTerminal { .. } => "handshake_terminal",
            ProtocolError::UpgradeNotSupported { .. } => "upgrade_not_supported",
            ProtocolError::DowngradeNotSupported { .. } => "downgrade_not_supported",
            ProtocolError::ExtensionNotFound { .. } => "extension_not_found",
            ProtocolError::ExtensionAlreadyExists { .. } => "extension_already_exists",
            ProtocolError::MaxExtensionsExceeded { .. } => "max_extensions_exceeded",
            ProtocolError::MaxCapabilitiesExceeded { .. } => "max_capabilities_exceeded",
            ProtocolError::MaxPendingHandshakesExceeded { .. } => "max_pending_handshakes_exceeded",
            ProtocolError::CapabilityNotSupported { .. } => "capability_not_supported",
            ProtocolError::IncompatibleVersions { .. } => "incompatible_versions",
            ProtocolError::NegotiationTimeout { .. } => "negotiation_timeout",
            ProtocolError::HandshakeTimeout { .. } => "handshake_timeout",
            ProtocolError::Internal { .. } => "internal",
        }
    }

    pub fn is_recoverable(&self) -> bool {
        matches!(
            self,
            ProtocolError::VersionNotSupported { .. }
                | ProtocolError::NegotiationTimeout { .. }
                | ProtocolError::HandshakeTimeout { .. }
                | ProtocolError::IncompatibleVersions { .. }
        )
    }

    pub fn suggested_action(&self) -> &'static str {
        match self {
            ProtocolError::InvalidConfig { .. } => "correct the protocol configuration",
            ProtocolError::VersionNotSupported { .. } => "use a supported protocol version",
            ProtocolError::VersionOutOfRange { .. } => "use a version within the supported range",
            ProtocolError::PeerNotFound { .. } => "register the peer first",
            ProtocolError::HandshakeNotFound { .. } => "create the handshake first",
            ProtocolError::HandshakeAlreadyExists { .. } => "use existing handshake",
            ProtocolError::HandshakeTerminal { .. } => "create a new handshake",
            ProtocolError::UpgradeNotSupported { .. } => "check upgrade path compatibility",
            ProtocolError::DowngradeNotSupported { .. } => "check downgrade path compatibility",
            ProtocolError::ExtensionNotFound { .. } => "register the extension first",
            ProtocolError::ExtensionAlreadyExists { .. } => "use existing extension",
            ProtocolError::MaxExtensionsExceeded { .. } => "remove some extensions or increase limit",
            ProtocolError::MaxCapabilitiesExceeded { .. } => {
                "remove some capabilities or increase limit"
            }
            ProtocolError::MaxPendingHandshakesExceeded { .. } => {
                "complete or cancel pending handshakes"
            }
            ProtocolError::CapabilityNotSupported { .. } => "use a supported capability",
            ProtocolError::IncompatibleVersions { .. } => "use compatible versions",
            ProtocolError::NegotiationTimeout { .. } => "retry negotiation",
            ProtocolError::HandshakeTimeout { .. } => "retry handshake",
            ProtocolError::Internal { .. } => "contact system administrator",
        }
    }
}

///////////////////////////////////////////////////////////////////////////////
// INTERNAL STATE
///////////////////////////////////////////////////////////////////////////////

/// Internal handshake state.
#[derive(Debug, Clone)]
struct InternalHandshake {
    handshake_id: u64,
    peer_id: NodeId,
    endpoint: Endpoint,
    offered_version: ProtocolVersion,
    negotiated_version: Option<ProtocolVersion>,
    state: HandshakeState,
    created_at: u64,
    completed_at: Option<u64>,
}

/// Internal peer version mapping.
#[derive(Debug, Clone)]
struct InternalPeerVersion {
    peer_id: NodeId,
    version: ProtocolVersion,
    last_updated: u64,
}

/// Internal protocol state.
#[derive(Debug, Clone)]
struct ProtocolState {
    config: ProtocolConfig,
    capabilities: ProtocolCapabilities,
    extensions: HashMap<ProtocolExtensionId, ProtocolExtension>,
    extension_counter: u64,
    handshakes: HashMap<u64, InternalHandshake>,
    handshake_counter: u64,
    peer_versions: HashMap<NodeId, ProtocolVersion>,
    upgrade_history: Vec<ProtocolUpgradeRecord>,
    downgrade_history: Vec<ProtocolDowngradeRecord>,
    upgrade_counter: u64,
    downgrade_counter: u64,
    stats: ProtocolStats,
    version: u64,
}

impl ProtocolState {
    fn new(config: ProtocolConfig) -> Self {
        Self {
            config: config.clone(),
            capabilities: ProtocolCapabilities::new(),
            extensions: HashMap::new(),
            extension_counter: 0,
            handshakes: HashMap::new(),
            handshake_counter: 0,
            peer_versions: HashMap::new(),
            upgrade_history: Vec::new(),
            downgrade_history: Vec::new(),
            upgrade_counter: 0,
            downgrade_counter: 0,
            stats: ProtocolStats::new(config.current_version),
            version: 0,
        }
    }
}

///////////////////////////////////////////////////////////////////////////////
// PROTOCOL ENGINE (REFERENCE IMPLEMENTATION)
///////////////////////////////////////////////////////////////////////////////

/// Reference protocol engine implementing the Protocol trait.
pub struct ProtocolEngine {
    state: std::cell::RefCell<ProtocolState>,
}

impl ProtocolEngine {
    pub fn new() -> Self {
        Self {
            state: std::cell::RefCell::new(ProtocolState::new(ProtocolConfig::default())),
        }
    }

    pub fn with_config(config: ProtocolConfig) -> Result<Self, ProtocolError> {
        config.validate()?;
        Ok(Self {
            state: std::cell::RefCell::new(ProtocolState::new(config)),
        })
    }

    fn ensure_config(&self) -> ProtocolConfig {
        self.state.borrow().config.clone()
    }
}

///////////////////////////////////////////////////////////////////////////////
// IMPL PROTOCOL FOR PROTOCOLENGINE
///////////////////////////////////////////////////////////////////////////////

impl private::Sealed for ProtocolEngine {}

impl Protocol for ProtocolEngine {
    fn configure_protocol(&self, config: ProtocolConfig) -> Result<(), ProtocolError> {
        config.validate()?;
        let mut state = self.state.borrow_mut();
        state.config = config.clone();
        state.stats = ProtocolStats::new(config.current_version);
        Ok(())
    }

    fn protocol_version(&self) -> ProtocolVersion {
        let state = self.state.borrow();
        state.config.current_version
    }

    fn set_protocol_version(&self, version: ProtocolVersion) -> Result<(), ProtocolError> {
        let config = self.ensure_config();
        if !config.is_version_supported(version) {
            return Err(ProtocolError::VersionOutOfRange { version });
        }
        let mut state = self.state.borrow_mut();
        state.config.current_version = version;
        state.stats.current_version = version;
        state.version += 1;
        Ok(())
    }

    fn supported_versions(&self) -> Vec<ProtocolVersion> {
        let state = self.state.borrow();
        let min = state.config.min_supported_version.0;
        let max = state.config.max_supported_version.0;
        (min..=max).map(ProtocolVersion).collect()
    }

    fn negotiate_version(
        &self,
        peer_id: NodeId,
        offered_versions: Vec<ProtocolVersion>,
    ) -> Result<ProtocolVersion, ProtocolError> {
        let config = self.ensure_config();
        let mut state = self.state.borrow_mut();

        state.stats.total_negotiations += 1;

        // Find the highest mutually supported version
        let mut best_version = None;
        for &offered in &offered_versions {
            if config.is_version_supported(offered) {
                match &best_version {
                    None => best_version = Some(offered),
                    Some(current) => if offered > *current { best_version = Some(offered); }
                }
            }
        }

        let negotiated = match best_version {
            Some(v) => v,
            None => {
                state.stats.failed_negotiations += 1;
                return Err(ProtocolError::IncompatibleVersions {
                    v1: config.current_version,
                    v2: offered_versions
                        .first()
                        .copied()
                        .unwrap_or(ProtocolVersion::new(0)),
                });
            }
        };

        state.stats.successful_negotiations += 1;
        state.peer_versions.insert(peer_id, negotiated);

        Ok(negotiated)
    }

    fn capabilities(&self) -> ProtocolCapabilities {
        let state = self.state.borrow();
        state.capabilities.clone()
    }

    fn add_capability(
        &self,
        capability: ProtocolCapability,
    ) -> Result<(), ProtocolError> {
        let config = self.ensure_config();
        let mut state = self.state.borrow_mut();

        let current_count = state.capabilities.enabled_count();
        if current_count >= config.max_capabilities {
            return Err(ProtocolError::MaxCapabilitiesExceeded {
                max_capabilities: config.max_capabilities,
            });
        }

        match capability {
            ProtocolCapability::Compression => state.capabilities.compression = true,
            ProtocolCapability::Encryption => state.capabilities.encryption = true,
            ProtocolCapability::Authentication => state.capabilities.authentication = true,
            ProtocolCapability::Fragmentation => state.capabilities.fragmentation = true,
            ProtocolCapability::Sequencing => state.capabilities.sequencing = true,
            ProtocolCapability::Acknowledgments => state.capabilities.acknowledgments = true,
            ProtocolCapability::FlowControl => state.capabilities.flow_control = true,
            ProtocolCapability::CongestionControl => state.capabilities.congestion_control = true,
            ProtocolCapability::Multicast => state.capabilities.multicast = true,
            ProtocolCapability::Broadcast => state.capabilities.broadcast = true,
            ProtocolCapability::Custom(n) => {
                state
                    .capabilities
                    .custom_capabilities
                    .insert(format!("custom_{}"), n > 0);
            }
        }

        state.version += 1;
        Ok(())
    }

    fn remove_capability(
        &self,
        capability: ProtocolCapability,
    ) -> Result<(), ProtocolError> {
        let mut state = self.state.borrow_mut();

        match capability {
            ProtocolCapability::Compression => state.capabilities.compression = false,
            ProtocolCapability::Encryption => state.capabilities.encryption = false,
            ProtocolCapability::Authentication => state.capabilities.authentication = false,
            ProtocolCapability::Fragmentation => state.capabilities.fragmentation = false,
            ProtocolCapability::Sequencing => state.capabilities.sequencing = false,
            ProtocolCapability::Acknowledgments => state.capabilities.acknowledgments = false,
            ProtocolCapability::FlowControl => state.capabilities.flow_control = false,
            ProtocolCapability::CongestionControl => state.capabilities.congestion_control = false,
            ProtocolCapability::Multicast => state.capabilities.multicast = false,
            ProtocolCapability::Broadcast => state.capabilities.broadcast = false,
            ProtocolCapability::Custom(n) => {
                state
                    .capabilities
                    .custom_capabilities
                    .remove(&format!("custom_{}", n));
            }
        }

        state.version += 1;
        Ok(())
    }

    fn has_capability(&self, capability: ProtocolCapability) -> bool {
        let state = self.state.borrow();
        match capability {
            ProtocolCapability::Compression => state.capabilities.compression,
            ProtocolCapability::Encryption => state.capabilities.encryption,
            ProtocolCapability::Authentication => state.capabilities.authentication,
            ProtocolCapability::Fragmentation => state.capabilities.fragmentation,
            ProtocolCapability::Sequencing => state.capabilities.sequencing,
            ProtocolCapability::Acknowledgments => state.capabilities.acknowledgments,
            ProtocolCapability::FlowControl => state.capabilities.flow_control,
            ProtocolCapability::CongestionControl => state.capabilities.congestion_control,
            ProtocolCapability::Multicast => state.capabilities.multicast,
            ProtocolCapability::Broadcast => state.capabilities.broadcast,
            ProtocolCapability::Custom(n) => state
                .capabilities
                .custom_capabilities
                .get(&format!("custom_{}", n))
                .copied()
                .unwrap_or(false),
        }
    }

    fn handshake(
        &self,
        peer_id: NodeId,
        endpoint: Endpoint,
    ) -> Result<ProtocolHandshakeResult, ProtocolError> {
        let config = self.ensure_config();
        let mut state = self.state.borrow_mut();

        if state.handshakes.len() as u32 >= config.max_pending_handshakes {
            return Err(ProtocolError::MaxPendingHandshakesExceeded {
                max_pending_handshakes: config.max_pending_handshakes,
            });
        }

        state.handshake_counter += 1;
        let handshake_id = state.handshake_counter;

        let handshake = InternalHandshake {
            handshake_id,
            peer_id,
            endpoint: endpoint.clone(),
            offered_version: state.config.current_version,
            negotiated_version: None,
            state: HandshakeState::Initiated,
            created_at: 0,
            completed_at: None,
        };

        state.handshakes.insert(handshake_id, handshake);
        state.stats.total_handshakes += 1;
        state.stats.pending_handshakes += 1;

        Ok(ProtocolHandshakeResult {
            handshake_id,
            peer_id,
            negotiated_version: state.config.current_version,
            negotiated_capabilities: state.capabilities.clone(),
            completed: false,
            completed_at: 0,
        })
    }

    fn complete_handshake(
        &self,
        peer_id: NodeId,
        handshake_id: u64,
    ) -> Result<ProtocolHandshakeResult, ProtocolError> {
        let mut state = self.state.borrow_mut();

        let handshake = state
            .handshakes
            .get_mut(&handshake_id)
            .ok_or_else(|| ProtocolError::HandshakeNotFound { handshake_id })?;

        if handshake.peer_id != peer_id {
            return Err(ProtocolError::PeerNotFound { peer_id });
        }

        if handshake.state.is_terminal() {
            return Err(ProtocolError::HandshakeTerminal { handshake_id });
        }

        handshake.state = HandshakeState::Completed;
        handshake.completed_at = Some(0);
        handshake.negotiated_version = Some(state.config.current_version);

        state.stats.completed_handshakes += 1;
        state.stats.pending_handshakes = state.stats.pending_handshakes.saturating_sub(1);

        state.peer_versions.insert(peer_id, state.config.current_version);

        Ok(ProtocolHandshakeResult {
            handshake_id,
            peer_id,
            negotiated_version: state.config.current_version,
            negotiated_capabilities: state.capabilities.clone(),
            completed: true,
            completed_at: 0,
        })
    }

    fn pending_handshakes(&self) -> Vec<ProtocolHandshakeRecord> {
        let state = self.state.borrow();
        state
            .handshakes
            .values()
            .filter(|h| !h.state.is_terminal())
            .map(|h| ProtocolHandshakeRecord {
                handshake_id: h.handshake_id,
                peer_id: h.peer_id,
                endpoint: h.endpoint.clone(),
                offered_version: h.offered_version,
                negotiated_version: h.negotiated_version,
                state: h.state,
                created_at: h.created_at,
                completed_at: h.completed_at,
            })
            .collect()
    }

    fn upgrade_protocol(
        &self,
        target_version: ProtocolVersion,
    ) -> Result<ProtocolUpgradeResult, ProtocolError> {
        let config = self.ensure_config();
        let mut state = self.state.borrow_mut();

        if !config.enable_upgrades {
            return Err(ProtocolError::UpgradeNotSupported {
                from: state.config.current_version,
                to: target_version,
            });
        }

        if !config.is_version_supported(target_version) {
            return Err(ProtocolError::VersionNotSupported { version: target_version });
        }

        if target_version <= state.config.current_version {
            return Err(ProtocolError::UpgradeNotSupported {
                from: state.config.current_version,
                to: target_version,
            });
        }

        let from_version = state.config.current_version;
        let start_ms = 0u64;

        state.config.current_version = target_version;
        state.stats.current_version = target_version;

        state.upgrade_counter += 1;
        let upgrade_id = state.upgrade_counter;

        let record = ProtocolUpgradeRecord {
            upgrade_id,
            from_version,
            to_version: target_version,
            success: true,
            upgraded_at: 0,
            duration_ms: 0,
        };

        state.upgrade_history.push(record);
        state.stats.total_upgrades += 1;
        state.stats.successful_upgrades += 1;

        state.version += 1;

        Ok(ProtocolUpgradeResult {
            from_version,
            to_version: target_version,
            success: true,
            upgraded_at: 0,
            duration_ms: 0,
        })
    }

    fn downgrade_protocol(
        &self,
        target_version: ProtocolVersion,
    ) -> Result<ProtocolDowngradeResult, ProtocolError> {
        let config = self.ensure_config();
        let mut state = self.state.borrow_mut();

        if !config.enable_downgrades {
            return Err(ProtocolError::DowngradeNotSupported {
                from: state.config.current_version,
                to: target_version,
            });
        }

        if !config.is_version_supported(target_version) {
            return Err(ProtocolError::VersionNotSupported { version: target_version });
        }

        if target_version >= state.config.current_version {
            return Err(ProtocolError::DowngradeNotSupported {
                from: state.config.current_version,
                to: target_version,
            });
        }

        let from_version = state.config.current_version;
        let start_ms = 0u64;

        state.config.current_version = target_version;
        state.stats.current_version = target_version;

        state.downgrade_counter += 1;
        let downgrade_id = state.downgrade_counter;

        let record = ProtocolDowngradeRecord {
            downgrade_id,
            from_version,
            to_version: target_version,
            success: true,
            downgraded_at: 0,
            duration_ms: 0,
        };

        state.downgrade_history.push(record);
        state.stats.total_downgrades += 1;
        state.stats.successful_downgrades += 1;

        state.version += 1;

        Ok(ProtocolDowngradeResult {
            from_version,
            to_version: target_version,
            success: true,
            downgraded_at: 0,
            duration_ms: 0,
        })
    }

    fn upgrade_history(&self) -> Vec<ProtocolUpgradeRecord> {
        let state = self.state.borrow();
        state.upgrade_history.clone()
    }

    fn downgrade_history(&self) -> Vec<ProtocolDowngradeRecord> {
        let state = self.state.borrow();
        state.downgrade_history.clone()
    }

    fn register_extension(
        &self,
        extension: ProtocolExtension,
    ) -> Result<ProtocolExtensionId, ProtocolError> {
        let config = self.ensure_config();
        let mut state = self.state.borrow_mut();

        if state.extensions.len() as u32 >= config.max_extensions {
            return Err(ProtocolError::MaxExtensionsExceeded {
                max_extensions: config.max_extensions,
            });
        }

        state.extension_counter += 1;
        let extension_id = ProtocolExtensionId(state.extension_counter);

        let mut ext = extension;
        ext.extension_id = extension_id;

        state.extensions.insert(extension_id, ext.clone());
        state.stats.total_extensions += 1;
        if ext.enabled {
            state.stats.active_extensions += 1;
        }

        state.version += 1;

        Ok(extension_id)
    }

    fn registered_extensions(&self) -> Vec<ProtocolExtension> {
        let state = self.state.borrow();
        state.extensions.values().cloned().collect()
    }

    fn extension_by_id(
        &self,
        extension_id: ProtocolExtensionId,
    ) -> Option<ProtocolExtension> {
        let state = self.state.borrow();
        state.extensions.get(&extension_id).cloned()
    }

    fn is_compatible(&self, version: ProtocolVersion) -> bool {
        let state = self.state.borrow();
        state.config.is_version_supported(version)
    }

    fn compatibility_matrix(&self) -> ProtocolCompatibilityMatrix {
        let state = self.state.borrow();
        let mut matrix = ProtocolCompatibilityMatrix::new();
        let versions = state.supported_versions_internal();
        for v1 in &versions {
            for v2 in &versions {
                matrix.set_compatibility(*v1, *v2, true);
            }
        }
        matrix
    }

    fn negotiate_features(
        &self,
        peer_id: NodeId,
        offered_features: Vec<ProtocolFeature>,
    ) -> Result<Vec<ProtocolFeature>, ProtocolError> {
        let state = self.state.borrow();
        if !state.peer_versions.contains_key(&peer_id) {
            return Err(ProtocolError::PeerNotFound { peer_id });
        }

        // All features are negotiable in the reference implementation
        let mut negotiated = Vec::new();
        for feature in offered_features {
            negotiated.push(feature);
        }
        Ok(negotiated)
    }

    fn stats(&self) -> ProtocolStats {
        let state = self.state.borrow();
        state.stats.clone()
    }

    fn is_healthy(&self) -> bool {
        let state = self.state.borrow();
        state.stats.failed_handshakes == 0
            || state.stats.handshake_success_rate() > 50.0
    }

    fn validate_config(&self, config: &ProtocolConfig) -> Result<(), ProtocolError> {
        config.validate()
    }

    fn snapshot(&self) -> ProtocolSnapshot {
        let state = self.state.borrow();
        ProtocolSnapshot {
            version: state.version,
            current_protocol_version: state.config.current_version,
            capabilities: state.capabilities.clone(),
            extensions: state.extensions.values().cloned().collect(),
            peer_versions: state.peer_versions.clone(),
            stats: state.stats.clone(),
        }
    }

    fn restore_from_snapshot(
        &self,
        snapshot: ProtocolSnapshot,
    ) -> Result<(), ProtocolError> {
        let mut state = self.state.borrow_mut();
        state.version = snapshot.version;
        state.config.current_version = snapshot.current_protocol_version;
        state.capabilities = snapshot.capabilities;
        state.stats = snapshot.stats;

        state.extensions.clear();
        for ext in &snapshot.extensions {
            state.extensions.insert(ext.extension_id, ext.clone());
        }

        state.peer_versions = snapshot.peer_versions;

        Ok(())
    }

    fn peers_by_version(&self, version: ProtocolVersion) -> Vec<NodeId> {
        let state = self.state.borrow();
        state
            .peer_versions
            .iter()
            .filter(|(_, v)| **v == version)
            .map(|(peer_id, _)| *peer_id)
            .collect()
    }

    fn peer_version(&self, peer_id: NodeId) -> Option<ProtocolVersion> {
        let state = self.state.borrow();
        state.peer_versions.get(&peer_id).copied()
    }

    fn set_peer_version(
        &self,
        peer_id: NodeId,
        version: ProtocolVersion,
    ) -> Result<(), ProtocolError> {
        let config = self.ensure_config();
        if !config.is_version_supported(version) {
            return Err(ProtocolError::VersionNotSupported { version });
        }
        let mut state = self.state.borrow_mut();
        state.peer_versions.insert(peer_id, version);
        state.version += 1;
        Ok(())
    }

    fn negotiation_timeout_ms(&self) -> u64 {
        let state = self.state.borrow();
        state.config.negotiation_timeout_ms
    }

    fn set_negotiation_timeout_ms(&self, timeout_ms: u64) -> Result<(), ProtocolError> {
        let mut state = self.state.borrow_mut();
        if timeout_ms == 0 {
            return Err(ProtocolError::InvalidConfig {
                field: "negotiation_timeout_ms".to_string(),
                reason: "timeout must be greater than 0".to_string(),
            });
        }
        state.config.negotiation_timeout_ms = timeout_ms;
        state.version += 1;
        Ok(())
    }

    /// Get the handshake timeout in milliseconds.
    pub fn handshake_timeout_ms(&self) -> u64 {
        let state = self.state.borrow();
        state.config.handshake_timeout_ms
    }

    /// Set the handshake timeout in milliseconds.
    pub fn set_handshake_timeout_ms(&self, timeout_ms: u64) -> Result<(), ProtocolError> {
        let mut state = self.state.borrow_mut();
        if timeout_ms == 0 {
            return Err(ProtocolError::InvalidConfig {
                field: "handshake_timeout_ms".to_string(),
                reason: "timeout must be greater than 0".to_string(),
            });
        }
        state.config.handshake_timeout_ms = timeout_ms;
        state.version += 1;
        Ok(())
    }

    /// Get the max pending handshakes.
    pub fn max_pending_handshakes(&self) -> u32 {
        let state = self.state.borrow();
        state.config.max_pending_handshakes
    }

    /// Set the max pending handshakes.
    pub fn set_max_pending_handshakes(&self, max: u32) -> Result<(), ProtocolError> {
        let mut state = self.state.borrow_mut();
        if max == 0 {
            return Err(ProtocolError::InvalidConfig {
                field: "max_pending_handshakes".to_string(),
                reason: "max must be greater than 0".to_string(),
            });
        }
        state.config.max_pending_handshakes = max;
        state.version += 1;
        Ok(())
    }

    /// Get the max capabilities.
    pub fn max_capabilities(&self) -> u32 {
        let state = self.state.borrow();
        state.config.max_capabilities
    }

    /// Set the max capabilities.
    pub fn set_max_capabilities(&self, max: u32) -> Result<(), ProtocolError> {
        let mut state = self.state.borrow_mut();
        if max == 0 {
            return Err(ProtocolError::InvalidConfig {
                field: "max_capabilities".to_string(),
                reason: "max must be greater than 0".to_string(),
            });
        }
        state.config.max_capabilities = max;
        state.version += 1;
        Ok(())
    }

    /// Get the max extensions.
    pub fn max_extensions(&self) -> u32 {
        let state = self.state.borrow();
        state.config.max_extensions
    }

    /// Set the max extensions.
    pub fn set_max_extensions(&self, max: u32) -> Result<(), ProtocolError> {
        let mut state = self.state.borrow_mut();
        if max == 0 {
            return Err(ProtocolError::InvalidConfig {
                field: "max_extensions".to_string(),
                reason: "max must be greater than 0".to_string(),
            });
        }
        state.config.max_extensions = max;
        state.version += 1;
        Ok(())
    }

    /// Check if auto-negotiation is enabled.
    pub fn is_auto_negotiation_enabled(&self) -> bool {
        let state = self.state.borrow();
        state.config.enable_auto_negotiation
    }

    /// Enable or disable auto-negotiation.
    pub fn set_auto_negotiation(&self, enabled: bool) -> Result<(), ProtocolError> {
        let mut state = self.state.borrow_mut();
        state.config.enable_auto_negotiation = enabled;
        state.version += 1;
        Ok(())
    }

    /// Check if upgrades are enabled.
    pub fn are_upgrades_enabled(&self) -> bool {
        let state = self.state.borrow();
        state.config.enable_upgrades
    }

    /// Enable or disable upgrades.
    pub fn set_upgrades_enabled(&self, enabled: bool) -> Result<(), ProtocolError> {
        let mut state = self.state.borrow_mut();
        state.config.enable_upgrades = enabled;
        state.version += 1;
        Ok(())
    }

    /// Check if downgrades are enabled.
    pub fn are_downgrades_enabled(&self) -> bool {
        let state = self.state.borrow();
        state.config.enable_downgrades
    }

    /// Enable or disable downgrades.
    pub fn set_downgrades_enabled(&self, enabled: bool) -> Result<(), ProtocolError> {
        let mut state = self.state.borrow_mut();
        state.config.enable_downgrades = enabled;
        state.version += 1;
        Ok(())
    }

    /// Check if capability negotiation is enabled.
    pub fn is_capability_negotiation_enabled(&self) -> bool {
        let state = self.state.borrow();
        state.config.enable_capability_negotiation
    }

    /// Enable or disable capability negotiation.
    pub fn set_capability_negotiation(&self, enabled: bool) -> Result<(), ProtocolError> {
        let mut state = self.state.borrow_mut();
        state.config.enable_capability_negotiation = enabled;
        state.version += 1;
        Ok(())
    }

    /// Check if strict version matching is enabled.
    pub fn is_strict_version_matching(&self) -> bool {
        let state = self.state.borrow();
        state.config.strict_version_matching
    }

    /// Enable or disable strict version matching.
    pub fn set_strict_version_matching(&self, enabled: bool) -> Result<(), ProtocolError> {
        let mut state = self.state.borrow_mut();
        state.config.strict_version_matching = enabled;
        state.version += 1;
        Ok(())
    }

    /// Get the min supported version.
    pub fn min_supported_version(&self) -> ProtocolVersion {
        let state = self.state.borrow();
        state.config.min_supported_version
    }

    /// Set the min supported version.
    pub fn set_min_supported_version(&self, version: ProtocolVersion) -> Result<(), ProtocolError> {
        let mut state = self.state.borrow_mut();
        if version.0 == 0 {
            return Err(ProtocolError::InvalidConfig {
                field: "min_supported_version".to_string(),
                reason: "version must be greater than 0".to_string(),
            });
        }
        if version > state.config.max_supported_version {
            return Err(ProtocolError::InvalidConfig {
                field: "min_supported_version".to_string(),
                reason: "min must be <= max".to_string(),
            });
        }
        state.config.min_supported_version = version;
        state.version += 1;
        Ok(())
    }

    /// Get the max supported version.
    pub fn max_supported_version(&self) -> ProtocolVersion {
        let state = self.state.borrow();
        state.config.max_supported_version
    }

    /// Set the max supported version.
    pub fn set_max_supported_version(&self, version: ProtocolVersion) -> Result<(), ProtocolError> {
        let mut state = self.state.borrow_mut();
        if version.0 == 0 {
            return Err(ProtocolError::InvalidConfig {
                field: "max_supported_version".to_string(),
                reason: "version must be greater than 0".to_string(),
            });
        }
        if version < state.config.min_supported_version {
            return Err(ProtocolError::InvalidConfig {
                field: "max_supported_version".to_string(),
                reason: "max must be >= min".to_string(),
            });
        }
        state.config.max_supported_version = version;
        state.version += 1;
        Ok(())
    }

    /// Get the current version's display string.
    pub fn current_version_display(&self) -> String {
        let state = self.state.borrow();
        state.config.current_version.display_string()
    }

    /// Get all pending handshake IDs.
    pub fn pending_handshake_ids(&self) -> Vec<u64> {
        let state = self.state.borrow();
        state
            .handshakes
            .values()
            .filter(|h| !h.state.is_terminal())
            .map(|h| h.handshake_id)
            .collect()
    }

    /// Get completed handshake IDs.
    pub fn completed_handshake_ids(&self) -> Vec<u64> {
        let state = self.state.borrow();
        state
            .handshakes
            .values()
            .filter(|h| h.state == HandshakeState::Completed)
            .map(|h| h.handshake_id)
            .collect()
    }

    /// Get failed handshake IDs.
    pub fn failed_handshake_ids(&self) -> Vec<u64> {
        let state = self.state.borrow();
        state
            .handshakes
            .values()
            .filter(|h| h.state == HandshakeState::Failed)
            .map(|h| h.handshake_id)
            .collect()
    }

    /// Cancel a pending handshake.
    pub fn cancel_handshake(&self, handshake_id: u64) -> Result<(), ProtocolError> {
        let mut state = self.state.borrow_mut();

        let handshake = state
            .handshakes
            .get_mut(&handshake_id)
            .ok_or_else(|| ProtocolError::HandshakeNotFound { handshake_id })?;

        if handshake.state.is_terminal() {
            return Err(ProtocolError::HandshakeTerminal { handshake_id });
        }

        handshake.state = HandshakeState::Failed;
        handshake.completed_at = Some(0);
        state.stats.failed_handshakes += 1;
        state.stats.pending_handshakes = state.stats.pending_handshakes.saturating_sub(1);
        state.version += 1;

        Ok(())
    }

    /// Reject a handshake.
    pub fn reject_handshake(&self, handshake_id: u64) -> Result<(), ProtocolError> {
        let mut state = self.state.borrow_mut();

        let handshake = state
            .handshakes
            .get_mut(&handshake_id)
            .ok_or_else(|| ProtocolError::HandshakeNotFound { handshake_id })?;

        if handshake.state.is_terminal() {
            return Err(ProtocolError::HandshakeTerminal { handshake_id });
        }

        handshake.state = HandshakeState::Rejected;
        handshake.completed_at = Some(0);
        state.stats.failed_handshakes += 1;
        state.stats.pending_handshakes = state.stats.pending_handshakes.saturating_sub(1);
        state.version += 1;

        Ok(())
    }

    /// Get handshake state by ID.
    pub fn handshake_state(&self, handshake_id: u64) -> Option<HandshakeState> {
        let state = self.state.borrow();
        state.handshakes.get(&handshake_id).map(|h| h.state)
    }

    /// Get peer count by version.
    pub fn peer_count_by_version(&self, version: ProtocolVersion) -> u32 {
        let state = self.state.borrow();
        state
            .peer_versions
            .values()
            .filter(|v| **v == version)
            .count() as u32
    }

    /// Get all peer versions.
    pub fn all_peer_versions(&self) -> Vec<(NodeId, ProtocolVersion)> {
        let state = self.state.borrow();
        state
            .peer_versions
            .iter()
            .map(|(peer_id, version)| (*peer_id, *version))
            .collect()
    }

    /// Remove a peer version.
    pub fn remove_peer_version(&self, peer_id: NodeId) -> Result<(), ProtocolError> {
        let mut state = self.state.borrow_mut();
        if state.peer_versions.remove(&peer_id).is_none() {
            return Err(ProtocolError::PeerNotFound { peer_id });
        }
        state.version += 1;
        Ok(())
    }

    /// Get extension count.
    pub fn extension_count(&self) -> u32 {
        let state = self.state.borrow();
        state.extensions.len() as u32
    }

    /// Get enabled extension count.
    pub fn enabled_extension_count(&self) -> u32 {
        let state = self.state.borrow();
        state
            .extensions
            .values()
            .filter(|e| e.enabled)
            .count() as u32
    }

    /// Enable an extension.
    pub fn enable_extension(&self, extension_id: ProtocolExtensionId) -> Result<(), ProtocolError> {
        let mut state = self.state.borrow_mut();
        let ext = state
            .extensions
            .get_mut(&extension_id)
            .ok_or_else(|| ProtocolError::ExtensionNotFound { extension_id })?;
        ext.enabled = true;
        state.stats.active_extensions = state
            .extensions
            .values()
            .filter(|e| e.enabled)
            .count() as u32;
        state.version += 1;
        Ok(())
    }

    /// Disable an extension.
    pub fn disable_extension(
        &self,
        extension_id: ProtocolExtensionId,
    ) -> Result<(), ProtocolError> {
        let mut state = self.state.borrow_mut();
        let ext = state
            .extensions
            .get_mut(&extension_id)
            .ok_or_else(|| ProtocolError::ExtensionNotFound { extension_id })?;
        ext.enabled = false;
        state.stats.active_extensions = state
            .extensions
            .values()
            .filter(|e| e.enabled)
            .count() as u32;
        state.version += 1;
        Ok(())
    }

    /// Remove an extension.
    pub fn remove_extension(&self, extension_id: ProtocolExtensionId) -> Result<(), ProtocolError> {
        let mut state = self.state.borrow_mut();
        if state.extensions.remove(&extension_id).is_none() {
            return Err(ProtocolError::ExtensionNotFound { extension_id });
        }
        state.stats.total_extensions = state.extensions.len() as u32;
        state.stats.active_extensions = state
            .extensions
            .values()
            .filter(|e| e.enabled)
            .count() as u32;
        state.version += 1;
        Ok(())
    }

    /// Get upgrade count by version pair.
    pub fn upgrade_count_by_pair(
        &self,
        from: ProtocolVersion,
        to: ProtocolVersion,
    ) -> u32 {
        let state = self.state.borrow();
        state
            .upgrade_history
            .iter()
            .filter(|u| u.from_version == from && u.to_version == to)
            .count() as u32
    }

    /// Get downgrade count by version pair.
    pub fn downgrade_count_by_pair(
        &self,
        from: ProtocolVersion,
        to: ProtocolVersion,
    ) -> u32 {
        let state = self.state.borrow();
        state
            .downgrade_history
            .iter()
            .filter(|d| d.from_version == from && d.to_version == to)
            .count() as u32
    }

    /// Get the latest upgrade record.
    pub fn latest_upgrade(&self) -> Option<ProtocolUpgradeRecord> {
        let state = self.state.borrow();
        state.upgrade_history.last().cloned()
    }

    /// Get the latest downgrade record.
    pub fn latest_downgrade(&self) -> Option<ProtocolDowngradeRecord> {
        let state = self.state.borrow();
        state.downgrade_history.last().cloned()
    }

    /// Clear upgrade history.
    pub fn clear_upgrade_history(&self) {
        let mut state = self.state.borrow_mut();
        state.upgrade_history.clear();
        state.upgrade_counter = 0;
        state.stats.total_upgrades = 0;
        state.stats.successful_upgrades = 0;
        state.stats.failed_upgrades = 0;
    }

    /// Clear downgrade history.
    pub fn clear_downgrade_history(&self) {
        let mut state = self.state.borrow_mut();
        state.downgrade_history.clear();
        state.downgrade_counter = 0;
        state.stats.total_downgrades = 0;
        state.stats.successful_downgrades = 0;
        state.stats.failed_downgrades = 0;
    }

    /// Clear all handshakes.
    pub fn clear_handshakes(&self) {
        let mut state = self.state.borrow_mut();
        state.handshakes.clear();
        state.handshake_counter = 0;
        state.stats.total_handshakes = 0;
        state.stats.completed_handshakes = 0;
        state.stats.failed_handshakes = 0;
        state.stats.pending_handshakes = 0;
    }

    /// Clear all peer versions.
    pub fn clear_peer_versions(&self) {
        let mut state = self.state.borrow_mut();
        state.peer_versions.clear();
    }

    /// Get protocol uptime percentage.
    pub fn protocol_uptime_percent(&self) -> f64 {
        let state = self.state.borrow();
        if state.stats.total_handshakes == 0 {
            return 100.0;
        }
        state.stats.handshake_success_rate()
    }

    /// Get protocol health score (0.0 to 1.0).
    pub fn protocol_health_score(&self) -> f64 {
        let state = self.state.borrow();
        let handshake_health = if state.stats.total_handshakes == 0 {
            1.0
        } else {
            state.stats.handshake_success_rate() / 100.0
        };
        let upgrade_health = if state.stats.total_upgrades == 0 {
            1.0
        } else {
            state.stats.upgrade_success_rate() / 100.0
        };
        let downgrade_health = if state.stats.total_downgrades == 0 {
            1.0
        } else {
            state.stats.downgrade_success_rate() / 100.0
        };
        (handshake_health + upgrade_health + downgrade_health) / 3.0
    }

    /// Clear all protocol state.
    pub fn clear_all(&self) {
        let mut state = self.state.borrow_mut();
        state.config = ProtocolConfig::default();
        state.capabilities = ProtocolCapabilities::new();
        state.extensions.clear();
        state.extension_counter = 0;
        state.handshakes.clear();
        state.handshake_counter = 0;
        state.peer_versions.clear();
        state.upgrade_history.clear();
        state.downgrade_history.clear();
        state.upgrade_counter = 0;
        state.downgrade_counter = 0;
        state.stats = ProtocolStats::new(state.config.current_version);
        state.version = 0;
    }

    /// Get the protocol graph size.
    pub fn graph_size(&self) -> (usize, usize, usize, usize) {
        let state = self.state.borrow();
        (
            state.handshakes.len(),
            state.extensions.len(),
            state.peer_versions.len(),
            state.upgrade_history.len() + state.downgrade_history.len(),
        )
    }

    /// Get the config snapshot.
    pub fn config_snapshot(&self) -> ProtocolConfig {
        let state = self.state.borrow();
        state.config.clone()
    }

    /// Check if a version is within the supported range.
    pub fn is_version_in_range(&self, version: ProtocolVersion) -> bool {
        let state = self.state.borrow();
        state.config.is_version_supported(version)
    }
}

// Add helper method for compatibility matrix
impl ProtocolState {
    fn supported_versions_internal(&self) -> Vec<ProtocolVersion> {
        let min = self.config.min_supported_version.0;
        let max = self.config.max_supported_version.0;
        (min..=max).map(ProtocolVersion).collect()
    }
}

///////////////////////////////////////////////////////////////////////////////
// TESTS
///////////////////////////////////////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use super::*;

    fn test_node_id(idx: u8) -> NodeId {
        let mut bytes = [0u8; 32];
        bytes[0] = idx;
        NodeId::new(bytes)
    }

    fn test_node_id1() -> NodeId {
        test_node_id(1)
    }

    fn test_node_id2() -> NodeId {
        test_node_id(2)
    }

    fn test_endpoint() -> Endpoint {
        Endpoint::new(CarrierType::Direct, vec![1, 2, 3, 4])
    }

    fn test_endpoint2() -> Endpoint {
        Endpoint::new(CarrierType::Direct, vec![5, 6, 7, 8])
    }

    fn test_protocol_version() -> ProtocolVersion {
        ProtocolVersion::new(1)
    }

    fn test_protocol_version2() -> ProtocolVersion {
        ProtocolVersion::new(2)
    }

    fn test_config() -> ProtocolConfig {
        ProtocolConfig::default()
    }

    // ========================================================================
    // ProtocolConfig tests
    // ========================================================================

    #[test]
    fn protocol_config_default() {
        let config = ProtocolConfig::default();
        assert_eq!(config.current_version.0, 1);
        assert_eq!(config.min_supported_version.0, 1);
        assert_eq!(config.max_supported_version.0, 100);
        assert!(config.enable_auto_negotiation);
        assert!(config.enable_upgrades);
        assert!(config.enable_downgrades);
        assert_eq!(config.negotiation_timeout_ms, 10000);
        assert_eq!(config.handshake_timeout_ms, 5000);
        assert_eq!(config.max_pending_handshakes, 100);
        assert!(config.enable_capability_negotiation);
        assert_eq!(config.max_capabilities, 50);
        assert_eq!(config.max_extensions, 20);
        assert!(!config.strict_version_matching);
    }

    #[test]
    fn protocol_config_new_valid() {
        let config = ProtocolConfig::new(ProtocolVersion::new(5)).unwrap();
        assert_eq!(config.current_version.0, 5);
    }

    #[test]
    fn protocol_config_new_zero_version() {
        let result = ProtocolConfig::new(ProtocolVersion::new(0));
        assert!(result.is_err());
    }

    #[test]
    fn protocol_config_validate_zero_current_version() {
        let mut config = ProtocolConfig::default();
        config.current_version = ProtocolVersion::new(0);
        assert!(config.validate().is_err());
    }

    #[test]
    fn protocol_config_validate_zero_min_version() {
        let mut config = ProtocolConfig::default();
        config.min_supported_version = ProtocolVersion::new(0);
        assert!(config.validate().is_err());
    }

    #[test]
    fn protocol_config_validate_zero_max_version() {
        let mut config = ProtocolConfig::default();
        config.max_supported_version = ProtocolVersion::new(0);
        assert!(config.validate().is_err());
    }

    #[test]
    fn protocol_config_validate_min_greater_than_max() {
        let mut config = ProtocolConfig::default();
        config.min_supported_version = ProtocolVersion::new(10);
        config.max_supported_version = ProtocolVersion::new(5);
        assert!(config.validate().is_err());
    }

    #[test]
    fn protocol_config_validate_current_out_of_range() {
        let mut config = ProtocolConfig::default();
        config.current_version = ProtocolVersion::new(200);
        assert!(config.validate().is_err());
    }

    #[test]
    fn protocol_config_validate_zero_negotiation_timeout() {
        let mut config = ProtocolConfig::default();
        config.negotiation_timeout_ms = 0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn protocol_config_validate_zero_handshake_timeout() {
        let mut config = ProtocolConfig::default();
        config.handshake_timeout_ms = 0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn protocol_config_validate_zero_max_pending_handshakes() {
        let mut config = ProtocolConfig::default();
        config.max_pending_handshakes = 0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn protocol_config_validate_zero_max_capabilities() {
        let mut config = ProtocolConfig::default();
        config.max_capabilities = 0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn protocol_config_validate_zero_max_extensions() {
        let mut config = ProtocolConfig::default();
        config.max_extensions = 0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn protocol_config_strict() {
        let config = ProtocolConfig::strict();
        assert_eq!(config.max_supported_version.0, 5);
        assert!(!config.enable_auto_negotiation);
        assert!(!config.enable_upgrades);
        assert!(!config.enable_downgrades);
        assert!(config.strict_version_matching);
        assert_eq!(config.max_capabilities, 10);
        assert_eq!(config.max_extensions, 5);
    }

    #[test]
    fn protocol_config_lenient() {
        let config = ProtocolConfig::lenient();
        assert_eq!(config.max_supported_version.0, 1000);
        assert!(config.enable_auto_negotiation);
        assert!(config.enable_upgrades);
        assert!(config.enable_downgrades);
        assert!(!config.strict_version_matching);
        assert_eq!(config.max_capabilities, 500);
        assert_eq!(config.max_extensions, 200);
    }

    #[test]
    fn protocol_config_version_range() {
        let config = ProtocolConfig::default();
        let (min, max) = config.version_range();
        assert_eq!(min.0, 1);
        assert_eq!(max.0, 100);
    }

    #[test]
    fn protocol_config_is_version_supported() {
        let config = ProtocolConfig::default();
        assert!(config.is_version_supported(ProtocolVersion::new(50)));
        assert!(!config.is_version_supported(ProtocolVersion::new(0)));
        assert!(!config.is_version_supported(ProtocolVersion::new(200)));
    }

    #[test]
    fn protocol_config_serialization_roundtrip() {
        let config = ProtocolConfig::default();
        let json = serde_json::to_string(&config).unwrap();
        let deserialized: ProtocolConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(config.current_version, deserialized.current_version);
    }

    // ========================================================================
    // ProtocolVersion tests
    // ========================================================================

    #[test]
    fn protocol_version_new() {
        let version = ProtocolVersion::new(42);
        assert_eq!(version.0, 42);
    }

    #[test]
    fn protocol_version_major_minor_patch() {
        let version = ProtocolVersion::new(1234);
        assert_eq!(version.major(), 1);
        assert_eq!(version.minor(), 2);
        assert_eq!(version.patch(), 34);
    }

    #[test]
    fn protocol_version_increment_major() {
        let version = ProtocolVersion::new(1000);
        let incremented = version.increment_major();
        assert_eq!(incremented.0, 2000);
    }

    #[test]
    fn protocol_version_increment_minor() {
        let version = ProtocolVersion::new(1100);
        let incremented = version.increment_minor();
        assert_eq!(incremented.0, 1200);
    }

    #[test]
    fn protocol_version_increment_patch() {
        let version = ProtocolVersion::new(1001);
        let incremented = version.increment_patch();
        assert_eq!(incremented.0, 1002);
    }

    #[test]
    fn protocol_version_display_string() {
        let version = ProtocolVersion::new(1023);
        assert_eq!(version.display_string(), "1.2.23");
    }

    #[test]
    fn protocol_version_ordering() {
        let v1 = ProtocolVersion::new(1);
        let v2 = ProtocolVersion::new(2);
        assert!(v1 < v2);
        assert!(v2 > v1);
        assert_eq!(v1, v1);
    }

    #[test]
    fn protocol_version_serialization_roundtrip() {
        let version = ProtocolVersion::new(42);
        let json = serde_json::to_string(&version).unwrap();
        let deserialized: ProtocolVersion = serde_json::from_str(&json).unwrap();
        assert_eq!(version, deserialized);
    }

    // ========================================================================
    // ProtocolCapabilities tests
    // ========================================================================

    #[test]
    fn protocol_capabilities_new() {
        let caps = ProtocolCapabilities::new();
        assert!(!caps.compression);
        assert!(!caps.encryption);
        assert!(!caps.authentication);
        assert_eq!(caps.enabled_count(), 0);
    }

    #[test]
    fn protocol_capabilities_enabled_count() {
        let mut caps = ProtocolCapabilities::new();
        caps.compression = true;
        caps.encryption = true;
        caps.authentication = true;
        assert_eq!(caps.enabled_count(), 3);
    }

    #[test]
    fn protocol_capabilities_has_capability() {
        let mut caps = ProtocolCapabilities::new();
        assert!(!caps.has_capability("compression"));
        caps.compression = true;
        assert!(caps.has_capability("compression"));
        assert!(!caps.has_capability("encryption"));
        assert!(!caps.has_capability("unknown"));
    }

    #[test]
    fn protocol_capabilities_set_capability() {
        let mut caps = ProtocolCapabilities::new();
        caps.set_capability("encryption", true);
        assert!(caps.encryption);
        caps.set_capability("encryption", false);
        assert!(!caps.encryption);
        caps.set_capability("custom_feature", true);
        assert!(caps.has_capability("custom_feature"));
    }

    #[test]
    fn protocol_capabilities_serialization_roundtrip() {
        let caps = ProtocolCapabilities::new();
        let json = serde_json::to_string(&caps).unwrap();
        let deserialized: ProtocolCapabilities = serde_json::from_str(&json).unwrap();
        assert_eq!(caps.enabled_count(), deserialized.enabled_count());
    }

    // ========================================================================
    // ProtocolCapability tests
    // ========================================================================

    #[test]
    fn protocol_capability_as_str() {
        assert_eq!(ProtocolCapability::Compression.as_str(), "compression");
        assert_eq!(ProtocolCapability::Encryption.as_str(), "encryption");
        assert_eq!(ProtocolCapability::Authentication.as_str(), "authentication");
        assert_eq!(ProtocolCapability::Custom(42).as_str(), "custom");
    }

    #[test]
    fn protocol_capability_from_str() {
        assert_eq!(
            ProtocolCapability::from_str("compression"),
            Some(ProtocolCapability::Compression)
        );
        assert_eq!(
            ProtocolCapability::from_str("encryption"),
            Some(ProtocolCapability::Encryption)
        );
        assert_eq!(ProtocolCapability::from_str("unknown"), None);
    }

    #[test]
    fn protocol_capability_serialization_roundtrip() {
        let cap = ProtocolCapability::Fragmentation;
        let json = serde_json::to_string(&cap).unwrap();
        let deserialized: ProtocolCapability = serde_json::from_str(&json).unwrap();
        assert_eq!(cap, deserialized);
    }

    // ========================================================================
    // HandshakeState tests
    // ========================================================================

    #[test]
    fn handshake_state_is_terminal() {
        assert!(HandshakeState::Completed.is_terminal());
        assert!(HandshakeState::Failed.is_terminal());
        assert!(HandshakeState::Rejected.is_terminal());
        assert!(HandshakeState::TimedOut.is_terminal());
        assert!(!HandshakeState::Initiated.is_terminal());
        assert!(!HandshakeState::OfferSent.is_terminal());
    }

    #[test]
    fn handshake_state_is_pending() {
        assert!(HandshakeState::Initiated.is_pending());
        assert!(HandshakeState::OfferSent.is_pending());
        assert!(!HandshakeState::Completed.is_pending());
    }

    #[test]
    fn handshake_state_display_name() {
        assert_eq!(HandshakeState::Initiated.display_name(), "initiated");
        assert_eq!(HandshakeState::Completed.display_name(), "completed");
        assert_eq!(HandshakeState::Failed.display_name(), "failed");
    }

    #[test]
    fn handshake_state_serialization_roundtrip() {
        let state = HandshakeState::Accepted;
        let json = serde_json::to_string(&state).unwrap();
        let deserialized: HandshakeState = serde_json::from_str(&json).unwrap();
        assert_eq!(state, deserialized);
    }

    // ========================================================================
    // ProtocolHandshakeResult tests
    // ========================================================================

    #[test]
    fn protocol_handshake_result_new() {
        let result = ProtocolHandshakeResult {
            handshake_id: 1,
            peer_id: test_node_id1(),
            negotiated_version: ProtocolVersion::new(1),
            negotiated_capabilities: ProtocolCapabilities::new(),
            completed: false,
            completed_at: 0,
        };
        assert_eq!(result.handshake_id, 1);
        assert_eq!(result.peer_id, test_node_id1());
        assert!(!result.completed);
    }

    #[test]
    fn protocol_handshake_result_serialization_roundtrip() {
        let result = ProtocolHandshakeResult {
            handshake_id: 42,
            peer_id: test_node_id2(),
            negotiated_version: ProtocolVersion::new(2),
            negotiated_capabilities: ProtocolCapabilities::new(),
            completed: true,
            completed_at: 1000,
        };
        let json = serde_json::to_string(&result).unwrap();
        let deserialized: ProtocolHandshakeResult = serde_json::from_str(&json).unwrap();
        assert_eq!(result, deserialized);
    }

    // ========================================================================
    // ProtocolHandshakeRecord tests
    // ========================================================================

    #[test]
    fn protocol_handshake_record_new() {
        let record = ProtocolHandshakeRecord {
            handshake_id: 1,
            peer_id: test_node_id1(),
            endpoint: test_endpoint(),
            offered_version: ProtocolVersion::new(1),
            negotiated_version: None,
            state: HandshakeState::Initiated,
            created_at: 1000,
            completed_at: None,
        };
        assert_eq!(record.handshake_id, 1);
        assert_eq!(record.state, HandshakeState::Initiated);
    }

    #[test]
    fn protocol_handshake_record_serialization_roundtrip() {
        let record = ProtocolHandshakeRecord {
            handshake_id: 2,
            peer_id: test_node_id2(),
            endpoint: test_endpoint2(),
            offered_version: ProtocolVersion::new(3),
            negotiated_version: Some(ProtocolVersion::new(2)),
            state: HandshakeState::Completed,
            created_at: 2000,
            completed_at: Some(3000),
        };
        let json = serde_json::to_string(&record).unwrap();
        let deserialized: ProtocolHandshakeRecord = serde_json::from_str(&json).unwrap();
        assert_eq!(record, deserialized);
    }

    // ========================================================================
    // ProtocolUpgradeResult / DowngradeResult tests
    // ========================================================================

    #[test]
    fn protocol_upgrade_result_new() {
        let result = ProtocolUpgradeResult {
            from_version: ProtocolVersion::new(1),
            to_version: ProtocolVersion::new(2),
            success: true,
            upgraded_at: 1000,
            duration_ms: 100,
        };
        assert!(result.success);
        assert_eq!(result.from_version.0, 1);
        assert_eq!(result.to_version.0, 2);
    }

    #[test]
    fn protocol_downgrade_result_new() {
        let result = ProtocolDowngradeResult {
            from_version: ProtocolVersion::new(2),
            to_version: ProtocolVersion::new(1),
            success: true,
            downgraded_at: 2000,
            duration_ms: 50,
        };
        assert!(result.success);
    }

    #[test]
    fn protocol_upgrade_record_serialization_roundtrip() {
        let record = ProtocolUpgradeRecord {
            upgrade_id: 1,
            from_version: ProtocolVersion::new(1),
            to_version: ProtocolVersion::new(2),
            success: true,
            upgraded_at: 1000,
            duration_ms: 100,
        };
        let json = serde_json::to_string(&record).unwrap();
        let deserialized: ProtocolUpgradeRecord = serde_json::from_str(&json).unwrap();
        assert_eq!(record, deserialized);
    }

    // ========================================================================
    // ProtocolExtension tests
    // ========================================================================

    #[test]
    fn protocol_extension_new() {
        let ext = ProtocolExtension::new("test_ext".to_string(), ProtocolVersion::new(1));
        assert_eq!(ext.name, "test_ext");
        assert!(ext.enabled);
    }

    #[test]
    fn protocol_extension_is_enabled() {
        let mut ext = ProtocolExtension::new("ext".to_string(), ProtocolVersion::new(1));
        assert!(ext.is_enabled());
        ext.enabled = false;
        assert!(!ext.is_enabled());
    }

    #[test]
    fn protocol_extension_serialization_roundtrip() {
        let ext = ProtocolExtension {
            extension_id: ProtocolExtensionId(1),
            name: "test".to_string(),
            version: ProtocolVersion::new(1),
            description: vec![1, 2, 3],
            enabled: true,
        };
        let json = serde_json::to_string(&ext).unwrap();
        let deserialized: ProtocolExtension = serde_json::from_str(&json).unwrap();
        assert_eq!(ext, deserialized);
    }

    // ========================================================================
    // ProtocolFeature tests
    // ========================================================================

    #[test]
    fn protocol_feature_as_str() {
        assert_eq!(ProtocolFeature::LargeMessages.as_str(), "large_messages");
        assert_eq!(ProtocolFeature::ZeroCopy.as_str(), "zero_copy");
        assert_eq!(ProtocolFeature::Backpressure.as_str(), "backpressure");
    }

    #[test]
    fn protocol_feature_serialization_roundtrip() {
        let feature = ProtocolFeature::Compression;
        let json = serde_json::to_string(&feature).unwrap();
        let deserialized: ProtocolFeature = serde_json::from_str(&json).unwrap();
        assert_eq!(feature, deserialized);
    }

    // ========================================================================
    // ProtocolCompatibilityMatrix tests
    // ========================================================================

    #[test]
    fn protocol_compatibility_matrix_new() {
        let matrix = ProtocolCompatibilityMatrix::new();
        assert!(!matrix.is_compatible(
            ProtocolVersion::new(1),
            ProtocolVersion::new(2)
        ));
    }

    #[test]
    fn protocol_compatibility_matrix_set_get() {
        let mut matrix = ProtocolCompatibilityMatrix::new();
        matrix.set_compatibility(
            ProtocolVersion::new(1),
            ProtocolVersion::new(2),
            true,
        );
        assert!(matrix.is_compatible(ProtocolVersion::new(1), ProtocolVersion::new(2)));
        assert!(matrix.is_compatible(ProtocolVersion::new(2), ProtocolVersion::new(1)));
    }

    #[test]
    fn protocol_compatibility_matrix_serialization_roundtrip() {
        let mut matrix = ProtocolCompatibilityMatrix::new();
        matrix.set_compatibility(
            ProtocolVersion::new(1),
            ProtocolVersion::new(2),
            true,
        );
        let json = serde_json::to_string(&matrix).unwrap();
        let deserialized: ProtocolCompatibilityMatrix = serde_json::from_str(&json).unwrap();
        assert!(deserialized.is_compatible(ProtocolVersion::new(1), ProtocolVersion::new(2)));
    }

    // ========================================================================
    // ProtocolStats tests
    // ========================================================================

    #[test]
    fn protocol_stats_new() {
        let stats = ProtocolStats::new(ProtocolVersion::new(1));
        assert_eq!(stats.current_version.0, 1);
        assert_eq!(stats.total_handshakes, 0);
        assert_eq!(stats.completed_handshakes, 0);
    }

    #[test]
    fn protocol_stats_handshake_success_rate() {
        let mut stats = ProtocolStats::new(ProtocolVersion::new(1));
        stats.total_handshakes = 100;
        stats.completed_handshakes = 85;
        stats.failed_handshakes = 15;
        assert!((stats.handshake_success_rate() - 85.0).abs() < 0.01);
    }

    #[test]
    fn protocol_stats_handshake_success_rate_zero() {
        let stats = ProtocolStats::new(ProtocolVersion::new(1));
        assert!((stats.handshake_success_rate() - 100.0).abs() < 0.01);
    }

    #[test]
    fn protocol_stats_upgrade_success_rate() {
        let mut stats = ProtocolStats::new(ProtocolVersion::new(1));
        stats.total_upgrades = 50;
        stats.successful_upgrades = 45;
        stats.failed_upgrades = 5;
        assert!((stats.upgrade_success_rate() - 90.0).abs() < 0.01);
    }

    #[test]
    fn protocol_stats_downgrade_success_rate() {
        let mut stats = ProtocolStats::new(ProtocolVersion::new(1));
        stats.total_downgrades = 30;
        stats.successful_downgrades = 27;
        stats.failed_downgrades = 3;
        assert!((stats.downgrade_success_rate() - 90.0).abs() < 0.01);
    }

    #[test]
    fn protocol_stats_negotiation_success_rate() {
        let mut stats = ProtocolStats::new(ProtocolVersion::new(1));
        stats.total_negotiations = 200;
        stats.successful_negotiations = 180;
        stats.failed_negotiations = 20;
        assert!((stats.negotiation_success_rate() - 90.0).abs() < 0.01);
    }

    #[test]
    fn protocol_stats_serialization_roundtrip() {
        let stats = ProtocolStats::new(ProtocolVersion::new(1));
        let json = serde_json::to_string(&stats).unwrap();
        let deserialized: ProtocolStats = serde_json::from_str(&json).unwrap();
        assert_eq!(stats.current_version, deserialized.current_version);
    }

    // ========================================================================
    // ProtocolSnapshot tests
    // ========================================================================

    #[test]
    fn protocol_snapshot_peer_count() {
        let snapshot = ProtocolSnapshot {
            version: 1,
            current_protocol_version: ProtocolVersion::new(1),
            capabilities: ProtocolCapabilities::new(),
            extensions: vec![],
            peer_versions: HashMap::new(),
            stats: ProtocolStats::new(ProtocolVersion::new(1)),
        };
        assert_eq!(snapshot.peer_count(), 0);
    }

    #[test]
    fn protocol_snapshot_extension_count() {
        let snapshot = ProtocolSnapshot {
            version: 1,
            current_protocol_version: ProtocolVersion::new(1),
            capabilities: ProtocolCapabilities::new(),
            extensions: vec![],
            peer_versions: HashMap::new(),
            stats: ProtocolStats::new(ProtocolVersion::new(1)),
        };
        assert_eq!(snapshot.extension_count(), 0);
    }

    #[test]
    fn protocol_snapshot_serialization_roundtrip() {
        let snapshot = ProtocolSnapshot {
            version: 1,
            current_protocol_version: ProtocolVersion::new(1),
            capabilities: ProtocolCapabilities::new(),
            extensions: vec![],
            peer_versions: HashMap::new(),
            stats: ProtocolStats::new(ProtocolVersion::new(1)),
        };
        let json = serde_json::to_string(&snapshot).unwrap();
        let deserialized: ProtocolSnapshot = serde_json::from_str(&json).unwrap();
        assert_eq!(snapshot.version, deserialized.version);
    }

    // ========================================================================
    // ProtocolError tests
    // ========================================================================

    #[test]
    fn protocol_error_category() {
        assert_eq!(
            ProtocolError::InvalidConfig {
                field: "x".to_string(),
                reason: "y".to_string(),
            }
            .category(),
            "invalid_config"
        );
        assert_eq!(
            ProtocolError::VersionNotSupported {
                version: ProtocolVersion::new(99),
            }
            .category(),
            "version_not_supported"
        );
    }

    #[test]
    fn protocol_error_is_recoverable() {
        assert!(ProtocolError::VersionNotSupported {
            version: ProtocolVersion::new(99),
        }
        .is_recoverable());
        assert!(ProtocolError::NegotiationTimeout {
            peer_id: test_node_id1(),
        }
        .is_recoverable());
        assert!(!ProtocolError::InvalidConfig {
            field: "x".to_string(),
            reason: "y".to_string(),
        }
        .is_recoverable());
    }

    #[test]
    fn protocol_error_suggested_action() {
        assert_eq!(
            ProtocolError::PeerNotFound {
                peer_id: test_node_id1(),
            }
            .suggested_action(),
            "register the peer first"
        );
        assert!(!ProtocolError::Internal {
            message: "test".to_string()
        }
        .suggested_action()
        .is_empty());
    }

    #[test]
    fn protocol_error_display() {
        let err = ProtocolError::VersionNotSupported {
            version: ProtocolVersion::new(99),
        };
        assert!(!err.to_string().is_empty());
    }

    #[test]
    fn protocol_error_serialization_roundtrip() {
        let err = ProtocolError::ExtensionNotFound {
            extension_id: ProtocolExtensionId(1),
        };
        let json = serde_json::to_string(&err).unwrap();
        let deserialized: ProtocolError = serde_json::from_str(&json).unwrap();
        assert!(matches!(deserialized, ProtocolError::ExtensionNotFound { .. }));
    }

    // ========================================================================
    // ProtocolEngine tests
    // ========================================================================

    #[test]
    fn protocol_engine_new() {
        let engine = ProtocolEngine::new();
        assert_eq!(engine.protocol_version().0, 1);
        assert!(engine.is_healthy());
    }

    #[test]
    fn protocol_engine_with_config() {
        let config = ProtocolConfig::new(ProtocolVersion::new(5)).unwrap();
        let engine = ProtocolEngine::with_config(config).unwrap();
        assert_eq!(engine.protocol_version().0, 5);
    }

    #[test]
    fn protocol_engine_with_config_invalid() {
        let mut config = ProtocolConfig::default();
        config.current_version = ProtocolVersion::new(0);
        let result = ProtocolEngine::with_config(config);
        assert!(result.is_err());
    }

    #[test]
    fn protocol_engine_configure_protocol() {
        let engine = ProtocolEngine::new();
        let config = ProtocolConfig::strict();
        assert!(engine.configure_protocol(config.clone()).is_ok());
        assert_eq!(engine.protocol_version().0, 1);
    }

    #[test]
    fn protocol_engine_configure_protocol_invalid() {
        let engine = ProtocolEngine::new();
        let mut config = ProtocolConfig::default();
        config.current_version = ProtocolVersion::new(0);
        let err = engine.configure_protocol(config).unwrap_err();
        assert!(matches!(err, ProtocolError::InvalidConfig { .. }));
    }

    #[test]
    fn protocol_engine_set_protocol_version() {
        let engine = ProtocolEngine::new();
        assert!(engine.set_protocol_version(ProtocolVersion::new(50)).is_ok());
        assert_eq!(engine.protocol_version().0, 50);
    }

    #[test]
    fn protocol_engine_set_protocol_version_out_of_range() {
        let engine = ProtocolEngine::new();
        let err = engine
            .set_protocol_version(ProtocolVersion::new(200))
            .unwrap_err();
        assert!(matches!(err, ProtocolError::VersionOutOfRange { .. }));
    }

    #[test]
    fn protocol_engine_supported_versions() {
        let engine = ProtocolEngine::new();
        let versions = engine.supported_versions();
        assert_eq!(versions[0], ProtocolVersion::new(1));
        assert_eq!(versions.last(), Some(&ProtocolVersion::new(100)));
    }

    #[test]
    fn protocol_engine_negotiate_version() {
        let engine = ProtocolEngine::new();
        let negotiated = engine
            .negotiate_version(
                test_node_id1(),
                vec![ProtocolVersion::new(1), ProtocolVersion::new(2)],
            )
            .unwrap();
        assert_eq!(negotiated, ProtocolVersion::new(2));
        assert_eq!(
            engine.peer_version(test_node_id1()),
            Some(ProtocolVersion::new(2))
        );
    }

    #[test]
    fn protocol_engine_negotiate_version_incompatible() {
        let engine = ProtocolEngine::new();
        let mut config = ProtocolConfig::default();
        config.max_supported_version = ProtocolVersion::new(5);
        let engine2 = ProtocolEngine::with_config(config).unwrap();
        let err = engine2
            .negotiate_version(
                test_node_id1(),
                vec![ProtocolVersion::new(100)],
            )
            .unwrap_err();
        assert!(matches!(err, ProtocolError::IncompatibleVersions { .. }));
    }

    #[test]
    fn protocol_engine_capabilities() {
        let engine = ProtocolEngine::new();
        let caps = engine.capabilities();
        assert_eq!(caps.enabled_count(), 0);
    }

    #[test]
    fn protocol_engine_add_capability() {
        let engine = ProtocolEngine::new();
        assert!(engine.add_capability(ProtocolCapability::Compression).is_ok());
        assert!(engine.has_capability(ProtocolCapability::Compression));
        assert!(!engine.has_capability(ProtocolCapability::Encryption));
    }

    #[test]
    fn protocol_engine_remove_capability() {
        let engine = ProtocolEngine::new();
        engine.add_capability(ProtocolCapability::Compression).unwrap();
        assert!(engine.remove_capability(ProtocolCapability::Compression).is_ok());
        assert!(!engine.has_capability(ProtocolCapability::Compression));
    }

    #[test]
    fn protocol_engine_max_capabilities_exceeded() {
        let engine = ProtocolEngine::new();
        let mut config = ProtocolConfig::default();
        config.max_capabilities = 1;
        engine.configure_protocol(config).unwrap();
        engine.add_capability(ProtocolCapability::Compression).unwrap();
        let err = engine
            .add_capability(ProtocolCapability::Encryption)
            .unwrap_err();
        assert!(matches!(err, ProtocolError::MaxCapabilitiesExceeded { .. }));
    }

    #[test]
    fn protocol_engine_handshake() {
        let engine = ProtocolEngine::new();
        let result = engine.handshake(test_node_id1(), test_endpoint()).unwrap();
        assert_eq!(result.handshake_id, 1);
        assert_eq!(result.peer_id, test_node_id1());
        assert!(!result.completed);
    }

    #[test]
    fn protocol_engine_complete_handshake() {
        let engine = ProtocolEngine::new();
        let result = engine.handshake(test_node_id1(), test_endpoint()).unwrap();
        let completed = engine
            .complete_handshake(test_node_id1(), result.handshake_id)
            .unwrap();
        assert!(completed.completed);
    }

    #[test]
    fn protocol_engine_complete_nonexistent_handshake() {
        let engine = ProtocolEngine::new();
        let err = engine
            .complete_handshake(test_node_id1(), 999)
            .unwrap_err();
        assert!(matches!(err, ProtocolError::HandshakeNotFound { .. }));
    }

    #[test]
    fn protocol_engine_pending_handshakes() {
        let engine = ProtocolEngine::new();
        engine.handshake(test_node_id1(), test_endpoint()).unwrap();
        engine.handshake(test_node_id2(), test_endpoint2()).unwrap();
        let pending = engine.pending_handshakes();
        assert_eq!(pending.len(), 2);
    }

    #[test]
    fn protocol_engine_max_pending_handshakes_exceeded() {
        let engine = ProtocolEngine::new();
        let mut config = ProtocolConfig::default();
        config.max_pending_handshakes = 1;
        engine.configure_protocol(config).unwrap();
        engine.handshake(test_node_id1(), test_endpoint()).unwrap();
        let err = engine.handshake(test_node_id2(), test_endpoint2()).unwrap_err();
        assert!(matches!(
            err,
            ProtocolError::MaxPendingHandshakesExceeded { .. }
        ));
    }

    #[test]
    fn protocol_engine_upgrade_protocol() {
        let engine = ProtocolEngine::new();
        let result = engine.upgrade_protocol(ProtocolVersion::new(50)).unwrap();
        assert!(result.success);
        assert_eq!(result.to_version.0, 50);
        assert_eq!(engine.protocol_version().0, 50);
    }

    #[test]
    fn protocol_engine_upgrade_not_supported() {
        let engine = ProtocolEngine::new();
        let mut config = ProtocolConfig::default();
        config.enable_upgrades = false;
        engine.configure_protocol(config).unwrap();
        let err = engine
            .upgrade_protocol(ProtocolVersion::new(50))
            .unwrap_err();
        assert!(matches!(err, ProtocolError::UpgradeNotSupported { .. }));
    }

    #[test]
    fn protocol_engine_downgrade_protocol() {
        let engine = ProtocolEngine::new();
        engine.set_protocol_version(ProtocolVersion::new(50)).unwrap();
        let result = engine.downgrade_protocol(ProtocolVersion::new(10)).unwrap();
        assert!(result.success);
        assert_eq!(result.to_version.0, 10);
    }

    #[test]
    fn protocol_engine_upgrade_history() {
        let engine = ProtocolEngine::new();
        engine.upgrade_protocol(ProtocolVersion::new(10)).unwrap();
        engine.upgrade_protocol(ProtocolVersion::new(20)).unwrap();
        let history = engine.upgrade_history();
        assert_eq!(history.len(), 2);
    }

    #[test]
    fn protocol_engine_downgrade_history() {
        let engine = ProtocolEngine::new();
        engine.set_protocol_version(ProtocolVersion::new(50)).unwrap();
        engine.downgrade_protocol(ProtocolVersion::new(10)).unwrap();
        let history = engine.downgrade_history();
        assert_eq!(history.len(), 1);
    }

    #[test]
    fn protocol_engine_register_extension() {
        let engine = ProtocolEngine::new();
        let ext = ProtocolExtension::new("test".to_string(), ProtocolVersion::new(1));
        let ext_id = engine.register_extension(ext).unwrap();
        assert_eq!(ext_id.0, 1);
        assert!(engine.extension_by_id(ext_id).is_some());
    }

    #[test]
    fn protocol_engine_register_extension_max_exceeded() {
        let engine = ProtocolEngine::new();
        let mut config = ProtocolConfig::default();
        config.max_extensions = 1;
        engine.configure_protocol(config).unwrap();
        engine
            .register_extension(ProtocolExtension::new("a".to_string(), ProtocolVersion::new(1)))
            .unwrap();
        let err = engine
            .register_extension(ProtocolExtension::new("b".to_string(), ProtocolVersion::new(1)))
            .unwrap_err();
        assert!(matches!(err, ProtocolError::MaxExtensionsExceeded { .. }));
    }

    #[test]
    fn protocol_engine_registered_extensions() {
        let engine = ProtocolEngine::new();
        engine
            .register_extension(ProtocolExtension::new("a".to_string(), ProtocolVersion::new(1)))
            .unwrap();
        engine
            .register_extension(ProtocolExtension::new("b".to_string(), ProtocolVersion::new(1)))
            .unwrap();
        let extensions = engine.registered_extensions();
        assert_eq!(extensions.len(), 2);
    }

    #[test]
    fn protocol_engine_extension_not_found() {
        let engine = ProtocolEngine::new();
        assert!(engine.extension_by_id(ProtocolExtensionId(999)).is_none());
    }

    #[test]
    fn protocol_engine_is_compatible() {
        let engine = ProtocolEngine::new();
        assert!(engine.is_compatible(ProtocolVersion::new(50)));
        assert!(!engine.is_compatible(ProtocolVersion::new(0)));
    }

    #[test]
    fn protocol_engine_compatibility_matrix() {
        let engine = ProtocolEngine::new();
        let matrix = engine.compatibility_matrix();
        assert!(matrix.is_compatible(ProtocolVersion::new(1), ProtocolVersion::new(2)));
    }

    #[test]
    fn protocol_engine_negotiate_features() {
        let engine = ProtocolEngine::new();
        engine.negotiate_version(test_node_id1(), vec![ProtocolVersion::new(1)]).unwrap();
        let features = engine
            .negotiate_features(
                test_node_id1(),
                vec![ProtocolFeature::Compression, ProtocolFeature::Encryption],
            )
            .unwrap();
        assert_eq!(features.len(), 2);
    }

    #[test]
    fn protocol_engine_negotiate_features_unknown_peer() {
        let engine = ProtocolEngine::new();
        let err = engine
            .negotiate_features(
                test_node_id1(),
                vec![ProtocolFeature::Compression],
            )
            .unwrap_err();
        assert!(matches!(err, ProtocolError::PeerNotFound { .. }));
    }

    #[test]
    fn protocol_engine_stats() {
        let engine = ProtocolEngine::new();
        engine.handshake(test_node_id1(), test_endpoint()).unwrap();
        engine.complete_handshake(test_node_id1(), 1).unwrap();
        let stats = engine.stats();
        assert_eq!(stats.total_handshakes, 1);
        assert_eq!(stats.completed_handshakes, 1);
    }

    #[test]
    fn protocol_engine_is_healthy() {
        let engine = ProtocolEngine::new();
        assert!(engine.is_healthy());
    }

    #[test]
    fn protocol_engine_validate_config() {
        let engine = ProtocolEngine::new();
        let config = ProtocolConfig::default();
        assert!(engine.validate_config(&config).is_ok());
    }

    #[test]
    fn protocol_engine_snapshot_restore() {
        let engine = ProtocolEngine::new();
        engine.add_capability(ProtocolCapability::Compression).unwrap();
        engine.register_extension(ProtocolExtension::new("ext".to_string(), ProtocolVersion::new(1))).unwrap();

        let snapshot = engine.snapshot();
        assert_eq!(snapshot.extension_count(), 1);

        let engine2 = ProtocolEngine::new();
        engine2.restore_from_snapshot(snapshot).unwrap();
        assert_eq!(engine2.extension_count(), 1);
        assert!(engine2.has_capability(ProtocolCapability::Compression));
    }

    #[test]
    fn protocol_engine_peers_by_version() {
        let engine = ProtocolEngine::new();
        engine.negotiate_version(test_node_id1(), vec![ProtocolVersion::new(1)]).unwrap();
        engine.negotiate_version(test_node_id2(), vec![ProtocolVersion::new(2)]).unwrap();
        let peers = engine.peers_by_version(ProtocolVersion::new(1));
        assert_eq!(peers.len(), 1);
        assert_eq!(peers[0], test_node_id1());
    }

    #[test]
    fn protocol_engine_peer_version() {
        let engine = ProtocolEngine::new();
        assert!(engine.peer_version(test_node_id1()).is_none());
        engine.negotiate_version(test_node_id1(), vec![ProtocolVersion::new(5)]).unwrap();
        assert_eq!(engine.peer_version(test_node_id1()), Some(ProtocolVersion::new(5)));
    }

    #[test]
    fn protocol_engine_set_peer_version() {
        let engine = ProtocolEngine::new();
        assert!(engine.set_peer_version(test_node_id1(), ProtocolVersion::new(3)).is_ok());
        assert_eq!(engine.peer_version(test_node_id1()), Some(ProtocolVersion::new(3)));
    }

    #[test]
    fn protocol_engine_set_peer_version_unsupported() {
        let engine = ProtocolEngine::new();
        let err = engine
            .set_peer_version(test_node_id1(), ProtocolVersion::new(0))
            .unwrap_err();
        assert!(matches!(err, ProtocolError::VersionNotSupported { .. }));
    }

    #[test]
    fn protocol_engine_negotiation_timeout() {
        let engine = ProtocolEngine::new();
        assert_eq!(engine.negotiation_timeout_ms(), 10000);
        engine.set_negotiation_timeout_ms(20000).unwrap();
        assert_eq!(engine.negotiation_timeout_ms(), 20000);
    }

    #[test]
    fn protocol_engine_handshake_timeout() {
        let engine = ProtocolEngine::new();
        assert_eq!(engine.handshake_timeout_ms(), 5000);
        engine.set_handshake_timeout_ms(10000).unwrap();
        assert_eq!(engine.handshake_timeout_ms(), 10000);
    }

    // ========================================================================
    // ProtocolEngine advanced methods tests
    // ========================================================================

    #[test]
    fn protocol_engine_max_pending_handshakes_get_set() {
        let engine = ProtocolEngine::new();
        assert_eq!(engine.max_pending_handshakes(), 100);
        engine.set_max_pending_handshakes(50).unwrap();
        assert_eq!(engine.max_pending_handshakes(), 50);
    }

    #[test]
    fn protocol_engine_max_capabilities_get_set() {
        let engine = ProtocolEngine::new();
        assert_eq!(engine.max_capabilities(), 50);
        engine.set_max_capabilities(25).unwrap();
        assert_eq!(engine.max_capabilities(), 25);
    }

    #[test]
    fn protocol_engine_max_extensions_get_set() {
        let engine = ProtocolEngine::new();
        assert_eq!(engine.max_extensions(), 20);
        engine.set_max_extensions(10).unwrap();
        assert_eq!(engine.max_extensions(), 10);
    }

    #[test]
    fn protocol_engine_auto_negotiation() {
        let engine = ProtocolEngine::new();
        assert!(engine.is_auto_negotiation_enabled());
        engine.set_auto_negotiation(false).unwrap();
        assert!(!engine.is_auto_negotiation_enabled());
    }

    #[test]
    fn protocol_engine_upgrades_enabled() {
        let engine = ProtocolEngine::new();
        assert!(engine.are_upgrades_enabled());
        engine.set_upgrades_enabled(false).unwrap();
        assert!(!engine.are_upgrades_enabled());
    }

    #[test]
    fn protocol_engine_downgrades_enabled() {
        let engine = ProtocolEngine::new();
        assert!(engine.are_downgrades_enabled());
        engine.set_downgrades_enabled(false).unwrap();
        assert!(!engine.are_downgrades_enabled());
    }

    #[test]
    fn protocol_engine_capability_negotiation() {
        let engine = ProtocolEngine::new();
        assert!(engine.is_capability_negotiation_enabled());
        engine.set_capability_negotiation(false).unwrap();
        assert!(!engine.is_capability_negotiation_enabled());
    }

    #[test]
    fn protocol_engine_strict_version_matching() {
        let engine = ProtocolEngine::new();
        assert!(!engine.is_strict_version_matching());
        engine.set_strict_version_matching(true).unwrap();
        assert!(engine.is_strict_version_matching());
    }

    #[test]
    fn protocol_engine_min_max_supported_version() {
        let engine = ProtocolEngine::new();
        assert_eq!(engine.min_supported_version().0, 1);
        assert_eq!(engine.max_supported_version().0, 100);
        engine.set_min_supported_version(ProtocolVersion::new(10)).unwrap();
        assert_eq!(engine.min_supported_version().0, 10);
        engine.set_max_supported_version(ProtocolVersion::new(200)).unwrap();
        assert_eq!(engine.max_supported_version().0, 200);
    }

    #[test]
    fn protocol_engine_current_version_display() {
        let engine = ProtocolEngine::new();
        assert_eq!(engine.current_version_display(), "1.0.0");
        engine.set_protocol_version(ProtocolVersion::new(1023)).unwrap();
        assert_eq!(engine.current_version_display(), "1.2.23");
    }

    #[test]
    fn protocol_engine_pending_handshake_ids() {
        let engine = ProtocolEngine::new();
        engine.handshake(test_node_id1(), test_endpoint()).unwrap();
        engine.handshake(test_node_id2(), test_endpoint2()).unwrap();
        let ids = engine.pending_handshake_ids();
        assert_eq!(ids.len(), 2);
    }

    #[test]
    fn protocol_engine_cancel_handshake() {
        let engine = ProtocolEngine::new();
        let result = engine.handshake(test_node_id1(), test_endpoint()).unwrap();
        assert!(engine.cancel_handshake(result.handshake_id).is_ok());
        assert_eq!(
            engine.handshake_state(result.handshake_id),
            Some(HandshakeState::Failed)
        );
    }

    #[test]
    fn protocol_engine_reject_handshake() {
        let engine = ProtocolEngine::new();
        let result = engine.handshake(test_node_id1(), test_endpoint()).unwrap();
        assert!(engine.reject_handshake(result.handshake_id).is_ok());
        assert_eq!(
            engine.handshake_state(result.handshake_id),
            Some(HandshakeState::Rejected)
        );
    }

    #[test]
    fn protocol_engine_handshake_state() {
        let engine = ProtocolEngine::new();
        let result = engine.handshake(test_node_id1(), test_endpoint()).unwrap();
        assert_eq!(
            engine.handshake_state(result.handshake_id),
            Some(HandshakeState::Initiated)
        );
        assert_eq!(engine.handshake_state(999), None);
    }

    #[test]
    fn protocol_engine_peer_count_by_version() {
        let engine = ProtocolEngine::new();
        engine.negotiate_version(test_node_id1(), vec![ProtocolVersion::new(1)]).unwrap();
        engine.negotiate_version(test_node_id2(), vec![ProtocolVersion::new(1)]).unwrap();
        assert_eq!(engine.peer_count_by_version(ProtocolVersion::new(1)), 2);
    }

    #[test]
    fn protocol_engine_all_peer_versions() {
        let engine = ProtocolEngine::new();
        engine.negotiate_version(test_node_id1(), vec![ProtocolVersion::new(1)]).unwrap();
        engine.negotiate_version(test_node_id2(), vec![ProtocolVersion::new(2)]).unwrap();
        let versions = engine.all_peer_versions();
        assert_eq!(versions.len(), 2);
    }

    #[test]
    fn protocol_engine_remove_peer_version() {
        let engine = ProtocolEngine::new();
        engine.negotiate_version(test_node_id1(), vec![ProtocolVersion::new(1)]).unwrap();
        assert!(engine.remove_peer_version(test_node_id1()).is_ok());
        assert!(engine.peer_version(test_node_id1()).is_none());
    }

    #[test]
    fn protocol_engine_remove_peer_version_not_found() {
        let engine = ProtocolEngine::new();
        let err = engine.remove_peer_version(test_node_id1()).unwrap_err();
        assert!(matches!(err, ProtocolError::PeerNotFound { .. }));
    }

    #[test]
    fn protocol_engine_extension_count() {
        let engine = ProtocolEngine::new();
        assert_eq!(engine.extension_count(), 0);
        engine
            .register_extension(ProtocolExtension::new("a".to_string(), ProtocolVersion::new(1)))
            .unwrap();
        assert_eq!(engine.extension_count(), 1);
    }

    #[test]
    fn protocol_engine_enable_disable_extension() {
        let engine = ProtocolEngine::new();
        let ext_id = engine
            .register_extension(ProtocolExtension::new("a".to_string(), ProtocolVersion::new(1)))
            .unwrap();
        assert!(engine.disable_extension(ext_id).is_ok());
        assert!(!engine.extension_by_id(ext_id).unwrap().enabled);
        assert!(engine.enable_extension(ext_id).is_ok());
        assert!(engine.extension_by_id(ext_id).unwrap().enabled);
    }

    #[test]
    fn protocol_engine_remove_extension() {
        let engine = ProtocolEngine::new();
        let ext_id = engine
            .register_extension(ProtocolExtension::new("a".to_string(), ProtocolVersion::new(1)))
            .unwrap();
        assert!(engine.remove_extension(ext_id).is_ok());
        assert!(engine.extension_by_id(ext_id).is_none());
    }

    #[test]
    fn protocol_engine_upgrade_count_by_pair() {
        let engine = ProtocolEngine::new();
        engine.upgrade_protocol(ProtocolVersion::new(10)).unwrap();
        assert_eq!(
            engine.upgrade_count_by_pair(ProtocolVersion::new(1), ProtocolVersion::new(10)),
            1
        );
    }

    #[test]
    fn protocol_engine_downgrade_count_by_pair() {
        let engine = ProtocolEngine::new();
        engine.set_protocol_version(ProtocolVersion::new(10)).unwrap();
        engine.downgrade_protocol(ProtocolVersion::new(5)).unwrap();
        assert_eq!(
            engine.downgrade_count_by_pair(ProtocolVersion::new(10), ProtocolVersion::new(5)),
            1
        );
    }

    #[test]
    fn protocol_engine_latest_upgrade_downgrade() {
        let engine = ProtocolEngine::new();
        assert!(engine.latest_upgrade().is_none());
        assert!(engine.latest_downgrade().is_none());
        engine.upgrade_protocol(ProtocolVersion::new(10)).unwrap();
        assert!(engine.latest_upgrade().is_some());
    }

    #[test]
    fn protocol_engine_clear_upgrade_downgrade_history() {
        let engine = ProtocolEngine::new();
        engine.upgrade_protocol(ProtocolVersion::new(10)).unwrap();
        engine.clear_upgrade_history();
        assert!(engine.latest_upgrade().is_none());
        engine.downgrade_protocol(ProtocolVersion::new(5)).unwrap();
        engine.clear_downgrade_history();
        assert!(engine.latest_downgrade().is_none());
    }

    #[test]
    fn protocol_engine_clear_handshakes() {
        let engine = ProtocolEngine::new();
        engine.handshake(test_node_id1(), test_endpoint()).unwrap();
        engine.clear_handshakes();
        assert_eq!(engine.pending_handshakes().len(), 0);
    }

    #[test]
    fn protocol_engine_clear_peer_versions() {
        let engine = ProtocolEngine::new();
        engine.negotiate_version(test_node_id1(), vec![ProtocolVersion::new(1)]).unwrap();
        engine.clear_peer_versions();
        assert!(engine.peer_version(test_node_id1()).is_none());
    }

    #[test]
    fn protocol_engine_protocol_uptime_percent() {
        let engine = ProtocolEngine::new();
        assert!((engine.protocol_uptime_percent() - 100.0).abs() < 0.01);
    }

    #[test]
    fn protocol_engine_protocol_health_score() {
        let engine = ProtocolEngine::new();
        let score = engine.protocol_health_score();
        assert!(score > 0.0);
        assert!(score <= 1.0);
    }

    #[test]
    fn protocol_engine_clear_all() {
        let engine = ProtocolEngine::new();
        engine.add_capability(ProtocolCapability::Compression).unwrap();
        engine.set_protocol_version(ProtocolVersion::new(50)).unwrap();
        engine.clear_all();
        assert_eq!(engine.protocol_version().0, 1);
        assert!(!engine.has_capability(ProtocolCapability::Compression));
    }

    #[test]
    fn protocol_engine_graph_size() {
        let engine = ProtocolEngine::new();
        engine.handshake(test_node_id1(), test_endpoint()).unwrap();
        engine.register_extension(ProtocolExtension::new("a".to_string(), ProtocolVersion::new(1))).unwrap();
        let (handshakes, extensions, peers, upgrades) = engine.graph_size();
        assert_eq!(handshakes, 1);
        assert_eq!(extensions, 1);
        assert_eq!(peers, 0);
    }

    #[test]
    fn protocol_engine_config_snapshot() {
        let engine = ProtocolEngine::new();
        let config = engine.config_snapshot();
        assert_eq!(config.current_version.0, 1);
    }

    #[test]
    fn protocol_engine_is_version_in_range() {
        let engine = ProtocolEngine::new();
        assert!(engine.is_version_in_range(ProtocolVersion::new(50)));
        assert!(!engine.is_version_in_range(ProtocolVersion::new(200)));
    }

    // ========================================================================
    // Serialization roundtrip tests
    // ========================================================================

    #[test]
    fn protocol_serialization_roundtrip_protocol_config() {
        let config = ProtocolConfig::default();
        let json = serde_json::to_string(&config).unwrap();
        let deserialized: ProtocolConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(config.current_version, deserialized.current_version);
    }

    #[test]
    fn protocol_serialization_roundtrip_protocol_stats() {
        let stats = ProtocolStats::new(ProtocolVersion::new(1));
        let json = serde_json::to_string(&stats).unwrap();
        let deserialized: ProtocolStats = serde_json::from_str(&json).unwrap();
        assert_eq!(stats.current_version, deserialized.current_version);
    }

    // ========================================================================
    // Integration tests
    // ========================================================================

    #[test]
    fn protocol_full_lifecycle() {
        let engine = ProtocolEngine::new();
        let config = ProtocolConfig::strict();
        engine.configure_protocol(config).unwrap();

        // Add capabilities
        engine.add_capability(ProtocolCapability::Compression).unwrap();
        engine.add_capability(ProtocolCapability::Encryption).unwrap();

        // Register extensions
        engine
            .register_extension(ProtocolExtension::new("ext1".to_string(), ProtocolVersion::new(1)))
            .unwrap();

        // Negotiate version with peer
        engine
            .negotiate_version(test_node_id1(), vec![ProtocolVersion::new(1), ProtocolVersion::new(2)])
            .unwrap();

        // Perform handshake
        let handshake = engine.handshake(test_node_id1(), test_endpoint()).unwrap();
        engine.complete_handshake(test_node_id1(), handshake.handshake_id).unwrap();

        // Upgrade protocol
        engine.upgrade_protocol(ProtocolVersion::new(2)).unwrap();

        // Check stats
        let stats = engine.stats();
        assert_eq!(stats.total_handshakes, 1);
        assert_eq!(stats.completed_handshakes, 1);

        // Check health
        let score = engine.protocol_health_score();
        assert!(score > 0.0);

        // Snapshot and restore
        let snapshot = engine.snapshot();
        let engine2 = ProtocolEngine::new();
        engine2.restore_from_snapshot(snapshot).unwrap();
        assert_eq!(engine2.protocol_version().0, 2);
        assert!(engine2.has_capability(ProtocolCapability::Compression));
    }

    #[test]
    fn protocol_multi_peer_negotiation() {
        let engine = ProtocolEngine::new();
        engine.negotiate_version(test_node_id1(), vec![ProtocolVersion::new(1)]).unwrap();
        engine.negotiate_version(test_node_id2(), vec![ProtocolVersion::new(2)]).unwrap();

        assert_eq!(engine.peer_version(test_node_id1()), Some(ProtocolVersion::new(1)));
        assert_eq!(engine.peer_version(test_node_id2()), Some(ProtocolVersion::new(2)));
        assert_eq!(engine.peer_count_by_version(ProtocolVersion::new(1)), 1);
        assert_eq!(engine.peer_count_by_version(ProtocolVersion::new(2)), 1);
    }

    #[test]
    fn protocol_extension_lifecycle() {
        let engine = ProtocolEngine::new();
        let ext_id = engine
            .register_extension(ProtocolExtension::new("test".to_string(), ProtocolVersion::new(1)))
            .unwrap();
        assert!(engine.extension_by_id(ext_id).unwrap().enabled);
        engine.disable_extension(ext_id).unwrap();
        assert!(!engine.extension_by_id(ext_id).unwrap().enabled);
        engine.enable_extension(ext_id).unwrap();
        assert!(engine.extension_by_id(ext_id).unwrap().enabled);
        engine.remove_extension(ext_id).unwrap();
        assert!(engine.extension_by_id(ext_id).is_none());
    }
}
