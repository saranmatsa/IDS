//! # ids-000-aa Contract: Routing
//!
//! `trait Routing` — logical → physical request resolution (locality-aware).
//! Carrier-agnostic: operates on opaque Endpoints and IDs only.

use crate::types::{
    ConsistencyProfile, DatasetId, Endpoint, OrderingPos, RecordId, RegionId,
};
use crate::errors::RoutingError;

///////////////////////////////////////////////////////////////////////////////
// SEALED TRAIT: Routing
///////////////////////////////////////////////////////////////////////////////

/// Logical-to-physical request resolver with locality awareness.
/// Sealed: only ids-000-aa and designated impl crates can implement.
pub trait Routing: private::Sealed {
    /// Resolve a logical (dataset, record) address to a physical endpoint.
    fn resolve_logical(
        &self,
        dataset_id: &DatasetId,
        record_id: &RecordId,
    ) -> Result<Endpoint, RoutingError>;

    /// Select the best endpoint from candidates using locality-aware routing.
    fn locality_aware_select(
        &self,
        candidates: Vec<Endpoint>,
        profile: ConsistencyProfile,
    ) -> Result<Endpoint, RoutingError>;

    /// Fall back to a replica-aware endpoint when the primary is unavailable.
    fn replica_aware_fallback(
        &self,
        primary: Endpoint,
        profile: ConsistencyProfile,
    ) -> Result<Endpoint, RoutingError>;

    /// Get the closest replica to a given endpoint for a dataset.
    fn closest_replica(
        &self,
        dataset_id: &DatasetId,
        from: Endpoint,
    ) -> Result<Vec<Endpoint>, RoutingError>;

    /// Get all replicas for a dataset.
    fn all_replicas(&self, dataset_id: &DatasetId) -> Result<Vec<Endpoint>, RoutingError>;

    /// Check if an endpoint is reachable for routing purposes.
    fn is_reachable(&self, endpoint: &Endpoint) -> bool {
        true
    }

    /// Get the routing table size for a dataset.
    fn routing_table_size(&self, dataset_id: &DatasetId) -> usize {
        0
    }

    /// Get the number of hops to an endpoint.
    fn hop_count(&self, endpoint: &Endpoint) -> u32 {
        1
    }

    /// Get the latency estimate to an endpoint in milliseconds.
    fn latency_ms(&self, endpoint: &Endpoint) -> u64 {
        10
    }

    /// Get the routing priority for an endpoint.
    fn endpoint_priority(&self, endpoint: &Endpoint) -> u32 {
        0
    }

    /// Check if routing is enabled for a dataset.
    fn is_routing_enabled(&self, dataset_id: &DatasetId) -> bool {
        true
    }

    /// Get the routing region for an endpoint.
    fn endpoint_region(&self, endpoint: &Endpoint) -> Option<RegionId> {
        None
    }

    /// Register a new route for a dataset.
    fn register_route(
        &self,
        dataset_id: &DatasetId,
        endpoint: Endpoint,
    ) -> Result<(), RoutingError>;

    /// Remove a route for a dataset.
    fn remove_route(
        &self,
        dataset_id: &DatasetId,
        endpoint: &Endpoint,
    ) -> Result<(), RoutingError>;

    /// Get the routing name for debugging.
    fn routing_name(&self) -> &'static str {
        "routing"
    }

    /// Check if the router is healthy.
    fn is_healthy(&self) -> bool {
        true
    }

    /// Get total routes registered.
    fn total_routes(&self) -> u64 {
        0
    }

    /// Get total resolutions performed.
    fn total_resolutions(&self) -> u64 {
        0
    }

    /// Flush pending route updates.
    fn flush(&self) -> Result<(), RoutingError> {
        Ok(())
    }

    /// Reset all routing state.
    fn reset(&self) -> Result<(), RoutingError> {
        Ok(())
    }
}

///////////////////////////////////////////////////////////////////////////////
// SEALED SUPERTRAIT MODULE
///////////////////////////////////////////////////////////////////////////////

mod private {
    /// Sealed supertrait — prevents downstream crates from implementing Routing.
    pub trait Sealed {}
    impl Sealed for crate::types::DatasetId {}
}

///////////////////////////////////////////////////////////////////////////////
// ERROR TYPE: RoutingError
///////////////////////////////////////////////////////////////////////////////

/// Errors returned by routing operations.
#[derive(Debug, thiserror::Error)]
pub enum RoutingError {
    #[error("route not found: dataset={dataset}, record={record}")]
    RouteNotFound { dataset: DatasetId, record: RecordId },

    #[error("endpoint unreachable: {endpoint:?}")]
    EndpointUnreachable { endpoint: Endpoint },

    #[error("no candidates available for dataset: {dataset}")]
    NoCandidates { dataset: DatasetId },

    #[error("routing table full: max {max} routes")]
    RoutingTableFull { max: usize },

    #[error("duplicate route: dataset={dataset}, endpoint={endpoint:?}")]
    DuplicateRoute { dataset: DatasetId, endpoint: Endpoint },

    #[error("resolve failed: dataset={dataset}, record={record}, reason={reason}")]
    ResolveFailed {
        dataset: DatasetId,
        record: RecordId,
        reason: String,
    },

    #[error("locality selection failed: reason={reason}")]
    LocalitySelectionFailed { reason: String },

    #[error("replica fallback failed: primary={primary:?}, reason={reason}")]
    ReplicaFallbackFailed { primary: Endpoint, reason: String },

    #[error("route registration failed: reason={reason}")]
    RouteRegistrationFailed { reason: String },

    #[error("route removal failed: reason={reason}")]
    RouteRemovalFailed { reason: String },

    #[error("routing not initialized")]
    NotInitialized,

    #[error("routing already initialized")]
    AlreadyInitialized,

    #[error("flush failed: {reason}")]
    FlushFailed { reason: String },

    #[error("reset failed: {reason}")]
    ResetFailed { reason: String },

    #[error("routing internal error: {message}")]
    Internal { message: String },

    #[error("invalid endpoint: {endpoint:?}")]
    InvalidEndpoint { endpoint: Endpoint },
}

impl RoutingError {
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            RoutingError::ResolveFailed { .. }
                | RoutingError::LocalitySelectionFailed { .. }
                | RoutingError::ReplicaFallbackFailed { .. }
                | RoutingError::RouteRegistrationFailed { .. }
                | RoutingError::FlushFailed { .. }
                | RoutingError::ResetFailed { .. }
                | RoutingError::Internal { .. }
        )
    }

    pub fn is_fatal(&self) -> bool {
        !self.is_retryable()
    }

    pub fn category(&self) -> &'static str {
        match self {
            RoutingError::RouteNotFound { .. } => "route_not_found",
            RoutingError::EndpointUnreachable { .. } => "endpoint_unreachable",
            RoutingError::NoCandidates { .. } => "no_candidates",
            RoutingError::RoutingTableFull { .. } => "routing_table_full",
            RoutingError::DuplicateRoute { .. } => "duplicate_route",
            RoutingError::ResolveFailed { .. } => "resolve_failed",
            RoutingError::LocalitySelectionFailed { .. } => "locality_selection_failed",
            RoutingError::ReplicaFallbackFailed { .. } => "replica_fallback_failed",
            RoutingError::RouteRegistrationFailed { .. } => "route_registration_failed",
            RoutingError::RouteRemovalFailed { .. } => "route_removal_failed",
            RoutingError::NotInitialized => "not_initialized",
            RoutingError::AlreadyInitialized => "already_initialized",
            RoutingError::FlushFailed { .. } => "flush_failed",
            RoutingError::ResetFailed { .. } => "reset_failed",
            RoutingError::Internal { .. } => "internal",
            RoutingError::InvalidEndpoint { .. } => "invalid_endpoint",
        }
    }

    pub fn plain_message(&self) -> String {
        match self {
            RoutingError::RouteNotFound { dataset, record } => {
                format!("route not found: dataset={dataset}, record={record}")
            }
            RoutingError::EndpointUnreachable { endpoint } => {
                format!("endpoint unreachable: {endpoint:?}")
            }
            RoutingError::NoCandidates { dataset } => {
                format!("no candidates available for dataset: {dataset}")
            }
            RoutingError::RoutingTableFull { max } => {
                format!("routing table full: max {max} routes")
            }
            RoutingError::DuplicateRoute { dataset, endpoint } => {
                format!("duplicate route: dataset={dataset}, endpoint={endpoint:?}")
            }
            RoutingError::ResolveFailed {
                dataset,
                record,
                reason,
            } => format!(
                "resolve failed: dataset={dataset}, record={record}, reason={reason}"
            ),
            RoutingError::LocalitySelectionFailed { reason } => {
                format!("locality selection failed: reason={reason}")
            }
            RoutingError::ReplicaFallbackFailed { primary, reason } => {
                format!(
                    "replica fallback failed: primary={primary:?}, reason={reason}"
                )
            }
            RoutingError::RouteRegistrationFailed { reason } => {
                format!("route registration failed: reason={reason}")
            }
            RoutingError::RouteRemovalFailed { reason } => {
                format!("route removal failed: reason={reason}")
            }
            RoutingError::NotInitialized => "routing not initialized".to_string(),
            RoutingError::AlreadyInitialized => "routing already initialized".to_string(),
            RoutingError::FlushFailed { reason } => {
                format!("flush failed: {reason}")
            }
            RoutingError::ResetFailed { reason } => {
                format!("reset failed: {reason}")
            }
            RoutingError::Internal { message } => {
                format!("routing internal error: {message}")
            }
            RoutingError::InvalidEndpoint { endpoint } => {
                format!("invalid endpoint: {endpoint:?}")
            }
        }
    }
}

///////////////////////////////////////////////////////////////////////////////
// ROUTE ENTRY
///////////////////////////////////////////////////////////////////////////////

/// A single routing table entry mapping a logical address to a physical endpoint.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RouteEntry {
    pub dataset_id: DatasetId,
    pub record_id: RecordId,
    pub endpoint: Endpoint,
    pub priority: u32,
    pub hop_count: u32,
    pub latency_ms: u64,
    pub region: Option<RegionId>,
    pub last_used_ms: u64,
    pub use_count: u64,
}

impl RouteEntry {
    pub fn new(
        dataset_id: DatasetId,
        record_id: RecordId,
        endpoint: Endpoint,
    ) -> Self {
        Self {
            dataset_id,
            record_id,
            endpoint,
            priority: 0,
            hop_count: 1,
            latency_ms: 10,
            region: None,
            last_used_ms: 0,
            use_count: 0,
        }
    }

    pub fn with_priority(mut self, priority: u32) -> Self {
        self.priority = priority;
        self
    }

    pub fn with_hop_count(mut self, hops: u32) -> Self {
        self.hop_count = hops;
        self
    }

    pub fn with_latency(mut self, ms: u64) -> Self {
        self.latency_ms = ms;
        self
    }

    pub fn with_region(mut self, region: RegionId) -> Self {
        self.region = Some(region);
        self
    }

    pub fn record_use(&mut self, now_ms: u64) {
        self.use_count += 1;
        self.last_used_ms = now_ms;
    }

    pub fn age_ms(&self, now_ms: u64) -> u64 {
        now_ms.saturating_sub(self.last_used_ms)
    }

    pub fn is_stale(&self, now_ms: u64, threshold_ms: u64) -> bool {
        self.age_ms(now_ms) > threshold_ms
    }
}

///////////////////////////////////////////////////////////////////////////////
// ROUTE TABLE
///////////////////////////////////////////////////////////////////////////////

/// Routing table storing route entries indexed by (dataset, record).
pub struct RouteTable {
    pub routes: std::cell::RefCell<
        std::collections::HashMap<(DatasetId, RecordId), RouteEntry>,
    >,
    pub max_routes: usize,
    pub total_lookups: u64,
    pub total_hits: u64,
    pub total_misses: u64,
}

impl RouteTable {
    pub fn new(max_routes: usize) -> Self {
        Self {
            routes: std::cell::RefCell::new(std::collections::HashMap::new()),
            max_routes,
            total_lookups: 0,
            total_hits: 0,
            total_misses: 0,
        }
    }

    pub fn insert(&self, entry: RouteEntry) -> Result<(), RoutingError> {
        let key = (entry.dataset_id.clone(), entry.record_id.clone());
        if self.routes.borrow().len() >= self.max_routes
            && !self.routes.borrow().contains_key(&key)
        {
            return Err(RoutingError::RoutingTableFull {
                max: self.max_routes,
            });
        }
        self.routes.borrow_mut().insert(key, entry);
        Ok(())
    }

    pub fn lookup(
        &self,
        dataset_id: &DatasetId,
        record_id: &RecordId,
    ) -> Option<RouteEntry> {
        self.total_lookups += 1;
        let key = (dataset_id.clone(), record_id.clone());
        match self.routes.borrow().get(&key) {
            Some(entry) => {
                self.total_hits += 1;
                Some(entry.clone())
            }
            None => {
                self.total_misses += 1;
                None
            }
        }
    }

    pub fn remove(&self, dataset_id: &DatasetId, record_id: &RecordId) -> bool {
        let key = (dataset_id.clone(), record_id.clone());
        self.routes.borrow_mut().remove(&key).is_some()
    }

    pub fn len(&self) -> usize {
        self.routes.borrow().len()
    }

    pub fn is_empty(&self) -> bool {
        self.routes.borrow().is_empty()
    }

    pub fn is_full(&self) -> bool {
        self.routes.borrow().len() >= self.max_routes
    }

    pub fn hit_rate(&self) -> f64 {
        if self.total_lookups == 0 {
            return 0.0;
        }
        self.total_hits as f64 / self.total_lookups as f64
    }

    pub fn clear(&self) {
        self.routes.borrow_mut().clear();
    }
}

impl Default for RouteTable {
    fn default() -> Self {
        Self::new(100000)
    }
}

///////////////////////////////////////////////////////////////////////////////
// LOCALITY METRICS
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LocalityMetrics {
    pub geographic_distance_km: f64,
    pub network_hops: u32,
    pub estimated_latency_ms: u64,
    pub load_factor: f64,
    pub health_score: f64,
}

impl LocalityMetrics {
    pub fn new() -> Self {
        Self {
            geographic_distance_km: 0.0,
            network_hops: 1,
            estimated_latency_ms: 10,
            load_factor: 0.0,
            health_score: 1.0,
        }
    }

    pub fn score(&self) -> f64 {
        // Lower is better; convert to a 0..1 score where 1.0 is ideal.
        let distance_penalty = (self.geographic_distance_km / 1000.0).min(1.0);
        let latency_penalty = (self.estimated_latency_ms as f64 / 1000.0).min(1.0);
        let load_penalty = self.load_factor.min(1.0);
        let health_bonus = self.health_score;
        (1.0 - distance_penalty * 0.3
            - latency_penalty * 0.3
            - load_penalty * 0.2
            + health_bonus * 0.2)
        .clamp(0.0, 1.0)
    }

    pub fn is_preferred(&self, other: &LocalityMetrics) -> bool {
        self.score() > other.score()
    }
}

impl Default for LocalityMetrics {
    fn default() -> Self {
        Self::new()
    }
}

///////////////////////////////////////////////////////////////////////////////
// LOCALITY AWARE SELECTOR
///////////////////////////////////////////////////////////////////////////////

/// Selects endpoints based on locality, load, and consistency requirements.
pub struct LocalityAwareSelector {
    pub metrics: std::cell::RefCell<std::collections::HashMap<Endpoint, LocalityMetrics>>,
    pub default_profile: ConsistencyProfile,
}

impl LocalityAwareSelector {
    pub fn new(default_profile: ConsistencyProfile) -> Self {
        Self {
            metrics: std::cell::RefCell::new(std::collections::HashMap::new()),
            default_profile,
        }
    }

    pub fn set_metrics(&self, endpoint: Endpoint, metrics: LocalityMetrics) {
        self.metrics.borrow_mut().insert(endpoint, metrics);
    }

    pub fn get_metrics(&self, endpoint: &Endpoint) -> LocalityMetrics {
        self.metrics
            .borrow()
            .get(endpoint)
            .copied()
            .unwrap_or_default()
    }

    pub fn select_best(
        &self,
        candidates: &[Endpoint],
        profile: ConsistencyProfile,
    ) -> Option<Endpoint> {
        if candidates.is_empty() {
            return None;
        }
        let mut best = candidates[0];
        let mut best_score = self.get_metrics(&best).score();
        for candidate in candidates.iter().skip(1) {
            let score = self.get_metrics(candidate).score();
            if score > best_score {
                best = *candidate;
                best_score = score;
            }
        }
        Some(best)
    }

    pub fn candidate_count(&self) -> usize {
        self.metrics.borrow().len()
    }

    pub fn clear_metrics(&self) {
        self.metrics.borrow_mut().clear();
    }
}

impl Default for LocalityAwareSelector {
    fn default() -> Self {
        Self::new(ConsistencyProfile::Eventual)
    }
}

///////////////////////////////////////////////////////////////////////////////
// ROUTING CONFIG
///////////////////////////////////////////////////////////////////////////////

/// Configuration for routing behavior.
#[derive(Debug, Clone)]
pub struct RoutingConfig {
    pub max_routes: usize,
    pub default_hop_count: u32,
    pub default_latency_ms: u64,
    pub enable_locality_aware: bool,
    pub enable_replica_fallback: bool,
    pub route_stale_threshold_ms: u64,
}

impl Default for RoutingConfig {
    fn default() -> Self {
        Self {
            max_routes: 100000,
            default_hop_count: 1,
            default_latency_ms: 10,
            enable_locality_aware: true,
            enable_replica_fallback: true,
            route_stale_threshold_ms: 300000,
        }
    }
}

impl RoutingConfig {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_max_routes(mut self, max: usize) -> Self {
        self.max_routes = max;
        self
    }

    pub fn with_default_latency(mut self, ms: u64) -> Self {
        self.default_latency_ms = ms;
        self
    }

    pub fn with_locality_aware(mut self, enabled: bool) -> Self {
        self.enable_locality_aware = enabled;
        self
    }

    pub fn with_replica_fallback(mut self, enabled: bool) -> Self {
        self.enable_replica_fallback = enabled;
        self
    }

    pub fn validate(&self) -> Result<(), RoutingError> {
        if self.max_routes == 0 {
            return Err(RoutingError::Internal {
                message: "max routes must be non-zero".to_string(),
            });
        }
        Ok(())
    }
}

///////////////////////////////////////////////////////////////////////////////
// ROUTING STATISTICS
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone, Default)]
pub struct RoutingStats {
    pub total_lookups: u64,
    pub successful_lookups: u64,
    pub failed_lookups: u64,
    pub total_registrations: u64,
    pub total_removals: u64,
    pub fallback_uses: u64,
    pub locality_selects: u64,
}

impl RoutingStats {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn lookup_success_rate(&self) -> f64 {
        if self.total_lookups == 0 {
            return 1.0;
        }
        self.successful_lookups as f64 / self.total_lookups as f64
    }

    pub fn fallback_rate(&self) -> f64 {
        if self.total_lookups == 0 {
            return 0.0;
        }
        self.fallback_uses as f64 / self.total_lookups as f64
    }
}

///////////////////////////////////////////////////////////////////////////////
// ROUTING PATH
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone)]
pub struct RoutingPath {
    pub hops: Vec<Endpoint>,
    pub total_latency_ms: u64,
    pub total_hops: u32,
}

impl RoutingPath {
    pub fn new() -> Self {
        Self {
            hops: Vec::new(),
            total_latency_ms: 0,
            total_hops: 0,
        }
    }

    pub fn add_hop(&mut self, endpoint: Endpoint, latency_ms: u64) {
        self.hops.push(endpoint);
        self.total_latency_ms += latency_ms;
        self.total_hops += 1;
    }

    pub fn is_empty(&self) -> bool {
        self.hops.is_empty()
    }

    pub fn final_hop(&self) -> Option<&Endpoint> {
        self.hops.last()
    }
}

impl Default for RoutingPath {
    fn default() -> Self {
        Self::new()
    }
}

///////////////////////////////////////////////////////////////////////////////
// ROUTING ERROR EXTENDED METHODS
///////////////////////////////////////////////////////////////////////////////

impl RoutingError {
    pub fn is_route_error(&self) -> bool {
        matches!(
            self,
            RoutingError::RouteNotFound { .. }
                | RoutingError::EndpointUnreachable { .. }
                | RoutingError::NoCandidates { .. }
                | RoutingError::DuplicateRoute { .. }
                | RoutingError::InvalidEndpoint { .. }
        )
    }

    pub fn suggested_action(&self) -> &'static str {
        match self {
            RoutingError::RouteNotFound { .. } => "register the route first",
            RoutingError::EndpointUnreachable { .. } => "check endpoint connectivity",
            RoutingError::NoCandidates { .. } => "add candidate endpoints",
            RoutingError::RoutingTableFull { .. } => "remove unused routes",
            RoutingError::DuplicateRoute { .. } => "use a different endpoint",
            RoutingError::ResolveFailed { .. } => "retry the resolve",
            RoutingError::LocalitySelectionFailed { .. } => "verify candidates exist",
            RoutingError::ReplicaFallbackFailed { .. } => "check replica health",
            RoutingError::RouteRegistrationFailed { .. } => "retry registration",
            RoutingError::RouteRemovalFailed { .. } => "retry removal",
            RoutingError::NotInitialized => "initialize the router",
            RoutingError::AlreadyInitialized => "router already running",
            RoutingError::FlushFailed { .. } => "retry flush",
            RoutingError::ResetFailed { .. } => "retry reset",
            RoutingError::Internal { .. } => "contact support",
            RoutingError::InvalidEndpoint { .. } => "use a valid endpoint",
        }
    }
}

///////////////////////////////////////////////////////////////////////////////
// ROUTING RESOLVER
///////////////////////////////////////////////////////////////////////////////

pub struct RoutingResolver {
    pub route_table: std::cell::RefCell<RouteTable>,
    pub selector: std::cell::RefCell<LocalityAwareSelector>,
    pub config: RoutingConfig,
}

impl RoutingResolver {
    pub fn new(config: RoutingConfig) -> Self {
        Self {
            route_table: std::cell::RefCell::new(RouteTable::new(config.max_routes)),
            selector: std::cell::RefCell::new(LocalityAwareSelector::new(
                ConsistencyProfile::Eventual,
            )),
            config,
        }
    }

    pub fn resolve(
        &self,
        dataset_id: &DatasetId,
        record_id: &RecordId,
    ) -> Result<Endpoint, RoutingError> {
        let table = self.route_table.borrow();
        if let Some(entry) = table.lookup(dataset_id, record_id) {
            drop(table);
            Ok(entry.endpoint)
        } else {
            drop(table);
            Err(RoutingError::RouteNotFound {
                dataset: dataset_id.clone(),
                record: record_id.clone(),
            })
        }
    }

    pub fn register(
        &self,
        dataset_id: DatasetId,
        record_id: RecordId,
        endpoint: Endpoint,
    ) -> Result<(), RoutingError> {
        let entry = RouteEntry::new(dataset_id, record_id, endpoint.clone());
        let mut table = self.route_table.borrow_mut();
        table.insert(entry)?;
        drop(table);
        self.selector
            .borrow_mut()
            .set_metrics(endpoint, LocalityMetrics::default());
        Ok(())
    }

    pub fn select_locality_aware(
        &self,
        dataset_id: &DatasetId,
        candidates: Vec<Endpoint>,
        profile: ConsistencyProfile,
    ) -> Result<Endpoint, RoutingError> {
        if candidates.is_empty() {
            return Err(RoutingError::NoCandidates {
                dataset: dataset_id.clone(),
            });
        }
        let selector = self.selector.borrow();
        selector
            .select_best(&candidates, profile)
            .ok_or_else(|| RoutingError::LocalitySelectionFailed {
                reason: "no suitable candidate".to_string(),
            })
    }

    pub fn route_count(&self) -> usize {
        self.route_table.borrow().len()
    }

    pub fn config(&self) -> &RoutingConfig {
        &self.config
    }
}

impl Default for RoutingResolver {
    fn default() -> Self {
        Self::new(RoutingConfig::default())
    }
}

///////////////////////////////////////////////////////////////////////////////
// PATH SELECTOR
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathSelectionStrategy {
    ShortestPath,
    LowestLatency,
    HighestBandwidth,
    LeastLoaded,
    RandomPath,
}

impl PathSelectionStrategy {
    pub fn as_str(&self) -> &'static str {
        match self {
            PathSelectionStrategy::ShortestPath => "shortest_path",
            PathSelectionStrategy::LowestLatency => "lowest_latency",
            PathSelectionStrategy::HighestBandwidth => "highest_bandwidth",
            PathSelectionStrategy::LeastLoaded => "least_loaded",
            PathSelectionStrategy::RandomPath => "random_path",
        }
    }
}

pub struct PathSelector {
    pub strategy: PathSelectionStrategy,
    pub paths: std::cell::RefCell<std::collections::HashMap<u64, RoutingPath>>,
    pub next_path_id: std::cell::Cell<u64>,
}

impl PathSelector {
    pub fn new(strategy: PathSelectionStrategy) -> Self {
        Self {
            strategy,
            paths: std::cell::RefCell::new(std::collections::HashMap::new()),
            next_path_id: std::cell::Cell::new(1),
        }
    }

    pub fn create_path(&self) -> u64 {
        let id = self.next_path_id.get();
        self.next_path_id.set(id + 1);
        self.paths.borrow_mut().insert(id, RoutingPath::new());
        id
    }

    pub fn add_hop_to_path(&self, path_id: u64, endpoint: Endpoint, latency_ms: u64) {
        if let Some(path) = self.paths.borrow_mut().get_mut(&path_id) {
            path.add_hop(endpoint, latency_ms);
        }
    }

    pub fn get_path(&self, path_id: u64) -> Option<RoutingPath> {
        self.paths.borrow().get(&path_id).cloned()
    }

    pub fn remove_path(&self, path_id: u64) -> bool {
        self.paths.borrow_mut().remove(&path_id).is_some()
    }

    pub fn path_count(&self) -> usize {
        self.paths.borrow().len()
    }

    pub fn select_best_path(&self, path_ids: &[u64]) -> Option<u64> {
        match self.strategy {
            PathSelectionStrategy::ShortestPath => {
                path_ids
                    .iter()
                    .min_by(|a, b| {
                        let pa = self.get_path(*a).map(|p| p.total_hops).unwrap_or(u32::MAX);
                        let pb = self.get_path(*b).map(|p| p.total_hops).unwrap_or(u32::MAX);
                        pa.cmp(&pb)
                    })
                    .copied()
            }
            PathSelectionStrategy::LowestLatency => {
                path_ids
                    .iter()
                    .min_by(|a, b| {
                        let la = self
                            .get_path(*a)
                            .map(|p| p.total_latency_ms)
                            .unwrap_or(u64::MAX);
                        let lb = self
                            .get_path(*b)
                            .map(|p| p.total_latency_ms)
                            .unwrap_or(u64::MAX);
                        la.cmp(&lb)
                    })
                    .copied()
            }
            PathSelectionStrategy::RandomPath => {
                path_ids.first().copied()
            }
            _ => path_ids.first().copied(),
        }
    }
}

impl Default for PathSelector {
    fn default() -> Self {
        Self::new(PathSelectionStrategy::ShortestPath)
    }
}

///////////////////////////////////////////////////////////////////////////////
// LOOP PREVENTION
///////////////////////////////////////////////////////////////////////////////

pub struct LoopPrevention {
    pub visited: std::cell::RefCell<std::collections::HashSet<Endpoint>>,
    pub max_hops: u32,
    pub loop_count: u64,
}

impl LoopPrevention {
    pub fn new(max_hops: u32) -> Self {
        Self {
            visited: std::cell::RefCell::new(std::collections::HashSet::new()),
            max_hops,
            loop_count: 0,
        }
    }

    pub fn check_hop(&self, endpoint: &Endpoint, current_hops: u32) -> Result<(), RoutingError> {
        if current_hops >= self.max_hops {
            self.loop_count += 1;
            return Err(RoutingError::Internal {
                message: "routing loop detected: max hops exceeded".to_string(),
            });
        }
        if self.visited.borrow().contains(endpoint) {
            self.loop_count += 1;
            return Err(RoutingError::Internal {
                message: "routing loop detected: endpoint revisited".to_string(),
            });
        }
        Ok(())
    }

    pub fn mark_visited(&self, endpoint: Endpoint) {
        self.visited.borrow_mut().insert(endpoint);
    }

    pub fn clear_visited(&self) {
        self.visited.borrow_mut().clear();
    }

    pub fn visited_count(&self) -> usize {
        self.visited.borrow().len()
    }

    pub fn total_loops(&self) -> u64 {
        self.loop_count
    }
}

impl Default for LoopPrevention {
    fn default() -> Self {
        Self::new(255)
    }
}

///////////////////////////////////////////////////////////////////////////////
// ROUTE EXPIRATION
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RouteStatus {
    Active,
    Stale,
    Expired,
    Invalid,
}

impl RouteStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            RouteStatus::Active => "active",
            RouteStatus::Stale => "stale",
            RouteStatus::Expired => "expired",
            RouteStatus::Invalid => "invalid",
        }
    }

    pub fn is_active(&self) -> bool {
        matches!(self, RouteStatus::Active)
    }

    pub fn is_terminal(&self) -> bool {
        matches!(self, RouteStatus::Expired | RouteStatus::Invalid)
    }
}

pub struct RouteExpiration {
    pub default_ttl_ms: u64,
    pub route_ttls: std::cell::RefCell<std::collections::HashMap<(DatasetId, RecordId), u64>>,
    pub route_statuses: std::cell::RefCell<
        std::collections::HashMap<(DatasetId, RecordId), RouteStatus>,
    >,
}

impl RouteExpiration {
    pub fn new(default_ttl_ms: u64) -> Self {
        Self {
            default_ttl_ms,
            route_ttls: std::cell::RefCell::new(std::collections::HashMap::new()),
            route_statuses: std::cell::RefCell::new(std::collections::HashMap::new()),
        }
    }

    pub fn set_ttl(
        &self,
        dataset_id: DatasetId,
        record_id: RecordId,
        ttl_ms: u64,
    ) {
        self.route_ttls
            .borrow_mut()
            .insert((dataset_id, record_id), ttl_ms);
        self.route_statuses
            .borrow_mut()
            .insert((dataset_id, record_id), RouteStatus::Active);
    }

    pub fn is_expired(
        &self,
        dataset_id: &DatasetId,
        record_id: &RecordId,
        now_ms: u64,
    ) -> bool {
        let key = (dataset_id.clone(), record_id.clone());
        let status = self.route_statuses.borrow().get(&key).copied();
        match status {
            Some(RouteStatus::Expired) | Some(RouteStatus::Invalid) => true,
            Some(RouteStatus::Active) | Some(RouteStatus::Stale) => {
                let ttl = self.route_ttls.borrow().get(&key).copied().unwrap_or(0);
                now_ms >= ttl
            }
            None => true,
        }
    }

    pub fn mark_expired(
        &self,
        dataset_id: &DatasetId,
        record_id: &RecordId,
    ) {
        let key = (dataset_id.clone(), record_id.clone());
        self.route_statuses
            .borrow_mut()
            .insert(key, RouteStatus::Expired);
    }

    pub fn mark_active(
        &self,
        dataset_id: &DatasetId,
        record_id: &RecordId,
    ) {
        let key = (dataset_id.clone(), record_id.clone());
        self.route_statuses
            .borrow_mut()
            .insert(key, RouteStatus::Active);
    }

    pub fn get_status(
        &self,
        dataset_id: &DatasetId,
        record_id: &RecordId,
    ) -> Option<RouteStatus> {
        let key = (dataset_id.clone(), record_id.clone());
        self.route_statuses.borrow().get(&key).copied()
    }

    pub fn active_count(&self) -> usize {
        self.route_statuses
            .borrow()
            .values()
            .filter(|s| matches!(s, RouteStatus::Active))
            .count()
    }
}

impl Default for RouteExpiration {
    fn default() -> Self {
        Self::new(300000)
    }
}

///////////////////////////////////////////////////////////////////////////////
// CONVERGENCE ENGINE
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConvergenceState {
    Converged,
    Converging,
    Diverged,
    Unknown,
}

impl ConvergenceState {
    pub fn as_str(&self) -> &'static str {
        match self {
            ConvergenceState::Converged => "converged",
            ConvergenceState::Converging => "converging",
            ConvergenceState::Diverged => "diverged",
            ConvergenceState::Unknown => "unknown",
        }
    }

    pub fn is_converged(&self) -> bool {
        matches!(self, ConvergenceState::Converged)
    }
}

pub struct ConvergenceEngine {
    pub state: std::cell::Cell<ConvergenceState>,
    pub convergence_rounds: u64,
    pub last_convergence_ms: u64,
    pub route_version: u64,
}

impl ConvergenceEngine {
    pub fn new() -> Self {
        Self {
            state: std::cell::Cell::new(ConvergenceState::Unknown),
            convergence_rounds: 0,
            last_convergence_ms: 0,
            route_version: 0,
        }
    }

    pub fn start_convergence(&self) {
        self.state.set(ConvergenceState::Converging);
        self.convergence_rounds += 1;
    }

    pub fn complete_convergence(&self, now_ms: u64, success: bool) {
        if success {
            self.state.set(ConvergenceState::Converged);
            self.route_version += 1;
        } else {
            self.state.set(ConvergenceState::Diverged);
        }
        self.last_convergence_ms = now_ms;
    }

    pub fn is_converged(&self) -> bool {
        self.state.get().is_converged()
    }

    pub fn route_version(&self) -> u64 {
        self.route_version
    }

    pub fn increment_version(&self) {
        self.route_version += 1;
    }
}

impl Default for ConvergenceEngine {
    fn default() -> Self {
        Self::new()
    }
}

///////////////////////////////////////////////////////////////////////////////
// LOCALITY METRICS COLLECTOR
///////////////////////////////////////////////////////////////////////////////

pub struct LocalityMetricsCollector {
    pub measurements: std::cell::RefCell<std::collections::HashMap<Endpoint, Vec<u64>>>,
    pub total_measurements: u64,
}

impl LocalityMetricsCollector {
    pub fn new() -> Self {
        Self {
            measurements: std::cell::RefCell::new(std::collections::HashMap::new()),
            total_measurements: 0,
        }
    }

    pub fn record_latency(&self, endpoint: Endpoint, latency_ms: u64) {
        self.measurements
            .borrow_mut()
            .entry(endpoint)
            .or_default()
            .push(latency_ms);
        self.total_measurements += 1;
    }

    pub fn average_latency(&self, endpoint: &Endpoint) -> Option<u64> {
        self.measurements
            .borrow()
            .get(endpoint)
            .map(|v| {
                if v.is_empty() {
                    0
                } else {
                    v.iter().sum::<u64>() / v.len() as u64
                }
            })
    }

    pub fn measurement_count(&self, endpoint: &Endpoint) -> usize {
        self.measurements
            .borrow()
            .get(endpoint)
            .map(|v| v.len())
            .unwrap_or(0)
    }

    pub fn total_measurements(&self) -> u64 {
        self.total_measurements
    }

    pub fn clear(&self) {
        self.measurements.borrow_mut().clear();
    }
}

impl Default for LocalityMetricsCollector {
    fn default() -> Self {
        Self::new()
    }
}

///////////////////////////////////////////////////////////////////////////////
// WORKLOAD AWARE ROUTER
///////////////////////////////////////////////////////////////////////////////

pub struct WorkloadAwareRouter {
    pub endpoint_loads: std::cell::RefCell<std::collections::HashMap<Endpoint, u64>>,
    pub max_load: u64,
}

impl WorkloadAwareRouter {
    pub fn new(max_load: u64) -> Self {
        Self {
            endpoint_loads: std::cell::RefCell::new(std::collections::HashMap::new()),
            max_load,
        }
    }

    pub fn set_load(&self, endpoint: Endpoint, load: u64) {
        self.endpoint_loads.borrow_mut().insert(endpoint, load);
    }

    pub fn get_load(&self, endpoint: &Endpoint) -> u64 {
        self.endpoint_loads.borrow().get(endpoint).copied().unwrap_or(0)
    }

    pub fn is_overloaded(&self, endpoint: &Endpoint) -> bool {
        self.get_load(endpoint) > self.max_load
    }

    pub fn select_least_loaded(
        &self,
        candidates: &[Endpoint],
    ) -> Option<Endpoint> {
        candidates
            .iter()
            .filter(|e| !self.is_overloaded(e))
            .min_by_key(|e| self.get_load(e))
            .copied()
    }

    pub fn endpoint_count(&self) -> usize {
        self.endpoint_loads.borrow().len()
    }
}

impl Default for WorkloadAwareRouter {
    fn default() -> Self {
        Self::new(10000)
    }
}

///////////////////////////////////////////////////////////////////////////////
// RESOURCE AWARE ROUTER
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourceType {
    CPU,
    Memory,
    Storage,
    Bandwidth,
}

impl ResourceType {
    pub fn as_str(&self) -> &'static str {
        match self {
            ResourceType::CPU => "cpu",
            ResourceType::Memory => "memory",
            ResourceType::Storage => "storage",
            ResourceType::Bandwidth => "bandwidth",
        }
    }
}

pub struct ResourceAwareRouter {
    pub resources: std::cell::RefCell<
        std::collections::HashMap<Endpoint, std::collections::HashMap<ResourceType, u64>>,
    >,
}

impl ResourceAwareRouter {
    pub fn new() -> Self {
        Self {
            resources: std::cell::RefCell::new(std::collections::HashMap::new()),
        }
    }

    pub fn set_resource(
        &self,
        endpoint: Endpoint,
        resource_type: ResourceType,
        value: u64,
    ) {
        self.resources
            .borrow_mut()
            .entry(endpoint)
            .or_default()
            .insert(resource_type, value);
    }

    pub fn get_resource(
        &self,
        endpoint: &Endpoint,
        resource_type: ResourceType,
    ) -> Option<u64> {
        self.resources
            .borrow()
            .get(endpoint)
            .and_then(|m| m.get(&resource_type).copied())
    }

    pub fn has_capacity(
        &self,
        endpoint: &Endpoint,
        resource_type: ResourceType,
        required: u64,
    ) -> bool {
        self.get_resource(endpoint, resource_type)
            .map(|v| v >= required)
            .unwrap_or(false)
    }

    pub fn endpoint_count(&self) -> usize {
        self.resources.borrow().len()
    }
}

impl Default for ResourceAwareRouter {
    fn default() -> Self {
        Self::new()
    }
}

///////////////////////////////////////////////////////////////////////////////
// MULTI PATH ROUTER
///////////////////////////////////////////////////////////////////////////////

pub struct MultiPathRouter {
    pub paths: std::cell::RefCell<std::collections::HashMap<DatasetId, Vec<Endpoint>>>,
    pub path_weights: std::cell::RefCell<std::collections::HashMap<(DatasetId, Endpoint), u32>>,
}

impl MultiPathRouter {
    pub fn new() -> Self {
        Self {
            paths: std::cell::RefCell::new(std::collections::HashMap::new()),
            path_weights: std::cell::RefCell::new(std::collections::HashMap::new()),
        }
    }

    pub fn add_path(&self, dataset_id: DatasetId, endpoint: Endpoint, weight: u32) {
        self.paths
            .borrow_mut()
            .entry(dataset_id)
            .or_default()
            .push(endpoint.clone());
        self.path_weights
            .borrow_mut()
            .insert((dataset_id, endpoint), weight);
    }

    pub fn get_paths(&self, dataset_id: &DatasetId) -> Vec<Endpoint> {
        self.paths
            .borrow()
            .get(dataset_id)
            .cloned()
            .unwrap_or_default()
    }

    pub fn path_count(&self, dataset_id: &DatasetId) -> usize {
        self.paths
            .borrow()
            .get(dataset_id)
            .map(|v| v.len())
            .unwrap_or(0)
    }

    pub fn get_weight(&self, dataset_id: &DatasetId, endpoint: &Endpoint) -> u32 {
        self.path_weights
            .borrow()
            .get(&(dataset_id.clone(), endpoint.clone()))
            .copied()
            .unwrap_or(1)
    }

    pub fn total_path_count(&self) -> usize {
        self.paths
            .borrow()
            .values()
            .map(|v| v.len())
            .sum()
    }
}

impl Default for MultiPathRouter {
    fn default() -> Self {
        Self::new()
    }
}

///////////////////////////////////////////////////////////////////////////////
// ROUTE VALIDATOR
///////////////////////////////////////////////////////////////////////////////

pub struct RouteValidator {
    pub validations: u64,
    pub validation_failures: u64,
    pub invalid_endpoints: u64,
}

impl RouteValidator {
    pub fn new() -> Self {
        Self {
            validations: 0,
            validation_failures: 0,
            invalid_endpoints: 0,
        }
    }

    pub fn validate_endpoint(&mut self, endpoint: &Endpoint) -> bool {
        self.validations += 1;
        if endpoint.address.is_empty() {
            self.validation_failures += 1;
            self.invalid_endpoints += 1;
            false
        } else {
            true
        }
    }

    pub fn validate_route(
        &mut self,
        dataset_id: &DatasetId,
        record_id: &RecordId,
        endpoint: &Endpoint,
    ) -> bool {
        self.validations += 1;
        let valid = !endpoint.address.is_empty();
        if !valid {
            self.validation_failures += 1;
        }
        valid
    }

    pub fn pass_rate(&self) -> f64 {
        if self.validations == 0 {
            return 1.0;
        }
        (self.validations - self.validation_failures) as f64 / self.validations as f64
    }
}

impl Default for RouteValidator {
    fn default() -> Self {
        Self::new()
    }
}

///////////////////////////////////////////////////////////////////////////////
// ENDPOINT HEALTH TRACKER
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EndpointHealthStatus {
    Healthy,
    Degraded,
    Unhealthy,
    Unknown,
}

impl EndpointHealthStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            EndpointHealthStatus::Healthy => "healthy",
            EndpointHealthStatus::Degraded => "degraded",
            EndpointHealthStatus::Unhealthy => "unhealthy",
            EndpointHealthStatus::Unknown => "unknown",
        }
    }

    pub fn is_healthy(&self) -> bool {
        matches!(self, EndpointHealthStatus::Healthy)
    }
}

pub struct EndpointHealthTracker {
    pub health: std::cell::RefCell<std::collections::HashMap<Endpoint, EndpointHealthStatus>>,
    pub health_checks: u64,
}

impl EndpointHealthTracker {
    pub fn new() -> Self {
        Self {
            health: std::cell::RefCell::new(std::collections::HashMap::new()),
            health_checks: 0,
        }
    }

    pub fn set_health(&self, endpoint: Endpoint, status: EndpointHealthStatus) {
        self.health.borrow_mut().insert(endpoint, status);
        self.health_checks += 1;
    }

    pub fn get_health(&self, endpoint: &Endpoint) -> EndpointHealthStatus {
        self.health
            .borrow()
            .get(endpoint)
            .copied()
            .unwrap_or(EndpointHealthStatus::Unknown)
    }

    pub fn healthy_count(&self) -> usize {
        self.health
            .borrow()
            .values()
            .filter(|s| matches!(s, EndpointHealthStatus::Healthy))
            .count()
    }

    pub fn total_checks(&self) -> u64 {
        self.health_checks
    }
}

impl Default for EndpointHealthTracker {
    fn default() -> Self {
        Self::new()
    }
}

///////////////////////////////////////////////////////////////////////////////
// ROUTE CACHE
///////////////////////////////////////////////////////////////////////////////

pub struct RouteCache {
    pub cache: std::cell::RefCell<std::collections::HashMap<(DatasetId, RecordId), Endpoint>>,
    pub max_entries: usize,
    pub hits: u64,
    pub misses: u64,
}

impl RouteCache {
    pub fn new(max_entries: usize) -> Self {
        Self {
            cache: std::cell::RefCell::new(std::collections::HashMap::new()),
            max_entries,
            hits: 0,
            misses: 0,
        }
    }

    pub fn get(&self, dataset_id: &DatasetId, record_id: &RecordId) -> Option<Endpoint> {
        let key = (dataset_id.clone(), record_id.clone());
        match self.cache.borrow().get(&key) {
            Some(ep) => {
                self.hits += 1;
                Some(ep.clone())
            }
            None => {
                self.misses += 1;
                None
            }
        }
    }

    pub fn put(
        &self,
        dataset_id: DatasetId,
        record_id: RecordId,
        endpoint: Endpoint,
    ) {
        if self.cache.borrow().len() >= self.max_entries {
            if let Some(first_key) = self.cache.borrow().keys().next().cloned() {
                self.cache.borrow_mut().remove(&first_key);
            }
        }
        self.cache
            .borrow_mut()
            .insert((dataset_id, record_id), endpoint);
    }

    pub fn hit_rate(&self) -> f64 {
        let total = self.hits + self.misses;
        if total == 0 {
            return 0.0;
        }
        self.hits as f64 / total as f64
    }

    pub fn len(&self) -> usize {
        self.cache.borrow().len()
    }

    pub fn clear(&self) {
        self.cache.borrow_mut().clear();
    }
}

impl Default for RouteCache {
    fn default() -> Self {
        Self::new(10000)
    }
}

///////////////////////////////////////////////////////////////////////////////
// PREFIX ROUTER
///////////////////////////////////////////////////////////////////////////////

pub struct PrefixRouter {
    pub prefixes: std::cell::RefCell<std::collections::HashMap<Vec<u8>, Endpoint>>,
}

impl PrefixRouter {
    pub fn new() -> Self {
        Self {
            prefixes: std::cell::RefCell::new(std::collections::HashMap::new()),
        }
    }

    pub fn register_prefix(&self, prefix: Vec<u8>, endpoint: Endpoint) {
        self.prefixes.borrow_mut().insert(prefix, endpoint);
    }

    pub fn lookup_prefix(&self, key: &[u8]) -> Option<Endpoint> {
        let prefixes = self.prefixes.borrow();
        let mut best: Option<(usize, Endpoint)> = None;
        for (prefix, endpoint) in prefixes.iter() {
            if key.starts_with(prefix.as_slice()) && prefix.len() > best.as_ref().map(|(l, _)| *l).unwrap_or(0) {
                best = Some((prefix.len(), endpoint.clone()));
            }
        }
        best.map(|(_, e)| e)
    }

    pub fn prefix_count(&self) -> usize {
        self.prefixes.borrow().len()
    }
}

impl Default for PrefixRouter {
    fn default() -> Self {
        Self::new()
    }
}

///////////////////////////////////////////////////////////////////////////////
// HASH ROUTER
///////////////////////////////////////////////////////////////////////////////

pub struct HashRouter {
    pub endpoints: std::cell::RefCell<Vec<Endpoint>>,
}

impl HashRouter {
    pub fn new() -> Self {
        Self {
            endpoints: std::cell::RefCell::new(Vec::new()),
        }
    }

    pub fn add_endpoint(&self, endpoint: Endpoint) {
        self.endpoints.borrow_mut().push(endpoint);
    }

    pub fn route_by_hash(&self, key: &[u8]) -> Option<Endpoint> {
        let endpoints = self.endpoints.borrow();
        if endpoints.is_empty() {
            return None;
        }
        let mut hash: u64 = 0x811c9dc5;
        for &byte in key {
            hash ^= byte as u64;
            hash = hash.wrapping_mul(0x01000193);
        }
        let idx = hash as usize % endpoints.len();
        Some(endpoints[idx].clone())
    }

    pub fn endpoint_count(&self) -> usize {
        self.endpoints.borrow().len()
    }
}

impl Default for HashRouter {
    fn default() -> Self {
        Self::new()
    }
}

///////////////////////////////////////////////////////////////////////////////
// CONSISTENT HASH ROUTER
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone)]
pub struct HashNode {
    pub endpoint: Endpoint,
    pub hash_value: u64,
    pub replica_count: u32,
}

pub struct ConsistentHashRouter {
    pub nodes: std::cell::RefCell<Vec<HashNode>>,
    pub virtual_nodes: u32,
}

impl ConsistentHashRouter {
    pub fn new(virtual_nodes: u32) -> Self {
        Self {
            nodes: std::cell::RefCell::new(Vec::new()),
            virtual_nodes,
        }
    }

    pub fn add_node(&self, endpoint: Endpoint, replica_count: u32) {
        let mut hash_value: u64 = 0;
        for &byte in endpoint.address.iter() {
            hash_value ^= byte as u64;
            hash_value = hash_value.wrapping_mul(0x01000193);
        }
        self.nodes.borrow_mut().push(HashNode {
            endpoint,
            hash_value,
            replica_count,
        });
    }

    pub fn route(&self, key: &[u8]) -> Option<Endpoint> {
        let nodes = self.nodes.borrow();
        if nodes.is_empty() {
            return None;
        }
        let mut key_hash: u64 = 0x811c9dc5;
        for &byte in key {
            key_hash ^= byte as u64;
            key_hash = key_hash.wrapping_mul(0x01000193);
        }
        nodes
            .iter()
            .min_by_key(|n| (n.hash_value.wrapping_sub(key_hash)).abs_diff(0))
            .map(|n| n.endpoint.clone())
    }

    pub fn node_count(&self) -> usize {
        self.nodes.borrow().len()
    }
}

impl Default for ConsistentHashRouter {
    fn default() -> Self {
        Self::new(100)
    }
}

///////////////////////////////////////////////////////////////////////////////
// ROUTING ERROR EXTENDED METHODS (additional)
///////////////////////////////////////////////////////////////////////////////

impl RoutingError {
    pub fn is_recoverable(&self) -> bool {
        matches!(
            self,
            RoutingError::EndpointUnreachable { .. }
                | RoutingError::NoCandidates { .. }
                | RoutingError::ResolveFailed { .. }
        )
    }

    pub fn requires_re_route(&self) -> bool {
        matches!(
            self,
            RoutingError::EndpointUnreachable { .. }
                | RoutingError::RouteNotFound { .. }
        )
    }
}

///////////////////////////////////////////////////////////////////////////////
// ROUTE OPTIMIZER
///////////////////////////////////////////////////////////////////////////////

pub struct RouteOptimizer {
    pub optimization_enabled: std::cell::Cell<bool>,
    pub optimization_rounds: u64,
    pub routes_optimized: u64,
}

impl RouteOptimizer {
    pub fn new() -> Self {
        Self {
            optimization_enabled: std::cell::Cell::new(true),
            optimization_rounds: 0,
            routes_optimized: 0,
        }
    }

    pub fn enable_optimization(&self) {
        self.optimization_enabled.set(true);
    }

    pub fn disable_optimization(&self) {
        self.optimization_enabled.set(false);
    }

    pub fn is_enabled(&self) -> bool {
        self.optimization_enabled.get()
    }

    pub fn optimize_route(
        &self,
        path: &mut RoutingPath,
    ) -> bool {
        if !self.optimization_enabled.get() {
            return false;
        }
        self.optimization_rounds += 1;
        if path.hops.len() > 1 {
            path.hops.dedup();
            self.routes_optimized += 1;
            true
        } else {
            false
        }
    }

    pub fn optimization_rounds(&self) -> u64 {
        self.optimization_rounds
    }

    pub fn routes_optimized(&self) -> u64 {
        self.routes_optimized
    }
}

impl Default for RouteOptimizer {
    fn default() -> Self {
        Self::new()
    }
}

///////////////////////////////////////////////////////////////////////////////
// TRAFFIC MANAGER
///////////////////////////////////////////////////////////////////////////////

pub struct TrafficManager {
    pub traffic_volume: std::cell::RefCell<std::collections::HashMap<Endpoint, u64>>,
    pub total_traffic: u64,
}

impl TrafficManager {
    pub fn new() -> Self {
        Self {
            traffic_volume: std::cell::RefCell::new(std::collections::HashMap::new()),
            total_traffic: 0,
        }
    }

    pub fn record_traffic(&self, endpoint: Endpoint, bytes: u64) {
        self.traffic_volume
            .borrow_mut()
            .entry(endpoint)
            .and_modify(|v| *v += bytes)
            .or_insert(bytes);
        self.total_traffic += bytes;
    }

    pub fn get_traffic(&self, endpoint: &Endpoint) -> u64 {
        self.traffic_volume.borrow().get(endpoint).copied().unwrap_or(0)
    }

    pub fn total_traffic(&self) -> u64 {
        self.total_traffic
    }

    pub fn endpoint_count(&self) -> usize {
        self.traffic_volume.borrow().len()
    }
}

impl Default for TrafficManager {
    fn default() -> Self {
        Self::new()
    }
}

///////////////////////////////////////////////////////////////////////////////
// BANDWIDTH MANAGER
///////////////////////////////////////////////////////////////////////////////

pub struct BandwidthManager {
    pub bandwidth_limits: std::cell::RefCell<std::collections::HashMap<Endpoint, u64>>,
    pub bandwidth_usage: std::cell::RefCell<std::collections::HashMap<Endpoint, u64>>,
}

impl BandwidthManager {
    pub fn new() -> Self {
        Self {
            bandwidth_limits: std::cell::RefCell::new(std::collections::HashMap::new()),
            bandwidth_usage: std::cell::RefCell::new(std::collections::HashMap::new()),
        }
    }

    pub fn set_limit(&self, endpoint: Endpoint, limit_bps: u64) {
        self.bandwidth_limits.borrow_mut().insert(endpoint, limit_bps);
    }

    pub fn get_limit(&self, endpoint: &Endpoint) -> Option<u64> {
        self.bandwidth_limits.borrow().get(endpoint).copied()
    }

    pub fn record_usage(&self, endpoint: Endpoint, bytes: u64) {
        self.bandwidth_usage
            .borrow_mut()
            .entry(endpoint)
            .and_modify(|v| *v += bytes)
            .or_insert(bytes);
    }

    pub fn get_usage(&self, endpoint: &Endpoint) -> u64 {
        self.bandwidth_usage.borrow().get(endpoint).copied().unwrap_or(0)
    }

    pub fn is_within_limit(&self, endpoint: &Endpoint) -> bool {
        let limit = self.get_limit(endpoint).unwrap_or(u64::MAX);
        self.get_usage(endpoint) <= limit
    }
}

impl Default for BandwidthManager {
    fn default() -> Self {
        Self::new()
    }
}

///////////////////////////////////////////////////////////////////////////////
// FAILOVER ROUTER
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailoverStrategy {
    Immediate,
    Graceful,
    Manual,
}

impl FailoverStrategy {
    pub fn as_str(&self) -> &'static str {
        match self {
            FailoverStrategy::Immediate => "immediate",
            FailoverStrategy::Graceful => "graceful",
            FailoverStrategy::Manual => "manual",
        }
    }
}

pub struct FailoverRouter {
    pub strategy: FailoverStrategy,
    pub primary: std::cell::RefCell<Option<Endpoint>>,
    pub backups: std::cell::RefCell<Vec<Endpoint>>,
    pub failover_count: u64,
}

impl FailoverRouter {
    pub fn new(strategy: FailoverStrategy) -> Self {
        Self {
            strategy,
            primary: std::cell::RefCell::new(None),
            backups: std::cell::RefCell::new(Vec::new()),
            failover_count: 0,
        }
    }

    pub fn set_primary(&self, endpoint: Endpoint) {
        self.primary.borrow_mut().replace(endpoint);
    }

    pub fn add_backup(&self, endpoint: Endpoint) {
        self.backups.borrow_mut().push(endpoint);
    }

    pub fn get_primary(&self) -> Option<Endpoint> {
        self.primary.borrow().clone()
    }

    pub fn failover(&self) -> Option<Endpoint> {
        self.failover_count += 1;
        self.backups.borrow().first().cloned()
    }

    pub fn failover_count(&self) -> u64 {
        self.failover_count
    }
}

impl Default for FailoverRouter {
    fn default() -> Self {
        Self::new(FailoverStrategy::Immediate)
    }
}

///////////////////////////////////////////////////////////////////////////////
// GEOGRAPHIC ROUTER
///////////////////////////////////////////////////////////////////////////////

pub struct GeographicRouter {
    pub regions: std::cell::RefCell<std::collections::HashMap<Endpoint, RegionId>>,
    pub region_endpoints: std::cell::RefCell<std::collections::HashMap<RegionId, Vec<Endpoint>>>,
}

impl GeographicRouter {
    pub fn new() -> Self {
        Self {
            regions: std::cell::RefCell::new(std::collections::HashMap::new()),
            region_endpoints: std::cell::RefCell::new(std::collections::HashMap::new()),
        }
    }

    pub fn assign_region(&self, endpoint: Endpoint, region: RegionId) {
        self.regions.borrow_mut().insert(endpoint.clone(), region.clone());
        self.region_endpoints
            .borrow_mut()
            .entry(region)
            .or_default()
            .push(endpoint);
    }

    pub fn get_region(&self, endpoint: &Endpoint) -> Option<RegionId> {
        self.regions.borrow().get(endpoint).cloned()
    }

    pub fn get_endpoints_in_region(
        &self,
        region: &RegionId,
    ) -> Vec<Endpoint> {
        self.region_endpoints
            .borrow()
            .get(region)
            .cloned()
            .unwrap_or_default()
    }

    pub fn region_count(&self) -> usize {
        self.region_endpoints.borrow().len()
    }
}

impl Default for GeographicRouter {
    fn default() -> Self {
        Self::new()
    }
}

///////////////////////////////////////////////////////////////////////////////
// TOPOLOGY MAPPER
///////////////////////////////////////////////////////////////////////////////

pub struct TopologyMapper {
    pub links: std::cell::RefCell<std::collections::HashMap<(Endpoint, Endpoint), u64>>,
    pub nodes: std::cell::RefCell<std::collections::HashSet<Endpoint>>,
}

impl TopologyMapper {
    pub fn new() -> Self {
        Self {
            links: std::cell::RefCell::new(std::collections::HashMap::new()),
            nodes: std::cell::RefCell::new(std::collections::HashSet::new()),
        }
    }

    pub fn add_node(&self, endpoint: Endpoint) {
        self.nodes.borrow_mut().insert(endpoint);
    }

    pub fn add_link(
        &self,
        source: Endpoint,
        target: Endpoint,
        cost: u64,
    ) {
        self.nodes.borrow_mut().insert(source.clone());
        self.nodes.borrow_mut().insert(target.clone());
        self.links
            .borrow_mut()
            .insert((source, target), cost);
    }

    pub fn get_link_cost(
        &self,
        source: &Endpoint,
        target: &Endpoint,
    ) -> Option<u64> {
        self.links.borrow().get(&(source.clone(), target.clone())).copied()
    }

    pub fn node_count(&self) -> usize {
        self.nodes.borrow().len()
    }

    pub fn link_count(&self) -> usize {
        self.links.borrow().len()
    }
}

impl Default for TopologyMapper {
    fn default() -> Self {
        Self::new()
    }
}

///////////////////////////////////////////////////////////////////////////////
// ROUTE ANALYTICS
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone, Default)]
pub struct RouteAnalytics {
    pub total_routes: u64,
    pub total_resolutions: u64,
    pub total_failures: u64,
    pub total_failovers: u64,
    pub average_hop_count: f64,
    pub average_latency_ms: f64,
}

impl RouteAnalytics {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn failure_rate(&self) -> f64 {
        if self.total_resolutions == 0 {
            return 0.0;
        }
        self.total_failures as f64 / self.total_resolutions as f64
    }

    pub fn success_rate(&self) -> f64 {
        1.0 - self.failure_rate()
    }

    pub fn record_resolution(&mut self, success: bool, hops: u32, latency_ms: u64) {
        self.total_resolutions += 1;
        if !success {
            self.total_failures += 1;
        }
        self.average_hop_count =
            (self.average_hop_count * (self.total_resolutions - 1) as f64 + hops as f64)
                / self.total_resolutions as f64;
        self.average_latency_ms =
            (self.average_latency_ms * (self.total_resolutions - 1) as f64 + latency_ms as f64)
                / self.total_resolutions as f64;
    }

    pub fn record_failover(&mut self) {
        self.total_failovers += 1;
    }
}

///////////////////////////////////////////////////////////////////////////////
// LOAD BALANCER
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoadBalancingStrategy {
    RoundRobin,
    LeastConnections,
    WeightedRoundRobin,
    SourceHash,
}

impl LoadBalancingStrategy {
    pub fn as_str(&self) -> &'static str {
        match self {
            LoadBalancingStrategy::RoundRobin => "round_robin",
            LoadBalancingStrategy::LeastConnections => "least_connections",
            LoadBalancingStrategy::WeightedRoundRobin => "weighted_round_robin",
            LoadBalancingStrategy::SourceHash => "source_hash",
        }
    }
}

pub struct LoadBalancer {
    pub strategy: LoadBalancingStrategy,
    pub endpoints: std::cell::RefCell<Vec<Endpoint>>,
    pub weights: std::cell::RefCell<std::collections::HashMap<Endpoint, u32>>,
    pub connections: std::cell::RefCell<std::collections::HashMap<Endpoint, u64>>,
    pub current_index: std::cell::Cell<usize>,
}

impl LoadBalancer {
    pub fn new(strategy: LoadBalancingStrategy) -> Self {
        Self {
            strategy,
            endpoints: std::cell::RefCell::new(Vec::new()),
            weights: std::cell::RefCell::new(std::collections::HashMap::new()),
            connections: std::cell::RefCell::new(std::collections::HashMap::new()),
            current_index: std::cell::Cell::new(0),
        }
    }

    pub fn add_endpoint(&self, endpoint: Endpoint, weight: u32) {
        self.endpoints.borrow_mut().push(endpoint.clone());
        self.weights.borrow_mut().insert(endpoint, weight);
        self.connections.borrow_mut().insert(endpoint, 0);
    }

    pub fn select(&self) -> Option<Endpoint> {
        let endpoints = self.endpoints.borrow();
        if endpoints.is_empty() {
            return None;
        }
        match self.strategy {
            LoadBalancingStrategy::RoundRobin => {
                let idx = self.current_index.get() % endpoints.len();
                self.current_index.set(idx + 1);
                Some(endpoints[idx].clone())
            }
            LoadBalancingStrategy::LeastConnections => {
                let conns = self.connections.borrow();
                endpoints
                    .iter()
                    .min_by_key(|e| conns.get(e).copied().unwrap_or(0))
                    .cloned()
            }
            _ => endpoints.first().cloned(),
        }
    }

    pub fn increment_connections(&self, endpoint: &Endpoint) {
        self.connections
            .borrow_mut()
            .entry(endpoint.clone())
            .and_modify(|v| *v += 1)
            .or_insert(1);
    }

    pub fn decrement_connections(&self, endpoint: &Endpoint) {
        if let Some(v) = self.connections.borrow_mut().get_mut(endpoint) {
            *v = v.saturating_sub(1);
        }
    }

    pub fn endpoint_count(&self) -> usize {
        self.endpoints.borrow().len()
    }
}

impl Default for LoadBalancer {
    fn default() -> Self {
        Self::new(LoadBalancingStrategy::RoundRobin)
    }
}

///////////////////////////////////////////////////////////////////////////////
// SESSION ROUTER
///////////////////////////////////////////////////////////////////////////////

pub struct SessionRouter {
    pub session_routes: std::cell::RefCell<std::collections::HashMap<crate::types::SessionID, Endpoint>>,
}

impl SessionRouter {
    pub fn new() -> Self {
        Self {
            session_routes: std::cell::RefCell::new(std::collections::HashMap::new()),
        }
    }

    pub fn route_session(
        &self,
        session_id: crate::types::SessionID,
        endpoint: Endpoint,
    ) {
        self.session_routes
            .borrow_mut()
            .insert(session_id, endpoint);
    }

    pub fn get_session_route(
        &self,
        session_id: crate::types::SessionID,
    ) -> Option<Endpoint> {
        self.session_routes.borrow().get(&session_id).cloned()
    }

    pub fn remove_session(&self, session_id: crate::types::SessionID) -> bool {
        self.session_routes.borrow_mut().remove(&session_id).is_some()
    }

    pub fn session_count(&self) -> usize {
        self.session_routes.borrow().len()
    }
}

impl Default for SessionRouter {
    fn default() -> Self {
        Self::new()
    }
}

///////////////////////////////////////////////////////////////////////////////
// DATAGRAM ROUTER
///////////////////////////////////////////////////////////////////////////////

pub struct DatagramRouter {
    pub datagrams_routed: u64,
    pub datagrams_dropped: u64,
    pub max_datagram_size: usize,
}

impl DatagramRouter {
    pub fn new(max_datagram_size: usize) -> Self {
        Self {
            datagrams_routed: 0,
            datagrams_dropped: 0,
            max_datagram_size,
        }
    }

    pub fn route_datagram(&self, size: usize) -> bool {
        if size > self.max_datagram_size {
            self.datagrams_dropped += 1;
            false
        } else {
            self.datagrams_routed += 1;
            true
        }
    }

    pub fn drop_rate(&self) -> f64 {
        let total = self.datagrams_routed + self.datagrams_dropped;
        if total == 0 {
            return 0.0;
        }
        self.datagrams_dropped as f64 / total as f64
    }

    pub fn max_datagram_size(&self) -> usize {
        self.max_datagram_size
    }
}

impl Default for DatagramRouter {
    fn default() -> Self {
        Self::new(65535)
    }
}

///////////////////////////////////////////////////////////////////////////////
// ROUTING ERROR EXTENDED METHODS (additional)
///////////////////////////////////////////////////////////////////////////////

impl RoutingError {
    pub fn is_recoverable(&self) -> bool {
        matches!(
            self,
            RoutingError::EndpointUnreachable { .. }
                | RoutingError::NoCandidates { .. }
                | RoutingError::ResolveFailed { .. }
        )
    }

    pub fn requires_re_route(&self) -> bool {
        matches!(
            self,
            RoutingError::EndpointUnreachable { .. } | RoutingError::RouteNotFound { .. }
        )
    }
}

///////////////////////////////////////////////////////////////////////////////
// TESTS
///////////////////////////////////////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use super::*;

    // Test implementation of Routing for testing the trait contract.
    struct TestRouter {
        name: &'static str,
        routes: std::cell::RefCell<std::collections::HashMap<(DatasetId, RecordId), Endpoint>>,
        lookups: std::cell::Cell<u64>,
        resolutions: std::cell::Cell<u64>,
    }

    impl TestRouter {
        fn new(name: &'static str) -> Self {
            Self {
                name,
                routes: std::cell::RefCell::new(std::collections::HashMap::new()),
                lookups: std::cell::Cell::new(0),
                resolutions: std::cell::Cell::new(0),
            }
        }
    }

    impl Routing for TestRouter {
        fn resolve_logical(
            &self,
            dataset_id: &DatasetId,
            record_id: &RecordId,
        ) -> Result<Endpoint, RoutingError> {
            self.lookups.set(self.lookups.get() + 1);
            self.resolutions.set(self.resolutions.get() + 1);
            let key = (dataset_id.clone(), record_id.clone());
            self.routes
                .borrow()
                .get(&key)
                .cloned()
                .ok_or_else(|| RoutingError::RouteNotFound {
                    dataset: dataset_id.clone(),
                    record: record_id.clone(),
                })
        }

        fn locality_aware_select(
            &self,
            candidates: Vec<Endpoint>,
            _profile: ConsistencyProfile,
        ) -> Result<Endpoint, RoutingError> {
            self.resolutions.set(self.resolutions.get() + 1);
            candidates
                .into_iter()
                .next()
                .ok_or_else(|| RoutingError::NoCandidates {
                    dataset: DatasetId::new([0; 32]),
                })
        }

        fn replica_aware_fallback(
            &self,
            primary: Endpoint,
            _profile: ConsistencyProfile,
        ) -> Result<Endpoint, RoutingError> {
            self.resolutions.set(self.resolutions.get() + 1);
            Ok(primary)
        }

        fn closest_replica(
            &self,
            dataset_id: &DatasetId,
            from: Endpoint,
        ) -> Result<Vec<Endpoint>, RoutingError> {
            let _ = dataset_id;
            let _ = from;
            Ok(vec![])
        }

        fn all_replicas(&self, dataset_id: &DatasetId) -> Result<Vec<Endpoint>, RoutingError> {
            let _ = dataset_id;
            Ok(vec![])
        }

        fn routing_name(&self) -> &'static str {
            self.name
        }

        fn total_routes(&self) -> u64 {
            self.routes.borrow().len() as u64
        }

        fn total_resolutions(&self) -> u64 {
            self.resolutions.get()
        }
    }

    #[test]
    fn routing_resolve_logical() {
        let router = TestRouter::new("test");
        let ds = DatasetId::new([1, 2, 3]);
        let rid = RecordId::new([4, 5, 6]);
        let ep = Endpoint::new(crate::types::CarrierType::Loopback, vec![1, 2, 3]).unwrap();

        // Register manually via the internal map (test-only access through trait)
        // We test the not-found path first
        let err = router.resolve_logical(&ds, &rid).unwrap_err();
        assert!(matches!(err, RoutingError::RouteNotFound { .. }));

        // Now test via the route table directly
        let table = RouteTable::new(100);
        let entry = RouteEntry::new(ds.clone(), rid.clone(), ep.clone());
        assert!(table.insert(entry).is_ok());
        let found = table.lookup(&ds, &rid).unwrap();
        assert_eq!(found.endpoint, ep);
    }

    #[test]
    fn routing_route_not_found() {
        let router = TestRouter::new("test");
        let ds = DatasetId::new([1]);
        let rid = RecordId::new([2]);
        let err = router.resolve_logical(&ds, &rid).unwrap_err();
        assert!(matches!(err, RoutingError::RouteNotFound { .. }));
    }

    #[test]
    fn routing_locality_aware_select() {
        let router = TestRouter::new("test");
        let ep1 = Endpoint::new(crate::types::CarrierType::Loopback, vec![1]).unwrap();
        let ep2 = Endpoint::new(crate::types::CarrierType::Loopback, vec![2]).unwrap();
        let selected = router
            .locality_aware_select(vec![ep1.clone(), ep2.clone()], ConsistencyProfile::Eventual)
            .unwrap();
        assert_eq!(selected, ep1);
    }

    #[test]
    fn routing_locality_aware_empty() {
        let router = TestRouter::new("test");
        let err = router
            .locality_aware_select(vec![], ConsistencyProfile::Strong)
            .unwrap_err();
        assert!(matches!(err, RoutingError::NoCandidates { .. }));
    }

    #[test]
    fn routing_replica_aware_fallback() {
        let router = TestRouter::new("test");
        let ep = Endpoint::new(crate::types::CarrierType::Loopback, vec![1]).unwrap();
        let fallback = router
            .replica_aware_fallback(ep.clone(), ConsistencyProfile::Eventual)
            .unwrap();
        assert_eq!(fallback, ep);
    }

    #[test]
    fn routing_error_category() {
        assert_eq!(
            RoutingError::RouteNotFound {
                dataset: DatasetId::new([1]),
                record: RecordId::new([2]),
            }
            .category(),
            "route_not_found"
        );
        assert_eq!(
            RoutingError::EndpointUnreachable {
                endpoint: Endpoint::new(crate::types::CarrierType::Loopback, vec![1]).unwrap()
            }
            .category(),
            "endpoint_unreachable"
        );
        assert_eq!(
            RoutingError::NoCandidates {
                dataset: DatasetId::new([1])
            }
            .category(),
            "no_candidates"
        );
        assert_eq!(
            RoutingError::RoutingTableFull { max: 1000 }.category(),
            "routing_table_full"
        );
        assert_eq!(
            RoutingError::ResolveFailed {
                dataset: DatasetId::new([1]),
                record: RecordId::new([2]),
                reason: "x".to_string()
            }
            .category(),
            "resolve_failed"
        );
        assert_eq!(
            RoutingError::Internal { message: "x".to_string() }.category(),
            "internal"
        );
    }

    #[test]
    fn routing_error_retryable_and_fatal() {
        assert!(RoutingError::ResolveFailed {
            dataset: DatasetId::new([1]),
            record: RecordId::new([2]),
            reason: "x".to_string()
        }
        .is_retryable());
        assert!(RoutingError::Internal { message: "x".to_string() }.is_retryable());
        assert!(!RoutingError::RouteNotFound {
            dataset: DatasetId::new([1]),
            record: RecordId::new([2]),
        }
        .is_retryable());
        assert!(RoutingError::RouteNotFound {
            dataset: DatasetId::new([1]),
            record: RecordId::new([2]),
        }
        .is_fatal());
    }

    #[test]
    fn routing_error_plain_message() {
        let msg = RoutingError::RouteNotFound {
            dataset: DatasetId::new([1]),
            record: RecordId::new([2]),
        }
        .plain_message();
        assert!(msg.contains("route not found"));
        let msg = RoutingError::EndpointUnreachable {
            endpoint: Endpoint::new(crate::types::CarrierType::Loopback, vec![1]).unwrap(),
        }
        .plain_message();
        assert!(msg.contains("endpoint unreachable"));
    }

    #[test]
    fn routing_error_suggested_action() {
        assert_eq!(
            RoutingError::RouteNotFound {
                dataset: DatasetId::new([1]),
                record: RecordId::new([2]),
            }
            .suggested_action(),
            "register the route first"
        );
        assert_eq!(
            RoutingError::RoutingTableFull { max: 100 }.suggested_action(),
            "remove unused routes"
        );
        assert_eq!(
            RoutingError::Internal { message: "x".to_string() }.suggested_action(),
            "contact support"
        );
    }

    #[test]
    fn routing_error_is_route_error() {
        assert!(RoutingError::RouteNotFound {
            dataset: DatasetId::new([1]),
            record: RecordId::new([2]),
        }
        .is_route_error());
        assert!(RoutingError::EndpointUnreachable {
            endpoint: Endpoint::new(crate::types::CarrierType::Loopback, vec![1]).unwrap()
        }
        .is_route_error());
        assert!(!RoutingError::Internal { message: "x".to_string() }.is_route_error());
    }

    // === RouteEntry tests ===
    #[test]
    fn route_entry_new_and_record_use() {
        let ep = Endpoint::new(crate::types::CarrierType::Loopback, vec![1]).unwrap();
        let entry = RouteEntry::new(
            DatasetId::new([1]),
            RecordId::new([2]),
            ep.clone(),
        );
        assert_eq!(entry.dataset_id, DatasetId::new([1]));
        assert_eq!(entry.record_id, RecordId::new([2]));
        assert_eq!(entry.endpoint, ep);
        assert_eq!(entry.priority, 0);
        assert_eq!(entry.hop_count, 1);
        assert_eq!(entry.latency_ms, 10);
        assert_eq!(entry.use_count, 0);
    }

    #[test]
    fn route_entry_with_options() {
        let ep = Endpoint::new(crate::types::CarrierType::Loopback, vec![1]).unwrap();
        let entry = RouteEntry::new(
            DatasetId::new([1]),
            RecordId::new([2]),
            ep.clone(),
        )
        .with_priority(5)
        .with_hop_count(3)
        .with_latency(50)
        .with_region(RegionId::new([3]));
        assert_eq!(entry.priority, 5);
        assert_eq!(entry.hop_count, 3);
        assert_eq!(entry.latency_ms, 50);
        assert_eq!(entry.region, Some(RegionId::new([3])));
    }

    #[test]
    fn route_entry_record_use_and_age() {
        let ep = Endpoint::new(crate::types::CarrierType::Loopback, vec![1]).unwrap();
        let mut entry = RouteEntry::new(
            DatasetId::new([1]),
            RecordId::new([2]),
            ep,
        );
        entry.record_use(100);
        assert_eq!(entry.use_count, 1);
        assert_eq!(entry.last_used_ms, 100);
        entry.record_use(200);
        assert_eq!(entry.use_count, 2);
        assert_eq!(entry.last_used_ms, 200);
        assert_eq!(entry.age_ms(300), 100);
        assert!(!entry.is_stale(250, 100));
        assert!(entry.is_stale(301, 100));
    }

    // === RouteTable tests ===
    #[test]
    fn route_table_insert_and_lookup() {
        let table = RouteTable::new(10);
        let ep = Endpoint::new(crate::types::CarrierType::Loopback, vec![1]).unwrap();
        let entry = RouteEntry::new(
            DatasetId::new([1]),
            RecordId::new([2]),
            ep.clone(),
        );
        assert!(table.insert(entry).is_ok());
        let found = table.lookup(&DatasetId::new([1]), &RecordId::new([2])).unwrap();
        assert_eq!(found.endpoint, ep);
        assert_eq!(table.len(), 1);
        assert!(!table.is_empty());
        assert!(!table.is_full());
    }

    #[test]
    fn route_table_lookup_miss() {
        let table = RouteTable::new(10);
        let result = table
            .lookup(&DatasetId::new([1]), &RecordId::new([2]))
            .is_none();
        assert!(result);
        assert_eq!(table.total_misses, 1);
        assert_eq!(table.total_hits, 0);
    }

    #[test]
    fn route_table_remove() {
        let table = RouteTable::new(10);
        let ep = Endpoint::new(crate::types::CarrierType::Loopback, vec![1]).unwrap();
        let entry = RouteEntry::new(
            DatasetId::new([1]),
            RecordId::new([2]),
            ep,
        );
        table.insert(entry).unwrap();
        assert!(table.remove(&DatasetId::new([1]), &RecordId::new([2])));
        assert!(!table.remove(&DatasetId::new([1]), &RecordId::new([2])));
        assert_eq!(table.len(), 0);
    }

    #[test]
    fn route_table_full() {
        let table = RouteTable::new(1);
        let ep = Endpoint::new(crate::types::CarrierType::Loopback, vec![1]).unwrap();
        let entry = RouteEntry::new(
            DatasetId::new([1]),
            RecordId::new([1]),
            ep.clone(),
        );
        table.insert(entry).unwrap();
        assert!(table.is_full());
        let ep2 = Endpoint::new(crate::types::CarrierType::Loopback, vec![2]).unwrap();
        let entry2 = RouteEntry::new(
            DatasetId::new([2]),
            RecordId::new([2]),
            ep2,
        );
        let err = table.insert(entry2).unwrap_err();
        assert!(matches!(err, RoutingError::RoutingTableFull { .. }));
    }

    #[test]
    fn route_table_hit_rate() {
        let table = RouteTable::new(10);
        let ep = Endpoint::new(crate::types::CarrierType::Loopback, vec![1]).unwrap();
        table
            .insert(RouteEntry::new(
                DatasetId::new([1]),
                RecordId::new([1]),
                ep,
            ))
            .unwrap();
        table.lookup(&DatasetId::new([1]), &RecordId::new([1]));
        table.lookup(&DatasetId::new([1]), &RecordId::new([1]));
        table.lookup(&DatasetId::new([2]), &RecordId::new([2]));
        assert!((table.hit_rate() - (2.0 / 3.0)).abs() < f64::EPSILON);
    }

    #[test]
    fn route_table_clear() {
        let table = RouteTable::new(10);
        let ep = Endpoint::new(crate::types::CarrierType::Loopback, vec![1]).unwrap();
        table
            .insert(RouteEntry::new(
                DatasetId::new([1]),
                RecordId::new([1]),
                ep,
            ))
            .unwrap();
        assert_eq!(table.len(), 1);
        table.clear();
        assert_eq!(table.len(), 0);
        assert!(table.is_empty());
    }

    #[test]
    fn route_table_default() {
        let table = RouteTable::default();
        assert_eq!(table.max_routes, 100000);
        assert!(table.is_empty());
    }

    // === LocalityMetrics tests ===
    #[test]
    fn locality_metrics_score() {
        let m1 = LocalityMetrics::new();
        let m2 = LocalityMetrics {
            geographic_distance_km: 5000.0,
            ..Default::default()
        };
        assert!(m1.score() > m2.score());
        assert!((m1.score() - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn locality_metrics_is_preferred() {
        let m1 = LocalityMetrics::new();
        let m2 = LocalityMetrics {
            geographic_distance_km: 10000.0,
            ..Default::default()
        };
        assert!(m1.is_preferred(&m2));
        assert!(!m2.is_preferred(&m1));
    }

    #[test]
    fn locality_metrics_default() {
        let m = LocalityMetrics::default();
        assert_eq!(m.geographic_distance_km, 0.0);
        assert_eq!(m.network_hops, 1);
        assert_eq!(m.estimated_latency_ms, 10);
    }

    // === LocalityAwareSelector tests ===
    #[test]
    fn locality_selector_select_best() {
        let selector = LocalityAwareSelector::new(ConsistencyProfile::Eventual);
        let ep1 = Endpoint::new(crate::types::CarrierType::Loopback, vec![1]).unwrap();
        let ep2 = Endpoint::new(crate::types::CarrierType::Loopback, vec![2]).unwrap();
        selector.set_metrics(
            ep1.clone(),
            LocalityMetrics {
                estimated_latency_ms: 5,
                ..Default::default()
            },
        );
        selector.set_metrics(
            ep2.clone(),
            LocalityMetrics {
                estimated_latency_ms: 50,
                ..Default::default()
            },
        );
        let best = selector.select_best(&[ep1.clone(), ep2.clone()], ConsistencyProfile::Eventual);
        assert_eq!(best.unwrap(), ep1);
    }

    #[test]
    fn locality_selector_empty() {
        let selector = LocalityAwareSelector::new(ConsistencyProfile::Strong);
        let result = selector.select_best(&[], ConsistencyProfile::Strong);
        assert!(result.is_none());
    }

    #[test]
    fn locality_selector_candidate_count() {
        let selector = LocalityAwareSelector::new(ConsistencyProfile::Eventual);
        assert_eq!(selector.candidate_count(), 0);
        let ep = Endpoint::new(crate::types::CarrierType::Loopback, vec![1]).unwrap();
        selector.set_metrics(ep, LocalityMetrics::default());
        assert_eq!(selector.candidate_count(), 1);
    }

    #[test]
    fn locality_selector_clear() {
        let selector = LocalityAwareSelector::new(ConsistencyProfile::Eventual);
        let ep = Endpoint::new(crate::types::CarrierType::Loopback, vec![1]).unwrap();
        selector.set_metrics(ep, LocalityMetrics::default());
        assert_eq!(selector.candidate_count(), 1);
        selector.clear_metrics();
        assert_eq!(selector.candidate_count(), 0);
    }

    // === RoutingConfig tests ===
    #[test]
    fn routing_config_default_and_validate() {
        let config = RoutingConfig::default();
        assert_eq!(config.max_routes, 100000);
        assert_eq!(config.default_hop_count, 1);
        assert_eq!(config.default_latency_ms, 10);
        assert!(config.enable_locality_aware);
        assert!(config.enable_replica_fallback);
        assert_eq!(config.route_stale_threshold_ms, 300000);
        assert!(config.validate().is_ok());

        let mut bad = RoutingConfig::default();
        bad.max_routes = 0;
        assert!(bad.validate().is_err());
    }

    #[test]
    fn routing_config_builder() {
        let config = RoutingConfig::new()
            .with_max_routes(50000)
            .with_default_latency(20)
            .with_locality_aware(false)
            .with_replica_fallback(false);
        assert_eq!(config.max_routes, 50000);
        assert_eq!(config.default_latency_ms, 20);
        assert!(!config.enable_locality_aware);
        assert!(!config.enable_replica_fallback);
    }

    // === RoutingStats tests ===
    #[test]
    fn routing_stats_default_and_rates() {
        let stats = RoutingStats::default();
        assert_eq!(stats.total_lookups, 0);
        assert!((stats.lookup_success_rate() - 1.0).abs() < f64::EPSILON);
        assert!((stats.fallback_rate() - 0.0).abs() < f64::EPSILON);

        let mut stats = stats;
        stats.total_lookups = 100;
        stats.successful_lookups = 90;
        stats.failed_lookups = 10;
        stats.fallback_uses = 5;
        stats.locality_selects = 80;
        assert!((stats.lookup_success_rate() - 0.9).abs() < f64::EPSILON);
        assert!((stats.fallback_rate() - 0.05).abs() < f64::EPSILON);
    }

    // === RoutingPath tests ===
    #[test]
    fn routing_path_add_hop() {
        let mut path = RoutingPath::new();
        assert!(path.is_empty());
        let ep1 = Endpoint::new(crate::types::CarrierType::Loopback, vec![1]).unwrap();
        let ep2 = Endpoint::new(crate::types::CarrierType::Loopback, vec![2]).unwrap();
        path.add_hop(ep1.clone(), 10);
        path.add_hop(ep2.clone(), 20);
        assert!(!path.is_empty());
        assert_eq!(path.total_hops, 2);
        assert_eq!(path.total_latency_ms, 30);
        assert_eq!(path.final_hop().unwrap(), &ep2);
    }

    #[test]
    fn routing_path_default() {
        let path = RoutingPath::default();
        assert!(path.is_empty());
        assert_eq!(path.total_latency_ms, 0);
        assert_eq!(path.total_hops, 0);
        assert!(path.final_hop().is_none());
    }

    // === Routing trait method defaults ===
    #[test]
    fn routing_default_methods() {
        let router = TestRouter::new("test");
        let ep = Endpoint::new(crate::types::CarrierType::Loopback, vec![1]).unwrap();
        assert!(router.is_reachable(&ep));
        assert_eq!(router.hop_count(&ep), 1);
        assert_eq!(router.latency_ms(&ep), 10);
        assert_eq!(router.endpoint_priority(&ep), 0);
        assert_eq!(router.routing_table_size(&DatasetId::new([1])), 0);
        assert!(router.is_routing_enabled(&DatasetId::new([1])));
        assert_eq!(router.endpoint_region(&ep), None);
        assert!(router.flush().is_ok());
        assert!(router.reset().is_ok());
        assert!(router.is_healthy());
    }

    // === TestRouter counts ===
    #[test]
    fn test_router_counts() {
        let router = TestRouter::new("test");
        let ds = DatasetId::new([1]);
        let rid = RecordId::new([2]);
        router.resolve_logical(&ds, &rid).unwrap_err();
        assert_eq!(router.lookups.get(), 1);
        assert_eq!(router.resolutions.get(), 1);
        assert_eq!(router.total_routes(), 0);
    }

    // === RegionId tests ===
    #[test]
    fn region_id_usage() {
        let r1 = RegionId::new([1; 32]);
        let r2 = RegionId::new([2; 32]);
        assert_ne!(r1, r2);
        let r3 = RegionId::new([1; 32]);
        assert_eq!(r1, r3);
    }

    // === RoutingResolver tests ===
    #[test]
    fn routing_resolver_new_and_resolve() {
        let resolver = RoutingResolver::default();
        let ep = Endpoint::new(crate::types::CarrierType::Loopback, vec![1]).unwrap();
        resolver
            .register(
                DatasetId::new([1]),
                RecordId::new([2]),
                ep.clone(),
            )
            .unwrap();
        let result = resolver
            .resolve(&DatasetId::new([1]), &RecordId::new([2]))
            .unwrap();
        assert_eq!(result, ep);
        assert_eq!(resolver.route_count(), 1);
    }

    #[test]
    fn routing_resolver_resolve_not_found() {
        let resolver = RoutingResolver::default();
        let err = resolver
            .resolve(&DatasetId::new([1]), &RecordId::new([2]))
            .unwrap_err();
        assert!(matches!(err, RoutingError::RouteNotFound { .. }));
    }

    #[test]
    fn routing_resolver_select_locality_aware() {
        let resolver = RoutingResolver::default();
        let candidates = vec![
            Endpoint::new(crate::types::CarrierType::Loopback, vec![1]).unwrap(),
            Endpoint::new(crate::types::CarrierType::Loopback, vec![2]).unwrap(),
        ];
        let result = resolver
            .select_locality_aware(
                &DatasetId::new([1]),
                candidates,
                ConsistencyProfile::Eventual,
            )
            .unwrap();
        assert!(!result.address.is_empty());
    }

    #[test]
    fn routing_resolver_select_empty_candidates() {
        let resolver = RoutingResolver::default();
        let err = resolver
            .select_locality_aware(&DatasetId::new([1]), vec![], ConsistencyProfile::Eventual)
            .unwrap_err();
        assert!(matches!(err, RoutingError::NoCandidates { .. }));
    }

    #[test]
    fn routing_resolver_default_config() {
        let resolver = RoutingResolver::default();
        assert!(resolver.config().max_routes > 0);
    }

    // === PathSelector tests ===
    #[test]
    fn path_selector_new_and_select() {
        let selector = PathSelector::new(PathSelectionStrategy::ShortestPath);
        let id1 = selector.create_path();
        let id2 = selector.create_path();
        selector.add_hop_to_path(id1, Endpoint::new(crate::types::CarrierType::Loopback, vec![1]).unwrap(), 10);
        selector.add_hop_to_path(id2, Endpoint::new(crate::types::CarrierType::Loopback, vec![2]).unwrap(), 5);
        let best = selector.select_best_path(&[id1, id2]).unwrap();
        assert_eq!(best, id2);
        assert_eq!(selector.path_count(), 2);
    }

    #[test]
    fn path_selector_get_and_remove_path() {
        let selector = PathSelector::default();
        let id = selector.create_path();
        assert!(selector.get_path(id).is_some());
        assert!(selector.remove_path(id));
        assert!(selector.get_path(id).is_none());
        assert_eq!(selector.path_count(), 0);
    }

    #[test]
    fn path_selector_strategies() {
        let lat = PathSelector::new(PathSelectionStrategy::LowestLatency);
        assert_eq!(lat.strategy.as_str(), "lowest_latency");
        let rand = PathSelector::new(PathSelectionStrategy::RandomPath);
        assert_eq!(rand.strategy.as_str(), "random_path");
        let hbw = PathSelector::new(PathSelectionStrategy::HighestBandwidth);
        assert_eq!(hbw.strategy.as_str(), "highest_bandwidth");
        let loaded = PathSelector::new(PathSelectionStrategy::LeastLoaded);
        assert_eq!(loaded.strategy.as_str(), "least_loaded");
    }

    // === LoopPrevention tests ===
    #[test]
    fn loop_prevention_check_and_mark() {
        let lp = LoopPrevention::new(5);
        let ep = Endpoint::new(crate::types::CarrierType::Loopback, vec![1]).unwrap();
        assert!(lp.check_hop(&ep, 1).is_ok());
        lp.mark_visited(ep.clone());
        assert!(lp.check_hop(&ep, 1).is_err());
        assert_eq!(lp.visited_count(), 1);
        assert_eq!(lp.total_loops(), 1);
    }

    #[test]
    fn loop_prevention_max_hops() {
        let lp = LoopPrevention::new(1);
        let ep = Endpoint::new(crate::types::CarrierType::Loopback, vec![1]).unwrap();
        assert!(lp.check_hop(&ep, 5).is_err());
        assert_eq!(lp.total_loops(), 1);
        lp.clear_visited();
        assert_eq!(lp.visited_count(), 0);
    }

    #[test]
    fn loop_prevention_default_max_hops() {
        let lp = LoopPrevention::default();
        assert_eq!(lp.max_hops, 255);
    }

    // === RouteExpiration tests ===
    #[test]
    fn route_expiration_set_and_check() {
        let re = RouteExpiration::new(1000);
        re.set_ttl(DatasetId::new([1]), RecordId::new([2]), 5000);
        assert!(!re.is_expired(&DatasetId::new([1]), &RecordId::new([2]), 1000));
        assert!(re.is_expired(&DatasetId::new([1]), &RecordId::new([2]), 6000));
        assert_eq!(re.get_status(&DatasetId::new([1]), &RecordId::new([2])), Some(RouteStatus::Active));
    }

    #[test]
    fn route_expiration_mark_expired() {
        let re = RouteExpiration::new(1000);
        re.set_ttl(DatasetId::new([1]), RecordId::new([2]), 5000);
        re.mark_expired(&DatasetId::new([1]), &RecordId::new([2]));
        assert!(re.is_expired(&DatasetId::new([1]), &RecordId::new([2]), 1000));
        assert_eq!(re.get_status(&DatasetId::new([1]), &RecordId::new([2])), Some(RouteStatus::Expired));
    }

    #[test]
    fn route_expiration_unknown_is_expired() {
        let re = RouteExpiration::new(1000);
        assert!(re.is_expired(&DatasetId::new([1]), &RecordId::new([2]), 1000));
    }

    #[test]
    fn route_expiration_default_ttl() {
        let re = RouteExpiration::default();
        assert_eq!(re.default_ttl_ms, 300000);
    }

    // === ConvergenceEngine tests ===
    #[test]
    fn convergence_engine_states() {
        let ce = ConvergenceEngine::new();
        assert!(!ce.is_converged());
        ce.start_convergence();
        assert!(!ce.is_converged());
        ce.complete_convergence(1000, true);
        assert!(ce.is_converged());
        assert_eq!(ce.route_version(), 1);
        ce.complete_convergence(2000, false);
        assert!(!ce.is_converged());
    }

    #[test]
    fn convergence_engine_rounds_and_version() {
        let ce = ConvergenceEngine::new();
        ce.start_convergence();
        ce.start_convergence();
        assert_eq!(ce.convergence_rounds, 2);
        ce.increment_version();
        assert_eq!(ce.route_version(), 1);
    }

    // === LocalityMetricsCollector tests ===
    #[test]
    fn locality_metrics_collector_record_and_average() {
        let lmc = LocalityMetricsCollector::default();
        let ep = Endpoint::new(crate::types::CarrierType::Loopback, vec![1]).unwrap();
        lmc.record_latency(ep.clone(), 10);
        lmc.record_latency(ep.clone(), 20);
        lmc.record_latency(ep.clone(), 30);
        assert_eq!(lmc.average_latency(&ep).unwrap(), 20);
        assert_eq!(lmc.measurement_count(&ep), 3);
        assert_eq!(lmc.total_measurements(), 3);
    }

    #[test]
    fn locality_metrics_collector_average_empty() {
        let lmc = LocalityMetricsCollector::default();
        let ep = Endpoint::new(crate::types::CarrierType::Loopback, vec![1]).unwrap();
        assert_eq!(lmc.average_latency(&ep), None);
        assert_eq!(lmc.measurement_count(&ep), 0);
    }

    #[test]
    fn locality_metrics_collector_clear() {
        let lmc = LocalityMetricsCollector::default();
        let ep = Endpoint::new(crate::types::CarrierType::Loopback, vec![1]).unwrap();
        lmc.record_latency(ep.clone(), 10);
        lmc.clear();
        assert_eq!(lmc.measurement_count(&ep), 0);
    }

    // === WorkloadAwareRouter tests ===
    #[test]
    fn workload_aware_router_load() {
        let war = WorkloadAwareRouter::new(100);
        let ep = Endpoint::new(crate::types::CarrierType::Loopback, vec![1]).unwrap();
        war.set_load(ep.clone(), 50);
        assert_eq!(war.get_load(&ep), 50);
        assert!(!war.is_overloaded(&ep));
    }

    #[test]
    fn workload_aware_router_overloaded() {
        let war = WorkloadAwareRouter::new(100);
        let ep = Endpoint::new(crate::types::CarrierType::Loopback, vec![1]).unwrap();
        war.set_load(ep.clone(), 150);
        assert!(war.is_overloaded(&ep));
    }

    #[test]
    fn workload_aware_router_select_least_loaded() {
        let war = WorkloadAwareRouter::new(1000);
        let ep1 = Endpoint::new(crate::types::CarrierType::Loopback, vec![1]).unwrap();
        let ep2 = Endpoint::new(crate::types::CarrierType::Loopback, vec![2]).unwrap();
        war.set_load(ep1.clone(), 10);
        war.set_load(ep2.clone(), 5);
        let selected = war.select_least_loaded(&[ep1.clone(), ep2.clone()]).unwrap();
        assert_eq!(selected, ep2);
    }

    #[test]
    fn workload_aware_router_default_max_load() {
        let war = WorkloadAwareRouter::default();
        assert_eq!(war.max_load, 10000);
    }

    // === ResourceAwareRouter tests ===
    #[test]
    fn resource_aware_router_get_set() {
        let rar = ResourceAwareRouter::default();
        let ep = Endpoint::new(crate::types::CarrierType::Loopback, vec![1]).unwrap();
        rar.set_resource(ep.clone(), ResourceType::CPU, 80);
        assert_eq!(rar.get_resource(&ep, ResourceType::CPU), Some(80));
        assert_eq!(rar.get_resource(&ep, ResourceType::Memory), None);
    }

    #[test]
    fn resource_aware_router_has_capacity() {
        let rar = ResourceAwareRouter::default();
        let ep = Endpoint::new(crate::types::CarrierType::Loopback, vec![1]).unwrap();
        rar.set_resource(ep.clone(), ResourceType::Storage, 100);
        assert!(rar.has_capacity(&ep, ResourceType::Storage, 50));
        assert!(!rar.has_capacity(&ep, ResourceType::Storage, 150));
    }

    #[test]
    fn resource_aware_router_resource_type_str() {
        assert_eq!(ResourceType::CPU.as_str(), "cpu");
        assert_eq!(ResourceType::Memory.as_str(), "memory");
        assert_eq!(ResourceType::Storage.as_str(), "storage");
        assert_eq!(ResourceType::Bandwidth.as_str(), "bandwidth");
    }

    // === MultiPathRouter tests ===
    #[test]
    fn multi_path_router_add_and_get() {
        let mpr = MultiPathRouter::default();
        let ep1 = Endpoint::new(crate::types::CarrierType::Loopback, vec![1]).unwrap();
        let ep2 = Endpoint::new(crate::types::CarrierType::Loopback, vec![2]).unwrap();
        mpr.add_path(DatasetId::new([1]), ep1.clone(), 10);
        mpr.add_path(DatasetId::new([1]), ep2.clone(), 20);
        let paths = mpr.get_paths(&DatasetId::new([1]));
        assert_eq!(paths.len(), 2);
        assert_eq!(mpr.path_count(&DatasetId::new([1])), 2);
        assert_eq!(mpr.total_path_count(), 2);
    }

    #[test]
    fn multi_path_router_weight() {
        let mpr = MultiPathRouter::default();
        let ep = Endpoint::new(crate::types::CarrierType::Loopback, vec![1]).unwrap();
        mpr.add_path(DatasetId::new([1]), ep.clone(), 42);
        assert_eq!(mpr.get_weight(&DatasetId::new([1]), &ep), 42);
        assert_eq!(mpr.get_weight(&DatasetId::new([1]), &ep), 42);
        assert_eq!(mpr.get_weight(&DatasetId::new([2]), &ep), 1);
    }

    // === RouteValidator tests ===
    #[test]
    fn route_validator_validate_endpoint() {
        let mut rv = RouteValidator::new();
        let valid_ep = Endpoint::new(crate::types::CarrierType::Loopback, vec![1]).unwrap();
        let invalid_ep = Endpoint::new(crate::types::CarrierType::Loopback, vec![]).unwrap();
        assert!(rv.validate_endpoint(&valid_ep));
        assert!(!rv.validate_endpoint(&invalid_ep));
        assert!(rv.pass_rate() < 1.0);
    }

    #[test]
    fn route_validator_pass_rate() {
        let mut rv = RouteValidator::new();
        let ep = Endpoint::new(crate::types::CarrierType::Loopback, vec![1]).unwrap();
        rv.validate_endpoint(&ep);
        rv.validate_endpoint(&ep);
        assert_eq!(rv.pass_rate(), 1.0);
        assert_eq!(rv.validations, 2);
    }

    #[test]
    fn route_validator_validate_route() {
        let mut rv = RouteValidator::new();
        let ep = Endpoint::new(crate::types::CarrierType::Loopback, vec![1]).unwrap();
        assert!(rv.validate_route(
            &DatasetId::new([1]),
            &RecordId::new([2]),
            &ep,
        ));
    }

    // === EndpointHealthTracker tests ===
    #[test]
    fn endpoint_health_tracker_set_and_get() {
        let eht = EndpointHealthTracker::default();
        let ep = Endpoint::new(crate::types::CarrierType::Loopback, vec![1]).unwrap();
        eht.set_health(ep.clone(), EndpointHealthStatus::Healthy);
        assert_eq!(eht.get_health(&ep), EndpointHealthStatus::Healthy);
        assert_eq!(eht.healthy_count(), 1);
    }

    #[test]
    fn endpoint_health_tracker_unknown_default() {
        let eht = EndpointHealthTracker::default();
        let ep = Endpoint::new(crate::types::CarrierType::Loopback, vec![1]).unwrap();
        assert_eq!(eht.get_health(&ep), EndpointHealthStatus::Unknown);
    }

    #[test]
    fn endpoint_health_status_str() {
        assert_eq!(EndpointHealthStatus::Healthy.as_str(), "healthy");
        assert_eq!(EndpointHealthStatus::Degraded.as_str(), "degraded");
        assert_eq!(EndpointHealthStatus::Unhealthy.as_str(), "unhealthy");
        assert_eq!(EndpointHealthStatus::Unknown.as_str(), "unknown");
        assert!(EndpointHealthStatus::Healthy.is_healthy());
        assert!(!EndpointHealthStatus::Unhealthy.is_healthy());
    }

    // === RouteCache tests ===
    #[test]
    fn route_cache_put_and_get() {
        let rc = RouteCache::new(100);
        let ep = Endpoint::new(crate::types::CarrierType::Loopback, vec![1]).unwrap();
        rc.put(DatasetId::new([1]), RecordId::new([2]), ep.clone());
        assert_eq!(rc.get(&DatasetId::new([1]), &RecordId::new([2])), Some(ep));
        assert_eq!(rc.len(), 1);
    }

    #[test]
    fn route_cache_miss() {
        let rc = RouteCache::new(100);
        assert_eq!(rc.get(&DatasetId::new([1]), &RecordId::new([2])), None);
        assert_eq!(rc.hits, 0);
        assert_eq!(rc.misses, 1);
    }

    #[test]
    fn route_cache_hit_rate() {
        let rc = RouteCache::new(100);
        let ep = Endpoint::new(crate::types::CarrierType::Loopback, vec![1]).unwrap();
        rc.put(DatasetId::new([1]), RecordId::new([2]), ep.clone());
        rc.get(&DatasetId::new([1]), &RecordId::new([2]));
        rc.get(&DatasetId::new([1]), &RecordId::new([3]));
        assert_eq!(rc.hit_rate(), 0.5);
    }

    #[test]
    fn route_cache_clear() {
        let rc = RouteCache::new(100);
        let ep = Endpoint::new(crate::types::CarrierType::Loopback, vec![1]).unwrap();
        rc.put(DatasetId::new([1]), RecordId::new([2]), ep.clone());
        rc.clear();
        assert_eq!(rc.len(), 0);
    }

    // === PrefixRouter tests ===
    #[test]
    fn prefix_router_register_and_lookup() {
        let pr = PrefixRouter::default();
        let ep = Endpoint::new(crate::types::CarrierType::Loopback, vec![1]).unwrap();
        pr.register_prefix(vec![1, 2, 3], ep.clone());
        assert_eq!(pr.lookup_prefix(&[1, 2, 3, 4]), Some(ep.clone()));
        assert_eq!(pr.lookup_prefix(&[1, 2]), None);
    }

    #[test]
    fn prefix_router_longest_match() {
        let pr = PrefixRouter::default();
        let ep1 = Endpoint::new(crate::types::CarrierType::Loopback, vec![1]).unwrap();
        let ep2 = Endpoint::new(crate::types::CarrierType::Loopback, vec![2]).unwrap();
        pr.register_prefix(vec![1], ep1.clone());
        pr.register_prefix(vec![1, 2], ep2.clone());
        assert_eq!(pr.lookup_prefix(&[1, 2, 3]), Some(ep2));
        assert_eq!(pr.prefix_count(), 2);
    }

    // === HashRouter tests ===
    #[test]
    fn hash_router_route_by_hash() {
        let hr = HashRouter::default();
        let ep1 = Endpoint::new(crate::types::CarrierType::Loopback, vec![1]).unwrap();
        let ep2 = Endpoint::new(crate::types::CarrierType::Loopback, vec![2]).unwrap();
        hr.add_endpoint(ep1.clone());
        hr.add_endpoint(ep2.clone());
        let result = hr.route_by_hash(b"key").unwrap();
        assert!(result == ep1 || result == ep2);
        assert_eq!(hr.endpoint_count(), 2);
    }

    #[test]
    fn hash_router_empty() {
        let hr = HashRouter::default();
        assert_eq!(hr.route_by_hash(b"key"), None);
    }

    // === ConsistentHashRouter tests ===
    #[test]
    fn consistent_hash_router_route() {
        let chr = ConsistentHashRouter::new(100);
        let ep1 = Endpoint::new(crate::types::CarrierType::Loopback, vec![1]).unwrap();
        let ep2 = Endpoint::new(crate::types::CarrierType::Loopback, vec![2]).unwrap();
        chr.add_node(ep1.clone(), 3);
        chr.add_node(ep2.clone(), 3);
        let result = chr.route(b"key").unwrap();
        assert!(result == ep1 || result == ep2);
        assert_eq!(chr.node_count(), 2);
    }

    #[test]
    fn consistent_hash_router_empty() {
        let chr = ConsistentHashRouter::new(100);
        assert_eq!(chr.route(b"key"), None);
    }

    // === RouteOptimizer tests ===
    #[test]
    fn route_optimizer_optimize() {
        let ro = RouteOptimizer::default();
        let mut path = RoutingPath::new();
        path.add_hop(Endpoint::new(crate::types::CarrierType::Loopback, vec![1]).unwrap(), 10);
        path.add_hop(Endpoint::new(crate::types::CarrierType::Loopback, vec![1]).unwrap(), 10);
        assert!(ro.optimize_route(&mut path));
        assert_eq!(ro.optimization_rounds(), 1);
        assert_eq!(ro.routes_optimized(), 1);
    }

    #[test]
    fn route_optimizer_disabled() {
        let ro = RouteOptimizer::default();
        ro.disable_optimization();
        assert!(!ro.is_enabled());
        let mut path = RoutingPath::new();
        path.add_hop(Endpoint::new(crate::types::CarrierType::Loopback, vec![1]).unwrap(), 10);
        path.add_hop(Endpoint::new(crate::types::CarrierType::Loopback, vec![1]).unwrap(), 10);
        assert!(!ro.optimize_route(&mut path));
    }

    // === TrafficManager tests ===
    #[test]
    fn traffic_manager_record_and_get() {
        let tm = TrafficManager::default();
        let ep = Endpoint::new(crate::types::CarrierType::Loopback, vec![1]).unwrap();
        tm.record_traffic(ep.clone(), 100);
        tm.record_traffic(ep.clone(), 200);
        assert_eq!(tm.get_traffic(&ep), 300);
        assert_eq!(tm.total_traffic(), 300);
    }

    #[test]
    fn traffic_manager_default_total() {
        let tm = TrafficManager::default();
        assert_eq!(tm.total_traffic(), 0);
    }

    // === BandwidthManager tests ===
    #[test]
    fn bandwidth_manager_limit_and_usage() {
        let bm = BandwidthManager::default();
        let ep = Endpoint::new(crate::types::CarrierType::Loopback, vec![1]).unwrap();
        bm.set_limit(ep.clone(), 1000);
        assert_eq!(bm.get_limit(&ep), Some(1000));
        bm.record_usage(ep.clone(), 500);
        assert!(bm.is_within_limit(&ep));
        bm.record_usage(ep.clone(), 600);
        assert!(!bm.is_within_limit(&ep));
    }

    // === FailoverRouter tests ===
    #[test]
    fn failover_router_primary_and_backup() {
        let fr = FailoverRouter::new(FailoverStrategy::Immediate);
        let primary = Endpoint::new(crate::types::CarrierType::Loopback, vec![1]).unwrap();
        let backup = Endpoint::new(crate::types::CarrierType::Loopback, vec![2]).unwrap();
        fr.set_primary(primary.clone());
        fr.add_backup(backup.clone());
        assert_eq!(fr.get_primary(), Some(primary));
        assert_eq!(fr.failover(), Some(backup));
        assert_eq!(fr.failover_count(), 1);
    }

    #[test]
    fn failover_router_strategies() {
        assert_eq!(FailoverStrategy::Immediate.as_str(), "immediate");
        assert_eq!(FailoverStrategy::Graceful.as_str(), "graceful");
        assert_eq!(FailoverStrategy::Manual.as_str(), "manual");
    }

    // === GeographicRouter tests ===
    #[test]
    fn geographic_router_assign_and_lookup() {
        let gr = GeographicRouter::default();
        let ep = Endpoint::new(crate::types::CarrierType::Loopback, vec![1]).unwrap();
        let region = RegionId::new([1; 32]);
        gr.assign_region(ep.clone(), region.clone());
        assert_eq!(gr.get_region(&ep), Some(region));
        let eps = gr.get_endpoints_in_region(&region);
        assert_eq!(eps.len(), 1);
        assert_eq!(eps[0], ep);
        assert_eq!(gr.region_count(), 1);
    }

    // === TopologyMapper tests ===
    #[test]
    fn topology_mapper_add_node_and_link() {
        let tm = TopologyMapper::default();
        let ep1 = Endpoint::new(crate::types::CarrierType::Loopback, vec![1]).unwrap();
        let ep2 = Endpoint::new(crate::types::CarrierType::Loopback, vec![2]).unwrap();
        tm.add_node(ep1.clone());
        tm.add_link(ep1.clone(), ep2.clone(), 42);
        assert_eq!(tm.node_count(), 2);
        assert_eq!(tm.link_count(), 1);
        assert_eq!(tm.get_link_cost(&ep1, &ep2), Some(42));
    }

    #[test]
    fn topology_mapper_unknown_link() {
        let tm = TopologyMapper::default();
        let ep1 = Endpoint::new(crate::types::CarrierType::Loopback, vec![1]).unwrap();
        let ep2 = Endpoint::new(crate::types::CarrierType::Loopback, vec![2]).unwrap();
        assert_eq!(tm.get_link_cost(&ep1, &ep2), None);
    }

    // === RouteAnalytics tests ===
    #[test]
    fn route_analytics_record_and_rates() {
        let mut ra = RouteAnalytics::new();
        ra.record_resolution(true, 3, 10);
        ra.record_resolution(true, 2, 5);
        ra.record_resolution(false, 1, 2);
        ra.record_failover();
        assert_eq!(ra.total_resolutions, 3);
        assert_eq!(ra.total_failures, 1);
        assert_eq!(ra.total_failovers, 1);
        assert!((ra.success_rate() - 0.66).abs() < 0.01);
        assert!((ra.failure_rate() - 0.34).abs() < 0.01);
    }

    #[test]
    fn route_analytics_default() {
        let ra = RouteAnalytics::default();
        assert_eq!(ra.total_resolutions, 0);
        assert_eq!(ra.total_failures, 0);
        assert_eq!(ra.failure_rate(), 0.0);
    }

    // === LoadBalancer tests ===
    #[test]
    fn load_balancer_select_round_robin() {
        let lb = LoadBalancer::new(LoadBalancingStrategy::RoundRobin);
        let ep1 = Endpoint::new(crate::types::CarrierType::Loopback, vec![1]).unwrap();
        let ep2 = Endpoint::new(crate::types::CarrierType::Loopback, vec![2]).unwrap();
        lb.add_endpoint(ep1.clone(), 1);
        lb.add_endpoint(ep2.clone(), 1);
        let first = lb.select().unwrap();
        let second = lb.select().unwrap();
        assert_ne!(first, second);
    }

    #[test]
    fn load_balancer_least_connections() {
        let lb = LoadBalancer::new(LoadBalancingStrategy::LeastConnections);
        let ep1 = Endpoint::new(crate::types::CarrierType::Loopback, vec![1]).unwrap();
        let ep2 = Endpoint::new(crate::types::CarrierType::Loopback, vec![2]).unwrap();
        lb.add_endpoint(ep1.clone(), 1);
        lb.add_endpoint(ep2.clone(), 1);
        lb.increment_connections(&ep1);
        let selected = lb.select().unwrap();
        assert_eq!(selected, ep2);
    }

    #[test]
    fn load_balancer_empty() {
        let lb = LoadBalancer::default();
        assert_eq!(lb.select(), None);
    }

    #[test]
    fn load_balancing_strategy_str() {
        assert_eq!(LoadBalancingStrategy::RoundRobin.as_str(), "round_robin");
        assert_eq!(LoadBalancingStrategy::LeastConnections.as_str(), "least_connections");
        assert_eq!(LoadBalancingStrategy::WeightedRoundRobin.as_str(), "weighted_round_robin");
        assert_eq!(LoadBalancingStrategy::SourceHash.as_str(), "source_hash");
    }

    // === SessionRouter tests ===
    #[test]
    fn session_router_route_and_get() {
        let sr = SessionRouter::default();
        let session_id = crate::types::SessionID::new([1]);
        let ep = Endpoint::new(crate::types::CarrierType::Loopback, vec![1]).unwrap();
        sr.route_session(session_id, ep.clone());
        assert_eq!(sr.get_session_route(session_id), Some(ep));
        assert_eq!(sr.session_count(), 1);
    }

    #[test]
    fn session_router_remove() {
        let sr = SessionRouter::default();
        let session_id = crate::types::SessionID::new([1]);
        let ep = Endpoint::new(crate::types::CarrierType::Loopback, vec![1]).unwrap();
        sr.route_session(session_id, ep.clone());
        assert!(sr.remove_session(session_id));
        assert!(!sr.remove_session(session_id));
        assert_eq!(sr.session_count(), 0);
    }

    // === DatagramRouter tests ===
    #[test]
    fn datagram_router_route_and_drop() {
        let dr = DatagramRouter::new(100);
        assert!(dr.route_datagram(50));
        assert!(!dr.route_datagram(200));
        assert_eq!(dr.datagrams_routed, 1);
        assert_eq!(dr.datagrams_dropped, 1);
        assert!((dr.drop_rate() - 0.5).abs() < 0.01);
    }

    #[test]
    fn datagram_router_max_size() {
        let dr = DatagramRouter::default();
        assert_eq!(dr.max_datagram_size(), 65535);
    }

    // === RoutingError extended methods tests ===
    #[test]
    fn routing_error_is_recoverable() {
        let ep = Endpoint::new(crate::types::CarrierType::Loopback, vec![1]).unwrap();
        assert!(RoutingError::EndpointUnreachable { endpoint: ep.clone() }.is_recoverable());
        assert!(!RoutingError::RouteNotFound {
            dataset: DatasetId::new([1]),
            record: RecordId::new([2]),
        }
        .is_recoverable());
    }

    #[test]
    fn routing_error_requires_re_route() {
        assert!(RoutingError::RouteNotFound {
            dataset: DatasetId::new([1]),
            record: RecordId::new([2]),
        }
        .requires_re_route());
        let ep = Endpoint::new(crate::types::CarrierType::Loopback, vec![1]).unwrap();
        assert!(RoutingError::EndpointUnreachable { endpoint: ep }.requires_re_route());
        assert!(!RoutingError::Internal {
            message: "x".to_string(),
        }
        .requires_re_route());
    }
}