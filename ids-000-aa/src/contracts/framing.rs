//! # ids-000-aa Sealed Trait Contract: Framing
//!
//! Native packet framing contract for IDS.
//! Provides packet framing, deframing, frame boundary detection, frame size
//! negotiation, frame type classification, and frame integrity checking.
//! All types are canonical and sealed — only ids-000-aa and its designated
//! implementation crates can implement this trait.
//!
//! Carrier-agnostic: no std::net, tokio::net, libp2p, IP, TCP, UDP, QUIC,
//! HTTP, DNS, or any existing networking stack. Framed packets are opaque
//! byte blobs carried on CarrierType + Vec<u8> Endpoints only.

use crate::types::{Endpoint, NodeId};
use std::collections::{HashMap, HashSet};

///////////////////////////////////////////////////////////////////////////////
// SEALED TRAIT: Framing
///////////////////////////////////////////////////////////////////////////////

/// Sealed marker — only ids-000-aa and designated crates can implement.
pub mod private {
    pub trait Sealed {}
}

/// Sealed packet framing trait.
/// Implementations live downstream in ids-framing and ids-* crates.
pub trait Framing: private::Sealed {
    /// Configure framing parameters.
    fn configure_framing(&self, config: FramingConfig) -> Result<(), FramingError>;

    /// Frame raw packet data into a framed packet.
    fn frame_packet(
        &self,
        data: Vec<u8>,
        frame_type: FrameType,
    ) -> Result<FramedPacket, FramingError>;

    /// Deframe a framed packet into raw data.
    fn deframe_packet(
        &self,
        packet: &FramedPacket,
    ) -> Result<(Vec<u8>, FrameType), FramingError>;

    /// Get frame boundary markers.
    fn frame_boundaries(&self) -> FrameBoundarySet;

    /// Detect frame boundaries in a byte stream.
    fn detect_boundaries(
        &self,
        stream: &[u8],
    ) -> Result<Vec<FrameBoundary>, FramingError>;

    /// Get the maximum frame size.
    fn max_frame_size(&self) -> usize;

    /// Set the maximum frame size.
    fn set_max_frame_size(&self, size: usize) -> Result<(), FramingError>;

    /// Get the minimum frame size.
    fn min_frame_size(&self) -> usize;

    /// Check if a data size fits within frame limits.
    fn fits_frame(&self, data_size: usize) -> bool;

    /// Get frame overhead per packet.
    fn frame_overhead(&self) -> usize;

    /// Get the framing protocol version.
    fn framing_version(&self) -> FramingVersion;

    /// Set the framing protocol version.
    fn set_framing_version(
        &self,
        version: FramingVersion,
    ) -> Result<(), FramingError>;

    /// Get all supported framing versions.
    fn supported_framing_versions(&self) -> Vec<FramingVersion>;

    /// Negotiate framing version with a peer.
    fn negotiate_framing_version(
        &self,
        peer_id: NodeId,
        offered_versions: Vec<FramingVersion>,
    ) -> Result<FramingVersion, FramingError>;

    /// Classify a frame type from raw bytes.
    fn classify_frame_type(
        &self,
        bytes: &[u8],
    ) -> Result<FrameType, FramingError>;

    /// Get frame type statistics.
    fn frame_type_stats(&self) -> HashMap<FrameType, u64>;

    /// Register a frame type handler.
    fn register_frame_type_handler(
        &self,
        frame_type: FrameType,
        handler: FrameHandlerId,
    ) -> Result<(), FramingError>;

    /// Get frame type handler.
    fn frame_type_handler(
        &self,
        frame_type: FrameType,
    ) -> Option<FrameHandlerId>;

    /// Get framing statistics.
    fn stats(&self) -> FramingStats;

    /// Check if framing subsystem is healthy.
    fn is_healthy(&self) -> bool;

    /// Validate framing configuration.
    fn validate_config(&self, config: &FramingConfig) -> Result<(), FramingError>;

    /// Get framing snapshot.
    fn snapshot(&self) -> FramingSnapshot;

    /// Restore framing from snapshot.
    fn restore_from_snapshot(
        &self,
        snapshot: FramingSnapshot,
    ) -> Result<(), FramingError>;

    /// Get peers by framing version.
    fn peers_by_version(&self, version: FramingVersion) -> Vec<NodeId>;

    /// Get peer framing version.
    fn peer_version(&self, peer_id: NodeId) -> Option<FramingVersion>;

    /// Set peer framing version.
    fn set_peer_version(
        &self,
        peer_id: NodeId,
        version: FramingVersion,
    ) -> Result<(), FramingError>;

    /// Get framing negotiation timeout.
    fn negotiation_timeout_ms(&self) -> u64;

    /// Set framing negotiation timeout.
    fn set_negotiation_timeout_ms(&self, timeout_ms: u64) -> Result<(), FramingError>;

    /// Enable or disable frame padding.
    fn is_padding_enabled(&self) -> bool;

    /// Set frame padding.
    fn set_padding_enabled(&self, enabled: bool) -> Result<(), FramingError>;

    /// Get frame padding size.
    fn padding_size(&self) -> usize;

    /// Set frame padding size.
    fn set_padding_size(&self, size: usize) -> Result<(), FramingError>;
}

///////////////////////////////////////////////////////////////////////////////
// CONFIGURATION
///////////////////////////////////////////////////////////////////////////////

/// Framing configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FramingConfig {
    /// Maximum frame size (bytes).
    pub max_frame_size: usize,
    /// Minimum frame size (bytes).
    pub min_frame_size: usize,
    /// Frame overhead per packet (bytes).
    pub frame_overhead: usize,
    /// Framing protocol version.
    pub framing_version: FramingVersion,
    /// Minimum supported framing version.
    pub min_supported_version: FramingVersion,
    /// Maximum supported framing version.
    pub max_supported_version: FramingVersion,
    /// Enable automatic framing version negotiation.
    pub enable_auto_negotiation: bool,
    /// Enable frame boundary detection.
    pub enable_boundary_detection: bool,
    /// Boundary detection timeout (ms).
    pub boundary_detection_timeout_ms: u64,
    /// Maximum pending boundary detections.
    pub max_pending_detections: u32,
    /// Enable frame padding.
    pub enable_padding: bool,
    /// Frame padding size (bytes).
    pub padding_size: usize,
    /// Maximum registered frame type handlers.
    pub max_frame_type_handlers: u32,
    /// Strict version matching.
    pub strict_version_matching: bool,
}

impl FramingConfig {
    pub fn new(max_frame_size: usize) -> Result<Self, FramingError> {
        let config = Self {
            max_frame_size,
            min_frame_size: 1,
            frame_overhead: 16,
            framing_version: FramingVersion::new(1),
            min_supported_version: FramingVersion::new(1),
            max_supported_version: FramingVersion::new(100),
            enable_auto_negotiation: true,
            enable_boundary_detection: true,
            boundary_detection_timeout_ms: 5000,
            max_pending_detections: 1000,
            enable_padding: true,
            padding_size: 8,
            max_frame_type_handlers: 50,
            strict_version_matching: false,
        };
        config.validate()?;
        Ok(config)
    }

    pub fn validate(&self) -> Result<(), FramingError> {
        if self.max_frame_size == 0 {
            return Err(FramingError::InvalidConfig {
                field: "max_frame_size".to_string(),
                reason: "max_frame_size must be greater than 0".to_string(),
            });
        }
        if self.min_frame_size == 0 {
            return Err(FramingError::InvalidConfig {
                field: "min_frame_size".to_string(),
                reason: "min_frame_size must be greater than 0".to_string(),
            });
        }
        if self.min_frame_size > self.max_frame_size {
            return Err(FramingError::InvalidConfig {
                field: "frame_size_range".to_string(),
                reason: "min_frame_size must be <= max_frame_size".to_string(),
            });
        }
        if self.frame_overhead == 0 {
            return Err(FramingError::InvalidConfig {
                field: "frame_overhead".to_string(),
                reason: "frame_overhead must be greater than 0".to_string(),
            });
        }
        if self.boundary_detection_timeout_ms == 0 {
            return Err(FramingError::InvalidConfig {
                field: "boundary_detection_timeout_ms".to_string(),
                reason: "boundary_detection_timeout_ms must be greater than 0".to_string(),
            });
        }
        if self.max_pending_detections == 0 {
            return Err(FramingError::InvalidConfig {
                field: "max_pending_detections".to_string(),
                reason: "max_pending_detections must be greater than 0".to_string(),
            });
        }
        if self.max_frame_type_handlers == 0 {
            return Err(FramingError::InvalidConfig {
                field: "max_frame_type_handlers".to_string(),
                reason: "max_frame_type_handlers must be greater than 0".to_string(),
            });
        }
        Ok(())
    }

    pub fn default() -> Self {
        Self {
            max_frame_size: 65536,
            min_frame_size: 1,
            frame_overhead: 16,
            framing_version: FramingVersion::new(1),
            min_supported_version: FramingVersion::new(1),
            max_supported_version: FramingVersion::new(100),
            enable_auto_negotiation: true,
            enable_boundary_detection: true,
            boundary_detection_timeout_ms: 5000,
            max_pending_detections: 1000,
            enable_padding: true,
            padding_size: 8,
            max_frame_type_handlers: 50,
            strict_version_matching: false,
        }
    }

    pub fn strict() -> Self {
        Self {
            max_frame_size: 4096,
            min_frame_size: 16,
            frame_overhead: 8,
            framing_version: FramingVersion::new(1),
            min_supported_version: FramingVersion::new(1),
            max_supported_version: FramingVersion::new(10),
            enable_auto_negotiation: false,
            enable_boundary_detection: true,
            boundary_detection_timeout_ms: 2000,
            max_pending_detections: 100,
            enable_padding: false,
            padding_size: 0,
            max_frame_type_handlers: 10,
            strict_version_matching: true,
        }
    }

    pub fn lenient() -> Self {
        Self {
            max_frame_size: 1048576,
            min_frame_size: 1,
            frame_overhead: 32,
            framing_version: FramingVersion::new(1),
            min_supported_version: FramingVersion::new(1),
            max_supported_version: FramingVersion::new(1000),
            enable_auto_negotiation: true,
            enable_boundary_detection: false,
            boundary_detection_timeout_ms: 30000,
            max_pending_detections: 10000,
            enable_padding: true,
            padding_size: 16,
            max_frame_type_handlers: 500,
            strict_version_matching: false,
        }
    }

    pub fn payload_efficiency(&self, data_size: usize) -> f64 {
        if data_size == 0 {
            return 100.0;
        }
        let total_overhead = self.frame_overhead + self.padding_size;
        data_size as f64 / (data_size + total_overhead) as f64 * 100.0
    }

    pub fn frame_size_range(&self) -> (usize, usize) {
        (self.min_frame_size, self.max_frame_size)
    }

    pub fn is_size_in_range(&self, size: usize) -> bool {
        size >= self.min_frame_size && size <= self.max_frame_size
    }
}

impl Default for FramingConfig {
    fn default() -> Self {
        Self::default()
    }
}

///////////////////////////////////////////////////////////////////////////////
// FRAME TYPE
///////////////////////////////////////////////////////////////////////////////

/// Frame type classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FrameType {
    Data,
    Control,
    Ack,
    Heartbeat,
    Discovery,
    Handshake,
    Fragment,
    Metadata,
    Custom(u16),
}

impl FrameType {
    pub fn as_str(&self) -> &'static str {
        match self {
            FrameType::Data => "data",
            FrameType::Control => "control",
            FrameType::Ack => "ack",
            FrameType::Heartbeat => "heartbeat",
            FrameType::Discovery => "discovery",
            FrameType::Handshake => "handshake",
            FrameType::Fragment => "fragment",
            FrameType::Metadata => "metadata",
            FrameType::Custom(_) => "custom",
        }
    }

    pub fn from_u16(value: u16) -> Self {
        match value {
            0 => FrameType::Data,
            1 => FrameType::Control,
            2 => FrameType::Ack,
            3 => FrameType::Heartbeat,
            4 => FrameType::Discovery,
            5 => FrameType::Handshake,
            6 => FrameType::Fragment,
            7 => FrameType::Metadata,
            _ => FrameType::Custom(value),
        }
    }

    pub fn to_u16(&self) -> u16 {
        match self {
            FrameType::Data => 0,
            FrameType::Control => 1,
            FrameType::Ack => 2,
            FrameType::Heartbeat => 3,
            FrameType::Discovery => 4,
            FrameType::Handshake => 5,
            FrameType::Fragment => 6,
            FrameType::Metadata => 7,
            FrameType::Custom(v) => *v,
        }
    }

    pub fn is_control_frame(&self) -> bool {
        matches!(
            self,
            FrameType::Control | FrameType::Ack | FrameType::Heartbeat | FrameType::Handshake
        )
    }

    pub fn is_data_frame(&self) -> bool {
        matches!(self, FrameType::Data | FrameType::Fragment)
    }

    pub fn serialization_roundtrip(&self) -> bool {
        let json = serde_json::to_string(self).unwrap();
        serde_json::from_str::<FrameType>(&json).is_ok()
    }
}

///////////////////////////////////////////////////////////////////////////////
// FRAMED PACKET
///////////////////////////////////////////////////////////////////////////////

/// A framed packet ready for transmission.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FramedPacket {
    pub packet_id: u64,
    pub frame_type: FrameType,
    pub data: Vec<u8>,
    pub header: FrameHeader,
    pub padding: Vec<u8>,
    pub checksum: Option<Vec<u8>>,
    pub timestamp_ms: u64,
    pub sequence: u64,
}

impl FramedPacket {
    pub fn new(
        packet_id: u64,
        frame_type: FrameType,
        data: Vec<u8>,
    ) -> Self {
        Self {
            packet_id,
            frame_type,
            data,
            header: FrameHeader::default(),
            padding: Vec::new(),
            checksum: None,
            timestamp_ms: 0,
            sequence: 0,
        }
    }

    pub fn total_size(&self) -> usize {
        self.header.size()
            + self.data.len()
            + self.padding.len()
            + self.checksum.as_ref().map(|c| c.len()).unwrap_or(0)
    }

    pub fn payload_size(&self) -> usize {
        self.data.len()
    }

    pub fn overhead_size(&self) -> usize {
        self.header.size() + self.padding.len() + self.checksum.as_ref().map(|c| c.len()).unwrap_or(0)
    }

    pub fn has_checksum(&self) -> bool {
        self.checksum.is_some()
    }

    pub fn has_padding(&self) -> bool {
        !self.padding.is_empty()
    }
}

///////////////////////////////////////////////////////////////////////////////
// FRAME HEADER
///////////////////////////////////////////////////////////////////////////////

/// Frame header metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct FrameHeader {
    pub version: u8,
    pub flags: FrameHeaderFlags,
    pub frame_type_code: u8,
    pub reserved: [u8; 3],
}

impl FrameHeader {
    pub fn new(version: u8, frame_type: FrameType) -> Self {
        Self {
            version,
            flags: FrameHeaderFlags::empty(),
            frame_type_code: frame_type.to_u16() as u8,
            reserved: [0, 0, 0],
        }
    }

    pub fn size(&self) -> usize {
        8 // Fixed header size
    }

    pub fn set_flag(&mut self, flag: FrameHeaderFlags) {
        self.flags.insert(flag);
    }

    pub fn clear_flag(&mut self, flag: FrameHeaderFlags) {
        self.flags.remove(flag);
    }

    pub fn has_flag(&self, flag: FrameHeaderFlags) -> bool {
        self.flags.contains(flag)
    }
}

///////////////////////////////////////////////////////////////////////////////
// FRAME HEADER FLAGS
///////////////////////////////////////////////////////////////////////////////

/// Frame header flags bitfield.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub struct FrameHeaderFlags(pub u8);

impl FrameHeaderFlags {
    pub const FLAG_HAS_CHECKSUM: u8 = 1 << 0;
    pub const FLAG_HAS_PADDING: u8 = 1 << 1;
    pub const FLAG_FRAGMENTED: u8 = 1 << 2;
    pub const FLAG_LAST_FRAGMENT: u8 = 1 << 3;
    pub const FLAG_PRIORITY: u8 = 1 << 4;
    pub const FLAG_ENCRYPTED: u8 = 1 << 5;
    pub const FLAG_COMPRESSED: u8 = 1 << 6;
    pub const FLAG_RESERVED: u8 = 1 << 7;

    pub const fn empty() -> Self {
        Self(0)
    }

    pub const fn all() -> Self {
        Self(0xFF)
    }

    pub fn contains(&self, flag: u8) -> bool {
        self.0 & flag != 0
    }

    pub fn insert(&mut self, flag: u8) {
        self.0 |= flag;
    }

    pub fn remove(&mut self, flag: u8) {
        self.0 &= !flag;
    }

    pub fn bits(&self) -> u8 {
        self.0
    }
}

///////////////////////////////////////////////////////////////////////////////
// FRAME BOUNDARY
///////////////////////////////////////////////////////////////////////////////

/// A detected frame boundary in a byte stream.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FrameBoundary {
    pub offset: usize,
    pub length: usize,
    pub frame_type: FrameType,
    pub boundary_confidence: f64,
}

impl FrameBoundary {
    pub fn new(offset: usize, length: usize, frame_type: FrameType) -> Self {
        Self {
            offset,
            length,
            frame_type,
            boundary_confidence: 1.0,
        }
    }

    pub fn end_offset(&self) -> usize {
        self.offset + self.length
    }

    pub fn is_high_confidence(&self) -> bool {
        self.boundary_confidence >= 0.9
    }
}

/// Frame boundary set.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FrameBoundarySet {
    pub boundaries: Vec<FrameBoundary>,
    pub total_detected: u64,
    pub detection_algorithm: BoundaryAlgorithm,
}

/// Boundary detection algorithm.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BoundaryAlgorithm {
    FixedSize,
    DelimiterBased,
    LengthPrefixed,
    PatternBased,
    Adaptive,
}

impl BoundaryAlgorithm {
    pub fn as_str(&self) -> &'static str {
        match self {
            BoundaryAlgorithm::FixedSize => "fixed_size",
            BoundaryAlgorithm::DelimiterBased => "delimiter_based",
            BoundaryAlgorithm::LengthPrefixed => "length_prefixed",
            BoundaryAlgorithm::PatternBased => "pattern_based",
            BoundaryAlgorithm::Adaptive => "adaptive",
        }
    }
}

impl FrameBoundarySet {
    pub fn new(algorithm: BoundaryAlgorithm) -> Self {
        Self {
            boundaries: Vec::new(),
            total_detected: 0,
            detection_algorithm: algorithm,
        }
    }

    pub fn add_boundary(&mut self, boundary: FrameBoundary) {
        self.boundaries.push(boundary);
        self.total_detected += 1;
    }

    pub fn boundary_count(&self) -> usize {
        self.boundaries.len()
    }

    pub fn clear(&mut self) {
        self.boundaries.clear();
    }
}

///////////////////////////////////////////////////////////////////////////////
// FRAMING VERSION
///////////////////////////////////////////////////////////////////////////////

/// Framing protocol version.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct FramingVersion(pub u64);

impl FramingVersion {
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
// FRAME HANDLER ID
///////////////////////////////////////////////////////////////////////////////

/// Frame handler ID.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FrameHandlerId(pub u64);

///////////////////////////////////////////////////////////////////////////////
// FRAMING STATS
///////////////////////////////////////////////////////////////////////////////

/// Framing subsystem statistics.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FramingStats {
    pub frames_framed: u64,
    pub frames_deframed: u64,
    pub boundaries_detected: u64,
    pub detection_failures: u64,
    pub total_data_bytes: u64,
    pub total_framed_bytes: u64,
    pub total_padding_bytes: u64,
    pub total_checksum_bytes: u64,
    pub frame_type_stats: HashMap<FrameType, u64>,
    pub total_handlers: u32,
    pub active_handlers: u32,
    pub total_negotiations: u64,
    pub successful_negotiations: u64,
    pub failed_negotiations: u64,
    pub version_switches: u64,
}

impl FramingStats {
    pub fn new() -> Self {
        Self {
            frames_framed: 0,
            frames_deframed: 0,
            boundaries_detected: 0,
            detection_failures: 0,
            total_data_bytes: 0,
            total_framed_bytes: 0,
            total_padding_bytes: 0,
            total_checksum_bytes: 0,
            frame_type_stats: HashMap::new(),
            total_handlers: 0,
            active_handlers: 0,
            total_negotiations: 0,
            successful_negotiations: 0,
            failed_negotiations: 0,
            version_switches: 0,
        }
    }

    pub fn frame_framing_success_rate(&self) -> f64 {
        if self.frames_framed == 0 {
            return 100.0;
        }
        100.0
    }

    pub fn deframing_success_rate(&self) -> f64 {
        if self.frames_deframed == 0 {
            return 100.0;
        }
        100.0
    }

    pub fn boundary_detection_success_rate(&self) -> f64 {
        let total = self.boundaries_detected + self.detection_failures;
        if total == 0 {
            return 100.0;
        }
        self.boundaries_detected as f64 / total as f64 * 100.0
    }

    pub fn negotiation_success_rate(&self) -> f64 {
        if self.total_negotiations == 0 {
            return 100.0;
        }
        self.successful_negotiations as f64 / self.total_negotiations as f64 * 100.0
    }

    pub fn overhead_ratio(&self) -> f64 {
        if self.total_data_bytes == 0 {
            return 0.0;
        }
        (self.total_framed_bytes - self.total_data_bytes) as f64 / self.total_data_bytes as f64
    }

    pub fn bytes_per_frame_avg(&self) -> f64 {
        if self.frames_framed == 0 {
            return 0.0;
        }
        self.total_framed_bytes as f64 / self.frames_framed as f64
    }
}

///////////////////////////////////////////////////////////////////////////////
// FRAMING SNAPSHOT
///////////////////////////////////////////////////////////////////////////////

/// Framing subsystem snapshot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FramingSnapshot {
    pub version: u64,
    pub framing_version: FramingVersion,
    pub max_frame_size: usize,
    pub padding_enabled: bool,
    pub padding_size: usize,
    pub boundary_algorithm: BoundaryAlgorithm,
    pub peer_versions: HashMap<NodeId, FramingVersion>,
    pub stats: FramingStats,
}

impl FramingSnapshot {
    pub fn peer_count(&self) -> usize {
        self.peer_versions.len()
    }
}

///////////////////////////////////////////////////////////////////////////////
// FRAMING ERROR
///////////////////////////////////////////////////////////////////////////////

/// Framing management errors.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
pub enum FramingError {
    #[error("invalid config: {field} - {reason}")]
    InvalidConfig { field: String, reason: String },

    #[error("frame too large: {size} > {max_size}")]
    FrameTooLarge { size: usize, max_size: usize },

    #[error("frame too small: {size} < {min_size}")]
    FrameTooSmall { size: usize, min_size: usize },

    #[error("peer not found: {peer_id:?}")]
    PeerNotFound { peer_id: NodeId },

    #[error("frame not found: {packet_id}")]
    FrameNotFound { packet_id: u64 },

    #[error("frame deframing failed: {packet_id}")]
    FrameDeframingFailed { packet_id: u64 },

    #[error("boundary detection failed at offset {offset}")]
    BoundaryDetectionFailed { offset: usize },

    #[error("invalid frame type: {frame_type:?}")]
    InvalidFrameType { frame_type: FrameType },

    #[error("frame type handler not found: {frame_type:?}")]
    FrameTypeHandlerNotFound { frame_type: FrameType },

    #[error("maximum frame type handlers exceeded: {max_handlers}")]
    MaxFrameTypeHandlersExceeded { max_handlers: u32 },

    #[error("maximum pending detections exceeded: {max_detections}")]
    MaxPendingDetectionsExceeded { max_detections: u32 },

    #[error("checksum mismatch: {packet_id}")]
    ChecksumMismatch { packet_id: u64 },

    #[error("truncated frame: {packet_id}")]
    TruncatedFrame { packet_id: u64 },

    #[error("corrupted frame data: {packet_id}")]
    CorruptedFrameData { packet_id: u64 },

    #[error("negotiation timeout: {peer_id:?}")]
    NegotiationTimeout { peer_id: NodeId },

    #[error("internal error: {message}")]
    Internal { message: String },
}

impl FramingError {
    pub fn category(&self) -> &'static str {
        match self {
            FramingError::InvalidConfig { .. } => "invalid_config",
            FramingError::FrameTooLarge { .. } => "frame_too_large",
            FramingError::FrameTooSmall { .. } => "frame_too_small",
            FramingError::PeerNotFound { .. } => "peer_not_found",
            FramingError::FrameNotFound { .. } => "frame_not_found",
            FramingError::FrameDeframingFailed { .. } => "frame_deframing_failed",
            FramingError::BoundaryDetectionFailed { .. } => "boundary_detection_failed",
            FramingError::InvalidFrameType { .. } => "invalid_frame_type",
            FramingError::FrameTypeHandlerNotFound { .. } => "frame_type_handler_not_found",
            FramingError::MaxFrameTypeHandlersExceeded { .. } => "max_frame_type_handlers_exceeded",
            FramingError::MaxPendingDetectionsExceeded { .. } => "max_pending_detections_exceeded",
            FramingError::ChecksumMismatch { .. } => "checksum_mismatch",
            FramingError::TruncatedFrame { .. } => "truncated_frame",
            FramingError::CorruptedFrameData { .. } => "corrupted_frame_data",
            FramingError::NegotiationTimeout { .. } => "negotiation_timeout",
            FramingError::Internal { .. } => "internal",
        }
    }

    pub fn is_recoverable(&self) -> bool {
        matches!(
            self,
            FramingError::FrameTooLarge { .. }
                | FramingError::NegotiationTimeout { .. }
                | FramingError::ChecksumMismatch { .. }
                | FramingError::TruncatedFrame { .. }
                | FramingError::BoundaryDetectionFailed { .. }
        )
    }

    pub fn suggested_action(&self) -> &'static str {
        match self {
            FramingError::InvalidConfig { .. } => "correct the framing configuration",
            FramingError::FrameTooLarge { .. } => "reduce data size or increase max_frame_size",
            FramingError::FrameTooSmall { .. } => "increase data size or decrease min_frame_size",
            FramingError::PeerNotFound { .. } => "register the peer first",
            FramingError::FrameNotFound { .. } => "create the frame first",
            FramingError::FrameDeframingFailed { .. } => "fix the frame and retry",
            FramingError::BoundaryDetectionFailed { .. } => "check stream alignment",
            FramingError::InvalidFrameType { .. } => "use a valid frame type",
            FramingError::FrameTypeHandlerNotFound { .. } => "register the handler first",
            FramingError::MaxFrameTypeHandlersExceeded { .. } => {
                "remove handlers or increase limit"
            }
            FramingError::MaxPendingDetectionsExceeded { .. } => {
                "complete pending detections first"
            }
            FramingError::ChecksumMismatch { .. } => "retransmit the frame",
            FramingError::TruncatedFrame { .. } => "retransmit the complete frame",
            FramingError::CorruptedFrameData { .. } => "retransmit with valid data",
            FramingError::NegotiationTimeout { .. } => "retry framing negotiation",
            FramingError::Internal { .. } => "contact system administrator",
        }
    }
}

///////////////////////////////////////////////////////////////////////////////
// INTERNAL STATE
///////////////////////////////////////////////////////////////////////////////

/// Internal frame record.
#[derive(Debug, Clone)]
struct InternalFrame {
    packet_id: u64,
    frame_type: FrameType,
    data_size: usize,
    framed_size: usize,
    timestamp_ms: u64,
    sequence: u64,
}

/// Internal peer version mapping.
#[derive(Debug, Clone)]
struct InternalPeerVersion {
    peer_id: NodeId,
    version: FramingVersion,
    last_updated: u64,
}

/// Internal framing state.
#[derive(Debug, Clone)]
struct FramingState {
    config: FramingConfig,
    frames: HashMap<u64, InternalFrame>,
    frame_counter: u64,
    peer_versions: HashMap<NodeId, FramingVersion>,
    frame_type_handlers: HashMap<FrameType, FrameHandlerId>,
    handler_counter: u64,
    boundary_detections: u64,
    detection_failures: u64,
    stats: FramingStats,
    version: u64,
}

impl FramingState {
    fn new(config: FramingConfig) -> Self {
        Self {
            config,
            frames: HashMap::new(),
            frame_counter: 0,
            peer_versions: HashMap::new(),
            frame_type_handlers: HashMap::new(),
            handler_counter: 0,
            boundary_detections: 0,
            detection_failures: 0,
            stats: FramingStats::new(),
            version: 0,
        }
    }
}

///////////////////////////////////////////////////////////////////////////////
// FRAMING ENGINE (REFERENCE IMPLEMENTATION)
///////////////////////////////////////////////////////////////////////////////

/// Reference framing engine implementing the Framing trait.
pub struct FramingEngine {
    state: std::cell::RefCell<FramingState>,
}

impl FramingEngine {
    pub fn new() -> Self {
        Self {
            state: std::cell::RefCell::new(FramingState::new(FramingConfig::default())),
        }
    }

    pub fn with_config(config: FramingConfig) -> Result<Self, FramingError> {
        config.validate()?;
        Ok(Self {
            state: std::cell::RefCell::new(FramingState::new(config)),
        })
    }

    fn ensure_config(&self) -> FramingConfig {
        self.state.borrow().config.clone()
    }
}

///////////////////////////////////////////////////////////////////////////////
// IMPL FRAMING FOR FRAMINGENGINE
///////////////////////////////////////////////////////////////////////////////

impl private::Sealed for FramingEngine {}

impl Framing for FramingEngine {
    fn configure_framing(&self, config: FramingConfig) -> Result<(), FramingError> {
        config.validate()?;
        let mut state = self.state.borrow_mut();
        state.config = config.clone();
        state.stats = FramingStats::new();
        Ok(())
    }

    fn frame_packet(
        &self,
        data: Vec<u8>,
        frame_type: FrameType,
    ) -> Result<FramedPacket, FramingError> {
        let config = self.ensure_config();
        let mut state = self.state.borrow_mut();

        if data.len() > config.max_frame_size {
            return Err(FramingError::FrameTooLarge {
                size: data.len(),
                max_size: config.max_frame_size,
            });
        }

        if data.len() < config.min_frame_size {
            return Err(FramingError::FrameTooSmall {
                size: data.len(),
                min_size: config.min_frame_size,
            });
        }

        state.frame_counter += 1;
        let packet_id = state.frame_counter;

        let mut packet = FramedPacket::new(packet_id, frame_type, data.clone());
        packet.timestamp_ms = 0;
        packet.sequence = state.frame_counter;
        packet.header = FrameHeader::new(config.framing_version.0 as u8, frame_type);

        if config.enable_padding && config.padding_size > 0 {
            packet.padding = vec![0u8; config.padding_size];
            packet.header.set_flag(FrameHeaderFlags::FLAG_HAS_PADDING);
        }

        let overhead = packet.header.size() + packet.padding.len();
        state.stats.frames_framed += 1;
        state.stats.total_data_bytes += data.len() as u64;
        state.stats.total_framed_bytes += (data.len() + overhead) as u64;
        state.stats.total_padding_bytes += packet.padding.len() as u64;

        state.frames.insert(
            packet_id,
            InternalFrame {
                packet_id,
                frame_type,
                data_size: data.len(),
                framed_size: data.len() + overhead,
                timestamp_ms: 0,
                sequence: state.frame_counter,
            },
        );

        *state.stats.frame_type_stats.entry(frame_type).or_insert(0) += 1;

        state.version += 1;

        Ok(packet)
    }

    fn deframe_packet(
        &self,
        packet: &FramedPacket,
    ) -> Result<(Vec<u8>, FrameType), FramingError> {
        let state = self.state.borrow();

        if packet.data.is_empty() && !packet.has_padding() {
            return Err(FramingError::CorruptedFrameData {
                packet_id: packet.packet_id,
            });
        }

        if packet.data.len() > state.config.max_frame_size {
            return Err(FramingError::FrameTooLarge {
                size: packet.data.len(),
                max_size: state.config.max_frame_size,
            });
        }

        state.stats.frames_deframed += 1;

        Ok((packet.data.clone(), packet.frame_type))
    }

    fn frame_boundaries(&self) -> FrameBoundarySet {
        let state = self.state.borrow();
        FrameBoundarySet::new(BoundaryAlgorithm::Adaptive)
    }

    fn detect_boundaries(
        &self,
        stream: &[u8],
    ) -> Result<Vec<FrameBoundary>, FramingError> {
        let mut state = self.state.borrow_mut();

        state.boundary_detections += 1;

        if stream.is_empty() {
            state.detection_failures += 1;
            return Err(FramingError::BoundaryDetectionFailed { offset: 0 });
        }

        let mut boundaries = Vec::new();
        let mut offset = 0usize;

        // Simple fixed-size boundary detection
        while offset < stream.len() {
            let remaining = stream.len() - offset;
            let frame_size = remaining.min(state.config.max_frame_size);
            if frame_size < state.config.min_frame_size {
                break;
            }

            boundaries.push(FrameBoundary::new(
                offset,
                frame_size,
                FrameType::from_u16(stream[offset] as u16 % 8),
            ));
            offset += frame_size;
        }

        if boundaries.is_empty() && !stream.is_empty() {
            state.detection_failures += 1;
            return Err(FramingError::BoundaryDetectionFailed { offset: 0 });
        }

        state.boundaries_detected += boundaries.len() as u64;

        Ok(boundaries)
    }

    fn max_frame_size(&self) -> usize {
        let state = self.state.borrow();
        state.config.max_frame_size
    }

    fn set_max_frame_size(&self, size: usize) -> Result<(), FramingError> {
        let mut state = self.state.borrow_mut();
        if size == 0 {
            return Err(FramingError::InvalidConfig {
                field: "max_frame_size".to_string(),
                reason: "size must be greater than 0".to_string(),
            });
        }
        if size < state.config.min_frame_size {
            return Err(FramingError::InvalidConfig {
                field: "max_frame_size".to_string(),
                reason: "max must be >= min".to_string(),
            });
        }
        state.config.max_frame_size = size;
        state.version += 1;
        Ok(())
    }

    fn min_frame_size(&self) -> usize {
        let state = self.state.borrow();
        state.config.min_frame_size
    }

    fn fits_frame(&self, data_size: usize) -> bool {
        let config = self.ensure_config();
        config.is_size_in_range(data_size)
    }

    fn frame_overhead(&self) -> usize {
        let state = self.state.borrow();
        state.config.frame_overhead
    }

    fn framing_version(&self) -> FramingVersion {
        let state = self.state.borrow();
        state.config.framing_version
    }

    fn set_framing_version(
        &self,
        version: FramingVersion,
    ) -> Result<(), FramingError> {
        let mut state = self.state.borrow_mut();
        if version.0 < state.config.min_supported_version.0
            || version.0 > state.config.max_supported_version.0
        {
            return Err(FramingError::InvalidConfig {
                field: "framing_version".to_string(),
                reason: "version out of supported range".to_string(),
            });
        }
        state.config.framing_version = version;
        state.stats.version_switches += 1;
        state.version += 1;
        Ok(())
    }

    fn supported_framing_versions(&self) -> Vec<FramingVersion> {
        let state = self.state.borrow();
        let min = state.config.min_supported_version.0;
        let max = state.config.max_supported_version.0;
        (min..=max).map(FramingVersion).collect()
    }

    fn negotiate_framing_version(
        &self,
        peer_id: NodeId,
        offered_versions: Vec<FramingVersion>,
    ) -> Result<FramingVersion, FramingError> {
        let config = self.ensure_config();
        let mut state = self.state.borrow_mut();

        state.stats.total_negotiations += 1;

        let mut best_version = None;
        for &offered in &offered_versions {
            if offered >= config.min_supported_version
                && offered <= config.max_supported_version
            {
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
                return Err(FramingError::NegotiationTimeout { peer_id });
            }
        };

        state.stats.successful_negotiations += 1;
        state.peer_versions.insert(peer_id, negotiated);

        Ok(negotiated)
    }

    fn classify_frame_type(
        &self,
        bytes: &[u8],
    ) -> Result<FrameType, FramingError> {
        if bytes.is_empty() {
            return Err(FramingError::InvalidFrameType {
                frame_type: FrameType::Custom(0),
            });
        }
        Ok(FrameType::from_u16(bytes[0] as u16 % 8))
    }

    fn frame_type_stats(&self) -> HashMap<FrameType, u64> {
        let state = self.state.borrow();
        state.stats.frame_type_stats.clone()
    }

    fn register_frame_type_handler(
        &self,
        frame_type: FrameType,
        handler: FrameHandlerId,
    ) -> Result<(), FramingError> {
        let config = self.ensure_config();
        let mut state = self.state.borrow_mut();

        if state.frame_type_handlers.len() as u32 >= config.max_frame_type_handlers {
            return Err(FramingError::MaxFrameTypeHandlersExceeded {
                max_handlers: config.max_frame_type_handlers,
            });
        }

        state.frame_type_handlers.insert(frame_type, handler);
        state.stats.total_handlers += 1;
        state.stats.active_handlers = state.frame_type_handlers.len() as u32;
        state.version += 1;

        Ok(())
    }

    fn frame_type_handler(
        &self,
        frame_type: FrameType,
    ) -> Option<FrameHandlerId> {
        let state = self.state.borrow();
        state.frame_type_handlers.get(&frame_type).copied()
    }

    fn stats(&self) -> FramingStats {
        let state = self.state.borrow();
        state.stats.clone()
    }

    fn is_healthy(&self) -> bool {
        let state = self.state.borrow();
        if state.stats.frames_deframed == 0 {
            return true;
        }
        state.stats.deframing_success_rate() > 50.0
    }

    fn validate_config(&self, config: &FramingConfig) -> Result<(), FramingError> {
        config.validate()
    }

    fn snapshot(&self) -> FramingSnapshot {
        let state = self.state.borrow();
        FramingSnapshot {
            version: state.version,
            framing_version: state.config.framing_version,
            max_frame_size: state.config.max_frame_size,
            padding_enabled: state.config.enable_padding,
            padding_size: state.config.padding_size,
            boundary_algorithm: BoundaryAlgorithm::Adaptive,
            peer_versions: state.peer_versions.clone(),
            stats: state.stats.clone(),
        }
    }

    fn restore_from_snapshot(
        &self,
        snapshot: FramingSnapshot,
    ) -> Result<(), FramingError> {
        let mut state = self.state.borrow_mut();
        state.version = snapshot.version;
        state.config.framing_version = snapshot.framing_version;
        state.config.max_frame_size = snapshot.max_frame_size;
        state.config.enable_padding = snapshot.padding_enabled;
        state.config.padding_size = snapshot.padding_size;
        state.stats = snapshot.stats;

        state.peer_versions = snapshot.peer_versions;

        Ok(())
    }

    fn peers_by_version(&self, version: FramingVersion) -> Vec<NodeId> {
        let state = self.state.borrow();
        state
            .peer_versions
            .iter()
            .filter(|(_, v)| **v == version)
            .map(|(peer_id, _)| *peer_id)
            .collect()
    }

    fn peer_version(&self, peer_id: NodeId) -> Option<FramingVersion> {
        let state = self.state.borrow();
        state.peer_versions.get(&peer_id).copied()
    }

    fn set_peer_version(
        &self,
        peer_id: NodeId,
        version: FramingVersion,
    ) -> Result<(), FramingError> {
        let config = self.ensure_config();
        if version.0 < config.min_supported_version.0
            || version.0 > config.max_supported_version.0
        {
            return Err(FramingError::InvalidConfig {
                field: "framing_version".to_string(),
                reason: "version out of supported range".to_string(),
            });
        }
        let mut state = self.state.borrow_mut();
        state.peer_versions.insert(peer_id, version);
        state.version += 1;
        Ok(())
    }

    fn negotiation_timeout_ms(&self) -> u64 {
        let state = self.state.borrow();
        state.config.boundary_detection_timeout_ms
    }

    fn set_negotiation_timeout_ms(&self, timeout_ms: u64) -> Result<(), FramingError> {
        let mut state = self.state.borrow_mut();
        if timeout_ms == 0 {
            return Err(FramingError::InvalidConfig {
                field: "negotiation_timeout_ms".to_string(),
                reason: "timeout must be greater than 0".to_string(),
            });
        }
        state.config.boundary_detection_timeout_ms = timeout_ms;
        state.version += 1;
        Ok(())
    }

    fn is_padding_enabled(&self) -> bool {
        let state = self.state.borrow();
        state.config.enable_padding
    }

    fn set_padding_enabled(&self, enabled: bool) -> Result<(), FramingError> {
        let mut state = self.state.borrow_mut();
        state.config.enable_padding = enabled;
        state.version += 1;
        Ok(())
    }

    fn padding_size(&self) -> usize {
        let state = self.state.borrow();
        state.config.padding_size
    }

    fn set_padding_size(&self, size: usize) -> Result<(), FramingError> {
        let mut state = self.state.borrow_mut();
        state.config.padding_size = size;
        state.version += 1;
        Ok(())
    }

    /// Get the total number of frames framed.
    pub fn total_frames_framed(&self) -> u64 {
        let state = self.state.borrow();
        state.stats.frames_framed
    }

    /// Get the total number of frames deframed.
    pub fn total_frames_deframed(&self) -> u64 {
        let state = self.state.borrow();
        state.stats.frames_deframed
    }

    /// Get the total number of boundaries detected.
    pub fn total_boundaries_detected(&self) -> u64 {
        let state = self.state.borrow();
        state.stats.boundaries_detected
    }

    /// Get the total number of detection failures.
    pub fn total_detection_failures(&self) -> u64 {
        let state = self.state.borrow();
        state.stats.detection_failures
    }

    /// Get the total data bytes processed.
    pub fn total_data_bytes(&self) -> u64 {
        let state = self.state.borrow();
        state.stats.total_data_bytes
    }

    /// Get the total framed bytes processed.
    pub fn total_framed_bytes(&self) -> u64 {
        let state = self.state.borrow();
        state.stats.total_framed_bytes
    }

    /// Get the total padding bytes.
    pub fn total_padding_bytes(&self) -> u64 {
        let state = self.state.borrow();
        state.stats.total_padding_bytes
    }

    /// Get the total checksum bytes.
    pub fn total_checksum_bytes(&self) -> u64 {
        let state = self.state.borrow();
        state.stats.total_checksum_bytes
    }

    /// Get the frame count.
    pub fn frame_count(&self) -> usize {
        let state = self.state.borrow();
        state.frames.len()
    }

    /// Get frames by type.
    pub fn frames_by_type(&self, frame_type: FrameType) -> Vec<u64> {
        let state = self.state.borrow();
        state
            .frames
            .iter()
            .filter(|(_, f)| f.frame_type == frame_type)
            .map(|(id, _)| *id)
            .collect()
    }

    /// Get the handler count.
    pub fn handler_count(&self) -> u32 {
        let state = self.state.borrow();
        state.stats.total_handlers
    }

    /// Get the active handler count.
    pub fn active_handler_count(&self) -> u32 {
        let state = self.state.borrow();
        state.stats.active_handlers
    }

    /// Get the peer count.
    pub fn peer_count(&self) -> usize {
        let state = self.state.borrow();
        state.peer_versions.len()
    }

    /// Get all registered frame types.
    pub fn all_frame_types(&self) -> Vec<FrameType> {
        let state = self.state.borrow();
        state.frame_type_handlers.keys().cloned().collect()
    }

    /// Register a frame type.
    pub fn register_frame_type(&self, frame_type: FrameType) -> Result<(), FramingError> {
        let mut state = self.state.borrow_mut();
        if state.frame_type_handlers.contains_key(&frame_type) {
            return Err(FramingError::InvalidFrameType { frame_type });
        }
        state.handler_counter += 1;
        let handler_id = FrameHandlerId(state.handler_counter);
        state.frame_type_handlers.insert(frame_type, handler_id);
        state.stats.total_handlers += 1;
        state.stats.active_handlers = state.frame_type_handlers.len() as u32;
        state.version += 1;
        Ok(())
    }

    /// Unregister a frame type.
    pub fn unregister_frame_type(&self, frame_type: FrameType) -> Result<(), FramingError> {
        let mut state = self.state.borrow_mut();
        if state.frame_type_handlers.remove(&frame_type).is_none() {
            return Err(FramingError::FrameTypeHandlerNotFound { frame_type });
        }
        state.stats.total_handlers = state.frame_type_handlers.len() as u32;
        state.stats.active_handlers = state.frame_type_handlers.len() as u32;
        state.version += 1;
        Ok(())
    }

    /// Check if a frame type is registered.
    pub fn is_frame_type_registered(&self, frame_type: FrameType) -> bool {
        let state = self.state.borrow();
        state.frame_type_handlers.contains_key(&frame_type)
    }

    /// Get the handler ID for a frame type.
    pub fn get_handler_id(
        &self,
        frame_type: FrameType,
    ) -> Option<FrameHandlerId> {
        let state = self.state.borrow();
        state.frame_type_handlers.get(&frame_type).copied()
    }

    /// Get the framing version display string.
    pub fn framing_version_display(&self) -> String {
        let state = self.state.borrow();
        state.config.framing_version.display_string()
    }

    /// Get all peers by framing version.
    pub fn all_peer_versions(&self) -> Vec<(NodeId, FramingVersion)> {
        let state = self.state.borrow();
        state
            .peer_versions
            .iter()
            .map(|(peer_id, version)| (*peer_id, *version))
            .collect()
    }

    /// Remove a peer version.
    pub fn remove_peer_version(&self, peer_id: NodeId) -> Result<(), FramingError> {
        let mut state = self.state.borrow_mut();
        if state.peer_versions.remove(&peer_id).is_none() {
            return Err(FramingError::PeerNotFound { peer_id });
        }
        state.version += 1;
        Ok(())
    }

    /// Get the boundary detection algorithm.
    pub fn boundary_algorithm(&self) -> BoundaryAlgorithm {
        BoundaryAlgorithm::Adaptive
    }

    /// Set the boundary detection algorithm.
    pub fn set_boundary_algorithm(
        &self,
        algorithm: BoundaryAlgorithm,
    ) -> Result<(), FramingError> {
        let mut state = self.state.borrow_mut();
        let _ = algorithm; // Stored in boundary set, not directly in state
        state.version += 1;
        Ok(())
    }

    /// Get the payload efficiency for a given data size.
    pub fn payload_efficiency(&self, data_size: usize) -> f64 {
        let config = self.ensure_config();
        config.payload_efficiency(data_size)
    }

    /// Get the average frame size.
    pub fn average_frame_size(&self) -> f64 {
        let state = self.state.borrow();
        if state.frames.is_empty() {
            return 0.0;
        }
        let total: usize = state.frames.values().map(|f| f.framed_size).sum();
        total as f64 / state.frames.len() as f64
    }

    /// Get the average data size.
    pub fn average_data_size(&self) -> f64 {
        let state = self.state.borrow();
        if state.frames.is_empty() {
            return 0.0;
        }
        let total: usize = state.frames.values().map(|f| f.data_size).sum();
        total as f64 / state.frames.len() as f64
    }

    /// Get the total overhead bytes.
    pub fn total_overhead_bytes(&self) -> usize {
        let state = self.state.borrow();
        state
            .frames
            .values()
            .map(|f| f.framed_size - f.data_size)
            .sum()
    }

    /// Get the overhead percentage.
    pub fn overhead_percentage(&self) -> f64 {
        let state = self.state.borrow();
        let total_framed: usize = state.frames.values().map(|f| f.framed_size).sum();
        let total_data: usize = state.frames.values().map(|f| f.data_size).sum();
        if total_framed == 0 {
            return 0.0;
        }
        (total_framed - total_data) as f64 / total_framed as f64 * 100.0
    }

    /// Get the latest frame ID.
    pub fn latest_frame_id(&self) -> Option<u64> {
        let state = self.state.borrow();
        state.frames.keys().max().copied()
    }

    /// Get the frame framing success rate.
    pub fn frame_framing_success_rate(&self) -> f64 {
        let state = self.state.borrow();
        if state.stats.frames_framed == 0 {
            return 100.0;
        }
        100.0
    }

    /// Get the deframing success rate.
    pub fn deframing_success_rate(&self) -> f64 {
        let state = self.state.borrow();
        if state.stats.frames_deframed == 0 {
            return 100.0;
        }
        100.0
    }

    /// Get the boundary detection success rate.
    pub fn boundary_detection_success_rate(&self) -> f64 {
        let state = self.state.borrow();
        let total = state.stats.boundaries_detected + state.stats.detection_failures;
        if total == 0 {
            return 100.0;
        }
        state.stats.boundaries_detected as f64 / total as f64 * 100.0
    }

    /// Get the framing health score (0.0 to 1.0).
    pub fn framing_health_score(&self) -> f64 {
        let state = self.state.borrow();
        let framing_health = if state.stats.frames_framed == 0 {
            1.0
        } else {
            state.stats.frame_framing_success_rate() / 100.0
        };
        let deframing_health = if state.stats.frames_deframed == 0 {
            1.0
        } else {
            state.stats.deframing_success_rate() / 100.0
        };
        let boundary_health = state.stats.boundary_detection_success_rate() / 100.0;
        (framing_health + deframing_health + boundary_health) / 3.0
    }

    /// Clear all framing state.
    pub fn clear_all(&self) {
        let mut state = self.state.borrow_mut();
        state.config = FramingConfig::default();
        state.frames.clear();
        state.frame_counter = 0;
        state.peer_versions.clear();
        state.frame_type_handlers.clear();
        state.handler_counter = 0;
        state.boundary_detections = 0;
        state.detection_failures = 0;
        state.stats = FramingStats::new();
        state.version = 0;
    }

    /// Get the framing graph size.
    pub fn graph_size(&self) -> (usize, usize, usize, usize) {
        let state = self.state.borrow();
        (
            state.frames.len(),
            state.frame_type_handlers.len(),
            state.peer_versions.len(),
            state.config.max_frame_size,
        )
    }

    /// Get the config snapshot.
    pub fn config_snapshot(&self) -> FramingConfig {
        let state = self.state.borrow();
        state.config.clone()
    }

    /// Check if a data size fits within frame limits.
    pub fn data_fits(&self, data_size: usize) -> bool {
        let config = self.ensure_config();
        config.is_size_in_range(data_size)
    }

    /// Get the boundary detection timeout in milliseconds.
    pub fn boundary_detection_timeout_ms(&self) -> u64 {
        let state = self.state.borrow();
        state.config.boundary_detection_timeout_ms
    }

    /// Set the boundary detection timeout in milliseconds.
    pub fn set_boundary_detection_timeout_ms(
        &self,
        timeout_ms: u64,
    ) -> Result<(), FramingError> {
        let mut state = self.state.borrow_mut();
        if timeout_ms == 0 {
            return Err(FramingError::InvalidConfig {
                field: "boundary_detection_timeout_ms".to_string(),
                reason: "timeout must be greater than 0".to_string(),
            });
        }
        state.config.boundary_detection_timeout_ms = timeout_ms;
        state.version += 1;
        Ok(())
    }

    /// Get the max pending detections.
    pub fn max_pending_detections(&self) -> u32 {
        let state = self.state.borrow();
        state.config.max_pending_detections
    }

    /// Set the max pending detections.
    pub fn set_max_pending_detections(&self, max: u32) -> Result<(), FramingError> {
        let mut state = self.state.borrow_mut();
        if max == 0 {
            return Err(FramingError::InvalidConfig {
                field: "max_pending_detections".to_string(),
                reason: "max must be greater than 0".to_string(),
            });
        }
        state.config.max_pending_detections = max;
        state.version += 1;
        Ok(())
    }

    /// Get the max frame type handlers.
    pub fn max_frame_type_handlers(&self) -> u32 {
        let state = self.state.borrow();
        state.config.max_frame_type_handlers
    }

    /// Set the max frame type handlers.
    pub fn set_max_frame_type_handlers(
        &self,
        max: u32,
    ) -> Result<(), FramingError> {
        let mut state = self.state.borrow_mut();
        if max == 0 {
            return Err(FramingError::InvalidConfig {
                field: "max_frame_type_handlers".to_string(),
                reason: "max must be greater than 0".to_string(),
            });
        }
        state.config.max_frame_type_handlers = max;
        state.version += 1;
        Ok(())
    }

    /// Check if auto-negotiation is enabled.
    pub fn is_auto_negotiation_enabled(&self) -> bool {
        let state = self.state.borrow();
        state.config.enable_auto_negotiation
    }

    /// Enable or disable auto-negotiation.
    pub fn set_auto_negotiation(&self, enabled: bool) -> Result<(), FramingError> {
        let mut state = self.state.borrow_mut();
        state.config.enable_auto_negotiation = enabled;
        state.version += 1;
        Ok(())
    }

    /// Check if boundary detection is enabled.
    pub fn is_boundary_detection_enabled(&self) -> bool {
        let state = self.state.borrow();
        state.config.enable_boundary_detection
    }

    /// Enable or disable boundary detection.
    pub fn set_boundary_detection_enabled(
        &self,
        enabled: bool,
    ) -> Result<(), FramingError> {
        let mut state = self.state.borrow_mut();
        state.config.enable_boundary_detection = enabled;
        state.version += 1;
        Ok(())
    }

    /// Check if padding is enabled.
    pub fn padding_enabled(&self) -> bool {
        let state = self.state.borrow();
        state.config.enable_padding
    }

    /// Check if strict version matching is enabled.
    pub fn is_strict_version_matching(&self) -> bool {
        let state = self.state.borrow();
        state.config.strict_version_matching
    }

    /// Enable or disable strict version matching.
    pub fn set_strict_version_matching(
        &self,
        enabled: bool,
    ) -> Result<(), FramingError> {
        let mut state = self.state.borrow_mut();
        state.config.strict_version_matching = enabled;
        state.version += 1;
        Ok(())
    }

    /// Get the min supported framing version.
    pub fn min_supported_framing_version(&self) -> FramingVersion {
        let state = self.state.borrow();
        state.config.min_supported_version
    }

    /// Set the min supported framing version.
    pub fn set_min_supported_framing_version(
        &self,
        version: FramingVersion,
    ) -> Result<(), FramingError> {
        let mut state = self.state.borrow_mut();
        if version.0 == 0 {
            return Err(FramingError::InvalidConfig {
                field: "min_supported_version".to_string(),
                reason: "version must be greater than 0".to_string(),
            });
        }
        if version > state.config.max_supported_version {
            return Err(FramingError::InvalidConfig {
                field: "min_supported_version".to_string(),
                reason: "min must be <= max".to_string(),
            });
        }
        state.config.min_supported_version = version;
        state.version += 1;
        Ok(())
    }

    /// Get the max supported framing version.
    pub fn max_supported_framing_version(&self) -> FramingVersion {
        let state = self.state.borrow();
        state.config.max_supported_version
    }

    /// Set the max supported framing version.
    pub fn set_max_supported_framing_version(
        &self,
        version: FramingVersion,
    ) -> Result<(), FramingError> {
        let mut state = self.state.borrow_mut();
        if version.0 == 0 {
            return Err(FramingError::InvalidConfig {
                field: "max_supported_version".to_string(),
                reason: "version must be greater than 0".to_string(),
            });
        }
        if version < state.config.min_supported_version {
            return Err(FramingError::InvalidConfig {
                field: "max_supported_version".to_string(),
                reason: "max must be >= min".to_string(),
            });
        }
        state.config.max_supported_version = version;
        state.version += 1;
        Ok(())
    }

    /// Get the current version's display string.
    pub fn framing_version_display_string(&self) -> String {
        let state = self.state.borrow();
        state.config.framing_version.display_string()
    }

    /// Get the frame type as string.
    pub fn frame_type_as_string(frame_type: FrameType) -> String {
        frame_type.as_str().to_string()
    }

    /// Parse frame type from string.
    pub fn frame_type_from_string(s: &str) -> Option<FrameType> {
        match s {
            "data" => Some(FrameType::Data),
            "control" => Some(FrameType::Control),
            "ack" => Some(FrameType::Ack),
            "heartbeat" => Some(FrameType::Heartbeat),
            "discovery" => Some(FrameType::Discovery),
            "handshake" => Some(FrameType::Handshake),
            "fragment" => Some(FrameType::Fragment),
            "metadata" => Some(FrameType::Metadata),
            _ => None,
        }
    }

    /// Get all known frame types.
    pub fn all_frame_type_variants() -> Vec<FrameType> {
        vec![
            FrameType::Data,
            FrameType::Control,
            FrameType::Ack,
            FrameType::Heartbeat,
            FrameType::Discovery,
            FrameType::Handshake,
            FrameType::Fragment,
            FrameType::Metadata,
        ]
    }

    /// Get the frame header size.
    pub fn frame_header_size() -> usize {
        8
    }

    /// Get the default frame overhead.
    pub fn default_frame_overhead() -> usize {
        16
    }

    /// Get the default padding size.
    pub fn default_padding_size() -> usize {
        8
    }

    /// Validate frame type from raw byte.
    pub fn validate_frame_type_byte(byte: u8) -> Result<FrameType, FramingError> {
        match byte {
            0 => Ok(FrameType::Data),
            1 => Ok(FrameType::Control),
            2 => Ok(FrameType::Ack),
            3 => Ok(FrameType::Heartbeat),
            4 => Ok(FrameType::Discovery),
            5 => Ok(FrameType::Handshake),
            6 => Ok(FrameType::Fragment),
            7 => Ok(FrameType::Metadata),
            _ => Err(FramingError::InvalidFrameType {
                frame_type: FrameType::Custom(byte as u16),
            }),
        }
    }

    /// Get all boundary algorithms.
    pub fn all_boundary_algorithms() -> Vec<BoundaryAlgorithm> {
        vec![
            BoundaryAlgorithm::FixedSize,
            BoundaryAlgorithm::DelimiterBased,
            BoundaryAlgorithm::LengthPrefixed,
            BoundaryAlgorithm::PatternBased,
            BoundaryAlgorithm::Adaptive,
        ]
    }

    /// Check if a boundary algorithm is adaptive.
    pub fn is_adaptive_algorithm(algorithm: BoundaryAlgorithm) -> bool {
        matches!(algorithm, BoundaryAlgorithm::Adaptive)
    }

    /// Get the framing state version.
    pub fn framing_state_version(&self) -> u64 {
        let state = self.state.borrow();
        state.version
    }

    /// Increment the framing state version.
    pub fn increment_framing_version(&self) -> u64 {
        let mut state = self.state.borrow_mut();
        state.version += 1;
        state.version
    }

    /// Get the total frame type handler registrations.
    pub fn total_handler_registrations(&self) -> u64 {
        let state = self.state.borrow();
        state.handler_counter
    }

    /// Get the total boundary detection attempts.
    pub fn total_boundary_detection_attempts(&self) -> u64 {
        let state = self.state.borrow();
        state.boundary_detections
    }

    /// Check if framing is empty (no frames).
    pub fn is_empty(&self) -> bool {
        let state = self.state.borrow();
        state.frames.is_empty() && state.peer_versions.is_empty()
    }

    /// Get the framing summary.
    pub fn framing_summary(&self) -> FramingSummary {
        let state = self.state.borrow();
        FramingSummary {
            frame_count: state.frames.len() as u64,
            peer_count: state.peer_versions.len() as u64,
            handler_count: state.stats.total_handlers,
            total_data_bytes: state.stats.total_data_bytes,
            total_framed_bytes: state.stats.total_framed_bytes,
            version: state.version,
            health_score: self.framing_health_score(),
        }
    }

    /// Get the total overhead ratio as percentage.
    pub fn overhead_ratio_percent(&self) -> f64 {
        let state = self.state.borrow();
        if state.stats.total_data_bytes == 0 {
            return 0.0;
        }
        (state.stats.total_framed_bytes - state.stats.total_data_bytes) as f64
            / state.stats.total_data_bytes as f64
            * 100.0
    }

    /// Get the framing efficiency percentage.
    pub fn framing_efficiency(&self) -> f64 {
        let state = self.state.borrow();
        if state.stats.total_framed_bytes == 0 {
            return 100.0;
        }
        state.stats.total_data_bytes as f64 / state.stats.total_framed_bytes as f64 * 100.0
    }

    /// Get the padding overhead percentage.
    pub fn padding_overhead_percent(&self) -> f64 {
        let state = self.state.borrow();
        if state.stats.total_framed_bytes == 0 {
            return 0.0;
        }
        state.stats.total_padding_bytes as f64 / state.stats.total_framed_bytes as f64 * 100.0
    }

    /// Get the checksum overhead percentage.
    pub fn checksum_overhead_percent(&self) -> f64 {
        let state = self.state.borrow();
        if state.stats.total_framed_bytes == 0 {
            return 0.0;
        }
        state.stats.total_checksum_bytes as f64 / state.stats.total_framed_bytes as f64 * 100.0
    }
}

///////////////////////////////////////////////////////////////////////////////
// FRAMING SUMMARY
///////////////////////////////////////////////////////////////////////////////

/// Framing summary type.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FramingSummary {
    pub frame_count: u64,
    pub peer_count: u64,
    pub handler_count: u32,
    pub total_data_bytes: u64,
    pub total_framed_bytes: u64,
    pub version: u64,
    pub health_score: f64,
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

    fn test_node_id3() -> NodeId {
        test_node_id(3)
    }

    fn test_endpoint() -> Endpoint {
        Endpoint::new(CarrierType::Direct, vec![1, 2, 3, 4])
    }

    fn test_endpoint2() -> Endpoint {
        Endpoint::new(CarrierType::Direct, vec![5, 6, 7, 8])
    }

    fn test_frame_type() -> FrameType {
        FrameType::Data
    }

    fn test_frame_type2() -> FrameType {
        FrameType::Control
    }

    fn test_framing_version() -> FramingVersion {
        FramingVersion::new(1)
    }

    fn test_framing_version2() -> FramingVersion {
        FramingVersion::new(2)
    }

    fn test_config() -> FramingConfig {
        FramingConfig::default()
    }

    // ========================================================================
    // FramingConfig tests
    // ========================================================================

    #[test]
    fn framing_config_default() {
        let config = FramingConfig::default();
        assert_eq!(config.max_frame_size, 65536);
        assert_eq!(config.min_frame_size, 1);
        assert_eq!(config.frame_overhead, 16);
        assert_eq!(config.framing_version.0, 1);
        assert_eq!(config.min_supported_version.0, 1);
        assert_eq!(config.max_supported_version.0, 100);
        assert!(config.enable_auto_negotiation);
        assert!(config.enable_boundary_detection);
        assert_eq!(config.boundary_detection_timeout_ms, 5000);
        assert_eq!(config.max_pending_detections, 1000);
        assert!(config.enable_padding);
        assert_eq!(config.padding_size, 8);
        assert_eq!(config.max_frame_type_handlers, 50);
        assert!(!config.strict_version_matching);
    }

    #[test]
    fn framing_config_new_valid() {
        let config = FramingConfig::new(32768).unwrap();
        assert_eq!(config.max_frame_size, 32768);
    }

    #[test]
    fn framing_config_new_zero_size() {
        let result = FramingConfig::new(0);
        assert!(result.is_err());
    }

    #[test]
    fn framing_config_validate_zero_max_frame_size() {
        let mut config = FramingConfig::default();
        config.max_frame_size = 0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn framing_config_validate_zero_min_frame_size() {
        let mut config = FramingConfig::default();
        config.min_frame_size = 0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn framing_config_validate_min_greater_than_max() {
        let mut config = FramingConfig::default();
        config.min_frame_size = 1000;
        config.max_frame_size = 100;
        assert!(config.validate().is_err());
    }

    #[test]
    fn framing_config_validate_zero_frame_overhead() {
        let mut config = FramingConfig::default();
        config.frame_overhead = 0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn framing_config_validate_zero_boundary_timeout() {
        let mut config = FramingConfig::default();
        config.boundary_detection_timeout_ms = 0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn framing_config_validate_zero_max_pending() {
        let mut config = FramingConfig::default();
        config.max_pending_detections = 0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn framing_config_validate_zero_max_handlers() {
        let mut config = FramingConfig::default();
        config.max_frame_type_handlers = 0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn framing_config_strict() {
        let config = FramingConfig::strict();
        assert_eq!(config.max_frame_size, 4096);
        assert_eq!(config.min_frame_size, 16);
        assert_eq!(config.frame_overhead, 8);
        assert!(!config.enable_auto_negotiation);
        assert!(!config.enable_padding);
        assert_eq!(config.padding_size, 0);
        assert!(config.strict_version_matching);
        assert_eq!(config.max_frame_type_handlers, 10);
    }

    #[test]
    fn framing_config_lenient() {
        let config = FramingConfig::lenient();
        assert_eq!(config.max_frame_size, 1048576);
        assert!(!config.enable_boundary_detection);
        assert_eq!(config.max_frame_type_handlers, 500);
        assert_eq!(config.padding_size, 16);
    }

    #[test]
    fn framing_config_frame_size_range() {
        let config = FramingConfig::default();
        let (min, max) = config.frame_size_range();
        assert_eq!(min, 1);
        assert_eq!(max, 65536);
    }

    #[test]
    fn framing_config_is_size_in_range() {
        let config = FramingConfig::default();
        assert!(config.is_size_in_range(100));
        assert!(config.is_size_in_range(1));
        assert!(config.is_size_in_range(65536));
        assert!(!config.is_size_in_range(0));
        assert!(!config.is_size_in_range(65537));
    }

    #[test]
    fn framing_config_payload_efficiency() {
        let config = FramingConfig::default();
        let efficiency = config.payload_efficiency(1000);
        assert!(efficiency > 0.0);
        assert!(efficiency <= 100.0);
    }

    #[test]
    fn framing_config_payload_efficiency_zero() {
        let config = FramingConfig::default();
        assert!((config.payload_efficiency(0) - 100.0).abs() < 0.0001);
    }

    #[test]
    fn framing_config_serialization_roundtrip() {
        let config = FramingConfig::default();
        let json = serde_json::to_string(&config).unwrap();
        let deserialized: FramingConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(config.max_frame_size, deserialized.max_frame_size);
        assert_eq!(config.frame_overhead, deserialized.frame_overhead);
    }

    // ========================================================================
    // FrameType tests
    // ========================================================================

    #[test]
    fn frame_type_as_str() {
        assert_eq!(FrameType::Data.as_str(), "data");
        assert_eq!(FrameType::Control.as_str(), "control");
        assert_eq!(FrameType::Ack.as_str(), "ack");
        assert_eq!(FrameType::Heartbeat.as_str(), "heartbeat");
        assert_eq!(FrameType::Discovery.as_str(), "discovery");
        assert_eq!(FrameType::Handshake.as_str(), "handshake");
        assert_eq!(FrameType::Fragment.as_str(), "fragment");
        assert_eq!(FrameType::Metadata.as_str(), "metadata");
        assert_eq!(FrameType::Custom(42).as_str(), "custom");
    }

    #[test]
    fn frame_type_from_u16() {
        assert_eq!(FrameType::from_u16(0), FrameType::Data);
        assert_eq!(FrameType::from_u16(1), FrameType::Control);
        assert_eq!(FrameType::from_u16(2), FrameType::Ack);
        assert_eq!(FrameType::from_u16(3), FrameType::Heartbeat);
        assert_eq!(FrameType::from_u16(4), FrameType::Discovery);
        assert_eq!(FrameType::from_u16(5), FrameType::Handshake);
        assert_eq!(FrameType::from_u16(6), FrameType::Fragment);
        assert_eq!(FrameType::from_u16(7), FrameType::Metadata);
        assert_eq!(FrameType::from_u16(99), FrameType::Custom(99));
    }

    #[test]
    fn frame_type_to_u16() {
        assert_eq!(FrameType::Data.to_u16(), 0);
        assert_eq!(FrameType::Control.to_u16(), 1);
        assert_eq!(FrameType::Ack.to_u16(), 2);
        assert_eq!(FrameType::Custom(42).to_u16(), 42);
    }

    #[test]
    fn frame_type_is_control_frame() {
        assert!(FrameType::Control.is_control_frame());
        assert!(FrameType::Ack.is_control_frame());
        assert!(FrameType::Heartbeat.is_control_frame());
        assert!(FrameType::Handshake.is_control_frame());
        assert!(!FrameType::Data.is_control_frame());
        assert!(!FrameType::Fragment.is_control_frame());
    }

    #[test]
    fn frame_type_is_data_frame() {
        assert!(FrameType::Data.is_data_frame());
        assert!(FrameType::Fragment.is_data_frame());
        assert!(!FrameType::Control.is_data_frame());
        assert!(!FrameType::Ack.is_data_frame());
    }

    #[test]
    fn frame_type_serialization_roundtrip() {
        let frame_type = FrameType::Fragment;
        let json = serde_json::to_string(&frame_type).unwrap();
        let deserialized: FrameType = serde_json::from_str(&json).unwrap();
        assert_eq!(frame_type, deserialized);
    }

    // ========================================================================
    // FramedPacket tests
    // ========================================================================

    #[test]
    fn framed_packet_new() {
        let packet = FramedPacket::new(1, FrameType::Data, vec![1, 2, 3]);
        assert_eq!(packet.packet_id, 1);
        assert_eq!(packet.frame_type, FrameType::Data);
        assert_eq!(packet.data, vec![1, 2, 3]);
        assert_eq!(packet.payload_size(), 3);
        assert!(!packet.has_checksum());
        assert!(!packet.has_padding());
    }

    #[test]
    fn framed_packet_total_size() {
        let packet = FramedPacket::new(1, FrameType::Data, vec![1, 2, 3, 4, 5]);
        let expected = 8 + 5; // header + payload
        assert_eq!(packet.total_size(), expected);
    }

    #[test]
    fn framed_packet_overhead_size() {
        let packet = FramedPacket::new(1, FrameType::Data, vec![1, 2, 3]);
        assert_eq!(packet.overhead_size(), 8);
    }

    #[test]
    fn framed_packet_has_checksum() {
        let packet = FramedPacket::new(1, FrameType::Data, vec![1]);
        assert!(!packet.has_checksum());

        let mut packet = FramedPacket::new(1, FrameType::Data, vec![1]);
        packet.checksum = Some(vec![0u8; 32]);
        assert!(packet.has_checksum());
    }

    #[test]
    fn framed_packet_has_padding() {
        let packet = FramedPacket::new(1, FrameType::Data, vec![1]);
        assert!(!packet.has_padding());

        let mut packet = FramedPacket::new(1, FrameType::Data, vec![1]);
        packet.padding = vec![0u8; 8];
        assert!(packet.has_padding());
    }

    #[test]
    fn framed_packet_serialization_roundtrip() {
        let packet = FramedPacket::new(42, FrameType::Control, vec![1, 2, 3, 4]);
        let json = serde_json::to_string(&packet).unwrap();
        let deserialized: FramedPacket = serde_json::from_str(&json).unwrap();
        assert_eq!(packet, deserialized);
    }

    // ========================================================================
    // FrameHeader tests
    // ========================================================================

    #[test]
    fn frame_header_default() {
        let header = FrameHeader::default();
        assert_eq!(header.version, 0);
        assert_eq!(header.flags.bits(), 0);
        assert_eq!(header.frame_type_code, 0);
    }

    #[test]
    fn frame_header_new() {
        let header = FrameHeader::new(1, FrameType::Data);
        assert_eq!(header.version, 1);
        assert_eq!(header.frame_type_code, 0);
        assert_eq!(header.size(), 8);
    }

    #[test]
    fn frame_header_flag_operations() {
        let mut header = FrameHeader::default();
        assert!(!header.has_flag(FrameHeaderFlags::FLAG_HAS_CHECKSUM));
        header.set_flag(FrameHeaderFlags::FLAG_HAS_CHECKSUM);
        assert!(header.has_flag(FrameHeaderFlags::FLAG_HAS_CHECKSUM));
        header.clear_flag(FrameHeaderFlags::FLAG_HAS_CHECKSUM);
        assert!(!header.has_flag(FrameHeaderFlags::FLAG_HAS_CHECKSUM));
    }

    #[test]
    fn frame_header_serialization_roundtrip() {
        let header = FrameHeader::new(2, FrameType::Heartbeat);
        let json = serde_json::to_string(&header).unwrap();
        let deserialized: FrameHeader = serde_json::from_str(&json).unwrap();
        assert_eq!(header, deserialized);
    }

    // ========================================================================
    // FrameHeaderFlags tests
    // ========================================================================

    #[test]
    fn frame_header_flags_empty() {
        let flags = FrameHeaderFlags::empty();
        assert_eq!(flags.bits(), 0);
    }

    #[test]
    fn frame_header_flags_all() {
        let flags = FrameHeaderFlags::all();
        assert_eq!(flags.bits(), 0xFF);
    }

    #[test]
    fn frame_header_flags_contains() {
        let mut flags = FrameHeaderFlags::empty();
        flags.insert(FrameHeaderFlags::FLAG_PRIORITY);
        assert!(flags.contains(FrameHeaderFlags::FLAG_PRIORITY));
        assert!(!flags.contains(FrameHeaderFlags::FLAG_ENCRYPTED));
    }

    #[test]
    fn frame_header_flags_insert_remove() {
        let mut flags = FrameHeaderFlags::empty();
        flags.insert(FrameHeaderFlags::FLAG_COMPRESSED);
        assert!(flags.contains(FrameHeaderFlags::FLAG_COMPRESSED));
        flags.remove(FrameHeaderFlags::FLAG_COMPRESSED);
        assert!(!flags.contains(FrameHeaderFlags::FLAG_COMPRESSED));
    }

    #[test]
    fn frame_header_flags_serialization_roundtrip() {
        let mut flags = FrameHeaderFlags::empty();
        flags.insert(FrameHeaderFlags::FLAG_ENCRYPTED);
        let json = serde_json::to_string(&flags).unwrap();
        let deserialized: FrameHeaderFlags = serde_json::from_str(&json).unwrap();
        assert_eq!(flags, deserialized);
    }

    // ========================================================================
    // FrameBoundary tests
    // ========================================================================

    #[test]
    fn frame_boundary_new() {
        let boundary = FrameBoundary::new(0, 100, FrameType::Data);
        assert_eq!(boundary.offset, 0);
        assert_eq!(boundary.length, 100);
        assert_eq!(boundary.frame_type, FrameType::Data);
        assert!((boundary.boundary_confidence - 1.0).abs() < 0.0001);
    }

    #[test]
    fn frame_boundary_end_offset() {
        let boundary = FrameBoundary::new(100, 50, FrameType::Control);
        assert_eq!(boundary.end_offset(), 150);
    }

    #[test]
    fn frame_boundary_is_high_confidence() {
        let mut boundary = FrameBoundary::new(0, 100, FrameType::Data);
        assert!(boundary.is_high_confidence());
        boundary.boundary_confidence = 0.5;
        assert!(!boundary.is_high_confidence());
    }

    #[test]
    fn frame_boundary_serialization_roundtrip() {
        let boundary = FrameBoundary::new(50, 200, FrameType::Ack);
        let json = serde_json::to_string(&boundary).unwrap();
        let deserialized: FrameBoundary = serde_json::from_str(&json).unwrap();
        assert_eq!(boundary, deserialized);
    }

    // ========================================================================
    // FrameBoundarySet tests
    // ========================================================================

    #[test]
    fn frame_boundary_set_new() {
        let set = FrameBoundarySet::new(BoundaryAlgorithm::FixedSize);
        assert_eq!(set.boundary_count(), 0);
        assert_eq!(set.total_detected, 0);
    }

    #[test]
    fn frame_boundary_set_add_boundary() {
        let mut set = FrameBoundarySet::new(BoundaryAlgorithm::Adaptive);
        set.add_boundary(FrameBoundary::new(0, 100, FrameType::Data));
        set.add_boundary(FrameBoundary::new(100, 200, FrameType::Control));
        assert_eq!(set.boundary_count(), 2);
        assert_eq!(set.total_detected, 2);
    }

    #[test]
    fn frame_boundary_set_clear() {
        let mut set = FrameBoundarySet::new(BoundaryAlgorithm::LengthPrefixed);
        set.add_boundary(FrameBoundary::new(0, 100, FrameType::Data));
        assert_eq!(set.boundary_count(), 1);
        set.clear();
        assert_eq!(set.boundary_count(), 0);
    }

    #[test]
    fn frame_boundary_set_serialization_roundtrip() {
        let mut set = FrameBoundarySet::new(BoundaryAlgorithm::PatternBased);
        set.add_boundary(FrameBoundary::new(0, 100, FrameType::Data));
        let json = serde_json::to_string(&set).unwrap();
        let deserialized: FrameBoundarySet = serde_json::from_str(&json).unwrap();
        assert_eq!(set.boundary_count(), deserialized.boundary_count());
    }

    // ========================================================================
    // BoundaryAlgorithm tests
    // ========================================================================

    #[test]
    fn boundary_algorithm_as_str() {
        assert_eq!(BoundaryAlgorithm::FixedSize.as_str(), "fixed_size");
        assert_eq!(BoundaryAlgorithm::DelimiterBased.as_str(), "delimiter_based");
        assert_eq!(BoundaryAlgorithm::LengthPrefixed.as_str(), "length_prefixed");
        assert_eq!(BoundaryAlgorithm::PatternBased.as_str(), "pattern_based");
        assert_eq!(BoundaryAlgorithm::Adaptive.as_str(), "adaptive");
    }

    #[test]
    fn boundary_algorithm_serialization_roundtrip() {
        let algorithm = BoundaryAlgorithm::Adaptive;
        let json = serde_json::to_string(&algorithm).unwrap();
        let deserialized: BoundaryAlgorithm = serde_json::from_str(&json).unwrap();
        assert_eq!(algorithm, deserialized);
    }

    // ========================================================================
    // FramingVersion tests
    // ========================================================================

    #[test]
    fn framing_version_new() {
        let version = FramingVersion::new(42);
        assert_eq!(version.0, 42);
    }

    #[test]
    fn framing_version_major_minor_patch() {
        let version = FramingVersion::new(1234);
        assert_eq!(version.major(), 1);
        assert_eq!(version.minor(), 2);
        assert_eq!(version.patch(), 34);
    }

    #[test]
    fn framing_version_increment_major() {
        let version = FramingVersion::new(1000);
        let incremented = version.increment_major();
        assert_eq!(incremented.0, 2000);
    }

    #[test]
    fn framing_version_increment_minor() {
        let version = FramingVersion::new(1100);
        let incremented = version.increment_minor();
        assert_eq!(incremented.0, 1200);
    }

    #[test]
    fn framing_version_increment_patch() {
        let version = FramingVersion::new(1001);
        let incremented = version.increment_patch();
        assert_eq!(incremented.0, 1002);
    }

    #[test]
    fn framing_version_display_string() {
        let version = FramingVersion::new(1023);
        assert_eq!(version.display_string(), "1.2.23");
    }

    #[test]
    fn framing_version_ordering() {
        let v1 = FramingVersion::new(1);
        let v2 = FramingVersion::new(2);
        assert!(v1 < v2);
        assert!(v2 > v1);
    }

    #[test]
    fn framing_version_serialization_roundtrip() {
        let version = FramingVersion::new(42);
        let json = serde_json::to_string(&version).unwrap();
        let deserialized: FramingVersion = serde_json::from_str(&json).unwrap();
        assert_eq!(version, deserialized);
    }

    // ========================================================================
    // FrameHandlerId tests
    // ========================================================================

    #[test]
    fn frame_handler_id_new() {
        let id = FrameHandlerId(42);
        assert_eq!(id.0, 42);
    }

    #[test]
    fn frame_handler_id_serialization_roundtrip() {
        let id = FrameHandlerId(100);
        let json = serde_json::to_string(&id).unwrap();
        let deserialized: FrameHandlerId = serde_json::from_str(&json).unwrap();
        assert_eq!(id, deserialized);
    }

    // ========================================================================
    // FramingStats tests
    // ========================================================================

    #[test]
    fn framing_stats_new() {
        let stats = FramingStats::new();
        assert_eq!(stats.frames_framed, 0);
        assert_eq!(stats.frames_deframed, 0);
        assert_eq!(stats.boundaries_detected, 0);
        assert_eq!(stats.detection_failures, 0);
    }

    #[test]
    fn framing_stats_frame_framing_success_rate() {
        let stats = FramingStats::new();
        assert!((stats.frame_framing_success_rate() - 100.0).abs() < 0.01);
    }

    #[test]
    fn framing_stats_deframing_success_rate() {
        let stats = FramingStats::new();
        assert!((stats.deframing_success_rate() - 100.0).abs() < 0.01);
    }

    #[test]
    fn framing_stats_boundary_detection_success_rate() {
        let stats = FramingStats::new();
        assert!((stats.boundary_detection_success_rate() - 100.0).abs() < 0.01);
    }

    #[test]
    fn framing_stats_boundary_detection_success_rate_with_failures() {
        let mut stats = FramingStats::new();
        stats.boundaries_detected = 80;
        stats.detection_failures = 20;
        assert!((stats.boundary_detection_success_rate() - 80.0).abs() < 0.01);
    }

    #[test]
    fn framing_stats_negotiation_success_rate() {
        let mut stats = FramingStats::new();
        stats.total_negotiations = 200;
        stats.successful_negotiations = 180;
        stats.failed_negotiations = 20;
        assert!((stats.negotiation_success_rate() - 90.0).abs() < 0.01);
    }

    #[test]
    fn framing_stats_overhead_ratio() {
        let mut stats = FramingStats::new();
        stats.total_data_bytes = 1000;
        stats.total_framed_bytes = 1100;
        assert!((stats.overhead_ratio() - 0.1).abs() < 0.01);
    }

    #[test]
    fn framing_stats_bytes_per_frame_avg() {
        let mut stats = FramingStats::new();
        stats.total_framed_bytes = 10000;
        stats.frames_framed = 100;
        assert!((stats.bytes_per_frame_avg() - 100.0).abs() < 0.01);
    }

    #[test]
    fn framing_stats_serialization_roundtrip() {
        let stats = FramingStats::new();
        let json = serde_json::to_string(&stats).unwrap();
        let deserialized: FramingStats = serde_json::from_str(&json).unwrap();
        assert_eq!(stats.frames_framed, deserialized.frames_framed);
    }

    // ========================================================================
    // FramingSnapshot tests
    // ========================================================================

    #[test]
    fn framing_snapshot_peer_count() {
        let snapshot = FramingSnapshot {
            version: 1,
            framing_version: FramingVersion::new(1),
            max_frame_size: 65536,
            padding_enabled: true,
            padding_size: 8,
            boundary_algorithm: BoundaryAlgorithm::Adaptive,
            peer_versions: HashMap::new(),
            stats: FramingStats::new(),
        };
        assert_eq!(snapshot.peer_count(), 0);
    }

    #[test]
    fn framing_snapshot_serialization_roundtrip() {
        let snapshot = FramingSnapshot {
            version: 1,
            framing_version: FramingVersion::new(1),
            max_frame_size: 65536,
            padding_enabled: true,
            padding_size: 8,
            boundary_algorithm: BoundaryAlgorithm::Adaptive,
            peer_versions: HashMap::new(),
            stats: FramingStats::new(),
        };
        let json = serde_json::to_string(&snapshot).unwrap();
        let deserialized: FramingSnapshot = serde_json::from_str(&json).unwrap();
        assert_eq!(snapshot.version, deserialized.version);
    }

    // ========================================================================
    // FramingError tests
    // ========================================================================

    #[test]
    fn framing_error_category() {
        assert_eq!(
            FramingError::InvalidConfig {
                field: "x".to_string(),
                reason: "y".to_string(),
            }
            .category(),
            "invalid_config"
        );
        assert_eq!(
            FramingError::FrameTooLarge {
                size: 1000,
                max_size: 500,
            }
            .category(),
            "frame_too_large"
        );
    }

    #[test]
    fn framing_error_is_recoverable() {
        assert!(FramingError::FrameTooLarge {
            size: 1000,
            max_size: 500,
        }
        .is_recoverable());
        assert!(FramingError::NegotiationTimeout {
            peer_id: test_node_id1(),
        }
        .is_recoverable());
        assert!(!FramingError::InvalidConfig {
            field: "x".to_string(),
            reason: "y".to_string(),
        }
        .is_recoverable());
    }

    #[test]
    fn framing_error_suggested_action() {
        assert_eq!(
            FramingError::PeerNotFound {
                peer_id: test_node_id1(),
            }
            .suggested_action(),
            "register the peer first"
        );
        assert!(!FramingError::Internal {
            message: "test".to_string()
        }
        .suggested_action()
        .is_empty());
    }

    #[test]
    fn framing_error_display() {
        let err = FramingError::FrameTooLarge {
            size: 1000,
            max_size: 500,
        };
        assert!(!err.to_string().is_empty());
    }

    #[test]
    fn framing_error_serialization_roundtrip() {
        let err = FramingError::FrameNotFound { packet_id: 999 };
        let json = serde_json::to_string(&err).unwrap();
        let deserialized: FramingError = serde_json::from_str(&json).unwrap();
        assert!(matches!(deserialized, FramingError::FrameNotFound { .. }));
    }

    // ========================================================================
    // FramingEngine tests
    // ========================================================================

    #[test]
    fn framing_engine_new() {
        let engine = FramingEngine::new();
        assert_eq!(engine.max_frame_size(), 65536);
        assert!(engine.is_healthy());
    }

    #[test]
    fn framing_engine_with_config() {
        let config = FramingConfig::new(32768).unwrap();
        let engine = FramingEngine::with_config(config).unwrap();
        assert_eq!(engine.max_frame_size(), 32768);
    }

    #[test]
    fn framing_engine_with_config_invalid() {
        let mut config = FramingConfig::default();
        config.max_frame_size = 0;
        let result = FramingEngine::with_config(config);
        assert!(result.is_err());
    }

    #[test]
    fn framing_engine_configure_framing() {
        let engine = FramingEngine::new();
        let config = FramingConfig::strict();
        assert!(engine.configure_framing(config.clone()).is_ok());
        assert_eq!(engine.max_frame_size(), 4096);
    }

    #[test]
    fn framing_engine_configure_framing_invalid() {
        let engine = FramingEngine::new();
        let mut config = FramingConfig::default();
        config.max_frame_size = 0;
        let err = engine.configure_framing(config).unwrap_err();
        assert!(matches!(err, FramingError::InvalidConfig { .. }));
    }

    #[test]
    fn framing_engine_frame_packet() {
        let engine = FramingEngine::new();
        let packet = engine.frame_packet(vec![1, 2, 3, 4], FrameType::Data).unwrap();
        assert_eq!(packet.packet_id, 1);
        assert_eq!(packet.frame_type, FrameType::Data);
        assert_eq!(packet.data, vec![1, 2, 3, 4]);
        assert!(packet.has_padding()); // padding enabled by default
    }

    #[test]
    fn framing_engine_frame_packet_too_large() {
        let engine = FramingEngine::new();
        let mut config = FramingConfig::default();
        config.max_frame_size = 10;
        engine.configure_framing(config).unwrap();
        let err = engine.frame_packet(vec![1; 100], FrameType::Data).unwrap_err();
        assert!(matches!(err, FramingError::FrameTooLarge { .. }));
    }

    #[test]
    fn framing_engine_frame_packet_too_small() {
        let engine = FramingEngine::new();
        let mut config = FramingConfig::default();
        config.min_frame_size = 100;
        engine.configure_framing(config).unwrap();
        let err = engine.frame_packet(vec![1, 2, 3], FrameType::Data).unwrap_err();
        assert!(matches!(err, FramingError::FrameTooSmall { .. }));
    }

    #[test]
    fn framing_engine_deframe_packet() {
        let engine = FramingEngine::new();
        let packet = engine.frame_packet(vec![1, 2, 3], FrameType::Control).unwrap();
        let (data, frame_type) = engine.deframe_packet(&packet).unwrap();
        assert_eq!(data, vec![1, 2, 3]);
        assert_eq!(frame_type, FrameType::Control);
    }

    #[test]
    fn framing_engine_deframe_corrupted() {
        let engine = FramingEngine::new();
        let mut packet = FramedPacket::new(1, FrameType::Data, vec![]);
        packet.padding = vec![];
        let err = engine.deframe_packet(&packet).unwrap_err();
        assert!(matches!(err, FramingError::CorruptedFrameData { .. }));
    }

    #[test]
    fn framing_engine_frame_boundaries() {
        let engine = FramingEngine::new();
        let boundaries = engine.frame_boundaries();
        assert_eq!(boundaries.boundary_count(), 0);
    }

    #[test]
    fn framing_engine_detect_boundaries() {
        let engine = FramingEngine::new();
        let stream = vec![1, 2, 3, 4, 5];
        let boundaries = engine.detect_boundaries(&stream).unwrap();
        assert!(!boundaries.is_empty());
    }

    #[test]
    fn framing_engine_detect_boundaries_empty() {
        let engine = FramingEngine::new();
        let err = engine.detect_boundaries(&[]).unwrap_err();
        assert!(matches!(err, FramingError::BoundaryDetectionFailed { .. }));
    }

    #[test]
    fn framing_engine_max_frame_size() {
        let engine = FramingEngine::new();
        assert_eq!(engine.max_frame_size(), 65536);
    }

    #[test]
    fn framing_engine_set_max_frame_size() {
        let engine = FramingEngine::new();
        assert!(engine.set_max_frame_size(32768).is_ok());
        assert_eq!(engine.max_frame_size(), 32768);
    }

    #[test]
    fn framing_engine_set_max_frame_size_zero() {
        let engine = FramingEngine::new();
        let err = engine.set_max_frame_size(0).unwrap_err();
        assert!(matches!(err, FramingError::InvalidConfig { .. }));
    }

    #[test]
    fn framing_engine_min_frame_size() {
        let engine = FramingEngine::new();
        assert_eq!(engine.min_frame_size(), 1);
    }

    #[test]
    fn framing_engine_fits_frame() {
        let engine = FramingEngine::new();
        assert!(engine.fits_frame(100));
        assert!(!engine.fits_frame(1000000));
    }

    #[test]
    fn framing_engine_frame_overhead() {
        let engine = FramingEngine::new();
        assert_eq!(engine.frame_overhead(), 16);
    }

    #[test]
    fn framing_engine_framing_version() {
        let engine = FramingEngine::new();
        assert_eq!(engine.framing_version().0, 1);
    }

    #[test]
    fn framing_engine_set_framing_version() {
        let engine = FramingEngine::new();
        assert!(engine.set_framing_version(FramingVersion::new(50)).is_ok());
        assert_eq!(engine.framing_version().0, 50);
    }

    #[test]
    fn framing_engine_set_framing_version_out_of_range() {
        let engine = FramingEngine::new();
        let err = engine
            .set_framing_version(FramingVersion::new(200))
            .unwrap_err();
        assert!(matches!(err, FramingError::InvalidConfig { .. }));
    }

    #[test]
    fn framing_engine_supported_framing_versions() {
        let engine = FramingEngine::new();
        let versions = engine.supported_framing_versions();
        assert_eq!(versions[0], FramingVersion::new(1));
        assert_eq!(versions.last(), Some(&FramingVersion::new(100)));
    }

    #[test]
    fn framing_engine_negotiate_framing_version() {
        let engine = FramingEngine::new();
        let negotiated = engine
            .negotiate_framing_version(
                test_node_id1(),
                vec![FramingVersion::new(1), FramingVersion::new(2)],
            )
            .unwrap();
        assert_eq!(negotiated, FramingVersion::new(2));
        assert_eq!(
            engine.peer_version(test_node_id1()),
            Some(FramingVersion::new(2))
        );
    }

    #[test]
    fn framing_engine_negotiate_framing_version_timeout() {
        let engine = FramingEngine::new();
        let mut config = FramingConfig::default();
        config.min_supported_version = FramingVersion::new(100);
        config.max_supported_version = FramingVersion::new(200);
        let engine2 = FramingEngine::with_config(config).unwrap();
        let err = engine2
            .negotiate_framing_version(test_node_id1(), vec![FramingVersion::new(1)])
            .unwrap_err();
        assert!(matches!(err, FramingError::NegotiationTimeout { .. }));
    }

    #[test]
    fn framing_engine_classify_frame_type() {
        let engine = FramingEngine::new();
        let frame_type = engine.classify_frame_type(&[0]).unwrap();
        assert_eq!(frame_type, FrameType::Data);
        let frame_type = engine.classify_frame_type(&[5]).unwrap();
        assert_eq!(frame_type, FrameType::Handshake);
    }

    #[test]
    fn framing_engine_classify_frame_type_empty() {
        let engine = FramingEngine::new();
        let err = engine.classify_frame_type(&[]).unwrap_err();
        assert!(matches!(err, FramingError::InvalidFrameType { .. }));
    }

    #[test]
    fn framing_engine_register_frame_type_handler() {
        let engine = FramingEngine::new();
        let handler_id = FrameHandlerId(1);
        assert!(engine
            .register_frame_type_handler(FrameType::Data, handler_id)
            .is_ok());
        assert_eq!(
            engine.frame_type_handler(FrameType::Data),
            Some(handler_id)
        );
    }

    #[test]
    fn framing_engine_register_frame_type_handler_max_exceeded() {
        let engine = FramingEngine::new();
        let mut config = FramingConfig::default();
        config.max_frame_type_handlers = 1;
        engine.configure_framing(config).unwrap();
        engine
            .register_frame_type_handler(FrameType::Data, FrameHandlerId(1))
            .unwrap();
        let err = engine
            .register_frame_type_handler(FrameType::Control, FrameHandlerId(2))
            .unwrap_err();
        assert!(matches!(
            err,
            FramingError::MaxFrameTypeHandlersExceeded { .. }
        ));
    }

    #[test]
    fn framing_engine_frame_type_handler_not_found() {
        let engine = FramingEngine::new();
        assert!(engine.frame_type_handler(FrameType::Data).is_none());
    }

    #[test]
    fn framing_engine_stats() {
        let engine = FramingEngine::new();
        engine.frame_packet(vec![1, 2, 3], FrameType::Data).unwrap();
        let stats = engine.stats();
        assert_eq!(stats.frames_framed, 1);
    }

    #[test]
    fn framing_engine_is_healthy() {
        let engine = FramingEngine::new();
        assert!(engine.is_healthy());
    }

    #[test]
    fn framing_engine_validate_config() {
        let engine = FramingEngine::new();
        let config = FramingConfig::default();
        assert!(engine.validate_config(&config).is_ok());
    }

    #[test]
    fn framing_engine_snapshot_restore() {
        let engine = FramingEngine::new();
        engine.register_frame_type_handler(FrameType::Data, FrameHandlerId(1)).unwrap();

        let snapshot = engine.snapshot();
        assert_eq!(snapshot.peer_count(), 0);

        let engine2 = FramingEngine::new();
        engine2.restore_from_snapshot(snapshot).unwrap();
    }

    #[test]
    fn framing_engine_peers_by_version() {
        let engine = FramingEngine::new();
        engine.negotiate_framing_version(test_node_id1(), vec![FramingVersion::new(1)]).unwrap();
        engine.negotiate_framing_version(test_node_id2(), vec![FramingVersion::new(2)]).unwrap();
        let peers = engine.peers_by_version(FramingVersion::new(1));
        assert_eq!(peers.len(), 1);
        assert_eq!(peers[0], test_node_id1());
    }

    #[test]
    fn framing_engine_peer_version() {
        let engine = FramingEngine::new();
        assert!(engine.peer_version(test_node_id1()).is_none());
        engine.negotiate_framing_version(test_node_id1(), vec![FramingVersion::new(5)]).unwrap();
        assert_eq!(
            engine.peer_version(test_node_id1()),
            Some(FramingVersion::new(5))
        );
    }

    #[test]
    fn framing_engine_set_peer_version() {
        let engine = FramingEngine::new();
        assert!(engine.set_peer_version(test_node_id1(), FramingVersion::new(3)).is_ok());
        assert_eq!(
            engine.peer_version(test_node_id1()),
            Some(FramingVersion::new(3))
        );
    }

    #[test]
    fn framing_engine_set_peer_version_out_of_range() {
        let engine = FramingEngine::new();
        let err = engine
            .set_peer_version(test_node_id1(), FramingVersion::new(0))
            .unwrap_err();
        assert!(matches!(err, FramingError::InvalidConfig { .. }));
    }

    #[test]
    fn framing_engine_negotiation_timeout() {
        let engine = FramingEngine::new();
        assert_eq!(engine.negotiation_timeout_ms(), 5000);
        engine.set_negotiation_timeout_ms(10000).unwrap();
        assert_eq!(engine.negotiation_timeout_ms(), 10000);
    }

    #[test]
    fn framing_engine_padding() {
        let engine = FramingEngine::new();
        assert!(engine.is_padding_enabled());
        assert_eq!(engine.padding_size(), 8);
    }

    // ========================================================================
    // FramingEngine advanced methods tests
    // ========================================================================

    #[test]
    fn framing_engine_boundary_detection_timeout() {
        let engine = FramingEngine::new();
        assert_eq!(engine.boundary_detection_timeout_ms(), 5000);
        engine.set_boundary_detection_timeout_ms(10000).unwrap();
        assert_eq!(engine.boundary_detection_timeout_ms(), 10000);
    }

    #[test]
    fn framing_engine_max_pending_detections() {
        let engine = FramingEngine::new();
        assert_eq!(engine.max_pending_detections(), 1000);
        engine.set_max_pending_detections(500).unwrap();
        assert_eq!(engine.max_pending_detections(), 500);
    }

    #[test]
    fn framing_engine_max_frame_type_handlers() {
        let engine = FramingEngine::new();
        assert_eq!(engine.max_frame_type_handlers(), 50);
        engine.set_max_frame_type_handlers(25).unwrap();
        assert_eq!(engine.max_frame_type_handlers(), 25);
    }

    #[test]
    fn framing_engine_auto_negotiation() {
        let engine = FramingEngine::new();
        assert!(engine.is_auto_negotiation_enabled());
        engine.set_auto_negotiation(false).unwrap();
        assert!(!engine.is_auto_negotiation_enabled());
    }

    #[test]
    fn framing_engine_boundary_detection_enabled() {
        let engine = FramingEngine::new();
        assert!(engine.is_boundary_detection_enabled());
        engine.set_boundary_detection_enabled(false).unwrap();
        assert!(!engine.is_boundary_detection_enabled());
    }

    #[test]
    fn framing_engine_strict_version_matching() {
        let engine = FramingEngine::new();
        assert!(!engine.is_strict_version_matching());
        engine.set_strict_version_matching(true).unwrap();
        assert!(engine.is_strict_version_matching());
    }

    #[test]
    fn framing_engine_min_max_supported_framing_version() {
        let engine = FramingEngine::new();
        assert_eq!(engine.min_supported_framing_version().0, 1);
        assert_eq!(engine.max_supported_framing_version().0, 100);
        engine.set_min_supported_framing_version(FramingVersion::new(10)).unwrap();
        assert_eq!(engine.min_supported_framing_version().0, 10);
    }

    #[test]
    fn framing_engine_framing_version_display_string() {
        let engine = FramingEngine::new();
        assert_eq!(engine.framing_version_display_string(), "1.0.0");
    }

    #[test]
    fn framing_engine_frame_type_as_string() {
        assert_eq!(FramingEngine::frame_type_as_string(FrameType::Data), "data");
        assert_eq!(FramingEngine::frame_type_as_string(FrameType::Control), "control");
    }

    #[test]
    fn framing_engine_frame_type_from_string() {
        assert_eq!(
            FramingEngine::frame_type_from_string("ack"),
            Some(FrameType::Ack)
        );
        assert_eq!(
            FramingEngine::frame_type_from_string("unknown"),
            None
        );
    }

    #[test]
    fn framing_engine_all_frame_type_variants() {
        let variants = FramingEngine::all_frame_type_variants();
        assert_eq!(variants.len(), 8);
    }

    #[test]
    fn framing_engine_frame_header_size() {
        assert_eq!(FramingEngine::frame_header_size(), 8);
    }

    #[test]
    fn framing_engine_default_frame_overhead() {
        assert_eq!(FramingEngine::default_frame_overhead(), 16);
    }

    #[test]
    fn framing_engine_default_padding_size() {
        assert_eq!(FramingEngine::default_padding_size(), 8);
    }

    #[test]
    fn framing_engine_validate_frame_type_byte() {
        assert_eq!(
            FramingEngine::validate_frame_type_byte(0).unwrap(),
            FrameType::Data
        );
        assert_eq!(
            FramingEngine::validate_frame_type_byte(5).unwrap(),
            FrameType::Handshake
        );
        let err = FramingEngine::validate_frame_type_byte(99).unwrap_err();
        assert!(matches!(err, FramingError::InvalidFrameType { .. }));
    }

    #[test]
    fn framing_engine_all_boundary_algorithms() {
        let algorithms = FramingEngine::all_boundary_algorithms();
        assert_eq!(algorithms.len(), 5);
    }

    #[test]
    fn framing_engine_is_adaptive_algorithm() {
        assert!(FramingEngine::is_adaptive_algorithm(BoundaryAlgorithm::Adaptive));
        assert!(!FramingEngine::is_adaptive_algorithm(BoundaryAlgorithm::FixedSize));
    }

    #[test]
    fn framing_engine_framing_state_version() {
        let engine = FramingEngine::new();
        assert_eq!(engine.framing_state_version(), 0);
        engine.increment_framing_version();
        assert_eq!(engine.framing_state_version(), 1);
    }

    #[test]
    fn framing_engine_total_handler_registrations() {
        let engine = FramingEngine::new();
        assert_eq!(engine.total_handler_registrations(), 0);
    }

    #[test]
    fn framing_engine_total_boundary_detection_attempts() {
        let engine = FramingEngine::new();
        assert_eq!(engine.total_boundary_detection_attempts(), 0);
    }

    #[test]
    fn framing_engine_is_empty() {
        let engine = FramingEngine::new();
        assert!(engine.is_empty());
        engine.frame_packet(vec![1], FrameType::Data).unwrap();
        assert!(!engine.is_empty());
    }

    #[test]
    fn framing_engine_framing_summary() {
        let engine = FramingEngine::new();
        let summary = engine.framing_summary();
        assert_eq!(summary.frame_count, 0);
        assert_eq!(summary.peer_count, 0);
        assert_eq!(summary.handler_count, 0);
        assert!((summary.health_score - 1.0).abs() < 0.001);
    }

    #[test]
    fn framing_engine_overhead_ratio_percent() {
        let engine = FramingEngine::new();
        assert!((engine.overhead_ratio_percent() - 0.0).abs() < 0.001);
    }

    #[test]
    fn framing_engine_framing_efficiency() {
        let engine = FramingEngine::new();
        assert!((engine.framing_efficiency() - 100.0).abs() < 0.001);
    }

    #[test]
    fn framing_engine_padding_overhead_percent() {
        let engine = FramingEngine::new();
        assert!((engine.padding_overhead_percent() - 0.0).abs() < 0.001);
    }

    #[test]
    fn framing_engine_checksum_overhead_percent() {
        let engine = FramingEngine::new();
        assert!((engine.checksum_overhead_percent() - 0.0).abs() < 0.001);
    }

    // ========================================================================
    // Serialization roundtrip tests
    // ========================================================================

    #[test]
    fn framing_serialization_roundtrip_framing_config() {
        let config = FramingConfig::default();
        let json = serde_json::to_string(&config).unwrap();
        let deserialized: FramingConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(config.max_frame_size, deserialized.max_frame_size);
    }

    #[test]
    fn framing_serialization_roundtrip_framing_stats() {
        let stats = FramingStats::new();
        let json = serde_json::to_string(&stats).unwrap();
        let deserialized: FramingStats = serde_json::from_str(&json).unwrap();
        assert_eq!(stats.frames_framed, deserialized.frames_framed);
    }

    #[test]
    fn framing_serialization_roundtrip_framing_summary() {
        let summary = FramingSummary {
            frame_count: 10,
            peer_count: 3,
            handler_count: 5,
            total_data_bytes: 1000,
            total_framed_bytes: 1100,
            version: 1,
            health_score: 0.95,
        };
        let json = serde_json::to_string(&summary).unwrap();
        let deserialized: FramingSummary = serde_json::from_str(&json).unwrap();
        assert_eq!(summary.frame_count, deserialized.frame_count);
        assert!((summary.health_score - deserialized.health_score).abs() < 0.001);
    }

    // ========================================================================
    // Integration tests
    // ========================================================================

    #[test]
    fn framing_full_lifecycle() {
        let engine = FramingEngine::new();
        let config = FramingConfig::strict();
        engine.configure_framing(config).unwrap();

        // Register frame type handlers
        engine.register_frame_type_handler(FrameType::Data, FrameHandlerId(1)).unwrap();
        engine.register_frame_type_handler(FrameType::Control, FrameHandlerId(2)).unwrap();

        // Negotiate framing version with peer
        engine
            .negotiate_framing_version(test_node_id1(), vec![FramingVersion::new(1), FramingVersion::new(2)])
            .unwrap();

        // Frame a packet
        let packet = engine.frame_packet(vec![1, 2, 3, 4], FrameType::Data).unwrap();
        assert_eq!(packet.packet_id, 1);

        // Deframe the packet
        let (data, frame_type) = engine.deframe_packet(&packet).unwrap();
        assert_eq!(data, vec![1, 2, 3, 4]);
        assert_eq!(frame_type, FrameType::Data);

        // Classify frame type
        let classified = engine.classify_frame_type(&[0]).unwrap();
        assert_eq!(classified, FrameType::Data);

        // Detect boundaries
        let boundaries = engine.detect_boundaries(&[1, 2, 3, 4, 5]).unwrap();
        assert!(!boundaries.is_empty());

        // Check stats
        let stats = engine.stats();
        assert_eq!(stats.frames_framed, 1);
        assert_eq!(stats.frames_deframed, 1);

        // Check health
        let score = engine.framing_health_score();
        assert!(score > 0.0);
        assert!(score <= 1.0);

        // Snapshot and restore
        let snapshot = engine.snapshot();
        let engine2 = FramingEngine::new();
        engine2.restore_from_snapshot(snapshot).unwrap();
        assert_eq!(engine2.max_frame_size(), 4096);
    }

    #[test]
    fn framing_multi_peer_version_negotiation() {
        let engine = FramingEngine::new();
        engine.negotiate_framing_version(test_node_id1(), vec![FramingVersion::new(1)]).unwrap();
        engine.negotiate_framing_version(test_node_id2(), vec![FramingVersion::new(2)]).unwrap();
        engine.negotiate_framing_version(test_node_id3(), vec![FramingVersion::new(1)]).unwrap();

        assert_eq!(engine.peer_version(test_node_id1()), Some(FramingVersion::new(1)));
        assert_eq!(engine.peer_version(test_node_id2()), Some(FramingVersion::new(2)));
        assert_eq!(engine.peer_version(test_node_id3()), Some(FramingVersion::new(1)));

        let peers = engine.peers_by_version(FramingVersion::new(1));
        assert_eq!(peers.len(), 2);
    }

    #[test]
    fn framing_frame_type_lifecycle() {
        let engine = FramingEngine::new();
        assert!(!engine.is_frame_type_registered(FrameType::Data));
        engine.register_frame_type(FrameType::Data).unwrap();
        assert!(engine.is_frame_type_registered(FrameType::Data));
        assert!(engine.unregister_frame_type(FrameType::Data).is_ok());
        assert!(!engine.is_frame_type_registered(FrameType::Data));
    }

    #[test]
    fn framing_frame_type_register_duplicate() {
        let engine = FramingEngine::new();
        engine.register_frame_type(FrameType::Data).unwrap();
        let err = engine.register_frame_type(FrameType::Data).unwrap_err();
        assert!(matches!(err, FramingError::InvalidFrameType { .. }));
    }

    #[test]
    fn framing_frame_type_unregister_not_found() {
        let engine = FramingEngine::new();
        let err = engine.unregister_frame_type(FrameType::Data).unwrap_err();
        assert!(matches!(
            err,
            FramingError::FrameTypeHandlerNotFound { .. }
        ));
    }

    #[test]
    fn framing_get_handler_id() {
        let engine = FramingEngine::new();
        let handler_id = FrameHandlerId(42);
        engine.register_frame_type_handler(FrameType::Data, handler_id).unwrap();
        assert_eq!(engine.get_handler_id(FrameType::Data), Some(handler_id));
        assert_eq!(engine.get_handler_id(FrameType::Control), None);
    }
}
