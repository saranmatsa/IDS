//! # Sealed Trait: Discovery — node/H-unit/region discovery (gossip)
//!
//! Carrier-agnostic: operates on opaque Endpoints and IDs only.
//! No std::net, tokio::net, libp2p, IP, TCP, UDP, QUIC, HTTP, or DNS.
//!
//! The Discovery trait defines how nodes, H-units, and regions discover
//! each other in a carrier-agnostic distributed system. Implementations
//! live downstream in ids-discovery and ids-* crates.

use crate::types::{
    ConsistencyProfile, DatasetId, Endpoint, HUnitId, NodeId, OrderingPos, RecordId, RegionId,
};
use crate::errors::DiscoveryError;

///////////////////////////////////////////////////////////////////////////////
// SEALED TRAIT: Discovery
///////////////////////////////////////////////////////////////////////////////

mod private {
    pub trait Sealed {}
}

impl private::Sealed for super::DiscoveryEngine {}
impl private::Sealed for super::GossipDiscovery {}
impl private::Sealed for super::NeighborDiscovery {}
impl private::Sealed for super::DiscoveryManager {}

/// Sealed trait for node / H-unit / region discovery.
/// Downstream crates can call methods and use as trait bounds but cannot implement.
pub trait Discovery: private::Sealed {
    /// Discover all nodes known to the local discovery engine.
    fn discover_nodes(&self) -> Result<Vec<NodeInfo>, DiscoveryError>;

    /// Discover H-units responsible for a dataset in a region.
    fn discover_hunits(
        &self,
        dataset_id: &DatasetId,
        region_id: &RegionId,
    ) -> Result<Vec<HUnitInfo>, DiscoveryError>;

    /// Discover regions in the network.
    fn discover_regions(&self) -> Result<Vec<RegionInfo>, DiscoveryError>;

    /// Register a node with the discovery engine.
    fn register_node(&self, node: NodeInfo) -> Result<(), DiscoveryError>;

    /// Deregister a node from the discovery engine.
    fn deregister_node(&self, node_id: &NodeId) -> Result<(), DiscoveryError>;

    /// Announce presence to peers (gossip).
    fn announce_presence(&self, node: NodeInfo) -> Result<(), DiscoveryError>;

    /// Gossip discovery state to a peer endpoint.
    fn gossip_to(&self, endpoint: &Endpoint, state: GossipState) -> Result<(), DiscoveryError>;

    /// Receive gossip state from a peer endpoint.
    fn receive_gossip(&self, endpoint: &Endpoint, state: GossipState) -> Result<(), DiscoveryError>;

    /// Look up an endpoint for a node ID.
    fn resolve_node(&self, node_id: &NodeId) -> Result<Endpoint, DiscoveryError>;

    /// Look up node IDs for an endpoint (reverse resolution).
    fn resolve_endpoint(&self, endpoint: &Endpoint) -> Result<Vec<NodeId>, DiscoveryError>;

    /// Get the discovery name for debugging.
    fn discovery_name(&self) -> &'static str;

    /// Get the total number of known nodes.
    fn node_count(&self) -> usize;

    /// Get the total number of known H-units.
    fn hunit_count(&self) -> usize;

    /// Get the total number of known regions.
    fn region_count(&self) -> usize;
}

///////////////////////////////////////////////////////////////////////////////
// DISCOVERY ERROR
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq)]
pub enum DiscoveryError {
    #[error("node not found: {node_id:?}")]
    NodeNotFound { node_id: NodeId },

    #[error("H-unit not found: {hunit_id:?}")]
    HUnitNotFound { hunit_id: HUnitId },

    #[error("region not found: {region_id:?}")]
    RegionNotFound { region_id: RegionId },

    #[error("endpoint unreachable: {endpoint:?}")]
    EndpointUnreachable { endpoint: Endpoint },

    #[error("discovery table full: max={max}")]
    DiscoveryTableFull { max: usize },

    #[error("duplicate node: {node_id:?}")]
    DuplicateNode { node_id: NodeId },

    #[error("duplicate H-unit: {hunit_id:?}")]
    DuplicateHUnit { hunit_id: HUnitId },

    #[error("duplicate region: {region_id:?}")]
    DuplicateRegion { region_id: RegionId },

    #[error("gossip failed: endpoint={endpoint:?}, reason={reason}")]
    GossipFailed { endpoint: Endpoint, reason: String },

    #[error("resolve failed: node={node_id:?}, reason={reason}")]
    ResolveFailed { node_id: NodeId, reason: String },

    #[error("announcement failed: reason={reason}")]
    AnnouncementFailed { reason: String },

    #[error("gossip state corrupt: reason={reason}")]
    GossipStateCorrupt { reason: String },

    #[error("peer timeout: {endpoint:?}")]
    PeerTimeout { endpoint: Endpoint },

    #[error("invalid gossip message: reason={reason}")]
    InvalidGossipMessage { reason: String },

    #[error("discovery engine not initialized")]
    NotInitialized,

    #[error("internal discovery error: {message}")]
    Internal { message: String },
}

impl DiscoveryError {
    pub fn category(&self) -> &'static str {
        match self {
            DiscoveryError::NodeNotFound { .. } => "node_not_found",
            DiscoveryError::HUnitNotFound { .. } => "hunit_not_found",
            DiscoveryError::RegionNotFound { .. } => "region_not_found",
            DiscoveryError::EndpointUnreachable { .. } => "endpoint_unreachable",
            DiscoveryError::DiscoveryTableFull { .. } => "table_full",
            DiscoveryError::DuplicateNode { .. } => "duplicate_node",
            DiscoveryError::DuplicateHUnit { .. } => "duplicate_hunit",
            DiscoveryError::DuplicateRegion { .. } => "duplicate_region",
            DiscoveryError::GossipFailed { .. } => "gossip_failed",
            DiscoveryError::ResolveFailed { .. } => "resolve_failed",
            DiscoveryError::AnnouncementFailed { .. } => "announcement_failed",
            DiscoveryError::GossipStateCorrupt { .. } => "gossip_corrupt",
            DiscoveryError::PeerTimeout { .. } => "peer_timeout",
            DiscoveryError::InvalidGossipMessage { .. } => "invalid_gossip",
            DiscoveryError::NotInitialized => "not_initialized",
            DiscoveryError::Internal { .. } => "internal",
        }
    }

    pub fn is_recoverable(&self) -> bool {
        matches!(
            self,
            DiscoveryError::EndpointUnreachable { .. }
                | DiscoveryError::PeerTimeout { .. }
                | DiscoveryError::GossipFailed { .. }
        )
    }

    pub fn requires_re_discovery(&self) -> bool {
        matches!(
            self,
            DiscoveryError::NodeNotFound { .. }
                | DiscoveryError::HUnitNotFound { .. }
                | DiscoveryError::RegionNotFound { .. }
                | DiscoveryError::ResolveFailed { .. }
        )
    }

    pub fn suggested_action(&self) -> &'static str {
        match self {
            DiscoveryError::NodeNotFound { .. } => "re-gossip discovery state",
            DiscoveryError::HUnitNotFound { .. } => "query region directory",
            DiscoveryError::RegionNotFound { .. } => "broadcast region announcement",
            DiscoveryError::EndpointUnreachable { .. } => "try alternate endpoint",
            DiscoveryError::DiscoveryTableFull { .. } => "expire stale entries",
            DiscoveryError::DuplicateNode { .. } => "update existing entry",
            DiscoveryError::DuplicateHUnit { .. } => "merge H-unit records",
            DiscoveryError::DuplicateRegion { .. } => "merge region records",
            DiscoveryError::GossipFailed { .. } => "retry gossip to peer",
            DiscoveryError::ResolveFailed { .. } => "refresh discovery cache",
            DiscoveryError::AnnouncementFailed { .. } => "retry announcement",
            DiscoveryError::GossipStateCorrupt { .. } => "discard corrupt gossip",
            DiscoveryError::PeerTimeout { .. } => "mark peer stale and re-discover",
            DiscoveryError::InvalidGossipMessage { .. } => "ignore invalid message",
            DiscoveryError::NotInitialized => "initialize discovery engine",
            DiscoveryError::Internal { .. } => "contact support",
        }
    }
}

///////////////////////////////////////////////////////////////////////////////
// NODE INFO
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeInfo {
    pub node_id: NodeId,
    pub endpoint: Endpoint,
    pub region_id: RegionId,
    pub capabilities: Vec<String>,
    pub last_seen_ms: u64,
    pub is_healthy: bool,
}

impl NodeInfo {
    pub fn new(node_id: NodeId, endpoint: Endpoint, region_id: RegionId) -> Self {
        Self {
            node_id,
            endpoint,
            region_id,
            capabilities: Vec::new(),
            last_seen_ms: 0,
            is_healthy: true,
        }
    }

    pub fn with_capabilities(mut self, capabilities: Vec<String>) -> Self {
        self.capabilities = capabilities;
        self
    }

    pub fn with_last_seen(mut self, last_seen_ms: u64) -> Self {
        self.last_seen_ms = last_seen_ms;
        self
    }

    pub fn mark_healthy(mut self) -> Self {
        self.is_healthy = true;
        self
    }

    pub fn mark_unhealthy(mut self) -> Self {
        self.is_healthy = false;
        self
    }

    pub fn is_stale(&self, now_ms: u64, stale_threshold_ms: u64) -> bool {
        now_ms.saturating_sub(self.last_seen_ms) > stale_threshold_ms
    }

    pub fn has_capability(&self, capability: &str) -> bool {
        self.capabilities.iter().any(|c| c == capability)
    }
}

///////////////////////////////////////////////////////////////////////////////
// H-UNIT INFO
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HUnitInfo {
    pub hunit_id: HUnitId,
    pub node_id: NodeId,
    pub region_id: RegionId,
    pub datasets: Vec<DatasetId>,
    pub capacity_total: u64,
    pub capacity_used: u64,
    pub is_active: bool,
}

impl HUnitInfo {
    pub fn new(
        hunit_id: HUnitId,
        node_id: NodeId,
        region_id: RegionId,
    ) -> Self {
        Self {
            hunit_id,
            node_id,
            region_id,
            datasets: Vec::new(),
            capacity_total: 0,
            capacity_used: 0,
            is_active: true,
        }
    }

    pub fn with_datasets(mut self, datasets: Vec<DatasetId>) -> Self {
        self.datasets = datasets;
        self
    }

    pub fn with_capacity(mut self, total: u64, used: u64) -> Self {
        self.capacity_total = total;
        self.capacity_used = used;
        self
    }

    pub fn mark_active(mut self) -> Self {
        self.is_active = true;
        self
    }

    pub fn mark_inactive(mut self) -> Self {
        self.is_active = false;
        self
    }

    pub fn available_capacity(&self) -> u64 {
        self.capacity_total.saturating_sub(self.capacity_used)
    }

    pub fn utilization_percent(&self) -> f64 {
        if self.capacity_total == 0 {
            return 0.0;
        }
        (self.capacity_used as f64 / self.capacity_total as f64) * 100.0
    }

    pub fn hosts_dataset(&self, dataset_id: &DatasetId) -> bool {
        self.datasets.iter().any(|d| d == dataset_id)
    }
}

///////////////////////////////////////////////////////////////////////////////
// REGION INFO
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegionInfo {
    pub region_id: RegionId,
    pub nodes: Vec<NodeId>,
    pub hunits: Vec<HUnitId>,
    pub is_primary: bool,
    pub latency_ms: u64,
}

impl RegionInfo {
    pub fn new(region_id: RegionId, is_primary: bool) -> Self {
        Self {
            region_id,
            nodes: Vec::new(),
            hunits: Vec::new(),
            is_primary,
            latency_ms: 0,
        }
    }

    pub fn with_nodes(mut self, nodes: Vec<NodeId>) -> Self {
        self.nodes = nodes;
        self
    }

    pub fn with_hunits(mut self, hunits: Vec<HUnitId>) -> Self {
        self.hunits = hunits;
        self
    }

    pub fn with_latency(mut self, latency_ms: u64) -> Self {
        self.latency_ms = latency_ms;
        self
    }

    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    pub fn hunit_count(&self) -> usize {
        self.hunits.len()
    }

    pub fn has_node(&self, node_id: &NodeId) -> bool {
        self.nodes.iter().any(|n| n == node_id)
    }
}

///////////////////////////////////////////////////////////////////////////////
// GOSSIP STATE
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GossipState {
    pub sender_node_id: NodeId,
    pub nodes: Vec<NodeInfo>,
    pub hunits: Vec<HUnitInfo>,
    pub regions: Vec<RegionInfo>,
    pub sequence_number: u64,
    pub timestamp_ms: u64,
}

impl GossipState {
    pub fn new(sender_node_id: NodeId) -> Self {
        Self {
            sender_node_id,
            nodes: Vec::new(),
            hunits: Vec::new(),
            regions: Vec::new(),
            sequence_number: 0,
            timestamp_ms: 0,
        }
    }

    pub fn with_sequence(mut self, seq: u64, timestamp_ms: u64) -> Self {
        self.sequence_number = seq;
        self.timestamp_ms = timestamp_ms;
        self
    }

    pub fn add_node(mut self, node: NodeInfo) -> Self {
        self.nodes.push(node);
        self
    }

    pub fn add_hunit(mut self, hunit: HUnitInfo) -> Self {
        self.hunits.push(hunit);
        self
    }

    pub fn add_region(mut self, region: RegionInfo) -> Self {
        self.regions.push(region);
        self
    }

    pub fn is_newer_than(&self, other: &GossipState) -> bool {
        if self.sequence_number != other.sequence_number {
            return self.sequence_number > other.sequence_number;
        }
        self.timestamp_ms > other.timestamp_ms
    }

    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    pub fn hunit_count(&self) -> usize {
        self.hunits.len()
    }

    pub fn region_count(&self) -> usize {
        self.regions.len()
    }
}

///////////////////////////////////////////////////////////////////////////////
// DISCOVERY CONFIG
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone)]
pub struct DiscoveryConfig {
    pub max_nodes: usize,
    pub max_hunits: usize,
    pub max_regions: usize,
    pub gossip_interval_ms: u64,
    pub stale_threshold_ms: u64,
    pub announcement_ttl_ms: u64,
    pub max_gossip_peers: usize,
}

impl DiscoveryConfig {
    pub fn new() -> Self {
        Self {
            max_nodes: 10000,
            max_hunits: 100000,
            max_regions: 1000,
            gossip_interval_ms: 5000,
            stale_threshold_ms: 300000,
            announcement_ttl_ms: 60000,
            max_gossip_peers: 50,
        }
    }

    pub fn with_max_nodes(mut self, max: usize) -> Self {
        self.max_nodes = max;
        self
    }

    pub fn with_gossip_interval(mut self, ms: u64) -> Self {
        self.gossip_interval_ms = ms;
        self
    }

    pub fn with_stale_threshold(mut self, ms: u64) -> Self {
        self.stale_threshold_ms = ms;
        self
    }
}

impl Default for DiscoveryConfig {
    fn default() -> Self {
        Self::new()
    }
}

///////////////////////////////////////////////////////////////////////////////
// DISCOVERY ENGINE
///////////////////////////////////////////////////////////////////////////////

pub struct DiscoveryEngine {
    pub nodes: std::cell::RefCell<std::collections::HashMap<NodeId, NodeInfo>>,
    pub hunits: std::cell::RefCell<std::collections::HashMap<HUnitId, HUnitInfo>>,
    pub regions: std::cell::RefCell<std::collections::HashMap<RegionId, RegionInfo>>,
    pub endpoint_to_nodes: std::cell::RefCell<std::collections::HashMap<Endpoint, Vec<NodeId>>>,
    pub config: DiscoveryConfig,
    pub gossip_count: u64,
    pub announcement_count: u64,
    pub resolve_count: u64,
}

impl DiscoveryEngine {
    pub fn new(config: DiscoveryConfig) -> Self {
        Self {
            nodes: std::cell::RefCell::new(std::collections::HashMap::new()),
            hunits: std::cell::RefCell::new(std::collections::HashMap::new()),
            regions: std::cell::RefCell::new(std::collections::HashMap::new()),
            endpoint_to_nodes: std::cell::RefCell::new(std::collections::HashMap::new()),
            config,
            gossip_count: 0,
            announcement_count: 0,
            resolve_count: 0,
        }
    }

    pub fn register_node(&self, node: NodeInfo) -> Result<(), DiscoveryError> {
        let mut nodes = self.nodes.borrow_mut();
        if nodes.len() >= self.config.max_nodes
            && !nodes.contains_key(&node.node_id)
        {
            return Err(DiscoveryError::DiscoveryTableFull {
                max: self.config.max_nodes,
            });
        }
        if nodes.contains_key(&node.node_id) {
            return Err(DiscoveryError::DuplicateNode {
                node_id: node.node_id.clone(),
            });
        }
        let endpoint_key = node.endpoint.clone();
        let node_id = node.node_id.clone();
        nodes.insert(node.node_id.clone(), node);
        drop(nodes);
        let mut ep_map = self.endpoint_to_nodes.borrow_mut();
        ep_map
            .entry(endpoint_key)
            .or_default()
            .push(node_id.clone());
        drop(ep_map);
        self.announcement_count += 1;
        Ok(())
    }

    pub fn deregister_node(&self, node_id: &NodeId) -> Result<(), DiscoveryError> {
        let mut nodes = self.nodes.borrow_mut();
        let node = match nodes.remove(node_id) {
            Some(n) => n,
            None => return Err(DiscoveryError::NodeNotFound {
                node_id: node_id.clone(),
            }),
        };
        drop(nodes);
        let mut ep_map = self.endpoint_to_nodes.borrow_mut();
        if let Some(node_ids) = ep_map.get_mut(&node.endpoint) {
            node_ids.retain(|id| id != node_id);
            if node_ids.is_empty() {
                ep_map.remove(&node.endpoint);
            }
        }
        Ok(())
    }

    pub fn discover_nodes(&self) -> Result<Vec<NodeInfo>, DiscoveryError> {
        let nodes = self.nodes.borrow();
        Ok(nodes.values().cloned().collect())
    }

    pub fn resolve_node(&self, node_id: &NodeId) -> Result<Endpoint, DiscoveryError> {
        let nodes = self.nodes.borrow();
        match nodes.get(node_id) {
            Some(node) => Ok(node.endpoint.clone()),
            None => Err(DiscoveryError::NodeNotFound {
                node_id: node_id.clone(),
            }),
        }
    }

    pub fn resolve_endpoint(
        &self,
        endpoint: &Endpoint,
    ) -> Result<Vec<NodeId>, DiscoveryError> {
        let ep_map = self.endpoint_to_nodes.borrow();
        match ep_map.get(endpoint) {
            Some(ids) => Ok(ids.clone()),
            None => Err(DiscoveryError::EndpointUnreachable {
                endpoint: endpoint.clone(),
            }),
        }
    }

    pub fn register_hunit(&self, hunit: HUnitInfo) -> Result<(), DiscoveryError> {
        let mut hunits = self.hunits.borrow_mut();
        if hunits.len() >= self.config.max_hunits && !hunits.contains_key(&hunit.hunit_id) {
            return Err(DiscoveryError::DiscoveryTableFull {
                max: self.config.max_hunits,
            });
        }
        if hunits.contains_key(&hunit.hunit_id) {
            return Err(DiscoveryError::DuplicateHUnit {
                hunit_id: hunit.hunit_id.clone(),
            });
        }
        hunits.insert(hunit.hunit_id.clone(), hunit);
        Ok(())
    }

    pub fn register_region(&self, region: RegionInfo) -> Result<(), DiscoveryError> {
        let mut regions = self.regions.borrow_mut();
        if regions.len() >= self.config.max_regions && !regions.contains_key(&region.region_id) {
            return Err(DiscoveryError::DiscoveryTableFull {
                max: self.config.max_regions,
            });
        }
        if regions.contains_key(&region.region_id) {
            return Err(DiscoveryError::DuplicateRegion {
                region_id: region.region_id.clone(),
            });
        }
        regions.insert(region.region_id.clone(), region);
        Ok(())
    }

    pub fn discover_hunits(
        &self,
        dataset_id: &DatasetId,
        region_id: &RegionId,
    ) -> Result<Vec<HUnitInfo>, DiscoveryError> {
        let hunits = self.hunits.borrow();
        let regions = self.regions.borrow();
        if !regions.contains_key(region_id) {
            return Err(DiscoveryError::RegionNotFound {
                region_id: region_id.clone(),
            });
        }
        drop(regions);
        Ok(hunits
            .values()
            .filter(|h| h.region_id == *region_id && h.hosts_dataset(dataset_id))
            .cloned()
            .collect())
    }

    pub fn discover_regions(&self) -> Result<Vec<RegionInfo>, DiscoveryError> {
        let regions = self.regions.borrow();
        Ok(regions.values().cloned().collect())
    }

    pub fn gossip_count(&self) -> u64 {
        self.gossip_count
    }

    pub fn announcement_count(&self) -> u64 {
        self.announcement_count
    }

    pub fn resolve_count(&self) -> u64 {
        self.resolve_count
    }

    pub fn node_count(&self) -> usize {
        self.nodes.borrow().len()
    }

    pub fn hunit_count(&self) -> usize {
        self.hunits.borrow().len()
    }

    pub fn region_count(&self) -> usize {
        self.regions.borrow().len()
    }
}

impl Discovery for DiscoveryEngine {
    fn discover_nodes(&self) -> Result<Vec<NodeInfo>, DiscoveryError> {
        self.discover_nodes()
    }

    fn discover_hunits(
        &self,
        dataset_id: &DatasetId,
        region_id: &RegionId,
    ) -> Result<Vec<HUnitInfo>, DiscoveryError> {
        self.discover_hunits(dataset_id, region_id)
    }

    fn discover_regions(&self) -> Result<Vec<RegionInfo>, DiscoveryError> {
        self.discover_regions()
    }

    fn register_node(&self, node: NodeInfo) -> Result<(), DiscoveryError> {
        self.register_node(node)
    }

    fn deregister_node(&self, node_id: &NodeId) -> Result<(), DiscoveryError> {
        self.deregister_node(node_id)
    }

    fn announce_presence(&self, node: NodeInfo) -> Result<(), DiscoveryError> {
        self.register_node(node)?;
        self.announcement_count += 1;
        Ok(())
    }

    fn gossip_to(&self, endpoint: &Endpoint, _state: GossipState) -> Result<(), DiscoveryError> {
        self.resolve_endpoint(endpoint)?;
        self.gossip_count += 1;
        Ok(())
    }

    fn receive_gossip(&self, _endpoint: &Endpoint, _state: GossipState) -> Result<(), DiscoveryError> {
        self.gossip_count += 1;
        Ok(())
    }

    fn resolve_node(&self, node_id: &NodeId) -> Result<Endpoint, DiscoveryError> {
        self.resolve_node(node_id)
    }

    fn resolve_endpoint(&self, endpoint: &Endpoint) -> Result<Vec<NodeId>, DiscoveryError> {
        self.resolve_endpoint(endpoint)
    }

    fn discovery_name(&self) -> &'static str {
        "discovery_engine"
    }

    fn node_count(&self) -> usize {
        self.node_count()
    }

    fn hunit_count(&self) -> usize {
        self.hunit_count()
    }

    fn region_count(&self) -> usize {
        self.region_count()
    }
}

impl Default for DiscoveryEngine {
    fn default() -> Self {
        Self::new(DiscoveryConfig::default())
    }
}

///////////////////////////////////////////////////////////////////////////////
// GOSSIP DISCOVERY
///////////////////////////////////////////////////////////////////////////////

pub struct GossipDiscovery {
    pub engine: std::cell::RefCell<DiscoveryEngine>,
    pub peer_endpoints: std::cell::RefCell<Vec<Endpoint>>,
    pub gossip_rounds: u64,
}

impl GossipDiscovery {
    pub fn new(engine: DiscoveryEngine) -> Self {
        Self {
            engine: std::cell::RefCell::new(engine),
            peer_endpoints: std::cell::RefCell::new(Vec::new()),
            gossip_rounds: 0,
        }
    }

    pub fn add_peer(&self, endpoint: Endpoint) {
        self.peer_endpoints.borrow_mut().push(endpoint);
    }

    pub fn gossip_round(&self) -> Result<(), DiscoveryError> {
        let engine = self.engine.borrow();
        let peers = self.peer_endpoints.borrow();
        let state = GossipState::new(engine.nodes.borrow().values().next().map(|n| n.node_id.clone()).unwrap_or_default());
        drop(engine);
        drop(peers);
        self.gossip_rounds += 1;
        Ok(())
    }

    pub fn gossip_rounds(&self) -> u64 {
        self.gossip_rounds
    }

    pub fn peer_count(&self) -> usize {
        self.peer_endpoints.borrow().len()
    }

    pub fn node_count(&self) -> usize {
        self.engine.borrow().node_count()
    }
}

impl Default for GossipDiscovery {
    fn default() -> Self {
        Self::new(DiscoveryEngine::default())
    }
}

///////////////////////////////////////////////////////////////////////////////
// NEIGHBOR DISCOVERY
///////////////////////////////////////////////////////////////////////////////

pub struct NeighborDiscovery {
    pub neighbors: std::cell::RefCell<std::collections::HashMap<Endpoint, u64>>,
    pub discovery_count: u64,
}

impl NeighborDiscovery {
    pub fn new() -> Self {
        Self {
            neighbors: std::cell::RefCell::new(std::collections::HashMap::new()),
            discovery_count: 0,
        }
    }

    pub fn add_neighbor(&self, endpoint: Endpoint, last_seen_ms: u64) {
        self.neighbors
            .borrow_mut()
            .insert(endpoint, last_seen_ms);
        self.discovery_count += 1;
    }

    pub fn remove_neighbor(&self, endpoint: &Endpoint) -> bool {
        self.neighbors.borrow_mut().remove(endpoint).is_some()
    }

    pub fn get_neighbors(&self) -> Vec<Endpoint> {
        self.neighbors.borrow().keys().cloned().collect()
    }

    pub fn neighbor_count(&self) -> usize {
        self.neighbors.borrow().len()
    }

    pub fn is_neighbor(&self, endpoint: &Endpoint) -> bool {
        self.neighbors.borrow().contains_key(endpoint)
    }

    pub fn discovery_count(&self) -> u64 {
        self.discovery_count
    }
}

impl Default for NeighborDiscovery {
    fn default() -> Self {
        Self::new()
    }
}

///////////////////////////////////////////////////////////////////////////////
// DISCOVERY MANAGER
///////////////////////////////////////////////////////////////////////////////

pub struct DiscoveryManager {
    pub engine: std::cell::RefCell<DiscoveryEngine>,
    pub gossip: std::cell::RefCell<GossipDiscovery>,
    pub neighbors: std::cell::RefCell<NeighborDiscovery>,
    pub discovery_rounds: u64,
}

impl DiscoveryManager {
    pub fn new() -> Self {
        Self {
            engine: std::cell::RefCell::new(DiscoveryEngine::default()),
            gossip: std::cell::RefCell::new(GossipDiscovery::default()),
            neighbors: std::cell::RefCell::new(NeighborDiscovery::default()),
            discovery_rounds: 0,
        }
    }

    pub fn register_node(&self, node: NodeInfo) -> Result<(), DiscoveryError> {
        self.engine.borrow_mut().register_node(node)
    }

    pub fn discover_all(&self) -> Result<DiscoverySummary, DiscoveryError> {
        let engine = self.engine.borrow();
        let nodes = engine.discover_nodes()?;
        let regions = engine.discover_regions()?;
        let hunit_count = engine.hunit_count();
        drop(engine);
        self.discovery_rounds += 1;
        Ok(DiscoverySummary {
            node_count: nodes.len(),
            hunit_count,
            region_count: regions.len(),
        })
    }

    pub fn add_gossip_peer(&self, endpoint: Endpoint) {
        self.gossip.borrow_mut().add_peer(endpoint);
    }

    pub fn add_neighbor(&self, endpoint: Endpoint, last_seen_ms: u64) {
        self.neighbors
            .borrow_mut()
            .add_neighbor(endpoint, last_seen_ms);
    }

    pub fn discovery_rounds(&self) -> u64 {
        self.discovery_rounds
    }

    pub fn total_node_count(&self) -> usize {
        self.engine.borrow().node_count()
    }
}

impl Default for DiscoveryManager {
    fn default() -> Self {
        Self::new()
    }
}

///////////////////////////////////////////////////////////////////////////////
// DISCOVERY SUMMARY
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoverySummary {
    pub node_count: usize,
    pub hunit_count: usize,
    pub region_count: usize,
}

///////////////////////////////////////////////////////////////////////////////
// DISCOVERY CACHE
///////////////////////////////////////////////////////////////////////////////

pub struct DiscoveryCache {
    pub node_cache: std::cell::RefCell<std::collections::HashMap<NodeId, Endpoint>>,
    pub hunit_cache: std::cell::RefCell<std::collections::HashMap<(DatasetId, RegionId), Vec<HUnitId>>>,
    pub region_cache: std::cell::RefCell<std::collections::HashMap<RegionId, RegionInfo>>,
    pub max_entries: usize,
    pub hits: u64,
    pub misses: u64,
}

impl DiscoveryCache {
    pub fn new(max_entries: usize) -> Self {
        Self {
            node_cache: std::cell::RefCell::new(std::collections::HashMap::new()),
            hunit_cache: std::cell::RefCell::new(std::collections::HashMap::new()),
            region_cache: std::cell::RefCell::new(std::collections::HashMap::new()),
            max_entries,
            hits: 0,
            misses: 0,
        }
    }

    pub fn cache_node(&self, node_id: NodeId, endpoint: Endpoint) {
        if self.node_cache.borrow().len() >= self.max_entries {
            if let Some(key) = self.node_cache.borrow().keys().next().cloned() {
                self.node_cache.borrow_mut().remove(&key);
            }
        }
        self.node_cache.borrow_mut().insert(node_id, endpoint);
    }

    pub fn get_cached_node(&self, node_id: &NodeId) -> Option<Endpoint> {
        match self.node_cache.borrow().get(node_id) {
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

    pub fn cache_hunits(
        &self,
        dataset_id: DatasetId,
        region_id: RegionId,
        hunit_ids: Vec<HUnitId>,
    ) {
        if self.hunit_cache.borrow().len() >= self.max_entries {
            if let Some(key) = self.hunit_cache.borrow().keys().next().cloned() {
                self.hunit_cache.borrow_mut().remove(&key);
            }
        }
        self.hunit_cache
            .borrow_mut()
            .insert((dataset_id, region_id), hunit_ids);
    }

    pub fn get_cached_hunits(
        &self,
        dataset_id: &DatasetId,
        region_id: &RegionId,
    ) -> Option<Vec<HUnitId>> {
        match self.hunit_cache.borrow().get(&(dataset_id.clone(), region_id.clone())) {
            Some(ids) => {
                self.hits += 1;
                Some(ids.clone())
            }
            None => {
                self.misses += 1;
                None
            }
        }
    }

    pub fn cache_region(&self, region: RegionInfo) {
        if self.region_cache.borrow().len() >= self.max_entries {
            if let Some(key) = self.region_cache.borrow().keys().next().cloned() {
                self.region_cache.borrow_mut().remove(&key);
            }
        }
        self.region_cache
            .borrow_mut()
            .insert(region.region_id.clone(), region);
    }

    pub fn get_cached_region(&self, region_id: &RegionId) -> Option<RegionInfo> {
        match self.region_cache.borrow().get(region_id) {
            Some(region) => {
                self.hits += 1;
                Some(region.clone())
            }
            None => {
                self.misses += 1;
                None
            }
        }
    }

    pub fn hit_rate(&self) -> f64 {
        let total = self.hits + self.misses;
        if total == 0 {
            return 0.0;
        }
        self.hits as f64 / total as f64
    }

    pub fn clear(&self) {
        self.node_cache.borrow_mut().clear();
        self.hunit_cache.borrow_mut().clear();
        self.region_cache.borrow_mut().clear();
    }

    pub fn entry_count(&self) -> usize {
        self.node_cache.borrow().len()
            + self.hunit_cache.borrow().len()
            + self.region_cache.borrow().len()
    }
}

impl Default for DiscoveryCache {
    fn default() -> Self {
        Self::new(10000)
    }
}

///////////////////////////////////////////////////////////////////////////////
// DISCOVERY EXPIRATION
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiscoveryEntryStatus {
    Fresh,
    Stale,
    Expired,
}

impl DiscoveryEntryStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            DiscoveryEntryStatus::Fresh => "fresh",
            DiscoveryEntryStatus::Stale => "stale",
            DiscoveryEntryStatus::Expired => "expired",
        }
    }

    pub fn is_fresh(&self) -> bool {
        matches!(self, DiscoveryEntryStatus::Fresh)
    }

    pub fn is_expired(&self) -> bool {
        matches!(self, DiscoveryEntryStatus::Expired)
    }
}

pub struct DiscoveryExpiration {
    pub entry_ttls: std::cell::RefCell<
        std::collections::HashMap<NodeId, (u64, DiscoveryEntryStatus)>,
    >,
    pub default_ttl_ms: u64,
    pub expired_count: u64,
}

impl DiscoveryExpiration {
    pub fn new(default_ttl_ms: u64) -> Self {
        Self {
            entry_ttls: std::cell::RefCell::new(std::collections::HashMap::new()),
            default_ttl_ms,
            expired_count: 0,
        }
    }

    pub fn set_ttl(&self, node_id: NodeId, ttl_ms: u64) {
        self.entry_ttls
            .borrow_mut()
            .insert(node_id, (ttl_ms, DiscoveryEntryStatus::Fresh));
    }

    pub fn get_status(
        &self,
        node_id: &NodeId,
        now_ms: u64,
    ) -> DiscoveryEntryStatus {
        let ttl_map = self.entry_ttls.borrow();
        match ttl_map.get(node_id) {
            Some((ttl, status)) => {
                if *status == DiscoveryEntryStatus::Expired {
                    DiscoveryEntryStatus::Expired
                } else if now_ms >= *ttl {
                    DiscoveryEntryStatus::Expired
                } else if now_ms.saturating_sub(*ttl / 2) > *ttl {
                    DiscoveryEntryStatus::Stale
                } else {
                    DiscoveryEntryStatus::Fresh
                }
            }
            None => DiscoveryEntryStatus::Expired,
        }
    }

    pub fn mark_expired(&self, node_id: &NodeId) {
        self.entry_ttls.borrow_mut().insert(
            node_id.clone(),
            (0, DiscoveryEntryStatus::Expired),
        );
        self.expired_count += 1;
    }

    pub fn mark_fresh(&self, node_id: NodeId, ttl_ms: u64) {
        self.entry_ttls
            .borrow_mut()
            .insert(node_id, (ttl_ms, DiscoveryEntryStatus::Fresh));
    }

    pub fn expired_count(&self) -> u64 {
        self.expired_count
    }

    pub fn default_ttl_ms(&self) -> u64 {
        self.default_ttl_ms
    }

    pub fn entry_count(&self) -> usize {
        self.entry_ttls.borrow().len()
    }
}

impl Default for DiscoveryExpiration {
    fn default() -> Self {
        Self::new(300000)
    }
}

///////////////////////////////////////////////////////////////////////////////
// DISCOVERY VALIDATOR
///////////////////////////////////////////////////////////////////////////////

pub struct DiscoveryValidator {
    pub validations: u64,
    pub validation_failures: u64,
    pub invalid_endpoints: u64,
    pub duplicate_nodes: u64,
}

impl DiscoveryValidator {
    pub fn new() -> Self {
        Self {
            validations: 0,
            validation_failures: 0,
            invalid_endpoints: 0,
            duplicate_nodes: 0,
        }
    }

    pub fn validate_node_info(&mut self, node: &NodeInfo) -> bool {
        self.validations += 1;
        if node.endpoint.address.is_empty() {
            self.validation_failures += 1;
            self.invalid_endpoints += 1;
            return false;
        }
        true
    }

    pub fn validate_gossip_state(&mut self, state: &GossipState) -> bool {
        self.validations += 1;
        if state.nodes.is_empty() && state.hunits.is_empty() && state.regions.is_empty() {
            self.validation_failures += 1;
            false
        } else {
            true
        }
    }

    pub fn record_duplicate(&mut self) {
        self.duplicate_nodes += 1;
    }

    pub fn pass_rate(&self) -> f64 {
        if self.validations == 0 {
            return 1.0;
        }
        (self.validations - self.validation_failures) as f64 / self.validations as f64
    }
}

impl Default for DiscoveryValidator {
    fn default() -> Self {
        Self::new()
    }
}

///////////////////////////////////////////////////////////////////////////////
// REGIONAL DISCOVERY
///////////////////////////////////////////////////////////////////////////////

pub struct RegionalDiscovery {
    pub region_nodes: std::cell::RefCell<
        std::collections::HashMap<RegionId, Vec<NodeId>>,
    >,
    pub cross_region_links: std::cell::RefCell<
        std::collections::HashMap<(RegionId, RegionId), u64>,
    >,
}

impl RegionalDiscovery {
    pub fn new() -> Self {
        Self {
            region_nodes: std::cell::RefCell::new(std::collections::HashMap::new()),
            cross_region_links: std::cell::RefCell::new(std::collections::HashMap::new()),
        }
    }

    pub fn assign_node_to_region(&self, region_id: RegionId, node_id: NodeId) {
        self.region_nodes
            .borrow_mut()
            .entry(region_id)
            .or_default()
            .push(node_id);
    }

    pub fn get_nodes_in_region(&self, region_id: &RegionId) -> Vec<NodeId> {
        self.region_nodes
            .borrow()
            .get(region_id)
            .cloned()
            .unwrap_or_default()
    }

    pub fn add_cross_region_link(
        &self,
        region_a: RegionId,
        region_b: RegionId,
        latency_ms: u64,
    ) {
        self.cross_region_links
            .borrow_mut()
            .insert((region_a, region_b), latency_ms);
    }

    pub fn get_cross_region_latency(
        &self,
        region_a: &RegionId,
        region_b: &RegionId,
    ) -> Option<u64> {
        self.cross_region_links
            .borrow()
            .get(&(region_a.clone(), region_b.clone()))
            .copied()
    }

    pub fn region_count(&self) -> usize {
        self.region_nodes.borrow().len()
    }

    pub fn cross_region_link_count(&self) -> usize {
        self.cross_region_links.borrow().len()
    }
}

impl Default for RegionalDiscovery {
    fn default() -> Self {
        Self::new()
    }
}

///////////////////////////////////////////////////////////////////////////////
// CROSS REGION DISCOVERY
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CrossRegionMode {
    Direct,
    ViaGateway,
    ViaRelay,
}

impl CrossRegionMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            CrossRegionMode::Direct => "direct",
            CrossRegionMode::ViaGateway => "via_gateway",
            CrossRegionMode::ViaRelay => "via_relay",
        }
    }
}

pub struct CrossRegionDiscovery {
    pub mode: CrossRegionMode,
    pub gateways: std::cell::RefCell<std::collections::HashMap<RegionId, Endpoint>>,
    pub region_latencies: std::cell::RefCell<
        std::collections::HashMap<(RegionId, RegionId), u64>,
    >,
}

impl CrossRegionDiscovery {
    pub fn new(mode: CrossRegionMode) -> Self {
        Self {
            mode,
            gateways: std::cell::RefCell::new(std::collections::HashMap::new()),
            region_latencies: std::cell::RefCell::new(std::collections::HashMap::new()),
        }
    }

    pub fn set_gateway(&self, region_id: RegionId, endpoint: Endpoint) {
        self.gateways.borrow_mut().insert(region_id, endpoint);
    }

    pub fn get_gateway(&self, region_id: &RegionId) -> Option<Endpoint> {
        self.gateways.borrow().get(region_id).cloned()
    }

    pub fn set_region_latency(
        &self,
        region_a: RegionId,
        region_b: RegionId,
        latency_ms: u64,
    ) {
        self.region_latencies
            .borrow_mut()
            .insert((region_a, region_b), latency_ms);
    }

    pub fn get_region_latency(
        &self,
        region_a: &RegionId,
        region_b: &RegionId,
    ) -> Option<u64> {
        self.region_latencies
            .borrow()
            .get(&(region_a.clone(), region_b.clone()))
            .copied()
    }

    pub fn gateway_count(&self) -> usize {
        self.gateways.borrow().len()
    }
}

impl Default for CrossRegionDiscovery {
    fn default() -> Self {
        Self::new(CrossRegionMode::Direct)
    }
}

///////////////////////////////////////////////////////////////////////////////
// DISCOVERY TOPOLOGY
///////////////////////////////////////////////////////////////////////////////

pub struct DiscoveryTopology {
    pub links: std::cell::RefCell<std::collections::HashMap<(NodeId, NodeId), u64>>,
    pub nodes: std::cell::RefCell<std::collections::HashSet<NodeId>>,
}

impl DiscoveryTopology {
    pub fn new() -> Self {
        Self {
            links: std::cell::RefCell::new(std::collections::HashMap::new()),
            nodes: std::cell::RefCell::new(std::collections::HashSet::new()),
        }
    }

    pub fn add_node(&self, node_id: NodeId) {
        self.nodes.borrow_mut().insert(node_id);
    }

    pub fn add_link(&self, node_a: NodeId, node_b: NodeId, cost: u64) {
        self.nodes.borrow_mut().insert(node_a.clone());
        self.nodes.borrow_mut().insert(node_b.clone());
        self.links
            .borrow_mut()
            .insert((node_a, node_b), cost);
    }

    pub fn get_link_cost(&self, node_a: &NodeId, node_b: &NodeId) -> Option<u64> {
        self.links
            .borrow()
            .get(&(node_a.clone(), node_b.clone()))
            .copied()
    }

    pub fn node_count(&self) -> usize {
        self.nodes.borrow().len()
    }

    pub fn link_count(&self) -> usize {
        self.links.borrow().len()
    }
}

impl Default for DiscoveryTopology {
    fn default() -> Self {
        Self::new()
    }
}

///////////////////////////////////////////////////////////////////////////////
// GOSSIP PROTOCOL
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GossipMessageType {
    NodeAnnounce,
    HUnitAnnounce,
    RegionAnnounce,
    StateSync,
    Heartbeat,
}

impl GossipMessageType {
    pub fn as_str(&self) -> &'static str {
        match self {
            GossipMessageType::NodeAnnounce => "node_announce",
            GossipMessageType::HUnitAnnounce => "hunit_announce",
            GossipMessageType::RegionAnnounce => "region_announce",
            GossipMessageType::StateSync => "state_sync",
            GossipMessageType::Heartbeat => "heartbeat",
        }
    }
}

pub struct GossipProtocol {
    pub messages_sent: u64,
    pub messages_received: u64,
    pub message_type_counts: std::cell::RefCell<
        std::collections::HashMap<GossipMessageType, u64>,
    >,
}

impl GossipProtocol {
    pub fn new() -> Self {
        Self {
            messages_sent: 0,
            messages_received: 0,
            message_type_counts: std::cell::RefCell::new(std::collections::HashMap::new()),
        }
    }

    pub fn record_sent(&mut self, msg_type: GossipMessageType) {
        self.messages_sent += 1;
        self.message_type_counts
            .borrow_mut()
            .entry(msg_type)
            .and_modify(|v| *v += 1)
            .or_insert(1);
    }

    pub fn record_received(&mut self, msg_type: GossipMessageType) {
        self.messages_received += 1;
        self.message_type_counts
            .borrow_mut()
            .entry(msg_type)
            .and_modify(|v| *v += 1)
            .or_insert(1);
    }

    pub fn total_messages(&self) -> u64 {
        self.messages_sent + self.messages_received
    }

    pub fn get_type_count(&self, msg_type: GossipMessageType) -> u64 {
        self.message_type_counts
            .borrow()
            .get(&msg_type)
            .copied()
            .unwrap_or(0)
    }
}

impl Default for GossipProtocol {
    fn default() -> Self {
        Self::new()
    }
}

///////////////////////////////////////////////////////////////////////////////
// DISCOVERY STATISTICS
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone, Default)]
pub struct DiscoveryStatistics {
    pub total_discoveries: u64,
    pub total_resolutions: u64,
    pub total_failures: u64,
    pub total_gossips: u64,
    pub total_announcements: u64,
    pub average_resolution_time_ms: f64,
}

impl DiscoveryStatistics {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn record_discovery(&mut self) {
        self.total_discoveries += 1;
    }

    pub fn record_resolution(&mut self, time_ms: u64, success: bool) {
        self.total_resolutions += 1;
        if !success {
            self.total_failures += 1;
        }
        self.average_resolution_time_ms =
            (self.average_resolution_time_ms * (self.total_resolutions - 1) as f64
                + time_ms as f64)
                / self.total_resolutions as f64;
    }

    pub fn record_gossip(&mut self) {
        self.total_gossips += 1;
    }

    pub fn record_announcement(&mut self) {
        self.total_announcements += 1;
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
}

///////////////////////////////////////////////////////////////////////////////
// NODE HEALTH TRACKER
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeHealthStatus {
    Healthy,
    Degraded,
    Unhealthy,
    Unknown,
}

impl NodeHealthStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            NodeHealthStatus::Healthy => "healthy",
            NodeHealthStatus::Degraded => "degraded",
            NodeHealthStatus::Unhealthy => "unhealthy",
            NodeHealthStatus::Unknown => "unknown",
        }
    }

    pub fn is_healthy(&self) -> bool {
        matches!(self, NodeHealthStatus::Healthy)
    }
}

pub struct NodeHealthTracker {
    pub health: std::cell::RefCell<std::collections::HashMap<NodeId, NodeHealthStatus>>,
    pub health_checks: u64,
}

impl NodeHealthTracker {
    pub fn new() -> Self {
        Self {
            health: std::cell::RefCell::new(std::collections::HashMap::new()),
            health_checks: 0,
        }
    }

    pub fn set_health(&self, node_id: NodeId, status: NodeHealthStatus) {
        self.health.borrow_mut().insert(node_id, status);
        self.health_checks += 1;
    }

    pub fn get_health(&self, node_id: &NodeId) -> NodeHealthStatus {
        self.health
            .borrow()
            .get(node_id)
            .copied()
            .unwrap_or(NodeHealthStatus::Unknown)
    }

    pub fn healthy_count(&self) -> usize {
        self.health
            .borrow()
            .values()
            .filter(|s| matches!(s, NodeHealthStatus::Healthy))
            .count()
    }

    pub fn total_checks(&self) -> u64 {
        self.health_checks
    }

    pub fn node_count(&self) -> usize {
        self.health.borrow().len()
    }
}

impl Default for NodeHealthTracker {
    fn default() -> Self {
        Self::new()
    }
}

///////////////////////////////////////////////////////////////////////////////
// REGION DIRECTORY
///////////////////////////////////////////////////////////////////////////////

pub struct RegionDirectory {
    pub regions: std::cell::RefCell<std::collections::HashMap<RegionId, RegionInfo>>,
    pub region_node_map: std::cell::RefCell<
        std::collections::HashMap<RegionId, Vec<NodeId>>,
    >,
}

impl RegionDirectory {
    pub fn new() -> Self {
        Self {
            regions: std::cell::RefCell::new(std::collections::HashMap::new()),
            region_node_map: std::cell::RefCell::new(std::collections::HashMap::new()),
        }
    }

    pub fn register_region(&self, region: RegionInfo) {
        self.regions
            .borrow_mut()
            .insert(region.region_id.clone(), region);
    }

    pub fn get_region(&self, region_id: &RegionId) -> Option<RegionInfo> {
        self.regions.borrow().get(region_id).cloned()
    }

    pub fn add_node_to_region(&self, region_id: RegionId, node_id: NodeId) {
        self.region_node_map
            .borrow_mut()
            .entry(region_id)
            .or_default()
            .push(node_id);
    }

    pub fn get_nodes_in_region(&self, region_id: &RegionId) -> Vec<NodeId> {
        self.region_node_map
            .borrow()
            .get(region_id)
            .cloned()
            .unwrap_or_default()
    }

    pub fn region_count(&self) -> usize {
        self.regions.borrow().len()
    }

    pub fn remove_region(&self, region_id: &RegionId) -> bool {
        self.regions.borrow_mut().remove(region_id).is_some()
            && self.region_node_map.borrow_mut().remove(region_id).is_some()
    }
}

impl Default for RegionDirectory {
    fn default() -> Self {
        Self::new()
    }
}

///////////////////////////////////////////////////////////////////////////////
// DISCOVERY INTEGRATION
///////////////////////////////////////////////////////////////////////////////

pub struct DiscoveryIntegration {
    pub bound_subsystems: std::cell::RefCell<Vec<String>>,
    pub integration_events: u64,
}

impl DiscoveryIntegration {
    pub fn new() -> Self {
        Self {
            bound_subsystems: std::cell::RefCell::new(Vec::new()),
            integration_events: 0,
        }
    }

    pub fn bind_subsystem(&self, subsystem: String) {
        self.bound_subsystems.borrow_mut().push(subsystem);
        self.integration_events += 1;
    }

    pub fn is_bound(&self, subsystem: &str) -> bool {
        self.bound_subsystems
            .borrow()
            .iter()
            .any(|s| s == subsystem)
    }

    pub fn bound_count(&self) -> usize {
        self.bound_subsystems.borrow().len()
    }

    pub fn event_count(&self) -> u64 {
        self.integration_events
    }

    pub fn clear_bindings(&self) {
        self.bound_subsystems.borrow_mut().clear();
    }
}

impl Default for DiscoveryIntegration {
    fn default() -> Self {
        Self::new()
    }
}

///////////////////////////////////////////////////////////////////////////////
// DISCOVERY WIRE FORMAT
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiscoveryWireVersion {
    V1,
    V2,
}

impl DiscoveryWireVersion {
    pub fn as_str(&self) -> &'static str {
        match self {
            DiscoveryWireVersion::V1 => "discovery_wire_v1",
            DiscoveryWireVersion::V2 => "discovery_wire_v2",
        }
    }

    pub fn header_size(&self) -> usize {
        match self {
            DiscoveryWireVersion::V1 => 8,
            DiscoveryWireVersion::V2 => 16,
        }
    }
}

pub struct DiscoveryWireFormat {
    pub version: DiscoveryWireVersion,
    pub encoded_messages: u64,
    pub decoded_messages: u64,
}

impl DiscoveryWireFormat {
    pub fn new(version: DiscoveryWireVersion) -> Self {
        Self {
            version,
            encoded_messages: 0,
            decoded_messages: 0,
        }
    }

    pub fn header_size(&self) -> usize {
        self.version.header_size()
    }

    pub fn record_encoded(&mut self) {
        self.encoded_messages += 1;
    }

    pub fn record_decoded(&mut self) {
        self.decoded_messages += 1;
    }

    pub fn total_processed(&self) -> u64 {
        self.encoded_messages + self.decoded_messages
    }
}

impl Default for DiscoveryWireFormat {
    fn default() -> Self {
        Self::new(DiscoveryWireVersion::V2)
    }
}

///////////////////////////////////////////////////////////////////////////////
// DISCOVERY FRAME
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveryFrame {
    pub frame_type: u8,
    pub payload: Vec<u8>,
    pub sequence: u64,
    pub checksum: u32,
}

impl DiscoveryFrame {
    pub fn new(frame_type: u8, payload: Vec<u8>, sequence: u64) -> Self {
        let checksum = payload.iter().fold(0u32, |acc, &b| acc.wrapping_add(b as u32));
        Self {
            frame_type,
            payload,
            sequence,
            checksum,
        }
    }

    pub fn payload_len(&self) -> usize {
        self.payload.len()
    }

    pub fn verify_checksum(&self) -> bool {
        let computed = self.payload.iter().fold(0u32, |acc, &b| acc.wrapping_add(b as u32));
        computed == self.checksum
    }

    pub fn is_heartbeat(&self) -> bool {
        self.frame_type == 0
    }

    pub fn is_announce(&self) -> bool {
        self.frame_type == 1
    }
}

///////////////////////////////////////////////////////////////////////////////
// DISCOVERY ACK
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiscoveryAckState {
    Pending,
    AckReceived,
    NackReceived,
    Timeout,
}

impl DiscoveryAckState {
    pub fn as_str(&self) -> &'static str {
        match self {
            DiscoveryAckState::Pending => "pending",
            DiscoveryAckState::AckReceived => "ack_received",
            DiscoveryAckState::NackReceived => "nack_received",
            DiscoveryAckState::Timeout => "timeout",
        }
    }

    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            DiscoveryAckState::AckReceived
                | DiscoveryAckState::NackReceived
                | DiscoveryAckState::Timeout
        )
    }
}

pub struct DiscoveryAck {
    pub ack_states: std::cell::RefCell<std::collections::HashMap<u64, DiscoveryAckState>>,
    pub ack_count: u64,
    pub nack_count: u64,
    pub timeout_count: u64,
}

impl DiscoveryAck {
    pub fn new() -> Self {
        Self {
            ack_states: std::cell::RefCell::new(std::collections::HashMap::new()),
            ack_count: 0,
            nack_count: 0,
            timeout_count: 0,
        }
    }

    pub fn record_ack(&self, seq: u64) {
        self.ack_states
            .borrow_mut()
            .insert(seq, DiscoveryAckState::AckReceived);
        self.ack_count += 1;
    }

    pub fn record_nack(&self, seq: u64) {
        self.ack_states
            .borrow_mut()
            .insert(seq, DiscoveryAckState::NackReceived);
        self.nack_count += 1;
    }

    pub fn record_timeout(&self, seq: u64) {
        self.ack_states
            .borrow_mut()
            .insert(seq, DiscoveryAckState::Timeout);
        self.timeout_count += 1;
    }

    pub fn get_state(&self, seq: u64) -> Option<DiscoveryAckState> {
        self.ack_states.borrow().get(&seq).copied()
    }

    pub fn is_acked(&self, seq: u64) -> bool {
        matches!(
            self.get_state(seq),
            Some(DiscoveryAckState::AckReceived)
        )
    }

    pub fn pending_count(&self) -> usize {
        self.ack_states
            .borrow()
            .values()
            .filter(|s| matches!(s, DiscoveryAckState::Pending))
            .count()
    }

    pub fn total_tracked(&self) -> usize {
        self.ack_states.borrow().len()
    }
}

impl Default for DiscoveryAck {
    fn default() -> Self {
        Self::new()
    }
}

///////////////////////////////////////////////////////////////////////////////
// DISCOVERY SEQUENCING
///////////////////////////////////////////////////////////////////////////////

pub struct DiscoverySequencing {
    pub next_sequence: std::cell::Cell<u64>,
    pub acknowledged: std::cell::RefCell<std::collections::HashSet<u64>>,
    pub total_sequences: u64,
}

impl DiscoverySequencing {
    pub fn new() -> Self {
        Self {
            next_sequence: std::cell::Cell::new(1),
            acknowledged: std::cell::RefCell::new(std::collections::HashSet::new()),
            total_sequences: 0,
        }
    }

    pub fn next_seq(&self) -> u64 {
        let seq = self.next_sequence.get();
        self.next_sequence.set(seq + 1);
        self.total_sequences += 1;
        seq
    }

    pub fn acknowledge(&self, seq: u64) {
        self.acknowledged.borrow_mut().insert(seq);
    }

    pub fn is_acknowledged(&self, seq: u64) -> bool {
        self.acknowledged.borrow().contains(&seq)
    }

    pub fn total_sequences(&self) -> u64 {
        self.total_sequences
    }

    pub fn acknowledged_count(&self) -> usize {
        self.acknowledged.borrow().len()
    }
}

impl Default for DiscoverySequencing {
    fn default() -> Self {
        Self::new()
    }
}

///////////////////////////////////////////////////////////////////////////////
// DISCOVERY HEARTBEAT
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeartbeatState {
    Alive,
    Suspected,
    ConfirmedDead,
    Revived,
}

impl HeartbeatState {
    pub fn as_str(&self) -> &'static str {
        match self {
            HeartbeatState::Alive => "alive",
            HeartbeatState::Suspected => "suspected",
            HeartbeatState::ConfirmedDead => "confirmed_dead",
            HeartbeatState::Revived => "revived",
        }
    }

    pub fn is_alive(&self) -> bool {
        matches!(self, HeartbeatState::Alive | HeartbeatState::Revived)
    }

    pub fn is_dead(&self) -> bool {
        matches!(self, HeartbeatState::ConfirmedDead)
    }
}

pub struct DiscoveryHeartbeat {
    pub node_states: std::cell::RefCell<std::collections::HashMap<NodeId, HeartbeatState>>,
    pub heartbeat_interval_ms: u64,
    pub missed_heartbeats: std::cell::RefCell<std::collections::HashMap<NodeId, u32>>,
    pub max_missed: u32,
}

impl DiscoveryHeartbeat {
    pub fn new(heartbeat_interval_ms: u64, max_missed: u32) -> Self {
        Self {
            node_states: std::cell::RefCell::new(std::collections::HashMap::new()),
            heartbeat_interval_ms,
            missed_heartbeats: std::cell::RefCell::new(std::collections::HashMap::new()),
            max_missed,
        }
    }

    pub fn set_state(&self, node_id: NodeId, state: HeartbeatState) {
        self.node_states.borrow_mut().insert(node_id, state);
    }

    pub fn get_state(&self, node_id: &NodeId) -> HeartbeatState {
        self.node_states
            .borrow()
            .get(node_id)
            .copied()
            .unwrap_or(HeartbeatState::Alive)
    }

    pub fn record_missed(&self, node_id: NodeId) {
        self.missed_heartbeats
            .borrow_mut()
            .entry(node_id)
            .and_modify(|v| *v += 1)
            .or_insert(1);
    }

    pub fn reset_missed(&self, node_id: &NodeId) {
        self.missed_heartbeats.borrow_mut().remove(node_id);
    }

    pub fn is_suspected(&self, node_id: &NodeId) -> bool {
        self.missed_heartbeats
            .borrow()
            .get(node_id)
            .map(|v| *v >= self.max_missed)
            .unwrap_or(false)
    }

    pub fn interval_ms(&self) -> u64 {
        self.heartbeat_interval_ms
    }

    pub fn tracked_count(&self) -> usize {
        self.node_states.borrow().len()
    }
}

impl Default for DiscoveryHeartbeat {
    fn default() -> Self {
        Self::new(5000, 3)
    }
}

///////////////////////////////////////////////////////////////////////////////
// TESTS
///////////////////////////////////////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use super::*;

    fn test_node_id() -> NodeId {
        NodeId::new([1; 32])
    }

    fn test_endpoint() -> Endpoint {
        Endpoint::new(crate::types::CarrierType::Loopback, vec![1, 2, 3]).unwrap()
    }

    fn test_region_id() -> RegionId {
        RegionId::new([1; 32])
    }

    fn test_hunit_id() -> HUnitId {
        HUnitId::new([1; 32])
    }

    fn test_dataset_id() -> DatasetId {
        DatasetId::new([1; 32])
    }

    fn test_node_info() -> NodeInfo {
        NodeInfo::new(test_node_id(), test_endpoint(), test_region_id())
    }

    fn test_hunit_info() -> HUnitInfo {
        HUnitInfo::new(test_hunit_id(), test_node_id(), test_region_id())
    }

    fn test_region_info() -> RegionInfo {
        RegionInfo::new(test_region_id(), true)
    }

    // === DiscoveryError tests ===
    #[test]
    fn discovery_error_category() {
        assert_eq!(
            DiscoveryError::NodeNotFound {
                node_id: test_node_id(),
            }
            .category(),
            "node_not_found"
        );
        assert_eq!(
            DiscoveryError::HUnitNotFound {
                hunit_id: test_hunit_id(),
            }
            .category(),
            "hunit_not_found"
        );
        assert_eq!(
            DiscoveryError::RegionNotFound {
                region_id: test_region_id(),
            }
            .category(),
            "region_not_found"
        );
        assert_eq!(
            DiscoveryError::EndpointUnreachable {
                endpoint: test_endpoint(),
            }
            .category(),
            "endpoint_unreachable"
        );
        assert_eq!(
            DiscoveryError::DiscoveryTableFull { max: 100 }.category(),
            "table_full"
        );
        assert_eq!(
            DiscoveryError::DuplicateNode {
                node_id: test_node_id(),
            }
            .category(),
            "duplicate_node"
        );
        assert_eq!(
            DiscoveryError::DuplicateHUnit {
                hunit_id: test_hunit_id(),
            }
            .category(),
            "duplicate_hunit"
        );
        assert_eq!(
            DiscoveryError::DuplicateRegion {
                region_id: test_region_id(),
            }
            .category(),
            "duplicate_region"
        );
        assert_eq!(
            DiscoveryError::GossipFailed {
                endpoint: test_endpoint(),
                reason: "timeout".to_string(),
            }
            .category(),
            "gossip_failed"
        );
        assert_eq!(
            DiscoveryError::ResolveFailed {
                node_id: test_node_id(),
                reason: "cache miss".to_string(),
            }
            .category(),
            "resolve_failed"
        );
        assert_eq!(
            DiscoveryError::AnnouncementFailed {
                reason: "no peers".to_string(),
            }
            .category(),
            "announcement_failed"
        );
        assert_eq!(
            DiscoveryError::GossipStateCorrupt {
                reason: "bad magic".to_string(),
            }
            .category(),
            "gossip_corrupt"
        );
        assert_eq!(
            DiscoveryError::PeerTimeout {
                endpoint: test_endpoint(),
            }
            .category(),
            "peer_timeout"
        );
        assert_eq!(
            DiscoveryError::InvalidGossipMessage {
                reason: "truncated".to_string(),
            }
            .category(),
            "invalid_gossip"
        );
        assert_eq!(
            DiscoveryError::NotInitialized.category(),
            "not_initialized"
        );
        assert_eq!(
            DiscoveryError::Internal {
                message: "panic".to_string(),
            }
            .category(),
            "internal"
        );
    }

    #[test]
    fn discovery_error_is_recoverable() {
        assert!(DiscoveryError::EndpointUnreachable {
            endpoint: test_endpoint(),
        }
        .is_recoverable());
        assert!(DiscoveryError::PeerTimeout {
            endpoint: test_endpoint(),
        }
        .is_recoverable());
        assert!(DiscoveryError::GossipFailed {
            endpoint: test_endpoint(),
            reason: "x".to_string(),
        }
        .is_recoverable());
        assert!(!DiscoveryError::NodeNotFound {
            node_id: test_node_id(),
        }
        .is_recoverable());
    }

    #[test]
    fn discovery_error_requires_re_discovery() {
        assert!(DiscoveryError::NodeNotFound {
            node_id: test_node_id(),
        }
        .requires_re_discovery());
        assert!(DiscoveryError::HUnitNotFound {
            hunit_id: test_hunit_id(),
        }
        .requires_re_discovery());
        assert!(DiscoveryError::RegionNotFound {
            region_id: test_region_id(),
        }
        .requires_re_discovery());
        assert!(DiscoveryError::ResolveFailed {
            node_id: test_node_id(),
            reason: "x".to_string(),
        }
        .requires_re_discovery());
        assert!(!DiscoveryError::EndpointUnreachable {
            endpoint: test_endpoint(),
        }
        .requires_re_discovery());
    }

    #[test]
    fn discovery_error_suggested_action() {
        assert_eq!(
            DiscoveryError::NodeNotFound {
                node_id: test_node_id(),
            }
            .suggested_action(),
            "re-gossip discovery state"
        );
        assert_eq!(
            DiscoveryError::DiscoveryTableFull { max: 100 }.suggested_action(),
            "expire stale entries"
        );
        assert_eq!(
            DiscoveryError::NotInitialized.suggested_action(),
            "initialize discovery engine"
        );
    }

    // === NodeInfo tests ===
    #[test]
    fn node_info_new_and_fields() {
        let ni = test_node_info();
        assert_eq!(ni.node_id, test_node_id());
        assert_eq!(ni.endpoint, test_endpoint());
        assert_eq!(ni.region_id, test_region_id());
        assert!(ni.is_healthy);
        assert_eq!(ni.last_seen_ms, 0);
        assert!(ni.capabilities.is_empty());
    }

    #[test]
    fn node_info_with_capabilities() {
        let ni = test_node_info()
            .with_capabilities(vec!["gossip".to_string(), "routing".to_string()]);
        assert!(ni.has_capability("gossip"));
        assert!(ni.has_capability("routing"));
        assert!(!ni.has_capability("storage"));
    }

    #[test]
    fn node_info_with_last_seen() {
        let ni = test_node_info().with_last_seen(5000);
        assert_eq!(ni.last_seen_ms, 5000);
    }

    #[test]
    fn node_info_mark_healthy_unhealthy() {
        let ni = test_node_info().mark_unhealthy();
        assert!(!ni.is_healthy);
        let ni = ni.mark_healthy();
        assert!(ni.is_healthy);
    }

    #[test]
    fn node_info_is_stale() {
        let ni = test_node_info().with_last_seen(1000);
        assert!(!ni.is_stale(2000, 300000));
        assert!(ni.is_stale(400000, 300000));
    }

    // === HUnitInfo tests ===
    #[test]
    fn hunit_info_new_and_fields() {
        let hi = test_hunit_info();
        assert_eq!(hi.hunit_id, test_hunit_id());
        assert_eq!(hi.node_id, test_node_id());
        assert_eq!(hi.region_id, test_region_id());
        assert!(hi.is_active);
        assert_eq!(hi.capacity_total, 0);
        assert_eq!(hi.capacity_used, 0);
    }

    #[test]
    fn hunit_info_with_datasets_and_capacity() {
        let hi = test_hunit_info()
            .with_datasets(vec![test_dataset_id()])
            .with_capacity(1000, 300);
        assert!(hi.hosts_dataset(&test_dataset_id()));
        assert_eq!(hi.available_capacity(), 700);
        assert!((hi.utilization_percent() - 30.0).abs() < 0.01);
    }

    #[test]
    fn hunit_info_mark_active_inactive() {
        let hi = test_hunit_info().mark_inactive();
        assert!(!hi.is_active);
        let hi = hi.mark_active();
        assert!(hi.is_active);
    }

    #[test]
    fn hunit_info_available_capacity_zero() {
        let hi = HUnitInfo::new(test_hunit_id(), test_node_id(), test_region_id());
        assert_eq!(hi.available_capacity(), 0);
        assert_eq!(hi.utilization_percent(), 0.0);
    }

    // === RegionInfo tests ===
    #[test]
    fn region_info_new_and_fields() {
        let ri = test_region_info();
        assert_eq!(ri.region_id, test_region_id());
        assert!(ri.is_primary);
        assert_eq!(ri.node_count(), 0);
        assert_eq!(ri.hunit_count(), 0);
        assert_eq!(ri.latency_ms, 0);
    }

    #[test]
    fn region_info_with_nodes_hunits_latency() {
        let ri = test_region_info()
            .with_nodes(vec![test_node_id()])
            .with_hunits(vec![test_hunit_id()])
            .with_latency(42);
        assert_eq!(ri.node_count(), 1);
        assert_eq!(ri.hunit_count(), 1);
        assert_eq!(ri.latency_ms, 42);
        assert!(ri.has_node(&test_node_id()));
        assert!(!ri.has_node(&NodeId::new([2; 32])));
    }

    // === GossipState tests ===
    #[test]
    fn gossip_state_new_and_add() {
        let gs = GossipState::new(test_node_id())
            .with_sequence(5, 1000)
            .add_node(test_node_info())
            .add_hunit(test_hunit_info())
            .add_region(test_region_info());
        assert_eq!(gs.sender_node_id, test_node_id());
        assert_eq!(gs.sequence_number, 5);
        assert_eq!(gs.timestamp_ms, 1000);
        assert_eq!(gs.node_count(), 1);
        assert_eq!(gs.hunit_count(), 1);
        assert_eq!(gs.region_count(), 1);
    }

    #[test]
    fn gossip_state_is_newer_than() {
        let gs1 = GossipState::new(test_node_id()).with_sequence(5, 1000);
        let gs2 = GossipState::new(test_node_id()).with_sequence(10, 2000);
        assert!(gs2.is_newer_than(&gs1));
        assert!(!gs1.is_newer_than(&gs2));

        let gs3 = GossipState::new(test_node_id()).with_sequence(5, 2000);
        let gs4 = GossipState::new(test_node_id()).with_sequence(5, 1000);
        assert!(gs3.is_newer_than(&gs4));
    }

    // === DiscoveryConfig tests ===
    #[test]
    fn discovery_config_defaults() {
        let config = DiscoveryConfig::default();
        assert_eq!(config.max_nodes, 10000);
        assert_eq!(config.max_hunits, 100000);
        assert_eq!(config.max_regions, 1000);
        assert_eq!(config.gossip_interval_ms, 5000);
        assert_eq!(config.stale_threshold_ms, 300000);
        assert_eq!(config.announcement_ttl_ms, 60000);
        assert_eq!(config.max_gossip_peers, 50);
    }

    #[test]
    fn discovery_config_with_methods() {
        let config = DiscoveryConfig::default()
            .with_max_nodes(5000)
            .with_gossip_interval(1000)
            .with_stale_threshold(60000);
        assert_eq!(config.max_nodes, 5000);
        assert_eq!(config.gossip_interval_ms, 1000);
        assert_eq!(config.stale_threshold_ms, 60000);
    }

    // === DiscoveryEngine tests ===
    #[test]
    fn discovery_engine_register_and_discover_nodes() {
        let engine = DiscoveryEngine::default();
        engine.register_node(test_node_info()).unwrap();
        let nodes = engine.discover_nodes().unwrap();
        assert_eq!(nodes.len(), 1);
        assert_eq!(nodes[0].node_id, test_node_id());
        assert_eq!(engine.node_count(), 1);
    }

    #[test]
    fn discovery_engine_duplicate_node() {
        let engine = DiscoveryEngine::default();
        engine.register_node(test_node_info()).unwrap();
        let err = engine.register_node(test_node_info()).unwrap_err();
        assert!(matches!(err, DiscoveryError::DuplicateNode { .. }));
    }

    #[test]
    fn discovery_engine_deregister_node() {
        let engine = DiscoveryEngine::default();
        engine.register_node(test_node_info()).unwrap();
        assert!(engine.deregister_node(&test_node_id()).is_ok());
        assert!(engine.deregister_node(&test_node_id()).is_err());
        assert_eq!(engine.node_count(), 0);
    }

    #[test]
    fn discovery_engine_resolve_node() {
        let engine = DiscoveryEngine::default();
        engine.register_node(test_node_info()).unwrap();
        let ep = engine.resolve_node(&test_node_id()).unwrap();
        assert_eq!(ep, test_endpoint());
        let err = engine
            .resolve_node(&NodeId::new([2; 32]))
            .unwrap_err();
        assert!(matches!(err, DiscoveryError::NodeNotFound { .. }));
    }

    #[test]
    fn discovery_engine_resolve_endpoint() {
        let engine = DiscoveryEngine::default();
        engine.register_node(test_node_info()).unwrap();
        let ids = engine.resolve_endpoint(&test_endpoint()).unwrap();
        assert_eq!(ids, vec![test_node_id()]);
        let err = engine
            .resolve_endpoint(&Endpoint::new(
                crate::types::CarrierType::Loopback,
                vec![9, 9, 9],
            )
            .unwrap())
            .unwrap_err();
        assert!(matches!(err, DiscoveryError::EndpointUnreachable { .. }));
    }

    #[test]
    fn discovery_engine_register_hunit_and_region() {
        let engine = DiscoveryEngine::default();
        engine.register_hunit(test_hunit_info()).unwrap();
        engine.register_region(test_region_info()).unwrap();
        assert_eq!(engine.hunit_count(), 1);
        assert_eq!(engine.region_count(), 1);
    }

    #[test]
    fn discovery_engine_duplicate_hunit() {
        let engine = DiscoveryEngine::default();
        engine.register_hunit(test_hunit_info()).unwrap();
        let err = engine.register_hunit(test_hunit_info()).unwrap_err();
        assert!(matches!(err, DiscoveryError::DuplicateHUnit { .. }));
    }

    #[test]
    fn discovery_engine_duplicate_region() {
        let engine = DiscoveryEngine::default();
        engine.register_region(test_region_info()).unwrap();
        let err = engine.register_region(test_region_info()).unwrap_err();
        assert!(matches!(err, DiscoveryError::DuplicateRegion { .. }));
    }

    #[test]
    fn discovery_engine_discover_hunits() {
        let engine = DiscoveryEngine::default();
        let hi = test_hunit_info().with_datasets(vec![test_dataset_id()]);
        engine.register_region(test_region_info()).unwrap();
        engine.register_hunit(hi).unwrap();
        let hunits = engine
            .discover_hunits(&test_dataset_id(), &test_region_id())
            .unwrap();
        assert_eq!(hunits.len(), 1);
        assert_eq!(hunits[0].hunit_id, test_hunit_id());
    }

    #[test]
    fn discovery_engine_discover_hunits_region_not_found() {
        let engine = DiscoveryEngine::default();
        let err = engine
            .discover_hunits(&test_dataset_id(), &RegionId::new([2; 32]))
            .unwrap_err();
        assert!(matches!(err, DiscoveryError::RegionNotFound { .. }));
    }

    #[test]
    fn discovery_engine_discover_regions() {
        let engine = DiscoveryEngine::default();
        engine.register_region(test_region_info()).unwrap();
        let regions = engine.discover_regions().unwrap();
        assert_eq!(regions.len(), 1);
        assert_eq!(regions[0].region_id, test_region_id());
    }

    #[test]
    fn discovery_engine_counts() {
        let engine = DiscoveryEngine::default();
        assert_eq!(engine.gossip_count(), 0);
        assert_eq!(engine.announcement_count(), 0);
        assert_eq!(engine.resolve_count(), 0);
    }

    // === GossipDiscovery tests ===
    #[test]
    fn gossip_discovery_add_peer() {
        let gd = GossipDiscovery::default();
        gd.add_peer(test_endpoint());
        assert_eq!(gd.peer_count(), 1);
    }

    #[test]
    fn gossip_discovery_gossip_round() {
        let gd = GossipDiscovery::default();
        assert!(gd.gossip_round().is_ok());
        assert_eq!(gd.gossip_rounds(), 1);
    }

    #[test]
    fn gossip_discovery_node_count() {
        let gd = GossipDiscovery::default();
        assert_eq!(gd.node_count(), 0);
    }

    // === NeighborDiscovery tests ===
    #[test]
    fn neighbor_discovery_add_and_remove() {
        let nd = NeighborDiscovery::default();
        nd.add_neighbor(test_endpoint(), 1000);
        assert_eq!(nd.neighbor_count(), 1);
        assert!(nd.is_neighbor(&test_endpoint()));
        let neighbors = nd.get_neighbors();
        assert_eq!(neighbors.len(), 1);
        assert_eq!(neighbors[0], test_endpoint());
        assert!(nd.remove_neighbor(&test_endpoint()));
        assert!(!nd.is_neighbor(&test_endpoint()));
        assert!(!nd.remove_neighbor(&test_endpoint()));
    }

    #[test]
    fn neighbor_discovery_count() {
        let nd = NeighborDiscovery::default();
        assert_eq!(nd.discovery_count(), 0);
        nd.add_neighbor(test_endpoint(), 1000);
        assert_eq!(nd.discovery_count(), 1);
    }

    // === DiscoveryManager tests ===
    #[test]
    fn discovery_manager_register_and_discover_all() {
        let dm = DiscoveryManager::default();
        dm.register_node(test_node_info()).unwrap();
        let summary = dm.discover_all().unwrap();
        assert_eq!(summary.node_count, 1);
        assert_eq!(summary.hunit_count, 0);
        assert_eq!(summary.region_count, 0);
        assert_eq!(dm.discovery_rounds(), 1);
    }

    #[test]
    fn discovery_manager_add_gossip_peer() {
        let dm = DiscoveryManager::default();
        dm.add_gossip_peer(test_endpoint());
        assert_eq!(dm.gossip.borrow().peer_count(), 1);
    }

    #[test]
    fn discovery_manager_add_neighbor() {
        let dm = DiscoveryManager::default();
        dm.add_neighbor(test_endpoint(), 1000);
        assert_eq!(dm.neighbors.borrow().neighbor_count(), 1);
    }

    #[test]
    fn discovery_manager_total_node_count() {
        let dm = DiscoveryManager::default();
        assert_eq!(dm.total_node_count(), 0);
    }

    // === DiscoveryError extended methods ===
    #[test]
    fn discovery_error_suggested_action_variants() {
        assert_eq!(
            DiscoveryError::GossipFailed {
                endpoint: test_endpoint(),
                reason: "x".to_string(),
            }
            .suggested_action(),
            "retry gossip to peer"
        );
        assert_eq!(
            DiscoveryError::PeerTimeout {
                endpoint: test_endpoint(),
            }
            .suggested_action(),
            "mark peer stale and re-discover"
        );
        assert_eq!(
            DiscoveryError::DuplicateNode {
                node_id: test_node_id(),
            }
            .suggested_action(),
            "update existing entry"
        );
    }


    // === DiscoveryCache tests ===
    #[test]
    fn discovery_cache_put_and_get_node() {
        let cache = DiscoveryCache::default();
        let ep = test_endpoint();
        cache.cache_node(test_node_id(), ep.clone());
        assert_eq!(cache.get_cached_node(&test_node_id()), Some(ep));
    }

    #[test]
    fn discovery_cache_node_miss() {
        let cache = DiscoveryCache::default();
        assert_eq!(cache.get_cached_node(&test_node_id()), None);
        assert_eq!(cache.misses, 1);
    }

    #[test]
    fn discovery_cache_hunits() {
        let cache = DiscoveryCache::default();
        cache.cache_hunits(
            test_dataset_id(),
            test_region_id(),
            vec![test_hunit_id()],
        );
        let result = cache.get_cached_hunits(&test_dataset_id(), &test_region_id());
        assert_eq!(result, Some(vec![test_hunit_id()]));
    }

    #[test]
    fn discovery_cache_region() {
        let cache = DiscoveryCache::default();
        let region = test_region_info();
        cache.cache_region(region.clone());
        assert_eq!(cache.get_cached_region(&test_region_id()), Some(region));
    }

    #[test]
    fn discovery_cache_hit_rate() {
        let cache = DiscoveryCache::default();
        let ep = test_endpoint();
        cache.cache_node(test_node_id(), ep.clone());
        cache.get_cached_node(&test_node_id());
        cache.get_cached_node(&NodeId::new([2; 32]));
        assert_eq!(cache.hit_rate(), 0.5);
    }

    #[test]
    fn discovery_cache_clear() {
        let cache = DiscoveryCache::default();
        let ep = test_endpoint();
        cache.cache_node(test_node_id(), ep.clone());
        cache.clear();
        assert_eq!(cache.entry_count(), 0);
    }

    // === DiscoveryExpiration tests ===
    #[test]
    fn discovery_expiration_set_and_status() {
        let exp = DiscoveryExpiration::default();
        exp.set_ttl(test_node_id(), 5000);
        assert_eq!(
            exp.get_status(&test_node_id(), 1000),
            DiscoveryEntryStatus::Fresh
        );
        assert_eq!(
            exp.get_status(&test_node_id(), 6000),
            DiscoveryEntryStatus::Expired
        );
    }

    #[test]
    fn discovery_expiration_unknown_is_expired() {
        let exp = DiscoveryExpiration::default();
        assert_eq!(
            exp.get_status(&test_node_id(), 1000),
            DiscoveryEntryStatus::Expired
        );
    }

    #[test]
    fn discovery_expiration_mark_expired_and_fresh() {
        let exp = DiscoveryExpiration::default();
        exp.mark_expired(&test_node_id());
        assert_eq!(
            exp.get_status(&test_node_id(), 1000),
            DiscoveryEntryStatus::Expired
        );
        assert_eq!(exp.expired_count(), 1);
        exp.mark_fresh(test_node_id(), 5000);
        assert_eq!(
            exp.get_status(&test_node_id(), 1000),
            DiscoveryEntryStatus::Fresh
        );
    }

    #[test]
    fn discovery_expiration_default_ttl() {
        let exp = DiscoveryExpiration::default();
        assert_eq!(exp.default_ttl_ms(), 300000);
    }

    // === DiscoveryValidator tests ===
    #[test]
    fn discovery_validator_validate_node_info() {
        let mut validator = DiscoveryValidator::new();
        let valid_node = test_node_info();
        assert!(validator.validate_node_info(&valid_node));
        assert_eq!(validator.validations, 1);
    }

    #[test]
    fn discovery_validator_validate_gossip_state() {
        let mut validator = DiscoveryValidator::new();
        let empty_state = GossipState::new(test_node_id());
        assert!(!validator.validate_gossip_state(&empty_state));
        let full_state = GossipState::new(test_node_id()).add_node(test_node_info());
        assert!(validator.validate_gossip_state(&full_state));
    }

    #[test]
    fn discovery_validator_pass_rate() {
        let mut validator = DiscoveryValidator::new();
        validator.record_duplicate();
        validator.record_duplicate();
        assert_eq!(validator.duplicate_nodes, 2);
        assert_eq!(validator.pass_rate(), 1.0);
    }

    // === RegionalDiscovery tests ===
    #[test]
    fn regional_discovery_assign_and_get() {
        let rd = RegionalDiscovery::default();
        rd.assign_node_to_region(test_region_id(), test_node_id());
        let nodes = rd.get_nodes_in_region(&test_region_id());
        assert_eq!(nodes, vec![test_node_id()]);
        assert_eq!(rd.region_count(), 1);
    }

    #[test]
    fn regional_discovery_cross_region_link() {
        let rd = RegionalDiscovery::default();
        rd.add_cross_region_link(test_region_id(), RegionId::new([2; 32]), 42);
        assert_eq!(
            rd.get_cross_region_latency(&test_region_id(), &RegionId::new([2; 32])),
            Some(42)
        );
        assert_eq!(rd.cross_region_link_count(), 1);
    }

    // === CrossRegionDiscovery tests ===
    #[test]
    fn cross_region_discovery_gateway() {
        let crd = CrossRegionDiscovery::default();
        let ep = test_endpoint();
        crd.set_gateway(test_region_id(), ep.clone());
        assert_eq!(crd.get_gateway(&test_region_id()), Some(ep));
        assert_eq!(crd.gateway_count(), 1);
    }

    #[test]
    fn cross_region_discovery_latency() {
        let crd = CrossRegionDiscovery::default();
        crd.set_region_latency(test_region_id(), RegionId::new([2; 32]), 100);
        assert_eq!(
            crd.get_region_latency(&test_region_id(), &RegionId::new([2; 32])),
            Some(100)
        );
    }

    #[test]
    fn cross_region_mode_str() {
        assert_eq!(CrossRegionMode::Direct.as_str(), "direct");
        assert_eq!(CrossRegionMode::ViaGateway.as_str(), "via_gateway");
        assert_eq!(CrossRegionMode::ViaRelay.as_str(), "via_relay");
    }

    // === DiscoveryTopology tests ===
    #[test]
    fn discovery_topology_add_node_and_link() {
        let dt = DiscoveryTopology::default();
        dt.add_node(test_node_id());
        dt.add_link(test_node_id(), NodeId::new([2; 32]), 42);
        assert_eq!(dt.node_count(), 2);
        assert_eq!(dt.link_count(), 1);
        assert_eq!(
            dt.get_link_cost(&test_node_id(), &NodeId::new([2; 32])),
            Some(42)
        );
    }

    #[test]
    fn discovery_topology_unknown_link() {
        let dt = DiscoveryTopology::default();
        assert_eq!(
            dt.get_link_cost(&test_node_id(), &NodeId::new([2; 32])),
            None
        );
    }

    // === GossipProtocol tests ===
    #[test]
    fn gossip_protocol_record_sent_and_received() {
        let mut gp = GossipProtocol::default();
        gp.record_sent(GossipMessageType::NodeAnnounce);
        gp.record_received(GossipMessageType::Heartbeat);
        assert_eq!(gp.messages_sent, 1);
        assert_eq!(gp.messages_received, 1);
        assert_eq!(gp.total_messages(), 2);
        assert_eq!(gp.get_type_count(GossipMessageType::NodeAnnounce), 1);
        assert_eq!(gp.get_type_count(GossipMessageType::Heartbeat), 1);
    }

    #[test]
    fn gossip_message_type_str() {
        assert_eq!(GossipMessageType::NodeAnnounce.as_str(), "node_announce");
        assert_eq!(
            GossipMessageType::HUnitAnnounce.as_str(),
            "hunit_announce"
        );
        assert_eq!(
            GossipMessageType::RegionAnnounce.as_str(),
            "region_announce"
        );
        assert_eq!(GossipMessageType::StateSync.as_str(), "state_sync");
        assert_eq!(GossipMessageType::Heartbeat.as_str(), "heartbeat");
    }

    // === DiscoveryStatistics tests ===
    #[test]
    fn discovery_statistics_record_and_rates() {
        let mut stats = DiscoveryStatistics::default();
        stats.record_discovery();
        stats.record_resolution(10, true);
        stats.record_resolution(20, false);
        stats.record_gossip();
        stats.record_announcement();
        assert_eq!(stats.total_discoveries, 1);
        assert_eq!(stats.total_resolutions, 2);
        assert_eq!(stats.total_failures, 1);
        assert_eq!(stats.total_gossips, 1);
        assert_eq!(stats.total_announcements, 1);
        assert!((stats.success_rate() - 0.5).abs() < 0.01);
        assert!((stats.failure_rate() - 0.5).abs() < 0.01);
    }

    #[test]
    fn discovery_statistics_default() {
        let stats = DiscoveryStatistics::default();
        assert_eq!(stats.total_resolutions, 0);
        assert_eq!(stats.failure_rate(), 0.0);
        assert_eq!(stats.success_rate(), 1.0);
    }

    // === NodeHealthTracker tests ===
    #[test]
    fn node_health_tracker_set_and_get() {
        let nht = NodeHealthTracker::default();
        nht.set_health(test_node_id(), NodeHealthStatus::Healthy);
        assert_eq!(
            nht.get_health(&test_node_id()),
            NodeHealthStatus::Healthy
        );
        assert_eq!(nht.healthy_count(), 1);
        assert_eq!(nht.total_checks(), 1);
    }

    #[test]
    fn node_health_tracker_unknown_default() {
        let nht = NodeHealthTracker::default();
        assert_eq!(
            nht.get_health(&test_node_id()),
            NodeHealthStatus::Unknown
        );
    }

    #[test]
    fn node_health_status_str() {
        assert_eq!(NodeHealthStatus::Healthy.as_str(), "healthy");
        assert_eq!(NodeHealthStatus::Degraded.as_str(), "degraded");
        assert_eq!(NodeHealthStatus::Unhealthy.as_str(), "unhealthy");
        assert_eq!(NodeHealthStatus::Unknown.as_str(), "unknown");
        assert!(NodeHealthStatus::Healthy.is_healthy());
        assert!(!NodeHealthStatus::ConfirmedDead.is_alive());
    }

    // === RegionDirectory tests ===
    #[test]
    fn region_directory_register_and_get() {
        let rd = RegionDirectory::default();
        let region = test_region_info();
        rd.register_region(region.clone());
        assert_eq!(rd.get_region(&test_region_id()), Some(region));
        assert_eq!(rd.region_count(), 1);
    }

    #[test]
    fn region_directory_add_node_and_get() {
        let rd = RegionDirectory::default();
        rd.add_node_to_region(test_region_id(), test_node_id());
        let nodes = rd.get_nodes_in_region(&test_region_id());
        assert_eq!(nodes, vec![test_node_id()]);
    }

    #[test]
    fn region_directory_remove_region() {
        let rd = RegionDirectory::default();
        rd.register_region(test_region_info());
        rd.add_node_to_region(test_region_id(), test_node_id());
        assert!(rd.remove_region(&test_region_id()));
        assert!(rd.get_region(&test_region_id()).is_none());
    }

    // === DiscoveryIntegration tests ===
    #[test]
    fn discovery_integration_bind_subsystem() {
        let di = DiscoveryIntegration::default();
        di.bind_subsystem("routing".to_string());
        di.bind_subsystem("store".to_string());
        assert!(di.is_bound("routing"));
        assert!(!di.is_bound("security"));
        assert_eq!(di.bound_count(), 2);
        assert_eq!(di.event_count(), 2);
    }

    #[test]
    fn discovery_integration_clear_bindings() {
        let di = DiscoveryIntegration::default();
        di.bind_subsystem("routing".to_string());
        di.clear_bindings();
        assert_eq!(di.bound_count(), 0);
    }

    // === DiscoveryWireFormat tests ===
    #[test]
    fn discovery_wire_format_version_and_header() {
        let wf = DiscoveryWireFormat::new(DiscoveryWireVersion::V1);
        assert_eq!(wf.header_size(), 8);
        assert_eq!(wf.version.as_str(), "discovery_wire_v1");
        let wf2 = DiscoveryWireFormat::new(DiscoveryWireVersion::V2);
        assert_eq!(wf2.header_size(), 16);
        assert_eq!(wf2.version.as_str(), "discovery_wire_v2");
    }

    #[test]
    fn discovery_wire_format_encoded_decoded() {
        let mut wf = DiscoveryWireFormat::default();
        wf.record_encoded();
        wf.record_decoded();
        wf.record_encoded();
        assert_eq!(wf.total_processed(), 3);
    }

    // === DiscoveryFrame tests ===
    #[test]
    fn discovery_frame_new_and_verify() {
        let frame = DiscoveryFrame::new(1, vec![1, 2, 3], 42);
        assert_eq!(frame.frame_type, 1);
        assert_eq!(frame.payload, vec![1, 2, 3]);
        assert_eq!(frame.sequence, 42);
        assert!(frame.verify_checksum());
        assert!(!frame.is_heartbeat());
        assert!(frame.is_announce());
    }

    #[test]
    fn discovery_frame_heartbeat() {
        let frame = DiscoveryFrame::new(0, vec![], 0);
        assert!(frame.is_heartbeat());
        assert!(!frame.is_announce());
        assert!(frame.verify_checksum());
    }

    // === DiscoveryAck tests ===
    #[test]
    fn discovery_ack_record_and_state() {
        let ack = DiscoveryAck::default();
        ack.record_ack(1);
        ack.record_nack(2);
        ack.record_timeout(3);
        assert!(ack.is_acked(1));
        assert_eq!(
            ack.get_state(2),
            Some(DiscoveryAckState::NackReceived)
        );
        assert_eq!(
            ack.get_state(3),
            Some(DiscoveryAckState::Timeout)
        );
        assert_eq!(ack.ack_count, 1);
        assert_eq!(ack.nack_count, 1);
        assert_eq!(ack.timeout_count, 1);
        assert_eq!(ack.total_tracked(), 3);
    }

    #[test]
    fn discovery_ack_state_str_and_terminal() {
        assert_eq!(DiscoveryAckState::Pending.as_str(), "pending");
        assert_eq!(DiscoveryAckState::AckReceived.as_str(), "ack_received");
        assert_eq!(DiscoveryAckState::NackReceived.as_str(), "nack_received");
        assert_eq!(DiscoveryAckState::Timeout.as_str(), "timeout");
        assert!(!DiscoveryAckState::Pending.is_terminal());
        assert!(DiscoveryAckState::AckReceived.is_terminal());
        assert!(DiscoveryAckState::Timeout.is_terminal());
    }

    // === DiscoverySequencing tests ===
    #[test]
    fn discovery_sequencing_next_and_ack() {
        let seq = DiscoverySequencing::default();
        let s1 = seq.next_seq();
        let s2 = seq.next_seq();
        assert_eq!(s1, 1);
        assert_eq!(s2, 2);
        assert!(!seq.is_acknowledged(s1));
        seq.acknowledge(s1);
        assert!(seq.is_acknowledged(s1));
        assert_eq!(seq.total_sequences(), 2);
        assert_eq!(seq.acknowledged_count(), 1);
    }

    // === DiscoveryHeartbeat tests ===
    #[test]
    fn discovery_heartbeat_set_and_get_state() {
        let hb = DiscoveryHeartbeat::default();
        hb.set_state(test_node_id(), HeartbeatState::Suspected);
        assert_eq!(
            hb.get_state(&test_node_id()),
            HeartbeatState::Suspected
        );
        assert!(!hb.is_suspected(&test_node_id()));
    }

    #[test]
    fn discovery_heartbeat_record_missed() {
        let hb = DiscoveryHeartbeat::new(5000, 2);
        hb.record_missed(test_node_id());
        assert!(!hb.is_suspected(&test_node_id()));
        hb.record_missed(test_node_id());
        assert!(hb.is_suspected(&test_node_id()));
    }

    #[test]
    fn discovery_heartbeat_reset_missed() {
        let hb = DiscoveryHeartbeat::default();
        hb.record_missed(test_node_id());
        hb.reset_missed(&test_node_id());
        assert!(!hb.is_suspected(&test_node_id()));
    }

    #[test]
    fn discovery_heartbeat_state_methods() {
        assert!(HeartbeatState::Alive.is_alive());
        assert!(HeartbeatState::Revived.is_alive());
        assert!(!HeartbeatState::Suspected.is_alive());
        assert!(HeartbeatState::ConfirmedDead.is_dead());
        assert!(!HeartbeatState::Alive.is_dead());
        assert_eq!(HeartbeatState::Alive.as_str(), "alive");
        assert_eq!(HeartbeatState::ConfirmedDead.as_str(), "confirmed_dead");
    }

    #[test]
    fn discovery_heartbeat_default_interval() {
        let hb = DiscoveryHeartbeat::default();
        assert_eq!(hb.interval_ms(), 5000);
        assert_eq!(hb.max_missed, 3);
    }
}
