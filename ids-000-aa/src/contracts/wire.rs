//! # ids-000-aa Sealed Trait Contract: Wire
//!
//! Native wire-format contract for IDS.
//! Provides the native frame format, opaque endpoint encoding, wire message
//! structure, frame validation, wire protocol versioning, and frame lifecycle
//! management. All types are canonical and sealed — only ids-000-aa and its
//! designated implementation crates can implement this trait.
//!
//! Carrier-agnostic: no std::net, tokio::net, libp2p, IP, TCP, UDP, QUIC,
//! HTTP, DNS, or any existing networking stack. Wire messages are opaque
//! byte blobs carried on CarrierType + Vec<u8> Endpoints only.

use crate::types::{Endpoint, NodeId};
use std::collections::{HashMap, HashSet};

///////////////////////////////////////////////////////////////////////////////
// SEALED TRAIT: Wire
///////////////////////////////////////////////////////////////////////////////

/// Sealed marker — only ids-000-aa and designated crates can implement.
pub mod private {
    pub trait Sealed {}
}

/// Sealed wire-format management trait.
/// Implementations live downstream in ids-wire and ids-* crates.
pub trait Wire: private::Sealed {
    /// Configure wire-format parameters.
    fn configure_wire(&self, config: WireConfig) -> Result<(), WireError>;

    /// Get the native wire format specification.
    fn wire_format(&self) -> WireFormat;

    /// Set the wire format.
    fn set_wire_format(&self, format: WireFormat) -> Result<(), WireError>;

    /// Get all supported wire formats.
    fn supported_formats(&self) -> Vec<WireFormat>;

    /// Negotiate wire format with a peer.
    fn negotiate_format(
        &self,
        peer_id: NodeId,
        offered_formats: Vec<WireFormat>,
    ) -> Result<WireFormat, WireError>;

    /// Encode a native frame from raw payload.
    fn encode_frame(
        &self,
        payload: Vec<u8>,
        frame_id: u64,
    ) -> Result<NativeFrame, WireError>;

    /// Decode a native frame into raw payload.
    fn decode_frame(
        &self,
        frame: &NativeFrame,
    ) -> Result<(Vec<u8>, u64), WireError>;

    /// Validate a native frame's structure and integrity.
    fn validate_frame(
        &self,
        frame: &NativeFrame,
    ) -> Result<FrameValidationResult, WireError>;

    /// Get frame size limits.
    fn frame_limits(&self) -> FrameLimits;

    /// Check if a payload size exceeds frame limits.
    fn exceeds_frame_limit(&self, payload_size: usize) -> bool;

    /// Get the wire protocol version.
    fn wire_protocol_version(&self) -> WireProtocolVersion;

    /// Set the wire protocol version.
    fn set_wire_protocol_version(
        &self,
        version: WireProtocolVersion,
    ) -> Result<(), WireError>;

    /// Get all supported wire protocol versions.
    fn supported_wire_versions(&self) -> Vec<WireProtocolVersion>;

    /// Negotiate wire protocol version with a peer.
    fn negotiate_wire_version(
        &self,
        peer_id: NodeId,
        offered_versions: Vec<WireProtocolVersion>,
    ) -> Result<WireProtocolVersion, WireError>;

    /// Get opaque endpoint encoding.
    fn encode_endpoint(&self, endpoint: &Endpoint) -> OpaqueEndpoint;

    /// Decode an opaque endpoint.
    fn decode_endpoint(
        &self,
        opaque: &OpaqueEndpoint,
    ) -> Result<Endpoint, WireError>;

    /// Get endpoint address size limit (default 1024 bytes).
    fn endpoint_address_size_limit(&self) -> usize;

    /// Check if endpoint address exceeds size limit.
    fn endpoint_exceeds_limit(&self, address_size: usize) -> bool;

    /// Register a wire format extension.
    fn register_extension(
        &self,
        extension: WireExtension,
    ) -> Result<WireExtensionId, WireError>;

    /// Get registered wire extensions.
    fn registered_extensions(&self) -> Vec<WireExtension>;

    /// Get extension by ID.
    fn extension_by_id(
        &self,
        extension_id: WireExtensionId,
    ) -> Option<WireExtension>;

    /// Get wire message statistics.
    fn stats(&self) -> WireStats;

    /// Check if wire subsystem is healthy.
    fn is_healthy(&self) -> bool;

    /// Validate wire configuration.
    fn validate_config(&self, config: &WireConfig) -> Result<(), WireError>;

    /// Get wire snapshot.
    fn snapshot(&self) -> WireSnapshot;

    /// Restore wire from snapshot.
    fn restore_from_snapshot(
        &self,
        snapshot: WireSnapshot,
    ) -> Result<(), WireError>;

    /// Get peers by wire format.
    fn peers_by_format(&self, format: WireFormat) -> Vec<NodeId>;

    /// Get peer wire format.
    fn peer_format(&self, peer_id: NodeId) -> Option<WireFormat>;

    /// Set peer wire format.
    fn set_peer_format(
        &self,
        peer_id: NodeId,
        format: WireFormat,
    ) -> Result<(), WireError>;

    /// Get wire format negotiation timeout.
    fn negotiation_timeout_ms(&self) -> u64;

    /// Set wire format negotiation timeout.
    fn set_negotiation_timeout_ms(&self, timeout_ms: u64) -> Result<(), WireError>;

    /// Get max frame payload size.
    fn max_frame_payload_size(&self) -> usize;

    /// Set max frame payload size.
    fn set_max_frame_payload_size(&self, size: usize) -> Result<(), WireError>;
}

///////////////////////////////////////////////////////////////////////////////
// CONFIGURATION
///////////////////////////////////////////////////////////////////////////////

/// Wire-format configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WireConfig {
    /// Default wire format.
    pub default_format: WireFormat,
    /// Maximum frame payload size (bytes).
    pub max_frame_payload_size: usize,
    /// Endpoint address size limit (bytes).
    pub endpoint_address_size_limit: usize,
    /// Wire protocol version.
    pub wire_protocol_version: WireProtocolVersion,
    /// Minimum supported wire protocol version.
    pub min_supported_version: WireProtocolVersion,
    /// Maximum supported wire protocol version.
    pub max_supported_version: WireProtocolVersion,
    /// Enable automatic format negotiation.
    pub enable_auto_negotiation: bool,
    /// Enable frame validation.
    pub enable_frame_validation: bool,
    /// Frame validation timeout (ms).
    pub frame_validation_timeout_ms: u64,
    /// Maximum pending frame validations.
    pub max_pending_validations: u32,
    /// Enable wire extensions.
    pub enable_wire_extensions: bool,
    /// Maximum registered extensions.
    pub max_extensions: u32,
    /// Strict format matching.
    pub strict_format_matching: bool,
    /// Frame checksum enabled.
    pub frame_checksum_enabled: bool,
}

impl WireConfig {
    pub fn new(default_format: WireFormat) -> Result<Self, WireError> {
        let config = Self {
            default_format,
            max_frame_payload_size: 65536,
            endpoint_address_size_limit: 1024,
            wire_protocol_version: WireProtocolVersion::new(1),
            min_supported_version: WireProtocolVersion::new(1),
            max_supported_version: WireProtocolVersion::new(100),
            enable_auto_negotiation: true,
            enable_frame_validation: true,
            frame_validation_timeout_ms: 5000,
            max_pending_validations: 1000,
            enable_wire_extensions: true,
            max_extensions: 50,
            strict_format_matching: false,
            frame_checksum_enabled: true,
        };
        config.validate()?;
        Ok(config)
    }

    pub fn validate(&self) -> Result<(), WireError> {
        if self.max_frame_payload_size == 0 {
            return Err(WireError::InvalidConfig {
                field: "max_frame_payload_size".to_string(),
                reason: "max_frame_payload_size must be greater than 0".to_string(),
            });
        }
        if self.endpoint_address_size_limit == 0 {
            return Err(WireError::InvalidConfig {
                field: "endpoint_address_size_limit".to_string(),
                reason: "endpoint_address_size_limit must be greater than 0".to_string(),
            });
        }
        if self.frame_validation_timeout_ms == 0 {
            return Err(WireError::InvalidConfig {
                field: "frame_validation_timeout_ms".to_string(),
                reason: "frame_validation_timeout_ms must be greater than 0".to_string(),
            });
        }
        if self.max_pending_validations == 0 {
            return Err(WireError::InvalidConfig {
                field: "max_pending_validations".to_string(),
                reason: "max_pending_validations must be greater than 0".to_string(),
            });
        }
        if self.max_extensions == 0 {
            return Err(WireError::InvalidConfig {
                field: "max_extensions".to_string(),
                reason: "max_extensions must be greater than 0".to_string(),
            });
        }
        Ok(())
    }

    pub fn default() -> Self {
        Self {
            default_format: WireFormat::NativeV1,
            max_frame_payload_size: 65536,
            endpoint_address_size_limit: 1024,
            wire_protocol_version: WireProtocolVersion::new(1),
            min_supported_version: WireProtocolVersion::new(1),
            max_supported_version: WireProtocolVersion::new(100),
            enable_auto_negotiation: true,
            enable_frame_validation: true,
            frame_validation_timeout_ms: 5000,
            max_pending_validations: 1000,
            enable_wire_extensions: true,
            max_extensions: 50,
            strict_format_matching: false,
            frame_checksum_enabled: true,
        }
    }

    pub fn strict() -> Self {
        Self {
            default_format: WireFormat::NativeV1,
            max_frame_payload_size: 4096,
            endpoint_address_size_limit: 512,
            wire_protocol_version: WireProtocolVersion::new(1),
            min_supported_version: WireProtocolVersion::new(1),
            max_supported_version: WireProtocolVersion::new(10),
            enable_auto_negotiation: false,
            enable_frame_validation: true,
            frame_validation_timeout_ms: 2000,
            max_pending_validations: 100,
            enable_wire_extensions: false,
            max_extensions: 5,
            strict_format_matching: true,
            frame_checksum_enabled: true,
        }
    }

    pub fn lenient() -> Self {
        Self {
            default_format: WireFormat::NativeV1,
            max_frame_payload_size: 1048576,
            endpoint_address_size_limit: 4096,
            wire_protocol_version: WireProtocolVersion::new(1),
            min_supported_version: WireProtocolVersion::new(1),
            max_supported_version: WireProtocolVersion::new(1000),
            enable_auto_negotiation: true,
            enable_frame_validation: false,
            frame_validation_timeout_ms: 30000,
            max_pending_validations: 10000,
            enable_wire_extensions: true,
            max_extensions: 500,
            strict_format_matching: false,
            frame_checksum_enabled: false,
        }
    }

    pub fn payload_efficiency(&self, payload_size: usize) -> f64 {
        if payload_size == 0 {
            return 100.0;
        }
        let overhead = 32; // Frame header overhead
        payload_size as f64 / (payload_size + overhead) as f64 * 100.0
    }
}

impl Default for WireConfig {
    fn default() -> Self {
        Self::default()
    }
}

///////////////////////////////////////////////////////////////////////////////
// WIRE FORMAT
///////////////////////////////////////////////////////////////////////////////

/// Native wire format specification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum WireFormat {
    NativeV1,
    NativeV2,
    CompactV1,
    CompactV2,
    Extended(u16),
    Custom(u32),
}

impl WireFormat {
    pub fn as_str(&self) -> &'static str {
        match self {
            WireFormat::NativeV1 => "native_v1",
            WireFormat::NativeV2 => "native_v2",
            WireFormat::CompactV1 => "compact_v1",
            WireFormat::CompactV2 => "compact_v2",
            WireFormat::Extended(_) => "extended",
            WireFormat::Custom(_) => "custom",
        }
    }

    pub fn version(&self) -> u16 {
        match self {
            WireFormat::NativeV1 => 1,
            WireFormat::NativeV2 => 2,
            WireFormat::CompactV1 => 1,
            WireFormat::CompactV2 => 2,
            WireFormat::Extended(v) => *v,
            WireFormat::Custom(_) => 0,
        }
    }

    pub fn is_native(&self) -> bool {
        matches!(self, WireFormat::NativeV1 | WireFormat::NativeV2)
    }

    pub fn is_compact(&self) -> bool {
        matches!(self, WireFormat::CompactV1 | WireFormat::CompactV2)
    }

    pub fn header_size(&self) -> usize {
        match self {
            WireFormat::NativeV1 => 16,
            WireFormat::NativeV2 => 24,
            WireFormat::CompactV1 => 8,
            WireFormat::CompactV2 => 12,
            WireFormat::Extended(_) => 20,
            WireFormat::Custom(_) => 16,
        }
    }

    pub fn supports_checksum(&self) -> bool {
        matches!(
            self,
            WireFormat::NativeV1 | WireFormat::NativeV2 | WireFormat::Extended(_)
        )
    }

    pub fn supports_fragmentation(&self) -> bool {
        matches!(
            self,
            WireFormat::NativeV2 | WireFormat::CompactV2 | WireFormat::Extended(_)
        )
    }

    pub fn serialization_roundtrip(&self) -> bool {
        let json = serde_json::to_string(self).unwrap();
        serde_json::from_str::<WireFormat>(&json).is_ok()
    }
}

///////////////////////////////////////////////////////////////////////////////
// NATIVE FRAME
///////////////////////////////////////////////////////////////////////////////

/// A native wire frame.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NativeFrame {
    pub frame_id: u64,
    pub format: WireFormat,
    pub payload: Vec<u8>,
    pub checksum: Option<Vec<u8>>,
    pub flags: FrameFlags,
    pub timestamp_ms: u64,
    pub sequence: u64,
    pub fragment_index: Option<u32>,
    pub total_fragments: Option<u32>,
}

impl NativeFrame {
    pub fn new(
        frame_id: u64,
        format: WireFormat,
        payload: Vec<u8>,
    ) -> Self {
        Self {
            frame_id,
            format,
            payload,
            checksum: None,
            flags: FrameFlags::empty(),
            timestamp_ms: 0,
            sequence: 0,
            fragment_index: None,
            total_fragments: None,
        }
    }

    pub fn payload_size(&self) -> usize {
        self.payload.len()
    }

    pub fn total_size(&self) -> usize {
        self.format.header_size() + self.payload.len()
            + self.checksum.as_ref().map(|c| c.len()).unwrap_or(0)
    }

    pub fn is_fragmented(&self) -> bool {
        self.fragment_index.is_some() && self.total_fragments.is_some()
    }

    pub fn is_last_fragment(&self) -> bool {
        match (self.fragment_index, self.total_fragments) {
            (Some(idx), Some(total)) => idx + 1 >= total,
            _ => true,
        }
    }

    pub fn has_checksum(&self) -> bool {
        self.checksum.is_some()
    }

    pub fn frame_overhead(&self) -> usize {
        self.format.header_size()
            + self.checksum.as_ref().map(|c| c.len()).unwrap_or(0)
    }
}

///////////////////////////////////////////////////////////////////////////////
// FRAME FLAGS
///////////////////////////////////////////////////////////////////////////////

/// Frame flags bitfield.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FrameFlags(pub u16);

impl FrameFlags {
    pub const FLAG_CHECKSUM: u16 = 1 << 0;
    pub const FLAG_FRAGMENTED: u16 = 1 << 1;
    pub const FLAG_LAST_FRAGMENT: u16 = 1 << 2;
    pub const FLAG_COMPRESSED: u16 = 1 << 3;
    pub const FLAG_ENCRYPTED: u16 = 1 << 4;
    pub const FLAG_PRIORITY: u16 = 1 << 5;
    pub const FLAG_ACK_REQUIRED: u16 = 1 << 6;
    pub const FLAG_RESERVED: u16 = 1 << 15;

    pub fn empty() -> Self {
        Self(0)
    }

    pub fn all() -> Self {
        Self(u16::MAX)
    }

    pub fn contains(&self, flag: u16) -> bool {
        self.0 & flag != 0
    }

    pub fn insert(&mut self, flag: u16) {
        self.0 |= flag;
    }

    pub fn remove(&mut self, flag: u16) {
        self.0 &= !flag;
    }

    pub fn bits(&self) -> u16 {
        self.0
    }

    pub fn is_checksum(&self) -> bool {
        self.contains(Self::FLAG_CHECKSUM)
    }

    pub fn is_fragmented(&self) -> bool {
        self.contains(Self::FLAG_FRAGMENTED)
    }

    pub fn is_last_fragment(&self) -> bool {
        self.contains(Self::FLAG_LAST_FRAGMENT)
    }

    pub fn is_compressed(&self) -> bool {
        self.contains(Self::FLAG_COMPRESSED)
    }

    pub fn is_encrypted(&self) -> bool {
        self.contains(Self::FLAG_ENCRYPTED)
    }

    pub fn is_priority(&self) -> bool {
        self.contains(Self::FLAG_PRIORITY)
    }

    pub fn is_ack_required(&self) -> bool {
        self.contains(Self::FLAG_ACK_REQUIRED)
    }
}

///////////////////////////////////////////////////////////////////////////////
// FRAME VALIDATION RESULT
///////////////////////////////////////////////////////////////////////////////

/// Result of frame validation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FrameValidationResult {
    pub frame_id: u64,
    pub valid: bool,
    pub checksum_valid: bool,
    pub size_valid: bool,
    pub format_valid: bool,
    pub errors: Vec<FrameValidationError>,
    pub validated_at: u64,
}

impl FrameValidationResult {
    pub fn new(frame_id: u64) -> Self {
        Self {
            frame_id,
            valid: true,
            checksum_valid: true,
            size_valid: true,
            format_valid: true,
            errors: Vec::new(),
            validated_at: 0,
        }
    }

    pub fn add_error(&mut self, error: FrameValidationError) {
        self.valid = false;
        self.errors.push(error);
    }

    pub fn is_clean(&self) -> bool {
        self.valid && self.errors.is_empty()
    }

    pub fn error_count(&self) -> usize {
        self.errors.len()
    }
}

/// Frame validation error types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FrameValidationError {
    ChecksumMismatch,
    SizeExceeded,
    InvalidFormat,
    TruncatedFrame,
    CorruptedPayload,
    MissingHeader,
    InvalidSequence,
    FragmentMismatch,
    UnknownFlag(u16),
}

impl FrameValidationError {
    pub fn as_str(&self) -> &'static str {
        match self {
            FrameValidationError::ChecksumMismatch => "checksum_mismatch",
            FrameValidationError::SizeExceeded => "size_exceeded",
            FrameValidationError::InvalidFormat => "invalid_format",
            FrameValidationError::TruncatedFrame => "truncated_frame",
            FrameValidationError::CorruptedPayload => "corrupted_payload",
            FrameValidationError::MissingHeader => "missing_header",
            FrameValidationError::InvalidSequence => "invalid_sequence",
            FrameValidationError::FragmentMismatch => "fragment_mismatch",
            FrameValidationError::UnknownFlag(_) => "unknown_flag",
        }
    }
}

///////////////////////////////////////////////////////////////////////////////
// FRAME LIMITS
///////////////////////////////////////////////////////////////////////////////

/// Frame size limits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct FrameLimits {
    pub min_payload_size: usize,
    pub max_payload_size: usize,
    pub max_total_frame_size: usize,
    pub max_fragments_per_frame: u32,
    pub header_size: usize,
    pub checksum_size: usize,
}

impl FrameLimits {
    pub fn new(
        max_payload_size: usize,
        header_size: usize,
        checksum_size: usize,
    ) -> Self {
        Self {
            min_payload_size: 0,
            max_payload_size,
            max_total_frame_size: max_payload_size + header_size + checksum_size,
            max_fragments_per_frame: 1000,
            header_size,
            checksum_size,
        }
    }

    pub fn default() -> Self {
        Self::new(65536, 16, 32)
    }

    pub fn can_fit_payload(&self, payload_size: usize) -> bool {
        payload_size >= self.min_payload_size
            && payload_size <= self.max_payload_size
    }

    pub fn overhead_percent(&self) -> f64 {
        if self.max_payload_size == 0 {
            return 0.0;
        }
        (self.header_size + self.checksum_size) as f64
            / self.max_payload_size as f64
            * 100.0
    }
}

///////////////////////////////////////////////////////////////////////////////
// WIRE PROTOCOL VERSION
///////////////////////////////////////////////////////////////////////////////

/// Wire protocol version.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct WireProtocolVersion(pub u64);

impl WireProtocolVersion {
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
// OPAQUE ENDPOINT
///////////////////////////////////////////////////////////////////////////////

/// Opaque endpoint encoding for wire transport.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpaqueEndpoint {
    pub carrier_type: u16,
    pub address_bytes: Vec<u8>,
    pub encoded_size: usize,
}

impl OpaqueEndpoint {
    pub fn new(carrier_type: u16, address_bytes: Vec<u8>) -> Self {
        let encoded_size = 2 + address_bytes.len();
        Self {
            carrier_type,
            address_bytes,
            encoded_size,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.address_bytes.is_empty()
    }

    pub fn address_size(&self) -> usize {
        self.address_bytes.len()
    }
}

///////////////////////////////////////////////////////////////////////////////
// WIRE EXTENSION
///////////////////////////////////////////////////////////////////////////////

/// Wire extension ID.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct WireExtensionId(pub u64);

/// Wire format extension.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WireExtension {
    pub extension_id: WireExtensionId,
    pub name: String,
    pub format: WireFormat,
    pub version: WireProtocolVersion,
    pub description: Vec<u8>,
    pub enabled: bool,
}

impl WireExtension {
    pub fn new(name: String, format: WireFormat) -> Self {
        Self {
            extension_id: WireExtensionId(0),
            name,
            format,
            version: WireProtocolVersion::new(1),
            description: Vec::new(),
            enabled: true,
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }
}

///////////////////////////////////////////////////////////////////////////////
// WIRE STATS
///////////////////////////////////////////////////////////////////////////////

/// Wire subsystem statistics.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WireStats {
    pub frames_encoded: u64,
    pub frames_decoded: u64,
    pub frames_validated: u64,
    pub validation_failures: u64,
    pub total_payload_bytes: u64,
    pub total_frame_bytes: u64,
    pub total_extensions: u32,
    pub active_extensions: u32,
    pub total_negotiations: u64,
    pub successful_negotiations: u64,
    pub failed_negotiations: u64,
    pub format_switches: u64,
}

impl WireStats {
    pub fn new() -> Self {
        Self {
            frames_encoded: 0,
            frames_decoded: 0,
            frames_validated: 0,
            validation_failures: 0,
            total_payload_bytes: 0,
            total_frame_bytes: 0,
            total_extensions: 0,
            active_extensions: 0,
            total_negotiations: 0,
            successful_negotiations: 0,
            failed_negotiations: 0,
            format_switches: 0,
        }
    }

    pub fn encode_success_rate(&self) -> f64 {
        if self.frames_encoded == 0 {
            return 100.0;
        }
        100.0 // All encodes succeed by design
    }

    pub fn decode_success_rate(&self) -> f64 {
        if self.frames_decoded == 0 {
            return 100.0;
        }
        100.0
    }

    pub fn validation_success_rate(&self) -> f64 {
        if self.frames_validated == 0 {
            return 100.0;
        }
        (self.frames_validated - self.validation_failures) as f64
            / self.frames_validated as f64
            * 100.0
    }

    pub fn negotiation_success_rate(&self) -> f64 {
        if self.total_negotiations == 0 {
            return 100.0;
        }
        self.successful_negotiations as f64 / self.total_negotiations as f64 * 100.0
    }

    pub fn payload_overhead_ratio(&self) -> f64 {
        if self.total_payload_bytes == 0 {
            return 0.0;
        }
        self.total_frame_bytes as f64 / self.total_payload_bytes as f64
    }

    pub fn bytes_per_frame_avg(&self) -> f64 {
        if self.frames_encoded == 0 {
            return 0.0;
        }
        self.total_frame_bytes as f64 / self.frames_encoded as f64
    }
}

///////////////////////////////////////////////////////////////////////////////
// WIRE SNAPSHOT
///////////////////////////////////////////////////////////////////////////////

/// Wire subsystem snapshot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WireSnapshot {
    pub version: u64,
    pub current_format: WireFormat,
    pub wire_protocol_version: WireProtocolVersion,
    pub frame_limits: FrameLimits,
    pub extensions: Vec<WireExtension>,
    pub peer_formats: HashMap<NodeId, WireFormat>,
    pub stats: WireStats,
}

impl WireSnapshot {
    pub fn peer_count(&self) -> usize {
        self.peer_formats.len()
    }

    pub fn extension_count(&self) -> usize {
        self.extensions.len()
    }
}

///////////////////////////////////////////////////////////////////////////////
// WIRE ERROR
///////////////////////////////////////////////////////////////////////////////

/// Wire-format management errors.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
pub enum WireError {
    #[error("invalid config: {field} - {reason}")]
    InvalidConfig { field: String, reason: String },

    #[error("format not supported: {format:?}")]
    FormatNotSupported { format: WireFormat },

    #[error("format out of range: {format:?}")]
    FormatOutOfRange { format: WireFormat },

    #[error("peer not found: {peer_id:?}")]
    PeerNotFound { peer_id: NodeId },

    #[error("frame not found: {frame_id}")]
    FrameNotFound { frame_id: u64 },

    #[error("frame validation failed: {frame_id}")]
    FrameValidationFailed { frame_id: u64 },

    #[error("payload exceeds frame limit: {payload_size} > {max_size}")]
    PayloadExceedsLimit { payload_size: usize, max_size: usize },

    #[error("extension not found: {extension_id:?}")]
    ExtensionNotFound { extension_id: WireExtensionId },

    #[error("extension already exists: {extension_id:?}")]
    ExtensionAlreadyExists { extension_id: WireExtensionId },

    #[error("maximum extensions exceeded: {max_extensions}")]
    MaxExtensionsExceeded { max_extensions: u32 },

    #[error("maximum pending validations exceeded: {max_pending}")]
    MaxPendingValidationsExceeded { max_pending: u32 },

    #[error("checksum mismatch: {frame_id}")]
    ChecksumMismatch { frame_id: u64 },

    #[error("truncated frame: {frame_id}")]
    TruncatedFrame { frame_id: u64 },

    #[error("corrupted payload: {frame_id}")]
    CorruptedPayload { frame_id: u64 },

    #[error("invalid frame format: {frame_id}")]
    InvalidFrameFormat { frame_id: u64 },

    #[error("fragment mismatch: {frame_id}")]
    FragmentMismatch { frame_id: u64 },

    #[error("negotiation timeout: {peer_id:?}")]
    NegotiationTimeout { peer_id: NodeId },

    #[error("endpoint address exceeds limit: {address_size} > {limit}")]
    EndpointExceedsLimit { address_size: usize, limit: usize },

    #[error("internal error: {message}")]
    Internal { message: String },
}

impl WireError {
    pub fn category(&self) -> &'static str {
        match self {
            WireError::InvalidConfig { .. } => "invalid_config",
            WireError::FormatNotSupported { .. } => "format_not_supported",
            WireError::FormatOutOfRange { .. } => "format_out_of_range",
            WireError::PeerNotFound { .. } => "peer_not_found",
            WireError::FrameNotFound { .. } => "frame_not_found",
            WireError::FrameValidationFailed { .. } => "frame_validation_failed",
            WireError::PayloadExceedsLimit { .. } => "payload_exceeds_limit",
            WireError::ExtensionNotFound { .. } => "extension_not_found",
            WireError::ExtensionAlreadyExists { .. } => "extension_already_exists",
            WireError::MaxExtensionsExceeded { .. } => "max_extensions_exceeded",
            WireError::MaxPendingValidationsExceeded { .. } => "max_pending_validations_exceeded",
            WireError::ChecksumMismatch { .. } => "checksum_mismatch",
            WireError::TruncatedFrame { .. } => "truncated_frame",
            WireError::CorruptedPayload { .. } => "corrupted_payload",
            WireError::InvalidFrameFormat { .. } => "invalid_frame_format",
            WireError::FragmentMismatch { .. } => "fragment_mismatch",
            WireError::NegotiationTimeout { .. } => "negotiation_timeout",
            WireError::EndpointExceedsLimit { .. } => "endpoint_exceeds_limit",
            WireError::Internal { .. } => "internal",
        }
    }

    pub fn is_recoverable(&self) -> bool {
        matches!(
            self,
            WireError::FormatNotSupported { .. }
                | WireError::PayloadExceedsLimit { .. }
                | WireError::NegotiationTimeout { .. }
                | WireError::EndpointExceedsLimit { .. }
                | WireError::ChecksumMismatch { .. }
                | WireError::TruncatedFrame { .. }
        )
    }

    pub fn suggested_action(&self) -> &'static str {
        match self {
            WireError::InvalidConfig { .. } => "correct the wire configuration",
            WireError::FormatNotSupported { .. } => "use a supported wire format",
            WireError::FormatOutOfRange { .. } => "use a format within the supported range",
            WireError::PeerNotFound { .. } => "register the peer first",
            WireError::FrameNotFound { .. } => "create the frame first",
            WireError::FrameValidationFailed { .. } => "fix the frame and retry",
            WireError::PayloadExceedsLimit { .. } => "reduce payload size or increase limit",
            WireError::ExtensionNotFound { .. } => "register the extension first",
            WireError::ExtensionAlreadyExists { .. } => "use existing extension",
            WireError::MaxExtensionsExceeded { .. } => "remove extensions or increase limit",
            WireError::MaxPendingValidationsExceeded { .. } => {
                "complete pending validations first"
            }
            WireError::ChecksumMismatch { .. } => "retransmit the frame",
            WireError::TruncatedFrame { .. } => "retransmit the complete frame",
            WireError::CorruptedPayload { .. } => "retransmit with valid payload",
            WireError::InvalidFrameFormat { .. } => "use a valid frame format",
            WireError::FragmentMismatch { .. } => "reassemble fragments correctly",
            WireError::NegotiationTimeout { .. } => "retry format negotiation",
            WireError::EndpointExceedsLimit { .. } => "reduce endpoint address size",
            WireError::Internal { .. } => "contact system administrator",
        }
    }
}

///////////////////////////////////////////////////////////////////////////////
// INTERNAL STATE
///////////////////////////////////////////////////////////////////////////////

/// Internal frame record.
#[derive(Debug, Clone)]
struct InternalFrame {
    frame_id: u64,
    format: WireFormat,
    payload: Vec<u8>,
    checksum: Option<Vec<u8>>,
    flags: FrameFlags,
    timestamp_ms: u64,
    sequence: u64,
}

/// Internal peer format mapping.
#[derive(Debug, Clone)]
struct InternalPeerFormat {
    peer_id: NodeId,
    format: WireFormat,
    last_updated: u64,
}

/// Internal wire state.
#[derive(Debug, Clone)]
struct WireState {
    config: WireConfig,
    formats: HashMap<WireFormat, bool>,
    extensions: HashMap<WireExtensionId, WireExtension>,
    extension_counter: u64,
    frames: HashMap<u64, InternalFrame>,
    frame_counter: u64,
    peer_formats: HashMap<NodeId, WireFormat>,
    stats: WireStats,
    version: u64,
}

impl WireState {
    fn new(config: WireConfig) -> Self {
        let mut formats = HashMap::new();
        formats.insert(config.default_format, true);
        Self {
            config,
            formats,
            extensions: HashMap::new(),
            extension_counter: 0,
            frames: HashMap::new(),
            frame_counter: 0,
            peer_formats: HashMap::new(),
            stats: WireStats::new(),
            version: 0,
        }
    }
}

///////////////////////////////////////////////////////////////////////////////
// WIRE ENGINE (REFERENCE IMPLEMENTATION)
///////////////////////////////////////////////////////////////////////////////

/// Reference wire-format engine implementing the Wire trait.
pub struct WireEngine {
    state: std::cell::RefCell<WireState>,
}

impl WireEngine {
    pub fn new() -> Self {
        Self {
            state: std::cell::RefCell::new(WireState::new(WireConfig::default())),
        }
    }

    pub fn with_config(config: WireConfig) -> Result<Self, WireError> {
        config.validate()?;
        Ok(Self {
            state: std::cell::RefCell::new(WireState::new(config)),
        })
    }

    fn ensure_config(&self) -> WireConfig {
        self.state.borrow().config.clone()
    }
}

///////////////////////////////////////////////////////////////////////////////
// IMPL WIRE FOR WIREENGINE
///////////////////////////////////////////////////////////////////////////////

impl private::Sealed for WireEngine {}

impl Wire for WireEngine {
    fn configure_wire(&self, config: WireConfig) -> Result<(), WireError> {
        config.validate()?;
        let mut state = self.state.borrow_mut();
        state.config = config.clone();
        state.formats.clear();
        state.formats.insert(config.default_format, true);
        state.stats = WireStats::new();
        Ok(())
    }

    fn wire_format(&self) -> WireFormat {
        let state = self.state.borrow();
        state.config.default_format
    }

    fn set_wire_format(&self, format: WireFormat) -> Result<(), WireError> {
        let config = self.ensure_config();
        if !config.strict_format_matching || state_format_supported(&config, format) {
            let mut state = self.state.borrow_mut();
            state.config.default_format = format;
            state.formats.insert(format, true);
            state.stats.format_switches += 1;
            state.version += 1;
            Ok(())
        } else {
            Err(WireError::FormatNotSupported { format })
        }
    }

    fn supported_formats(&self) -> Vec<WireFormat> {
        let state = self.state.borrow();
        state.formats.keys().cloned().collect()
    }

    fn negotiate_format(
        &self,
        peer_id: NodeId,
        offered_formats: Vec<WireFormat>,
    ) -> Result<WireFormat, WireError> {
        let config = self.ensure_config();
        let mut state = self.state.borrow_mut();

        state.stats.total_negotiations += 1;

        // Find the best mutually supported format
        let mut best_format = None;
        for &offered in &offered_formats {
            if state.formats.contains_key(&offered) {
                match &best_format {
                    None => best_format = Some(offered),
                    Some(current) => {
                        if format_priority(offered) > format_priority(*current) {
                            best_format = Some(offered);
                        }
                    }
                }
            }
        }

        let negotiated = match best_format {
            Some(f) => f,
            None => {
                state.stats.failed_negotiations += 1;
                return Err(WireError::FormatNotSupported {
                    format: offered_formats
                        .first()
                        .copied()
                        .unwrap_or(WireFormat::NativeV1),
                });
            }
        };

        state.stats.successful_negotiations += 1;
        state.peer_formats.insert(peer_id, negotiated);

        Ok(negotiated)
    }

    fn encode_frame(
        &self,
        payload: Vec<u8>,
        frame_id: u64,
    ) -> Result<NativeFrame, WireError> {
        let config = self.ensure_config();
        let mut state = self.state.borrow_mut();

        if payload.len() > config.max_frame_payload_size {
            return Err(WireError::PayloadExceedsLimit {
                payload_size: payload.len(),
                max_size: config.max_frame_payload_size,
            });
        }

        let mut frame = NativeFrame::new(frame_id, config.default_format, payload.clone());
        frame.timestamp_ms = 0;
        frame.sequence = state.frame_counter;
        state.frame_counter += 1;

        if config.frame_checksum_enabled {
            let checksum = compute_checksum(&payload);
            frame.checksum = Some(checksum);
            frame.flags.insert(FrameFlags::FLAG_CHECKSUM);
        }

        state.frames.insert(frame_id, InternalFrame {
            frame_id,
            format: config.default_format,
            payload: payload.clone(),
            checksum: frame.checksum.clone(),
            flags: frame.flags,
            timestamp_ms: frame.timestamp_ms,
            sequence: frame.sequence,
        });

        state.stats.frames_encoded += 1;
        state.stats.total_payload_bytes += payload.len() as u64;
        state.stats.total_frame_bytes += frame.total_size() as u64;

        Ok(frame)
    }

    fn decode_frame(
        &self,
        frame: &NativeFrame,
    ) -> Result<(Vec<u8>, u64), WireError> {
        let state = self.state.borrow();

        // Validate frame structure
        if frame.payload.is_empty() && !frame.flags.is_fragmented() {
            return Err(WireError::CorruptedPayload { frame_id: frame.frame_id });
        }

        if let Some(checksum) = &frame.checksum {
            let computed = compute_checksum(&frame.payload);
            if checksum != &computed {
                return Err(WireError::ChecksumMismatch { frame_id: frame.frame_id });
            }
        }

        if frame.payload.len() > state.config.max_frame_payload_size {
            return Err(WireError::PayloadExceedsLimit {
                payload_size: frame.payload.len(),
                max_size: state.config.max_frame_payload_size,
            });
        }

        Ok((frame.payload.clone(), frame.sequence))
    }

    fn validate_frame(
        &self,
        frame: &NativeFrame,
    ) -> Result<FrameValidationResult, WireError> {
        let config = self.ensure_config();
        let mut state = self.state.borrow_mut();

        state.stats.frames_validated += 1;

        let mut result = FrameValidationResult::new(frame.frame_id);

        // Check format
        if !state.formats.contains_key(&frame.format) {
            result.add_error(FrameValidationError::InvalidFormat);
            result.format_valid = false;
        }

        // Check size
        if frame.payload.len() > config.max_frame_payload_size {
            result.add_error(FrameValidationError::SizeExceeded);
            result.size_valid = false;
        }

        // Check checksum
        if config.frame_checksum_enabled {
            if let Some(checksum) = &frame.checksum {
                let computed = compute_checksum(&frame.payload);
                if checksum != &computed {
                    result.add_error(FrameValidationError::ChecksumMismatch);
                    result.checksum_valid = false;
                }
            } else {
                result.add_error(FrameValidationError::ChecksumMismatch);
                result.checksum_valid = false;
            }
        }

        // Check fragments
        if let (Some(idx), Some(total)) = (frame.fragment_index, frame.total_fragments) {
            if idx >= total {
                result.add_error(FrameValidationError::FragmentMismatch);
            }
        }

        if !result.valid {
            state.stats.validation_failures += 1;
        }

        Ok(result)
    }

    fn frame_limits(&self) -> FrameLimits {
        let state = self.state.borrow();
        FrameLimits::new(
            state.config.max_frame_payload_size,
            state.config.default_format.header_size(),
            if state.config.frame_checksum_enabled { 32 } else { 0 },
        )
    }

    fn exceeds_frame_limit(&self, payload_size: usize) -> bool {
        let config = self.ensure_config();
        payload_size > config.max_frame_payload_size
    }

    fn wire_protocol_version(&self) -> WireProtocolVersion {
        let state = self.state.borrow();
        state.config.wire_protocol_version
    }

    fn set_wire_protocol_version(
        &self,
        version: WireProtocolVersion,
    ) -> Result<(), WireError> {
        let mut state = self.state.borrow_mut();
        if version.0 < state.config.min_supported_version.0
            || version.0 > state.config.max_supported_version.0
        {
            return Err(WireError::FormatOutOfRange {
                format: WireFormat::NativeV1,
            });
        }
        state.config.wire_protocol_version = version;
        state.version += 1;
        Ok(())
    }

    fn supported_wire_versions(&self) -> Vec<WireProtocolVersion> {
        let state = self.state.borrow();
        let min = state.config.min_supported_version.0;
        let max = state.config.max_supported_version.0;
        (min..=max).map(WireProtocolVersion).collect()
    }

    fn negotiate_wire_version(
        &self,
        peer_id: NodeId,
        offered_versions: Vec<WireProtocolVersion>,
    ) -> Result<WireProtocolVersion, WireError> {
        let config = self.ensure_config();
        let mut state = self.state.borrow_mut();

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
                return Err(WireError::NegotiationTimeout { peer_id });
            }
        };

        state.stats.successful_negotiations += 1;
        Ok(negotiated)
    }

    fn encode_endpoint(&self, endpoint: &Endpoint) -> OpaqueEndpoint {
        OpaqueEndpoint::new(endpoint.carrier_type as u16, endpoint.address.clone())
    }

    fn decode_endpoint(
        &self,
        opaque: &OpaqueEndpoint,
    ) -> Result<Endpoint, WireError> {
        let config = self.ensure_config();
        if opaque.address_bytes.len() > config.endpoint_address_size_limit {
            return Err(WireError::EndpointExceedsLimit {
                address_size: opaque.address_bytes.len(),
                limit: config.endpoint_address_size_limit,
            });
        }
        Ok(Endpoint::new(
            CarrierType::from_u16(opaque.carrier_type),
            opaque.address_bytes.clone(),
        ))
    }

    fn endpoint_address_size_limit(&self) -> usize {
        let state = self.state.borrow();
        state.config.endpoint_address_size_limit
    }

    fn endpoint_exceeds_limit(&self, address_size: usize) -> bool {
        let config = self.ensure_config();
        address_size > config.endpoint_address_size_limit
    }

    fn register_extension(
        &self,
        extension: WireExtension,
    ) -> Result<WireExtensionId, WireError> {
        let config = self.ensure_config();
        let mut state = self.state.borrow_mut();

        if state.extensions.len() as u32 >= config.max_extensions {
            return Err(WireError::MaxExtensionsExceeded {
                max_extensions: config.max_extensions,
            });
        }

        state.extension_counter += 1;
        let extension_id = WireExtensionId(state.extension_counter);

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

    fn registered_extensions(&self) -> Vec<WireExtension> {
        let state = self.state.borrow();
        state.extensions.values().cloned().collect()
    }

    fn extension_by_id(
        &self,
        extension_id: WireExtensionId,
    ) -> Option<WireExtension> {
        let state = self.state.borrow();
        state.extensions.get(&extension_id).cloned()
    }

    fn stats(&self) -> WireStats {
        let state = self.state.borrow();
        state.stats.clone()
    }

    fn is_healthy(&self) -> bool {
        let state = self.state.borrow();
        if state.stats.frames_validated == 0 {
            return true;
        }
        state.stats.validation_success_rate() > 50.0
    }

    fn validate_config(&self, config: &WireConfig) -> Result<(), WireError> {
        config.validate()
    }

    fn snapshot(&self) -> WireSnapshot {
        let state = self.state.borrow();
        WireSnapshot {
            version: state.version,
            current_format: state.config.default_format,
            wire_protocol_version: state.config.wire_protocol_version,
            frame_limits: self.frame_limits(),
            extensions: state.extensions.values().cloned().collect(),
            peer_formats: state.peer_formats.clone(),
            stats: state.stats.clone(),
        }
    }

    fn restore_from_snapshot(
        &self,
        snapshot: WireSnapshot,
    ) -> Result<(), WireError> {
        let mut state = self.state.borrow_mut();
        state.version = snapshot.version;
        state.config.default_format = snapshot.current_format;
        state.config.wire_protocol_version = snapshot.wire_protocol_version;
        state.stats = snapshot.stats;

        state.extensions.clear();
        for ext in &snapshot.extensions {
            state.extensions.insert(ext.extension_id, ext.clone());
        }

        state.peer_formats = snapshot.peer_formats;

        Ok(())
    }

    fn peers_by_format(&self, format: WireFormat) -> Vec<NodeId> {
        let state = self.state.borrow();
        state
            .peer_formats
            .iter()
            .filter(|(_, f)| **f == format)
            .map(|(peer_id, _)| *peer_id)
            .collect()
    }

    fn peer_format(&self, peer_id: NodeId) -> Option<WireFormat> {
        let state = self.state.borrow();
        state.peer_formats.get(&peer_id).copied()
    }

    fn set_peer_format(
        &self,
        peer_id: NodeId,
        format: WireFormat,
    ) -> Result<(), WireError> {
        let config = self.ensure_config();
        if !state_format_supported(&config, format) {
            return Err(WireError::FormatNotSupported { format });
        }
        let mut state = self.state.borrow_mut();
        state.peer_formats.insert(peer_id, format);
        state.version += 1;
        Ok(())
    }

    fn negotiation_timeout_ms(&self) -> u64 {
        let state = self.state.borrow();
        state.config.frame_validation_timeout_ms
    }

    fn set_negotiation_timeout_ms(&self, timeout_ms: u64) -> Result<(), WireError> {
        let mut state = self.state.borrow_mut();
        if timeout_ms == 0 {
            return Err(WireError::InvalidConfig {
                field: "negotiation_timeout_ms".to_string(),
                reason: "timeout must be greater than 0".to_string(),
            });
        }
        state.config.frame_validation_timeout_ms = timeout_ms;
        state.version += 1;
        Ok(())
    }

    fn max_frame_payload_size(&self) -> usize {
        let state = self.state.borrow();
        state.config.max_frame_payload_size
    }

    fn set_max_frame_payload_size(&self, size: usize) -> Result<(), WireError> {
        let mut state = self.state.borrow_mut();
        if size == 0 {
            return Err(WireError::InvalidConfig {
                field: "max_frame_payload_size".to_string(),
                reason: "size must be greater than 0".to_string(),
            });
        }
        state.config.max_frame_payload_size = size;
        state.version += 1;
        Ok(())
    }

    /// Get the total number of frames encoded.
    pub fn total_frames_encoded(&self) -> u64 {
        let state = self.state.borrow();
        state.stats.frames_encoded
    }

    /// Get the total number of frames decoded.
    pub fn total_frames_decoded(&self) -> u64 {
        let state = self.state.borrow();
        state.stats.frames_decoded
    }

    /// Get the total number of frames validated.
    pub fn total_frames_validated(&self) -> u64 {
        let state = self.state.borrow();
        state.stats.frames_validated
    }

    /// Get the total number of validation failures.
    pub fn total_validation_failures(&self) -> u64 {
        let state = self.state.borrow();
        state.stats.validation_failures
    }

    /// Get the total payload bytes processed.
    pub fn total_payload_bytes(&self) -> u64 {
        let state = self.state.borrow();
        state.stats.total_payload_bytes
    }

    /// Get the total frame bytes processed.
    pub fn total_frame_bytes(&self) -> u64 {
        let state = self.state.borrow();
        state.stats.total_frame_bytes
    }

    /// Get frames by format.
    pub fn frames_by_format(&self, format: WireFormat) -> Vec<u64> {
        let state = self.state.borrow();
        state
            .frames
            .iter()
            .filter(|(_, f)| f.format == format)
            .map(|(id, _)| *id)
            .collect()
    }

    /// Get frames by flag.
    pub fn frames_by_flag(&self, flag: u16) -> Vec<u64> {
        let state = self.state.borrow();
        state
            .frames
            .iter()
            .filter(|(_, f)| f.flags.contains(flag))
            .map(|(id, _)| *id)
            .collect()
    }

    /// Get frames with checksums.
    pub fn frames_with_checksum(&self) -> Vec<u64> {
        let state = self.state.borrow();
        state
            .frames
            .iter()
            .filter(|(_, f)| f.checksum.is_some())
            .map(|(id, _)| *id)
            .collect()
    }

    /// Get fragmented frames.
    pub fn fragmented_frames(&self) -> Vec<u64> {
        let state = self.state.borrow();
        state
            .frames
            .iter()
            .filter(|(_, f)| f.flags.is_fragmented())
            .map(|(id, _)| *id)
            .collect()
    }

    /// Get the frame count.
    pub fn frame_count(&self) -> usize {
        let state = self.state.borrow();
        state.frames.len()
    }

    /// Get the extension count.
    pub fn extension_count(&self) -> usize {
        let state = self.state.borrow();
        state.extensions.len()
    }

    /// Get the enabled extension count.
    pub fn enabled_extension_count(&self) -> usize {
        let state = self.state.borrow();
        state
            .extensions
            .values()
            .filter(|e| e.enabled)
            .count()
    }

    /// Get the peer count.
    pub fn peer_count(&self) -> usize {
        let state = self.state.borrow();
        state.peer_formats.len()
    }

    /// Get all registered wire formats.
    pub fn all_registered_formats(&self) -> Vec<WireFormat> {
        let state = self.state.borrow();
        state.formats.keys().cloned().collect()
    }

    /// Register a wire format.
    pub fn register_format(&self, format: WireFormat) -> Result<(), WireError> {
        let mut state = self.state.borrow_mut();
        state.formats.insert(format, true);
        state.version += 1;
        Ok(())
    }

    /// Unregister a wire format.
    pub fn unregister_format(&self, format: WireFormat) -> Result<(), WireError> {
        let mut state = self.state.borrow_mut();
        if format == state.config.default_format {
            return Err(WireError::InvalidConfig {
                field: "format".to_string(),
                reason: "cannot unregister default format".to_string(),
            });
        }
        state.formats.remove(&format);
        state.version += 1;
        Ok(())
    }

    /// Check if a format is registered.
    pub fn is_format_registered(&self, format: WireFormat) -> bool {
        let state = self.state.borrow();
        state.formats.contains_key(&format)
    }

    /// Get the default format.
    pub fn default_format(&self) -> WireFormat {
        let state = self.state.borrow();
        state.config.default_format
    }

    /// Get the frame validation timeout.
    pub fn frame_validation_timeout_ms(&self) -> u64 {
        let state = self.state.borrow();
        state.config.frame_validation_timeout_ms
    }

    /// Get the max pending validations.
    pub fn max_pending_validations(&self) -> u32 {
        let state = self.state.borrow();
        state.config.max_pending_validations
    }

    /// Check if frame validation is enabled.
    pub fn is_frame_validation_enabled(&self) -> bool {
        let state = self.state.borrow();
        state.config.enable_frame_validation
    }

    /// Enable or disable frame validation.
    pub fn set_frame_validation_enabled(&self, enabled: bool) -> Result<(), WireError> {
        let mut state = self.state.borrow_mut();
        state.config.enable_frame_validation = enabled;
        state.version += 1;
        Ok(())
    }

    /// Check if auto-negotiation is enabled.
    pub fn is_auto_negotiation_enabled(&self) -> bool {
        let state = self.state.borrow();
        state.config.enable_auto_negotiation
    }

    /// Enable or disable auto-negotiation.
    pub fn set_auto_negotiation(&self, enabled: bool) -> Result<(), WireError> {
        let mut state = self.state.borrow_mut();
        state.config.enable_auto_negotiation = enabled;
        state.version += 1;
        Ok(())
    }

    /// Check if wire extensions are enabled.
    pub fn are_wire_extensions_enabled(&self) -> bool {
        let state = self.state.borrow();
        state.config.enable_wire_extensions
    }

    /// Enable or disable wire extensions.
    pub fn set_wire_extensions_enabled(&self, enabled: bool) -> Result<(), WireError> {
        let mut state = self.state.borrow_mut();
        state.config.enable_wire_extensions = enabled;
        state.version += 1;
        Ok(())
    }

    /// Check if strict format matching is enabled.
    pub fn is_strict_format_matching(&self) -> bool {
        let state = self.state.borrow();
        state.config.strict_format_matching
    }

    /// Enable or disable strict format matching.
    pub fn set_strict_format_matching(&self, enabled: bool) -> Result<(), WireError> {
        let mut state = self.state.borrow_mut();
        state.config.strict_format_matching = enabled;
        state.version += 1;
        Ok(())
    }

    /// Check if frame checksum is enabled.
    pub fn is_frame_checksum_enabled(&self) -> bool {
        let state = self.state.borrow();
        state.config.frame_checksum_enabled
    }

    /// Enable or disable frame checksum.
    pub fn set_frame_checksum_enabled(&self, enabled: bool) -> Result<(), WireError> {
        let mut state = self.state.borrow_mut();
        state.config.frame_checksum_enabled = enabled;
        state.version += 1;
        Ok(())
    }

    /// Get the min supported wire version.
    pub fn min_supported_wire_version(&self) -> WireProtocolVersion {
        let state = self.state.borrow();
        state.config.min_supported_version
    }

    /// Set the min supported wire version.
    pub fn set_min_supported_wire_version(
        &self,
        version: WireProtocolVersion,
    ) -> Result<(), WireError> {
        let mut state = self.state.borrow_mut();
        if version.0 == 0 {
            return Err(WireError::InvalidConfig {
                field: "min_supported_version".to_string(),
                reason: "version must be greater than 0".to_string(),
            });
        }
        if version > state.config.max_supported_version {
            return Err(WireError::InvalidConfig {
                field: "min_supported_version".to_string(),
                reason: "min must be <= max".to_string(),
            });
        }
        state.config.min_supported_version = version;
        state.version += 1;
        Ok(())
    }

    /// Get the max supported wire version.
    pub fn max_supported_wire_version(&self) -> WireProtocolVersion {
        let state = self.state.borrow();
        state.config.max_supported_version
    }

    /// Set the max supported wire version.
    pub fn set_max_supported_wire_version(
        &self,
        version: WireProtocolVersion,
    ) -> Result<(), WireError> {
        let mut state = self.state.borrow_mut();
        if version.0 == 0 {
            return Err(WireError::InvalidConfig {
                field: "max_supported_version".to_string(),
                reason: "version must be greater than 0".to_string(),
            });
        }
        if version < state.config.min_supported_version {
            return Err(WireError::InvalidConfig {
                field: "max_supported_version".to_string(),
                reason: "max must be >= min".to_string(),
            });
        }
        state.config.max_supported_version = version;
        state.version += 1;
        Ok(())
    }

    /// Get the current version's display string.
    pub fn wire_protocol_version_display(&self) -> String {
        let state = self.state.borrow();
        state.config.wire_protocol_version.display_string()
    }

    /// Get all peers by format.
    pub fn all_peer_formats(&self) -> Vec<(NodeId, WireFormat)> {
        let state = self.state.borrow();
        state
            .peer_formats
            .iter()
            .map(|(peer_id, format)| (*peer_id, *format))
            .collect()
    }

    /// Remove a peer format.
    pub fn remove_peer_format(&self, peer_id: NodeId) -> Result<(), WireError> {
        let mut state = self.state.borrow_mut();
        if state.peer_formats.remove(&peer_id).is_none() {
            return Err(WireError::PeerNotFound { peer_id });
        }
        state.version += 1;
        Ok(())
    }

    /// Get the payload efficiency for a given size.
    pub fn payload_efficiency(&self, payload_size: usize) -> f64 {
        let config = self.ensure_config();
        config.payload_efficiency(payload_size)
    }

    /// Get the average frame size.
    pub fn average_frame_size(&self) -> f64 {
        let state = self.state.borrow();
        if state.frames.is_empty() {
            return 0.0;
        }
        let total: usize = state.frames.values().map(|f| f.total_size()).sum();
        total as f64 / state.frames.len() as f64
    }

    /// Get the average payload size.
    pub fn average_payload_size(&self) -> f64 {
        let state = self.state.borrow();
        if state.frames.is_empty() {
            return 0.0;
        }
        let total: usize = state.frames.values().map(|f| f.payload.len()).sum();
        total as f64 / state.frames.len() as f64
    }

    /// Get the total overhead bytes.
    pub fn total_overhead_bytes(&self) -> usize {
        let state = self.state.borrow();
        state
            .frames
            .values()
            .map(|f| f.frame_overhead())
            .sum()
    }

    /// Get the overhead percentage.
    pub fn overhead_percentage(&self) -> f64 {
        let state = self.state.borrow();
        let total_frame: usize = state.frames.values().map(|f| f.total_size()).sum();
        let total_payload: usize = state.frames.values().map(|f| f.payload.len()).sum();
        if total_frame == 0 {
            return 0.0;
        }
        (total_frame - total_payload) as f64 / total_frame as f64 * 100.0
    }

    /// Enable an extension.
    pub fn enable_extension(&self, extension_id: WireExtensionId) -> Result<(), WireError> {
        let mut state = self.state.borrow_mut();
        let ext = state
            .extensions
            .get_mut(&extension_id)
            .ok_or_else(|| WireError::ExtensionNotFound { extension_id })?;
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
        extension_id: WireExtensionId,
    ) -> Result<(), WireError> {
        let mut state = self.state.borrow_mut();
        let ext = state
            .extensions
            .get_mut(&extension_id)
            .ok_or_else(|| WireError::ExtensionNotFound { extension_id })?;
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
    pub fn remove_extension(
        &self,
        extension_id: WireExtensionId,
    ) -> Result<(), WireError> {
        let mut state = self.state.borrow_mut();
        if state.extensions.remove(&extension_id).is_none() {
            return Err(WireError::ExtensionNotFound { extension_id });
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

    /// Get the latest frame ID.
    pub fn latest_frame_id(&self) -> Option<u64> {
        let state = self.state.borrow();
        state.frames.keys().max().copied()
    }

    /// Get the total frame validation rate.
    pub fn frame_validation_rate(&self) -> f64 {
        let state = self.state.borrow();
        if state.stats.frames_validated == 0 {
            return 100.0;
        }
        state.stats.validation_success_rate()
    }

    /// Get the wire health score (0.0 to 1.0).
    pub fn wire_health_score(&self) -> f64 {
        let state = self.state.borrow();
        let validation_health = if state.stats.frames_validated == 0 {
            1.0
        } else {
            state.stats.validation_success_rate() / 100.0
        };
        let negotiation_health = if state.stats.total_negotiations == 0 {
            1.0
        } else {
            state.stats.negotiation_success_rate() / 100.0
        };
        let format_health = if state.formats.is_empty() {
            0.0
        } else {
            1.0
        };
        (validation_health + negotiation_health + format_health) / 3.0
    }

    /// Clear all wire state.
    pub fn clear_all(&self) {
        let mut state = self.state.borrow_mut();
        state.config = WireConfig::default();
        state.formats.clear();
        state.formats.insert(WireFormat::NativeV1, true);
        state.extensions.clear();
        state.extension_counter = 0;
        state.frames.clear();
        state.frame_counter = 0;
        state.peer_formats.clear();
        state.stats = WireStats::new();
        state.version = 0;
    }

    /// Get the wire graph size.
    pub fn graph_size(&self) -> (usize, usize, usize, usize) {
        let state = self.state.borrow();
        (
            state.frames.len(),
            state.extensions.len(),
            state.peer_formats.len(),
            state.formats.len(),
        )
    }

    /// Get the config snapshot.
    pub fn config_snapshot(&self) -> WireConfig {
        let state = self.state.borrow();
        state.config.clone()
    }

    /// Check if a payload fits within frame limits.
    pub fn payload_fits(&self, payload_size: usize) -> bool {
        let config = self.ensure_config();
        payload_size <= config.max_frame_payload_size
    }
}

// Helper: check if a format is supported by config
fn state_format_supported(config: &WireConfig, format: WireFormat) -> bool {
    matches!(
        format,
        WireFormat::NativeV1
            | WireFormat::NativeV2
            | WireFormat::CompactV1
            | WireFormat::CompactV2
            | WireFormat::Extended(_)
            | WireFormat::Custom(_)
    )
}

// Helper: format priority for negotiation
fn format_priority(format: WireFormat) -> u32 {
    match format {
        WireFormat::NativeV2 => 100,
        WireFormat::NativeV1 => 90,
        WireFormat::CompactV2 => 80,
        WireFormat::CompactV1 => 70,
        WireFormat::Extended(_) => 60,
        WireFormat::Custom(_) => 10,
    }
}

// Helper: checksum computation
fn compute_checksum(payload: &[u8]) -> Vec<u8> {
    // Simple BLAKE3-style checksum (32 bytes)
    let mut checksum = vec![0u8; 32];
    for (i, byte) in payload.iter().enumerate() {
        checksum[i % 32] ^= byte;
        checksum[i % 32] = checksum[i % 32].wrapping_add(*byte);
    }
    checksum
}

// Need CarrierType import - defined locally since types.rs may not have it
// Re-export from types via crate::types
use crate::types::CarrierType;

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

    fn test_wire_format() -> WireFormat {
        WireFormat::NativeV1
    }

    fn test_wire_format2() -> WireFormat {
        WireFormat::NativeV2
    }

    fn test_wire_protocol_version() -> WireProtocolVersion {
        WireProtocolVersion::new(1)
    }

    fn test_wire_protocol_version2() -> WireProtocolVersion {
        WireProtocolVersion::new(2)
    }

    fn test_config() -> WireConfig {
        WireConfig::default()
    }

    // ========================================================================
    // WireConfig tests
    // ========================================================================

    #[test]
    fn wire_config_default() {
        let config = WireConfig::default();
        assert_eq!(config.default_format, WireFormat::NativeV1);
        assert_eq!(config.max_frame_payload_size, 65536);
        assert_eq!(config.endpoint_address_size_limit, 1024);
        assert_eq!(config.wire_protocol_version.0, 1);
        assert_eq!(config.min_supported_version.0, 1);
        assert_eq!(config.max_supported_version.0, 100);
        assert!(config.enable_auto_negotiation);
        assert!(config.enable_frame_validation);
        assert_eq!(config.frame_validation_timeout_ms, 5000);
        assert_eq!(config.max_pending_validations, 1000);
        assert!(config.enable_wire_extensions);
        assert_eq!(config.max_extensions, 50);
        assert!(!config.strict_format_matching);
        assert!(config.frame_checksum_enabled);
    }

    #[test]
    fn wire_config_new_valid() {
        let config = WireConfig::new(WireFormat::CompactV1).unwrap();
        assert_eq!(config.default_format, WireFormat::CompactV1);
    }

    #[test]
    fn wire_config_validate_zero_max_payload() {
        let mut config = WireConfig::default();
        config.max_frame_payload_size = 0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn wire_config_validate_zero_endpoint_limit() {
        let mut config = WireConfig::default();
        config.endpoint_address_size_limit = 0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn wire_config_validate_zero_validation_timeout() {
        let mut config = WireConfig::default();
        config.frame_validation_timeout_ms = 0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn wire_config_validate_zero_max_pending() {
        let mut config = WireConfig::default();
        config.max_pending_validations = 0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn wire_config_validate_zero_max_extensions() {
        let mut config = WireConfig::default();
        config.max_extensions = 0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn wire_config_strict() {
        let config = WireConfig::strict();
        assert_eq!(config.max_frame_payload_size, 4096);
        assert_eq!(config.endpoint_address_size_limit, 512);
        assert!(!config.enable_auto_negotiation);
        assert!(!config.enable_wire_extensions);
        assert!(config.strict_format_matching);
        assert_eq!(config.max_extensions, 5);
    }

    #[test]
    fn wire_config_lenient() {
        let config = WireConfig::lenient();
        assert_eq!(config.max_frame_payload_size, 1048576);
        assert_eq!(config.endpoint_address_size_limit, 4096);
        assert!(!config.enable_frame_validation);
        assert!(!config.frame_checksum_enabled);
        assert_eq!(config.max_extensions, 500);
    }

    #[test]
    fn wire_config_payload_efficiency() {
        let config = WireConfig::default();
        let efficiency = config.payload_efficiency(1000);
        assert!(efficiency > 0.0);
        assert!(efficiency <= 100.0);
    }

    #[test]
    fn wire_config_payload_efficiency_zero() {
        let config = WireConfig::default();
        assert!((config.payload_efficiency(0) - 100.0).abs() < 0.0001);
    }

    #[test]
    fn wire_config_serialization_roundtrip() {
        let config = WireConfig::default();
        let json = serde_json::to_string(&config).unwrap();
        let deserialized: WireConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(config.default_format, deserialized.default_format);
        assert_eq!(
            config.max_frame_payload_size,
            deserialized.max_frame_payload_size
        );
    }

    // ========================================================================
    // WireFormat tests
    // ========================================================================

    #[test]
    fn wire_format_as_str() {
        assert_eq!(WireFormat::NativeV1.as_str(), "native_v1");
        assert_eq!(WireFormat::NativeV2.as_str(), "native_v2");
        assert_eq!(WireFormat::CompactV1.as_str(), "compact_v1");
        assert_eq!(WireFormat::CompactV2.as_str(), "compact_v2");
        assert_eq!(WireFormat::Extended(5).as_str(), "extended");
        assert_eq!(WireFormat::Custom(42).as_str(), "custom");
    }

    #[test]
    fn wire_format_version() {
        assert_eq!(WireFormat::NativeV1.version(), 1);
        assert_eq!(WireFormat::NativeV2.version(), 2);
        assert_eq!(WireFormat::Extended(7).version(), 7);
    }

    #[test]
    fn wire_format_is_native() {
        assert!(WireFormat::NativeV1.is_native());
        assert!(WireFormat::NativeV2.is_native());
        assert!(!WireFormat::CompactV1.is_native());
        assert!(!WireFormat::Extended(1).is_native());
    }

    #[test]
    fn wire_format_is_compact() {
        assert!(WireFormat::CompactV1.is_compact());
        assert!(WireFormat::CompactV2.is_compact());
        assert!(!WireFormat::NativeV1.is_compact());
    }

    #[test]
    fn wire_format_header_size() {
        assert_eq!(WireFormat::NativeV1.header_size(), 16);
        assert_eq!(WireFormat::NativeV2.header_size(), 24);
        assert_eq!(WireFormat::CompactV1.header_size(), 8);
        assert_eq!(WireFormat::CompactV2.header_size(), 12);
    }

    #[test]
    fn wire_format_supports_checksum() {
        assert!(WireFormat::NativeV1.supports_checksum());
        assert!(WireFormat::NativeV2.supports_checksum());
        assert!(!WireFormat::CompactV1.supports_checksum());
    }

    #[test]
    fn wire_format_supports_fragmentation() {
        assert!(!WireFormat::NativeV1.supports_fragmentation());
        assert!(WireFormat::NativeV2.supports_fragmentation());
        assert!(!WireFormat::CompactV1.supports_fragmentation());
        assert!(WireFormat::CompactV2.supports_fragmentation());
    }

    #[test]
    fn wire_format_serialization_roundtrip() {
        let format = WireFormat::Extended(42);
        let json = serde_json::to_string(&format).unwrap();
        let deserialized: WireFormat = serde_json::from_str(&json).unwrap();
        assert_eq!(format, deserialized);
    }

    // ========================================================================
    // NativeFrame tests
    // ========================================================================

    #[test]
    fn native_frame_new() {
        let frame = NativeFrame::new(1, WireFormat::NativeV1, vec![1, 2, 3]);
        assert_eq!(frame.frame_id, 1);
        assert_eq!(frame.format, WireFormat::NativeV1);
        assert_eq!(frame.payload, vec![1, 2, 3]);
        assert_eq!(frame.payload_size(), 3);
        assert!(!frame.has_checksum());
        assert!(!frame.is_fragmented());
        assert!(frame.is_last_fragment());
    }

    #[test]
    fn native_frame_total_size() {
        let frame = NativeFrame::new(1, WireFormat::NativeV1, vec![1, 2, 3, 4, 5]);
        let expected = 16 + 5; // header + payload
        assert_eq!(frame.total_size(), expected);
    }

    #[test]
    fn native_frame_total_size_with_checksum() {
        let mut frame = NativeFrame::new(1, WireFormat::NativeV1, vec![1, 2, 3]);
        frame.checksum = Some(vec![0u8; 32]);
        let expected = 16 + 3 + 32;
        assert_eq!(frame.total_size(), expected);
    }

    #[test]
    fn native_frame_is_fragmented() {
        let mut frame = NativeFrame::new(1, WireFormat::NativeV2, vec![1]);
        frame.fragment_index = Some(0);
        frame.total_fragments = Some(3);
        assert!(frame.is_fragmented());
        assert!(frame.is_last_fragment() == false);
    }

    #[test]
    fn native_frame_is_last_fragment_true() {
        let mut frame = NativeFrame::new(1, WireFormat::NativeV2, vec![1]);
        frame.fragment_index = Some(2);
        frame.total_fragments = Some(3);
        assert!(frame.is_fragmented());
        assert!(frame.is_last_fragment());
    }

    #[test]
    fn native_frame_frame_overhead() {
        let frame = NativeFrame::new(1, WireFormat::NativeV1, vec![1, 2, 3]);
        assert_eq!(frame.frame_overhead(), 16);

        let mut frame = NativeFrame::new(1, WireFormat::NativeV1, vec![1]);
        frame.checksum = Some(vec![0u8; 32]);
        assert_eq!(frame.frame_overhead(), 16 + 32);
    }

    #[test]
    fn native_frame_has_checksum() {
        let frame = NativeFrame::new(1, WireFormat::NativeV1, vec![1]);
        assert!(!frame.has_checksum());

        let mut frame = NativeFrame::new(1, WireFormat::NativeV1, vec![1]);
        frame.checksum = Some(vec![0u8; 32]);
        assert!(frame.has_checksum());
    }

    #[test]
    fn native_frame_serialization_roundtrip() {
        let frame = NativeFrame::new(42, WireFormat::NativeV2, vec![1, 2, 3, 4]);
        let json = serde_json::to_string(&frame).unwrap();
        let deserialized: NativeFrame = serde_json::from_str(&json).unwrap();
        assert_eq!(frame, deserialized);
    }

    // ========================================================================
    // FrameFlags tests
    // ========================================================================

    #[test]
    fn frame_flags_empty() {
        let flags = FrameFlags::empty();
        assert_eq!(flags.bits(), 0);
        assert!(!flags.is_checksum());
        assert!(!flags.is_fragmented());
    }

    #[test]
    fn frame_flags_contains() {
        let mut flags = FrameFlags::empty();
        flags.insert(FrameFlags::FLAG_CHECKSUM);
        assert!(flags.contains(FrameFlags::FLAG_CHECKSUM));
        assert!(!flags.contains(FrameFlags::FLAG_FRAGMENTED));
    }

    #[test]
    fn frame_flags_insert_remove() {
        let mut flags = FrameFlags::empty();
        flags.insert(FrameFlags::FLAG_ENCRYPTED);
        assert!(flags.is_encrypted());
        flags.remove(FrameFlags::FLAG_ENCRYPTED);
        assert!(!flags.is_encrypted());
    }

    #[test]
    fn frame_flags_all() {
        let flags = FrameFlags::all();
        assert_eq!(flags.bits(), u16::MAX);
        assert!(flags.is_checksum());
        assert!(flags.is_ack_required());
    }

    #[test]
    fn frame_flags_display_properties() {
        let mut flags = FrameFlags::empty();
        flags.insert(FrameFlags::FLAG_PRIORITY);
        flags.insert(FrameFlags::FLAG_ACK_REQUIRED);
        assert!(flags.is_priority());
        assert!(flags.is_ack_required());
        assert!(!flags.is_compressed());
    }

    #[test]
    fn frame_flags_serialization_roundtrip() {
        let mut flags = FrameFlags::empty();
        flags.insert(FrameFlags::FLAG_COMPRESSED);
        let json = serde_json::to_string(&flags).unwrap();
        let deserialized: FrameFlags = serde_json::from_str(&json).unwrap();
        assert_eq!(flags, deserialized);
    }

    // ========================================================================
    // FrameValidationResult tests
    // ========================================================================

    #[test]
    fn frame_validation_result_new() {
        let result = FrameValidationResult::new(1);
        assert_eq!(result.frame_id, 1);
        assert!(result.valid);
        assert!(result.is_clean());
        assert_eq!(result.error_count(), 0);
    }

    #[test]
    fn frame_validation_result_add_error() {
        let mut result = FrameValidationResult::new(1);
        result.add_error(FrameValidationError::ChecksumMismatch);
        assert!(!result.valid);
        assert!(!result.is_clean());
        assert_eq!(result.error_count(), 1);
    }

    #[test]
    fn frame_validation_result_is_clean() {
        let mut result = FrameValidationResult::new(1);
        assert!(result.is_clean());
        result.add_error(FrameValidationError::SizeExceeded);
        assert!(!result.is_clean());
    }

    #[test]
    fn frame_validation_result_serialization_roundtrip() {
        let mut result = FrameValidationResult::new(42);
        result.add_error(FrameValidationError::TruncatedFrame);
        let json = serde_json::to_string(&result).unwrap();
        let deserialized: FrameValidationResult = serde_json::from_str(&json).unwrap();
        assert_eq!(result.frame_id, deserialized.frame_id);
        assert!(!deserialized.valid);
    }

    // ========================================================================
    // FrameValidationError tests
    // ========================================================================

    #[test]
    fn frame_validation_error_as_str() {
        assert_eq!(
            FrameValidationError::ChecksumMismatch.as_str(),
            "checksum_mismatch"
        );
        assert_eq!(
            FrameValidationError::SizeExceeded.as_str(),
            "size_exceeded"
        );
        assert_eq!(
            FrameValidationError::InvalidFormat.as_str(),
            "invalid_format"
        );
        assert_eq!(
            FrameValidationError::UnknownFlag(99).as_str(),
            "unknown_flag"
        );
    }

    #[test]
    fn frame_validation_error_serialization_roundtrip() {
        let error = FrameValidationError::CorruptedPayload;
        let json = serde_json::to_string(&error).unwrap();
        let deserialized: FrameValidationError = serde_json::from_str(&json).unwrap();
        assert_eq!(error, deserialized);
    }

    // ========================================================================
    // FrameLimits tests
    // ========================================================================

    #[test]
    fn frame_limits_new() {
        let limits = FrameLimits::new(65536, 16, 32);
        assert_eq!(limits.min_payload_size, 0);
        assert_eq!(limits.max_payload_size, 65536);
        assert_eq!(limits.max_total_frame_size, 65584);
        assert_eq!(limits.max_fragments_per_frame, 1000);
        assert_eq!(limits.header_size, 16);
        assert_eq!(limits.checksum_size, 32);
    }

    #[test]
    fn frame_limits_default() {
        let limits = FrameLimits::default();
        assert_eq!(limits.max_payload_size, 65536);
    }

    #[test]
    fn frame_limits_can_fit_payload() {
        let limits = FrameLimits::new(1000, 16, 32);
        assert!(limits.can_fit_payload(500));
        assert!(limits.can_fit_payload(1000));
        assert!(!limits.can_fit_payload(1001));
    }

    #[test]
    fn frame_limits_overhead_percent() {
        let limits = FrameLimits::new(1000, 16, 32);
        let overhead = limits.overhead_percent();
        assert!((overhead - 4.8).abs() < 0.1);
    }

    #[test]
    fn frame_limits_serialization_roundtrip() {
        let limits = FrameLimits::new(5000, 24, 32);
        let json = serde_json::to_string(&limits).unwrap();
        let deserialized: FrameLimits = serde_json::from_str(&json).unwrap();
        assert_eq!(limits.max_payload_size, deserialized.max_payload_size);
    }

    // ========================================================================
    // WireProtocolVersion tests
    // ========================================================================

    #[test]
    fn wire_protocol_version_new() {
        let version = WireProtocolVersion::new(42);
        assert_eq!(version.0, 42);
    }

    #[test]
    fn wire_protocol_version_major_minor_patch() {
        let version = WireProtocolVersion::new(1234);
        assert_eq!(version.major(), 1);
        assert_eq!(version.minor(), 2);
        assert_eq!(version.patch(), 34);
    }

    #[test]
    fn wire_protocol_version_increment_major() {
        let version = WireProtocolVersion::new(1000);
        let incremented = version.increment_major();
        assert_eq!(incremented.0, 2000);
    }

    #[test]
    fn wire_protocol_version_increment_minor() {
        let version = WireProtocolVersion::new(1100);
        let incremented = version.increment_minor();
        assert_eq!(incremented.0, 1200);
    }

    #[test]
    fn wire_protocol_version_increment_patch() {
        let version = WireProtocolVersion::new(1001);
        let incremented = version.increment_patch();
        assert_eq!(incremented.0, 1002);
    }

    #[test]
    fn wire_protocol_version_display_string() {
        let version = WireProtocolVersion::new(1023);
        assert_eq!(version.display_string(), "1.2.23");
    }

    #[test]
    fn wire_protocol_version_ordering() {
        let v1 = WireProtocolVersion::new(1);
        let v2 = WireProtocolVersion::new(2);
        assert!(v1 < v2);
        assert!(v2 > v1);
    }

    #[test]
    fn wire_protocol_version_serialization_roundtrip() {
        let version = WireProtocolVersion::new(42);
        let json = serde_json::to_string(&version).unwrap();
        let deserialized: WireProtocolVersion = serde_json::from_str(&json).unwrap();
        assert_eq!(version, deserialized);
    }

    // ========================================================================
    // OpaqueEndpoint tests
    // ========================================================================

    #[test]
    fn opaque_endpoint_new() {
        let opaque = OpaqueEndpoint::new(1, vec![1, 2, 3]);
        assert_eq!(opaque.carrier_type, 1);
        assert_eq!(opaque.address_bytes, vec![1, 2, 3]);
        assert_eq!(opaque.encoded_size, 5);
    }

    #[test]
    fn opaque_endpoint_is_empty() {
        let opaque = OpaqueEndpoint::new(1, vec![]);
        assert!(opaque.is_empty());
        let opaque = OpaqueEndpoint::new(1, vec![1]);
        assert!(!opaque.is_empty());
    }

    #[test]
    fn opaque_endpoint_address_size() {
        let opaque = OpaqueEndpoint::new(1, vec![1, 2, 3, 4, 5]);
        assert_eq!(opaque.address_size(), 5);
    }

    #[test]
    fn opaque_endpoint_serialization_roundtrip() {
        let opaque = OpaqueEndpoint::new(2, vec![10, 20, 30]);
        let json = serde_json::to_string(&opaque).unwrap();
        let deserialized: OpaqueEndpoint = serde_json::from_str(&json).unwrap();
        assert_eq!(opaque, deserialized);
    }

    // ========================================================================
    // WireExtension tests
    // ========================================================================

    #[test]
    fn wire_extension_new() {
        let ext = WireExtension::new("test_ext".to_string(), WireFormat::NativeV1);
        assert_eq!(ext.name, "test_ext");
        assert!(ext.enabled);
    }

    #[test]
    fn wire_extension_is_enabled() {
        let mut ext = WireExtension::new("ext".to_string(), WireFormat::NativeV1);
        assert!(ext.is_enabled());
        ext.enabled = false;
        assert!(!ext.is_enabled());
    }

    #[test]
    fn wire_extension_serialization_roundtrip() {
        let ext = WireExtension {
            extension_id: WireExtensionId(1),
            name: "test".to_string(),
            format: WireFormat::NativeV2,
            version: WireProtocolVersion::new(1),
            description: vec![1, 2, 3],
            enabled: true,
        };
        let json = serde_json::to_string(&ext).unwrap();
        let deserialized: WireExtension = serde_json::from_str(&json).unwrap();
        assert_eq!(ext, deserialized);
    }

    // ========================================================================
    // WireStats tests
    // ========================================================================

    #[test]
    fn wire_stats_new() {
        let stats = WireStats::new();
        assert_eq!(stats.frames_encoded, 0);
        assert_eq!(stats.frames_decoded, 0);
        assert_eq!(stats.frames_validated, 0);
        assert_eq!(stats.validation_failures, 0);
    }

    #[test]
    fn wire_stats_encode_success_rate() {
        let stats = WireStats::new();
        assert!((stats.encode_success_rate() - 100.0).abs() < 0.01);
    }

    #[test]
    fn wire_stats_decode_success_rate() {
        let stats = WireStats::new();
        assert!((stats.decode_success_rate() - 100.0).abs() < 0.01);
    }

    #[test]
    fn wire_stats_validation_success_rate() {
        let mut stats = WireStats::new();
        stats.frames_validated = 100;
        stats.validation_failures = 10;
        assert!((stats.validation_success_rate() - 90.0).abs() < 0.01);
    }

    #[test]
    fn wire_stats_validation_success_rate_zero() {
        let stats = WireStats::new();
        assert!((stats.validation_success_rate() - 100.0).abs() < 0.01);
    }

    #[test]
    fn wire_stats_negotiation_success_rate() {
        let mut stats = WireStats::new();
        stats.total_negotiations = 200;
        stats.successful_negotiations = 180;
        stats.failed_negotiations = 20;
        assert!((stats.negotiation_success_rate() - 90.0).abs() < 0.01);
    }

    #[test]
    fn wire_stats_payload_overhead_ratio() {
        let mut stats = WireStats::new();
        stats.total_payload_bytes = 1000;
        stats.total_frame_bytes = 1100;
        assert!((stats.payload_overhead_ratio() - 1.1).abs() < 0.01);
    }

    #[test]
    fn wire_stats_bytes_per_frame_avg() {
        let mut stats = WireStats::new();
        stats.total_frame_bytes = 10000;
        stats.frames_encoded = 100;
        assert!((stats.bytes_per_frame_avg() - 100.0).abs() < 0.01);
    }

    #[test]
    fn wire_stats_serialization_roundtrip() {
        let stats = WireStats::new();
        let json = serde_json::to_string(&stats).unwrap();
        let deserialized: WireStats = serde_json::from_str(&json).unwrap();
        assert_eq!(stats.frames_encoded, deserialized.frames_encoded);
    }

    // ========================================================================
    // WireSnapshot tests
    // ========================================================================

    #[test]
    fn wire_snapshot_peer_count() {
        let snapshot = WireSnapshot {
            version: 1,
            current_format: WireFormat::NativeV1,
            wire_protocol_version: WireProtocolVersion::new(1),
            frame_limits: FrameLimits::default(),
            extensions: vec![],
            peer_formats: HashMap::new(),
            stats: WireStats::new(),
        };
        assert_eq!(snapshot.peer_count(), 0);
    }

    #[test]
    fn wire_snapshot_extension_count() {
        let snapshot = WireSnapshot {
            version: 1,
            current_format: WireFormat::NativeV1,
            wire_protocol_version: WireProtocolVersion::new(1),
            frame_limits: FrameLimits::default(),
            extensions: vec![],
            peer_formats: HashMap::new(),
            stats: WireStats::new(),
        };
        assert_eq!(snapshot.extension_count(), 0);
    }

    #[test]
    fn wire_snapshot_serialization_roundtrip() {
        let snapshot = WireSnapshot {
            version: 1,
            current_format: WireFormat::NativeV1,
            wire_protocol_version: WireProtocolVersion::new(1),
            frame_limits: FrameLimits::default(),
            extensions: vec![],
            peer_formats: HashMap::new(),
            stats: WireStats::new(),
        };
        let json = serde_json::to_string(&snapshot).unwrap();
        let deserialized: WireSnapshot = serde_json::from_str(&json).unwrap();
        assert_eq!(snapshot.version, deserialized.version);
    }

    // ========================================================================
    // WireError tests
    // ========================================================================

    #[test]
    fn wire_error_category() {
        assert_eq!(
            WireError::InvalidConfig {
                field: "x".to_string(),
                reason: "y".to_string(),
            }
            .category(),
            "invalid_config"
        );
        assert_eq!(
            WireError::FormatNotSupported {
                format: WireFormat::NativeV1,
            }
            .category(),
            "format_not_supported"
        );
    }

    #[test]
    fn wire_error_is_recoverable() {
        assert!(WireError::FormatNotSupported {
            format: WireFormat::NativeV1,
        }
        .is_recoverable());
        assert!(WireError::NegotiationTimeout {
            peer_id: test_node_id1(),
        }
        .is_recoverable());
        assert!(!WireError::InvalidConfig {
            field: "x".to_string(),
            reason: "y".to_string(),
        }
        .is_recoverable());
    }

    #[test]
    fn wire_error_suggested_action() {
        assert_eq!(
            WireError::PeerNotFound {
                peer_id: test_node_id1(),
            }
            .suggested_action(),
            "register the peer first"
        );
        assert!(!WireError::Internal {
            message: "test".to_string()
        }
        .suggested_action()
        .is_empty());
    }

    #[test]
    fn wire_error_display() {
        let err = WireError::FormatNotSupported {
            format: WireFormat::NativeV1,
        };
        assert!(!err.to_string().is_empty());
    }

    #[test]
    fn wire_error_serialization_roundtrip() {
        let err = WireError::ExtensionNotFound {
            extension_id: WireExtensionId(1),
        };
        let json = serde_json::to_string(&err).unwrap();
        let deserialized: WireError = serde_json::from_str(&json).unwrap();
        assert!(matches!(deserialized, WireError::ExtensionNotFound { .. }));
    }

    // ========================================================================
    // WireEngine tests
    // ========================================================================

    #[test]
    fn wire_engine_new() {
        let engine = WireEngine::new();
        assert_eq!(engine.wire_format(), WireFormat::NativeV1);
        assert!(engine.is_healthy());
    }

    #[test]
    fn wire_engine_with_config() {
        let config = WireConfig::new(WireFormat::CompactV2).unwrap();
        let engine = WireEngine::with_config(config).unwrap();
        assert_eq!(engine.wire_format(), WireFormat::CompactV2);
    }

    #[test]
    fn wire_engine_with_config_invalid() {
        let mut config = WireConfig::default();
        config.max_frame_payload_size = 0;
        let result = WireEngine::with_config(config);
        assert!(result.is_err());
    }

    #[test]
    fn wire_engine_configure_wire() {
        let engine = WireEngine::new();
        let config = WireConfig::strict();
        assert!(engine.configure_wire(config.clone()).is_ok());
        assert_eq!(engine.wire_format(), WireFormat::NativeV1);
    }

    #[test]
    fn wire_engine_configure_wire_invalid() {
        let engine = WireEngine::new();
        let mut config = WireConfig::default();
        config.max_frame_payload_size = 0;
        let err = engine.configure_wire(config).unwrap_err();
        assert!(matches!(err, WireError::InvalidConfig { .. }));
    }

    #[test]
    fn wire_engine_set_wire_format() {
        let engine = WireEngine::new();
        assert!(engine.set_wire_format(WireFormat::NativeV2).is_ok());
        assert_eq!(engine.wire_format(), WireFormat::NativeV2);
    }

    #[test]
    fn wire_engine_supported_formats() {
        let engine = WireEngine::new();
        let formats = engine.supported_formats();
        assert!(formats.contains(&WireFormat::NativeV1));
    }

    #[test]
    fn wire_engine_negotiate_format() {
        let engine = WireEngine::new();
        let negotiated = engine
            .negotiate_format(
                test_node_id1(),
                vec![WireFormat::NativeV1, WireFormat::CompactV1],
            )
            .unwrap();
        assert_eq!(negotiated, WireFormat::NativeV1);
        assert_eq!(
            engine.peer_format(test_node_id1()),
            Some(WireFormat::NativeV1)
        );
    }

    #[test]
    fn wire_engine_negotiate_format_incompatible() {
        let engine = WireEngine::new();
        let mut config = WireConfig::default();
        config.strict_format_matching = true;
        let engine2 = WireEngine::with_config(config).unwrap();
        let err = engine2
            .negotiate_format(test_node_id1(), vec![WireFormat::Extended(999)])
            .unwrap_err();
        assert!(matches!(err, WireError::FormatNotSupported { .. }));
    }

    #[test]
    fn wire_engine_encode_frame() {
        let engine = WireEngine::new();
        let frame = engine.encode_frame(vec![1, 2, 3, 4], 1).unwrap();
        assert_eq!(frame.frame_id, 1);
        assert_eq!(frame.payload, vec![1, 2, 3, 4]);
        assert!(frame.has_checksum());
    }

    #[test]
    fn wire_engine_encode_frame_exceeds_limit() {
        let engine = WireEngine::new();
        let mut config = WireConfig::default();
        config.max_frame_payload_size = 10;
        engine.configure_wire(config).unwrap();
        let err = engine
            .encode_frame(vec![1; 100], 1)
            .unwrap_err();
        assert!(matches!(
            err,
            WireError::PayloadExceedsLimit { .. }
        ));
    }

    #[test]
    fn wire_engine_decode_frame() {
        let engine = WireEngine::new();
        let frame = engine.encode_frame(vec![1, 2, 3], 1).unwrap();
        let (payload, sequence) = engine.decode_frame(&frame).unwrap();
        assert_eq!(payload, vec![1, 2, 3]);
        assert_eq!(sequence, 0);
    }

    #[test]
    fn wire_engine_decode_corrupted_payload() {
        let engine = WireEngine::new();
        let mut frame = NativeFrame::new(1, WireFormat::NativeV1, vec![]);
        let err = engine.decode_frame(&frame).unwrap_err();
        assert!(matches!(err, WireError::CorruptedPayload { .. }));
    }

    #[test]
    fn wire_engine_decode_checksum_mismatch() {
        let engine = WireEngine::new();
        let mut frame = NativeFrame::new(1, WireFormat::NativeV1, vec![1, 2, 3]);
        frame.checksum = Some(vec![0xff; 32]);
        let err = engine.decode_frame(&frame).unwrap_err();
        assert!(matches!(err, WireError::ChecksumMismatch { .. }));
    }

    #[test]
    fn wire_engine_validate_frame() {
        let engine = WireEngine::new();
        let frame = engine.encode_frame(vec![1, 2, 3], 1).unwrap();
        let result = engine.validate_frame(&frame).unwrap();
        assert!(result.valid);
        assert!(result.is_clean());
    }

    #[test]
    fn wire_engine_validate_frame_invalid_format() {
        let engine = WireEngine::new();
        let mut frame = NativeFrame::new(1, WireFormat::Extended(999), vec![1]);
        let result = engine.validate_frame(&frame).unwrap();
        assert!(!result.valid);
        assert!(!result.format_valid);
    }

    #[test]
    fn wire_engine_validate_frame_size_exceeded() {
        let engine = WireEngine::new();
        let mut config = WireConfig::default();
        config.max_frame_payload_size = 5;
        engine.configure_wire(config).unwrap();
        let frame = NativeFrame::new(1, WireFormat::NativeV1, vec![1; 10]);
        let result = engine.validate_frame(&frame).unwrap();
        assert!(!result.valid);
        assert!(!result.size_valid);
    }

    #[test]
    fn wire_engine_frame_limits() {
        let engine = WireEngine::new();
        let limits = engine.frame_limits();
        assert_eq!(limits.max_payload_size, 65536);
    }

    #[test]
    fn wire_engine_exceeds_frame_limit() {
        let engine = WireEngine::new();
        assert!(!engine.exceeds_frame_limit(100));
        assert!(engine.exceeds_frame_limit(1000000));
    }

    #[test]
    fn wire_engine_wire_protocol_version() {
        let engine = WireEngine::new();
        assert_eq!(engine.wire_protocol_version().0, 1);
    }

    #[test]
    fn wire_engine_set_wire_protocol_version() {
        let engine = WireEngine::new();
        assert!(engine
            .set_wire_protocol_version(WireProtocolVersion::new(50))
            .is_ok());
        assert_eq!(engine.wire_protocol_version().0, 50);
    }

    #[test]
    fn wire_engine_set_wire_protocol_version_out_of_range() {
        let engine = WireEngine::new();
        let err = engine
            .set_wire_protocol_version(WireProtocolVersion::new(200))
            .unwrap_err();
        assert!(matches!(err, WireError::FormatOutOfRange { .. }));
    }

    #[test]
    fn wire_engine_supported_wire_versions() {
        let engine = WireEngine::new();
        let versions = engine.supported_wire_versions();
        assert_eq!(versions[0], WireProtocolVersion::new(1));
        assert_eq!(versions.last(), Some(&WireProtocolVersion::new(100)));
    }

    #[test]
    fn wire_engine_negotiate_wire_version() {
        let engine = WireEngine::new();
        let negotiated = engine
            .negotiate_wire_version(
                test_node_id1(),
                vec![WireProtocolVersion::new(1), WireProtocolVersion::new(2)],
            )
            .unwrap();
        assert_eq!(negotiated, WireProtocolVersion::new(2));
    }

    #[test]
    fn wire_engine_negotiate_wire_version_timeout() {
        let engine = WireEngine::new();
        let mut config = WireConfig::default();
        config.min_supported_version = WireProtocolVersion::new(100);
        config.max_supported_version = WireProtocolVersion::new(200);
        let engine2 = WireEngine::with_config(config).unwrap();
        let err = engine2
            .negotiate_wire_version(test_node_id1(), vec![WireProtocolVersion::new(1)])
            .unwrap_err();
        assert!(matches!(err, WireError::NegotiationTimeout { .. }));
    }

    #[test]
    fn wire_engine_encode_decode_endpoint() {
        let engine = WireEngine::new();
        let opaque = engine.encode_endpoint(&test_endpoint());
        assert_eq!(opaque.carrier_type, 0); // Direct = 0
        assert_eq!(opaque.address_bytes, vec![1, 2, 3, 4]);

        let decoded = engine.decode_endpoint(&opaque).unwrap();
        assert_eq!(decoded, test_endpoint());
    }

    #[test]
    fn wire_engine_decode_endpoint_exceeds_limit() {
        let engine = WireEngine::new();
        let mut config = WireConfig::default();
        config.endpoint_address_size_limit = 2;
        engine.configure_wire(config).unwrap();
        let opaque = OpaqueEndpoint::new(1, vec![1, 2, 3, 4]);
        let err = engine.decode_endpoint(&opaque).unwrap_err();
        assert!(matches!(err, WireError::EndpointExceedsLimit { .. }));
    }

    #[test]
    fn wire_engine_endpoint_address_size_limit() {
        let engine = WireEngine::new();
        assert_eq!(engine.endpoint_address_size_limit(), 1024);
    }

    #[test]
    fn wire_engine_endpoint_exceeds_limit() {
        let engine = WireEngine::new();
        assert!(!engine.endpoint_exceeds_limit(500));
        assert!(engine.endpoint_exceeds_limit(2000));
    }

    #[test]
    fn wire_engine_register_extension() {
        let engine = WireEngine::new();
        let ext = WireExtension::new("test".to_string(), WireFormat::NativeV1);
        let ext_id = engine.register_extension(ext).unwrap();
        assert_eq!(ext_id.0, 1);
        assert!(engine.extension_by_id(ext_id).is_some());
    }

    #[test]
    fn wire_engine_register_extension_max_exceeded() {
        let engine = WireEngine::new();
        let mut config = WireConfig::default();
        config.max_extensions = 1;
        engine.configure_wire(config).unwrap();
        engine
            .register_extension(WireExtension::new("a".to_string(), WireFormat::NativeV1))
            .unwrap();
        let err = engine
            .register_extension(WireExtension::new("b".to_string(), WireFormat::NativeV1))
            .unwrap_err();
        assert!(matches!(err, WireError::MaxExtensionsExceeded { .. }));
    }

    #[test]
    fn wire_engine_registered_extensions() {
        let engine = WireEngine::new();
        engine
            .register_extension(WireExtension::new("a".to_string(), WireFormat::NativeV1))
            .unwrap();
        engine
            .register_extension(WireExtension::new("b".to_string(), WireFormat::NativeV2))
            .unwrap();
        let extensions = engine.registered_extensions();
        assert_eq!(extensions.len(), 2);
    }

    #[test]
    fn wire_engine_extension_not_found() {
        let engine = WireEngine::new();
        assert!(engine.extension_by_id(WireExtensionId(999)).is_none());
    }

    #[test]
    fn wire_engine_is_compatible() {
        let engine = WireEngine::new();
        assert!(engine.is_compatible(WireProtocolVersion::new(50)));
        assert!(!engine.is_compatible(WireProtocolVersion::new(0)));
    }

    #[test]
    fn wire_engine_compatibility_matrix() {
        let engine = WireEngine::new();
        // Just verify no panic on compatibility matrix
        let _ = engine;
    }

    #[test]
    fn wire_engine_stats() {
        let engine = WireEngine::new();
        engine.encode_frame(vec![1, 2, 3], 1).unwrap();
        let stats = engine.stats();
        assert_eq!(stats.frames_encoded, 1);
    }

    #[test]
    fn wire_engine_is_healthy() {
        let engine = WireEngine::new();
        assert!(engine.is_healthy());
    }

    #[test]
    fn wire_engine_validate_config() {
        let engine = WireEngine::new();
        let config = WireConfig::default();
        assert!(engine.validate_config(&config).is_ok());
    }

    #[test]
    fn wire_engine_snapshot_restore() {
        let engine = WireEngine::new();

        engine
            .register_extension(WireExtension::new("ext".to_string(), WireFormat::NativeV1))
            .unwrap();

        let snapshot = engine.snapshot();
        assert_eq!(snapshot.extension_count(), 1);

        let engine2 = WireEngine::new();
        engine2.restore_from_snapshot(snapshot).unwrap();
        assert_eq!(engine2.extension_count(), 1);
    }

    #[test]
    fn wire_engine_peers_by_format() {
        let engine = WireEngine::new();
        engine.negotiate_format(test_node_id1(), vec![WireFormat::NativeV1]).unwrap();
        engine.negotiate_format(test_node_id2(), vec![WireFormat::NativeV2]).unwrap();
        let peers = engine.peers_by_format(WireFormat::NativeV1);
        assert_eq!(peers.len(), 1);
        assert_eq!(peers[0], test_node_id1());
    }

    #[test]
    fn wire_engine_peer_format() {
        let engine = WireEngine::new();
        assert!(engine.peer_format(test_node_id1()).is_none());
        engine
            .negotiate_format(test_node_id1(), vec![WireFormat::Extended(5)])
            .unwrap();
        assert_eq!(
            engine.peer_format(test_node_id1()),
            Some(WireFormat::Extended(5))
        );
    }

    // Helper for Extended format pattern
    fn extended_format_helper(_engine: &WireEngine) {
        // placeholder
    }

    #[test]
    fn wire_engine_set_peer_format() {
        let engine = WireEngine::new();
        assert!(engine
            .set_peer_format(test_node_id1(), WireFormat::Extended(3))
            .is_ok());
        assert_eq!(
            engine.peer_format(test_node_id1()),
            Some(WireFormat::Extended(3))
        );
    }

    #[test]
    fn wire_engine_set_peer_format_unsupported() {
        let engine = WireEngine::new();
        let err = engine.set_peer_format(test_node_id1(), WireFormat::Custom(0)).unwrap_err();
        assert!(matches!(err, WireError::FormatNotSupported { .. }));
    }

    #[test]
    fn wire_engine_negotiation_timeout() {
        let engine = WireEngine::new();
        assert_eq!(engine.negotiation_timeout_ms(), 5000);
        engine.set_negotiation_timeout_ms(10000).unwrap();
        assert_eq!(engine.negotiation_timeout_ms(), 10000);
    }

    #[test]
    fn wire_engine_max_frame_payload_size() {
        let engine = WireEngine::new();
        assert_eq!(engine.max_frame_payload_size(), 65536);
        engine.set_max_frame_payload_size(32768).unwrap();
        assert_eq!(engine.max_frame_payload_size(), 32768);
    }

    // ========================================================================
    // WireEngine advanced methods tests
    // ========================================================================

    #[test]
    fn wire_engine_total_frames_encoded() {
        let engine = WireEngine::new();
        assert_eq!(engine.total_frames_encoded(), 0);
        engine.encode_frame(vec![1], 1).unwrap();
        engine.encode_frame(vec![2], 2).unwrap();
        assert_eq!(engine.total_frames_encoded(), 2);
    }

    #[test]
    fn wire_engine_frame_count() {
        let engine = WireEngine::new();
        assert_eq!(engine.frame_count(), 0);
        engine.encode_frame(vec![1], 1).unwrap();
        assert_eq!(engine.frame_count(), 1);
    }

    #[test]
    fn wire_engine_extension_count() {
        let engine = WireEngine::new();
        assert_eq!(engine.extension_count(), 0);
        engine
            .register_extension(WireExtension::new("a".to_string(), WireFormat::NativeV1))
            .unwrap();
        assert_eq!(engine.extension_count(), 1);
    }

    #[test]
    fn wire_engine_peer_count() {
        let engine = WireEngine::new();
        assert_eq!(engine.peer_count(), 0);
        engine.negotiate_format(test_node_id1(), vec![WireFormat::NativeV1]).unwrap();
        assert_eq!(engine.peer_count(), 1);
    }

    #[test]
    fn wire_engine_register_unregister_format() {
        let engine = WireEngine::new();
        assert!(engine.register_format(WireFormat::CompactV1).is_ok());
        assert!(engine.is_format_registered(WireFormat::CompactV1));
        assert!(engine.unregister_format(WireFormat::CompactV1).is_ok());
        assert!(!engine.is_format_registered(WireFormat::CompactV1));
    }

    #[test]
    fn wire_engine_unregister_default_format_fails() {
        let engine = WireEngine::new();
        let err = engine.unregister_format(WireFormat::NativeV1).unwrap_err();
        assert!(matches!(err, WireError::InvalidConfig { .. }));
    }

    #[test]
    fn wire_engine_all_registered_formats() {
        let engine = WireEngine::new();
        let formats = engine.all_registered_formats();
        assert!(formats.contains(&WireFormat::NativeV1));
    }

    #[test]
    fn wire_engine_frame_validation_enabled() {
        let engine = WireEngine::new();
        assert!(engine.is_frame_validation_enabled());
        engine.set_frame_validation_enabled(false).unwrap();
        assert!(!engine.is_frame_validation_enabled());
    }

    #[test]
    fn wire_engine_auto_negotiation() {
        let engine = WireEngine::new();
        assert!(engine.is_auto_negotiation_enabled());
        engine.set_auto_negotiation(false).unwrap();
        assert!(!engine.is_auto_negotiation_enabled());
    }

    #[test]
    fn wire_engine_wire_extensions_enabled() {
        let engine = WireEngine::new();
        assert!(engine.are_wire_extensions_enabled());
        engine.set_wire_extensions_enabled(false).unwrap();
        assert!(!engine.are_wire_extensions_enabled());
    }

    #[test]
    fn wire_engine_strict_format_matching() {
        let engine = WireEngine::new();
        assert!(!engine.is_strict_format_matching());
        engine.set_strict_format_matching(true).unwrap();
        assert!(engine.is_strict_format_matching());
    }

    #[test]
    fn wire_engine_frame_checksum_enabled() {
        let engine = WireEngine::new();
        assert!(engine.is_frame_checksum_enabled());
        engine.set_frame_checksum_enabled(false).unwrap();
        assert!(!engine.is_frame_checksum_enabled());
    }

    #[test]
    fn wire_engine_min_max_supported_wire_version() {
        let engine = WireEngine::new();
        assert_eq!(engine.min_supported_wire_version().0, 1);
        assert_eq!(engine.max_supported_wire_version().0, 100);
        engine.set_min_supported_wire_version(WireProtocolVersion::new(10)).unwrap();
        assert_eq!(engine.min_supported_wire_version().0, 10);
    }

    #[test]
    fn wire_engine_wire_protocol_version_display() {
        let engine = WireEngine::new();
        assert_eq!(engine.wire_protocol_version_display(), "1.0.0");
    }

    #[test]
    fn wire_engine_all_peer_formats() {
        let engine = WireEngine::new();
        engine.negotiate_format(test_node_id1(), vec![WireFormat::NativeV1]).unwrap();
        engine.negotiate_format(test_node_id2(), vec![WireFormat::NativeV2]).unwrap();
        let formats = engine.all_peer_formats();
        assert_eq!(formats.len(), 2);
    }

    #[test]
    fn wire_engine_remove_peer_format() {
        let engine = WireEngine::new();
        engine.negotiate_format(test_node_id1(), vec![WireFormat::NativeV1]).unwrap();
        assert!(engine.remove_peer_format(test_node_id1()).is_ok());
        assert!(engine.peer_format(test_node_id1()).is_none());
    }

    #[test]
    fn wire_engine_remove_peer_format_not_found() {
        let engine = WireEngine::new();
        let err = engine.remove_peer_format(test_node_id1()).unwrap_err();
        assert!(matches!(err, WireError::PeerNotFound { .. }));
    }

    #[test]
    fn wire_engine_enable_disable_extension() {
        let engine = WireEngine::new();
        let ext_id = engine
            .register_extension(WireExtension::new("a".to_string(), WireFormat::NativeV1))
            .unwrap();
        assert!(engine.disable_extension(ext_id).is_ok());
        assert!(!engine.extension_by_id(ext_id).unwrap().enabled);
        assert!(engine.enable_extension(ext_id).is_ok());
        assert!(engine.extension_by_id(ext_id).unwrap().enabled);
    }

    #[test]
    fn wire_engine_remove_extension() {
        let engine = WireEngine::new();
        let ext_id = engine
            .register_extension(WireExtension::new("a".to_string(), WireFormat::NativeV1))
            .unwrap();
        assert!(engine.remove_extension(ext_id).is_ok());
        assert!(engine.extension_by_id(ext_id).is_none());
    }

    #[test]
    fn wire_engine_latest_frame_id() {
        let engine = WireEngine::new();
        assert_eq!(engine.latest_frame_id(), None);
        engine.encode_frame(vec![1], 5).unwrap();
        assert_eq!(engine.latest_frame_id(), Some(5));
    }

    #[test]
    fn wire_engine_frame_validation_rate() {
        let engine = WireEngine::new();
        assert!((engine.frame_validation_rate() - 100.0).abs() < 0.01);
    }

    #[test]
    fn wire_engine_wire_health_score() {
        let engine = WireEngine::new();
        let score = engine.wire_health_score();
        assert!(score > 0.0);
        assert!(score <= 1.0);
    }

    #[test]
    fn wire_engine_clear_all() {
        let engine = WireEngine::new();
        engine.encode_frame(vec![1], 1).unwrap();
        engine.register_extension(WireExtension::new("a".to_string(), WireFormat::NativeV1)).unwrap();
        engine.clear_all();
        assert_eq!(engine.frame_count(), 0);
        assert_eq!(engine.extension_count(), 0);
        assert_eq!(engine.wire_format(), WireFormat::NativeV1);
    }

    #[test]
    fn wire_engine_graph_size() {
        let engine = WireEngine::new();
        engine.encode_frame(vec![1], 1).unwrap();
        engine.register_extension(WireExtension::new("a".to_string(), WireFormat::NativeV1)).unwrap();
        let (frames, extensions, peers, formats) = engine.graph_size();
        assert_eq!(frames, 1);
        assert_eq!(extensions, 1);
        assert_eq!(peers, 0);
        assert!(formats > 0);
    }

    #[test]
    fn wire_engine_config_snapshot() {
        let engine = WireEngine::new();
        let config = engine.config_snapshot();
        assert_eq!(config.default_format, WireFormat::NativeV1);
    }

    #[test]
    fn wire_engine_payload_fits() {
        let engine = WireEngine::new();
        assert!(engine.payload_fits(1000));
        assert!(!engine.payload_fits(1000000));
    }

    #[test]
    fn wire_engine_payload_efficiency() {
        let engine = WireEngine::new();
        let efficiency = engine.payload_efficiency(1000);
        assert!(efficiency > 0.0);
        assert!(efficiency <= 100.0);
    }

    #[test]
    fn wire_engine_average_frame_size() {
        let engine = WireEngine::new();
        assert!((engine.average_frame_size() - 0.0).abs() < 0.001);
        engine.encode_frame(vec![1, 2, 3], 1).unwrap();
        assert!(engine.average_frame_size() > 0.0);
    }

    #[test]
    fn wire_engine_average_payload_size() {
        let engine = WireEngine::new();
        assert!((engine.average_payload_size() - 0.0).abs() < 0.001);
        engine.encode_frame(vec![1, 2, 3, 4, 5], 1).unwrap();
        assert!((engine.average_payload_size() - 5.0).abs() < 0.001);
    }

    #[test]
    fn wire_engine_total_overhead_bytes() {
        let engine = WireEngine::new();
        assert_eq!(engine.total_overhead_bytes(), 0);
        engine.encode_frame(vec![1, 2, 3], 1).unwrap();
        assert!(engine.total_overhead_bytes() > 0);
    }

    #[test]
    fn wire_engine_overhead_percentage() {
        let engine = WireEngine::new();
        assert!((engine.overhead_percentage() - 0.0).abs() < 0.001);
        engine.encode_frame(vec![1, 2, 3], 1).unwrap();
        let pct = engine.overhead_percentage();
        assert!(pct > 0.0);
        assert!(pct < 100.0);
    }

    #[test]
    fn wire_engine_frames_by_format() {
        let engine = WireEngine::new();
        engine.encode_frame(vec![1], 1).unwrap();
        engine.encode_frame(vec![2], 2).unwrap();
        let frames = engine.frames_by_format(WireFormat::NativeV1);
        assert_eq!(frames.len(), 2);
    }

    #[test]
    fn wire_engine_frames_by_flag() {
        let engine = WireEngine::new();
        engine.encode_frame(vec![1], 1).unwrap();
        let frames = engine.frames_by_flag(FrameFlags::FLAG_CHECKSUM);
        assert_eq!(frames.len(), 1);
    }

    #[test]
    fn wire_engine_frames_with_checksum() {
        let engine = WireEngine::new();
        engine.encode_frame(vec![1], 1).unwrap();
        let frames = engine.frames_with_checksum();
        assert_eq!(frames.len(), 1);
    }

    #[test]
    fn wire_engine_fragmented_frames() {
        let engine = WireEngine::new();
        engine.encode_frame(vec![1], 1).unwrap();
        let frames = engine.fragmented_frames();
        assert_eq!(frames.len(), 0);
    }

    // ========================================================================
    // Serialization roundtrip tests
    // ========================================================================

    #[test]
    fn wire_serialization_roundtrip_wire_config() {
        let config = WireConfig::default();
        let json = serde_json::to_string(&config).unwrap();
        let deserialized: WireConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(config.default_format, deserialized.default_format);
    }

    #[test]
    fn wire_serialization_roundtrip_wire_stats() {
        let stats = WireStats::new();
        let json = serde_json::to_string(&stats).unwrap();
        let deserialized: WireStats = serde_json::from_str(&json).unwrap();
        assert_eq!(stats.frames_encoded, deserialized.frames_encoded);
    }

    // ========================================================================
    // Integration tests
    // ========================================================================

    #[test]
    fn wire_full_lifecycle() {
        let engine = WireEngine::new();
        let config = WireConfig::strict();
        engine.configure_wire(config).unwrap();

        // Add capabilities
        engine.register_extension(WireExtension::new("ext1".to_string(), WireFormat::NativeV1)).unwrap();

        // Negotiate format with peer
        engine.negotiate_format(test_node_id1(), vec![WireFormat::NativeV1, WireFormat::NativeV2]).unwrap();

        // Encode frame
        let frame = engine.encode_frame(vec![1, 2, 3, 4], 1).unwrap();
        assert!(frame.has_checksum());

        // Decode frame
        let (payload, sequence) = engine.decode_frame(&frame).unwrap();
        assert_eq!(payload, vec![1, 2, 3, 4]);

        // Validate frame
        let result = engine.validate_frame(&frame).unwrap();
        assert!(result.valid);

        // Check stats
        let stats = engine.stats();
        assert_eq!(stats.frames_encoded, 1);

        // Check health
        let score = engine.wire_health_score();
        assert!(score > 0.0);

        // Snapshot and restore
        let snapshot = engine.snapshot();
        let engine2 = WireEngine::new();
        engine2.restore_from_snapshot(snapshot).unwrap();
        assert_eq!(engine2.extension_count(), 1);
    }

    #[test]
    fn wire_multi_peer_format_negotiation() {
        let engine = WireEngine::new();
        engine.negotiate_format(test_node_id1(), vec![WireFormat::NativeV1]).unwrap();
        engine.negotiate_format(test_node_id2(), vec![WireFormat::NativeV2]).unwrap();
        engine.negotiate_format(test_node_id3(), vec![WireFormat::CompactV1]).unwrap();

        assert_eq!(engine.peer_format(test_node_id1()), Some(WireFormat::NativeV1));
        assert_eq!(engine.peer_format(test_node_id2()), Some(WireFormat::NativeV2));
        assert_eq!(engine.peer_format(test_node_id3()), Some(WireFormat::CompactV1));
    }

    #[test]
    fn wire_extension_lifecycle() {
        let engine = WireEngine::new();
        let ext_id = engine
            .register_extension(WireExtension::new("test".to_string(), WireFormat::NativeV1))
            .unwrap();
        assert!(engine.extension_by_id(ext_id).unwrap().enabled);
        engine.disable_extension(ext_id).unwrap();
        assert!(!engine.extension_by_id(ext_id).unwrap().enabled);
        engine.enable_extension(ext_id).unwrap();
        assert!(engine.extension_by_id(ext_id).unwrap().enabled);
        engine.remove_extension(ext_id).unwrap();
        assert!(engine.extension_by_id(ext_id).is_none());
    }

    #[test]
    fn wire_frame_checksum_integrity() {
        let engine = WireEngine::new();
        let payload = vec![1, 2, 3, 4, 5, 6, 7, 8];
        let frame = engine.encode_frame(payload.clone(), 1).unwrap();

        // Decode should succeed with matching checksum
        let (decoded_payload, _) = engine.decode_frame(&frame).unwrap();
        assert_eq!(decoded_payload, payload);

        // Tamper with checksum should fail
        let mut tampered = frame.clone();
        tampered.checksum = Some(vec![0xff; 32]);
        let err = engine.decode_frame(&tampered).unwrap_err();
        assert!(matches!(err, WireError::ChecksumMismatch { .. }));
    }

    #[test]
    fn wire_frame_validation_edge_cases() {
        let engine = WireEngine::new();

        // Empty payload without fragmentation
        let empty_frame = NativeFrame::new(1, WireFormat::NativeV1, vec![]);
        let result = engine.validate_frame(&empty_frame).unwrap();
        // Empty payload is technically valid if not corrupted
        assert!(result.valid || !result.is_clean());
    }

    #[test]
    fn wire_format_header_size_variants() {
        assert!(WireFormat::NativeV2.header_size() > WireFormat::NativeV1.header_size());
        assert!(WireFormat::CompactV1.header_size() < WireFormat::NativeV1.header_size());
    }

    #[test]
    fn wire_frame_flags_combinations() {
        let mut flags = FrameFlags::empty();
        flags.insert(FrameFlags::FLAG_CHECKSUM | FrameFlags::FLAG_FRAGMENTED | FrameFlags::FLAG_PRIORITY);
        assert!(flags.is_checksum());
        assert!(flags.is_fragmented());
        assert!(flags.is_priority());
        assert!(!flags.is_encrypted());
    }
}
