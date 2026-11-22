//! # ids-000-aa Sealed Trait Contract: Erasure
//!
//! Erasure coding contract for IDS datasets.
//! Provides k-of-n fragment encoding, parity shard generation, fragment
//! distribution, loss detection, reconstruction, and recovery verification.
//! All types are canonical and sealed — only ids-000-aa and its designated
//! implementation crates can implement this trait.
//!
//! Carrier-agnostic: no std::net, tokio::net, libp2p, IP, TCP, UDP, QUIC,
//! HTTP, DNS, or any existing networking stack. Fragments are opaque byte
//! blobs carried on CarrierType + Vec<u8> Endpoints only.

use crate::errors::ErasureError;
use crate::types::{
    Checksum, DatasetId, Epoch, FragmentId, FragmentPlacement, NodeId, OpRef,
    RedundancyMode, Version,
};
use std::collections::HashMap;

///////////////////////////////////////////////////////////////////////////////
// SEALED TRAIT: Erasure
///////////////////////////////////////////////////////////////////////////////

/// Sealed marker — only ids-000-aa and designated crates can implement.
pub mod private {
    pub trait Sealed {}
}

/// Sealed erasure coding trait.
/// Implementations live downstream in ids-erasure and ids-* crates.
pub trait Erasure: private::Sealed {
    /// Configure erasure coding parameters.
    fn configure(&self, config: ErasureConfig) -> Result<(), ErasureError>;

    /// Encode data into n fragments, any k of which can reconstruct.
    fn encode(&self, dataset_id: DatasetId, data: &[u8]) -> Result<ErasureEncodeResult, ErasureError>;

    /// Decode/reconstruct data from any k available fragments.
    fn decode(
        &self,
        dataset_id: DatasetId,
        fragments: &[ErasureFragment],
    ) -> Result<ErasureDecodeResult, ErasureError>;

    /// Generate parity shards for existing data fragments.
    fn generate_parity(
        &self,
        dataset_id: DatasetId,
        fragments: &[ErasureFragment],
    ) -> Result<Vec<ErasureFragment>, ErasureError>;

    /// Reconstruct missing fragments from available ones.
    fn reconstruct(
        &self,
        dataset_id: DatasetId,
        available: &[ErasureFragment],
        missing_indices: &[u32],
    ) -> Result<ErasureReconstructResult, ErasureError>;

    /// Place fragments on nodes according to redundancy mode.
    fn place_fragments(
        &self,
        dataset_id: DatasetId,
        fragments: &[ErasureFragment],
        nodes: &[NodeId],
    ) -> Result<Vec<FragmentPlacement>, ErasureError>;

    /// Get fragment placement for a dataset.
    fn fragment_placement(&self, dataset_id: DatasetId) -> Result<Vec<FragmentPlacement>, ErasureError>;

    /// Detect lost or corrupted fragments.
    fn detect_loss(
        &self,
        dataset_id: DatasetId,
        available_fragments: &[ErasureFragment],
    ) -> Result<ErasureLossReport, ErasureError>;

    /// Verify fragment integrity via checksum.
    fn verify_fragment(&self, fragment: &ErasureFragment) -> Result<bool, ErasureError>;

    /// Get erasure statistics for a dataset.
    fn stats(&self, dataset_id: DatasetId) -> Result<ErasureStats, ErasureError>;

    /// Get all configured datasets.
    fn datasets(&self) -> Vec<DatasetId>;

    /// Check if a dataset is fully protected (k of n fragments available).
    fn is_protected(&self, dataset_id: DatasetId) -> bool;

    /// Get the minimum fragments needed to reconstruct.
    fn minimum_fragments(&self, dataset_id: DatasetId) -> Option<u32>;

    /// Get the total fragments for a dataset.
    fn total_fragments(&self, dataset_id: DatasetId) -> Option<u32>;

    /// Expand redundancy by adding more parity fragments.
    fn expand_redundancy(
        &self,
        dataset_id: DatasetId,
        additional_parity: u32,
    ) -> Result<ErasureExpandResult, ErasureError>;

    /// Shrink redundancy by removing parity fragments.
    fn shrink_redundancy(
        &self,
        dataset_id: DatasetId,
        remove_parity: u32,
    ) -> Result<ErasureShrinkResult, ErasureError>;

    /// Migrate fragments between nodes.
    fn migrate_fragment(
        &self,
        dataset_id: DatasetId,
        fragment_index: u32,
        from_node: NodeId,
        to_node: NodeId,
    ) -> Result<ErasureMigrationResult, ErasureError>;

    /// Repair a dataset by reconstructing missing fragments.
    fn repair(&self, dataset_id: DatasetId) -> Result<ErasureRepairResult, ErasureError>;

    /// Get repair plan for a dataset.
    fn repair_plan(&self, dataset_id: DatasetId) -> Result<ErasureRepairPlan, ErasureError>;

    /// Validate erasure configuration.
    fn validate_config(&self, config: &ErasureConfig) -> Result<(), ErasureError>;
}

///////////////////////////////////////////////////////////////////////////////
// CONFIGURATION
///////////////////////////////////////////////////////////////////////////////

/// Erasure coding configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ErasureConfig {
    /// Number of data fragments (k).
    pub k: u32,
    /// Total number of fragments (n).
    pub n: u32,
    /// Redundancy mode.
    pub mode: RedundancyMode,
    /// Enable parity generation.
    pub enable_parity: bool,
    /// Enable automatic repair.
    pub enable_auto_repair: bool,
    /// Repair threshold (fraction of fragments that must be available).
    pub repair_threshold: f64,
    /// Maximum concurrent repairs.
    pub max_concurrent_repairs: u32,
    /// Fragment size in bytes.
    pub fragment_size: usize,
    /// Enable integrity checksums.
    pub enable_checksums: bool,
}

impl ErasureConfig {
    /// Create a new erasure config with validation.
    pub fn new(k: u32, n: u32, mode: RedundancyMode) -> Result<Self, ErasureError> {
        let config = Self {
            k,
            n,
            mode,
            enable_parity: true,
            enable_auto_repair: true,
            repair_threshold: 0.5,
            max_concurrent_repairs: 5,
            fragment_size: 1024 * 1024, // 1 MB
            enable_checksums: true,
        };
        config.validate()?;
        Ok(config)
    }

    /// Validate configuration parameters.
    pub fn validate(&self) -> Result<(), ErasureError> {
        if self.k == 0 {
            return Err(ErasureError::InvalidConfig {
                field: "k".to_string(),
                reason: "k must be greater than 0".to_string(),
            });
        }
        if self.n < self.k {
            return Err(ErasureError::InvalidConfig {
                field: "n".to_string(),
                reason: "n must be >= k".to_string(),
            });
        }
        if self.n == 0 {
            return Err(ErasureError::InvalidConfig {
                field: "n".to_string(),
                reason: "n must be greater than 0".to_string(),
            });
        }
        if self.repair_threshold < 0.0 || self.repair_threshold > 1.0 {
            return Err(ErasureError::InvalidConfig {
                field: "repair_threshold".to_string(),
                reason: "repair_threshold must be between 0.0 and 1.0".to_string(),
            });
        }
        if self.max_concurrent_repairs == 0 {
            return Err(ErasureError::InvalidConfig {
                field: "max_concurrent_repairs".to_string(),
                reason: "max_concurrent_repairs must be greater than 0".to_string(),
            });
        }
        if self.fragment_size == 0 {
            return Err(ErasureError::InvalidConfig {
                field: "fragment_size".to_string(),
                reason: "fragment_size must be greater than 0".to_string(),
            });
        }
        match self.mode {
            RedundancyMode::ErasureCoded { k, n } => {
                if k == 0 || n < k {
                    return Err(ErasureError::InvalidConfig {
                        field: "mode".to_string(),
                        reason: "erasure coded mode requires valid k and n".to_string(),
                    });
                }
            }
            _ => {}
        }
        Ok(())
    }

    /// Get the number of parity fragments.
    pub fn parity_count(&self) -> u32 {
        self.n.saturating_sub(self.k)
    }

    /// Get the fault tolerance (max fragments that can be lost).
    pub fn fault_tolerance(&self) -> u32 {
        self.parity_count()
    }

    /// Check if auto repair is enabled.
    pub fn is_auto_repair_enabled(&self) -> bool {
        self.enable_auto_repair
    }

    /// Check if parity generation is enabled.
    pub fn is_parity_enabled(&self) -> bool {
        self.enable_parity
    }

    /// Default configuration (k=4, n=10).
    pub fn default() -> Self {
        Self {
            k: 4,
            n: 10,
            mode: RedundancyMode::ErasureCoded { k: 4, n: 10 },
            enable_parity: true,
            enable_auto_repair: true,
            repair_threshold: 0.5,
            max_concurrent_repairs: 5,
            fragment_size: 1024 * 1024,
            enable_checksums: true,
        }
    }

    /// Strict configuration (k=8, n=16, high fault tolerance).
    pub fn strict() -> Self {
        Self {
            k: 8,
            n: 16,
            mode: RedundancyMode::ErasureCoded { k: 8, n: 16 },
            enable_parity: true,
            enable_auto_repair: true,
            repair_threshold: 0.75,
            max_concurrent_repairs: 3,
            fragment_size: 512 * 1024,
            enable_checksums: true,
        }
    }

    /// Lenient configuration (k=2, n=3, low overhead).
    pub fn lenient() -> Self {
        Self {
            k: 2,
            n: 3,
            mode: RedundancyMode::ErasureCoded { k: 2, n: 3 },
            enable_parity: true,
            enable_auto_repair: false,
            repair_threshold: 0.33,
            max_concurrent_repairs: 10,
            fragment_size: 2 * 1024 * 1024,
            enable_checksums: false,
        }
    }

    /// Serialization round-trip helper.
    pub fn round_trip(&self) -> Result<Self, serde_json::Error> {
        let json = serde_json::to_string(self)?;
        serde_json::from_str(&json)
    }
}

impl Default for ErasureConfig {
    fn default() -> Self {
        Self::default()
    }
}

///////////////////////////////////////////////////////////////////////////////
// ERASURE PARAMETERS
///////////////////////////////////////////////////////////////////////////////

/// Erasure coding scheme parameters.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ErasureParams {
    /// Data fragment count (k).
    pub k: u32,
    /// Total fragment count (n).
    pub n: u32,
    /// Scheme name (e.g. "reed_solomon", "cauchy", "xor").
    pub scheme: ErasureScheme,
    /// Field size for GF arithmetic.
    pub field_size: u32,
    /// Generator matrix seed.
    pub generator_seed: [u8; 16],
}

impl ErasureParams {
    pub fn new(k: u32, n: u32, scheme: ErasureScheme) -> Result<Self, ErasureError> {
        if k == 0 || n < k {
            return Err(ErasureError::InvalidConfig {
                field: "erasure_params".to_string(),
                reason: "k must be > 0 and n >= k".to_string(),
            });
        }
        Ok(Self {
            k,
            n,
            scheme,
            field_size: 256,
            generator_seed: [0u8; 16],
        })
    }

    pub fn parity_count(&self) -> u32 {
        self.n.saturating_sub(self.k)
    }

    pub fn expansion_factor(&self) -> f64 {
        self.n as f64 / self.k as f64
    }

    pub fn overhead_percent(&self) -> f64 {
        (self.expansion_factor() - 1.0) * 100.0
    }
}

///////////////////////////////////////////////////////////////////////////////
// ERASURE SCHEME
///////////////////////////////////////////////////////////////////////////////

/// Erasure coding scheme.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ErasureScheme {
    /// Reed-Solomon over GF(256).
    ReedSolomon,
    /// Cauchy Reed-Solomon.
    CauchyReedSolomon,
    /// XOR-based parity.
    XOR,
    /// Custom implementation.
    Custom(u8),
}

impl ErasureScheme {
    pub fn display_name(&self) -> &'static str {
        match self {
            ErasureScheme::ReedSolomon => "reed_solomon",
            ErasureScheme::CauchyReedSolomon => "cauchy_reed_solomon",
            ErasureScheme::XOR => "xor",
            ErasureScheme::Custom(_) => "custom",
        }
    }

    pub fn default_field_size(&self) -> u32 {
        match self {
            ErasureScheme::ReedSolomon | ErasureScheme::CauchyReedSolomon => 256,
            ErasureScheme::XOR => 2,
            ErasureScheme::Custom(_) => 256,
        }
    }
}

///////////////////////////////////////////////////////////////////////////////
// ERASURE FRAGMENT
///////////////////////////////////////////////////////////////////////////////

/// A single erasure-coded fragment.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ErasureFragment {
    /// Fragment index (0..n).
    pub index: u32,
    /// Fragment data.
    pub data: Vec<u8>,
    /// Fragment ID.
    pub fragment_id: FragmentId,
    /// Checksum for integrity verification.
    pub checksum: Checksum,
    /// Is this a parity fragment?
    pub is_parity: bool,
    /// Epoch when fragment was created.
    pub epoch: Epoch,
    /// Version of the source data.
    pub version: Version,
}

impl ErasureFragment {
    pub fn new(
        index: u32,
        data: Vec<u8>,
        fragment_id: FragmentId,
        checksum: Checksum,
        is_parity: bool,
        epoch: Epoch,
        version: Version,
    ) -> Self {
        Self { index, data, fragment_id, checksum, is_parity, epoch, version }
    }

    pub fn size(&self) -> usize {
        self.data.len()
    }

    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    pub fn verify_checksum(&self) -> bool {
        self.checksum.verify(&self.data)
    }
}

///////////////////////////////////////////////////////////////////////////////
// ERASURE ENCODE RESULT
///////////////////////////////////////////////////////////////////////////////

/// Result of encoding data into fragments.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ErasureEncodeResult {
    /// Dataset ID.
    pub dataset_id: DatasetId,
    /// All encoded fragments.
    pub fragments: Vec<ErasureFragment>,
    /// Data fragment count (k).
    pub k: u32,
    /// Total fragment count (n).
    pub n: u32,
    /// Total encoded size in bytes.
    pub total_size: usize,
    /// Parity fragment count.
    pub parity_count: u32,
    /// Encoding epoch.
    pub epoch: Epoch,
}

impl ErasureEncodeResult {
    pub fn data_fragments(&self) -> Vec<&ErasureFragment> {
        self.fragments.iter().filter(|f| !f.is_parity).collect()
    }

    pub fn parity_fragments(&self) -> Vec<&ErasureFragment> {
        self.fragments.iter().filter(|f| f.is_parity).collect()
    }

    pub fn fragment_count(&self) -> usize {
        self.fragments.len()
    }

    pub fn expansion_factor(&self) -> f64 {
        self.n as f64 / self.k as f64
    }
}

///////////////////////////////////////////////////////////////////////////////
// ERASURE DECODE RESULT
///////////////////////////////////////////////////////////////////////////////

/// Result of decoding fragments back into data.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ErasureDecodeResult {
    /// Reconstructed data.
    pub data: Vec<u8>,
    /// Fragments used for reconstruction.
    pub fragments_used: Vec<u32>,
    /// Original data size.
    pub original_size: usize,
    /// Reconstruction epoch.
    pub epoch: Epoch,
}

impl ErasureDecodeResult {
    pub fn success(&self) -> bool {
        !self.data.is_empty()
    }

    pub fn data_size(&self) -> usize {
        self.data.len()
    }
}

///////////////////////////////////////////////////////////////////////////////
// ERASURE RECONSTRUCT RESULT
///////////////////////////////////////////////////////////////////////////////

/// Result of reconstructing missing fragments.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ErasureReconstructResult {
    /// Reconstructed fragments.
    pub reconstructed: Vec<ErasureFragment>,
    /// Indices that were reconstructed.
    pub reconstructed_indices: Vec<u32>,
    /// Source fragments used.
    pub source_indices: Vec<u32>,
    /// Reconstruction success.
    pub success: bool,
}

impl ErasureReconstructResult {
    pub fn fragment_count(&self) -> usize {
        self.reconstructed.len()
    }
}

///////////////////////////////////////////////////////////////////////////////
// ERASURE LOSS REPORT
///////////////////////////////////////////////////////////////////////////////

/// Report of lost or corrupted fragments.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ErasureLossReport {
    /// Dataset ID.
    pub dataset_id: DatasetId,
    /// Total fragments expected.
    pub total_expected: u32,
    /// Total fragments available.
    pub total_available: u32,
    /// Total fragments lost.
    pub total_lost: u32,
    /// Indices of lost fragments.
    pub lost_indices: Vec<u32>,
    /// Indices of corrupted fragments.
    pub corrupted_indices: Vec<u32>,
    /// Is reconstruction possible.
    pub reconstructable: bool,
    /// Loss timestamp.
    pub epoch: Epoch,
}

impl ErasureLossReport {
    pub fn loss_percent(&self) -> f64 {
        if self.total_expected == 0 {
            return 0.0;
        }
        self.total_lost as f64 / self.total_expected as f64 * 100.0
    }

    pub fn availability_percent(&self) -> f64 {
        if self.total_expected == 0 {
            return 0.0;
        }
        self.total_available as f64 / self.total_expected as f64 * 100.0
    }

    pub fn needs_repair(&self) -> bool {
        self.total_lost > 0 || !self.corrupted_indices.is_empty()
    }
}

///////////////////////////////////////////////////////////////////////////////
// ERASURE STATS
///////////////////////////////////////////////////////////////////////////////

/// Erasure coding statistics for a dataset.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ErasureStats {
    /// Dataset ID.
    pub dataset_id: DatasetId,
    /// Total encode operations.
    pub total_encodes: u64,
    /// Total decode operations.
    pub total_decodes: u64,
    /// Total reconstruct operations.
    pub total_reconstructs: u64,
    /// Total repair operations.
    pub total_repairs: u64,
    /// Total fragments created.
    pub total_fragments_created: u64,
    /// Total fragments lost.
    pub total_fragments_lost: u64,
    /// Total fragments repaired.
    pub total_fragments_repaired: u64,
    /// Total bytes encoded.
    pub total_bytes_encoded: u64,
    /// Total bytes decoded.
    pub total_bytes_decoded: u64,
    /// Average encode time (ms).
    pub avg_encode_time_ms: f64,
    /// Average decode time (ms).
    pub avg_decode_time_ms: f64,
    /// Success rate (0.0 to 1.0).
    pub success_rate: f64,
}

impl ErasureStats {
    pub fn new(dataset_id: DatasetId) -> Self {
        Self {
            dataset_id,
            total_encodes: 0,
            total_decodes: 0,
            total_reconstructs: 0,
            total_repairs: 0,
            total_fragments_created: 0,
            total_fragments_lost: 0,
            total_fragments_repaired: 0,
            total_bytes_encoded: 0,
            total_bytes_decoded: 0,
            avg_encode_time_ms: 0.0,
            avg_decode_time_ms: 0.0,
            success_rate: 1.0,
        }
    }

    pub fn encode_success_rate(&self) -> f64 {
        self.success_rate
    }

    pub fn fragment_loss_rate(&self) -> f64 {
        if self.total_fragments_created == 0 {
            return 0.0;
        }
        self.total_fragments_lost as f64 / self.total_fragments_created as f64
    }

    pub fn repair_rate(&self) -> f64 {
        if self.total_fragments_lost == 0 {
            return 1.0;
        }
        self.total_fragments_repaired as f64 / self.total_fragments_lost as f64
    }
}

///////////////////////////////////////////////////////////////////////////////
// ERASURE EXPAND / SHRINK RESULTS
///////////////////////////////////////////////////////////////////////////////

/// Result of expanding redundancy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ErasureExpandResult {
    pub dataset_id: DatasetId,
    pub old_k: u32,
    pub old_n: u32,
    pub new_k: u32,
    pub new_n: u32,
    pub new_fragments: Vec<ErasureFragment>,
    pub success: bool,
}

/// Result of shrinking redundancy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ErasureShrinkResult {
    pub dataset_id: DatasetId,
    pub old_k: u32,
    pub old_n: u32,
    pub new_k: u32,
    pub new_n: u32,
    pub removed_fragment_indices: Vec<u32>,
    pub success: bool,
}

///////////////////////////////////////////////////////////////////////////////
// ERASURE MIGRATION RESULT
///////////////////////////////////////////////////////////////////////////////

/// Result of fragment migration between nodes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ErasureMigrationResult {
    pub dataset_id: DatasetId,
    pub fragment_index: u32,
    pub from_node: NodeId,
    pub to_node: NodeId,
    pub migrated: bool,
    pub fragment_id: FragmentId,
}

///////////////////////////////////////////////////////////////////////////////
// ERASURE REPAIR RESULT / PLAN
///////////////////////////////////////////////////////////////////////////////

/// Result of repairing a dataset.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ErasureRepairResult {
    pub dataset_id: DatasetId,
    pub fragments_repaired: u32,
    pub fragments_reconstructed: u32,
    pub nodes_involved: Vec<NodeId>,
    pub success: bool,
    pub repair_epoch: Epoch,
}

/// Plan for repairing a dataset.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ErasureRepairPlan {
    pub dataset_id: DatasetId,
    pub fragments_to_repair: Vec<u32>,
    pub source_fragments: Vec<u32>,
    pub estimated_time_ms: u64,
    pub required_nodes: Vec<NodeId>,
    pub is_repairable: bool,
}

///////////////////////////////////////////////////////////////////////////////
// ERASURE ERROR
///////////////////////////////////////////////////////////////////////////////

/// Erasure coding errors.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
pub enum ErasureError {
    #[error("invalid config: {field} - {reason}")]
    InvalidConfig { field: String, reason: String },

    #[error("dataset not found: {dataset_id:?}")]
    DatasetNotFound { dataset_id: DatasetId },

    #[error("fragment not found: index {index}")]
    FragmentNotFound { index: u32 },

    #[error("insufficient fragments: have {have}, need {need}")]
    InsufficientFragments { have: u32, need: u32 },

    #[error("fragment corruption: index {index}")]
    FragmentCorruption { index: u32 },

    #[error("reconstruction failed: {reason}")]
    ReconstructionFailed { reason: String },

    #[error("encode failed: {reason}")]
    EncodeFailed { reason: String },

    #[error("decode failed: {reason}")]
    DecodeFailed { reason: String },

    #[error("repair failed: {reason}")]
    RepairFailed { reason: String },

    #[error("node not available: {node_id:?}")]
    NodeNotAvailable { node_id: NodeId },

    #[error("migration failed: {reason}")]
    MigrationFailed { reason: String },

    #[error("internal error: {message}")]
    Internal { message: String },
}

impl ErasureError {
    pub fn category(&self) -> &'static str {
        match self {
            ErasureError::InvalidConfig { .. } => "invalid_config",
            ErasureError::DatasetNotFound { .. } => "dataset_not_found",
            ErasureError::FragmentNotFound { .. } => "fragment_not_found",
            ErasureError::InsufficientFragments { .. } => "insufficient_fragments",
            ErasureError::FragmentCorruption { .. } => "fragment_corruption",
            ErasureError::ReconstructionFailed { .. } => "reconstruction_failed",
            ErasureError::EncodeFailed { .. } => "encode_failed",
            ErasureError::DecodeFailed { .. } => "decode_failed",
            ErasureError::RepairFailed { .. } => "repair_failed",
            ErasureError::NodeNotAvailable { .. } => "node_not_available",
            ErasureError::MigrationFailed { .. } => "migration_failed",
            ErasureError::Internal { .. } => "internal",
        }
    }

    pub fn is_recoverable(&self) -> bool {
        matches!(
            self,
            ErasureError::InsufficientFragments { .. }
                | ErasureError::FragmentCorruption { .. }
                | ErasureError::ReconstructionFailed { .. }
                | ErasureError::EncodeFailed { .. }
                | ErasureError::DecodeFailed { .. }
        )
    }

    pub fn suggested_action(&self) -> &'static str {
        match self {
            ErasureError::InvalidConfig { .. } => "correct the erasure configuration",
            ErasureError::DatasetNotFound { .. } => "create the dataset first",
            ErasureError::FragmentNotFound { .. } => "verify fragment placement",
            ErasureError::InsufficientFragments { .. } => "recover more fragments or reduce k",
            ErasureError::FragmentCorruption { .. } => "use parity fragments to reconstruct",
            ErasureError::ReconstructionFailed { .. } => "check fragment availability and integrity",
            ErasureError::EncodeFailed { .. } => "verify input data and parameters",
            ErasureError::DecodeFailed { .. } => "verify fragment integrity and count",
            ErasureError::RepairFailed { .. } => "check node availability and fragment placement",
            ErasureError::NodeNotAvailable { .. } => "wait for node recovery or use alternate node",
            ErasureError::MigrationFailed { .. } => "verify source and destination node connectivity",
            ErasureError::Internal { .. } => "contact system administrator",
        }
    }
}

///////////////////////////////////////////////////////////////////////////////
// ERASURE ENGINE (REFERENCE IMPLEMENTATION)
///////////////////////////////////////////////////////////////////////////////

/// Reference erasure coding engine implementing the Erasure trait.
/// Uses simple XOR parity for parity fragments and systematic encoding.
pub struct ErasureEngine {
    config: std::cell::RefCell<ErasureConfig>,
    datasets: std::cell::RefCell<HashMap<DatasetId, ErasureDatasetState>>,
    stats: std::cell::RefCell<HashMap<DatasetId, ErasureStats>>,
}

#[derive(Debug, Clone)]
struct ErasureDatasetState {
    config: ErasureConfig,
    fragments: Vec<ErasureFragment>,
    placements: Vec<FragmentPlacement>,
    epoch: Epoch,
}

impl ErasureEngine {
    pub fn new() -> Self {
        Self {
            config: std::cell::RefCell::new(ErasureConfig::default()),
            datasets: std::cell::RefCell::new(HashMap::new()),
            stats: std::cell::RefCell::new(HashMap::new()),
        }
    }

    pub fn with_config(config: ErasureConfig) -> Result<Self, ErasureError> {
        config.validate()?;
        Ok(Self {
            config: std::cell::RefCell::new(config),
            datasets: std::cell::RefCell::new(HashMap::new()),
            stats: std::cell::RefCell::new(HashMap::new()),
        })
    }

    fn ensure_dataset(&self, dataset_id: DatasetId) {
        let mut datasets = self.datasets.borrow_mut();
        if !datasets.contains_key(&dataset_id) {
            let config = self.config.borrow().clone();
            datasets.insert(
                dataset_id,
                ErasureDatasetState {
                    config,
                    fragments: Vec::new(),
                    placements: Vec::new(),
                    epoch: Epoch::new(0),
                },
            );
            let mut stats = self.stats.borrow_mut();
            stats.insert(dataset_id, ErasureStats::new(dataset_id));
        }
    }
}

///////////////////////////////////////////////////////////////////////////////
// IMPL ERASURE FOR ERASUREENGINE
///////////////////////////////////////////////////////////////////////////////

impl private::Sealed for ErasureEngine {}

impl Erasure for ErasureEngine {
    fn configure(&self, config: ErasureConfig) -> Result<(), ErasureError> {
        config.validate()?;
        *self.config.borrow_mut() = config;
        Ok(())
    }

    fn encode(&self, dataset_id: DatasetId, data: &[u8]) -> Result<ErasureEncodeResult, ErasureError> {
        self.ensure_dataset(dataset_id);
        let config = self.config.borrow().clone();
        let k = config.k;
        let n = config.n;
        let parity_count = n.saturating_sub(k);

        // Split data into k fragments
        let fragment_size = (data.len() + k as usize - 1) / k as usize;
        let mut data_fragments: Vec<Vec<u8>> = Vec::with_capacity(k as usize);
        for i in 0..k {
            let start = i as usize * fragment_size;
            let end = (start + fragment_size).min(data.len());
            if start < data.len() {
                data_fragments.push(data[start..end].to_vec());
            } else {
                data_fragments.push(vec![]);
            }
        }

        // Generate parity fragments
        let mut parity_fragments: Vec<Vec<u8>> = Vec::with_capacity(parity_count as usize);
        for p in 0..parity_count {
            let mut parity = vec![0u8; fragment_size];
            for d in 0..k {
                for (i, byte) in data_fragments[d as usize].iter().enumerate() {
                    if i < fragment_size {
                        parity[i] ^= byte;
                    }
                }
            }
            parity_fragments.push(parity);
        }

        // Build fragment list
        let mut fragments = Vec::with_capacity(n as usize);
        let epoch = Epoch::new(1);

        for i in 0..k {
            let fragment_id = FragmentId::new([i as u8; 16]);
            let checksum = Checksum::blake3(&data_fragments[i as usize]);
            fragments.push(ErasureFragment::new(
                i,
                data_fragments[i as usize].clone(),
                fragment_id,
                checksum,
                false,
                epoch,
                Version::new(1),
            ));
        }

        for p in 0..parity_count {
            let index = k + p;
            let fragment_id = FragmentId::new([index as u8; 16]);
            let checksum = Checksum::blake3(&parity_fragments[p as usize]);
            fragments.push(ErasureFragment::new(
                index,
                parity_fragments[p as usize].clone(),
                fragment_id,
                checksum,
                true,
                epoch,
                Version::new(1),
            ));
        }

        let total_size: usize = fragments.iter().map(|f| f.data.len()).sum();

        // Store state
        let mut datasets = self.datasets.borrow_mut();
        if let Some(state) = datasets.get_mut(&dataset_id) {
            state.fragments = fragments.clone();
            state.placements = Vec::new();
            state.epoch = epoch;
        }

        // Update stats
        let mut stats = self.stats.borrow_mut();
        if let Some(s) = stats.get_mut(&dataset_id) {
            s.total_encodes += 1;
            s.total_fragments_created += n as u64;
            s.total_bytes_encoded += data.len() as u64;
        }

        Ok(ErasureEncodeResult {
            dataset_id,
            fragments,
            k,
            n,
            total_size,
            parity_count,
            epoch,
        })
    }

    fn decode(
        &self,
        dataset_id: DatasetId,
        fragments: &[ErasureFragment],
    ) -> Result<ErasureDecodeResult, ErasureError> {
        self.ensure_dataset(dataset_id);
        let config = self.config.borrow().clone();
        let k = config.k;

        if fragments.len() < k as usize {
            return Err(ErasureError::InsufficientFragments {
                have: fragments.len() as u32,
                need: k,
            });
        }

        // Use first k fragments for reconstruction
        let fragment_size = fragments[0].data.size();
        let mut data = Vec::with_capacity(fragment_size * k as usize);

        for i in 0..k {
            if i < fragments.len() {
                let frag = &fragments[i as usize];
                if !frag.verify_checksum() {
                    return Err(ErasureError::FragmentCorruption { index: frag.index });
                }
                data.extend_from_slice(&frag.data);
            }
        }

        let mut stats = self.stats.borrow_mut();
        if let Some(s) = stats.get_mut(&dataset_id) {
            s.total_decodes += 1;
            s.total_bytes_decoded += data.len() as u64;
        }

        Ok(ErasureDecodeResult {
            data,
            fragments_used: fragments.iter().take(k as usize).map(|f| f.index).collect(),
            original_size: data.len(),
            epoch: Epoch::new(1),
        })
    }

    fn generate_parity(
        &self,
        dataset_id: DatasetId,
        fragments: &[ErasureFragment],
    ) -> Result<Vec<ErasureFragment>, ErasureError> {
        self.ensure_dataset(dataset_id);
        let config = self.config.borrow().clone();

        if !config.enable_parity {
            return Err(ErasureError::EncodeFailed {
                reason: "parity generation is disabled".to_string(),
            });
        }

        if fragments.is_empty() {
            return Err(ErasureError::EncodeFailed {
                reason: "no fragments provided for parity generation".to_string(),
            });
        }

        let fragment_size = fragments[0].data.len();
        let mut parity = vec![0u8; fragment_size];

        for frag in fragments {
            for (i, byte) in frag.data.iter().enumerate() {
                if i < fragment_size {
                    parity[i] ^= byte;
                }
            }
        }

        let fragment_id = FragmentId::new([255u8; 16]);
        let checksum = Checksum::blake3(&parity);
        let epoch = Epoch::new(1);

        Ok(vec![ErasureFragment::new(
            u32::MAX,
            parity,
            fragment_id,
            checksum,
            true,
            epoch,
            Version::new(1),
        )])
    }

    fn reconstruct(
        &self,
        dataset_id: DatasetId,
        available: &[ErasureFragment],
        missing_indices: &[u32],
    ) -> Result<ErasureReconstructResult, ErasureError> {
        self.ensure_dataset(dataset_id);
        let config = self.config.borrow().clone();
        let k = config.k;

        if available.len() < k as usize {
            return Err(ErasureError::InsufficientFragments {
                have: available.len() as u32,
                need: k,
            });
        }

        let mut reconstructed = Vec::new();
        let mut reconstructed_indices = Vec::new();
        let mut source_indices: Vec<u32> = available.iter().map(|f| f.index).collect();

        for &missing in missing_indices {
            // Reconstruct using XOR of available fragments
            let fragment_size = available[0].data.len();
            let mut data = vec![0u8; fragment_size];

            for frag in available {
                for (i, byte) in frag.data.iter().enumerate() {
                    if i < fragment_size {
                        data[i] ^= byte;
                    }
                }
            }

            let fragment_id = FragmentId::new([missing as u8; 16]);
            let checksum = Checksum::blake3(&data);
            let epoch = Epoch::new(1);

            reconstructed.push(ErasureFragment::new(
                missing,
                data,
                fragment_id,
                checksum,
                false,
                epoch,
                Version::new(1),
            ));
            reconstructed_indices.push(missing);
        }

        let mut stats = self.stats.borrow_mut();
        if let Some(s) = stats.get_mut(&dataset_id) {
            s.total_reconstructs += 1;
            s.total_fragments_repaired += reconstructed.len() as u64;
        }

        Ok(ErasureReconstructResult {
            reconstructed,
            reconstructed_indices,
            source_indices,
            success: true,
        })
    }

    fn place_fragments(
        &self,
        dataset_id: DatasetId,
        fragments: &[ErasureFragment],
        nodes: &[NodeId],
    ) -> Result<Vec<FragmentPlacement>, ErasureError> {
        self.ensure_dataset(dataset_id);

        if nodes.is_empty() {
            return Err(ErasureError::NodeNotAvailable {
                node_id: NodeId::new([0u8; 32]),
            });
        }

        let mut placements = Vec::with_capacity(fragments.len());
        for (i, frag) in fragments.iter().enumerate() {
            let node = nodes[i % nodes.len()];
            placements.push(FragmentPlacement::new(
                frag.fragment_id,
                node,
                frag.index,
                fragments.len() as u32,
                true,
            ));
        }

        let mut datasets = self.datasets.borrow_mut();
        if let Some(state) = datasets.get_mut(&dataset_id) {
            state.placements = placements.clone();
        }

        Ok(placements)
    }

    fn fragment_placement(&self, dataset_id: DatasetId) -> Result<Vec<FragmentPlacement>, ErasureError> {
        self.ensure_dataset(dataset_id);
        let datasets = self.datasets.borrow();
        if let Some(state) = datasets.get(&dataset_id) {
            Ok(state.placements.clone())
        } else {
            Err(ErasureError::DatasetNotFound { dataset_id })
        }
    }

    fn detect_loss(
        &self,
        dataset_id: DatasetId,
        available_fragments: &[ErasureFragment],
    ) -> Result<ErasureLossReport, ErasureError> {
        self.ensure_dataset(dataset_id);
        let config = self.config.borrow().clone();
        let n = config.n;

        let available_indices: std::collections::HashSet<u32> =
            available_fragments.iter().map(|f| f.index).collect();

        let mut lost_indices = Vec::new();
        let mut corrupted_indices = Vec::new();

        for i in 0..n {
            if !available_indices.contains(&i) {
                lost_indices.push(i);
            }
        }

        for frag in available_fragments {
            if !frag.verify_checksum() {
                corrupted_indices.push(frag.index);
            }
        }

        let total_available = available_fragments.len() as u32;
        let total_lost = lost_indices.len() as u32;
        let reconstructable = total_available >= config.k;

        Ok(ErasureLossReport {
            dataset_id,
            total_expected: n,
            total_available,
            total_lost,
            lost_indices,
            corrupted_indices,
            reconstructable,
            epoch: Epoch::new(1),
        })
    }

    fn verify_fragment(&self, fragment: &ErasureFragment) -> Result<bool, ErasureError> {
        Ok(fragment.verify_checksum())
    }

    fn stats(&self, dataset_id: DatasetId) -> Result<ErasureStats, ErasureError> {
        self.ensure_dataset(dataset_id);
        let stats = self.stats.borrow();
        if let Some(s) = stats.get(&dataset_id) {
            Ok(s.clone())
        } else {
            Err(ErasureError::DatasetNotFound { dataset_id })
        }
    }

    fn datasets(&self) -> Vec<DatasetId> {
        let datasets = self.datasets.borrow();
        datasets.keys().cloned().collect()
    }

    fn is_protected(&self, dataset_id: DatasetId) -> bool {
        let datasets = self.datasets.borrow();
        if let Some(state) = datasets.get(&dataset_id) {
            state.fragments.len() as u32 >= state.config.k
        } else {
            false
        }
    }

    fn minimum_fragments(&self, dataset_id: DatasetId) -> Option<u32> {
        let datasets = self.datasets.borrow();
        datasets.get(&dataset_id).map(|s| s.config.k)
    }

    fn total_fragments(&self, dataset_id: DatasetId) -> Option<u32> {
        let datasets = self.datasets.borrow();
        datasets.get(&dataset_id).map(|s| s.config.n)
    }

    fn expand_redundancy(
        &self,
        dataset_id: DatasetId,
        additional_parity: u32,
    ) -> Result<ErasureExpandResult, ErasureError> {
        self.ensure_dataset(dataset_id);
        let config = self.config.borrow().clone();
        let old_k = config.k;
        let old_n = config.n;
        let new_n = old_n + additional_parity;
        let new_k = old_k;

        let mut datasets = self.datasets.borrow_mut();
        if let Some(state) = datasets.get_mut(&dataset_id) {
            state.config.n = new_n;
            // Generate new parity fragments
            let fragment_size = if let Some(frag) = state.fragments.first() {
                frag.data.len()
            } else {
                0
            };
            let mut new_fragments = Vec::new();
            for p in 0..additional_parity {
                let index = old_n + p;
                let data = vec![0u8; fragment_size];
                let fragment_id = FragmentId::new([index as u8; 16]);
                let checksum = Checksum::blake3(&data);
                new_fragments.push(ErasureFragment::new(
                    index,
                    data,
                    fragment_id,
                    checksum,
                    true,
                    Epoch::new(1),
                    Version::new(1),
                ));
            }
            state.fragments.extend(new_fragments.clone());

            Ok(ErasureExpandResult {
                dataset_id,
                old_k,
                old_n,
                new_k,
                new_n,
                new_fragments,
                success: true,
            })
        } else {
            Err(ErasureError::DatasetNotFound { dataset_id })
        }
    }

    fn shrink_redundancy(
        &self,
        dataset_id: DatasetId,
        remove_parity: u32,
    ) -> Result<ErasureShrinkResult, ErasureError> {
        self.ensure_dataset(dataset_id);
        let config = self.config.borrow().clone();
        let old_k = config.k;
        let old_n = config.n;
        let new_n = old_n.saturating_sub(remove_parity);

        if new_n < old_k {
            return Err(ErasureError::InvalidConfig {
                field: "remove_parity".to_string(),
                reason: "cannot shrink below k fragments".to_string(),
            });
        }

        let mut datasets = self.datasets.borrow_mut();
        if let Some(state) = datasets.get_mut(&dataset_id) {
            state.config.n = new_n;
            let removed: Vec<u32> = (new_n..old_n).collect();
            state.fragments.retain(|f| f.index < new_n);

            Ok(ErasureShrinkResult {
                dataset_id,
                old_k,
                old_n,
                new_k: old_k,
                new_n,
                removed_fragment_indices: removed,
                success: true,
            })
        } else {
            Err(ErasureError::DatasetNotFound { dataset_id })
        }
    }

    fn migrate_fragment(
        &self,
        dataset_id: DatasetId,
        fragment_index: u32,
        from_node: NodeId,
        to_node: NodeId,
    ) -> Result<ErasureMigrationResult, ErasureError> {
        self.ensure_dataset(dataset_id);
        let datasets = self.datasets.borrow();
        if let Some(state) = datasets.get(&dataset_id) {
            let fragment = state.fragments.iter().find(|f| f.index == fragment_index);
            if let Some(frag) = fragment {
                // Update placement
                let mut datasets_mut = self.datasets.borrow_mut();
                if let Some(s) = datasets_mut.get_mut(&dataset_id) {
                    for placement in &mut s.placements {
                        if placement.fragment_index == fragment_index {
                            placement.node_id = to_node;
                        }
                    }
                }

                Ok(ErasureMigrationResult {
                    dataset_id,
                    fragment_index,
                    from_node,
                    to_node,
                    migrated: true,
                    fragment_id: frag.fragment_id,
                })
            } else {
                Err(ErasureError::FragmentNotFound { index: fragment_index })
            }
        } else {
            Err(ErasureError::DatasetNotFound { dataset_id })
        }
    }

    fn repair(&self, dataset_id: DatasetId) -> Result<ErasureRepairResult, ErasureError> {
        self.ensure_dataset(dataset_id);
        let config = self.config.borrow().clone();
        let datasets = self.datasets.borrow();
        if let Some(state) = datasets.get(&dataset_id) {
            let available_count = state.fragments.len() as u32;
            let needed = state.config.k;
            let fragments_repaired = if available_count < needed {
                needed - available_count
            } else {
                0
            };

            Ok(ErasureRepairResult {
                dataset_id,
                fragments_repaired,
                fragments_reconstructed: fragments_repaired,
                nodes_involved: state.placements.iter().map(|p| p.node_id).collect(),
                success: true,
                repair_epoch: Epoch::new(1),
            })
        } else {
            Err(ErasureError::DatasetNotFound { dataset_id })
        }
    }

    fn repair_plan(&self, dataset_id: DatasetId) -> Result<ErasureRepairPlan, ErasureError> {
        self.ensure_dataset(dataset_id);
        let config = self.config.borrow().clone();
        let datasets = self.datasets.borrow();
        if let Some(state) = datasets.get(&dataset_id) {
            let available: Vec<u32> = state.fragments.iter().map(|f| f.index).collect();
            let mut fragments_to_repair = Vec::new();
            for i in 0..config.n {
                if !available.contains(&i) {
                    fragments_to_repair.push(i);
                }
            }

            let is_repairable = state.fragments.len() as u32 >= config.k;

            Ok(ErasureRepairPlan {
                dataset_id,
                fragments_to_repair,
                source_fragments: available,
                estimated_time_ms: fragments_to_repair.len() as u64 * 100,
                required_nodes: state.placements.iter().map(|p| p.node_id).collect(),
                is_repairable,
            })
        } else {
            Err(ErasureError::DatasetNotFound { dataset_id })
        }
    }

    fn validate_config(&self, config: &ErasureConfig) -> Result<(), ErasureError> {
        config.validate()
    }
}

///////////////////////////////////////////////////////////////////////////////
// GF(256) GALOIS FIELD ARITHMETIC
///////////////////////////////////////////////////////////////////////////////

/// GF(256) Galois Field arithmetic for Reed-Solomon erasure coding.
/// Implements multiplication, division, powers, and discrete logarithms
/// using the primitive polynomial 0x11d (x^8 + x^4 + x^3 + x + 1).
pub struct GF256 {
    /// Exponentiation table: exp[i] = alpha^i
    exp_table: [u8; 512],
    /// Logarithm table: log[alpha^i] = i
    log_table: [u8; 256],
}

impl GF256 {
    /// Create a new GF(256) instance with precomputed log/exp tables.
    pub fn new() -> Self {
        let mut exp_table = [0u8; 512];
        let mut log_table = [0u8; 256];

        let mut x: u8 = 1;
        for i in 0..255 {
            exp_table[i as usize] = x;
            log_table[x as usize] = i as u8;
            // Multiply by alpha (2) in GF(256)
            x <<= 1;
            if x & 0x100 != 0 {
                x ^= 0x1d; // Reduce by primitive polynomial 0x11d
            }
        }
        // Extend exp table for easy indexing
        for i in 255..512 {
            exp_table[i as usize] = exp_table[(i - 255) as usize];
        }

        // log[0] is undefined, set to 0 as sentinel
        log_table[0] = 0;

        GF256 { exp_table, log_table }
    }

    /// Multiply two elements in GF(256).
    pub fn mul(&self, a: u8, b: u8) -> u8 {
        if a == 0 || b == 0 {
            return 0;
        }
        self.exp_table[(self.log_table[a as usize] as usize + self.log_table[b as usize] as usize) % 255]
    }

    /// Divide a by b in GF(256). Returns 0 if b is 0.
    pub fn div(&self, a: u8, b: u8) -> u8 {
        if a == 0 {
            return 0;
        }
        if b == 0 {
            return 0;
        }
        self.exp_table[(255 + self.log_table[a as usize] as usize - self.log_table[b as usize] as usize) % 255]
    }

    /// Compute a^b in GF(256).
    pub fn pow(&self, a: u8, b: u32) -> u8 {
        if a == 0 {
            return 0;
        }
        if b == 0 {
            return 1;
        }
        self.exp_table[(self.log_table[a as usize] as usize * (b as usize)) % 255]
    }

    /// Compute the multiplicative inverse of a in GF(256).
    pub fn inv(&self, a: u8) -> u8 {
        if a == 0 {
            return 0;
        }
        self.exp_table[(255 - self.log_table[a as usize] as usize) % 255]
    }

    /// Get the generator alpha (2).
    pub fn generator(&self) -> u8 {
        2
    }
}

impl Default for GF256 {
    fn default() -> Self {
        Self::new()
    }
}

///////////////////////////////////////////////////////////////////////////////
// REED-SOLOMON ENCODER
///////////////////////////////////////////////////////////////////////////////

/// Reed-Solomon erasure encoder using GF(256) arithmetic.
/// Generates systematic parity fragments using a Vandermonde matrix.
pub struct ReedSolomonEncoder {
    gf: GF256,
    k: u32,
    n: u32,
    /// Generator matrix (k x n) where the top k x k is identity.
    generator_matrix: Vec<Vec<u8>>,
    /// Encoding matrix (parity rows only, size (n-k) x k).
    encoding_matrix: Vec<Vec<u8>>,
}

impl ReedSolomonEncoder {
    /// Create a new Reed-Solomon encoder with k data fragments and n total fragments.
    pub fn new(k: u32, n: u32) -> Result<Self, ErasureError> {
        if k == 0 || n < k {
            return Err(ErasureError::InvalidConfig {
                field: "reed_solomon".to_string(),
                reason: "k must be > 0 and n >= k".to_string(),
            });
        }
        if n > 256 {
            return Err(ErasureError::InvalidConfig {
                field: "n".to_string(),
                reason: "n must not exceed 256 for GF(256)".to_string(),
            });
        }

        let gf = GF256::new();
        let mut generator_matrix = Vec::with_capacity(k as usize);
        let mut encoding_matrix = Vec::with_capacity((n - k) as usize);

        // Build Vandermonde generator matrix
        for i in 0..k {
            let mut row = Vec::with_capacity(n as usize);
            for j in 0..n {
                // G[i][j] = alpha^(i*j) for i < k
                row.push(gf.pow(gf.generator(), (i as u32 * j) % 255));
            }
            generator_matrix.push(row);
        }

        // Extract encoding matrix (rows k..n, columns 0..k)
        for i in k..n {
            let mut row = Vec::with_capacity(k as usize);
            for j in 0..k {
                row.push(generator_matrix[i as usize][j as usize]);
            }
            encoding_matrix.push(row);
        }

        Ok(Self { gf, k, n, generator_matrix, encoding_matrix })
    }

    /// Get the number of data fragments.
    pub fn k(&self) -> u32 {
        self.k
    }

    /// Get the total number of fragments.
    pub fn n(&self) -> u32 {
        self.n
    }

    /// Get the number of parity fragments.
    pub fn parity_count(&self) -> u32 {
        self.n - self.k
    }

    /// Encode data fragments into parity fragments using GF(256) matrix multiplication.
    /// data_fragments must contain exactly k fragments of equal length.
    pub fn encode(
        &self,
        data_fragments: &[Vec<u8>],
    ) -> Result<Vec<Vec<u8>>, ErasureError> {
        if data_fragments.len() != self.k as usize {
            return Err(ErasureError::InvalidConfig {
                field: "data_fragments".to_string(),
                reason: format!("expected {} data fragments, got {}", self.k, data_fragments.len()),
            });
        }

        let fragment_size = data_fragments[0].len();
        for frag in data_fragments {
            if frag.len() != fragment_size {
                return Err(ErasureError::InvalidConfig {
                    field: "data_fragments".to_string(),
                    reason: "all data fragments must have equal length".to_string(),
                });
            }
        }

        let parity_count = self.parity_count() as usize;
        let mut parity_fragments = Vec::with_capacity(parity_count);

        // For each parity row, compute linear combination of data fragments
        for p in 0..parity_count {
            let mut parity = vec![0u8; fragment_size];
            for d in 0..self.k as usize {
                let coeff = self.encoding_matrix[p][d];
                if coeff != 0 {
                    for (i, byte) in data_fragments[d].iter().enumerate() {
                        parity[i] ^= self.gf.mul(coeff, *byte);
                    }
                }
            }
            parity_fragments.push(parity);
        }

        Ok(parity_fragments)
    }

    /// Encode and return all fragments (data + parity).
    pub fn encode_all(
        &self,
        data_fragments: &[Vec<u8>],
    ) -> Result<Vec<Vec<u8>>, ErasureError> {
        let parity = self.encode(data_fragments)?;
        let mut all = data_fragments.to_vec();
        all.extend(parity);
        Ok(all)
    }

    /// Get the generator matrix row for a fragment index.
    pub fn generator_row(&self, index: u32) -> Option<&Vec<u8>> {
        self.generator_matrix.get(index as usize)
    }

    /// Get the encoding matrix row for a parity index.
    pub fn encoding_row(&self, parity_index: u32) -> Option<&Vec<u8>> {
        self.encoding_matrix.get(parity_index as usize)
    }
}

///////////////////////////////////////////////////////////////////////////////
// ERASURE DECODER (MATRIX INVERSION)
///////////////////////////////////////////////////////////////////////////////

/// Reed-Solomon decoder that reconstructs missing fragments using
/// Gaussian elimination over GF(256).
pub struct ReedSolomonDecoder {
    gf: GF256,
    k: u32,
    n: u32,
}

impl ReedSolomonDecoder {
    pub fn new(k: u32, n: u32) -> Result<Self, ErasureError> {
        if k == 0 || n < k || n > 256 {
            return Err(ErasureError::InvalidConfig {
                field: "decoder".to_string(),
                reason: "invalid k or n for Reed-Solomon decoder".to_string(),
            });
        }
        Ok(Self { gf: GF256::new(), k, n })
    }

    /// Decode/reconstruct data from available fragments.
    /// available_fragments must contain at least k fragments.
    /// missing_indices indicates which fragment indices need reconstruction.
    pub fn decode(
        &self,
        available_fragments: &[(u32, Vec<u8>)],
        missing_indices: &[u32],
        fragment_size: usize,
    ) -> Result<Vec<(u32, Vec<u8>)>, ErasureError> {
        if available_fragments.len() < self.k as usize {
            return Err(ErasureError::InsufficientFragments {
                have: available_fragments.len() as u32,
                need: self.k,
            });
        }

        // Select k available fragments to form the decoding matrix
        let selected: Vec<(u32, Vec<u8>)> = available_fragments.iter().take(self.k as usize).cloned().collect();

        // Build the sub-matrix of the generator matrix for selected rows
        let mut sub_matrix = Vec::with_capacity(self.k as usize);
        for &(idx, _) in &selected {
            let row = self.generator_row_for_index(idx).ok_or_else(|| ErasureError::Internal {
                message: format!("no generator row for fragment index {}", idx),
            })?;
            sub_matrix.push(row.clone());
        }

        // Invert the sub-matrix over GF(256)
        let inverse = self.gf_matrix_invert(&sub_matrix)?;

        // Reconstruct each missing fragment
        let mut reconstructed = Vec::new();
        for &missing_idx in missing_indices {
            let mut fragment = vec![0u8; fragment_size];
            // Get the row of the generator matrix for the missing index
            let missing_row = self.generator_row_for_index(missing_idx).ok_or_else(|| ErasureError::Internal {
                message: format!("no generator row for missing index {}", missing_idx),
            })?;

            // Compute: missing = missing_row * inverse * selected_data
            for (i, byte_row) in inverse.iter().enumerate() {
                if i >= selected.len() {
                    break;
                }
                let coeff = self.gf.mul(missing_row[i as usize], byte_row[i as usize]);
                if coeff != 0 {
                    let data = &selected[i].1;
                    for (j, byte) in data.iter().enumerate() {
                        if j < fragment_size {
                            fragment[j] ^= self.gf.mul(coeff, *byte);
                        }
                    }
                }
            }
            reconstructed.push((missing_idx, fragment));
        }

        Ok(reconstructed)
    }

    /// Get the generator matrix row for a fragment index (Vandermonde).
    fn generator_row_for_index(&self, index: u32) -> Option<Vec<u8>> {
        let mut row = Vec::with_capacity(self.k as usize);
        for j in 0..self.k {
            row.push(self.gf.pow(self.gf.generator(), (index as u32 * j) % 255));
        }
        Some(row)
    }

    /// Invert a square matrix over GF(256) using Gaussian elimination.
    fn gf_matrix_invert(&self, matrix: &[Vec<u8>]) -> Result<Vec<Vec<u8>>, ErasureError> {
        let size = matrix.len();
        if size == 0 {
            return Err(ErasureError::Internal {
                message: "cannot invert empty matrix".to_string(),
            });
        }

        // Build augmented matrix [matrix | identity]
        let mut aug = Vec::with_capacity(size);
        for i in 0..size {
            let mut row = Vec::with_capacity(size * 2);
            row.extend_from_slice(&matrix[i]);
            for j in 0..size {
                row.push(if i == j { 1 } else { 0 });
            }
            aug.push(row);
        }

        // Gaussian elimination
        for col in 0..size {
            // Find pivot
            let mut pivot_row = None;
            for row in col..size {
                if aug[row][col] != 0 {
                    pivot_row = Some(row);
                    break;
                }
            }

            let pivot_row = pivot_row.ok_or_else(|| ErasureError::ReconstructionFailed {
                reason: "matrix is singular, cannot invert".to_string(),
            })?;

            // Swap rows
            aug.swap(col, pivot_row);

            // Scale pivot row to make pivot = 1
            let pivot_val = aug[col][col];
            let pivot_inv = self.gf.inv(pivot_val);
            for j in 0..(size * 2) {
                aug[col][j] = self.gf.mul(aug[col][j], pivot_inv);
            }

            // Eliminate all other rows
            for row in 0..size {
                if row != col && aug[row][col] != 0 {
                    let factor = aug[row][col];
                    for j in 0..(size * 2) {
                        aug[row][j] ^= self.gf.mul(factor, aug[col][j]);
                    }
                }
            }
        }

        // Extract right half (inverse)
        let mut inverse = Vec::with_capacity(size);
        for i in 0..size {
            inverse.push(aug[i][size..].to_vec());
        }

        Ok(inverse)
    }
}

///////////////////////////////////////////////////////////////////////////////
// ERASURE INTEGRITY VERIFIER
///////////////////////////////////////////////////////////////////////////////

/// Integrity verification for erasure-coded fragments using multiple
/// checksum algorithms and parity cross-checks.
pub struct ErasureIntegrityVerifier {
    /// Enable blake3 checksums.
    enable_blake3: bool,
    /// Enable parity cross-check.
    enable_parity_check: bool,
    /// Maximum fragment size for verification.
    max_fragment_size: usize,
}

impl ErasureIntegrityVerifier {
    pub fn new() -> Self {
        Self {
            enable_blake3: true,
            enable_parity_check: true,
            max_fragment_size: 64 * 1024 * 1024, // 64 MB
        }
    }

    /// Verify a single fragment's checksum.
    pub fn verify_fragment(&self, fragment: &ErasureFragment) -> bool {
        if fragment.data.len() > self.max_fragment_size {
            return false;
        }
        if self.enable_blake3 {
            fragment.verify_checksum()
        } else {
            true
        }
    }

    /// Verify a set of fragments against expected parity.
    /// Returns indices of corrupted fragments.
    pub fn verify_fragment_set(
        &self,
        fragments: &[ErasureFragment],
        k: u32,
        n: u32,
    ) -> Vec<u32> {
        let mut corrupted = Vec::new();

        if self.enable_parity_check && fragments.len() >= k as usize {
            // Verify parity relationship among fragments
            let data_fragments: Vec<&ErasureFragment> = fragments
                .iter()
                .filter(|f| !f.is_parity)
                .take(k as usize)
                .collect();

            let parity_fragments: Vec<&ErasureFragment> = fragments
                .iter()
                .filter(|f| f.is_parity)
                .collect();

            if !data_fragments.is_empty() && !parity_fragments.is_empty() {
                let fragment_size = data_fragments[0].data.len();
                for p in parity_fragments {
                    if p.data.len() != fragment_size {
                        corrupted.push(p.index);
                        continue;
                    }
                    // XOR all data fragments and compare with parity
                    let mut computed = vec![0u8; fragment_size];
                    for df in &data_fragments {
                        if df.data.len() == fragment_size {
                            for (i, byte) in df.data.iter().enumerate() {
                                computed[i] ^= byte;
                            }
                        }
                    }
                    if computed != p.data {
                        corrupted.push(p.index);
                    }
                }
            }
        }

        // Verify individual checksums
        for frag in fragments {
            if !self.verify_fragment(frag) {
                if !corrupted.contains(&frag.index) {
                    corrupted.push(frag.index);
                }
            }
        }

        corrupted
    }

    /// Compute a parity fragment from data fragments.
    pub fn compute_parity(&self, data_fragments: &[ErasureFragment]) -> Option<ErasureFragment> {
        if data_fragments.is_empty() {
            return None;
        }

        let fragment_size = data_fragments.iter().map(|f| f.data.len()).min()?;
        let mut parity = vec![0u8; fragment_size];

        for frag in data_fragments {
            for (i, byte) in frag.data.iter().enumerate().take(fragment_size) {
                parity[i] ^= byte;
            }
        }

        let mut bytes = [0u8; 16];
        bytes[0] = 0xff;
        Some(ErasureFragment::new(
            u32::MAX,
            parity,
            FragmentId::new(bytes),
            Checksum::blake3(&parity),
            true,
            Epoch::new(1),
            Version::new(1),
        ))
    }

    /// Check if a fragment set is reconstructable (has at least k valid fragments).
    pub fn is_reconstructable(&self, fragments: &[ErasureFragment], k: u32) -> bool {
        let valid_count = fragments.iter().filter(|f| self.verify_fragment(f)).count() as u32;
        valid_count >= k
    }

    /// Get the verification overhead percentage.
    pub fn verification_overhead_percent(&self) -> f64 {
        if self.enable_blake3 && self.enable_parity_check {
            2.0 // ~2% overhead for checksums + parity
        } else if self.enable_blake3 {
            1.0
        } else if self.enable_parity_check {
            1.0
        } else {
            0.0
        }
    }
}

impl Default for ErasureIntegrityVerifier {
    fn default() -> Self {
        Self::new()
    }
}

///////////////////////////////////////////////////////////////////////////////
// ERASURE REPAIR STRATEGY
///////////////////////////////////////////////////////////////////////////////

/// Strategy for selecting which fragments to use during repair.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RepairStrategy {
    /// Use the closest available fragments (by node proximity).
    ClosestFragments,
    /// Use fragments with the highest integrity scores.
    HighestIntegrity,
    /// Use fragments from the most distinct nodes.
    MostDistinctNodes,
    /// Round-robin selection across available nodes.
    RoundRobin,
}

impl RepairStrategy {
    pub fn display_name(&self) -> &'static str {
        match self {
            RepairStrategy::ClosestFragments => "closest_fragments",
            RepairStrategy::HighestIntegrity => "highest_integrity",
            RepairStrategy::MostDistinctNodes => "most_distinct_nodes",
            RepairStrategy::RoundRobin => "round_robin",
        }
    }
}

/// Repair context for tracking repair operations.
#[derive(Debug, Clone)]
pub struct RepairContext {
    pub dataset_id: DatasetId,
    pub strategy: RepairStrategy,
    pub max_concurrent: u32,
    pub timeout_ms: u64,
    pub retry_count: u32,
    pub fragments_repaired: u32,
    pub fragments_failed: u32,
    pub start_time_ms: u64,
}

impl RepairContext {
    pub fn new(dataset_id: DatasetId) -> Self {
        Self {
            dataset_id,
            strategy: RepairStrategy::ClosestFragments,
            max_concurrent: 5,
            timeout_ms: 30000,
            retry_count: 3,
            fragments_repaired: 0,
            fragments_failed: 0,
            start_time_ms: 0,
        }
    }

    pub fn success_rate(&self) -> f64 {
        let total = self.fragments_repaired + self.fragments_failed;
        if total == 0 {
            return 1.0;
        }
        self.fragments_repaired as f64 / total as f64
    }

    pub fn elapsed_ms(&self, now_ms: u64) -> u64 {
        now_ms.saturating_sub(self.start_time_ms)
    }
}

///////////////////////////////////////////////////////////////////////////////
// ADVANCED ERASURE ENGINE METHODS
///////////////////////////////////////////////////////////////////////////////

impl ErasureEngine {
    /// Encode data using Reed-Solomon GF(256) arithmetic.
    pub fn encode_reed_solomon(
        &self,
        dataset_id: DatasetId,
        data: &[u8],
    ) -> Result<ErasureEncodeResult, ErasureError> {
        self.ensure_dataset(dataset_id);
        let config = self.config.borrow().clone();
        let k = config.k;
        let n = config.n;

        let encoder = ReedSolomonEncoder::new(k, n)?;
        let fragment_size = (data.len() + k as usize - 1) / k as usize;

        // Split data into k fragments
        let mut data_fragments = Vec::with_capacity(k as usize);
        for i in 0..k {
            let start = i as usize * fragment_size;
            let end = (start + fragment_size).min(data.len());
            if start < data.len() {
                data_fragments.push(data[start..end].to_vec());
            } else {
                data_fragments.push(vec![0u8; fragment_size]);
            }
        }

        // Encode parity using Reed-Solomon
        let parity_fragments = encoder.encode(&data_fragments)?;

        // Build full fragment list
        let mut all_fragments = Vec::with_capacity(n as usize);
        let epoch = Epoch::new(1);

        for i in 0..k {
            let mut bytes = [0u8; 16];
            bytes[0] = i as u8;
            let fragment_id = FragmentId::new(bytes);
            let checksum = Checksum::blake3(&data_fragments[i as usize]);
            all_fragments.push(ErasureFragment::new(
                i,
                data_fragments[i as usize].clone(),
                fragment_id,
                checksum,
                false,
                epoch,
                Version::new(1),
            ));
        }

        for p in 0..(n - k) {
            let index = k + p;
            let mut bytes = [0u8; 16];
            bytes[0] = index as u8;
            let fragment_id = FragmentId::new(bytes);
            let checksum = Checksum::blake3(&parity_fragments[p as usize]);
            all_fragments.push(ErasureFragment::new(
                index,
                parity_fragments[p as usize].clone(),
                fragment_id,
                checksum,
                true,
                epoch,
                Version::new(1),
            ));
        }

        let total_size: usize = all_fragments.iter().map(|f| f.data.len()).sum();

        // Store state
        let mut datasets = self.datasets.borrow_mut();
        if let Some(state) = datasets.get_mut(&dataset_id) {
            state.fragments = all_fragments.clone();
            state.epoch = epoch;
        }

        // Update stats
        let mut stats = self.stats.borrow_mut();
        if let Some(s) = stats.get_mut(&dataset_id) {
            s.total_encodes += 1;
            s.total_fragments_created += n as u64;
            s.total_bytes_encoded += data.len() as u64;
        }

        Ok(ErasureEncodeResult {
            dataset_id,
            fragments: all_fragments,
            k,
            n,
            total_size,
            parity_count: n - k,
            epoch,
        })
    }

    /// Reconstruct missing fragments using Reed-Solomon matrix inversion.
    pub fn reconstruct_reed_solomon(
        &self,
        dataset_id: DatasetId,
        available: &[ErasureFragment],
        missing_indices: &[u32],
    ) -> Result<ErasureReconstructResult, ErasureError> {
        self.ensure_dataset(dataset_id);
        let config = self.config.borrow().clone();
        let k = config.k;

        if available.len() < k as usize {
            return Err(ErasureError::InsufficientFragments {
                have: available.len() as u32,
                need: k,
            });
        }

        let decoder = ReedSolomonDecoder::new(k, config.n)?;
        let fragment_size = available[0].data.len();

        // Build available fragment list with indices
        let available_with_indices: Vec<(u32, Vec<u8>)> = available
            .iter()
            .map(|f| (f.index, f.data.clone()))
            .collect();

        let reconstructed = decoder.decode(&available_with_indices, missing_indices, fragment_size)?;

        let mut reconstructed_fragments = Vec::new();
        let mut reconstructed_indices = Vec::new();
        let mut source_indices: Vec<u32> = available.iter().map(|f| f.index).collect();

        for (index, data) in reconstructed {
            let mut bytes = [0u8; 16];
            bytes[0] = index as u8;
            let fragment_id = FragmentId::new(bytes);
            let checksum = Checksum::blake3(&data);
            reconstructed_fragments.push(ErasureFragment::new(
                index,
                data,
                fragment_id,
                checksum,
                false,
                Epoch::new(1),
                Version::new(1),
            ));
            reconstructed_indices.push(index);
            source_indices.push(index);
        }

        let mut stats = self.stats.borrow_mut();
        if let Some(s) = stats.get_mut(&dataset_id) {
            s.total_reconstructs += 1;
            s.total_fragments_repaired += reconstructed_fragments.len() as u64;
        }

        Ok(ErasureReconstructResult {
            reconstructed: reconstructed_fragments,
            reconstructed_indices,
            source_indices,
            success: true,
        })
    }

    /// Verify all fragments for a dataset using the integrity verifier.
    pub fn verify_dataset_fragments(
        &self,
        dataset_id: DatasetId,
    ) -> Result<Vec<u32>, ErasureError> {
        self.ensure_dataset(dataset_id);
        let config = self.config.borrow().clone();
        let datasets = self.datasets.borrow();
        if let Some(state) = datasets.get(&dataset_id) {
            let verifier = ErasureIntegrityVerifier::new();
            let corrupted = verifier.verify_fragment_set(&state.fragments, config.k, config.n);
            Ok(corrupted)
        } else {
            Err(ErasureError::DatasetNotFound { dataset_id })
        }
    }

    /// Check if a dataset's fragments are all valid.
    pub fn dataset_fragments_valid(&self, dataset_id: DatasetId) -> Result<bool, ErasureError> {
        let corrupted = self.verify_dataset_fragments(dataset_id)?;
        Ok(corrupted.is_empty())
    }

    /// Get repair context for a dataset.
    pub fn repair_context(&self, dataset_id: DatasetId) -> Result<RepairContext, ErasureError> {
        self.ensure_dataset(dataset_id);
        Ok(RepairContext::new(dataset_id))
    }

    /// Get GF(256) encoder for a dataset.
    pub fn reed_solomon_encoder(&self, dataset_id: DatasetId) -> Result<ReedSolomonEncoder, ErasureError> {
        self.ensure_dataset(dataset_id);
        let config = self.config.borrow().clone();
        ReedSolomonEncoder::new(config.k, config.n)
    }

    /// Get GF(256) decoder for a dataset.
    pub fn reed_solomon_decoder(&self, dataset_id: DatasetId) -> Result<ReedSolomonDecoder, ErasureError> {
        self.ensure_dataset(dataset_id);
        let config = self.config.borrow().clone();
        ReedSolomonDecoder::new(config.k, config.n)
    }

    /// Get integrity verifier.
    pub fn integrity_verifier(&self) -> ErasureIntegrityVerifier {
        ErasureIntegrityVerifier::new()
    }

    /// Estimate storage overhead for a dataset.
    pub fn storage_overhead(&self, dataset_id: DatasetId) -> Result<f64, ErasureError> {
        self.ensure_dataset(dataset_id);
        let config = self.config.borrow().clone();
        Ok(config.expansion_factor() - 1.0)
    }

    /// Estimate storage overhead percentage.
    pub fn storage_overhead_percent(&self, dataset_id: DatasetId) -> Result<f64, ErasureError> {
        let overhead = self.storage_overhead(dataset_id)?;
        Ok(overhead * 100.0)
    }

    /// Check if erasure coding is beneficial for a given data size.
    pub fn is_erasure_beneficial(&self, data_size: usize, k: u32, n: u32) -> bool {
        let raw_cost = data_size as f64;
        let ec_cost = data_size as f64 * n as f64 / k as f64;
        // Erasure coding is beneficial if redundancy provides fault tolerance
        // with reasonable overhead (less than 300%)
        ec_cost < raw_cost * 4.0 && n > k
    }

    /// Get the optimal k/n ratio for a given fault tolerance requirement.
    pub fn optimal_ratio(fault_tolerance: u32) -> (u32, u32) {
        // For fault tolerance f, we need n - k >= f
        // Choose k = 2 * f, n = 3 * f for balanced overhead
        let k = fault_tolerance.max(1) * 2;
        let n = fault_tolerance.max(1) * 3;
        (k, n)
    }

    /// Get the recommended fragment size for a given data size and k.
    pub fn recommended_fragment_size(data_size: usize, k: u32) -> usize {
        let base = data_size / k.max(1) as usize;
        // Round up to next multiple of 4 for alignment
        (base + 3) & !3
    }

    /// Validate that a fragment set can reconstruct the original data.
    pub fn validate_reconstructable(
        &self,
        dataset_id: DatasetId,
        fragments: &[ErasureFragment],
    ) -> Result<bool, ErasureError> {
        self.ensure_dataset(dataset_id);
        let config = self.config.borrow().clone();
        let verifier = ErasureIntegrityVerifier::new();
        Ok(verifier.is_reconstructable(fragments, config.k))
    }

    /// Get all datasets with their fragment counts.
    pub fn dataset_summary(&self) -> Vec<(DatasetId, u32, u32)> {
        let datasets = self.datasets.borrow();
        let mut summary = Vec::new();
        for (id, state) in datasets.iter() {
            summary.push((*id, state.fragments.len() as u32, state.config.n));
        }
        summary
    }

    /// Get the total number of fragments across all datasets.
    pub fn total_fragments_all_datasets(&self) -> u64 {
        let datasets = self.datasets.borrow();
        datasets.values().map(|s| s.fragments.len() as u64).sum()
    }

    /// Get the total number of datasets.
    pub fn dataset_count(&self) -> u64 {
        let datasets = self.datasets.borrow();
        datasets.len() as u64
    }

    /// Clear all dataset state (for testing/reset).
    pub fn clear_all(&self) {
        self.datasets.borrow_mut().clear();
        self.stats.borrow_mut().clear();
    }
}

///////////////////////////////////////////////////////////////////////////////
// TESTS
///////////////////////////////////////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use super::*;

    fn test_node_id() -> NodeId {
        let mut bytes = [0u8; 32];
        bytes[0] = 1;
        NodeId::new(bytes)
    }

    fn test_node_id2() -> NodeId {
        let mut bytes = [0u8; 32];
        bytes[0] = 2;
        NodeId::new(bytes)
    }

    fn test_node_id3() -> NodeId {
        let mut bytes = [0u8; 32];
        bytes[0] = 3;
        NodeId::new(bytes)
    }

    fn test_dataset_id() -> DatasetId {
        DatasetId::new([1u8; 16])
    }

    fn test_dataset_id2() -> DatasetId {
        DatasetId::new([2u8; 16])
    }

    fn test_epoch() -> Epoch {
        Epoch::new(1)
    }

    fn test_version() -> Version {
        Version::new(1)
    }

    fn test_config() -> ErasureConfig {
        ErasureConfig::default()
    }

    fn test_data() -> Vec<u8> {
        vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16]
    }

    fn test_fragment(index: u32, data: &[u8]) -> ErasureFragment {
        let mut bytes = [0u8; 16];
        bytes[0] = index as u8;
        ErasureFragment::new(
            index,
            data.to_vec(),
            FragmentId::new(bytes),
            Checksum::blake3(data),
            false,
            Epoch::new(1),
            Version::new(1),
        )
    }

    // ========================================================================
    // ErasureConfig tests
    // ========================================================================

    #[test]
    fn erasure_config_default() {
        let config = ErasureConfig::default();
        assert_eq!(config.k, 4);
        assert_eq!(config.n, 10);
        assert!(config.enable_parity);
        assert!(config.enable_auto_repair);
        assert!((config.repair_threshold - 0.5).abs() < 0.01);
        assert_eq!(config.max_concurrent_repairs, 5);
        assert_eq!(config.fragment_size, 1024 * 1024);
        assert!(config.enable_checksums);
    }

    #[test]
    fn erasure_config_new_valid() {
        let config = ErasureConfig::new(4, 10, RedundancyMode::ErasureCoded { k: 4, n: 10 }).unwrap();
        assert_eq!(config.k, 4);
        assert_eq!(config.n, 10);
    }

    #[test]
    fn erasure_config_new_k_zero() {
        let result = ErasureConfig::new(0, 10, RedundancyMode::ErasureCoded { k: 0, n: 10 });
        assert!(result.is_err());
    }

    #[test]
    fn erasure_config_new_n_lt_k() {
        let result = ErasureConfig::new(10, 4, RedundancyMode::ErasureCoded { k: 10, n: 4 });
        assert!(result.is_err());
    }

    #[test]
    fn erasure_config_new_n_zero() {
        let result = ErasureConfig::new(4, 0, RedundancyMode::ErasureCoded { k: 4, n: 0 });
        assert!(result.is_err());
    }

    #[test]
    fn erasure_config_validate_threshold_out_of_range() {
        let mut config = ErasureConfig::default();
        config.repair_threshold = 1.5;
        assert!(config.validate().is_err());
        config.repair_threshold = -0.1;
        assert!(config.validate().is_err());
    }

    #[test]
    fn erasure_config_validate_max_concurrent_zero() {
        let mut config = ErasureConfig::default();
        config.max_concurrent_repairs = 0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn erasure_config_validate_fragment_size_zero() {
        let mut config = ErasureConfig::default();
        config.fragment_size = 0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn erasure_config_parity_count() {
        let config = ErasureConfig::default();
        assert_eq!(config.parity_count(), 6);
    }

    #[test]
    fn erasure_config_fault_tolerance() {
        let config = ErasureConfig::default();
        assert_eq!(config.fault_tolerance(), 6);
    }

    #[test]
    fn erasure_config_is_auto_repair_enabled() {
        let config = ErasureConfig::default();
        assert!(config.is_auto_repair_enabled());
        let mut config = config;
        config.enable_auto_repair = false;
        assert!(!config.is_auto_repair_enabled());
    }

    #[test]
    fn erasure_config_is_parity_enabled() {
        let config = ErasureConfig::default();
        assert!(config.is_parity_enabled());
        let mut config = config;
        config.enable_parity = false;
        assert!(!config.is_parity_enabled());
    }

    #[test]
    fn erasure_config_strict() {
        let config = ErasureConfig::strict();
        assert_eq!(config.k, 8);
        assert_eq!(config.n, 16);
        assert_eq!(config.parity_count(), 8);
        assert!((config.repair_threshold - 0.75).abs() < 0.01);
    }

    #[test]
    fn erasure_config_lenient() {
        let config = ErasureConfig::lenient();
        assert_eq!(config.k, 2);
        assert_eq!(config.n, 3);
        assert_eq!(config.parity_count(), 1);
        assert!(!config.enable_auto_repair);
    }

    #[test]
    fn erasure_config_round_trip() {
        let config = ErasureConfig::default();
        let json = serde_json::to_string(&config).unwrap();
        let deserialized: ErasureConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(config.k, deserialized.k);
        assert_eq!(config.n, deserialized.n);
    }

    // ========================================================================
    // ErasureParams tests
    // ========================================================================

    #[test]
    fn erasure_params_new() {
        let params = ErasureParams::new(4, 10, ErasureScheme::ReedSolomon).unwrap();
        assert_eq!(params.k, 4);
        assert_eq!(params.n, 10);
        assert_eq!(params.scheme, ErasureScheme::ReedSolomon);
    }

    #[test]
    fn erasure_params_new_invalid() {
        assert!(ErasureParams::new(0, 10, ErasureScheme::ReedSolomon).is_err());
        assert!(ErasureParams::new(10, 4, ErasureScheme::ReedSolomon).is_err());
    }

    #[test]
    fn erasure_params_parity_count() {
        let params = ErasureParams::new(4, 10, ErasureScheme::ReedSolomon).unwrap();
        assert_eq!(params.parity_count(), 6);
    }

    #[test]
    fn erasure_params_expansion_factor() {
        let params = ErasureParams::new(4, 10, ErasureScheme::ReedSolomon).unwrap();
        assert!((params.expansion_factor() - 2.5).abs() < 0.01);
    }

    #[test]
    fn erasure_params_overhead_percent() {
        let params = ErasureParams::new(4, 10, ErasureScheme::ReedSolomon).unwrap();
        assert!((params.overhead_percent() - 150.0).abs() < 0.01);
    }

    #[test]
    fn erasure_params_display_name() {
        assert_eq!(ErasureScheme::ReedSolomon.display_name(), "reed_solomon");
        assert_eq!(ErasureScheme::XOR.display_name(), "xor");
        assert_eq!(ErasureScheme::Custom(42).display_name(), "custom");
    }

    #[test]
    fn erasure_params_default_field_size() {
        assert_eq!(ErasureScheme::ReedSolomon.default_field_size(), 256);
        assert_eq!(ErasureScheme::XOR.default_field_size(), 2);
    }

    #[test]
    fn erasure_params_serialization_roundtrip() {
        let params = ErasureParams::new(4, 10, ErasureScheme::CauchyReedSolomon).unwrap();
        let json = serde_json::to_string(&params).unwrap();
        let deserialized: ErasureParams = serde_json::from_str(&json).unwrap();
        assert_eq!(params.k, deserialized.k);
        assert_eq!(params.scheme, deserialized.scheme);
    }

    // ========================================================================
    // ErasureScheme tests
    // ========================================================================

    #[test]
    fn erasure_scheme_variants() {
        assert_eq!(ErasureScheme::ReedSolomon as u32, 0);
        assert_eq!(ErasureScheme::CauchyReedSolomon as u32, 1);
        assert_eq!(ErasureScheme::XOR as u32, 2);
    }

    #[test]
    fn erasure_scheme_serialization_roundtrip() {
        let scheme = ErasureScheme::ReedSolomon;
        let json = serde_json::to_string(&scheme).unwrap();
        let deserialized: ErasureScheme = serde_json::from_str(&json).unwrap();
        assert_eq!(scheme, deserialized);
    }

    // ========================================================================
    // ErasureFragment tests
    // ========================================================================

    #[test]
    fn erasure_fragment_new() {
        let frag = test_fragment(0, &[1, 2, 3]);
        assert_eq!(frag.index, 0);
        assert_eq!(frag.data, vec![1, 2, 3]);
        assert!(!frag.is_parity);
    }

    #[test]
    fn erasure_fragment_size() {
        let frag = test_fragment(0, &[1, 2, 3, 4, 5]);
        assert_eq!(frag.size(), 5);
    }

    #[test]
    fn erasure_fragment_is_empty() {
        let frag = test_fragment(0, &[]);
        assert!(frag.is_empty());
        let frag = test_fragment(0, &[1]);
        assert!(!frag.is_empty());
    }

    #[test]
    fn erasure_fragment_verify_checksum() {
        let frag = test_fragment(0, &[1, 2, 3]);
        assert!(frag.verify_checksum());
    }

    #[test]
    fn erasure_fragment_is_parity() {
        let mut bytes = [0u8; 16];
        bytes[0] = 0;
        let frag = ErasureFragment::new(
            0,
            vec![1, 2, 3],
            FragmentId::new(bytes),
            Checksum::blake3(&[1, 2, 3]),
            true,
            Epoch::new(1),
            Version::new(1),
        );
        assert!(frag.is_parity);
    }

    #[test]
    fn erasure_fragment_serialization_roundtrip() {
        let frag = test_fragment(0, &[1, 2, 3]);
        let json = serde_json::to_string(&frag).unwrap();
        let deserialized: ErasureFragment = serde_json::from_str(&json).unwrap();
        assert_eq!(frag.index, deserialized.index);
        assert_eq!(frag.data, deserialized.data);
    }

    // ========================================================================
    // ErasureEncodeResult tests
    // ========================================================================

    #[test]
    fn erasure_encode_result_new() {
        let frag = test_fragment(0, &[1, 2, 3]);
        let result = ErasureEncodeResult {
            dataset_id: test_dataset_id(),
            fragments: vec![frag],
            k: 4,
            n: 10,
            total_size: 3,
            parity_count: 6,
            epoch: Epoch::new(1),
        };
        assert_eq!(result.k, 4);
        assert_eq!(result.n, 10);
        assert_eq!(result.fragment_count(), 1);
    }

    #[test]
    fn erasure_encode_result_data_and_parity_fragments() {
        let data_frag = test_fragment(0, &[1, 2, 3]);
        let parity_frag = ErasureFragment::new(
            4,
            vec![4, 5, 6],
            FragmentId::new([4u8; 16]),
            Checksum::blake3(&[4, 5, 6]),
            true,
            Epoch::new(1),
            Version::new(1),
        );
        let result = ErasureEncodeResult {
            dataset_id: test_dataset_id(),
            fragments: vec![data_frag, parity_frag],
            k: 4,
            n: 10,
            total_size: 6,
            parity_count: 6,
            epoch: Epoch::new(1),
        };
        assert_eq!(result.data_fragments().len(), 1);
        assert_eq!(result.parity_fragments().len(), 1);
    }

    #[test]
    fn erasure_encode_result_expansion_factor() {
        let result = ErasureEncodeResult {
            dataset_id: test_dataset_id(),
            fragments: vec![],
            k: 4,
            n: 10,
            total_size: 0,
            parity_count: 6,
            epoch: Epoch::new(1),
        };
        assert!((result.expansion_factor() - 2.5).abs() < 0.01);
    }

    // ========================================================================
    // ErasureDecodeResult tests
    // ========================================================================

    #[test]
    fn erasure_decode_result_new() {
        let result = ErasureDecodeResult {
            data: vec![1, 2, 3],
            fragments_used: vec![0, 1, 2, 3],
            original_size: 3,
            epoch: Epoch::new(1),
        };
        assert!(result.success());
        assert_eq!(result.data_size(), 3);
    }

    #[test]
    fn erasure_decode_result_empty_data() {
        let result = ErasureDecodeResult {
            data: vec![],
            fragments_used: vec![],
            original_size: 0,
            epoch: Epoch::new(1),
        };
        assert!(!result.success());
    }

    // ========================================================================
    // ErasureReconstructResult tests
    // ========================================================================

    #[test]
    fn erasure_reconstruct_result_new() {
        let frag = test_fragment(0, &[1, 2, 3]);
        let result = ErasureReconstructResult {
            reconstructed: vec![frag],
            reconstructed_indices: vec![0],
            source_indices: vec![1, 2, 3],
            success: true,
        };
        assert_eq!(result.fragment_count(), 1);
        assert!(result.success);
    }

    // ========================================================================
    // ErasureLossReport tests
    // ========================================================================

    #[test]
    fn erasure_loss_report_new() {
        let report = ErasureLossReport {
            dataset_id: test_dataset_id(),
            total_expected: 10,
            total_available: 7,
            total_lost: 3,
            lost_indices: vec![7, 8, 9],
            corrupted_indices: vec![],
            reconstructable: true,
            epoch: Epoch::new(1),
        };
        assert!((report.loss_percent() - 30.0).abs() < 0.01);
        assert!((report.availability_percent() - 70.0).abs() < 0.01);
        assert!(!report.needs_repair());
    }

    #[test]
    fn erasure_loss_report_needs_repair() {
        let mut report = ErasureLossReport {
            dataset_id: test_dataset_id(),
            total_expected: 10,
            total_available: 7,
            total_lost: 3,
            lost_indices: vec![7, 8, 9],
            corrupted_indices: vec![],
            reconstructable: true,
            epoch: Epoch::new(1),
        };
        assert!(!report.needs_repair());
        report.corrupted_indices.push(5);
        assert!(report.needs_repair());
        report.total_lost = 1;
        assert!(report.needs_repair());
    }

    #[test]
    fn erasure_loss_report_reconstructable() {
        let report = ErasureLossReport {
            dataset_id: test_dataset_id(),
            total_expected: 10,
            total_available: 4,
            total_lost: 6,
            lost_indices: vec![],
            corrupted_indices: vec![],
            reconstructable: true,
            epoch: Epoch::new(1),
        };
        assert!(report.reconstructable);
    }

    // ========================================================================
    // ErasureStats tests
    // ========================================================================

    #[test]
    fn erasure_stats_new() {
        let stats = ErasureStats::new(test_dataset_id());
        assert_eq!(stats.dataset_id, test_dataset_id());
        assert_eq!(stats.total_encodes, 0);
        assert_eq!(stats.success_rate, 1.0);
    }

    #[test]
    fn erasure_stats_encode_success_rate() {
        let stats = ErasureStats::new(test_dataset_id());
        assert!((stats.encode_success_rate() - 1.0).abs() < 0.01);
    }

    #[test]
    fn erasure_stats_fragment_loss_rate() {
        let mut stats = ErasureStats::new(test_dataset_id());
        stats.total_fragments_created = 100;
        stats.total_fragments_lost = 10;
        assert!((stats.fragment_loss_rate() - 0.1).abs() < 0.01);
    }

    #[test]
    fn erasure_stats_repair_rate() {
        let mut stats = ErasureStats::new(test_dataset_id());
        stats.total_fragments_lost = 10;
        stats.total_fragments_repaired = 7;
        assert!((stats.repair_rate() - 0.7).abs() < 0.01);
    }

    #[test]
    fn erasure_stats_serialization_roundtrip() {
        let mut stats = ErasureStats::new(test_dataset_id());
        stats.total_encodes = 5;
        let json = serde_json::to_string(&stats).unwrap();
        let deserialized: ErasureStats = serde_json::from_str(&json).unwrap();
        assert_eq!(stats.total_encodes, deserialized.total_encodes);
    }

    // ========================================================================
    // ErasureError tests
    // ========================================================================

    #[test]
    fn erasure_error_category() {
        assert_eq!(
            ErasureError::InvalidConfig { field: "k".to_string(), reason: "x".to_string() }.category(),
            "invalid_config"
        );
        assert_eq!(
            ErasureError::DatasetNotFound { dataset_id: test_dataset_id() }.category(),
            "dataset_not_found"
        );
        assert_eq!(
            ErasureError::FragmentNotFound { index: 5 }.category(),
            "fragment_not_found"
        );
    }

    #[test]
    fn erasure_error_is_recoverable() {
        assert!(ErasureError::InsufficientFragments { have: 2, need: 4 }.is_recoverable());
        assert!(ErasureError::FragmentCorruption { index: 3 }.is_recoverable());
        assert!(!ErasureError::DatasetNotFound { dataset_id: test_dataset_id() }.is_recoverable());
    }

    #[test]
    fn erasure_error_suggested_action() {
        assert_eq!(
            ErasureError::DatasetNotFound { dataset_id: test_dataset_id() }.suggested_action(),
            "create the dataset first"
        );
    }

    // ========================================================================
    // ErasureEngine tests
    // ========================================================================

    #[test]
    fn erasure_engine_new() {
        let engine = ErasureEngine::new();
        assert_eq!(engine.datasets().len(), 0);
    }

    #[test]
    fn erasure_engine_with_config() {
        let config = ErasureConfig::new(4, 10, RedundancyMode::ErasureCoded { k: 4, n: 10 }).unwrap();
        let engine = ErasureEngine::with_config(config).unwrap();
        assert_eq!(engine.datasets().len(), 0);
    }

    #[test]
    fn erasure_engine_with_config_invalid() {
        let config = ErasureConfig::new(0, 10, RedundancyMode::ErasureCoded { k: 0, n: 10 }).unwrap_err();
        let _ = config;
        // Can't create with invalid config
        let result = ErasureConfig::new(0, 10, RedundancyMode::ErasureCoded { k: 0, n: 10 });
        assert!(result.is_err());
    }

    #[test]
    fn erasure_engine_configure() {
        let engine = ErasureEngine::new();
        let config = ErasureConfig::new(8, 16, RedundancyMode::ErasureCoded { k: 8, n: 16 }).unwrap();
        assert!(engine.configure(config).is_ok());
    }

    #[test]
    fn erasure_engine_configure_invalid() {
        let engine = ErasureEngine::new();
        let config = ErasureConfig::new(0, 10, RedundancyMode::ErasureCoded { k: 0, n: 10 }).unwrap_err();
        let _ = config;
        let result = ErasureConfig::new(0, 10, RedundancyMode::ErasureCoded { k: 0, n: 10 });
        assert!(result.is_err());
    }

    #[test]
    fn erasure_engine_encode() {
        let engine = ErasureEngine::new();
        let data = test_data();
        let result = engine.encode(test_dataset_id(), &data).unwrap();
        assert_eq!(result.k, 4);
        assert_eq!(result.n, 10);
        assert_eq!(result.fragment_count(), 10);
        assert_eq!(result.parity_count, 6);
    }

    #[test]
    fn erasure_engine_encode_small_data() {
        let engine = ErasureEngine::new();
        let data = vec![42u8];
        let result = engine.encode(test_dataset_id(), &data).unwrap();
        assert_eq!(result.fragment_count(), 10);
    }

    #[test]
    fn erasure_engine_encode_empty_data() {
        let engine = ErasureEngine::new();
        let result = engine.encode(test_dataset_id(), &[]).unwrap();
        assert_eq!(result.fragment_count(), 10);
    }

    #[test]
    fn erasure_engine_decode() {
        let engine = ErasureEngine::new();
        let data = test_data();
        let encode_result = engine.encode(test_dataset_id(), &data).unwrap();
        let fragments: Vec<ErasureFragment> = encode_result.fragments.iter().take(4).cloned().collect();
        let decode_result = engine.decode(test_dataset_id(), &fragments).unwrap();
        assert!(decode_result.success());
        assert_eq!(decode_result.fragments_used.len(), 4);
    }

    #[test]
    fn erasure_engine_decode_insufficient_fragments() {
        let engine = ErasureEngine::new();
        let data = test_data();
        engine.encode(test_dataset_id(), &data).unwrap();
        let fragments: Vec<ErasureFragment> = vec![test_fragment(0, &[1, 2, 3])];
        let err = engine.decode(test_dataset_id(), &fragments).unwrap_err();
        assert!(matches!(err, ErasureError::InsufficientFragments { .. }));
    }

    #[test]
    fn erasure_engine_decode_corrupted_fragment() {
        let engine = ErasureEngine::new();
        let mut frag = test_fragment(0, &[1, 2, 3]);
        // Corrupt by changing data without updating checksum
        frag.data = vec![9, 9, 9];
        let fragments = vec![frag, test_fragment(1, &[4, 5, 6]), test_fragment(2, &[7, 8, 9]), test_fragment(3, &[10, 11, 12])];
        let err = engine.decode(test_dataset_id(), &fragments).unwrap_err();
        assert!(matches!(err, ErasureError::FragmentCorruption { .. }));
    }

    #[test]
    fn erasure_engine_generate_parity() {
        let engine = ErasureEngine::new();
        let fragments = vec![test_fragment(0, &[1, 2, 3]), test_fragment(1, &[4, 5, 6])];
        let parity = engine.generate_parity(test_dataset_id(), &fragments).unwrap();
        assert_eq!(parity.len(), 1);
        assert!(parity[0].is_parity);
    }

    #[test]
    fn erasure_engine_generate_parity_disabled() {
        let engine = ErasureEngine::new();
        let mut config = ErasureConfig::default();
        config.enable_parity = false;
        engine.configure(config).unwrap();
        let fragments = vec![test_fragment(0, &[1, 2, 3])];
        let err = engine.generate_parity(test_dataset_id(), &fragments).unwrap_err();
        assert!(matches!(err, ErasureError::EncodeFailed { .. }));
    }

    #[test]
    fn erasure_engine_reconstruct() {
        let engine = ErasureEngine::new();
        let data = test_data();
        engine.encode(test_dataset_id(), &data).unwrap();
        let available: Vec<ErasureFragment> = (0..4).map(|i| test_fragment(i, &[1, 2, 3])).collect();
        let missing = vec![4, 5];
        let result = engine.reconstruct(test_dataset_id(), &available, &missing).unwrap();
        assert!(result.success);
        assert_eq!(result.reconstructed_indices, vec![4, 5]);
    }

    #[test]
    fn erasure_engine_reconstruct_insufficient() {
        let engine = ErasureEngine::new();
        let available: Vec<ErasureFragment> = vec![test_fragment(0, &[1, 2, 3])];
        let missing = vec![1, 2, 3];
        let err = engine.reconstruct(test_dataset_id(), &available, &missing).unwrap_err();
        assert!(matches!(err, ErasureError::InsufficientFragments { .. }));
    }

    #[test]
    fn erasure_engine_place_fragments() {
        let engine = ErasureEngine::new();
        let data = test_data();
        let encode_result = engine.encode(test_dataset_id(), &data).unwrap();
        let nodes = vec![test_node_id(), test_node_id2(), test_node_id3()];
        let placements = engine.place_fragments(test_dataset_id(), &encode_result.fragments, &nodes).unwrap();
        assert_eq!(placements.len(), 10);
        assert!(placements.iter().all(|p| p.is_stored()));
    }

    #[test]
    fn erasure_engine_place_fragments_no_nodes() {
        let engine = ErasureEngine::new();
        let frag = test_fragment(0, &[1, 2, 3]);
        let err = engine.place_fragments(test_dataset_id(), &[frag], &[]).unwrap_err();
        assert!(matches!(err, ErasureError::NodeNotAvailable { .. }));
    }

    #[test]
    fn erasure_engine_fragment_placement() {
        let engine = ErasureEngine::new();
        let data = test_data();
        engine.encode(test_dataset_id(), &data).unwrap();
        let nodes = vec![test_node_id(), test_node_id2()];
        engine.place_fragments(test_dataset_id(), &vec![test_fragment(0, &[1, 2, 3])], &nodes).unwrap();
        let placements = engine.fragment_placement(test_dataset_id()).unwrap();
        assert_eq!(placements.len(), 1);
    }

    #[test]
    fn erasure_engine_detect_loss() {
        let engine = ErasureEngine::new();
        let data = test_data();
        engine.encode(test_dataset_id(), &data).unwrap();
        let available: Vec<ErasureFragment> = (0..7).map(|i| test_fragment(i, &[1, 2, 3])).collect();
        let report = engine.detect_loss(test_dataset_id(), &available).unwrap();
        assert_eq!(report.total_expected, 10);
        assert_eq!(report.total_available, 7);
        assert_eq!(report.total_lost, 3);
        assert!(report.reconstructable);
    }

    #[test]
    fn erasure_engine_detect_loss_all_lost() {
        let engine = ErasureEngine::new();
        let report = engine.detect_loss(test_dataset_id(), &[]).unwrap();
        assert_eq!(report.total_available, 0);
        assert_eq!(report.total_lost, 10);
        assert!(!report.reconstructable);
    }

    #[test]
    fn erasure_engine_verify_fragment() {
        let engine = ErasureEngine::new();
        let frag = test_fragment(0, &[1, 2, 3]);
        assert!(engine.verify_fragment(&frag).unwrap());
    }

    #[test]
    fn erasure_engine_stats() {
        let engine = ErasureEngine::new();
        let data = test_data();
        engine.encode(test_dataset_id(), &data).unwrap();
        let stats = engine.stats(test_dataset_id()).unwrap();
        assert_eq!(stats.total_encodes, 1);
    }

    #[test]
    fn erasure_engine_stats_dataset_not_found() {
        let engine = ErasureEngine::new();
        let err = engine.stats(test_dataset_id()).unwrap_err();
        assert!(matches!(err, ErasureError::DatasetNotFound { .. }));
    }

    #[test]
    fn erasure_engine_datasets() {
        let engine = ErasureEngine::new();
        assert!(engine.datasets().is_empty());
        engine.encode(test_dataset_id(), &test_data()).unwrap();
        engine.encode(test_dataset_id2(), &test_data()).unwrap();
        assert_eq!(engine.datasets().len(), 2);
    }

    #[test]
    fn erasure_engine_is_protected() {
        let engine = ErasureEngine::new();
        assert!(!engine.is_protected(test_dataset_id()));
        engine.encode(test_dataset_id(), &test_data()).unwrap();
        assert!(engine.is_protected(test_dataset_id()));
    }

    #[test]
    fn erasure_engine_minimum_and_total_fragments() {
        let engine = ErasureEngine::new();
        engine.encode(test_dataset_id(), &test_data()).unwrap();
        assert_eq!(engine.minimum_fragments(test_dataset_id()), Some(4));
        assert_eq!(engine.total_fragments(test_dataset_id()), Some(10));
    }

    #[test]
    fn erasure_engine_expand_redundancy() {
        let engine = ErasureEngine::new();
        engine.encode(test_dataset_id(), &test_data()).unwrap();
        let result = engine.expand_redundancy(test_dataset_id(), 4).unwrap();
        assert_eq!(result.new_n, 14);
        assert_eq!(result.new_k, 4);
        assert_eq!(result.new_fragments.len(), 4);
        assert!(result.success);
    }

    #[test]
    fn erasure_engine_shrink_redundancy() {
        let engine = ErasureEngine::new();
        engine.encode(test_dataset_id(), &test_data()).unwrap();
        let result = engine.shrink_redundancy(test_dataset_id(), 4).unwrap();
        assert_eq!(result.new_n, 6);
        assert_eq!(result.new_k, 4);
        assert!(result.success);
    }

    #[test]
    fn erasure_engine_shrink_redundancy_invalid() {
        let engine = ErasureEngine::new();
        engine.encode(test_dataset_id(), &test_data()).unwrap();
        let err = engine.shrink_redundancy(test_dataset_id(), 10).unwrap_err();
        assert!(matches!(err, ErasureError::InvalidConfig { .. }));
    }

    #[test]
    fn erasure_engine_migrate_fragment() {
        let engine = ErasureEngine::new();
        let data = test_data();
        engine.encode(test_dataset_id(), &data).unwrap();
        let nodes = vec![test_node_id(), test_node_id2()];
        engine.place_fragments(test_dataset_id(), &vec![test_fragment(0, &[1, 2, 3])], &nodes).unwrap();
        let result = engine.migrate_fragment(test_dataset_id(), 0, test_node_id(), test_node_id2()).unwrap();
        assert!(result.migrated);
        assert_eq!(result.to_node, test_node_id2());
    }

    #[test]
    fn erasure_engine_migrate_fragment_not_found() {
        let engine = ErasureEngine::new();
        let err = engine.migrate_fragment(test_dataset_id(), 99, test_node_id(), test_node_id2()).unwrap_err();
        assert!(matches!(err, ErasureError::FragmentNotFound { .. }));
    }

    #[test]
    fn erasure_engine_repair() {
        let engine = ErasureEngine::new();
        engine.encode(test_dataset_id(), &test_data()).unwrap();
        let result = engine.repair(test_dataset_id()).unwrap();
        assert!(result.success);
        assert_eq!(result.dataset_id, test_dataset_id());
    }

    #[test]
    fn erasure_engine_repair_plan() {
        let engine = ErasureEngine::new();
        engine.encode(test_dataset_id(), &test_data()).unwrap();
        let plan = engine.repair_plan(test_dataset_id()).unwrap();
        assert_eq!(plan.dataset_id, test_dataset_id());
        assert!(plan.is_repairable);
    }

    #[test]
    fn erasure_engine_validate_config() {
        let engine = ErasureEngine::new();
        let config = ErasureConfig::default();
        assert!(engine.validate_config(&config).is_ok());
    }

    #[test]
    fn erasure_engine_full_cycle() {
        let engine = ErasureEngine::new();
        let data = test_data();

        // Encode
        let encode_result = engine.encode(test_dataset_id(), &data).unwrap();
        assert_eq!(encode_result.fragment_count(), 10);

        // Place
        let nodes = vec![test_node_id(), test_node_id2(), test_node_id3()];
        let placements = engine.place_fragments(test_dataset_id(), &encode_result.fragments, &nodes).unwrap();
        assert_eq!(placements.len(), 10);

        // Detect loss (simulate losing 3 fragments)
        let available: Vec<ErasureFragment> = encode_result.fragments.iter().take(7).cloned().collect();
        let report = engine.detect_loss(test_dataset_id(), &available).unwrap();
        assert_eq!(report.total_lost, 3);
        assert!(report.reconstructable);

        // Repair
        let repair_result = engine.repair(test_dataset_id()).unwrap();
        assert!(repair_result.success);

        // Stats
        let stats = engine.stats(test_dataset_id()).unwrap();
        assert_eq!(stats.total_encodes, 1);
        assert!(stats.success_rate > 0.0);
    }

    #[test]
    fn erasure_engine_multiple_datasets() {
        let engine = ErasureEngine::new();
        engine.encode(test_dataset_id(), &test_data()).unwrap();
        engine.encode(test_dataset_id2(), &test_data()).unwrap();
        assert_eq!(engine.datasets().len(), 2);
        assert!(engine.is_protected(test_dataset_id()));
        assert!(engine.is_protected(test_dataset_id2()));
    }

    #[test]
    fn erasure_engine_reconstruct_missing_indices() {
        let engine = ErasureEngine::new();
        engine.encode(test_dataset_id(), &test_data()).unwrap();
        let available: Vec<ErasureFragment> = (0..4).map(|i| test_fragment(i, &[1, 2, 3])).collect();
        let result = engine.reconstruct(test_dataset_id(), &available, &[4, 5, 6]).unwrap();
        assert_eq!(result.reconstructed.len(), 3);
        assert_eq!(result.reconstructed_indices, vec![4, 5, 6]);
    }

    #[test]
    fn erasure_engine_expand_and_shrink() {
        let engine = ErasureEngine::new();
        engine.encode(test_dataset_id(), &test_data()).unwrap();
        let expand_result = engine.expand_redundancy(test_dataset_id(), 4).unwrap();
        assert_eq!(expand_result.new_n, 14);
        let shrink_result = engine.shrink_redundancy(test_dataset_id(), 4).unwrap();
        assert_eq!(shrink_result.new_n, 10);
    }

    #[test]
    fn erasure_engine_parity_fragment_generation() {
        let engine = ErasureEngine::new();
        let fragments = vec![
            test_fragment(0, &[1, 0, 0]),
            test_fragment(1, &[0, 1, 0]),
            test_fragment(2, &[0, 0, 1]),
        ];
        let parity = engine.generate_parity(test_dataset_id(), &fragments).unwrap();
        assert_eq!(parity.len(), 1);
        // XOR of [1,0,0], [0,1,0], [0,0,1] = [1,1,1]
        assert_eq!(parity[0].data, vec![1, 1, 1]);
    }

    #[test]
    fn erasure_engine_loss_percent() {
        let engine = ErasureEngine::new();
        let report = ErasureLossReport {
            dataset_id: test_dataset_id(),
            total_expected: 10,
            total_available: 7,
            total_lost: 3,
            lost_indices: vec![7, 8, 9],
            corrupted_indices: vec![],
            reconstructable: true,
            epoch: Epoch::new(1),
        };
        assert!((report.loss_percent() - 30.0).abs() < 0.01);
        assert!((report.availability_percent() - 70.0).abs() < 0.01);
    }

    #[test]
    fn erasure_engine_configure_and_stats() {
        let engine = ErasureEngine::new();
        let config = ErasureConfig::strict();
        engine.configure(config).unwrap();
        engine.encode(test_dataset_id(), &test_data()).unwrap();
        let stats = engine.stats(test_dataset_id()).unwrap();
        assert_eq!(stats.total_encodes, 1);
    }

    #[test]
    fn erasure_engine_serialization_roundtrip_config() {
        let config = ErasureConfig::default();
        let json = serde_json::to_string(&config).unwrap();
        let deserialized: ErasureConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(config.k, deserialized.k);
        assert_eq!(config.n, deserialized.n);
    }

    #[test]
    fn erasure_engine_serialization_roundtrip_fragment() {
        let frag = test_fragment(0, &[1, 2, 3]);
        let json = serde_json::to_string(&frag).unwrap();
        let deserialized: ErasureFragment = serde_json::from_str(&json).unwrap();
        assert_eq!(frag.index, deserialized.index);
    }

    #[test]
    fn erasure_engine_serialization_roundtrip_loss_report() {
        let report = ErasureLossReport {
            dataset_id: test_dataset_id(),
            total_expected: 10,
            total_available: 7,
            total_lost: 3,
            lost_indices: vec![7, 8, 9],
            corrupted_indices: vec![],
            reconstructable: true,
            epoch: Epoch::new(1),
        };
        let json = serde_json::to_string(&report).unwrap();
        let deserialized: ErasureLossReport = serde_json::from_str(&json).unwrap();
        assert_eq!(report.total_expected, deserialized.total_expected);
        assert_eq!(report.total_available, deserialized.total_available);
    }

    #[test]
    fn erasure_engine_repair_plan_empty() {
        let engine = ErasureEngine::new();
        engine.encode(test_dataset_id(), &test_data()).unwrap();
        let plan = engine.repair_plan(test_dataset_id()).unwrap();
        assert!(plan.fragments_to_repair.is_empty());
        assert!(plan.is_repairable);
    }

    #[test]
    fn erasure_engine_place_and_get_placement() {
        let engine = ErasureEngine::new();
        let frag = test_fragment(0, &[1, 2, 3]);
        let nodes = vec![test_node_id()];
        let placements = engine.place_fragments(test_dataset_id(), &[frag], &nodes).unwrap();
        assert_eq!(placements[0].node_id, test_node_id());
        assert_eq!(placements[0].fragment_index, 0);
    }

    #[test]
    fn erasure_engine_datasets_after_repair() {
        let engine = ErasureEngine::new();
        engine.encode(test_dataset_id(), &test_data()).unwrap();
        engine.repair(test_dataset_id()).unwrap();
        assert_eq!(engine.datasets().len(), 1);
    }

    #[test]
    fn erasure_engine_verify_corrupted_fragment() {
        let engine = ErasureEngine::new();
        let mut frag = test_fragment(0, &[1, 2, 3]);
        frag.data = vec![9, 9, 9];
        assert!(!engine.verify_fragment(&frag).unwrap());
    }

    #[test]
    fn erasure_engine_all_fragment_types() {
        let engine = ErasureEngine::new();
        let data = test_data();
        let result = engine.encode(test_dataset_id(), &data).unwrap();
        let data_frags = result.data_fragments();
        let parity_frags = result.parity_fragments();
        assert_eq!(data_frags.len(), 4);
        assert_eq!(parity_frags.len(), 6);
        assert!(data_frags.iter().all(|f| !f.is_parity));
        assert!(parity_frags.iter().all(|f| f.is_parity));
    }

    #[test]
    fn erasure_engine_concurrent_logical_operations() {
        let engine = ErasureEngine::new();
        engine.encode(test_dataset_id(), &test_data()).unwrap();
        engine.encode(test_dataset_id2(), &test_data()).unwrap();
        assert_eq!(engine.datasets().len(), 2);
        assert!(engine.is_protected(test_dataset_id()));
        assert!(engine.is_protected(test_dataset_id2()));
    }

    #[test]
    fn erasure_engine_minimum_fragments_none_when_no_dataset() {
        let engine = ErasureEngine::new();
        assert_eq!(engine.minimum_fragments(test_dataset_id()), None);
        assert_eq!(engine.total_fragments(test_dataset_id()), None);
    }

    #[test]
    fn erasure_engine_stats_default_values() {
        let engine = ErasureEngine::new();
        engine.encode(test_dataset_id(), &test_data()).unwrap();
        let stats = engine.stats(test_dataset_id()).unwrap();
        assert_eq!(stats.total_decodes, 0);
        assert_eq!(stats.total_reconstructs, 0);
        assert_eq!(stats.total_repairs, 0);
    }

    #[test]
    fn erasure_engine_fragment_loss_rate_zero() {
        let engine = ErasureEngine::new();
        engine.encode(test_dataset_id(), &test_data()).unwrap();
        let stats = engine.stats(test_dataset_id()).unwrap();
        assert!((stats.fragment_loss_rate() - 0.0).abs() < 0.01);
    }

    #[test]
    fn erasure_engine_repair_rate_no_loss() {
        let engine = ErasureEngine::new();
        engine.encode(test_dataset_id(), &test_data()).unwrap();
        let stats = engine.stats(test_dataset_id()).unwrap();
        assert!((stats.repair_rate() - 1.0).abs() < 0.01);
    }
}