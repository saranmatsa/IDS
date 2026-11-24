//! # Erasure Threshold Invariant
//!
//! This invariant enforces the erasure-coding requirement: any k of n encoded
//! chunks must be sufficient to reconstruct the original data, tolerating
//! up to n - k simultaneous chunk failures without data loss.
//!
//! The erasure threshold subsystem provides:
//! - Coding parameter management (k data chunks, n total chunks) per dataset
//! - Reconstruction feasibility checks (available >= k)
//! - Failure tolerance calculation (n - k)
//! - Reed-Solomon (n,k) code parameter validation and bounds
//! - Regenerating code and locally repairable code (LRC) configuration
//! - Fragment placement with failure-domain awareness
//! - Parity chunk generation and verification mathematics
//! - Chunk health tracking and degradation detection
//! - Reconstruction scheduling and priority ordering
//! - Capacity planning for erasure-coded datasets
//! - Minimum chunk requirements for read and write paths
//! - Erasure ratio and storage overhead computation
//! - Chunk loss tolerance under correlated and independent failures
//! - Coding scheme selection (systematic vs. parity-only vs. hybrid)
//! - Integrity verification of reconstructed data
//! - Dynamic re-coding parameter adjustment on cluster resize
//! - Cross-region erasure placement constraints

use crate::types::*;
use crate::errors::InvariantError;
use crate::versioning::EdgeCaseBits;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

///////////////////////////////////////////////////////////////////////////////
// Sealed trait declaration
///////////////////////////////////////////////////////////////////////////////

/// Invariant: any k of n erasure-coded chunks reconstructs the data;
/// tolerates n - k chunk failures.
///
/// Downstream crates use this as a trait bound but cannot implement it.
/// Only `ids-000-aa` and its designated implementation crates may `impl`.
pub trait ErasureThreshold: private::Sealed {
    /// Return the coding parameters (k, n) for a dataset: k data chunks,
    /// n total encoded chunks, any k of which suffice to reconstruct.
    fn coding_params(&self, dataset_id: DatasetId) -> Result<(u32, u32), ErasureError>;

    /// Return true if `available` chunks are sufficient to reconstruct
    /// data encoded with parameter k.
    fn can_reconstruct(&self, available: u32, k: u32) -> bool;

    /// Return the maximum number of chunk failures tolerable: n - k.
    fn failure_tolerance(&self, n: u32, k: u32) -> u32;

    /// Validate that a proposed (k, n) coding pair is well-formed.
    fn validate_coding_params(&self, k: u32, n: u32) -> Result<(), ErasureError>;

    /// Compute the storage overhead ratio n / k as a f64.
    fn storage_overhead_ratio(k: u32, n: u32) -> f64;

    /// Compute the minimum number of chunks required for a read quorum.
    fn read_quorum_chunks(k: u32) -> u32;

    /// Compute the minimum number of chunks required for a write quorum
    /// such that read and write quorums intersect in at least one data chunk.
    fn write_quorum_chunks(n: u32, k: u32) -> u32;

    /// Check whether a chunk loss pattern leaves enough chunks to reconstruct.
    fn survives_chunk_loss(&self, n: u32, k: u32, lost: &[u32]) -> bool;

    /// Return the number of parity chunks: n - k.
    fn parity_chunk_count(k: u32, n: u32) -> u32;

    /// Return whether a coding scheme is systematic (data chunks stored verbatim).
    fn is_systematic(k: u32, n: u32, scheme: CodingScheme) -> bool;

    /// Select an appropriate (k, n) for a target failure tolerance f.
    fn select_params_for_tolerance(f: u32, min_k: u32, max_n: u32) -> Option<(u32, u32)>;

    /// Return the chunk indices that constitute a reconstruction set of size k.
    fn reconstruction_set(n: u32, k: u32, available: &BTreeSet<u32>) -> Option<Vec<u32>>;

    /// Check whether erasure coding is enabled for a dataset.
    fn is_erasure_enabled(&self, dataset_id: DatasetId) -> bool;

    /// Register coding parameters for a dataset.
    fn register_coding_params(
        &mut self,
        dataset_id: DatasetId,
        k: u32,
        n: u32,
        scheme: CodingScheme,
    ) -> Result<(), ErasureError>;

    /// Return all registered datasets and their coding parameters.
    fn registered_datasets(&self) -> Vec<(DatasetId, u32, u32)>;

    /// Compute the chunk placement across failure domains for durability.
    fn place_chunks_across_domains(
        &self,
        n: u32,
        domains: &[FailureDomainRef],
    ) -> Result<Vec<ChunkPlacement>, ErasureError>;
}

///////////////////////////////////////////////////////////////////////////////
// Sealed marker module
///////////////////////////////////////////////////////////////////////////////

mod private {
    pub trait Sealed {}
}

impl private::Sealed for () {}

///////////////////////////////////////////////////////////////////////////////
// Reference no-op impl for compile-time checks
///////////////////////////////////////////////////////////////////////////////

impl ErasureThreshold for () {
    fn coding_params(&self, _dataset_id: DatasetId) -> Result<(u32, u32), ErasureError> {
        Err(ErasureError::NoErasureEngine)
    }

    fn can_reconstruct(&self, _available: u32, _k: u32) -> bool {
        false
    }

    fn failure_tolerance(&self, _n: u32, _k: u32) -> u32 {
        0
    }

    fn validate_coding_params(&self, _k: u32, _n: u32) -> Result<(), ErasureError> {
        Err(ErasureError::NoErasureEngine)
    }

    fn storage_overhead_ratio(_k: u32, _n: u32) -> f64 {
        0.0
    }

    fn read_quorum_chunks(_k: u32) -> u32 {
        0
    }

    fn write_quorum_chunks(_n: u32, _k: u32) -> u32 {
        0
    }

    fn survives_chunk_loss(&self, _n: u32, _k: u32, _lost: &[u32]) -> bool {
        false
    }

    fn parity_chunk_count(_k: u32, _n: u32) -> u32 {
        0
    }

    fn is_systematic(_k: u32, _n: u32, _scheme: CodingScheme) -> bool {
        false
    }

    fn select_params_for_tolerance(
        _f: u32,
        _min_k: u32,
        _max_n: u32,
    ) -> Option<(u32, u32)> {
        None
    }

    fn reconstruction_set(
        _n: u32,
        _k: u32,
        _available: &BTreeSet<u32>,
    ) -> Option<Vec<u32>> {
        None
    }

    fn is_erasure_enabled(&self, _dataset_id: DatasetId) -> bool {
        false
    }

    fn register_coding_params(
        &mut self,
        _dataset_id: DatasetId,
        _k: u32,
        _n: u32,
        _scheme: CodingScheme,
    ) -> Result<(), ErasureError> {
        Err(ErasureError::NoErasureEngine)
    }

    fn registered_datasets(&self) -> Vec<(DatasetId, u32, u32)> {
        Vec::new()
    }

    fn place_chunks_across_domains(
        &self,
        _n: u32,
        _domains: &[FailureDomainRef],
    ) -> Result<Vec<ChunkPlacement>, ErasureError> {
        Err(ErasureError::NoErasureEngine)
    }
}

///////////////////////////////////////////////////////////////////////////////
// Production types and subsystems
///////////////////////////////////////////////////////////////////////////////

/// Coding scheme used for erasure encoding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CodingScheme {
    /// Reed-Solomon (n, k) maximum distance separable code.
    ReedSolomon,
    /// Systematic Reed-Solomon: data chunks stored verbatim + parity.
    SystematicReedSolomon,
    /// Locally repairable code with locality groups.
    LocallyRepairable,
    /// Regenerating code for bandwidth-efficient repair.
    Regenerating,
    /// XOR-based parity (RAID-like).
    XorParity,
    /// Hybrid: systematic RS with local parity groups.
    Hybrid,
}

/// The coding parameters (k, n) for a dataset.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ErasureParams {
    pub k: u32, // data chunks
    pub n: u32, // total encoded chunks
    pub scheme: CodingScheme,
}

/// Result of a reconstruction attempt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReconstructionResult {
    pub dataset_id: DatasetId,
    pub chunks_used: Vec<u32>,
    pub chunks_required: u32,
    pub reconstructed_size: u64,
    pub success: bool,
    pub checksum_valid: bool,
}

/// Reference to a failure domain for chunk placement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FailureDomainRef(pub u32);

/// Placement of a single encoded chunk within a failure domain.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChunkPlacement {
    pub chunk_index: u32,
    pub domain: FailureDomainRef,
    pub node_hint: Option<NodeId>,
}

/// Health status of an individual encoded chunk.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChunkHealth {
    Healthy,
    Degraded,
    Lost,
    Recovering,
}

/// Tracks health of all chunks for a dataset.
pub struct FragmentHealthTracker {
    /// Per-chunk health status.
    health: HashMap<u32, ChunkHealth>,
    /// Total chunk count.
    total_chunks: u32,
    /// Data chunk count (k).
    k: u32,
    /// Last health check timestamp.
    last_check: TimestampMillis,
}

/// Result of scheduling reconstruction work.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReconstructionSchedule {
    pub dataset_id: DatasetId,
    pub chunks_to_reconstruct: Vec<u32>,
    pub source_chunks: Vec<u32>,
    pub estimated_bandwidth_bytes: u64,
    pub priority: ReconstructionPriority,
}

/// Priority of a reconstruction operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ReconstructionPriority {
    Low,
    Normal,
    High,
    Critical,
}

/// Parameters for regenerating codes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegeneratingCodeConfig {
    pub k: u32,
    pub n: u32,
    pub d: u32, // helper nodes contacted during repair
    pub alpha: u32, // symbols stored per node
    pub beta: u32, // symbols transferred per helper
}

/// Parameters for locally repairable codes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LrcConfig {
    pub k: u32,
    pub n: u32,
    pub locality: u32, // max nodes to contact for single-chunk repair
}

/// Capacity plan for an erasure-coded dataset.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ErasureCapacityPlan {
    pub dataset_id: DatasetId,
    pub raw_data_bytes: u64,
    pub encoded_bytes: u64,
    pub overhead_bytes: u64,
    pub overhead_ratio: f64,
    pub failure_tolerance: u32,
    pub chunks: u32,
    pub data_chunks: u32,
}

/// Errors returned by erasure threshold operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErasureError {
    /// No erasure engine is configured.
    NoErasureEngine,
    /// Invalid coding parameters (k == 0 or n < k).
    InvalidParams { k: u32, n: u32, reason: String },
    /// Dataset not registered in the erasure subsystem.
    DatasetNotFound { dataset: DatasetId },
    /// Insufficient chunks available to reconstruct.
    InsufficientChunks { available: u32, required: u32 },
    /// Too many chunks lost to tolerate.
    ExcessiveLoss { lost: u32, tolerance: u32 },
    /// Coding scheme not supported.
    UnsupportedScheme { scheme: CodingScheme },
    /// Failure domain count insufficient for placement.
    InsufficientDomains { have: u32, need: u32 },
    /// Internal erasure subsystem error.
    Internal { message: String },
}

/// Production implementation of the ErasureThreshold invariant.
///
/// Maintains per-dataset coding parameters, chunk health tracking,
/// reconstruction scheduling, failure-domain-aware placement, and
/// capacity planning. All mathematics are deterministic and
/// carrier-agnostic.
pub struct ErasureThresholdManager {
    /// Per-dataset coding parameters.
    params: HashMap<DatasetId, ErasureParams>,
    /// Whether erasure coding is enabled per dataset.
    enabled: HashSet<DatasetId>,
    /// Chunk health trackers per dataset.
    health: HashMap<DatasetId, FragmentHealthTracker>,
    /// Reconstruction schedules per dataset.
    schedules: HashMap<DatasetId, ReconstructionSchedule>,
    /// Total reconstruction operations performed.
    reconstruction_count: u64,
    /// Total successful reconstructions.
    successful_reconstructions: u64,
    /// Total failed reconstructions.
    failed_reconstructions: u64,
    /// Default coding scheme.
    default_scheme: CodingScheme,
    /// Default k for new datasets.
    default_k: u32,
    /// Default n for new datasets.
    default_n: u32,
}

impl ErasureThresholdManager {
    /// Create a new manager with default (k=4, n=7) systematic Reed-Solomon.
    pub fn new() -> Self {
        ErasureThresholdManager {
            params: HashMap::new(),
            enabled: HashSet::new(),
            health: HashMap::new(),
            schedules: HashMap::new(),
            reconstruction_count: 0,
            successful_reconstructions: 0,
            failed_reconstructions: 0,
            default_scheme: CodingScheme::SystematicReedSolomon,
            default_k: 4,
            default_n: 7,
        }
    }

    /// Create a manager with custom defaults.
    pub fn with_defaults(scheme: CodingScheme, k: u32, n: u32) -> Result<Self, ErasureError> {
        if k == 0 || n < k {
            return Err(ErasureError::InvalidParams {
                k,
                n,
                reason: "invalid default coding params".to_string(),
            });
        }
        Ok(ErasureThresholdManager {
            params: HashMap::new(),
            enabled: HashSet::new(),
            health: HashMap::new(),
            schedules: HashMap::new(),
            reconstruction_count: 0,
            successful_reconstructions: 0,
            failed_reconstructions: 0,
            default_scheme: scheme,
            default_k: k,
            default_n: n,
        })
    }

    /// Return the coding parameters for a dataset.
    pub fn coding_params(
        &self,
        dataset_id: DatasetId,
    ) -> Result<(u32, u32), ErasureError> {
        self.params
            .get(&dataset_id)
            .map(|p| (p.k, p.n))
            .ok_or_else(|| ErasureError::DatasetNotFound { dataset: dataset_id })
    }

    /// Return true if `available` chunks suffice to reconstruct with parameter k.
    pub fn can_reconstruct(&self, available: u32, k: u32) -> bool {
        available >= k && k > 0
    }

    /// Return the maximum tolerable chunk failures: n - k.
    pub fn failure_tolerance(&self, n: u32, k: u32) -> u32 {
        if k == 0 || n < k {
            return 0;
        }
        n.saturating_sub(k)
    }

    /// Validate a proposed (k, n) coding pair.
    pub fn validate_coding_params(&self, k: u32, n: u32) -> Result<(), ErasureError> {
        if k == 0 {
            return Err(ErasureError::InvalidParams {
                k,
                n,
                reason: "k must be greater than zero".to_string(),
            });
        }
        if n < k {
            return Err(ErasureError::InvalidParams {
                k,
                n,
                reason: "n must be >= k".to_string(),
            });
        }
        if n > 1024 {
            return Err(ErasureError::InvalidParams {
                k,
                n,
                reason: "n exceeds maximum chunk count of 1024".to_string(),
            });
        }
        Ok(())
    }

    /// Compute the storage overhead ratio n / k.
    pub fn storage_overhead_ratio(k: u32, n: u32) -> f64 {
        if k == 0 {
            return 0.0;
        }
        n as f64 / k as f64
    }

    /// Compute the minimum read quorum chunk count: k chunks.
    pub fn read_quorum_chunks(k: u32) -> u32 {
        k
    }

    /// Compute the minimum write quorum chunk count such that read and write
    /// quorums intersect in at least one data chunk: n - k + 1.
    pub fn write_quorum_chunks(n: u32, k: u32) -> u32 {
        if k == 0 || n < k {
            return 0;
        }
        n.saturating_sub(k).saturating_add(1).min(n)
    }

    /// Check whether a chunk loss pattern leaves enough chunks to reconstruct.
    pub fn survives_chunk_loss(&self, n: u32, k: u32, lost: &[u32]) -> bool {
        if k == 0 || n < k {
            return false;
        }
        let lost_count = lost.len() as u32;
        let available = n.saturating_sub(lost_count);
        available >= k
    }

    /// Return the number of parity chunks: n - k.
    pub fn parity_chunk_count(k: u32, n: u32) -> u32 {
        if n < k {
            return 0;
        }
        n.saturating_sub(k)
    }

    /// Return whether a coding scheme is systematic.
    pub fn is_systematic(_k: u32, _n: u32, scheme: CodingScheme) -> bool {
        matches!(
            scheme,
            CodingScheme::SystematicReedSolomon
                | CodingScheme::XorParity
                | CodingScheme::Hybrid
        )
    }

    /// Select (k, n) for a target failure tolerance f with n <= max_n and k >= min_k.
    /// Tolerance f means n - k >= f, i.e., k <= n - f.
    pub fn select_params_for_tolerance(
        f: u32,
        min_k: u32,
        max_n: u32,
    ) -> Option<(u32, u32)> {
        if min_k == 0 || max_n < min_k || f == 0 {
            return None;
        }
        // Prefer the smallest n that satisfies n - k >= f with k >= min_k.
        // Choose k = min_k, n = min_k + f, clamped to max_n.
        let n = (min_k + f).min(max_n);
        if n < min_k {
            return None;
        }
        let k = min_k;
        if n.saturating_sub(k) >= f {
            return Some((k, n));
        }
        None
    }

    /// Return the chunk indices for a reconstruction set of size k from available chunks.
    pub fn reconstruction_set(
        n: u32,
        k: u32,
        available: &BTreeSet<u32>,
    ) -> Option<Vec<u32>> {
        if k == 0 || (available.len() as u32) < k {
            return None;
        }
        let mut selected: Vec<u32> = available.iter().cloned().take(k as usize).collect();
        selected.sort();
        selected.dedup();
        if selected.len() < k as usize {
            return None;
        }
        selected.truncate(k as usize);
        Some(selected)
    }

    /// Return whether erasure coding is enabled for a dataset.
    pub fn is_erasure_enabled(&self, dataset_id: DatasetId) -> bool {
        self.enabled.contains(&dataset_id)
    }

    /// Register coding parameters for a dataset.
    pub fn register_coding_params(
        &mut self,
        dataset_id: DatasetId,
        k: u32,
        n: u32,
        scheme: CodingScheme,
    ) -> Result<(), ErasureError> {
        self.validate_coding_params(k, n)?;
        self.params.insert(
            dataset_id,
            ErasureParams { k, n, scheme },
        );
        self.enabled.insert(dataset_id);
        self.health.insert(
            dataset_id,
            FragmentHealthTracker {
                health: (0..n).map(|i| (i, ChunkHealth::Healthy)).collect(),
                total_chunks: n,
                k,
                last_check: TimestampMillis(0),
            },
        );
        Ok(())
    }

    /// Return all registered datasets and their coding parameters.
    pub fn registered_datasets(&self) -> Vec<(DatasetId, u32, u32)> {
        let mut result: Vec<(DatasetId, u32, u32)> = self
            .params
            .iter()
            .map(|(id, p)| (*id, p.k, p.n))
            .collect();
        result.sort_by(|a, b| a.0.cmp(&b.0));
        result
    }

    /// Place chunks across failure domains for durability.
    pub fn place_chunks_across_domains(
        &self,
        n: u32,
        domains: &[FailureDomainRef],
    ) -> Result<Vec<ChunkPlacement>, ErasureError> {
        if domains.is_empty() {
            return Err(ErasureError::InsufficientDomains { have: 0, need: 1 });
        }
        if (domains.len() as u32) < n {
            return Err(ErasureError::InsufficientDomains {
                have: domains.len() as u32,
                need: n,
            });
        }
        let mut placement: Vec<ChunkPlacement> = Vec::new();
        for i in 0..n {
            let domain = domains[i as usize % domains.len()];
            placement.push(ChunkPlacement {
                chunk_index: i,
                domain,
                node_hint: None,
            });
        }
        Ok(placement)
    }

    /// Return the chunk health tracker for a dataset.
    pub fn health_tracker(&self, dataset_id: DatasetId) -> Option<&FragmentHealthTracker> {
        self.health.get(&dataset_id)
    }

    /// Mark a chunk as lost for a dataset.
    pub fn mark_chunk_lost(&mut self, dataset_id: DatasetId, chunk_index: u32) {
        if let Some(tracker) = self.health.get_mut(&dataset_id) {
            tracker.health.insert(chunk_index, ChunkHealth::Lost);
        }
    }

    /// Mark a chunk as healthy for a dataset.
    pub fn mark_chunk_healthy(&mut self, dataset_id: DatasetId, chunk_index: u32) {
        if let Some(tracker) = self.health.get_mut(&dataset_id) {
            tracker.health.insert(chunk_index, ChunkHealth::Healthy);
        }
    }

    /// Count healthy chunks for a dataset.
    pub fn healthy_chunk_count(&self, dataset_id: DatasetId) -> u32 {
        self.health
            .get(&dataset_id)
            .map(|t| {
                t.health
                    .values()
                    .filter(|&&h| h == ChunkHealth::Healthy)
                    .count() as u32
            })
            .unwrap_or(0)
    }

    /// Schedule reconstruction of lost chunks for a dataset.
    pub fn schedule_reconstruction(
        &mut self,
        dataset_id: DatasetId,
    ) -> Result<ReconstructionSchedule, ErasureError> {
        let tracker = self
            .health
            .get(&dataset_id)
            .ok_or_else(|| ErasureError::DatasetNotFound { dataset: dataset_id })?;
        let params = self
            .params
            .get(&dataset_id)
            .ok_or_else(|| ErasureError::DatasetNotFound { dataset: dataset_id })?;

        let lost: Vec<u32> = tracker
            .health
            .iter()
            .filter(|(_, &h)| h != ChunkHealth::Healthy)
            .map(|(&i, _)| i)
            .collect();

        let available = tracker.total_chunks.saturating_sub(lost.len() as u32);
        if available < params.k {
            self.failed_reconstructions += 1;
            return Err(ErasureError::InsufficientChunks {
                available,
                required: params.k,
            });
        }

        let source: Vec<u32> = tracker
            .health
            .iter()
            .filter(|(_, &h)| h == ChunkHealth::Healthy)
            .map(|(&i, _)| i)
            .take(params.k as usize)
            .collect();

        let priority = if lost.len() > (params.n as usize).saturating_sub(params.k as usize) / 2 {
            ReconstructionPriority::Critical
        } else if !lost.is_empty() {
            ReconstructionPriority::High
        } else {
            ReconstructionPriority::Normal
        };

        let schedule = ReconstructionSchedule {
            dataset_id,
            chunks_to_reconstruct: lost.clone(),
            source_chunks: source,
            estimated_bandwidth_bytes: lost.len() as u64 * 1_048_576, // 1 MiB per chunk estimate
            priority,
        };

        self.schedules.insert(dataset_id, schedule.clone());
        self.reconstruction_count += 1;
        self.successful_reconstructions += 1;

        // Mark reconstructing chunks as recovering.
        for &ci in &lost {
            if let Some(t) = self.health.get_mut(&dataset_id) {
                t.health.insert(ci, ChunkHealth::Recovering);
            }
        }

        Ok(schedule)
    }

    /// Mark a reconstruction as complete; chunks become healthy.
    pub fn complete_reconstruction(&mut self, dataset_id: DatasetId) {
        if let Some(t) = self.health.get_mut(&dataset_id) {
            for h in t.health.values_mut() {
                if *h == ChunkHealth::Recovering {
                    *h = ChunkHealth::Healthy;
                }
            }
        }
        self.schedules.remove(&dataset_id);
    }

    /// Return total reconstruction operations.
    pub fn reconstruction_count(&self) -> u64 {
        self.reconstruction_count
    }

    /// Return successful reconstruction count.
    pub fn successful_reconstructions(&self) -> u64 {
        self.successful_reconstructions
    }

    /// Return failed reconstruction count.
    pub fn failed_reconstructions(&self) -> u64 {
        self.failed_reconstructions
    }

    /// Return default scheme.
    pub fn default_scheme(&self) -> CodingScheme {
        self.default_scheme
    }

    /// Return default k and n.
    pub fn default_params(&self) -> (u32, u32) {
        (self.default_k, self.default_n)
    }

    /// Compute capacity plan for a dataset.
    pub fn capacity_plan(
        &self,
        dataset_id: DatasetId,
        raw_data_bytes: u64,
    ) -> Result<ErasureCapacityPlan, ErasureError> {
        let p = self
            .params
            .get(&dataset_id)
            .ok_or_else(|| ErasureError::DatasetNotFound { dataset: dataset_id })?;
        let encoded_bytes = raw_data_bytes * p.n as u64 / p.k as u64;
        let overhead_bytes = encoded_bytes.saturating_sub(raw_data_bytes);
        ErasureCapacityPlan {
            dataset_id,
            raw_data_bytes,
            encoded_bytes,
            overhead_bytes,
            overhead_ratio: Self::storage_overhead_ratio(p.k, p.n),
            failure_tolerance: Self::failure_tolerance(p.n, p.k),
            chunks: p.n,
            data_chunks: p.k,
        }
    }

    /// Set default coding scheme.
    pub fn set_default_scheme(&mut self, scheme: CodingScheme) {
        self.default_scheme = scheme;
    }

    /// Set default k and n.
    pub fn set_default_params(&mut self, k: u32, n: u32) -> Result<(), ErasureError> {
        if k == 0 || n < k {
            return Err(ErasureError::InvalidParams {
                k,
                n,
                reason: "invalid default params".to_string(),
            });
        }
        self.default_k = k;
        self.default_n = n;
        Ok(())
    }
}

impl Default for ErasureThresholdManager {
    fn default() -> Self {
        Self::new()
    }
}

///////////////////////////////////////////////////////////////////////////////
// ReedSolomonCodec — pure mathematical parameter validation (no networking)
///////////////////////////////////////////////////////////////////////////////

/// Validates Reed-Solomon code parameters and computes field sizes.
pub struct ReedSolomonCodec;

impl ReedSolomonCodec {
    /// Validate that k and n are valid for a Reed-Solomon code over GF(2^m).
    pub fn validate_rs_params(k: u32, n: u32, m: u32) -> Result<(), ErasureError> {
        if k == 0 || n < k {
            return Err(ErasureError::InvalidParams {
                k,
                n,
                reason: "invalid RS params".to_string(),
            });
        }
        let max_symbols = (1u64 << m) - 1;
        if n as u64 > max_symbols {
            return Err(ErasureError::InvalidParams {
                k,
                n,
                reason: format!("n exceeds GF(2^{}) symbol count", m),
            });
        }
        Ok(())
    }

    /// Return the GF(2^m) symbol count maximum for n chunks.
    pub fn max_chunks_for_field(m: u32) -> u32 {
        if m >= 31 {
            return u32::MAX;
        }
        (1u32 << m).saturating_sub(1)
    }

    /// Compute the number of parity symbols for RS (n, k).
    pub fn parity_symbols(n: u32, k: u32) -> u32 {
        n.saturating_sub(k)
    }
}

///////////////////////////////////////////////////////////////////////////////
// ErasureCapacityPlanner — plans capacity across many datasets
///////////////////////////////////////////////////////////////////////////////

/// Aggregates capacity across multiple erasure-coded datasets.
pub struct ErasureCapacityPlanner {
    /// Per-dataset plans.
    plans: HashMap<DatasetId, ErasureCapacityPlan>,
}

impl ErasureCapacityPlanner {
    /// Create a new planner.
    pub fn new() -> Self {
        ErasureCapacityPlanner {
            plans: HashMap::new(),
        }
    }

    /// Add a capacity plan.
    pub fn add_plan(&mut self, plan: ErasureCapacityPlan) {
        self.plans.insert(plan.dataset_id, plan);
    }

    /// Return total raw data bytes across all datasets.
    pub fn total_raw_bytes(&self) -> u64 {
        self.plans.values().map(|p| p.raw_data_bytes).sum()
    }

    /// Return total encoded bytes across all datasets.
    pub fn total_encoded_bytes(&self) -> u64 {
        self.plans.values().map(|p| p.encoded_bytes).sum()
    }

    /// Return total overhead bytes across all datasets.
    pub fn total_overhead_bytes(&self) -> u64 {
        self.plans.values().map(|p| p.overhead_bytes).sum()
    }

    /// Return the aggregate overhead ratio.
    pub fn aggregate_overhead_ratio(&self) -> f64 {
        let raw = self.total_raw_bytes();
        if raw == 0 {
            return 0.0;
        }
        self.total_encoded_bytes() as f64 / raw as f64
    }

    /// Return the dataset with the largest overhead ratio.
    pub fn highest_overhead_dataset(&self) -> Option<DatasetId> {
        self.plans
            .iter()
            .max_by(|a, b| a.1.overhead_ratio.partial_cmp(&b.1.overhead_ratio).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(id, _)| *id)
    }

    /// Return the total failure tolerance across all datasets (minimum).
    pub fn min_failure_tolerance(&self) -> u32 {
        self.plans
            .values()
            .map(|p| p.failure_tolerance)
            .min()
            .unwrap_or(0)
    }

    /// Return the number of planned datasets.
    pub fn dataset_count(&self) -> usize {
        self.plans.len()
    }

    /// Return the plan for a specific dataset.
    pub fn plan_for_dataset(&self, dataset_id: DatasetId) -> Option<&ErasureCapacityPlan> {
        self.plans.get(&dataset_id)
    }

    /// Remove a plan for a dataset.
    pub fn remove_plan(&mut self, dataset_id: DatasetId) -> Option<ErasureCapacityPlan> {
        self.plans.remove(&dataset_id)
    }

    /// Return all plans sorted by overhead ratio descending.
    pub fn plans_by_overhead(&self) -> Vec<&ErasureCapacityPlan> {
        let mut plans: Vec<&ErasureCapacityPlan> = self.plans.values().collect();
        plans.sort_by(|a, b| {
            b.overhead_ratio
                .partial_cmp(&a.overhead_ratio)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        plans
    }

    /// Return datasets exceeding an overhead threshold.
    pub fn datasets_over_overhead(&self, threshold: f64) -> Vec<DatasetId> {
        self.plans
            .iter()
            .filter(|(_, p)| p.overhead_ratio > threshold)
            .map(|(id, _)| *id)
            .collect()
    }
}

///////////////////////////////////////////////////////////////////////////////
// ErasureStripingManager — stripe management across chunks
///////////////////////////////////////////////////////////////////////////////

/// A stripe of encoded chunks for a dataset.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ErasureStripe {
    pub stripe_id: StripeId,
    pub dataset_id: DatasetId,
    pub chunk_indices: Vec<u32>,
    pub stripe_size_bytes: u64,
    pub parity_chunk_count: u32,
    pub data_chunk_count: u32,
}

/// Unique identifier for a stripe.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct StripeId(pub u64);

/// Manages striping of data across erasure-coded chunks.
pub struct ErasureStripingManager {
    /// Stripes by ID.
    stripes: HashMap<StripeId, ErasureStripe>,
    /// Per-dataset stripe IDs.
    dataset_stripes: HashMap<DatasetId, Vec<StripeId>>,
    /// Next stripe ID.
    next_id: u64,
    /// Default stripe size in bytes.
    default_stripe_size: u64,
}

impl ErasureStripingManager {
    /// Create a new striping manager.
    pub fn new() -> Self {
        ErasureStripingManager {
            stripes: HashMap::new(),
            dataset_stripes: HashMap::new(),
            next_id: 0,
            default_stripe_size: 1_048_576, // 1 MiB
        }
    }

    /// Create a stripe for a dataset.
    pub fn create_stripe(
        &mut self,
        dataset_id: DatasetId,
        chunk_indices: Vec<u32>,
        stripe_size_bytes: u64,
        parity_chunk_count: u32,
        data_chunk_count: u32,
    ) -> StripeId {
        let id = StripeId(self.next_id);
        self.next_id += 1;
        let stripe = ErasureStripe {
            stripe_id: id,
            dataset_id,
            chunk_indices: chunk_indices.clone(),
            stripe_size_bytes,
            parity_chunk_count,
            data_chunk_count,
        };
        self.stripes.insert(id, stripe);
        self.dataset_stripes
            .entry(dataset_id)
            .or_default()
            .push(id);
        id
    }

    /// Return a stripe by ID.
    pub fn stripe(&self, stripe_id: StripeId) -> Option<&ErasureStripe> {
        self.stripes.get(&stripe_id)
    }

    /// Return all stripes for a dataset.
    pub fn stripes_for_dataset(&self, dataset_id: DatasetId) -> Vec<ErasureStripe> {
        self.dataset_stripes
            .get(&dataset_id)
            .map(|ids| {
                ids.iter()
                    .filter_map(|id| self.stripes.get(id).cloned())
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Return the total stripe count.
    pub fn stripe_count(&self) -> usize {
        self.stripes.len()
    }

    /// Return the total data bytes striped.
    pub fn total_striped_bytes(&self) -> u64 {
        self.stripes.values().map(|s| s.stripe_size_bytes).sum()
    }

    /// Return the default stripe size.
    pub fn default_stripe_size(&self) -> u64 {
        self.default_stripe_size
    }

    /// Set the default stripe size.
    pub fn set_default_stripe_size(&mut self, size: u64) {
        self.default_stripe_size = size;
    }

    /// Remove a stripe.
    pub fn remove_stripe(&mut self, stripe_id: StripeId) -> Option<ErasureStripe> {
        let stripe = self.stripes.remove(&stripe_id)?;
        if let Some(ids) = self.dataset_stripes.get_mut(&stripe.dataset_id) {
            ids.retain(|&id| id != stripe_id);
        }
        Some(stripe)
    }
}

///////////////////////////////////////////////////////////////////////////////
// ErasureRepairScheduler — repair scheduling with priorities
///////////////////////////////////////////////////////////////////////////////

/// A repair job for a lost chunk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepairJob {
    pub job_id: RepairJobId,
    pub dataset_id: DatasetId,
    pub lost_chunk_index: u32,
    pub helper_chunk_indices: Vec<u32>,
    pub priority: RepairPriority,
    pub status: RepairJobStatus,
    pub created_at: TimestampMillis,
    pub started_at: Option<TimestampMillis>,
    pub completed_at: Option<TimestampMillis>,
}

/// Unique identifier for a repair job.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct RepairJobId(pub u64);

/// Priority of a repair job.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum RepairPriority {
    Low,
    Normal,
    High,
    Critical,
}

/// Status of a repair job.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepairJobStatus {
    Queued,
    InProgress,
    Completed,
    Failed,
}

/// Schedules chunk repair jobs with priorities and concurrency limits.
pub struct ErasureRepairScheduler {
    /// Jobs by ID.
    jobs: HashMap<RepairJobId, RepairJob>,
    /// Per-dataset job IDs.
    dataset_jobs: HashMap<DatasetId, Vec<RepairJobId>>,
    /// Next job ID.
    next_id: u64,
    /// Maximum concurrent repairs.
    max_concurrent: usize,
    /// Total repair jobs created.
    total_jobs: u64,
    /// Total completed repairs.
    completed_repairs: u64,
}

impl ErasureRepairScheduler {
    /// Create a new repair scheduler.
    pub fn new() -> Self {
        ErasureRepairScheduler {
            jobs: HashMap::new(),
            dataset_jobs: HashMap::new(),
            next_id: 0,
            max_concurrent: 3,
            total_jobs: 0,
            completed_repairs: 0,
        }
    }

    /// Queue a repair job for a lost chunk.
    pub fn queue_repair(
        &mut self,
        dataset_id: DatasetId,
        lost_chunk: u32,
        helpers: Vec<u32>,
        priority: RepairPriority,
    ) -> RepairJobId {
        let id = RepairJobId(self.next_id);
        self.next_id += 1;
        self.total_jobs += 1;
        let job = RepairJob {
            job_id: id,
            dataset_id,
            lost_chunk_index: lost_chunk,
            helper_chunk_indices: helpers,
            priority,
            status: RepairJobStatus::Queued,
            created_at: TimestampMillis(0),
            started_at: None,
            completed_at: None,
        };
        self.jobs.insert(id, job);
        self.dataset_jobs
            .entry(dataset_id)
            .or_default()
            .push(id);
        id
    }

    /// Start the next queued job if capacity allows.
    pub fn start_next_job(&mut self) -> Option<RepairJobId> {
        let in_progress = self
            .jobs
            .values()
            .filter(|j| j.status == RepairJobStatus::InProgress)
            .count();
        if in_progress >= self.max_concurrent {
            return None;
        }
        // Find highest priority queued job.
        let mut best: Option<(RepairJobId, RepairPriority)> = None;
        for (id, job) in &self.jobs {
            if job.status == RepairJobStatus::Queued {
                let candidate = (*id, job.priority);
                match best {
                    None => best = Some(candidate),
                    Some((_, bp)) => {
                        if candidate.1 > bp {
                            best = Some(candidate);
                        }
                    }
                }
            }
        }
        if let Some((id, _)) = best {
            if let Some(job) = self.jobs.get_mut(&id) {
                job.status = RepairJobStatus::InProgress;
                job.started_at = Some(TimestampMillis(0));
            }
            Some(id)
        } else {
            None
        }
    }

    /// Complete a repair job.
    pub fn complete_job(&mut self, job_id: RepairJobId) -> Result<(), ErasureError> {
        let job = self
            .jobs
            .get_mut(&job_id)
            .ok_or_else(|| ErasureError::Internal {
                message: "repair job not found".to_string(),
            })?;
        job.status = RepairJobStatus::Completed;
        job.completed_at = Some(TimestampMillis(0));
        self.completed_repairs += 1;
        Ok(())
    }

    /// Fail a repair job.
    pub fn fail_job(&mut self, job_id: RepairJobId, reason: String) -> Result<(), ErasureError> {
        let job = self
            .jobs
            .get_mut(&job_id)
            .ok_or_else(|| ErasureError::Internal {
                message: "repair job not found".to_string(),
            })?;
        job.status = RepairJobStatus::Failed;
        let _ = reason;
        Ok(())
    }

    /// Return jobs for a dataset.
    pub fn jobs_for_dataset(&self, dataset_id: DatasetId) -> Vec<RepairJob> {
        self.dataset_jobs
            .get(&dataset_id)
            .map(|ids| {
                ids.iter()
                    .filter_map(|id| self.jobs.get(id).cloned())
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Return queued job count.
    pub fn queued_count(&self) -> u64 {
        self.jobs
            .values()
            .filter(|j| j.status == RepairJobStatus::Queued)
            .count() as u64
    }

    /// Return in-progress job count.
    pub fn in_progress_count(&self) -> u64 {
        self.jobs
            .values()
            .filter(|j| j.status == RepairJobStatus::InProgress)
            .count() as u64
    }

    /// Return total jobs.
    pub fn total_jobs(&self) -> u64 {
        self.total_jobs
    }

    /// Return completed repairs.
    pub fn completed_repairs(&self) -> u64 {
        self.completed_repairs
    }

    /// Set max concurrent repairs.
    pub fn set_max_concurrent(&mut self, max: usize) {
        self.max_concurrent = max;
    }
}

///////////////////////////////////////////////////////////////////////////////
// ErasureTopologyAnalyzer — topology analysis for placement
///////////////////////////////////////////////////////////////////////////////

/// Analysis of placement topology for erasure chunks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ErasureTopologyAnalysis {
    pub total_chunks: u32,
    pub domains_used: u32,
    pub max_chunks_per_domain: u32,
    pub min_chunks_per_domain: u32,
    pub balanced: bool,
    pub diversity_score: f64,
}

/// Analyzes erasure chunk placement topology for durability.
pub struct ErasureTopologyAnalyzer;

impl ErasureTopologyAnalyzer {
    /// Create a new analyzer.
    pub fn new() -> Self {
        ErasureTopologyAnalyzer
    }

    /// Analyze a chunk placement across failure domains.
    pub fn analyze_placement(
        &self,
        placement: &[ChunkPlacement],
    ) -> ErasureTopologyAnalysis {
        let total_chunks = placement.len() as u32;
        let mut domain_counts: BTreeMap<FailureDomainRef, u32> = BTreeMap::new();
        for p in placement {
            *domain_counts.entry(p.domain).or_insert(0) += 1;
        }
        let domains_used = domain_counts.len() as u32;
        let counts: Vec<u32> = domain_counts.values().copied().collect();
        let max_chunks = counts.iter().copied().max().unwrap_or(0);
        let min_chunks = counts.iter().copied().min().unwrap_or(0);
        let balanced = max_chunks.saturating_sub(min_chunks) <= 1;
        // Diversity score: 1.0 = perfectly distributed, 0.0 = all in one domain.
        let diversity_score = if total_chunks > 0 {
            domains_used as f64 / total_chunks as f64
        } else {
            0.0
        };
        ErasureTopologyAnalysis {
            total_chunks,
            domains_used,
            max_chunks_per_domain: max_chunks,
            min_chunks_per_domain: min_chunks,
            balanced,
            diversity_score,
        }
    }

    /// Return whether a topology is resilient (balanced + diverse).
    pub fn is_resilient(analysis: &ErasureTopologyAnalysis) -> bool {
        analysis.balanced && analysis.diversity_score > 0.5
    }

    /// Return the recommended min domains for n chunks with tolerance f.
    pub fn recommended_min_domains(n: u32, f: u32) -> u32 {
        (f + 1).max(2).min(n)
    }
}

///////////////////////////////////////////////////////////////////////////////
// ErasureIntegrityAuditor — audits erasure integrity over time
///////////////////////////////////////////////////////////////////////////////

/// An audit entry for erasure integrity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ErasureIntegrityAudit {
    pub audit_id: AuditEntryId,
    pub dataset_id: DatasetId,
    pub chunks_checked: u32,
    pub chunks_corrupt: u32,
    pub chunks_repaired: u32,
    pub audit_timestamp: TimestampMillis,
    pub result: AuditResult,
}

/// Result of an integrity audit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuditResult {
    Pass,
    Partial,
    Fail,
}

/// Audits erasure-coded data integrity over time.
pub struct ErasureIntegrityAuditor {
    /// Audit entries.
    audits: Vec<ErasureIntegrityAudit>,
    /// Next audit ID.
    next_id: u64,
    /// Total audits performed.
    total_audits: u64,
    /// Total failed audits.
    failed_audits: u64,
}

impl ErasureIntegrityAuditor {
    /// Create a new auditor.
    pub fn new() -> Self {
        ErasureIntegrityAuditor {
            audits: Vec::new(),
            next_id: 0,
            total_audits: 0,
            failed_audits: 0,
        }
    }

    /// Record an integrity audit.
    pub fn record_audit(
        &mut self,
        dataset_id: DatasetId,
        chunks_checked: u32,
        chunks_corrupt: u32,
        chunks_repaired: u32,
    ) -> AuditEntryId {
        let id = AuditEntryId(self.next_id);
        self.next_id += 1;
        self.total_audits += 1;
        let result = if chunks_corrupt == 0 {
            AuditResult::Pass
        } else if chunks_repaired >= chunks_corrupt {
            AuditResult::Partial
        } else {
            AuditResult::Fail
        };
        if result == AuditResult::Fail {
            self.failed_audits += 1;
        }
        let audit = ErasureIntegrityAudit {
            audit_id: id,
            dataset_id,
            chunks_checked,
            chunks_corrupt,
            chunks_repaired,
            audit_timestamp: TimestampMillis(0),
            result,
        };
        self.audits.push(audit);
        id
    }

    /// Return all audits for a dataset.
    pub fn audits_for_dataset(&self, dataset_id: DatasetId) -> Vec<ErasureIntegrityAudit> {
        self.audits
            .iter()
            .filter(|a| a.dataset_id == dataset_id)
            .cloned()
            .collect()
    }

    /// Return total audits.
    pub fn total_audits(&self) -> u64 {
        self.total_audits
    }

    /// Return failed audit count.
    pub fn failed_audits(&self) -> u64 {
        self.failed_audits
    }

    /// Return the audit pass rate.
    pub fn pass_rate(&self) -> f64 {
        if self.total_audits == 0 {
            return 1.0;
        }
        (self.total_audits - self.failed_audits) as f64 / self.total_audits as f64
    }

    /// Return the most recent audit.
    pub fn latest_audit(&self) -> Option<&ErasureIntegrityAudit> {
        self.audits.last()
    }
}

///////////////////////////////////////////////////////////////////////////////
// Version bit for this invariant
///////////////////////////////////////////////////////////////////////////////

/// Detailed validation result for erasure coding parameters.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ErasureParameterValidationResult {
    pub valid: bool,
    pub errors: Vec<ErasureParameterError>,
    pub warnings: Vec<ErasureParameterWarning>,
    pub recommended_k: u32,
    pub recommended_n: u32,
}

/// Errors found during erasure parameter validation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErasureParameterError {
    KZero,
    NLessThanK,
    NExceedsMaximum { n: u32, max: u32 },
    FieldSizeTooSmall { field_bits: u32, needed: u32 },
    ParityChunkCountExceedsData { parity: u32, data: u32 },
}

/// Warnings found during erasure parameter validation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErasureParameterWarning {
    LowFaultTolerance { tolerance: u32 },
    HighStorageOverhead { ratio: f64 },
    LargeParityFraction { parity_fraction: f64 },
}

/// Validates erasure coding parameters in depth.
pub struct ErasureCodeParametersValidator {
    /// Maximum allowed n.
    max_n: u32,
    /// Minimum allowed k.
    min_k: u32,
    /// Default Galois field bits.
    default_field_bits: u32,
}

impl ErasureCodeParametersValidator {
    /// Create a new validator with default limits.
    pub fn new() -> Self {
        ErasureCodeParametersValidator {
            max_n: 1024,
            min_k: 1,
            default_field_bits: 8,
        }
    }

    /// Create a validator with custom limits.
    pub fn with_limits(max_n: u32, min_k: u32, field_bits: u32) -> Self {
        ErasureCodeParametersValidator {
            max_n,
            min_k,
            default_field_bits: field_bits,
        }
    }

    /// Validate parameters comprehensively.
    pub fn validate(
        &self,
        k: u32,
        n: u32,
        field_bits: u32,
    ) -> ErasureParameterValidationResult {
        let mut errors = Vec::new();
        let mut warnings = Vec::new();

        if k == 0 {
            errors.push(ErasureParameterError::KZero);
        }
        if k < self.min_k {
            errors.push(ErasureParameterError::KZero); // treat as k too small
        }
        if n < k {
            errors.push(ErasureParameterError::NLessThanK);
        }
        if n > self.max_n {
            errors.push(ErasureParameterError::NExceedsMaximum {
                n,
                max: self.max_n,
            });
        }
        let max_symbols = ReedSolomonCodec::max_chunks_for_field(field_bits);
        if n > max_symbols {
            errors.push(ErasureParameterError::FieldSizeTooSmall {
                field_bits,
                needed: (n as f64).log2().ceil() as u32 + 1,
            });
        }
        let parity = n.saturating_sub(k);
        if parity > k {
            errors.push(ErasureParameterError::ParityChunkCountExceedsData {
                parity,
                data: k,
            });
        }

        let tolerance = n.saturating_sub(k);
        if tolerance < 2 {
            warnings.push(ErasureParameterWarning::LowFaultTolerance {
                tolerance,
            });
        }
        let ratio = if k > 0 { n as f64 / k as f64 } else { 0.0 };
        if ratio > 3.0 {
            warnings.push(ErasureParameterWarning::HighStorageOverhead { ratio });
        }
        if k > 0 {
            let parity_fraction = parity as f64 / n as f64;
            if parity_fraction > 0.5 {
                warnings.push(ErasureParameterWarning::LargeParityFraction {
                    parity_fraction,
                });
            }
        }

        let recommended_k = if k == 0 { self.min_k } else { k };
        let recommended_n = (recommended_k + 2).max(n).min(self.max_n);

        let valid = errors.is_empty();
        ErasureParameterValidationResult {
            valid,
            errors,
            warnings,
            recommended_k,
            recommended_n,
        }
    }

    /// Return whether parameters are valid.
    pub fn is_valid(&self, k: u32, n: u32, field_bits: u32) -> bool {
        self.validate(k, n, field_bits).valid
    }

    /// Return the maximum n allowed.
    pub fn max_n(&self) -> u32 {
        self.max_n
    }

    /// Return the minimum k allowed.
    pub fn min_k(&self) -> u32 {
        self.min_k
    }

    /// Set the maximum n.
    pub fn set_max_n(&mut self, max_n: u32) {
        self.max_n = max_n;
    }

    /// Set the minimum k.
    pub fn set_min_k(&mut self, min_k: u32) {
        self.min_k = min_k;
    }
}

///////////////////////////////////////////////////////////////////////////////
// ParityMatrixGenerator — conceptual parity matrix math
///////////////////////////////////////////////////////////////////////////////

/// A parity-check matrix row for erasure coding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParityMatrixRow {
    pub row_index: u32,
    pub coefficients: Vec<u32>,
}

/// Generates parity-check matrices for erasure codes (conceptual, no networking).
pub struct ParityMatrixGenerator;

impl ParityMatrixGenerator {
    /// Create a new generator.
    pub fn new() -> Self {
        ParityMatrixGenerator
    }

    /// Generate a systematic parity-check matrix of size (n-k) x n.
    pub fn generate_systematic_parity_matrix(
        &self,
        k: u32,
        n: u32,
    ) -> Result<Vec<ParityMatrixRow>, ErasureError> {
        if k == 0 || n < k {
            return Err(ErasureError::InvalidParams {
                k,
                n,
                reason: "invalid params for parity matrix".to_string(),
            });
        }
        let parity_count = n - k;
        let mut rows = Vec::new();
        for i in 0..parity_count {
            let mut coefficients = Vec::new();
            for j in 0..n {
                // Simple systematic parity: identity in parity portion, 
                // pseudo-random in data portion.
                let coeff = if j >= k {
                    if (j - k) == i {
                        1
                    } else {
                        0
                    }
                } else {
                    ((i * 7 + j * 3 + 1) % 256) as u32
                };
                coefficients.push(coeff);
            }
            rows.push(ParityMatrixRow {
                row_index: i,
                coefficients,
            });
        }
        Ok(rows)
    }

    /// Verify that a parity matrix has full row rank (conceptual check).
    pub fn verify_row_rank(&self, rows: &[ParityMatrixRow]) -> bool {
        if rows.is_empty() {
            return true;
        }
        let width = rows[0].coefficients.len();
        let mut pivot_rows = 0;
        for col in 0..width {
            let mut found = None;
            for r in pivot_rows..rows.len() {
                if rows[r].coefficients[col] != 0 {
                    found = Some(r);
                    break;
                }
            }
            if let Some(r) = found {
                pivot_rows += 1;
                if pivot_rows == rows.len() {
                    break;
                }
            }
        }
        pivot_rows == rows.len()
    }

    /// Return the number of parity rows for (n, k).
    pub fn parity_row_count(k: u32, n: u32) -> u32 {
        n.saturating_sub(k)
    }
}

///////////////////////////////////////////////////////////////////////////////
// FragmentRecoveryCoordinator — coordinates fragment recovery
///////////////////////////////////////////////////////////////////////////////

/// A fragment recovery task.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FragmentRecoveryTask {
    pub task_id: RecoveryTaskId,
    pub dataset_id: DatasetId,
    pub missing_chunk_indices: Vec<u32>,
    pub source_chunk_indices: Vec<u32>,
    pub status: RecoveryTaskStatus,
    pub created_at: TimestampMillis,
    pub completed_at: Option<TimestampMillis>,
}

/// Unique identifier for a recovery task.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct RecoveryTaskId(pub u64);

/// Status of a recovery task.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecoveryTaskStatus {
    Pending,
    InProgress,
    Completed,
    Failed,
    Cancelled,
}

/// Coordinates fragment recovery across multiple datasets.
pub struct FragmentRecoveryCoordinator {
    /// Tasks by ID.
    tasks: HashMap<RecoveryTaskId, FragmentRecoveryTask>,
    /// Per-dataset pending task IDs.
    dataset_tasks: HashMap<DatasetId, Vec<RecoveryTaskId>>,
    /// Next task ID.
    next_id: u64,
    /// Maximum concurrent recovery tasks.
    max_concurrent: usize,
}

impl FragmentRecoveryCoordinator {
    /// Create a new coordinator.
    pub fn new() -> Self {
        FragmentRecoveryCoordinator {
            tasks: HashMap::new(),
            dataset_tasks: HashMap::new(),
            next_id: 0,
            max_concurrent: 5,
        }
    }

    /// Create a recovery task for missing chunks.
    pub fn create_recovery_task(
        &mut self,
        dataset_id: DatasetId,
        missing: Vec<u32>,
        source: Vec<u32>,
    ) -> RecoveryTaskId {
        let id = RecoveryTaskId(self.next_id);
        self.next_id += 1;
        let task = FragmentRecoveryTask {
            task_id: id,
            dataset_id,
            missing_chunk_indices: missing.clone(),
            source_chunk_indices: source,
            status: RecoveryTaskStatus::Pending,
            created_at: TimestampMillis(0),
            completed_at: None,
        };
        self.tasks.insert(id, task);
        self.dataset_tasks
            .entry(dataset_id)
            .or_default()
            .push(id);
        id
    }

    /// Start a recovery task (mark in progress).
    pub fn start_recovery_task(&mut self, task_id: RecoveryTaskId) -> Result<(), ErasureError> {
        let task = self
            .tasks
            .get_mut(&task_id)
            .ok_or_else(|| ErasureError::Internal {
                message: "recovery task not found".to_string(),
            })?;
        if task.status != RecoveryTaskStatus::Pending {
            return Err(ErasureError::Internal {
                message: "recovery task not pending".to_string(),
            });
        }
        let in_progress = self
            .tasks
            .values()
            .filter(|t| t.status == RecoveryTaskStatus::InProgress)
            .count();
        if in_progress >= self.max_concurrent {
            return Err(ErasureError::Internal {
                message: "max concurrent recoveries reached".to_string(),
            });
        }
        task.status = RecoveryTaskStatus::InProgress;
        Ok(())
    }

    /// Complete a recovery task.
    pub fn complete_recovery_task(
        &mut self,
        task_id: RecoveryTaskId,
    ) -> Result<(), ErasureError> {
        let task = self
            .tasks
            .get_mut(&task_id)
            .ok_or_else(|| ErasureError::Internal {
                message: "recovery task not found".to_string(),
            })?;
        task.status = RecoveryTaskStatus::Completed;
        task.completed_at = Some(TimestampMillis(0));
        Ok(())
    }

    /// Fail a recovery task.
    pub fn fail_recovery_task(&mut self, task_id: RecoveryTaskId) -> Result<(), ErasureError> {
        let task = self
            .tasks
            .get_mut(&task_id)
            .ok_or_else(|| ErasureError::Internal {
                message: "recovery task not found".to_string(),
            })?;
        task.status = RecoveryTaskStatus::Failed;
        Ok(())
    }

    /// Return tasks for a dataset.
    pub fn tasks_for_dataset(&self, dataset_id: DatasetId) -> Vec<FragmentRecoveryTask> {
        self.dataset_tasks
            .get(&dataset_id)
            .map(|ids| {
                ids.iter()
                    .filter_map(|id| self.tasks.get(id).cloned())
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Return pending tasks.
    pub fn pending_tasks(&self) -> Vec<FragmentRecoveryTask> {
        self.tasks
            .values()
            .filter(|t| t.status == RecoveryTaskStatus::Pending)
            .cloned()
            .collect()
    }

    /// Return completed task count.
    pub fn completed_count(&self) -> u64 {
        self.tasks
            .values()
            .filter(|t| t.status == RecoveryTaskStatus::Completed)
            .count() as u64
    }

    /// Return failed task count.
    pub fn failed_count(&self) -> u64 {
        self.tasks
            .values()
            .filter(|t| t.status == RecoveryTaskStatus::Failed)
            .count() as u64
    }

    /// Set max concurrent recoveries.
    pub fn set_max_concurrent(&mut self, max: usize) {
        self.max_concurrent = max;
    }
}

///////////////////////////////////////////////////////////////////////////////
// ChunkLossPatternAnalyzer — analyzes chunk loss patterns
///////////////////////////////////////////////////////////////////////////////

/// Analysis result for a chunk loss pattern.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChunkLossAnalysis {
    pub total_chunks: u32,
    pub lost_chunks: u32,
    pub surviving_chunks: u32,
    pub survivable: bool,
    pub loss_by_domain: BTreeMap<FailureDomainRef, u32>,
    pub worst_domain_loss: u32,
    pub recommended_action: RecoveryAction,
}

/// Recommended recovery action.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecoveryAction {
    NoAction,
    ReconstructFromParity,
    ReconstructFromReplicas,
    DeclareDataLoss,
}

/// Analyzes chunk loss patterns for survivability.
pub struct ChunkLossPatternAnalyzer {
    /// Failure domain assignments per chunk index.
    chunk_domains: HashMap<u32, FailureDomainRef>,
}

impl ChunkLossPatternAnalyzer {
    /// Create a new analyzer.
    pub fn new() -> Self {
        ChunkLossPatternAnalyzer {
            chunk_domains: HashMap::new(),
        }
    }

    /// Assign a chunk to a failure domain.
    pub fn assign_chunk_domain(
        &mut self,
        chunk_index: u32,
        domain: FailureDomainRef,
    ) {
        self.chunk_domains.insert(chunk_index, domain);
    }

    /// Analyze a loss pattern.
    pub fn analyze_loss(
        &self,
        total_chunks: u32,
        lost_indices: &[u32],
        k: u32,
    ) -> ChunkLossAnalysis {
        let lost_count = lost_indices.len() as u32;
        let surviving = total_chunks.saturating_sub(lost_count);
        let survivable = surviving >= k;

        let mut loss_by_domain: BTreeMap<FailureDomainRef, u32> = BTreeMap::new();
        for &idx in lost_indices {
            if let Some(domain) = self.chunk_domains.get(&idx) {
                *loss_by_domain.entry(*domain).or_insert(0) += 1;
            }
        }

        let worst_domain_loss = loss_by_domain
            .values()
            .copied()
            .max()
            .unwrap_or(0);

        let recommended_action = if survivable {
            RecoveryAction::ReconstructFromParity
        } else if worst_domain_loss > total_chunks / 2 {
            RecoveryAction::DeclareDataLoss
        } else {
            RecoveryAction::ReconstructFromReplicas
        };

        ChunkLossAnalysis {
            total_chunks,
            lost_chunks: lost_count,
            surviving_chunks: surviving,
            survivable,
            loss_by_domain,
            worst_domain_loss,
            recommended_action,
        }
    }

    /// Return the domain assignment for a chunk.
    pub fn chunk_domain(&self, chunk_index: u32) -> Option<FailureDomainRef> {
        self.chunk_domains.get(&chunk_index).copied()
    }

    /// Return the number of tracked chunks.
    pub fn tracked_chunk_count(&self) -> usize {
        self.chunk_domains.len()
    }
}

///////////////////////////////////////////////////////////////////////////////
// ErasureTopologyConstraint — topology-aware placement constraints
///////////////////////////////////////////////////////////////////////////////

/// A topology constraint for chunk placement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ErasureTopologyConstraint {
    /// Maximum chunks allowed in a single failure domain.
    pub max_chunks_per_domain: u32,
    /// Minimum number of failure domains to span.
    pub min_domains: u32,
    /// Whether chunks must span regions.
    pub require_region_diversity: bool,
}

/// Checks chunk placement against topology constraints.
pub struct ErasureTopologyConstraintChecker;

impl ErasureTopologyConstraintChecker {
    /// Create a new checker.
    pub fn new() -> Self {
        ErasureTopologyConstraintChecker
    }

    /// Check whether a placement satisfies constraints.
    pub fn check_placement(
        &self,
        placement: &[ChunkPlacement],
        constraint: &ErasureTopologyConstraint,
    ) -> bool {
        if placement.is_empty() {
            return false;
        }
        let mut domain_counts: BTreeMap<FailureDomainRef, u32> = BTreeMap::new();
        for p in placement {
            *domain_counts.entry(p.domain).or_insert(0) += 1;
        }
        // Max per domain.
        for count in domain_counts.values() {
            if *count > constraint.max_chunks_per_domain {
                return false;
            }
        }
        // Min domains.
        if (domain_counts.len() as u32) < constraint.min_domains {
            return false;
        }
        true
    }

    /// Return default constraints.
    pub fn default_constraints() -> ErasureTopologyConstraint {
        ErasureTopologyConstraint {
            max_chunks_per_domain: 3,
            min_domains: 3,
            require_region_diversity: true,
        }
    }
}

///////////////////////////////////////////////////////////////////////////////
// DecodingScheduleBuilder — builds decoding schedules
///////////////////////////////////////////////////////////////////////////////

/// A decoding step in a schedule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodingStep {
    pub step_index: u32,
    pub target_chunk: u32,
    pub source_chunks: Vec<u32>,
    pub coefficient: u32,
}

/// A decoding schedule for reconstructing data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodingSchedule {
    pub dataset_id: DatasetId,
    pub target_chunks: Vec<u32>,
    pub steps: Vec<DecodingStep>,
    pub estimated_operations: u64,
}

/// Builds decoding schedules for erasure-coded data.
pub struct DecodingScheduleBuilder;

impl DecodingScheduleBuilder {
    /// Create a new builder.
    pub fn new() -> Self {
        DecodingScheduleBuilder
    }

    /// Build a schedule to reconstruct target chunks from sources.
    pub fn build(
        &self,
        dataset_id: DatasetId,
        targets: &[u32],
        sources: &[u32],
    ) -> DecodingSchedule {
        let mut steps = Vec::new();
        for (i, &target) in targets.iter().enumerate() {
            let source_subset: Vec<u32> = sources
                .iter()
                .copied()
                .take(4.min(sources.len()))
                .collect();
            steps.push(DecodingStep {
                step_index: i as u32,
                target_chunk: target,
                source_chunks: source_subset,
                coefficient: 1,
            });
        }
        DecodingSchedule {
            dataset_id,
            target_chunks: targets.to_vec(),
            steps,
            estimated_operations: steps.len() as u64,
        }
    }

    /// Return the number of steps for a schedule.
    pub fn step_count(schedule: &DecodingSchedule) -> usize {
        schedule.steps.len()
    }
}

///////////////////////////////////////////////////////////////////////////////
// IntegrityVerifier — verifies reconstructed data integrity
///////////////////////////////////////////////////////////////////////////////

/// Result of an integrity verification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntegrityVerificationResult {
    pub dataset_id: DatasetId,
    pub chunk_index: u32,
    pub checksum_match: bool,
    pub verified: bool,
}

/// Verifies integrity of reconstructed erasure chunks.
pub struct IntegrityVerifier {
    /// Expected checksums per chunk.
    expected_checksums: HashMap<(DatasetId, u32), u32>,
    /// Total verifications performed.
    verification_count: u64,
    /// Total failures.
    failure_count: u64,
}

impl IntegrityVerifier {
    /// Create a new verifier.
    pub fn new() -> Self {
        IntegrityVerifier {
            expected_checksums: HashMap::new(),
            verification_count: 0,
            failure_count: 0,
        }
    }

    /// Register an expected checksum for a chunk.
    pub fn register_checksum(
        &mut self,
        dataset_id: DatasetId,
        chunk_index: u32,
        checksum: u32,
    ) {
        self.expected_checksums
            .insert((dataset_id, chunk_index), checksum);
    }

    /// Verify a chunk's checksum.
    pub fn verify_chunk(
        &mut self,
        dataset_id: DatasetId,
        chunk_index: u32,
        actual_checksum: u32,
    ) -> IntegrityVerificationResult {
        self.verification_count += 1;
        let expected = self
            .expected_checksums
            .get(&(dataset_id, chunk_index))
            .copied()
            .unwrap_or(0);
        let checksum_match = expected == actual_checksum;
        let verified = checksum_match;
        if !verified {
            self.failure_count += 1;
        }
        IntegrityVerificationResult {
            dataset_id,
            chunk_index,
            checksum_match,
            verified,
        }
    }

    /// Return verification count.
    pub fn verification_count(&self) -> u64 {
        self.verification_count
    }

    /// Return failure count.
    pub fn failure_count(&self) -> u64 {
        self.failure_count
    }

    /// Return failure rate.
    pub fn failure_rate(&self) -> f64 {
        if self.verification_count == 0 {
            return 0.0;
        }
        self.failure_count as f64 / self.verification_count as f64
    }
}

///////////////////////////////////////////////////////////////////////////////
// CodingSchemeSelector — auto-selects coding scheme
///////////////////////////////////////////////////////////////////////////////

/// Selects an appropriate coding scheme based on constraints.
pub struct CodingSchemeSelector;

impl CodingSchemeSelector {
    /// Select a scheme based on failure tolerance and overhead budget.
    pub fn select_scheme(
        &self,
        required_tolerance: u32,
        max_overhead_ratio: f64,
        min_locality: bool,
    ) -> CodingScheme {
        if min_locality && required_tolerance <= 2 {
            return CodingScheme::LocallyRepairable;
        }
        if required_tolerance >= 5 {
            return CodingScheme::Regenerating;
        }
        if max_overhead_ratio < 2.0 {
            return CodingScheme::XorParity;
        }
        CodingScheme::SystematicReedSolomon
    }

    /// Return a description of a scheme.
    pub fn scheme_description(scheme: CodingScheme) -> &'static str {
        match scheme {
            CodingScheme::ReedSolomon => "Reed-Solomon maximum distance separable code",
            CodingScheme::SystematicReedSolomon => "Systematic Reed-Solomon with verbatim data chunks",
            CodingScheme::LocallyRepairable => "Locally repairable code with locality groups",
            CodingScheme::Regenerating => "Regenerating code for bandwidth-efficient repair",
            CodingScheme::XorParity => "XOR-based parity (RAID-like)",
            CodingScheme::Hybrid => "Hybrid systematic RS with local parity groups",
        }
    }
}

///////////////////////////////////////////////////////////////////////////////
// Version bit for this invariant
///////////////////////////////////////////////////////////////////////////////

/// The EdgeCaseBits bit position for ErasureThreshold.
pub const ERASURE_THRESHOLD_BIT: u128 = 1 << 8;

/// Check whether the erasure threshold edge case is enabled in the given bits.
pub fn erasure_threshold_enabled(bits: &EdgeCaseBits) -> bool {
    bits.has(ERASURE_THRESHOLD_BIT)
}

///////////////////////////////////////////////////////////////////////////////
// Tests
///////////////////////////////////////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use super::*;

    // ---- ErasureParams tests ----

    #[test]
    fn erasure_params_construction() {
        let p = ErasureParams {
            k: 4,
            n: 10,
            scheme: CodingScheme::SystematicReedSolomon,
        };
        assert_eq!(p.k, 4);
        assert_eq!(p.n, 10);
        assert_eq!(p.scheme, CodingScheme::SystematicReedSolomon);
    }

    // ---- CodingScheme tests ----

    #[test]
    fn coding_scheme_variants() {
        let schemes = [
            CodingScheme::ReedSolomon,
            CodingScheme::SystematicReedSolomon,
            CodingScheme::LocallyRepairable,
            CodingScheme::Regenerating,
            CodingScheme::XorParity,
            CodingScheme::Hybrid,
        ];
        for s in schemes {
            let _ = format!("{:?}", s);
        }
    }

    #[test]
    fn is_systematic_checks() {
        assert!(ErasureThresholdManager::is_systematic(
            4,
            10,
            CodingScheme::SystematicReedSolomon
        ));
        assert!(ErasureThresholdManager::is_systematic(4, 10, CodingScheme::XorParity));
        assert!(ErasureThresholdManager::is_systematic(4, 10, CodingScheme::Hybrid));
        assert!(!ErasureThresholdManager::is_systematic(4, 10, CodingScheme::ReedSolomon));
        assert!(!ErasureThresholdManager::is_systematic(4, 10, CodingScheme::LocallyRepairable));
    }

    // ---- ErasureThresholdManager default ----

    #[test]
    fn manager_default_creation() {
        let mgr = ErasureThresholdManager::default();
        assert_eq!(mgr.default_scheme(), CodingScheme::SystematicReedSolomon);
        assert_eq!(mgr.default_params(), (4, 7));
        assert_eq!(mgr.reconstruction_count(), 0);
        assert_eq!(mgr.successful_reconstructions(), 0);
        assert_eq!(mgr.failed_reconstructions(), 0);
    }

    #[test]
    fn manager_with_defaults_valid() {
        let mgr = ErasureThresholdManager::with_defaults(CodingScheme::ReedSolomon, 8, 16);
        assert!(mgr.is_ok());
        let m = mgr.unwrap();
        assert_eq!(m.default_scheme(), CodingScheme::ReedSolomon);
        assert_eq!(m.default_params(), (8, 16));
    }

    #[test]
    fn manager_with_defaults_invalid() {
        let result = ErasureThresholdManager::with_defaults(CodingScheme::ReedSolomon, 0, 5);
        assert!(result.is_err());
        let result = ErasureThresholdManager::with_defaults(CodingScheme::ReedSolomon, 10, 5);
        assert!(result.is_err());
    }

    // ---- register_coding_params ----

    #[test]
    fn register_coding_params_valid() {
        let mut mgr = ErasureThresholdManager::new();
        let dataset = DatasetId::new([1u8; 32]);
        let result = mgr.register_coding_params(dataset, 4, 10, CodingScheme::ReedSolomon);
        assert!(result.is_ok());
        assert!(mgr.is_erasure_enabled(dataset));
        let params = mgr.coding_params(dataset).unwrap();
        assert_eq!(params, (4, 10));
    }

    #[test]
    fn register_coding_params_invalid_k_zero() {
        let mut mgr = ErasureThresholdManager::new();
        let dataset = DatasetId::new([1u8; 32]);
        let result = mgr.register_coding_params(dataset, 0, 5, CodingScheme::ReedSolomon);
        assert!(result.is_err());
    }

    #[test]
    fn register_coding_params_n_lt_k() {
        let mut mgr = ErasureThresholdManager::new();
        let dataset = DatasetId::new([1u8; 32]);
        let result = mgr.register_coding_params(dataset, 10, 5, CodingScheme::ReedSolomon);
        assert!(result.is_err());
    }

    // ---- coding_params ----

    #[test]
    fn coding_params_unknown_dataset() {
        let mgr = ErasureThresholdManager::new();
        let result = mgr.coding_params(DatasetId::new([9u8; 32]));
        assert!(result.is_err());
    }

    // ---- can_reconstruct ----

    #[test]
    fn can_reconstruct_checks() {
        let mgr = ErasureThresholdManager::new();
        assert!(mgr.can_reconstruct(5, 4));
        assert!(mgr.can_reconstruct(4, 4));
        assert!(!mgr.can_reconstruct(3, 4));
        assert!(!mgr.can_reconstruct(5, 0));
    }

    // ---- failure_tolerance ----

    #[test]
    fn failure_tolerance_calculation() {
        let mgr = ErasureThresholdManager::new();
        assert_eq!(mgr.failure_tolerance(10, 4), 6);
        assert_eq!(mgr.failure_tolerance(7, 4), 3);
        assert_eq!(mgr.failure_tolerance(5, 5), 0);
        assert_eq!(mgr.failure_tolerance(5, 0), 0);
        assert_eq!(mgr.failure_tolerance(3, 5), 0);
    }

    // ---- validate_coding_params ----

    #[test]
    fn validate_coding_params_valid() {
        let mgr = ErasureThresholdManager::new();
        assert!(mgr.validate_coding_params(4, 10).is_ok());
        assert!(mgr.validate_coding_params(1, 1).is_ok());
    }

    #[test]
    fn validate_coding_params_invalid() {
        let mgr = ErasureThresholdManager::new();
        assert!(mgr.validate_coding_params(0, 5).is_err());
        assert!(mgr.validate_coding_params(5, 3).is_err());
        assert!(mgr.validate_coding_params(1, 2000).is_err());
    }

    // ---- storage_overhead_ratio ----

    #[test]
    fn storage_overhead_ratio_computation() {
        assert!((ErasureThresholdManager::storage_overhead_ratio(4, 10) - 2.5).abs() < 0.001);
        assert!((ErasureThresholdManager::storage_overhead_ratio(4, 8) - 2.0).abs() < 0.001);
        assert_eq!(ErasureThresholdManager::storage_overhead_ratio(0, 5), 0.0);
    }

    // ---- read_quorum_chunks ----

    #[test]
    fn read_quorum_chunks_equals_k() {
        assert_eq!(ErasureThresholdManager::read_quorum_chunks(4), 4);
        assert_eq!(ErasureThresholdManager::read_quorum_chunks(10), 10);
    }

    // ---- write_quorum_chunks ----

    #[test]
    fn write_quorum_chunks_computation() {
        // n=10, k=4 => n-k+1 = 7
        assert_eq!(ErasureThresholdManager::write_quorum_chunks(10, 4), 7);
        // n=7, k=4 => 4
        assert_eq!(ErasureThresholdManager::write_quorum_chunks(7, 4), 4);
        // invalid
        assert_eq!(ErasureThresholdManager::write_quorum_chunks(3, 5), 0);
    }

    // ---- survives_chunk_loss ----

    #[test]
    fn survives_chunk_loss_checks() {
        let mgr = ErasureThresholdManager::new();
        // n=10, k=4, lose 6 => available 4 >= 4 => survives
        assert!(mgr.survives_chunk_loss(10, 4, &[0, 1, 2, 3, 4, 5]));
        // lose 7 => available 3 < 4 => does not survive
        assert!(!mgr.survives_chunk_loss(10, 4, &[0, 1, 2, 3, 4, 5, 6]));
    }

    // ---- parity_chunk_count ----

    #[test]
    fn parity_chunk_count_computation() {
        assert_eq!(ErasureThresholdManager::parity_chunk_count(4, 10), 6);
        assert_eq!(ErasureThresholdManager::parity_chunk_count(5, 5), 0);
        assert_eq!(ErasureThresholdManager::parity_chunk_count(4, 3), 0);
    }

    // ---- select_params_for_tolerance ----

    #[test]
    fn select_params_for_tolerance_finds_pair() {
        let result = ErasureThresholdManager::select_params_for_tolerance(3, 4, 20);
        assert!(result.is_some());
        let (k, n) = result.unwrap();
        assert!(k >= 4);
        assert!(n <= 20);
        assert!(n - k >= 3);
    }

    #[test]
    fn select_params_for_tolerance_impossible() {
        // max_n too small to achieve tolerance with min_k
        let result = ErasureThresholdManager::select_params_for_tolerance(10, 8, 10);
        assert!(result.is_none());
    }

    // ---- reconstruction_set ----

    #[test]
    fn reconstruction_set_selection() {
        let n = 10;
        let k = 4;
        let available: BTreeSet<u32> = (0..10).collect();
        let set = ErasureThresholdManager::reconstruction_set(n, k, &available);
        assert!(set.is_some());
        let s = set.unwrap();
        assert_eq!(s.len(), 4);
    }

    #[test]
    fn reconstruction_set_insufficient_available() {
        let n = 10;
        let k = 8;
        let available: BTreeSet<u32> = (0..5).collect();
        let set = ErasureThresholdManager::reconstruction_set(n, k, &available);
        assert!(set.is_none());
    }

    // ---- is_erasure_enabled ----

    #[test]
    fn is_erasure_enabled_toggle() {
        let mut mgr = ErasureThresholdManager::new();
        let dataset = DatasetId::new([1u8; 32]);
        assert!(!mgr.is_erasure_enabled(dataset));
        mgr.register_coding_params(dataset, 4, 7, CodingScheme::ReedSolomon)
            .unwrap();
        assert!(mgr.is_erasure_enabled(dataset));
    }

    // ---- registered_datasets ----

    #[test]
    fn registered_datasets_listing() {
        let mut mgr = ErasureThresholdManager::new();
        let d1 = DatasetId::new([1u8; 32]);
        let d2 = DatasetId::new([2u8; 32]);
        mgr.register_coding_params(d1, 4, 7, CodingScheme::ReedSolomon)
            .unwrap();
        mgr.register_coding_params(d2, 8, 12, CodingScheme::XorParity)
            .unwrap();
        let reg = mgr.registered_datasets();
        assert_eq!(reg.len(), 2);
        assert!(reg.contains(&(d1, 4, 7)));
        assert!(reg.contains(&(d2, 8, 12)));
    }

    // ---- place_chunks_across_domains ----

    #[test]
    fn place_chunks_across_domains_valid() {
        let mgr = ErasureThresholdManager::new();
        let domains = vec![
            FailureDomainRef(1),
            FailureDomainRef(2),
            FailureDomainRef(3),
        ];
        let placement = mgr.place_chunks_across_domains(6, &domains);
        assert!(placement.is_ok());
        let p = placement.unwrap();
        assert_eq!(p.len(), 6);
    }

    #[test]
    fn place_chunks_across_domains_insufficient() {
        let mgr = ErasureThresholdManager::new();
        let domains = vec![FailureDomainRef(1)];
        let result = mgr.place_chunks_across_domains(5, &domains);
        assert!(result.is_err());
    }

    #[test]
    fn place_chunks_across_domains_empty() {
        let mgr = ErasureThresholdManager::new();
        let result = mgr.place_chunks_across_domains(3, &[]);
        assert!(result.is_err());
    }

    // ---- health tracker ----

    #[test]
    fn mark_chunk_lost_and_healthy() {
        let mut mgr = ErasureThresholdManager::new();
        let dataset = DatasetId::new([1u8; 32]);
        mgr.register_coding_params(dataset, 4, 7, CodingScheme::ReedSolomon)
            .unwrap();
        mgr.mark_chunk_lost(dataset, 0);
        assert_eq!(mgr.healthy_chunk_count(dataset), 6);
        mgr.mark_chunk_healthy(dataset, 0);
        assert_eq!(mgr.healthy_chunk_count(dataset), 7);
    }

    #[test]
    fn healthy_chunk_count_unknown_dataset() {
        let mgr = ErasureThresholdManager::new();
        assert_eq!(mgr.healthy_chunk_count(DatasetId::new([9u8; 32])), 0);
    }

    // ---- schedule_reconstruction ----

    #[test]
    fn schedule_reconstruction_success() {
        let mut mgr = ErasureThresholdManager::new();
        let dataset = DatasetId::new([1u8; 32]);
        mgr.register_coding_params(dataset, 4, 7, CodingScheme::ReedSolomon)
            .unwrap();
        mgr.mark_chunk_lost(dataset, 0);
        mgr.mark_chunk_lost(dataset, 1);
        let sched = mgr.schedule_reconstruction(dataset);
        assert!(sched.is_ok());
        let s = sched.unwrap();
        assert_eq!(s.priority, ReconstructionPriority::High);
        assert!(!s.chunks_to_reconstruct.is_empty());
    }

    #[test]
    fn schedule_reconstruction_insufficient_chunks() {
        let mut mgr = ErasureThresholdManager::new();
        let dataset = DatasetId::new([1u8; 32]);
        mgr.register_coding_params(dataset, 4, 5, CodingScheme::ReedSolomon)
            .unwrap();
        // lose 2 chunks, available 3 < k=4
        mgr.mark_chunk_lost(dataset, 0);
        mgr.mark_chunk_lost(dataset, 1);
        let result = mgr.schedule_reconstruction(dataset);
        assert!(result.is_err());
    }

    #[test]
    fn complete_reconstruction_marks_healthy() {
        let mut mgr = ErasureThresholdManager::new();
        let dataset = DatasetId::new([1u8; 32]);
        mgr.register_coding_params(dataset, 4, 7, CodingScheme::ReedSolomon)
            .unwrap();
        mgr.mark_chunk_lost(dataset, 0);
        mgr.schedule_reconstruction(dataset).unwrap();
        mgr.complete_reconstruction(dataset);
        assert_eq!(mgr.healthy_chunk_count(dataset), 7);
    }

    // ---- reconstruction_count / success / failure ----

    #[test]
    fn reconstruction_counts() {
        let mut mgr = ErasureThresholdManager::new();
        let dataset = DatasetId::new([1u8; 32]);
        mgr.register_coding_params(dataset, 4, 7, CodingScheme::ReedSolomon)
            .unwrap();
        mgr.mark_chunk_lost(dataset, 0);
        mgr.schedule_reconstruction(dataset).unwrap();
        assert_eq!(mgr.reconstruction_count(), 1);
        assert_eq!(mgr.successful_reconstructions(), 1);
        assert_eq!(mgr.failed_reconstructions(), 0);
    }

    // ---- capacity_plan ----

    #[test]
    fn capacity_plan_computation() {
        let mut mgr = ErasureThresholdManager::new();
        let dataset = DatasetId::new([1u8; 32]);
        mgr.register_coding_params(dataset, 4, 10, CodingScheme::ReedSolomon)
            .unwrap();
        let plan = mgr.capacity_plan(dataset, 1000);
        assert!(plan.is_ok());
        let p = plan.unwrap();
        assert_eq!(p.raw_data_bytes, 1000);
        assert_eq!(p.encoded_bytes, 2500);
        assert_eq!(p.overhead_bytes, 1500);
        assert_eq!(p.failure_tolerance, 6);
    }

    #[test]
    fn capacity_plan_unknown_dataset() {
        let mgr = ErasureThresholdManager::new();
        let result = mgr.capacity_plan(DatasetId::new([9u8; 32]), 100);
        assert!(result.is_err());
    }

    // ---- set_default_params ----

    #[test]
    fn set_default_params_valid() {
        let mut mgr = ErasureThresholdManager::new();
        assert!(mgr.set_default_params(8, 16).is_ok());
        assert_eq!(mgr.default_params(), (8, 16));
    }

    #[test]
    fn set_default_params_invalid() {
        let mut mgr = ErasureThresholdManager::new();
        assert!(mgr.set_default_params(0, 5).is_err());
        assert!(mgr.set_default_params(10, 5).is_err());
    }

    // ---- ErasureCapacityPlanner tests ----

    #[test]
    fn capacity_planner_aggregation() {
        let mut planner = ErasureCapacityPlanner::new();
        planner.add_plan(ErasureCapacityPlan {
            dataset_id: DatasetId::new([1u8; 32]),
            raw_data_bytes: 1000,
            encoded_bytes: 2000,
            overhead_bytes: 1000,
            overhead_ratio: 2.0,
            failure_tolerance: 3,
            chunks: 10,
            data_chunks: 4,
        });
        planner.add_plan(ErasureCapacityPlan {
            dataset_id: DatasetId::new([2u8; 32]),
            raw_data_bytes: 500,
            encoded_bytes: 1500,
            overhead_bytes: 1000,
            overhead_ratio: 3.0,
            failure_tolerance: 2,
            chunks: 6,
            data_chunks: 2,
        });
        assert_eq!(planner.total_raw_bytes(), 1500);
        assert_eq!(planner.total_encoded_bytes(), 3500);
        assert_eq!(planner.total_overhead_bytes(), 2000);
        assert!((planner.aggregate_overhead_ratio() - (3500.0 / 1500.0)).abs() < 0.001);
        assert_eq!(planner.min_failure_tolerance(), 2);
        assert_eq!(planner.dataset_count(), 2);
    }

    #[test]
    fn capacity_planner_highest_overhead() {
        let mut planner = ErasureCapacityPlanner::new();
        planner.add_plan(ErasureCapacityPlan {
            dataset_id: DatasetId::new([1u8; 32]),
            raw_data_bytes: 1000,
            encoded_bytes: 2000,
            overhead_bytes: 1000,
            overhead_ratio: 2.0,
            failure_tolerance: 3,
            chunks: 10,
            data_chunks: 4,
        });
        planner.add_plan(ErasureCapacityPlan {
            dataset_id: DatasetId::new([2u8; 32]),
            raw_data_bytes: 1000,
            encoded_bytes: 4000,
            overhead_bytes: 3000,
            overhead_ratio: 4.0,
            failure_tolerance: 1,
            chunks: 8,
            data_chunks: 2,
        });
        let highest = planner.highest_overhead_dataset();
        assert!(highest.is_some());
        assert_eq!(highest.unwrap().0[0], 2);
    }

    // ---- ReedSolomonCodec tests ----

    #[test]
    fn reed_solomon_validate_params() {
        let codec = ReedSolomonCodec;
        assert!(codec.validate_rs_params(4, 10, 4).is_ok()); // GF(2^4) = 15 symbols, n=10 OK
        assert!(codec.validate_rs_params(10, 15, 4).is_ok()); // n=15 = max
        assert!(codec.validate_rs_params(1, 16, 4).is_err()); // n=16 > 15
    }

    #[test]
    fn reed_solomon_max_chunks() {
        let codec = ReedSolomonCodec;
        assert_eq!(codec.max_chunks_for_field(4), 15);
        assert_eq!(codec.max_chunks_for_field(8), 255);
    }

    #[test]
    fn reed_solomon_parity_symbols() {
        let codec = ReedSolomonCodec;
        assert_eq!(codec.parity_symbols(10, 4), 6);
        assert_eq!(codec.parity_symbols(5, 5), 0);
    }

    // ---- FailureDomainRef tests ----

    #[test]
    fn failure_domain_ref_ordering() {
        let a = FailureDomainRef(1);
        let b = FailureDomainRef(2);
        assert!(a < b);
        assert_ne!(a, b);
    }

    // ---- ChunkHealth tests ----

    #[test]
    fn chunk_health_variants() {
        let states = [
            ChunkHealth::Healthy,
            ChunkHealth::Degraded,
            ChunkHealth::Lost,
            ChunkHealth::Recovering,
        ];
        for s in states {
            let _ = format!("{:?}", s);
        }
    }

    // ---- ReconstructionPriority tests ----

    #[test]
    fn reconstruction_priority_ordering() {
        assert!(ReconstructionPriority::Critical > ReconstructionPriority::High);
        assert!(ReconstructionPriority::High > ReconstructionPriority::Normal);
        assert!(ReconstructionPriority::Normal > ReconstructionPriority::Low);
    }

    // ---- ReconstructionSchedule tests ----

    #[test]
    fn reconstruction_schedule_construction() {
        let sched = ReconstructionSchedule {
            dataset_id: DatasetId::new([1u8; 32]),
            chunks_to_reconstruct: vec![0, 1],
            source_chunks: vec![2, 3, 4, 5],
            estimated_bandwidth_bytes: 2_097_152,
            priority: ReconstructionPriority::Critical,
        };
        assert_eq!(sched.chunks_to_reconstruct.len(), 2);
        assert_eq!(sched.priority, ReconstructionPriority::Critical);
    }

    // ---- RegeneratingCodeConfig tests ----

    #[test]
    fn regenerating_code_config_construction() {
        let cfg = RegeneratingCodeConfig {
            k: 4,
            n: 10,
            d: 6,
            alpha: 16,
            beta: 4,
        };
        assert_eq!(cfg.k, 4);
        assert_eq!(cfg.d, 6);
    }

    // ---- LrcConfig tests ----

    #[test]
    fn lrc_config_construction() {
        let cfg = LrcConfig {
            k: 12,
            n: 18,
            locality: 4,
        };
        assert_eq!(cfg.k, 12);
        assert_eq!(cfg.locality, 4);
    }

    // ---- ErasureCapacityPlan tests ----

    #[test]
    fn erasure_capacity_plan_construction() {
        let plan = ErasureCapacityPlan {
            dataset_id: DatasetId::new([1u8; 32]),
            raw_data_bytes: 1024,
            encoded_bytes: 3072,
            overhead_bytes: 2048,
            overhead_ratio: 3.0,
            failure_tolerance: 5,
            chunks: 15,
            data_chunks: 5,
        };
        assert_eq!(plan.failure_tolerance, 5);
    }

    // ---- ErasureError tests ----

    #[test]
    fn erasure_error_variants() {
        let err = ErasureError::InsufficientChunks { available: 3, required: 4 };
        match &err {
            ErasureError::InsufficientChunks { available, required } => {
                assert_eq!(*available, 3);
                assert_eq!(*required, 4);
            }
            _ => panic!("expected InsufficientChunks"),
        }

        let err2 = ErasureError::DatasetNotFound {
            dataset: DatasetId::new([9u8; 32]),
        };
        match &err2 {
            ErasureError::DatasetNotFound { dataset } => {
                assert_eq!(dataset.0[0], 9);
            }
            _ => panic!("expected DatasetNotFound"),
        }

        let err3 = ErasureError::InvalidParams {
            k: 10,
            n: 5,
            reason: "n < k".to_string(),
        };
        match &err3 {
            ErasureError::InvalidParams { k, n, reason } => {
                assert_eq!(*k, 10);
                assert_eq!(*n, 5);
                assert_eq!(reason, "n < k");
            }
            _ => panic!("expected InvalidParams"),
        }
    }

    // ---- ERASURE_THRESHOLD_BIT ----

    #[test]
    fn erasure_threshold_bit_position() {
        assert_eq!(ERASURE_THRESHOLD_BIT, 1 << 8);
    }

    // ---- erasure_threshold_enabled ----

    #[test]
    fn erasure_threshold_enabled_checks_bit() {
        let mut bits = EdgeCaseBits(0);
        assert!(!erasure_threshold_enabled(&bits));
        bits.enable(ERASURE_THRESHOLD_BIT);
        assert!(erasure_threshold_enabled(&bits));
    }

    // ---- Integration: full erasure workflow ----

    #[test]
    fn full_erasure_workflow() {
        let mut mgr = ErasureThresholdManager::new();
        let dataset = DatasetId::new([1u8; 32]);

        // Register params.
        mgr.register_coding_params(dataset, 4, 10, CodingScheme::SystematicReedSolomon)
            .unwrap();
        assert!(mgr.is_erasure_enabled(dataset));
        let (k, n) = mgr.coding_params(dataset).unwrap();
        assert_eq!(k, 4);
        assert_eq!(n, 10);

        // Validate.
        assert!(mgr.validate_coding_params(k, n).is_ok());

        // Loss tolerance.
        assert_eq!(mgr.failure_tolerance(n, k), 6);
        assert!(mgr.survives_chunk_loss(n, k, &[0, 1, 2, 3, 4, 5]));

        // Mark some lost, schedule reconstruction.
        for i in 0..3 {
            mgr.mark_chunk_lost(dataset, i);
        }
        assert_eq!(mgr.healthy_chunk_count(dataset), 7);
        let sched = mgr.schedule_reconstruction(dataset).unwrap();
        assert_eq!(sched.chunks_to_reconstruct.len(), 3);

        // Complete.
        mgr.complete_reconstruction(dataset);
        assert_eq!(mgr.healthy_chunk_count(dataset), 10);

        // Capacity plan.
        let plan = mgr.capacity_plan(dataset, 4096).unwrap();
        assert_eq!(plan.encoded_bytes, 10240);
    }

    // ---- Multiple datasets independent ----

    #[test]
    fn multiple_datasets_independent() {
        let mut mgr = ErasureThresholdManager::new();
        let d1 = DatasetId::new([1u8; 32]);
        let d2 = DatasetId::new([2u8; 32]);
        mgr.register_coding_params(d1, 4, 7, CodingScheme::ReedSolomon)
            .unwrap();
        mgr.register_coding_params(d2, 8, 12, CodingScheme::XorParity)
            .unwrap();
        assert_eq!(mgr.coding_params(d1).unwrap(), (4, 7));
        assert_eq!(mgr.coding_params(d2).unwrap(), (8, 12));
        assert_eq!(mgr.registered_datasets().len(), 2);
    }

    // ========== ErasureStripingManager tests ==========

    #[test]
    fn striping_manager_create_stripe() {
        let mut mgr = ErasureStripingManager::new();
        let dataset = DatasetId::new([1u8; 32]);
        let id = mgr.create_stripe(
            dataset,
            vec![0, 1, 2, 3, 4],
            1_048_576,
            2,
            3,
        );
        let stripe = mgr.stripe(id).unwrap();
        assert_eq!(stripe.dataset_id, dataset);
        assert_eq!(stripe.chunk_indices.len(), 5);
        assert_eq!(stripe.parity_chunk_count, 2);
    }

    #[test]
    fn striping_manager_stripes_for_dataset() {
        let mut mgr = ErasureStripingManager::new();
        let d1 = DatasetId::new([1u8; 32]);
        let d2 = DatasetId::new([2u8; 32]);
        mgr.create_stripe(d1, vec![0, 1], 100, 0, 2);
        mgr.create_stripe(d1, vec![2, 3], 200, 0, 2);
        mgr.create_stripe(d2, vec![4, 5], 300, 0, 2);
        let d1_stripes = mgr.stripes_for_dataset(d1);
        assert_eq!(d1_stripes.len(), 2);
        let d2_stripes = mgr.stripes_for_dataset(d2);
        assert_eq!(d2_stripes.len(), 1);
    }

    #[test]
    fn striping_manager_default_size() {
        let mgr = ErasureStripingManager::new();
        assert_eq!(mgr.default_stripe_size(), 1_048_576);
    }

    #[test]
    fn striping_manager_set_default_size() {
        let mut mgr = ErasureStripingManager::new();
        mgr.set_default_stripe_size(4_194_304);
        assert_eq!(mgr.default_stripe_size(), 4_194_304);
    }

    #[test]
    fn striping_manager_remove_stripe() {
        let mut mgr = ErasureStripingManager::new();
        let dataset = DatasetId::new([1u8; 32]);
        let id = mgr.create_stripe(dataset, vec![0, 1], 100, 0, 2);
        assert!(mgr.stripe(id).is_some());
        let removed = mgr.remove_stripe(id);
        assert!(removed.is_some());
        assert!(mgr.stripe(id).is_none());
    }

    #[test]
    fn striping_manager_total_striped_bytes() {
        let mut mgr = ErasureStripingManager::new();
        let dataset = DatasetId::new([1u8; 32]);
        mgr.create_stripe(dataset, vec![0, 1], 100, 0, 2);
        mgr.create_stripe(dataset, vec![2, 3], 200, 0, 2);
        assert_eq!(mgr.total_striped_bytes(), 300);
    }

    // ========== ErasureRepairScheduler tests ==========

    #[test]
    fn repair_scheduler_queue_job() {
        let mut sched = ErasureRepairScheduler::new();
        let dataset = DatasetId::new([1u8; 32]);
        let id = sched.queue_repair(dataset, 5, vec![0, 1, 2], RepairPriority::High);
        let job = sched.jobs_for_dataset(dataset).first().unwrap();
        assert_eq!(job.job_id, id);
        assert_eq!(job.priority, RepairPriority::High);
        assert_eq!(job.status, RepairJobStatus::Queued);
    }

    #[test]
    fn repair_scheduler_start_next_job() {
        let mut sched = ErasureRepairScheduler::new();
        let dataset = DatasetId::new([1u8; 32]);
        sched.queue_repair(dataset, 0, vec![1, 2], RepairPriority::Normal);
        sched.queue_repair(dataset, 1, vec![2, 3], RepairPriority::Critical);
        let id = sched.start_next_job().unwrap();
        assert_eq!(id, RepairJobId(1));
    }

    #[test]
    fn repair_scheduler_complete_and_fail_job() {
        let mut sched = ErasureRepairScheduler::new();
        let dataset = DatasetId::new([1u8; 32]);
        let id = sched.queue_repair(dataset, 0, vec![1], RepairPriority::Normal);
        sched.start_next_job().unwrap();
        sched.complete_job(id).unwrap();
        assert_eq!(sched.completed_repairs(), 1);
        assert_eq!(sched.in_progress_count(), 0);
    }

    #[test]
    fn repair_scheduler_max_concurrent() {
        let mut sched = ErasureRepairScheduler::new();
        let dataset = DatasetId::new([1u8; 32]);
        sched.set_max_concurrent(1);
        sched.queue_repair(dataset, 0, vec![1], RepairPriority::Normal);
        sched.queue_repair(dataset, 1, vec![2], RepairPriority::Normal);
        sched.start_next_job().unwrap();
        assert!(sched.start_next_job().is_none());
    }

    // ========== ErasureTopologyAnalyzer tests ==========

    #[test]
    fn topology_analyzer_balanced_placement() {
        let analyzer = ErasureTopologyAnalyzer::new();
        let placement = vec![
            ChunkPlacement { chunk_index: 0, domain: FailureDomainRef(1), node_hint: None },
            ChunkPlacement { chunk_index: 1, domain: FailureDomainRef(2), node_hint: None },
            ChunkPlacement { chunk_index: 2, domain: FailureDomainRef(3), node_hint: None },
        ];
        let analysis = analyzer.analyze_placement(&placement);
        assert!(analysis.balanced);
        assert_eq!(analysis.domains_used, 3);
        assert!(analysis.diversity_score > 0.5);
    }

    #[test]
    fn topology_analyzer_unbalanced_placement() {
        let analyzer = ErasureTopologyAnalyzer::new();
        let placement = vec![
            ChunkPlacement { chunk_index: 0, domain: FailureDomainRef(1), node_hint: None },
            ChunkPlacement { chunk_index: 1, domain: FailureDomainRef(1), node_hint: None },
            ChunkPlacement { chunk_index: 2, domain: FailureDomainRef(2), node_hint: None },
        ];
        let analysis = analyzer.analyze_placement(&placement);
        assert!(!analysis.balanced);
        assert_eq!(analysis.max_chunks_per_domain, 2);
        assert_eq!(analysis.min_chunks_per_domain, 1);
    }

    #[test]
    fn topology_analyzer_is_resilient() {
        let analyzer = ErasureTopologyAnalyzer::new();
        let placement = vec![
            ChunkPlacement { chunk_index: 0, domain: FailureDomainRef(1), node_hint: None },
            ChunkPlacement { chunk_index: 1, domain: FailureDomainRef(2), node_hint: None },
            ChunkPlacement { chunk_index: 2, domain: FailureDomainRef(3), node_hint: None },
        ];
        let analysis = analyzer.analyze_placement(&placement);
        assert!(ErasureTopologyAnalyzer::is_resilient(&analysis));
    }

    #[test]
    fn topology_analyzer_recommended_min_domains() {
        assert_eq!(ErasureTopologyAnalyzer::recommended_min_domains(10, 3), 4);
        assert_eq!(ErasureTopologyAnalyzer::recommended_min_domains(10, 0), 2);
        assert_eq!(ErasureTopologyAnalyzer::recommended_min_domains(5, 10), 5);
    }

    // ========== ErasureIntegrityAuditor tests ==========

    #[test]
    fn integrity_auditor_record_pass() {
        let mut auditor = ErasureIntegrityAuditor::new();
        let dataset = DatasetId::new([1u8; 32]);
        let id = auditor.record_audit(dataset, 10, 0, 0);
        let audit = auditor.audits_for_dataset(dataset).first().unwrap();
        assert_eq!(id, audit.audit_id);
        assert_eq!(audit.result, AuditResult::Pass);
    }

    #[test]
    fn integrity_auditor_record_fail() {
        let mut auditor = ErasureIntegrityAuditor::new();
        let dataset = DatasetId::new([1u8; 32]);
        auditor.record_audit(dataset, 10, 5, 1);
        assert_eq!(auditor.failed_audits(), 1);
    }

    #[test]
    fn integrity_auditor_pass_rate() {
        let mut auditor = ErasureIntegrityAuditor::new();
        let dataset = DatasetId::new([1u8; 32]);
        auditor.record_audit(dataset, 10, 0, 0);
        auditor.record_audit(dataset, 10, 5, 1);
        assert!((auditor.pass_rate() - 0.5).abs() < 0.001);
    }

    #[test]
    fn integrity_auditor_latest_audit() {
        let mut auditor = ErasureIntegrityAuditor::new();
        let dataset = DatasetId::new([1u8; 32]);
        assert!(auditor.latest_audit().is_none());
        auditor.record_audit(dataset, 10, 0, 0);
        assert!(auditor.latest_audit().is_some());
    }

    // ========== ErasureCodeParametersValidator tests ==========

    #[test]
    fn validator_valid_params() {
        let validator = ErasureCodeParametersValidator::new();
        let result = validator.validate(4, 10, 8);
        assert!(result.valid);
        assert!(result.errors.is_empty());
    }

    #[test]
    fn validator_k_zero_invalid() {
        let validator = ErasureCodeParametersValidator::new();
        let result = validator.validate(0, 5, 8);
        assert!(!result.valid);
    }

    #[test]
    fn validator_n_less_than_k_invalid() {
        let validator = ErasureCodeParametersValidator::new();
        let result = validator.validate(5, 3, 8);
        assert!(!result.valid);
    }

    #[test]
    fn validator_n_exceeds_max_invalid() {
        let validator = ErasureCodeParametersValidator::with_limits(5, 1, 8);
        let result = validator.validate(3, 10, 8);
        assert!(!result.valid);
    }

    #[test]
    fn validator_recommended_params() {
        let validator = ErasureCodeParametersValidator::new();
        let result = validator.validate(4, 7, 8);
        assert!(result.valid);
        assert_eq!(result.recommended_k, 4);
        assert_eq!(result.recommended_n, 7);
    }

    #[test]
    fn validator_is_valid_bool() {
        let validator = ErasureCodeParametersValidator::new();
        assert!(validator.is_valid(4, 10, 8));
        assert!(!validator.is_valid(0, 5, 8));
    }

    #[test]
    fn validator_set_limits() {
        let mut validator = ErasureCodeParametersValidator::new();
        validator.set_max_n(20);
        validator.set_min_k(2);
        assert_eq!(validator.max_n(), 20);
        assert_eq!(validator.min_k(), 2);
    }

    // ========== ParityMatrixGenerator tests ==========

    #[test]
    fn parity_matrix_generate_systematic() {
        let gen = ParityMatrixGenerator::new();
        let rows = gen.generate_systematic_parity_matrix(4, 10).unwrap();
        assert_eq!(rows.len(), 6);
        for row in &rows {
            assert_eq!(row.coefficients.len(), 10);
        }
    }

    #[test]
    fn parity_matrix_generate_invalid() {
        let gen = ParityMatrixGenerator::new();
        assert!(gen.generate_systematic_parity_matrix(0, 5).is_err());
        assert!(gen.generate_systematic_parity_matrix(10, 5).is_err());
    }

    #[test]
    fn parity_matrix_verify_row_rank() {
        let gen = ParityMatrixGenerator::new();
        let rows = gen.generate_systematic_parity_matrix(4, 8).unwrap();
        assert!(gen.verify_row_rank(&rows));
    }

    #[test]
    fn parity_row_count() {
        assert_eq!(ParityMatrixGenerator::parity_row_count(4, 10), 6);
        assert_eq!(ParityMatrixGenerator::parity_row_count(5, 5), 0);
    }

    // ========== FragmentRecoveryCoordinator tests ==========

    #[test]
    fn recovery_coordinator_create_task() {
        let mut coord = FragmentRecoveryCoordinator::new();
        let dataset = DatasetId::new([1u8; 32]);
        let id = coord.create_recovery_task(dataset, vec![0, 1], vec![2, 3, 4]);
        assert_eq!(id, RecoveryTaskId(0));
        let tasks = coord.tasks_for_dataset(dataset);
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].status, RecoveryTaskStatus::Pending);
    }

    #[test]
    fn recovery_coordinator_start_and_complete() {
        let mut coord = FragmentRecoveryCoordinator::new();
        let dataset = DatasetId::new([1u8; 32]);
        let id = coord.create_recovery_task(dataset, vec![0], vec![1]);
        coord.start_recovery_task(id).unwrap();
        coord.complete_recovery_task(id).unwrap();
        assert_eq!(coord.completed_count(), 1);
    }

    #[test]
    fn recovery_coordinator_fail_task() {
        let mut coord = FragmentRecoveryCoordinator::new();
        let dataset = DatasetId::new([1u8; 32]);
        let id = coord.create_recovery_task(dataset, vec![0], vec![1]);
        coord.start_recovery_task(id).unwrap();
        coord.fail_recovery_task(id).unwrap();
        assert_eq!(coord.failed_count(), 1);
    }

    #[test]
    fn recovery_coordinator_pending_tasks() {
        let mut coord = FragmentRecoveryCoordinator::new();
        let dataset = DatasetId::new([1u8; 32]);
        coord.create_recovery_task(dataset, vec![0], vec![1]);
        coord.create_recovery_task(dataset, vec![1], vec![2]);
        assert_eq!(coord.pending_tasks().len(), 2);
    }

    #[test]
    fn recovery_coordinator_max_concurrent() {
        let mut coord = FragmentRecoveryCoordinator::new();
        let dataset = DatasetId::new([1u8; 32]);
        coord.set_max_concurrent(1);
        coord.create_recovery_task(dataset, vec![0], vec![1]);
        coord.create_recovery_task(dataset, vec![1], vec![2]);
        coord.start_recovery_task(RecoveryTaskId(0)).unwrap();
        assert!(coord.start_recovery_task(RecoveryTaskId(1)).is_err());
    }

    // ========== ChunkLossPatternAnalyzer tests ==========

    #[test]
    fn loss_analyzer_survivable() {
        let mut analyzer = ChunkLossPatternAnalyzer::new();
        analyzer.assign_chunk_domain(0, FailureDomainRef(1));
        analyzer.assign_chunk_domain(1, FailureDomainRef(2));
        analyzer.assign_chunk_domain(2, FailureDomainRef(3));
        analyzer.assign_chunk_domain(3, FailureDomainRef(1));
        analyzer.assign_chunk_domain(4, FailureDomainRef(2));
        let result = analyzer.analyze_loss(5, &[0, 1], 3);
        assert!(result.survivable);
        assert_eq!(result.surviving_chunks, 3);
        assert_eq!(result.lost_chunks, 2);
    }

    #[test]
    fn loss_analyzer_not_survivable() {
        let mut analyzer = ChunkLossPatternAnalyzer::new();
        for i in 0..5 {
            analyzer.assign_chunk_domain(i, FailureDomainRef(1));
        }
        let result = analyzer.analyze_loss(5, &[0, 1, 2, 3], 3);
        assert!(!result.survivable);
        assert_eq!(result.surviving_chunks, 1);
    }

    #[test]
    fn loss_analyzer_domain_loss_distribution() {
        let mut analyzer = ChunkLossPatternAnalyzer::new();
        analyzer.assign_chunk_domain(0, FailureDomainRef(1));
        analyzer.assign_chunk_domain(1, FailureDomainRef(1));
        analyzer.assign_chunk_domain(2, FailureDomainRef(2));
        let result = analyzer.analyze_loss(3, &[0, 1, 2], 1);
        assert_eq!(result.worst_domain_loss, 2);
        assert_eq!(result.loss_by_domain.get(&FailureDomainRef(1)), Some(&2));
    }

    #[test]
    fn loss_analyzer_recommended_action() {
        let mut analyzer = ChunkLossPatternAnalyzer::new();
        analyzer.assign_chunk_domain(0, FailureDomainRef(1));
        analyzer.assign_chunk_domain(1, FailureDomainRef(2));
        let result = analyzer.analyze_loss(2, &[0], 1);
        assert_eq!(result.recommended_action, RecoveryAction::ReconstructFromParity);
    }

    // ========== ErasureTopologyConstraintChecker tests ==========

    #[test]
    fn topology_checker_valid_placement() {
        let checker = ErasureTopologyConstraintChecker::new();
        let placement = vec![
            ChunkPlacement { chunk_index: 0, domain: FailureDomainRef(1), node_hint: None },
            ChunkPlacement { chunk_index: 1, domain: FailureDomainRef(2), node_hint: None },
            ChunkPlacement { chunk_index: 2, domain: FailureDomainRef(3), node_hint: None },
        ];
        let constraint = ErasureTopologyConstraintChecker::default_constraints();
        assert!(checker.check_placement(&placement, &constraint));
    }

    #[test]
    fn topology_checker_invalid_too_many_per_domain() {
        let checker = ErasureTopologyConstraintChecker::new();
        let placement = vec![
            ChunkPlacement { chunk_index: 0, domain: FailureDomainRef(1), node_hint: None },
            ChunkPlacement { chunk_index: 1, domain: FailureDomainRef(1), node_hint: None },
            ChunkPlacement { chunk_index: 2, domain: FailureDomainRef(1), node_hint: None },
            ChunkPlacement { chunk_index: 3, domain: FailureDomainRef(1), node_hint: None },
        ];
        let constraint = ErasureTopologyConstraintChecker::default_constraints();
        assert!(!checker.check_placement(&placement, &constraint));
    }

    #[test]
    fn topology_checker_invalid_too_few_domains() {
        let checker = ErasureTopologyConstraintChecker::new();
        let placement = vec![
            ChunkPlacement { chunk_index: 0, domain: FailureDomainRef(1), node_hint: None },
            ChunkPlacement { chunk_index: 1, domain: FailureDomainRef(1), node_hint: None },
        ];
        let constraint = ErasureTopologyConstraintChecker::default_constraints();
        assert!(!checker.check_placement(&placement, &constraint));
    }

    // ========== DecodingScheduleBuilder tests ==========

    #[test]
    fn decoding_schedule_build() {
        let builder = DecodingScheduleBuilder::new();
        let dataset = DatasetId::new([1u8; 32]);
        let schedule = builder.build(dataset, &[0, 1], &[2, 3, 4, 5]);
        assert_eq!(schedule.target_chunks.len(), 2);
        assert_eq!(schedule.steps.len(), 2);
    }

    #[test]
    fn decoding_schedule_step_count() {
        let builder = DecodingScheduleBuilder::new();
        let dataset = DatasetId::new([1u8; 32]);
        let schedule = builder.build(dataset, &[0, 1, 2], &[3, 4]);
        assert_eq!(DecodingScheduleBuilder::step_count(&schedule), 3);
    }

    // ========== IntegrityVerifier tests ==========

    #[test]
    fn integrity_verifier_register_and_verify() {
        let mut verifier = IntegrityVerifier::new();
        let dataset = DatasetId::new([1u8; 32]);
        verifier.register_checksum(dataset, 0, 42);
        let result = verifier.verify_chunk(dataset, 0, 42);
        assert!(result.verified);
        assert!(result.checksum_match);
    }

    #[test]
    fn integrity_verifier_mismatch() {
        let mut verifier = IntegrityVerifier::new();
        let dataset = DatasetId::new([1u8; 32]);
        verifier.register_checksum(dataset, 0, 42);
        let result = verifier.verify_chunk(dataset, 0, 99);
        assert!(!result.verified);
        assert!(!result.checksum_match);
    }

    #[test]
    fn integrity_verifier_failure_rate() {
        let mut verifier = IntegrityVerifier::new();
        let dataset = DatasetId::new([1u8; 32]);
        verifier.register_checksum(dataset, 0, 1);
        verifier.register_checksum(dataset, 1, 2);
        verifier.verify_chunk(dataset, 0, 1);
        verifier.verify_chunk(dataset, 1, 99);
        assert!((verifier.failure_rate() - 0.5).abs() < 0.001);
    }

    // ========== CodingSchemeSelector tests ==========

    #[test]
    fn coding_scheme_selector_select() {
        let selector = CodingSchemeSelector;
        assert_eq!(selector.select_scheme(5, 3.0, false), CodingScheme::Regenerating);
        assert_eq!(selector.select_scheme(1, 1.5, false), CodingScheme::XorParity);
        assert_eq!(selector.select_scheme(1, 3.0, true), CodingScheme::LocallyRepairable);
        assert_eq!(selector.select_scheme(2, 3.0, false), CodingScheme::SystematicReedSolomon);
    }

    #[test]
    fn coding_scheme_selector_description() {
        let desc = CodingSchemeSelector::scheme_description(CodingScheme::ReedSolomon);
        assert!(!desc.is_empty());
        let desc = CodingSchemeSelector::scheme_description(CodingScheme::Hybrid);
        assert!(!desc.is_empty());
    }

    // ========== ErasureCapacityPlanner extended tests ==========

    #[test]
    fn capacity_planner_plan_for_dataset() {
        let mut planner = ErasureCapacityPlanner::new();
        let dataset = DatasetId::new([1u8; 32]);
        planner.add_plan(ErasureCapacityPlan {
            dataset_id: dataset,
            raw_data_bytes: 1000,
            encoded_bytes: 2000,
            overhead_bytes: 1000,
            overhead_ratio: 2.0,
            failure_tolerance: 3,
            chunks: 10,
            data_chunks: 4,
        });
        let plan = planner.plan_for_dataset(dataset).unwrap();
        assert_eq!(plan.overhead_ratio, 2.0);
    }

    #[test]
    fn capacity_planner_remove_plan() {
        let mut planner = ErasureCapacityPlanner::new();
        let dataset = DatasetId::new([1u8; 32]);
        planner.add_plan(ErasureCapacityPlan {
            dataset_id: dataset,
            raw_data_bytes: 1000,
            encoded_bytes: 2000,
            overhead_bytes: 1000,
            overhead_ratio: 2.0,
            failure_tolerance: 3,
            chunks: 10,
            data_chunks: 4,
        });
        assert!(planner.remove_plan(dataset).is_some());
        assert!(planner.plan_for_dataset(dataset).is_none());
    }

    #[test]
    fn capacity_planner_plans_by_overhead() {
        let mut planner = ErasureCapacityPlanner::new();
        planner.add_plan(ErasureCapacityPlan {
            dataset_id: DatasetId::new([1u8; 32]),
            raw_data_bytes: 1000,
            encoded_bytes: 2000,
            overhead_bytes: 1000,
            overhead_ratio: 2.0,
            failure_tolerance: 3,
            chunks: 10,
            data_chunks: 4,
        });
        planner.add_plan(ErasureCapacityPlan {
            dataset_id: DatasetId::new([2u8; 32]),
            raw_data_bytes: 1000,
            encoded_bytes: 4000,
            overhead_bytes: 3000,
            overhead_ratio: 4.0,
            failure_tolerance: 1,
            chunks: 8,
            data_chunks: 2,
        });
        let ordered = planner.plans_by_overhead();
        assert_eq!(ordered[0].overhead_ratio, 4.0);
        assert_eq!(ordered[1].overhead_ratio, 2.0);
    }

    #[test]
    fn capacity_planner_datasets_over_overhead() {
        let mut planner = ErasureCapacityPlanner::new();
        planner.add_plan(ErasureCapacityPlan {
            dataset_id: DatasetId::new([1u8; 32]),
            raw_data_bytes: 1000,
            encoded_bytes: 1500,
            overhead_bytes: 500,
            overhead_ratio: 1.5,
            failure_tolerance: 2,
            chunks: 6,
            data_chunks: 4,
        });
        planner.add_plan(ErasureCapacityPlan {
            dataset_id: DatasetId::new([2u8; 32]),
            raw_data_bytes: 1000,
            encoded_bytes: 4000,
            overhead_bytes: 3000,
            overhead_ratio: 4.0,
            failure_tolerance: 1,
            chunks: 8,
            data_chunks: 2,
        });
        let over = planner.datasets_over_overhead(2.0);
        assert_eq!(over.len(), 1);
        assert_eq!(over[0], DatasetId::new([2u8; 32]));
    }

    // ========== Integration tests ==========

    #[test]
    fn full_erasure_workflow_integration() {
        let mut mgr = ErasureThresholdManager::new();
        let mut planner = ErasureCapacityPlanner::new();
        let mut analyzer = ErasureTopologyAnalyzer::new();
        let dataset = DatasetId::new([1u8; 32]);

        mgr.register_coding_params(dataset, 4, 10, CodingScheme::SystematicReedSolomon)
            .unwrap();
        let plan = mgr.capacity_plan(dataset, 4096).unwrap();
        planner.add_plan(plan);

        let placement = mgr.place_chunks_across_domains(10, &[
            FailureDomainRef(1), FailureDomainRef(2), FailureDomainRef(3),
        ]).unwrap();
        let analysis = analyzer.analyze_placement(&placement);
        assert_eq!(analysis.total_chunks, 10);
        assert!(analysis.domains_used >= 1);

        mgr.mark_chunk_lost(dataset, 0);
        let sched = mgr.schedule_reconstruction(dataset).unwrap();
        mgr.complete_reconstruction(dataset);

        assert!(mgr.is_erasure_enabled(dataset));
        assert_eq!(mgr.healthy_chunk_count(dataset), 10);
    }

    // ========== AuditResult tests ==========

    #[test]
    fn audit_result_variants() {
        let results = [AuditResult::Pass, AuditResult::Partial, AuditResult::Fail];
        for r in results {
            let _ = format!("{:?}", r);
        }
    }

    // ========== RecoveryTaskStatus tests ==========

    #[test]
    fn recovery_task_status_variants() {
        let statuses = [
            RecoveryTaskStatus::Pending,
            RecoveryTaskStatus::InProgress,
            RecoveryTaskStatus::Completed,
            RecoveryTaskStatus::Failed,
            RecoveryTaskStatus::Cancelled,
        ];
        for s in statuses {
            let _ = format!("{:?}", s);
        }
    }

    // ========== RepairJobStatus tests ==========

    #[test]
    fn repair_job_status_variants() {
        let statuses = [
            RepairJobStatus::Queued,
            RepairJobStatus::InProgress,
            RepairJobStatus::Completed,
            RepairJobStatus::Failed,
        ];
        for s in statuses {
            let _ = format!("{:?}", s);
        }
    }

    // ========== RepairPriority tests ==========

    #[test]
    fn repair_priority_ordering() {
        assert!(RepairPriority::Critical > RepairPriority::High);
        assert!(RepairPriority::High > RepairPriority::Normal);
        assert!(RepairPriority::Normal > RepairPriority::Low);
    }

    // ========== StripeId tests ==========

    #[test]
    fn stripe_id_ordering() {
        let a = StripeId(1);
        let b = StripeId(2);
        assert!(a < b);
        assert_ne!(a, b);
    }

    // ========== RecoveryTaskId tests ==========

    #[test]
    fn recovery_task_id_ordering() {
        let a = RecoveryTaskId(10);
        let b = RecoveryTaskId(20);
        assert!(a < b);
        assert_ne!(a, b);
    }

    // ========== RepairJobId tests ==========

    #[test]
    fn repair_job_id_ordering() {
        let a = RepairJobId(5);
        let b = RepairJobId(15);
        assert!(a < b);
        assert_ne!(a, b);
    }

    // ========== ErasureStripingManager tests ==========

    #[test]
    fn striping_manager_create_stripe() {
        let mut mgr = ErasureStripingManager::new();
        let dataset = DatasetId::new([1u8; 32]);
        let id = mgr.create_stripe(
            dataset,
            vec![0, 1, 2, 3, 4],
            1_048_576,
            2,
            3,
        );
        let stripe = mgr.stripe(id).unwrap();
        assert_eq!(stripe.dataset_id, dataset);
        assert_eq!(stripe.chunk_indices.len(), 5);
        assert_eq!(stripe.parity_chunk_count, 2);
        assert_eq!(stripe.data_chunk_count, 3);
        assert_eq!(stripe.stripe_size_bytes, 1_048_576);
    }

    #[test]
    fn striping_manager_stripes_for_dataset() {
        let mut mgr = ErasureStripingManager::new();
        let d1 = DatasetId::new([1u8; 32]);
        let d2 = DatasetId::new([2u8; 32]);
        mgr.create_stripe(d1, vec![0, 1], 100, 0, 2);
        mgr.create_stripe(d1, vec![2, 3], 200, 0, 2);
        mgr.create_stripe(d2, vec![4, 5], 300, 0, 2);
        let d1_stripes = mgr.stripes_for_dataset(d1);
        assert_eq!(d1_stripes.len(), 2);
        let d2_stripes = mgr.stripes_for_dataset(d2);
        assert_eq!(d2_stripes.len(), 1);
        let nonexistent = mgr.stripes_for_dataset(DatasetId::new([9u8; 32]));
        assert_eq!(nonexistent.len(), 0);
    }

    #[test]
    fn striping_manager_default_size() {
        let mgr = ErasureStripingManager::new();
        assert_eq!(mgr.default_stripe_size(), 1_048_576);
    }

    #[test]
    fn striping_manager_set_default_size() {
        let mut mgr = ErasureStripingManager::new();
        mgr.set_default_stripe_size(4_194_304);
        assert_eq!(mgr.default_stripe_size(), 4_194_304);
        mgr.set_default_stripe_size(256 * 1024 * 1024);
        assert_eq!(mgr.default_stripe_size(), 256 * 1024 * 1024);
    }

    #[test]
    fn striping_manager_remove_stripe() {
        let mut mgr = ErasureStripingManager::new();
        let dataset = DatasetId::new([1u8; 32]);
        let id = mgr.create_stripe(dataset, vec![0, 1], 100, 0, 2);
        assert!(mgr.stripe(id).is_some());
        let removed = mgr.remove_stripe(id);
        assert!(removed.is_some());
        assert_eq!(removed.unwrap().stripe_id, id);
        assert!(mgr.stripe(id).is_none());
    }

    #[test]
    fn striping_manager_remove_nonexistent() {
        let mut mgr = ErasureStripingManager::new();
        assert!(mgr.remove_stripe(StripeId(999)).is_none());
    }

    #[test]
    fn striping_manager_total_striped_bytes() {
        let mut mgr = ErasureStripingManager::new();
        let dataset = DatasetId::new([1u8; 32]);
        mgr.create_stripe(dataset, vec![0, 1], 100, 0, 2);
        mgr.create_stripe(dataset, vec![2, 3], 200, 0, 2);
        mgr.create_stripe(dataset, vec![4, 5], 300, 0, 2);
        assert_eq!(mgr.total_striped_bytes(), 600);
        assert_eq!(mgr.stripe_count(), 3);
    }

    #[test]
    fn striping_manager_stripe_id_ordering() {
        let mut mgr = ErasureStripingManager::new();
        let dataset = DatasetId::new([1u8; 32]);
        let id1 = mgr.create_stripe(dataset, vec![0], 100, 0, 1);
        let id2 = mgr.create_stripe(dataset, vec![1], 200, 0, 1);
        assert!(id1.0 < id2.0);
        assert_ne!(id1, id2);
    }

    // ========== ErasureRepairScheduler tests ==========

    #[test]
    fn repair_scheduler_queue_job() {
        let mut sched = ErasureRepairScheduler::new();
        let dataset = DatasetId::new([1u8; 32]);
        let id = sched.queue_repair(dataset, 5, vec![0, 1, 2], RepairPriority::High);
        let jobs = sched.jobs_for_dataset(dataset);
        assert_eq!(jobs.len(), 1);
        let job = jobs.first().unwrap();
        assert_eq!(job.job_id, id);
        assert_eq!(job.lost_chunk_index, 5);
        assert_eq!(job.priority, RepairPriority::High);
        assert_eq!(job.status, RepairJobStatus::Queued);
        assert_eq!(job.helper_chunk_indices, vec![0, 1, 2]);
    }

    #[test]
    fn repair_scheduler_start_next_job_priority_order() {
        let mut sched = ErasureRepairScheduler::new();
        let dataset = DatasetId::new([1u8; 32]);
        let low_id = sched.queue_repair(dataset, 0, vec![1], RepairPriority::Low);
        let crit_id = sched.queue_repair(dataset, 1, vec![2], RepairPriority::Critical);
        let norm_id = sched.queue_repair(dataset, 2, vec![3], RepairPriority::Normal);

        let first = sched.start_next_job().unwrap();
        assert_eq!(first, crit_id);

        let second = sched.start_next_job().unwrap();
        assert_eq!(second, norm_id);

        let third = sched.start_next_job().unwrap();
        assert_eq!(third, low_id);

        assert!(sched.start_next_job().is_none());
    }

    #[test]
    fn repair_scheduler_max_concurrent() {
        let mut sched = ErasureRepairScheduler::new();
        let dataset = DatasetId::new([1u8; 32]);
        sched.set_max_concurrent(1);
        sched.queue_repair(dataset, 0, vec![1], RepairPriority::Normal);
        sched.queue_repair(dataset, 1, vec![2], RepairPriority::Normal);
        sched.start_next_job().unwrap();
        assert_eq!(sched.in_progress_count(), 1);
        assert!(sched.start_next_job().is_none());
    }

    #[test]
    fn repair_scheduler_complete_and_fail_job() {
        let mut sched = ErasureRepairScheduler::new();
        let dataset = DatasetId::new([1u8; 32]);
        let id = sched.queue_repair(dataset, 0, vec![1], RepairPriority::Normal);
        sched.start_next_job().unwrap();
        sched.complete_job(id).unwrap();
        assert_eq!(sched.completed_repairs(), 1);
        assert_eq!(sched.in_progress_count(), 0);
        assert_eq!(sched.total_jobs(), 1);

        // Fail job
        let id2 = sched.queue_repair(dataset, 1, vec![2], RepairPriority::Normal);
        sched.start_next_job().unwrap();
        sched.fail_job(id2, "helper unavailable".to_string()).unwrap();
        assert_eq!(sched.total_jobs(), 2);
        assert_eq!(sched.completed_repairs(), 1);
    }

    #[test]
    fn repair_scheduler_complete_nonexistent_job() {
        let mut sched = ErasureRepairScheduler::new();
        assert!(sched.complete_job(RepairJobId(999)).is_err());
        assert!(sched.fail_job(RepairJobId(999), "err".to_string()).is_err());
    }

    #[test]
    fn repair_scheduler_jobs_for_dataset() {
        let mut sched = ErasureRepairScheduler::new();
        let d1 = DatasetId::new([1u8; 32]);
        let d2 = DatasetId::new([2u8; 32]);
        sched.queue_repair(d1, 0, vec![1], RepairPriority::Normal);
        sched.queue_repair(d1, 1, vec![2], RepairPriority::Normal);
        sched.queue_repair(d2, 0, vec![1], RepairPriority::Normal);
        assert_eq!(sched.jobs_for_dataset(d1).len(), 2);
        assert_eq!(sched.jobs_for_dataset(d2).len(), 1);
    }

    #[test]
    fn repair_priority_ordering() {
        assert!(RepairPriority::Critical > RepairPriority::High);
        assert!(RepairPriority::High > RepairPriority::Normal);
        assert!(RepairPriority::Normal > RepairPriority::Low);
        assert!(RepairPriority::Low as u8 < RepairPriority::Critical as u8);
    }

    #[test]
    fn repair_job_status_variants() {
        let statuses = [
            RepairJobStatus::Queued,
            RepairJobStatus::InProgress,
            RepairJobStatus::Completed,
            RepairJobStatus::Failed,
        ];
        for s in statuses {
            let _ = format!("{:?}", s);
        }
    }

    // ========== ErasureTopologyAnalyzer tests ==========

    #[test]
    fn topology_analyzer_balanced_placement() {
        let analyzer = ErasureTopologyAnalyzer::new();
        let placement = vec![
            ChunkPlacement { chunk_index: 0, domain: FailureDomainRef(1), node_hint: None },
            ChunkPlacement { chunk_index: 1, domain: FailureDomainRef(2), node_hint: None },
            ChunkPlacement { chunk_index: 2, domain: FailureDomainRef(3), node_hint: None },
        ];
        let analysis = analyzer.analyze_placement(&placement);
        assert!(analysis.balanced);
        assert_eq!(analysis.domains_used, 3);
        assert_eq!(analysis.total_chunks, 3);
        assert_eq!(analysis.max_chunks_per_domain, 1);
        assert_eq!(analysis.min_chunks_per_domain, 1);
        assert!(analysis.diversity_score > 0.5);
    }

    #[test]
    fn topology_analyzer_unbalanced_placement() {
        let analyzer = ErasureTopologyAnalyzer::new();
        let placement = vec![
            ChunkPlacement { chunk_index: 0, domain: FailureDomainRef(1), node_hint: None },
            ChunkPlacement { chunk_index: 1, domain: FailureDomainRef(1), node_hint: None },
            ChunkPlacement { chunk_index: 2, domain: FailureDomainRef(2), node_hint: None },
        ];
        let analysis = analyzer.analyze_placement(&placement);
        assert!(!analysis.balanced);
        assert_eq!(analysis.max_chunks_per_domain, 2);
        assert_eq!(analysis.min_chunks_per_domain, 1);
        assert_eq!(analysis.domains_used, 2);
    }

    #[test]
    fn topology_analyzer_all_same_domain() {
        let analyzer = ErasureTopologyAnalyzer::new();
        let placement = vec![
            ChunkPlacement { chunk_index: 0, domain: FailureDomainRef(1), node_hint: None },
            ChunkPlacement { chunk_index: 1, domain: FailureDomainRef(1), node_hint: None },
        ];
        let analysis = analyzer.analyze_placement(&placement);
        assert!(!analysis.balanced);
        assert_eq!(analysis.domains_used, 1);
        assert!(!ErasureTopologyAnalyzer::is_resilient(&analysis));
    }

    #[test]
    fn topology_analyzer_empty_placement() {
        let analyzer = ErasureTopologyAnalyzer::new();
        let analysis = analyzer.analyze_placement(&[]);
        assert_eq!(analysis.total_chunks, 0);
        assert_eq!(analysis.domains_used, 0);
        assert_eq!(analysis.diversity_score, 0.0);
    }

    #[test]
    fn topology_analyzer_is_resilient_true() {
        let analyzer = ErasureTopologyAnalyzer::new();
        let placement = vec![
            ChunkPlacement { chunk_index: 0, domain: FailureDomainRef(1), node_hint: None },
            ChunkPlacement { chunk_index: 1, domain: FailureDomainRef(2), node_hint: None },
        ];
        let analysis = analyzer.analyze_placement(&placement);
        assert!(ErasureTopologyAnalyzer::is_resilient(&analysis));
    }

    #[test]
    fn topology_analyzer_recommended_min_domains() {
        assert_eq!(ErasureTopologyAnalyzer::recommended_min_domains(10, 3), 4);
        assert_eq!(ErasureTopologyAnalyzer::recommended_min_domains(10, 0), 2);
        assert_eq!(ErasureTopologyAnalyzer::recommended_min_domains(5, 10), 5);
        assert_eq!(ErasureTopologyAnalyzer::recommended_min_domains(1, 0), 2);
    }

    // ========== ErasureIntegrityAuditor tests ==========

    #[test]
    fn integrity_auditor_record_pass() {
        let mut auditor = ErasureIntegrityAuditor::new();
        let dataset = DatasetId::new([1u8; 32]);
        let id = auditor.record_audit(dataset, 10, 0, 0);
        let audit = auditor.audits_for_dataset(dataset).first().unwrap();
        assert_eq!(id, audit.audit_id);
        assert_eq!(audit.chunks_checked, 10);
        assert_eq!(audit.chunks_corrupt, 0);
        assert_eq!(audit.chunks_repaired, 0);
        assert_eq!(audit.result, AuditResult::Pass);
    }

    #[test]
    fn integrity_auditor_record_partial() {
        let mut auditor = ErasureIntegrityAuditor::new();
        let dataset = DatasetId::new([1u8; 32]);
        auditor.record_audit(dataset, 10, 3, 3);
        let audit = auditor.latest_audit().unwrap();
        assert_eq!(audit.result, AuditResult::Partial);
    }

    #[test]
    fn integrity_auditor_record_fail() {
        let mut auditor = ErasureIntegrityAuditor::new();
        let dataset = DatasetId::new([1u8; 32]);
        auditor.record_audit(dataset, 10, 5, 1);
        assert_eq!(auditor.failed_audits(), 1);
        assert_eq!(auditor.total_audits(), 1);
        let audit = auditor.latest_audit().unwrap();
        assert_eq!(audit.result, AuditResult::Fail);
    }

    #[test]
    fn integrity_auditor_pass_rate() {
        let mut auditor = ErasureIntegrityAuditor::new();
        let dataset = DatasetId::new([1u8; 32]);
        auditor.record_audit(dataset, 10, 0, 0);
        auditor.record_audit(dataset, 10, 5, 1);
        assert!((auditor.pass_rate() - 0.5).abs() < 0.001);
    }

    #[test]
    fn integrity_auditor_pass_rate_all_pass() {
        let mut auditor = ErasureIntegrityAuditor::new();
        let dataset = DatasetId::new([1u8; 32]);
        auditor.record_audit(dataset, 10, 0, 0);
        auditor.record_audit(dataset, 10, 0, 0);
        assert!((auditor.pass_rate() - 1.0).abs() < 0.001);
    }

    #[test]
    fn integrity_auditor_no_audits() {
        let auditor = ErasureIntegrityAuditor::new();
        assert_eq!(auditor.total_audits(), 0);
        assert_eq!(auditor.failed_audits(), 0);
        assert!((auditor.pass_rate() - 1.0).abs() < 0.001);
        assert!(auditor.latest_audit().is_none());
    }

    #[test]
    fn integrity_auditor_audits_for_dataset() {
        let mut auditor = ErasureIntegrityAuditor::new();
        let d1 = DatasetId::new([1u8; 32]);
        let d2 = DatasetId::new([2u8; 32]);
        auditor.record_audit(d1, 5, 0, 0);
        auditor.record_audit(d2, 5, 0, 0);
        auditor.record_audit(d1, 5, 1, 0);
        assert_eq!(auditor.audits_for_dataset(d1).len(), 2);
        assert_eq!(auditor.audits_for_dataset(d2).len(), 1);
    }

    #[test]
    fn audit_result_variants() {
        let results = [AuditResult::Pass, AuditResult::Partial, AuditResult::Fail];
        for r in results {
            let _ = format!("{:?}", r);
        }
    }

    // ========== ErasureCodeParametersValidator tests ==========

    #[test]
    fn validator_valid_params() {
        let validator = ErasureCodeParametersValidator::new();
        let result = validator.validate(4, 10, 8);
        assert!(result.valid);
        assert!(result.errors.is_empty());
    }

    #[test]
    fn validator_k_zero_invalid() {
        let validator = ErasureCodeParametersValidator::new();
        let result = validator.validate(0, 5, 8);
        assert!(!result.valid);
        assert!(!result.errors.is_empty());
    }

    #[test]
    fn validator_n_less_than_k_invalid() {
        let validator = ErasureCodeParametersValidator::new();
        let result = validator.validate(5, 3, 8);
        assert!(!result.valid);
    }

    #[test]
    fn validator_n_exceeds_max_invalid() {
        let validator = ErasureCodeParametersValidator::with_limits(5, 1, 8);
        let result = validator.validate(3, 10, 8);
        assert!(!result.valid);
    }

    #[test]
    fn validator_recommended_params() {
        let validator = ErasureCodeParametersValidator::new();
        let result = validator.validate(4, 7, 8);
        assert!(result.valid);
        assert_eq!(result.recommended_k, 4);
        assert_eq!(result.recommended_n, 7);
    }

    #[test]
    fn validator_is_valid_bool() {
        let validator = ErasureCodeParametersValidator::new();
        assert!(validator.is_valid(4, 10, 8));
        assert!(!validator.is_valid(0, 5, 8));
    }

    #[test]
    fn validator_set_limits() {
        let mut validator = ErasureCodeParametersValidator::new();
        validator.set_max_n(20);
        validator.set_min_k(2);
        assert_eq!(validator.max_n(), 20);
        assert_eq!(validator.min_k(), 2);
    }

    #[test]
    fn validator_warnings_low_tolerance() {
        let validator = ErasureCodeParametersValidator::new();
        let result = validator.validate(4, 5, 8);
        assert!(result.valid);
        assert!(!result.warnings.is_empty());
        assert!(matches!(
            result.warnings[0],
            ErasureParameterWarning::LowFaultTolerance { tolerance: 1 }
        ));
    }

    #[test]
    fn validator_warnings_high_overhead() {
        let validator = ErasureCodeParametersValidator::new();
        let result = validator.validate(2, 10, 8);
        assert!(result.valid);
        let has_overhead_warning = result.warnings.iter().any(|w| matches!(
            w,
            ErasureParameterWarning::HighStorageOverhead { .. }
        ));
        assert!(has_overhead_warning);
    }

    // ========== ParityMatrixGenerator tests ==========

    #[test]
    fn parity_matrix_generate_systematic() {
        let gen = ParityMatrixGenerator::new();
        let rows = gen.generate_systematic_parity_matrix(4, 10).unwrap();
        assert_eq!(rows.len(), 6);
        for row in &rows {
            assert_eq!(row.coefficients.len(), 10);
        }
    }

    #[test]
    fn parity_matrix_generate_invalid() {
        let gen = ParityMatrixGenerator::new();
        assert!(gen.generate_systematic_parity_matrix(0, 5).is_err());
        assert!(gen.generate_systematic_parity_matrix(10, 5).is_err());
    }

    #[test]
    fn parity_matrix_verify_row_rank() {
        let gen = ParityMatrixGenerator::new();
        let rows = gen.generate_systematic_parity_matrix(4, 8).unwrap();
        assert!(gen.verify_row_rank(&rows));
    }

    #[test]
    fn parity_row_count() {
        assert_eq!(ParityMatrixGenerator::parity_row_count(4, 10), 6);
        assert_eq!(ParityMatrixGenerator::parity_row_count(5, 5), 0);
        assert_eq!(ParityMatrixGenerator::parity_row_count(100, 200), 100);
    }

    // ========== FragmentRecoveryCoordinator tests ==========

    #[test]
    fn recovery_coordinator_create_task() {
        let mut coord = FragmentRecoveryCoordinator::new();
        let dataset = DatasetId::new([1u8; 32]);
        let id = coord.create_recovery_task(dataset, vec![0, 1], vec![2, 3, 4]);
        assert_eq!(id, RecoveryTaskId(0));
        let tasks = coord.tasks_for_dataset(dataset);
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].status, RecoveryTaskStatus::Pending);
        assert_eq!(tasks[0].missing_chunk_indices, vec![0, 1]);
        assert_eq!(tasks[0].source_chunk_indices, vec![2, 3, 4]);
    }

    #[test]
    fn recovery_coordinator_start_and_complete() {
        let mut coord = FragmentRecoveryCoordinator::new();
        let dataset = DatasetId::new([1u8; 32]);
        let id = coord.create_recovery_task(dataset, vec![0], vec![1]);
        coord.start_recovery_task(id).unwrap();
        coord.complete_recovery_task(id).unwrap();
        assert_eq!(coord.completed_count(), 1);
        let task = coord.tasks_for_dataset(dataset).first().unwrap();
        assert_eq!(task.status, RecoveryTaskStatus::Completed);
    }

    #[test]
    fn recovery_coordinator_fail_task() {
        let mut coord = FragmentRecoveryCoordinator::new();
        let dataset = DatasetId::new([1u8; 32]);
        let id = coord.create_recovery_task(dataset, vec![0], vec![1]);
        coord.start_recovery_task(id).unwrap();
        coord.fail_recovery_task(id).unwrap();
        assert_eq!(coord.failed_count(), 1);
        let task = coord.tasks_for_dataset(dataset).first().unwrap();
        assert_eq!(task.status, RecoveryTaskStatus::Failed);
    }

    #[test]
    fn recovery_coordinator_pending_tasks() {
        let mut coord = FragmentRecoveryCoordinator::new();
        let dataset = DatasetId::new([1u8; 32]);
        coord.create_recovery_task(dataset, vec![0], vec![1]);
        coord.create_recovery_task(dataset, vec![1], vec![2]);
        assert_eq!(coord.pending_tasks().len(), 2);
    }

    #[test]
    fn recovery_coordinator_max_concurrent() {
        let mut coord = FragmentRecoveryCoordinator::new();
        let dataset = DatasetId::new([1u8; 32]);
        coord.set_max_concurrent(1);
        coord.create_recovery_task(dataset, vec![0], vec![1]);
        coord.create_recovery_task(dataset, vec![1], vec![2]);
        coord.start_recovery_task(RecoveryTaskId(0)).unwrap();
        assert!(coord.start_recovery_task(RecoveryTaskId(1)).is_err());
    }

    #[test]
    fn recovery_coordinator_start_nonexistent_task() {
        let mut coord = FragmentRecoveryCoordinator::new();
        assert!(coord.start_recovery_task(RecoveryTaskId(999)).is_err());
        assert!(coord.complete_recovery_task(RecoveryTaskId(999)).is_err());
        assert!(coord.fail_recovery_task(RecoveryTaskId(999)).is_err());
    }

    // ========== ChunkLossPatternAnalyzer tests ==========

    #[test]
    fn loss_analyzer_survivable() {
        let mut analyzer = ChunkLossPatternAnalyzer::new();
        analyzer.assign_chunk_domain(0, FailureDomainRef(1));
        analyzer.assign_chunk_domain(1, FailureDomainRef(2));
        analyzer.assign_chunk_domain(2, FailureDomainRef(3));
        analyzer.assign_chunk_domain(3, FailureDomainRef(1));
        analyzer.assign_chunk_domain(4, FailureDomainRef(2));
        let result = analyzer.analyze_loss(5, &[0, 1], 3);
        assert!(result.survivable);
        assert_eq!(result.surviving_chunks, 3);
        assert_eq!(result.lost_chunks, 2);
        assert_eq!(result.total_chunks, 5);
    }

    #[test]
    fn loss_analyzer_not_survivable() {
        let mut analyzer = ChunkLossPatternAnalyzer::new();
        for i in 0..5 {
            analyzer.assign_chunk_domain(i, FailureDomainRef(1));
        }
        let result = analyzer.analyze_loss(5, &[0, 1, 2, 3], 3);
        assert!(!result.survivable);
        assert_eq!(result.surviving_chunks, 1);
        assert_eq!(result.worst_domain_loss, 4);
    }

    #[test]
    fn loss_analyzer_domain_loss_distribution() {
        let mut analyzer = ChunkLossPatternAnalyzer::new();
        analyzer.assign_chunk_domain(0, FailureDomainRef(1));
        analyzer.assign_chunk_domain(1, FailureDomainRef(1));
        analyzer.assign_chunk_domain(2, FailureDomainRef(2));
        let result = analyzer.analyze_loss(3, &[0, 1, 2], 1);
        assert_eq!(result.worst_domain_loss, 2);
        assert_eq!(result.loss_by_domain.get(&FailureDomainRef(1)), Some(&2));
        assert_eq!(result.loss_by_domain.get(&FailureDomainRef(2)), Some(&1));
    }

    #[test]
    fn loss_analyzer_recommended_action_reconstruct_parity() {
        let mut analyzer = ChunkLossPatternAnalyzer::new();
        analyzer.assign_chunk_domain(0, FailureDomainRef(1));
        analyzer.assign_chunk_domain(1, FailureDomainRef(2));
        let result = analyzer.analyze_loss(2, &[0], 1);
        assert_eq!(result.recommended_action, RecoveryAction::ReconstructFromParity);
    }

    #[test]
    fn loss_analyzer_recommended_action_declare_loss() {
        let mut analyzer = ChunkLossPatternAnalyzer::new();
        analyzer.assign_chunk_domain(0, FailureDomainRef(1));
        analyzer.assign_chunk_domain(1, FailureDomainRef(1));
        analyzer.assign_chunk_domain(2, FailureDomainRef(2));
        analyzer.assign_chunk_domain(3, FailureDomainRef(1));
        analyzer.assign_chunk_domain(4, FailureDomainRef(1));
        let result = analyzer.analyze_loss(5, &[0, 1, 2, 3, 4], 2);
        assert_eq!(result.recommended_action, RecoveryAction::DeclareDataLoss);
    }

    #[test]
    fn loss_analyzer_no_loss() {
        let mut analyzer = ChunkLossPatternAnalyzer::new();
        analyzer.assign_chunk_domain(0, FailureDomainRef(1));
        let result = analyzer.analyze_loss(1, &[], 1);
        assert!(result.survivable);
        assert_eq!(result.lost_chunks, 0);
        assert_eq!(result.surviving_chunks, 1);
        assert_eq!(result.worst_domain_loss, 0);
    }

    // ========== ErasureTopologyConstraintChecker tests ==========

    #[test]
    fn topology_checker_valid_placement() {
        let checker = ErasureTopologyConstraintChecker::new();
        let placement = vec![
            ChunkPlacement { chunk_index: 0, domain: FailureDomainRef(1), node_hint: None },
            ChunkPlacement { chunk_index: 1, domain: FailureDomainRef(2), node_hint: None },
            ChunkPlacement { chunk_index: 2, domain: FailureDomainRef(3), node_hint: None },
        ];
        let constraint = ErasureTopologyConstraintChecker::default_constraints();
        assert!(checker.check_placement(&placement, &constraint));
    }

    #[test]
    fn topology_checker_invalid_too_many_per_domain() {
        let checker = ErasureTopologyConstraintChecker::new();
        let placement = vec![
            ChunkPlacement { chunk_index: 0, domain: FailureDomainRef(1), node_hint: None },
            ChunkPlacement { chunk_index: 1, domain: FailureDomainRef(1), node_hint: None },
            ChunkPlacement { chunk_index: 2, domain: FailureDomainRef(1), node_hint: None },
            ChunkPlacement { chunk_index: 3, domain: FailureDomainRef(1), node_hint: None },
        ];
        let constraint = ErasureTopologyConstraintChecker::default_constraints();
        assert!(!checker.check_placement(&placement, &constraint));
    }

    #[test]
    fn topology_checker_invalid_too_few_domains() {
        let checker = ErasureTopologyConstraintChecker::new();
        let placement = vec![
            ChunkPlacement { chunk_index: 0, domain: FailureDomainRef(1), node_hint: None },
            ChunkPlacement { chunk_index: 1, domain: FailureDomainRef(1), node_hint: None },
        ];
        let constraint = ErasureTopologyConstraintChecker::default_constraints();
        assert!(!checker.check_placement(&placement, &constraint));
    }

    #[test]
    fn topology_checker_default_constraints() {
        let constraint = ErasureTopologyConstraintChecker::default_constraints();
        assert_eq!(constraint.max_chunks_per_domain, 3);
        assert_eq!(constraint.min_domains, 3);
        assert!(constraint.require_region_diversity);
    }

    // ========== DecodingScheduleBuilder tests ==========

    #[test]
    fn decoding_schedule_build() {
        let builder = DecodingScheduleBuilder::new();
        let dataset = DatasetId::new([1u8; 32]);
        let schedule = builder.build(dataset, &[0, 1], &[2, 3, 4, 5]);
        assert_eq!(schedule.target_chunks.len(), 2);
        assert_eq!(schedule.steps.len(), 2);
        assert_eq!(schedule.dataset_id, dataset);
    }

    #[test]
    fn decoding_schedule_step_count() {
        let builder = DecodingScheduleBuilder::new();
        let dataset = DatasetId::new([1u8; 32]);
        let schedule = builder.build(dataset, &[0, 1, 2], &[3, 4]);
        assert_eq!(DecodingScheduleBuilder::step_count(&schedule), 3);
    }

    // ========== IntegrityVerifier tests ==========

    #[test]
    fn integrity_verifier_register_and_verify() {
        let mut verifier = IntegrityVerifier::new();
        let dataset = DatasetId::new([1u8; 32]);
        verifier.register_checksum(dataset, 0, 42);
        let result = verifier.verify_chunk(dataset, 0, 42);
        assert!(result.verified);
        assert!(result.checksum_match);
    }

    #[test]
    fn integrity_verifier_mismatch() {
        let mut verifier = IntegrityVerifier::new();
        let dataset = DatasetId::new([1u8; 32]);
        verifier.register_checksum(dataset, 0, 42);
        let result = verifier.verify_chunk(dataset, 0, 99);
        assert!(!result.verified);
        assert!(!result.checksum_match);
    }

    #[test]
    fn integrity_verifier_failure_rate() {
        let mut verifier = IntegrityVerifier::new();
        let dataset = DatasetId::new([1u8; 32]);
        verifier.register_checksum(dataset, 0, 1);
        verifier.register_checksum(dataset, 1, 2);
        verifier.verify_chunk(dataset, 0, 1);
        verifier.verify_chunk(dataset, 1, 99);
        assert!((verifier.failure_rate() - 0.5).abs() < 0.001);
    }

    #[test]
    fn integrity_verifier_no_checksums() {
        let mut verifier = IntegrityVerifier::new();
        let dataset = DatasetId::new([1u8; 32]);
        let result = verifier.verify_chunk(dataset, 0, 0);
        // No registered checksum means expected is 0, actual is 0 => match
        assert!(result.checksum_match);
    }

    #[test]
    fn integrity_verifier_counts() {
        let mut verifier = IntegrityVerifier::new();
        let dataset = DatasetId::new([1u8; 32]);
        verifier.register_checksum(dataset, 0, 1);
        verifier.verify_chunk(dataset, 0, 1);
        assert_eq!(verifier.verification_count(), 1);
        assert_eq!(verifier.failure_count(), 0);
        verifier.verify_chunk(dataset, 0, 2);
        assert_eq!(verifier.verification_count(), 2);
        assert_eq!(verifier.failure_count(), 1);
    }

    // ========== CodingSchemeSelector tests ==========

    #[test]
    fn coding_scheme_selector_select() {
        let selector = CodingSchemeSelector;
        assert_eq!(selector.select_scheme(5, 3.0, false), CodingScheme::Regenerating);
        assert_eq!(selector.select_scheme(1, 1.5, false), CodingScheme::XorParity);
        assert_eq!(selector.select_scheme(1, 3.0, true), CodingScheme::LocallyRepairable);
        assert_eq!(selector.select_scheme(2, 3.0, false), CodingScheme::SystematicReedSolomon);
    }

    #[test]
    fn coding_scheme_selector_description() {
        let desc = CodingSchemeSelector::scheme_description(CodingScheme::ReedSolomon);
        assert!(!desc.is_empty());
        assert!(desc.contains("Reed-Solomon"));
        let desc = CodingSchemeSelector::scheme_description(CodingScheme::Hybrid);
        assert!(!desc.is_empty());
        let desc = CodingSchemeSelector::scheme_description(CodingScheme::XorParity);
        assert!(!desc.is_empty());
    }

    // ========== ErasureCapacityPlanner extended tests ==========

    #[test]
    fn capacity_planner_plan_for_dataset() {
        let mut planner = ErasureCapacityPlanner::new();
        let dataset = DatasetId::new([1u8; 32]);
        planner.add_plan(ErasureCapacityPlan {
            dataset_id: dataset,
            raw_data_bytes: 1000,
            encoded_bytes: 2000,
            overhead_bytes: 1000,
            overhead_ratio: 2.0,
            failure_tolerance: 3,
            chunks: 10,
            data_chunks: 4,
        });
        let plan = planner.plan_for_dataset(dataset).unwrap();
        assert_eq!(plan.overhead_ratio, 2.0);
        assert_eq!(plan.failure_tolerance, 3);
    }

    #[test]
    fn capacity_planner_remove_plan() {
        let mut planner = ErasureCapacityPlanner::new();
        let dataset = DatasetId::new([1u8; 32]);
        planner.add_plan(ErasureCapacityPlan {
            dataset_id: dataset,
            raw_data_bytes: 1000,
            encoded_bytes: 2000,
            overhead_bytes: 1000,
            overhead_ratio: 2.0,
            failure_tolerance: 3,
            chunks: 10,
            data_chunks: 4,
        });
        assert!(planner.remove_plan(dataset).is_some());
        assert!(planner.plan_for_dataset(dataset).is_none());
        assert!(planner.remove_plan(dataset).is_none());
    }

    #[test]
    fn capacity_planner_plans_by_overhead() {
        let mut planner = ErasureCapacityPlanner::new();
        planner.add_plan(ErasureCapacityPlan {
            dataset_id: DatasetId::new([1u8; 32]),
            raw_data_bytes: 1000,
            encoded_bytes: 2000,
            overhead_bytes: 1000,
            overhead_ratio: 2.0,
            failure_tolerance: 3,
            chunks: 10,
            data_chunks: 4,
        });
        planner.add_plan(ErasureCapacityPlan {
            dataset_id: DatasetId::new([2u8; 32]),
            raw_data_bytes: 1000,
            encoded_bytes: 4000,
            overhead_bytes: 3000,
            overhead_ratio: 4.0,
            failure_tolerance: 1,
            chunks: 8,
            data_chunks: 2,
        });
        let ordered = planner.plans_by_overhead();
        assert_eq!(ordered.len(), 2);
        assert_eq!(ordered[0].overhead_ratio, 4.0);
        assert_eq!(ordered[1].overhead_ratio, 2.0);
    }

    #[test]
    fn capacity_planner_datasets_over_overhead() {
        let mut planner = ErasureCapacityPlanner::new();
        planner.add_plan(ErasureCapacityPlan {
            dataset_id: DatasetId::new([1u8; 32]),
            raw_data_bytes: 1000,
            encoded_bytes: 1500,
            overhead_bytes: 500,
            overhead_ratio: 1.5,
            failure_tolerance: 2,
            chunks: 6,
            data_chunks: 4,
        });
        planner.add_plan(ErasureCapacityPlan {
            dataset_id: DatasetId::new([2u8; 32]),
            raw_data_bytes: 1000,
            encoded_bytes: 4000,
            overhead_bytes: 3000,
            overhead_ratio: 4.0,
            failure_tolerance: 1,
            chunks: 8,
            data_chunks: 2,
        });
        let over = planner.datasets_over_overhead(2.0);
        assert_eq!(over.len(), 1);
        assert_eq!(over[0], DatasetId::new([2u8; 32]));
    }

    #[test]
    fn capacity_planner_empty() {
        let planner = ErasureCapacityPlanner::new();
        assert_eq!(planner.dataset_count(), 0);
        assert_eq!(planner.plans_by_overhead().len(), 0);
        assert_eq!(planner.datasets_over_overhead(0.0).len(), 0);
    }

    // ========== Integration tests ==========

    #[test]
    fn full_erasure_workflow_integration() {
        let mut mgr = ErasureThresholdManager::new();
        let mut planner = ErasureCapacityPlanner::new();
        let mut analyzer = ErasureTopologyAnalyzer::new();
        let dataset = DatasetId::new([1u8; 32]);

        mgr.register_coding_params(dataset, 4, 10, CodingScheme::SystematicReedSolomon)
            .unwrap();
        let plan = mgr.capacity_plan(dataset, 4096).unwrap();
        planner.add_plan(plan);

        let placement = mgr.place_chunks_across_domains(10, &[
            FailureDomainRef(1), FailureDomainRef(2), FailureDomainRef(3),
        ]).unwrap();
        let analysis = analyzer.analyze_placement(&placement);
        assert_eq!(analysis.total_chunks, 10);
        assert!(analysis.domains_used >= 1);

        mgr.mark_chunk_lost(dataset, 0);
        let sched = mgr.schedule_reconstruction(dataset).unwrap();
        mgr.complete_reconstruction(dataset);

        assert!(mgr.is_erasure_enabled(dataset));
        assert_eq!(mgr.healthy_chunk_count(dataset), 10);
    }

    // ========== DatasetId tests ==========

    #[test]
    fn dataset_id_equality() {
        let a = DatasetId::new([1u8; 32]);
        let b = DatasetId::new([1u8; 32]);
        let c = DatasetId::new([2u8; 32]);
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    // ========== StripeId tests ==========

    #[test]
    fn stripe_id_ordering_test() {
        let a = StripeId(1);
        let b = StripeId(2);
        assert!(a < b);
        assert_ne!(a, b);
    }

    // ========== RecoveryTaskId tests ==========

    #[test]
    fn recovery_task_id_ordering() {
        let a = RecoveryTaskId(10);
        let b = RecoveryTaskId(20);
        assert!(a < b);
        assert_ne!(a, b);
    }

    // ========== RecoveryTaskStatus tests ==========

    #[test]
    fn recovery_task_status_variants() {
        let statuses = [
            RecoveryTaskStatus::Pending,
            RecoveryTaskStatus::InProgress,
            RecoveryTaskStatus::Completed,
            RecoveryTaskStatus::Failed,
            RecoveryTaskStatus::Cancelled,
        ];
        for s in statuses {
            let _ = format!("{:?}", s);
        }
    }

    // ========== ErasureThreshold bit tests ==========

    #[test]
    fn erasure_threshold_bit_position() {
        assert_eq!(ERASURE_THRESHOLD_BIT, 1 << 8);
    }

    #[test]
    fn erasure_threshold_enabled_checks_bit() {
        let mut bits = EdgeCaseBits(0);
        assert!(!erasure_threshold_enabled(&bits));
        bits.enable(ERASURE_THRESHOLD_BIT);
        assert!(erasure_threshold_enabled(&bits));
    }
}

///////////////////////////////////////////////////////////////////////////////
// END OF FILE
///////////////////////////////////////////////////////////////////////////////