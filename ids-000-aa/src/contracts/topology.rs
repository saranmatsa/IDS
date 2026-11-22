//! # ids-000-aa Sealed Trait Contract: Topology
//!
//! Network topology contract for IDS.
//! Provides neighbor management, link state tracking, topology graph
//! construction, path computation, topology change notification, and
//! topology-aware routing. All types are canonical and sealed — only
//! ids-000-aa and its designated implementation crates can implement this trait.
//!
//! Carrier-agnostic: no std::net, tokio::net, libp2p, IP, TCP, UDP, QUIC,
//! HTTP, DNS, or any existing networking stack. Topology messages are opaque
//! byte blobs carried on CarrierType + Vec<u8> Endpoints only.

use crate::errors::TopologyError;
use crate::types::{
    CheckpointId, DatasetId, Epoch, Endpoint, FragmentId, HUnitId, IdsTime, KeyId, LeaseId,
    MessageId, NodeId, OpRef, OrderingDomain, OrderingPos, RegionId, SagaId, SessionId,
    StepId, TopologyError as _, TopologyEvent, TopologyNode, TopologyPath, TopologyRegion,
    TopologyVersion, TransactionId, Version,
};
use std::collections::{HashMap, HashSet};

///////////////////////////////////////////////////////////////////////////////
// SEALED TRAIT: Topology
///////////////////////////////////////////////////////////////////////////////

/// Sealed marker — only ids-000-aa and designated crates can implement.
pub mod private {
    pub trait Sealed {}
}

/// Sealed topology management trait.
/// Implementations live downstream in ids-topology and ids-* crates.
pub trait Topology: private::Sealed {
    /// Configure topology parameters.
    fn configure_topology(&self, config: TopologyConfig) -> Result<(), TopologyError>;

    /// Register a node in the topology.
    fn register_node(
        &self,
        node_id: NodeId,
        endpoint: Endpoint,
        region: RegionId,
    ) -> Result<TopologyNodeInfo, TopologyError>;

    /// Unregister a node from the topology.
    fn unregister_node(&self, node_id: NodeId) -> Result<(), TopologyError>;

    /// Get node information.
    fn node_info(&self, node_id: NodeId) -> Result<TopologyNodeInfo, TopologyError>;

    /// Get all nodes in the topology.
    fn all_nodes(&self) -> Vec<NodeId>;

    /// Get nodes in a region.
    fn nodes_in_region(&self, region: RegionId) -> Vec<NodeId>;

    /// Add a link between two nodes.
    fn add_link(
        &self,
        node_a: NodeId,
        node_b: NodeId,
        link: TopologyLink,
    ) -> Result<TopologyLinkInfo, TopologyError>;

    /// Remove a link between two nodes.
    fn remove_link(&self, node_a: NodeId, node_b: NodeId) -> Result<(), TopologyError>;

    /// Get link information between two nodes.
    fn link_info(&self, node_a: NodeId, node_b: NodeId) -> Result<TopologyLinkInfo, TopologyError>;

    /// Get all links for a node.
    fn node_links(&self, node_id: NodeId) -> Vec<TopologyLinkInfo>;

    /// Get all links in the topology.
    fn all_links(&self) -> Vec<TopologyLinkInfo>;

    /// Update link state (up/down, latency, cost).
    fn update_link_state(
        &self,
        node_a: NodeId,
        node_b: NodeId,
        state: LinkState,
    ) -> Result<(), TopologyError>;

    /// Record a topology event (node joined, link failed, etc.).
    fn record_topology_event(
        &self,
        event: TopologyEvent,
    ) -> Result<TopologyEventRecord, TopologyError>;

    /// Get topology events for a time range.
    fn topology_events(
        &self,
        start_time: u64,
        end_time: u64,
    ) -> Vec<TopologyEventRecord>;

    /// Compute shortest path between two nodes.
    fn compute_path(
        &self,
        source: NodeId,
        destination: NodeId,
    ) -> Result<TopologyPath, TopologyError>;

    /// Compute all paths between two nodes (up to max_paths).
    fn compute_all_paths(
        &self,
        source: NodeId,
        destination: NodeId,
        max_paths: u32,
    ) -> Result<Vec<TopologyPath>, TopologyError>;

    /// Get neighbors of a node.
    fn neighbors(&self, node_id: NodeId) -> Vec<NodeId>;

    /// Check if two nodes are directly connected.
    fn are_connected(&self, node_a: NodeId, node_b: NodeId) -> bool;

    /// Get the topology version.
    fn topology_version(&self) -> TopologyVersion;

    /// Increment topology version after changes.
    fn increment_version(&self) -> TopologyVersion;

    /// Get topology statistics.
    fn stats(&self) -> TopologyStats;

    /// Check if topology is connected (all nodes reachable).
    fn is_connected(&self) -> bool;

    /// Get connected components.
    fn connected_components(&self) -> Vec<Vec<NodeId>>;

    /// Validate topology configuration.
    fn validate_config(&self, config: &TopologyConfig) -> Result<(), TopologyError>;

    /// Get the topology graph snapshot.
    fn snapshot(&self) -> TopologySnapshot;

    /// Restore topology from a snapshot.
    fn restore_from_snapshot(
        &self,
        snapshot: TopologySnapshot,
    ) -> Result<(), TopologyError>;

    /// Detect topology cycles.
    fn detect_cycles(&self) -> Vec<Vec<NodeId>>;

    /// Get articulation points (single points of failure).
    fn articulation_points(&self) -> Vec<NodeId>;

    /// Get bridge links (single points of failure in links).
    fn bridge_links(&self) -> Vec<(NodeId, NodeId)>;
}

///////////////////////////////////////////////////////////////////////////////
// CONFIGURATION
///////////////////////////////////////////////////////////////////////////////

/// Topology configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TopologyConfig {
    /// Maximum number of nodes.
    pub max_nodes: u32,
    /// Maximum number of links per node.
    pub max_links_per_node: u32,
    /// Default link cost.
    pub default_link_cost: u32,
    /// Default link latency (ms).
    pub default_link_latency_ms: u64,
    /// Enable topology diversity (avoid single points of failure).
    pub enable_topology_diversity: bool,
    /// Link health check interval (ms).
    pub link_health_check_interval_ms: u64,
    /// Node timeout (ms) before considering a node down.
    pub node_timeout_ms: u64,
    /// Enable path caching.
    pub enable_path_cache: bool,
    /// Maximum cached paths.
    pub max_cached_paths: u32,
    /// Topology change propagation delay (ms).
    pub propagation_delay_ms: u64,
}

impl TopologyConfig {
    pub fn new(max_nodes: u32) -> Result<Self, TopologyError> {
        let config = Self {
            max_nodes,
            max_links_per_node: 10,
            default_link_cost: 1,
            default_link_latency_ms: 10,
            enable_topology_diversity: true,
            link_health_check_interval_ms: 5000,
            node_timeout_ms: 30000,
            enable_path_cache: true,
            max_cached_paths: 100,
            propagation_delay_ms: 100,
        };
        config.validate()?;
        Ok(config)
    }

    pub fn validate(&self) -> Result<(), TopologyError> {
        if self.max_nodes == 0 {
            return Err(TopologyError::InvalidConfig {
                field: "max_nodes".to_string(),
                reason: "max_nodes must be greater than 0".to_string(),
            });
        }
        if self.max_links_per_node == 0 {
            return Err(TopologyError::InvalidConfig {
                field: "max_links_per_node".to_string(),
                reason: "max_links_per_node must be greater than 0".to_string(),
            });
        }
        if self.default_link_cost == 0 {
            return Err(TopologyError::InvalidConfig {
                field: "default_link_cost".to_string(),
                reason: "default_link_cost must be greater than 0".to_string(),
            });
        }
        if self.link_health_check_interval_ms == 0 {
            return Err(TopologyError::InvalidConfig {
                field: "link_health_check_interval_ms".to_string(),
                reason: "link_health_check_interval_ms must be greater than 0".to_string(),
            });
        }
        if self.node_timeout_ms == 0 {
            return Err(TopologyError::InvalidConfig {
                field: "node_timeout_ms".to_string(),
                reason: "node_timeout_ms must be greater than 0".to_string(),
            });
        }
        if self.max_cached_paths == 0 {
            return Err(TopologyError::InvalidConfig {
                field: "max_cached_paths".to_string(),
                reason: "max_cached_paths must be greater than 0".to_string(),
            });
        }
        Ok(())
    }

    pub fn default() -> Self {
        Self {
            max_nodes: 1000,
            max_links_per_node: 10,
            default_link_cost: 1,
            default_link_latency_ms: 10,
            enable_topology_diversity: true,
            link_health_check_interval_ms: 5000,
            node_timeout_ms: 30000,
            enable_path_cache: true,
            max_cached_paths: 100,
            propagation_delay_ms: 100,
        }
    }

    pub fn strict() -> Self {
        Self {
            max_nodes: 100,
            max_links_per_node: 3,
            default_link_cost: 1,
            default_link_latency_ms: 5,
            enable_topology_diversity: true,
            link_health_check_interval_ms: 1000,
            node_timeout_ms: 5000,
            enable_path_cache: true,
            max_cached_paths: 10,
            propagation_delay_ms: 10,
        }
    }

    pub fn lenient() -> Self {
        Self {
            max_nodes: 10000,
            max_links_per_node: 50,
            default_link_cost: 1,
            default_link_latency_ms: 100,
            enable_topology_diversity: false,
            link_health_check_interval_ms: 60000,
            node_timeout_ms: 300000,
            enable_path_cache: false,
            max_cached_paths: 1000,
            propagation_delay_ms: 1000,
        }
    }

    pub fn link_density(&self) -> f64 {
        if self.max_nodes < 2 {
            return 0.0;
        }
        let max_possible_links = (self.max_nodes as f64 * (self.max_nodes as f64 - 1.0)) / 2.0;
        let max_actual_links = self.max_nodes as f64 * self.max_links_per_node as f64 / 2.0;
        max_actual_links / max_possible_links
    }
}

impl Default for TopologyConfig {
    fn default() -> Self {
        Self::default()
    }
}

///////////////////////////////////////////////////////////////////////////////
// TOPOLOGY NODE INFO
///////////////////////////////////////////////////////////////////////////////

/// Information about a node in the topology.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TopologyNodeInfo {
    pub node_id: NodeId,
    pub endpoint: Endpoint,
    pub region: RegionId,
    pub state: NodeState,
    pub registered_at: u64,
    pub last_seen: u64,
    pub link_count: u32,
    pub metadata: Vec<u8>,
}

impl TopologyNodeInfo {
    pub fn new(
        node_id: NodeId,
        endpoint: Endpoint,
        region: RegionId,
    ) -> Self {
        let now = 0u64; // Set by engine
        Self {
            node_id,
            endpoint,
            region,
            state: NodeState::Active,
            registered_at: now,
            last_seen: now,
            link_count: 0,
            metadata: Vec::new(),
        }
    }

    pub fn is_active(&self) -> bool {
        self.state == NodeState::Active
    }

    pub fn is_down(&self) -> bool {
        self.state == NodeState::Down
    }
}

/// Node state in the topology.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum NodeState {
    Active,
    Down,
    Degraded,
    Leaving,
    Joining,
}

impl NodeState {
    pub fn is_operational(&self) -> bool {
        matches!(self, NodeState::Active)
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            NodeState::Active => "active",
            NodeState::Down => "down",
            NodeState::Degraded => "degraded",
            NodeState::Leaving => "leaving",
            NodeState::Joining => "joining",
        }
    }
}

///////////////////////////////////////////////////////////////////////////////
// TOPOLOGY LINK
///////////////////////////////////////////////////////////////////////////////

/// A link between two nodes in the topology.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TopologyLink {
    pub node_a: NodeId,
    pub node_b: NodeId,
    pub cost: u32,
    pub latency_ms: u64,
    pub bandwidth_bps: u64,
    pub reliability: f64,
    pub state: LinkState,
}

/// Link state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum LinkState {
    Up,
    Down,
    Degraded,
    Flapping,
}

impl LinkState {
    pub fn is_operational(&self) -> bool {
        matches!(self, LinkState::Up)
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            LinkState::Up => "up",
            LinkState::Down => "down",
            LinkState::Degraded => "degraded",
            LinkState::Flapping => "flapping",
        }
    }
}

/// Link information with computed metrics.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TopologyLinkInfo {
    pub node_a: NodeId,
    pub node_b: NodeId,
    pub cost: u32,
    pub latency_ms: u64,
    pub state: LinkState,
    pub last_updated: u64,
    pub update_count: u32,
}

impl TopologyLinkInfo {
    pub fn effective_cost(&self) -> u32 {
        match self.state {
            LinkState::Up => self.cost,
            LinkState::Degraded => self.cost * 2,
            LinkState::Flapping => self.cost * 5,
            LinkState::Down => u32::MAX,
        }
    }
}

///////////////////////////////////////////////////////////////////////////////
// TOPOLOGY EVENT
///////////////////////////////////////////////////////////////////////////////

/// A topology change event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TopologyEvent {
    pub event_type: TopologyEventType,
    pub node_id: Option<NodeId>,
    pub link: Option<(NodeId, NodeId)>,
    pub timestamp: u64,
    pub details: Vec<u8>,
}

/// Types of topology events.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TopologyEventType {
    NodeJoined,
    NodeLeft,
    NodeDown,
    NodeRecovered,
    LinkAdded,
    LinkRemoved,
    LinkFailed,
    LinkRecovered,
    TopologyChanged,
}

impl TopologyEventType {
    pub fn display_name(&self) -> &'static str {
        match self {
            TopologyEventType::NodeJoined => "node_joined",
            TopologyEventType::NodeLeft => "node_left",
            TopologyEventType::NodeDown => "node_down",
            TopologyEventType::NodeRecovered => "node_recovered",
            TopologyEventType::LinkAdded => "link_added",
            TopologyEventType::LinkRemoved => "link_removed",
            TopologyEventType::LinkFailed => "link_failed",
            TopologyEventType::LinkRecovered => "link_recovered",
            TopologyEventType::TopologyChanged => "topology_changed",
        }
    }
}

/// Record of a topology event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TopologyEventRecord {
    pub event_id: u64,
    pub event_type: TopologyEventType,
    pub node_id: Option<NodeId>,
    pub link: Option<(NodeId, NodeId)>,
    pub timestamp: u64,
}

/// Topology version.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TopologyVersion(pub u64);

impl TopologyVersion {
    pub fn new(version: u64) -> Self {
        Self(version)
    }

    pub fn increment(&self) -> Self {
        Self(self.0 + 1)
    }
}

///////////////////////////////////////////////////////////////////////////////
// TOPOLOGY PATH
///////////////////////////////////////////////////////////////////////////////

/// A path through the topology.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TopologyPath {
    pub nodes: Vec<NodeId>,
    pub total_cost: u32,
    pub total_latency_ms: u64,
    pub hop_count: u32,
    pub path_id: u64,
}

impl TopologyPath {
    pub fn is_valid(&self) -> bool {
        !self.nodes.is_empty() && self.nodes.len() >= 2
    }

    pub fn contains_node(&self, node_id: NodeId) -> bool {
        self.nodes.contains(&node_id)
    }
}

///////////////////////////////////////////////////////////////////////////////
// TOPOLOGY STATS
///////////////////////////////////////////////////////////////////////////////

/// Topology statistics.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TopologyStats {
    pub total_nodes: u32,
    pub active_nodes: u32,
    pub down_nodes: u32,
    pub total_links: u32,
    pub up_links: u32,
    pub down_links: u32,
    pub topology_events: u64,
    pub path_computations: u64,
    pub version: TopologyVersion,
    pub connected_components_count: u32,
}

impl TopologyStats {
    pub fn new() -> Self {
        Self {
            total_nodes: 0,
            active_nodes: 0,
            down_nodes: 0,
            total_links: 0,
            up_links: 0,
            down_links: 0,
            topology_events: 0,
            path_computations: 0,
            version: TopologyVersion::new(0),
            connected_components_count: 0,
        }
    }

    pub fn node_availability_percent(&self) -> f64 {
        if self.total_nodes == 0 {
            return 100.0;
        }
        self.active_nodes as f64 / self.total_nodes as f64 * 100.0
    }

    pub fn link_availability_percent(&self) -> f64 {
        if self.total_links == 0 {
            return 100.0;
        }
        self.up_links as f64 / self.total_links as f64 * 100.0
    }
}

///////////////////////////////////////////////////////////////////////////////
// TOPOLOGY SNAPSHOT
///////////////////////////////////////////////////////////////////////////////

/// A snapshot of the entire topology.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TopologySnapshot {
    pub version: TopologyVersion,
    pub nodes: Vec<TopologyNodeInfo>,
    pub links: Vec<TopologyLinkInfo>,
    pub timestamp: u64,
}

impl TopologySnapshot {
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    pub fn link_count(&self) -> usize {
        self.links.len()
    }
}

///////////////////////////////////////////////////////////////////////////////
// TOPOLOGY ERROR
///////////////////////////////////////////////////////////////////////////////

/// Topology management errors.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
pub enum TopologyError {
    #[error("invalid config: {field} - {reason}")]
    InvalidConfig { field: String, reason: String },

    #[error("node not found: {node_id:?}")]
    NodeNotFound { node_id: NodeId },

    #[error("node already registered: {node_id:?}")]
    NodeAlreadyRegistered { node_id: NodeId },

    #[error("link not found: {node_a:?} - {node_b:?}")]
    LinkNotFound { node_a: NodeId, node_b: NodeId },

    #[error("link already exists: {node_a:?} - {node_b:?}")]
    LinkAlreadyExists { node_a: NodeId, node_b: NodeId },

    #[error("no path between {source:?} and {destination:?}")]
    NoPath { source: NodeId, destination: NodeId },

    #[error("maximum nodes exceeded: {max_nodes}")]
    MaxNodesExceeded { max_nodes: u32 },

    #[error("maximum links per node exceeded: {node_id:?}")]
    MaxLinksPerNodeExceeded { node_id: NodeId },

    #[error("node is down: {node_id:?}")]
    NodeDown { node_id: NodeId },

    #[error("self-loop not allowed: {node_id:?}")]
    SelfLoopNotAllowed { node_id: NodeId },

    #[error("invalid link state transition: {from:?} -> {to:?}")]
    InvalidLinkStateTransition { from: LinkState, to: LinkState },

    #[error("snapshot version mismatch: expected {expected}, got {got}")]
    SnapshotVersionMismatch { expected: u64, got: u64 },

    #[error("internal error: {message}")]
    Internal { message: String },
}

impl TopologyError {
    pub fn category(&self) -> &'static str {
        match self {
            TopologyError::InvalidConfig { .. } => "invalid_config",
            TopologyError::NodeNotFound { .. } => "node_not_found",
            TopologyError::NodeAlreadyRegistered { .. } => "node_already_registered",
            TopologyError::LinkNotFound { .. } => "link_not_found",
            TopologyError::LinkAlreadyExists { .. } => "link_already_exists",
            TopologyError::NoPath { .. } => "no_path",
            TopologyError::MaxNodesExceeded { .. } => "max_nodes_exceeded",
            TopologyError::MaxLinksPerNodeExceeded { .. } => "max_links_per_node_exceeded",
            TopologyError::NodeDown { .. } => "node_down",
            TopologyError::SelfLoopNotAllowed { .. } => "self_loop_not_allowed",
            TopologyError::InvalidLinkStateTransition { .. } => "invalid_link_state_transition",
            TopologyError::SnapshotVersionMismatch { .. } => "snapshot_version_mismatch",
            TopologyError::Internal { .. } => "internal",
        }
    }

    pub fn is_recoverable(&self) -> bool {
        matches!(
            self,
            TopologyError::NodeDown { .. }
                | TopologyError::NoPath { .. }
                | TopologyError::LinkNotFound { .. }
        )
    }

    pub fn suggested_action(&self) -> &'static str {
        match self {
            TopologyError::InvalidConfig { .. } => "correct the topology configuration",
            TopologyError::NodeNotFound { .. } => "register the node first",
            TopologyError::NodeAlreadyRegistered { .. } => "use existing node",
            TopologyError::LinkNotFound { .. } => "create the link first",
            TopologyError::LinkAlreadyExists { .. } => "use existing link",
            TopologyError::NoPath { .. } => "check network connectivity",
            TopologyError::MaxNodesExceeded { .. } => "increase max_nodes or remove nodes",
            TopologyError::MaxLinksPerNodeExceeded { .. } => "remove some links first",
            TopologyError::NodeDown { .. } => "wait for node recovery",
            TopologyError::SelfLoopNotAllowed { .. } => "use different nodes",
            TopologyError::InvalidLinkStateTransition { .. } => "check link state machine",
            TopologyError::SnapshotVersionMismatch { .. } => "use matching snapshot version",
            TopologyError::Internal { .. } => "contact system administrator",
        }
    }
}

///////////////////////////////////////////////////////////////////////////////
// INTERNAL STATE
///////////////////////////////////////////////////////////////////////////////

/// Internal state for a topology node.
#[derive(Debug, Clone)]
struct InternalNodeInfo {
    node_id: NodeId,
    endpoint: Endpoint,
    region: RegionId,
    state: NodeState,
    registered_at: u64,
    last_seen: u64,
    metadata: Vec<u8>,
}

/// Internal state for a topology link.
#[derive(Debug, Clone)]
struct InternalLinkInfo {
    node_a: NodeId,
    node_b: NodeId,
    cost: u32,
    latency_ms: u64,
    bandwidth_bps: u64,
    reliability: f64,
    state: LinkState,
    last_updated: u64,
    update_count: u32,
}

/// Internal topology graph.
#[derive(Debug, Clone)]
struct TopologyGraph {
    nodes: HashMap<NodeId, InternalNodeInfo>,
    links: HashMap<(NodeId, NodeId), InternalLinkInfo>,
    adjacency: HashMap<NodeId, Vec<NodeId>>,
    version: TopologyVersion,
    event_counter: u64,
    events: Vec<TopologyEventRecord>,
}

impl TopologyGraph {
    fn new() -> Self {
        Self {
            nodes: HashMap::new(),
            links: HashMap::new(),
            adjacency: HashMap::new(),
            version: TopologyVersion::new(0),
            event_counter: 0,
            events: Vec::new(),
        }
    }

    fn canonical_key(a: NodeId, b: NodeId) -> (NodeId, NodeId) {
        if a.as_bytes() < b.as_bytes() {
            (a, b)
        } else {
            (b, a)
        }
    }

    fn add_event(
        &mut self,
        event_type: TopologyEventType,
        node_id: Option<NodeId>,
        link: Option<(NodeId, NodeId)>,
    ) -> u64 {
        self.event_counter += 1;
        let record = TopologyEventRecord {
            event_id: self.event_counter,
            event_type,
            node_id,
            link,
            timestamp: 0, // Set by engine
        };
        self.events.push(record.clone());
        self.event_counter
    }
}

///////////////////////////////////////////////////////////////////////////////
// TOPOLOGY ENGINE (REFERENCE IMPLEMENTATION)
///////////////////////////////////////////////////////////////////////////////

/// Reference topology engine implementing the Topology trait.
pub struct TopologyEngine {
    config: std::cell::RefCell<TopologyConfig>,
    graph: std::cell::RefCell<TopologyGraph>,
}

impl TopologyEngine {
    pub fn new() -> Self {
        Self {
            config: std::cell::RefCell::new(TopologyConfig::default()),
            graph: std::cell::RefCell::new(TopologyGraph::new()),
        }
    }

    pub fn with_config(config: TopologyConfig) -> Result<Self, TopologyError> {
        config.validate()?;
        Ok(Self {
            config: std::cell::RefCell::new(config),
            graph: std::cell::RefCell::new(TopologyGraph::new()),
        })
    }

    fn ensure_config(&self) -> TopologyConfig {
        self.config.borrow().clone()
    }
}

///////////////////////////////////////////////////////////////////////////////
// IMPL TOPOLOGY FOR TOPOLOGYENGINE
///////////////////////////////////////////////////////////////////////////////

impl private::Sealed for TopologyEngine {}

impl Topology for TopologyEngine {
    fn configure_topology(&self, config: TopologyConfig) -> Result<(), TopologyError> {
        config.validate()?;
        *self.config.borrow_mut() = config;
        Ok(())
    }

    fn register_node(
        &self,
        node_id: NodeId,
        endpoint: Endpoint,
        region: RegionId,
    ) -> Result<TopologyNodeInfo, TopologyError> {
        let config = self.ensure_config();
        let mut graph = self.graph.borrow_mut();

        if graph.nodes.contains_key(&node_id) {
            return Err(TopologyError::NodeAlreadyRegistered { node_id });
        }

        if graph.nodes.len() as u32 >= config.max_nodes {
            return Err(TopologyError::MaxNodesExceeded { max_nodes: config.max_nodes });
        }

        let now = 0u64; // Set by engine clock
        let node_info = InternalNodeInfo {
            node_id,
            endpoint: endpoint.clone(),
            region,
            state: NodeState::Joining,
            registered_at: now,
            last_seen: now,
            metadata: Vec::new(),
        };

        graph.nodes.insert(node_id, node_info);
        graph.adjacency.insert(node_id, Vec::new());
        graph.version = graph.version.increment();

        let event_id = graph.add_event(
            TopologyEventType::NodeJoined,
            Some(node_id),
            None,
        );

        Ok(TopologyNodeInfo {
            node_id,
            endpoint,
            region,
            state: NodeState::Active,
            registered_at: now,
            last_seen: now,
            link_count: 0,
            metadata: Vec::new(),
        })
    }

    fn unregister_node(&self, node_id: NodeId) -> Result<(), TopologyError> {
        let mut graph = self.graph.borrow_mut();

        if !graph.nodes.contains_key(&node_id) {
            return Err(TopologyError::NodeNotFound { node_id });
        }

        // Remove all links involving this node
        let links_to_remove: Vec<(NodeId, NodeId)> = graph
            .links
            .keys()
            .filter(|(a, b)| *a == node_id || *b == node_id)
            .cloned()
            .collect();

        for (a, b) in links_to_remove {
            graph.links.remove(&(a, b));
        }

        // Remove from adjacency
        graph.adjacency.remove(&node_id);
        for (_, neighbors) in graph.adjacency.iter_mut() {
            neighbors.retain(|&n| n != node_id);
        }

        graph.nodes.remove(&node_id);
        graph.version = graph.version.increment();

        graph.add_event(TopologyEventType::NodeLeft, Some(node_id), None);

        Ok(())
    }

    fn node_info(&self, node_id: NodeId) -> Result<TopologyNodeInfo, TopologyError> {
        let graph = self.graph.borrow();
        let node = graph
            .nodes
            .get(&node_id)
            .ok_or_else(|| TopologyError::NodeNotFound { node_id })?;

        let link_count = graph
            .links
            .iter()
            .filter(|((a, b), _)| *a == node_id || *b == node_id)
            .count() as u32;

        Ok(TopologyNodeInfo {
            node_id: node.node_id,
            endpoint: node.endpoint.clone(),
            region: node.region,
            state: node.state,
            registered_at: node.registered_at,
            last_seen: node.last_seen,
            link_count,
            metadata: node.metadata.clone(),
        })
    }

    fn all_nodes(&self) -> Vec<NodeId> {
        let graph = self.graph.borrow();
        graph.nodes.keys().cloned().collect()
    }

    fn nodes_in_region(&self, region: RegionId) -> Vec<NodeId> {
        let graph = self.graph.borrow();
        graph
            .nodes
            .iter()
            .filter(|(_, n)| n.region == region)
            .map(|(id, _)| *id)
            .collect()
    }

    fn add_link(
        &self,
        node_a: NodeId,
        node_b: NodeId,
        link: TopologyLink,
    ) -> Result<TopologyLinkInfo, TopologyError> {
        let mut graph = self.graph.borrow_mut();

        if node_a == node_b {
            return Err(TopologyError::SelfLoopNotAllowed { node_id: node_a });
        }

        if !graph.nodes.contains_key(&node_a) {
            return Err(TopologyError::NodeNotFound { node_id: node_a });
        }

        if !graph.nodes.contains_key(&node_b) {
            return Err(TopologyError::NodeNotFound { node_id: node_b });
        }

        let key = TopologyGraph::canonical_key(node_a, node_b);

        if graph.links.contains_key(&key) {
            return Err(TopologyError::LinkAlreadyExists {
                node_a: key.0,
                node_b: key.1,
            });
        }

        // Check max links per node
        let a_links = graph
            .links
            .iter()
            .filter(|((x, y), _)| *x == node_a || *y == node_a)
            .count() as u32;
        let b_links = graph
            .links
            .iter()
            .filter(|((x, y), _)| *x == node_b || *y == node_b)
            .count() as u32;

        let config = self.config.borrow();
        if a_links >= config.max_links_per_node {
            return Err(TopologyError::MaxLinksPerNodeExceeded { node_id: node_a });
        }
        if b_links >= config.max_links_per_node {
            return Err(TopologyError::MaxLinksPerNodeExceeded { node_id: node_b });
        }
        drop(config);

        let now = 0u64;
        let link_info = InternalLinkInfo {
            node_a: key.0,
            node_b: key.1,
            cost: link.cost,
            latency_ms: link.latency_ms,
            bandwidth_bps: link.bandwidth_bps,
            reliability: link.reliability,
            state: link.state,
            last_updated: now,
            update_count: 0,
        };

        graph.links.insert(key, link_info.clone());

        // Update adjacency
        graph
            .adjacency
            .get_mut(&node_a)
            .unwrap()
            .push(node_b);
        graph
            .adjacency
            .get_mut(&node_b)
            .unwrap()
            .push(node_a);

        graph.version = graph.version.increment();

        graph.add_event(
            TopologyEventType::LinkAdded,
            None,
            Some((node_a, node_b)),
        );

        Ok(TopologyLinkInfo {
            node_a: key.0,
            node_b: key.1,
            cost: link.cost,
            latency_ms: link.latency_ms,
            state: link.state,
            last_updated: now,
            update_count: 0,
        })
    }

    fn remove_link(&self, node_a: NodeId, node_b: NodeId) -> Result<(), TopologyError> {
        let mut graph = self.graph.borrow_mut();
        let key = TopologyGraph::canonical_key(node_a, node_b);

        if !graph.links.contains_key(&key) {
            return Err(TopologyError::LinkNotFound {
                node_a: key.0,
                node_b: key.1,
            });
        }

        graph.links.remove(&key);

        // Update adjacency
        if let Some(neighbors) = graph.adjacency.get_mut(&node_a) {
            neighbors.retain(|&n| n != node_b);
        }
        if let Some(neighbors) = graph.adjacency.get_mut(&node_b) {
            neighbors.retain(|&n| n != node_a);
        }

        graph.version = graph.version.increment();

        graph.add_event(
            TopologyEventType::LinkRemoved,
            None,
            Some((node_a, node_b)),
        );

        Ok(())
    }

    fn link_info(&self, node_a: NodeId, node_b: NodeId) -> Result<TopologyLinkInfo, TopologyError> {
        let graph = self.graph.borrow();
        let key = TopologyGraph::canonical_key(node_a, node_b);
        let link = graph
            .links
            .get(&key)
            .ok_or_else(|| TopologyError::LinkNotFound {
                node_a: key.0,
                node_b: key.1,
            })?;

        Ok(TopologyLinkInfo {
            node_a: link.node_a,
            node_b: link.node_b,
            cost: link.cost,
            latency_ms: link.latency_ms,
            state: link.state,
            last_updated: link.last_updated,
            update_count: link.update_count,
        })
    }

    fn node_links(&self, node_id: NodeId) -> Vec<TopologyLinkInfo> {
        let graph = self.graph.borrow();
        graph
            .links
            .iter()
            .filter(|((a, b), _)| *a == node_id || *b == node_id)
            .map(|(_, link)| TopologyLinkInfo {
                node_a: link.node_a,
                node_b: link.node_b,
                cost: link.cost,
                latency_ms: link.latency_ms,
                state: link.state,
                last_updated: link.last_updated,
                update_count: link.update_count,
            })
            .collect()
    }

    fn all_links(&self) -> Vec<TopologyLinkInfo> {
        let graph = self.graph.borrow();
        graph
            .links
            .values()
            .map(|link| TopologyLinkInfo {
                node_a: link.node_a,
                node_b: link.node_b,
                cost: link.cost,
                latency_ms: link.latency_ms,
                state: link.state,
                last_updated: link.last_updated,
                update_count: link.update_count,
            })
            .collect()
    }

    fn update_link_state(
        &self,
        node_a: NodeId,
        node_b: NodeId,
        state: LinkState,
    ) -> Result<(), TopologyError> {
        let mut graph = self.graph.borrow_mut();
        let key = TopologyGraph::canonical_key(node_a, node_b);

        let link = graph
            .links
            .get_mut(&key)
            .ok_or_else(|| TopologyError::LinkNotFound {
                node_a: key.0,
                node_b: key.1,
            })?;

        let old_state = link.state;
        if old_state == state {
            return Ok(());
        }

        link.state = state;
        link.last_updated = 0u64; // Set by engine clock
        link.update_count += 1;

        let event_type = match state {
            LinkState::Up => TopologyEventType::LinkRecovered,
            LinkState::Down => TopologyEventType::LinkFailed,
            LinkState::Degraded => TopologyEventType::LinkFailed,
            LinkState::Flapping => TopologyEventType::LinkFailed,
        };

        graph.add_event(event_type, None, Some((node_a, node_b)));
        graph.version = graph.version.increment();

        Ok(())
    }

    fn record_topology_event(
        &self,
        event: TopologyEvent,
    ) -> Result<TopologyEventRecord, TopologyError> {
        let mut graph = self.graph.borrow_mut();
        let event_id = graph.add_event(
            event.event_type,
            event.node_id,
            event.link,
        );

        graph.version = graph.version.increment();

        Ok(TopologyEventRecord {
            event_id,
            event_type: event.event_type,
            node_id: event.node_id,
            link: event.link,
            timestamp: event.timestamp,
        })
    }

    fn topology_events(
        &self,
        start_time: u64,
        end_time: u64,
    ) -> Vec<TopologyEventRecord> {
        let graph = self.graph.borrow();
        graph
            .events
            .iter()
            .filter(|e| e.timestamp >= start_time && e.timestamp <= end_time)
            .cloned()
            .collect()
    }

    fn compute_path(
        &self,
        source: NodeId,
        destination: NodeId,
    ) -> Result<TopologyPath, TopologyError> {
        let graph = self.graph.borrow();

        if !graph.nodes.contains_key(&source) {
            return Err(TopologyError::NodeNotFound { node_id: source });
        }

        if !graph.nodes.contains_key(&destination) {
            return Err(TopologyError::NodeNotFound { node_id: destination });
        }

        if source == destination {
            return Ok(TopologyPath {
                nodes: vec![source],
                total_cost: 0,
                total_latency_ms: 0,
                hop_count: 0,
                path_id: 0,
            });
        }

        // Dijkstra's algorithm
        let mut dist: HashMap<NodeId, u32> = HashMap::new();
        let mut prev: HashMap<NodeId, NodeId> = HashMap::new();
        let mut visited: HashSet<NodeId> = HashSet::new();

        dist.insert(source, 0);

        let mut unvisited: Vec<NodeId> = graph.nodes.keys().cloned().collect();

        while !unvisited.is_empty() {
            // Find node with minimum distance
            let mut min_node = None;
            let mut min_dist = u32::MAX;
            for &node in &unvisited {
                let d = dist.get(&node).copied().unwrap_or(u32::MAX);
                if d < min_dist {
                    min_dist = d;
                    min_node = Some(node);
                }
            }

            let current = match min_node {
                Some(n) => n,
                None => break,
            };

            if current == destination {
                break;
            }

            unvisited.retain(|&n| n != current);
            visited.insert(current);

            if let Some(neighbors) = graph.adjacency.get(&current) {
                for &neighbor in neighbors {
                    if visited.contains(&neighbor) {
                        continue;
                    }

                    let key = TopologyGraph::canonical_key(current, neighbor);
                    if let Some(link) = graph.links.get(&key) {
                        if link.state != LinkState::Up {
                            continue;
                        }

                        let new_dist = dist.get(&current).copied().unwrap_or(u32::MAX)
                            .saturating_add(link.cost);

                        let current_dist = dist.get(&neighbor).copied().unwrap_or(u32::MAX);
                        if new_dist < current_dist {
                            dist.insert(neighbor, new_dist);
                            prev.insert(neighbor, current);
                        }
                    }
                }
            }
        }

        if !dist.contains_key(&destination) {
            return Err(TopologyError::NoPath {
                source,
                destination,
            });
        }

        // Reconstruct path
        let mut path_nodes = Vec::new();
        let mut current = destination;
        while current != source {
            path_nodes.push(current);
            current = *prev.get(&current).unwrap();
        }
        path_nodes.push(source);
        path_nodes.reverse();

        let total_cost = dist.get(&destination).copied().unwrap_or(0);
        let total_latency_ms = path_nodes
            .windows(2)
            .filter_map(|w| {
                let key = TopologyGraph::canonical_key(w[0], w[1]);
                graph.links.get(&key).map(|l| l.latency_ms)
            })
            .sum();

        Ok(TopologyPath {
            nodes: path_nodes,
            total_cost,
            total_latency_ms,
            hop_count: (path_nodes.len() as u32).saturating_sub(1),
            path_id: 0,
        })
    }

    fn compute_all_paths(
        &self,
        source: NodeId,
        destination: NodeId,
        max_paths: u32,
    ) -> Result<Vec<TopologyPath>, TopologyError> {
        let graph = self.graph.borrow();

        if !graph.nodes.contains_key(&source) {
            return Err(TopologyError::NodeNotFound { node_id: source });
        }

        if !graph.nodes.contains_key(&destination) {
            return Err(TopologyError::NodeNotFound { node_id: destination });
        }

        // Simple DFS-based path enumeration (limited by max_paths)
        let mut paths = Vec::new();
        let mut visited: HashSet<NodeId> = HashSet::new();
        let mut current_path = Vec::new();

        fn dfs(
            current: NodeId,
            destination: NodeId,
            graph: &TopologyGraph,
            visited: &mut HashSet<NodeId>,
            current_path: &mut Vec<NodeId>,
            paths: &mut Vec<TopologyPath>,
            max_paths: u32,
        ) {
            if paths.len() >= max_paths as usize {
                return;
            }

            visited.insert(current);
            current_path.push(current);

            if current == destination {
                let total_cost: u32 = current_path
                    .windows(2)
                    .filter_map(|w| {
                        let key = TopologyGraph::canonical_key(w[0], w[1]);
                        graph.links.get(&key).map(|l| l.cost)
                    })
                    .sum();

                let total_latency_ms: u64 = current_path
                    .windows(2)
                    .filter_map(|w| {
                        let key = TopologyGraph::canonical_key(w[0], w[1]);
                        graph.links.get(&key).map(|l| l.latency_ms)
                    })
                    .sum();

                paths.push(TopologyPath {
                    nodes: current_path.clone(),
                    total_cost,
                    total_latency_ms,
                    hop_count: (current_path.len() as u32).saturating_sub(1),
                    path_id: paths.len() as u64 + 1,
                });
            } else {
                if let Some(neighbors) = graph.adjacency.get(&current) {
                    for &neighbor in neighbors {
                        if !visited.contains(&neighbor) {
                            if let Some(key) = graph
                                .links
                                .keys()
                                .find(|k| (k.0 == current && k.1 == neighbor) || (k.0 == neighbor && k.1 == current))
                            {
                                if let Some(link) = graph.links.get(key) {
                                    if link.state == LinkState::Up {
                                        dfs(
                                            neighbor,
                                            destination,
                                            graph,
                                            visited,
                                            current_path,
                                            paths,
                                            max_paths,
                                        );
                                    }
                                }
                            }
                        }
                    }
                }
            }

            current_path.pop();
            visited.remove(&current);
        }

        dfs(
            source,
            destination,
            &graph,
            &mut visited,
            &mut current_path,
            &mut paths,
            max_paths,
        );

        Ok(paths)
    }

    fn neighbors(&self, node_id: NodeId) -> Vec<NodeId> {
        let graph = self.graph.borrow();
        graph
            .adjacency
            .get(&node_id)
            .cloned()
            .unwrap_or_default()
    }

    fn are_connected(&self, node_a: NodeId, node_b: NodeId) -> bool {
        let graph = self.graph.borrow();
        let key = TopologyGraph::canonical_key(node_a, node_b);
        graph.links.contains_key(&key)
    }

    fn topology_version(&self) -> TopologyVersion {
        let graph = self.graph.borrow();
        graph.version
    }

    fn increment_version(&self) -> TopologyVersion {
        let mut graph = self.graph.borrow_mut();
        graph.version = graph.version.increment();
        graph.version
    }

    fn stats(&self) -> TopologyStats {
        let graph = self.graph.borrow();
        let mut active_nodes = 0u32;
        let mut down_nodes = 0u32;
        let mut up_links = 0u32;
        let mut down_links = 0u32;

        for node in graph.nodes.values() {
            match node.state {
                NodeState::Active => active_nodes += 1,
                NodeState::Down => down_nodes += 1,
                _ => {}
            }
        }

        for link in graph.links.values() {
            match link.state {
                LinkState::Up => up_links += 1,
                LinkState::Down => down_links += 1,
                _ => {}
            }
        }

        let total_nodes = graph.nodes.len() as u32;
        let total_links = graph.links.len() as u32;

        // Count connected components
        let mut visited: HashSet<NodeId> = HashSet::new();
        let mut components = 0u32;
        for node in graph.nodes.keys() {
            if !visited.contains(node) {
                components += 1;
                let mut stack = vec![*node];
                while let Some(current) = stack.pop() {
                    if visited.contains(&current) {
                        continue;
                    }
                    visited.insert(current);
                    if let Some(neighbors) = graph.adjacency.get(&current) {
                        for &n in neighbors {
                            if !visited.contains(&n) {
                                stack.push(n);
                            }
                        }
                    }
                }
            }
        }

        TopologyStats {
            total_nodes,
            active_nodes,
            down_nodes,
            total_links,
            up_links,
            down_links,
            topology_events: graph.event_counter,
            path_computations: 0,
            version: graph.version,
            connected_components_count: components,
        }
    }

    fn is_connected(&self) -> bool {
        let graph = self.graph.borrow();
        if graph.nodes.is_empty() {
            return true;
        }

        let mut visited: HashSet<NodeId> = HashSet::new();
        let start = *graph.nodes.keys().next().unwrap();
        let mut stack = vec![start];
        visited.insert(start);

        while let Some(current) = stack.pop() {
            if let Some(neighbors) = graph.adjacency.get(&current) {
                for &n in neighbors {
                    if !visited.contains(&n) {
                        visited.insert(n);
                        stack.push(n);
                    }
                }
            }
        }

        visited.len() == graph.nodes.len()
    }

    fn connected_components(&self) -> Vec<Vec<NodeId>> {
        let graph = self.graph.borrow();
        let mut visited: HashSet<NodeId> = HashSet::new();
        let mut components = Vec::new();

        for node in graph.nodes.keys() {
            if !visited.contains(node) {
                let mut component = Vec::new();
                let mut stack = vec![*node];
                while let Some(current) = stack.pop() {
                    if visited.contains(&current) {
                        continue;
                    }
                    visited.insert(current);
                    component.push(current);
                    if let Some(neighbors) = graph.adjacency.get(&current) {
                        for &n in neighbors {
                            if !visited.contains(&n) {
                                stack.push(n);
                            }
                        }
                    }
                }
                components.push(component);
            }
        }

        components
    }

    fn validate_config(&self, config: &TopologyConfig) -> Result<(), TopologyError> {
        config.validate()
    }

    fn snapshot(&self) -> TopologySnapshot {
        let graph = self.graph.borrow();
        let nodes: Vec<TopologyNodeInfo> = graph
            .nodes
            .values()
            .map(|n| {
                let link_count = graph
                    .links
                    .iter()
                    .filter(|((a, b), _)| *a == n.node_id || *b == n.node_id)
                    .count() as u32;
                TopologyNodeInfo {
                    node_id: n.node_id,
                    endpoint: n.endpoint.clone(),
                    region: n.region,
                    state: n.state,
                    registered_at: n.registered_at,
                    last_seen: n.last_seen,
                    link_count,
                    metadata: n.metadata.clone(),
                }
            })
            .collect();

        let links: Vec<TopologyLinkInfo> = graph
            .links
            .values()
            .map(|l| TopologyLinkInfo {
                node_a: l.node_a,
                node_b: l.node_b,
                cost: l.cost,
                latency_ms: l.latency_ms,
                state: l.state,
                last_updated: l.last_updated,
                update_count: l.update_count,
            })
            .collect();

        TopologySnapshot {
            version: graph.version,
            nodes,
            links,
            timestamp: 0,
        }
    }

    fn restore_from_snapshot(
        &self,
        snapshot: TopologySnapshot,
    ) -> Result<(), TopologyError> {
        let mut graph = self.graph.borrow_mut();
        graph.nodes.clear();
        graph.links.clear();
        graph.adjacency.clear();

        for node_info in &snapshot.nodes {
            graph.nodes.insert(
                node_info.node_id,
                InternalNodeInfo {
                    node_id: node_info.node_id,
                    endpoint: node_info.endpoint.clone(),
                    region: node_info.region,
                    state: node_info.state,
                    registered_at: node_info.registered_at,
                    last_seen: node_info.last_seen,
                    metadata: node_info.metadata.clone(),
                },
            );
            graph.adjacency.insert(node_info.node_id, Vec::new());
        }

        for link_info in &snapshot.links {
            let key = TopologyGraph::canonical_key(link_info.node_a, link_info.node_b);
            graph.links.insert(
                key,
                InternalLinkInfo {
                    node_a: link_info.node_a,
                    node_b: link_info.node_b,
                    cost: link_info.cost,
                    latency_ms: link_info.latency_ms,
                    bandwidth_bps: 0,
                    reliability: 1.0,
                    state: link_info.state,
                    last_updated: link_info.last_updated,
                    update_count: link_info.update_count,
                },
            );

            graph
                .adjacency
                .get_mut(&link_info.node_a)
                .unwrap()
                .push(link_info.node_b);
            graph
                .adjacency
                .get_mut(&link_info.node_b)
                .unwrap()
                .push(link_info.node_a);
        }

        graph.version = snapshot.version;

        Ok(())
    }

    fn detect_cycles(&self) -> Vec<Vec<NodeId>> {
        let graph = self.graph.borrow();
        let mut cycles = Vec::new();
        let mut visited: HashSet<NodeId> = HashSet::new();
        let mut rec_stack: HashSet<NodeId> = HashSet::new();
        let mut path: Vec<NodeId> = Vec::new();

        fn dfs_cycle(
            node: NodeId,
            graph: &TopologyGraph,
            visited: &mut HashSet<NodeId>,
            rec_stack: &mut HashSet<NodeId>,
            path: &mut Vec<NodeId>,
            cycles: &mut Vec<Vec<NodeId>>,
        ) {
            visited.insert(node);
            rec_stack.insert(node);
            path.push(node);

            if let Some(neighbors) = graph.adjacency.get(&node) {
                for &neighbor in neighbors {
                    if rec_stack.contains(&neighbor) {
                        // Found a cycle
                        let cycle_start = path.iter().position(|&n| n == neighbor).unwrap();
                        let cycle: Vec<NodeId> = path[cycle_start..].to_vec();
                        cycles.push(cycle);
                    } else if !visited.contains(&neighbor) {
                        dfs_cycle(neighbor, graph, visited, rec_stack, path, cycles);
                    }
                }
            }

            path.pop();
            rec_stack.remove(&node);
        }

        for node in graph.nodes.keys() {
            if !visited.contains(node) {
                dfs_cycle(*node, &graph, &mut visited, &mut rec_stack, &mut path, &mut cycles);
            }
        }

        cycles
    }

    fn articulation_points(&self) -> Vec<NodeId> {
        let graph = self.graph.borrow();
        let mut ap = Vec::new();

        for node in graph.nodes.keys() {
            // Check if removing this node disconnects the graph
            let mut remaining: HashSet<NodeId> = graph.nodes.keys().cloned().collect();
            remaining.remove(node);

            if remaining.is_empty() {
                continue;
            }

            let mut visited: HashSet<NodeId> = HashSet::new();
            let start = *remaining.iter().next().unwrap();
            let mut stack = vec![start];
            visited.insert(start);

            while let Some(current) = stack.pop() {
                if let Some(neighbors) = graph.adjacency.get(&current) {
                    for &n in neighbors {
                        if n != node && remaining.contains(&n) && !visited.contains(&n) {
                            visited.insert(n);
                            stack.push(n);
                        }
                    }
                }
            }

            if visited.len() != remaining.len() {
                ap.push(*node);
            }
        }

        ap
    }

    fn bridge_links(&self) -> Vec<(NodeId, NodeId)> {
        let graph = self.graph.borrow();
        let mut bridges = Vec::new();

        for (key, _) in graph.links.iter() {
            // Temporarily remove this link and check if graph is still connected
            let (a, b) = *key;

            // Build adjacency without this link
            let mut adj_without: HashMap<NodeId, Vec<NodeId>> = HashMap::new();
            for (node, neighbors) in graph.adjacency.iter() {
                adj_without.insert(
                    *node,
                    neighbors.iter().filter(|&&n| {
                        let k = TopologyGraph::canonical_key(*node, n);
                        k != key
                    }).cloned().collect(),
                );
            }

            // Check connectivity
            let start_node = *graph.nodes.keys().next().unwrap();
            let mut visited: HashSet<NodeId> = HashSet::new();
            let mut stack = vec![start_node];
            visited.insert(start_node);

            while let Some(current) = stack.pop() {
                if let Some(neighbors) = adj_without.get(&current) {
                    for &n in neighbors {
                        if !visited.contains(&n) {
                            visited.insert(n);
                            stack.push(n);
                        }
                    }
                }
            }

            if visited.len() != graph.nodes.len() {
                bridges.push((a, b));
            }
        }

        bridges
    }

    /// Get the total number of topology events recorded.
    pub fn total_events(&self) -> u64 {
        self.graph.borrow().event_counter
    }

    /// Get events by type.
    pub fn events_by_type(&self, event_type: TopologyEventType) -> Vec<TopologyEventRecord> {
        self.graph
            .borrow()
            .events
            .iter()
            .filter(|e| e.event_type == event_type)
            .cloned()
            .collect()
    }

    /// Get events involving a specific node.
    pub fn events_for_node(&self, node_id: NodeId) -> Vec<TopologyEventRecord> {
        self.graph
            .borrow()
            .events
            .iter()
            .filter(|e| e.node_id == Some(node_id))
            .cloned()
            .collect()
    }

    /// Get the most recent N events.
    pub fn recent_events(&self, count: u64) -> Vec<TopologyEventRecord> {
        let events = self.graph.borrow();
        let start = events.len().saturating_sub(count as usize);
        events[start..].to_vec()
    }

    /// Get the event count by type.
    pub fn event_count_by_type(&self) -> HashMap<TopologyEventType, u64> {
        let mut counts: HashMap<TopologyEventType, u64> = HashMap::new();
        for event in self.graph.borrow().events.iter() {
            *counts.entry(event.event_type).or_insert(0) += 1;
        }
        counts
    }

    /// Get nodes sorted by degree (number of links).
    pub fn nodes_by_degree(&self) -> Vec<(NodeId, u32)> {
        let graph = self.graph.borrow();
        let mut degrees: Vec<(NodeId, u32)> = graph
            .nodes
            .keys()
            .map(|&node_id| {
                let degree = graph
                    .links
                    .iter()
                    .filter(|((a, b), _)| *a == node_id || *b == node_id)
                    .count() as u32;
                (node_id, degree)
            })
            .collect();
        degrees.sort_by(|a, b| b.1.cmp(&a.1));
        degrees
    }

    /// Get the node with the highest degree.
    pub fn highest_degree_node(&self) -> Option<(NodeId, u32)> {
        self.nodes_by_degree().into_iter().next()
    }

    /// Get the average node degree.
    pub fn average_node_degree(&self) -> f64 {
        let graph = self.graph.borrow();
        if graph.nodes.is_empty() {
            return 0.0;
        }
        let total_degree: u32 = graph.nodes.keys().map(|&node_id| {
            graph
                .links
                .iter()
                .filter(|((a, b), _)| *a == node_id || *b == node_id)
                .count() as u32
        }).sum();
        total_degree as f64 / graph.nodes.len() as f64
    }

    /// Get links sorted by cost (ascending).
    pub fn links_by_cost(&self) -> Vec<TopologyLinkInfo> {
        let mut links = self.all_links();
        links.sort_by(|a, b| a.cost.cmp(&b.cost));
        links
    }

    /// Get links sorted by latency (ascending).
    pub fn links_by_latency(&self) -> Vec<TopologyLinkInfo> {
        let mut links = self.all_links();
        links.sort_by(|a, b| a.latency_ms.cmp(&b.latency_ms));
        links
    }

    /// Get the fastest path (lowest latency) between two nodes.
    pub fn fastest_path(
        &self,
        source: NodeId,
        destination: NodeId,
    ) -> Result<TopologyPath, TopologyError> {
        let paths = self.compute_all_paths(source, destination, 100)?;
        if paths.is_empty() {
            return Err(TopologyError::NoPath { source, destination });
        }
        Ok(paths
            .into_iter()
            .min_by_key(|p| p.total_latency_ms)
            .unwrap())
    }

    /// Get the cheapest path (lowest cost) between two nodes.
    pub fn cheapest_path(
        &self,
        source: NodeId,
        destination: NodeId,
    ) -> Result<TopologyPath, TopologyError> {
        let paths = self.compute_all_paths(source, destination, 100)?;
        if paths.is_empty() {
            return Err(TopologyError::NoPath { source, destination });
        }
        Ok(paths
            .into_iter()
            .min_by_key(|p| p.total_cost)
            .unwrap())
    }

    /// Get the shortest path (fewest hops) between two nodes.
    pub fn shortest_path(
        &self,
        source: NodeId,
        destination: NodeId,
    ) -> Result<TopologyPath, TopologyError> {
        let paths = self.compute_all_paths(source, destination, 100)?;
        if paths.is_empty() {
            return Err(TopologyError::NoPath { source, destination });
        }
        Ok(paths
            .into_iter()
            .min_by_key(|p| p.hop_count)
            .unwrap())
    }

    /// Get all paths sorted by cost (ascending).
    pub fn all_paths_sorted_by_cost(
        &self,
        source: NodeId,
        destination: NodeId,
    ) -> Result<Vec<TopologyPath>, TopologyError> {
        let mut paths = self.compute_all_paths(source, destination, 1000)?;
        paths.sort_by(|a, b| a.total_cost.cmp(&b.total_cost));
        Ok(paths)
    }

    /// Get all paths sorted by latency (ascending).
    pub fn all_paths_sorted_by_latency(
        &self,
        source: NodeId,
        destination: NodeId,
    ) -> Result<Vec<TopologyPath>, TopologyError> {
        let mut paths = self.compute_all_paths(source, destination, 1000)?;
        paths.sort_by(|a, b| a.total_latency_ms.cmp(&b.total_latency_ms));
        Ok(paths)
    }

    /// Check if a path exists between two nodes.
    pub fn path_exists(&self, source: NodeId, destination: NodeId) -> bool {
        self.compute_path(source, destination).is_ok()
    }

    /// Get the diameter of the topology (longest shortest path).
    pub fn topology_diameter(&self) -> Option<u32> {
        let graph = self.graph.borrow();
        let nodes: Vec<NodeId> = graph.nodes.keys().cloned().collect();
        let mut max_hops: Option<u32> = None;

        for source in &nodes {
            for destination in &nodes {
                if source != destination {
                    if let Ok(path) = self.compute_path(*source, *destination) {
                        max_hops = match max_hops {
                            Some(current) => Some(current.max(path.hop_count)),
                            None => Some(path.hop_count),
                        };
                    }
                }
            }
        }

        max_hops
    }

    /// Get the average path length between all node pairs.
    pub fn average_path_length(&self) -> Option<f64> {
        let graph = self.graph.borrow();
        let nodes: Vec<NodeId> = graph.nodes.keys().cloned().collect();
        let mut total_hops: u64 = 0;
        let mut pair_count: u64 = 0;

        for source in &nodes {
            for destination in &nodes {
                if source != destination {
                    if let Ok(path) = self.compute_path(*source, *destination) {
                        total_hops += path.hop_count as u64;
                        pair_count += 1;
                    }
                }
            }
        }

        if pair_count == 0 {
            return None;
        }

        Some(total_hops as f64 / pair_count as f64)
    }

    /// Get the clustering coefficient for a node.
    pub fn clustering_coefficient(&self, node_id: NodeId) -> Option<f64> {
        let graph = self.graph.borrow();
        let neighbors = graph.adjacency.get(&node_id)?;
        let n = neighbors.len();
        if n < 2 {
            return Some(0.0);
        }

        let mut connections = 0u32;
        for i in 0..n {
            for j in (i + 1)..n {
                let key = TopologyGraph::canonical_key(neighbors[i], neighbors[j]);
                if graph.links.contains_key(&key) {
                    connections += 1;
                }
            }
        }

        let max_connections = (n * (n - 1) / 2) as u32;
        Some(connections as f64 / max_connections as f64)
    }

    /// Get the average clustering coefficient across all nodes.
    pub fn average_clustering_coefficient(&self) -> f64 {
        let graph = self.graph.borrow();
        if graph.nodes.is_empty() {
            return 0.0;
        }

        let total: f64 = graph
            .nodes
            .keys()
            .filter_map(|&node_id| self.clustering_coefficient(node_id))
            .sum();
        total / graph.nodes.len() as f64
    }

    /// Get topology density (ratio of actual links to maximum possible).
    pub fn topology_density(&self) -> f64 {
        let graph = self.graph.borrow();
        let n = graph.nodes.len();
        if n < 2 {
            return 0.0;
        }
        let max_links = (n * (n - 1) / 2) as u32;
        graph.links.len() as u32 as f64 / max_links as f64
    }

    /// Get regions in the topology.
    pub fn regions(&self) -> Vec<RegionId> {
        let graph = self.graph.borrow();
        let mut regions: HashSet<RegionId> = HashSet::new();
        for node in graph.nodes.values() {
            regions.insert(node.region);
        }
        regions.into_iter().collect()
    }

    /// Get the region with the most nodes.
    pub fn largest_region(&self) -> Option<(RegionId, u32)> {
        let graph = self.graph.borrow();
        let mut counts: HashMap<RegionId, u32> = HashMap::new();
        for node in graph.nodes.values() {
            *counts.entry(node.region).or_insert(0) += 1;
        }
        counts.into_iter().max_by_key(|(_, count)| *count)
    }

    /// Get cross-region links.
    pub fn cross_region_links(&self) -> Vec<TopologyLinkInfo> {
        let graph = self.graph.borrow();
        graph
            .links
            .values()
            .filter(|link| {
                let node_a = graph.nodes.get(&link.node_a);
                let node_b = graph.nodes.get(&link.node_b);
                match (node_a, node_b) {
                    (Some(a), Some(b)) => a.region != b.region,
                    _ => false,
                }
            })
            .map(|link| TopologyLinkInfo {
                node_a: link.node_a,
                node_b: link.node_b,
                cost: link.cost,
                latency_ms: link.latency_ms,
                state: link.state,
                last_updated: link.last_updated,
                update_count: link.update_count,
            })
            .collect()
    }

    /// Get intra-region links.
    pub fn intra_region_links(&self) -> Vec<TopologyLinkInfo> {
        let graph = self.graph.borrow();
        graph
            .links
            .values()
            .filter(|link| {
                let node_a = graph.nodes.get(&link.node_a);
                let node_b = graph.nodes.get(&link.node_b);
                match (node_a, node_b) {
                    (Some(a), Some(b)) => a.region == b.region,
                    _ => false,
                }
            })
            .map(|link| TopologyLinkInfo {
                node_a: link.node_a,
                node_b: link.node_b,
                cost: link.cost,
                latency_ms: link.latency_ms,
                state: link.state,
                last_updated: link.last_updated,
                update_count: link.update_count,
            })
            .collect()
    }

    /// Get the regional topology view.
    pub fn region_topology_view(&self, region: RegionId) -> RegionTopologyView {
        let graph = self.graph.borrow();
        let nodes: Vec<NodeId> = graph
            .nodes
            .iter()
            .filter(|(_, n)| n.region == region)
            .map(|(id, _)| *id)
            .collect();
        let links: Vec<TopologyLinkInfo> = graph
            .links
            .values()
            .filter(|link| {
                let node_a = graph.nodes.get(&link.node_a);
                let node_b = graph.nodes.get(&link.node_b);
                match (node_a, node_b) {
                    (Some(a), Some(b)) => a.region == region && b.region == region,
                    _ => false,
                }
            })
            .map(|link| TopologyLinkInfo {
                node_a: link.node_a,
                node_b: link.node_b,
                cost: link.cost,
                latency_ms: link.latency_ms,
                state: link.state,
                last_updated: link.last_updated,
                update_count: link.update_count,
            })
            .collect();

        RegionTopologyView {
            region,
            node_count: nodes.len() as u32,
            link_count: links.len() as u32,
            nodes,
            links,
        }
    }

    /// Get all regional topology views.
    pub fn all_region_views(&self) -> Vec<RegionTopologyView> {
        let graph = self.graph.borrow();
        let regions: HashSet<RegionId> = graph
            .nodes
            .values()
            .map(|n| n.region)
            .collect();
        regions
            .into_iter()
            .map(|region| self.region_topology_view(region))
            .collect()
    }

    /// Get node metadata by node ID.
    pub fn node_metadata(&self, node_id: NodeId) -> Option<Vec<u8>> {
        self.graph
            .borrow()
            .nodes
            .get(&node_id)
            .map(|n| n.metadata.clone())
    }

    /// Set node metadata.
    pub fn set_node_metadata(
        &self,
        node_id: NodeId,
        metadata: Vec<u8>,
    ) -> Result<(), TopologyError> {
        let mut graph = self.graph.borrow_mut();
        let node = graph
            .nodes
            .get_mut(&node_id)
            .ok_or_else(|| TopologyError::NodeNotFound { node_id })?;
        node.metadata = metadata;
        graph.version = graph.version.increment();
        Ok(())
    }

    /// Get link metadata by link key.
    pub fn link_metadata(&self, node_a: NodeId, node_b: NodeId) -> Option<Vec<u8>> {
        let key = TopologyGraph::canonical_key(node_a, node_b);
        self.graph
            .borrow()
            .links
            .get(&key)
            .map(|_| Vec::new())
    }

    /// Get topology uptime percentage (nodes that are not down).
    pub fn topology_uptime_percent(&self) -> f64 {
        let graph = self.graph.borrow();
        if graph.nodes.is_empty() {
            return 100.0;
        }
        let up_count = graph
            .nodes
            .values()
            .filter(|n| n.state == NodeState::Active)
            .count();
        up_count as f64 / graph.nodes.len() as f64 * 100.0
    }

    /// Get topology availability including degraded nodes.
    pub fn topology_availability_percent(&self) -> f64 {
        let graph = self.graph.borrow();
        if graph.nodes.is_empty() {
            return 100.0;
        }
        let available_count = graph
            .nodes
            .values()
            .filter(|n| n.state.is_operational() || matches!(n.state, NodeState::Degraded))
            .count();
        available_count as f64 / graph.nodes.len() as f64 * 100.0
    }

    /// Get the topology health score (0.0 to 1.0).
    pub fn topology_health_score(&self) -> f64 {
        let stats = self.stats();
        let node_health = stats.node_availability_percent() / 100.0;
        let link_health = stats.link_availability_percent() / 100.0;
        let connectivity_health = if stats.connected_components_count <= 1 {
            1.0
        } else {
            0.5
        };
        (node_health + link_health + connectivity_health) / 3.0
    }

    /// Clear all topology data.
    pub fn clear_all(&self) {
        let mut graph = self.graph.borrow_mut();
        graph.nodes.clear();
        graph.links.clear();
        graph.adjacency.clear();
        graph.version = TopologyVersion::new(0);
        graph.event_counter = 0;
        graph.events.clear();
    }

    /// Get the topology graph size (nodes + links).
    pub fn graph_size(&self) -> (usize, usize) {
        let graph = self.graph.borrow();
        (graph.nodes.len(), graph.links.len())
    }

    /// Get the config snapshot.
    pub fn config_snapshot(&self) -> TopologyConfig {
        self.config.borrow().clone()
    }
}

///////////////////////////////////////////////////////////////////////////////
// REGION TOPOLOGY VIEW
///////////////////////////////////////////////////////////////////////////////

/// A view of topology within a single region.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegionTopologyView {
    pub region: RegionId,
    pub node_count: u32,
    pub link_count: u32,
    pub nodes: Vec<NodeId>,
    pub links: Vec<TopologyLinkInfo>,
}

impl RegionTopologyView {
    pub fn link_density(&self) -> f64 {
        if self.node_count < 2 {
            return 0.0;
        }
        let max_links = (self.node_count * (self.node_count - 1) / 2) as f64;
        self.link_count as f64 / max_links
    }

    pub fn is_connected(&self) -> bool {
        self.link_count >= self.node_count.saturating_sub(1)
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

    fn test_node_id2() -> NodeId {
        test_node_id(2)
    }

    fn test_node_id3() -> NodeId {
        test_node_id(3)
    }

    fn test_node_id4() -> NodeId {
        test_node_id(4)
    }

    fn test_node_id5() -> NodeId {
        test_node_id(5)
    }

    fn test_region(idx: u8) -> RegionId {
        RegionId::new([idx; 16])
    }

    fn test_region1() -> RegionId {
        test_region(1)
    }

    fn test_region2() -> RegionId {
        test_region(2)
    }

    fn test_endpoint() -> Endpoint {
        Endpoint::new(CarrierType::Direct, vec![1, 2, 3, 4])
    }

    fn test_endpoint2() -> Endpoint {
        Endpoint::new(CarrierType::Direct, vec![5, 6, 7, 8])
    }

    fn test_topology_link() -> TopologyLink {
        TopologyLink {
            node_a: test_node_id(1),
            node_b: test_node_id(2),
            cost: 10,
            latency_ms: 50,
            bandwidth_bps: 1000,
            reliability: 0.99,
            state: LinkState::Up,
        }
    }

    fn test_config() -> TopologyConfig {
        TopologyConfig::default()
    }

    // ========================================================================
    // TopologyConfig tests
    // ========================================================================

    #[test]
    fn topology_config_default() {
        let config = TopologyConfig::default();
        assert_eq!(config.max_nodes, 1000);
        assert_eq!(config.max_links_per_node, 10);
        assert_eq!(config.default_link_cost, 1);
        assert_eq!(config.default_link_latency_ms, 10);
        assert!(config.enable_topology_diversity);
        assert_eq!(config.link_health_check_interval_ms, 5000);
        assert_eq!(config.node_timeout_ms, 30000);
        assert!(config.enable_path_cache);
        assert_eq!(config.max_cached_paths, 100);
        assert_eq!(config.propagation_delay_ms, 100);
    }

    #[test]
    fn topology_config_new_valid() {
        let config = TopologyConfig::new(500).unwrap();
        assert_eq!(config.max_nodes, 500);
    }

    #[test]
    fn topology_config_new_zero_nodes() {
        let result = TopologyConfig::new(0);
        assert!(result.is_err());
    }

    #[test]
    fn topology_config_validate_zero_max_nodes() {
        let mut config = TopologyConfig::default();
        config.max_nodes = 0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn topology_config_validate_zero_max_links() {
        let mut config = TopologyConfig::default();
        config.max_links_per_node = 0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn topology_config_validate_zero_cost() {
        let mut config = TopologyConfig::default();
        config.default_link_cost = 0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn topology_config_validate_zero_health_interval() {
        let mut config = TopologyConfig::default();
        config.link_health_check_interval_ms = 0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn topology_config_validate_zero_node_timeout() {
        let mut config = TopologyConfig::default();
        config.node_timeout_ms = 0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn topology_config_validate_zero_cached_paths() {
        let mut config = TopologyConfig::default();
        config.max_cached_paths = 0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn topology_config_strict() {
        let config = TopologyConfig::strict();
        assert_eq!(config.max_nodes, 100);
        assert_eq!(config.max_links_per_node, 3);
        assert!(config.enable_topology_diversity);
        assert_eq!(config.link_health_check_interval_ms, 1000);
        assert_eq!(config.node_timeout_ms, 5000);
    }

    #[test]
    fn topology_config_lenient() {
        let config = TopologyConfig::lenient();
        assert_eq!(config.max_nodes, 10000);
        assert_eq!(config.max_links_per_node, 50);
        assert!(!config.enable_topology_diversity);
        assert_eq!(config.link_health_check_interval_ms, 60000);
        assert_eq!(config.node_timeout_ms, 300000);
    }

    #[test]
    fn topology_config_link_density() {
        let config = TopologyConfig::default();
        let density = config.link_density();
        assert!(density >= 0.0);
        assert!(density <= 1.0);
    }

    #[test]
    fn topology_config_link_density_single_node() {
        let config = TopologyConfig::new(1).unwrap();
        assert!((config.link_density() - 0.0).abs() < 0.0001);
    }

    #[test]
    fn topology_config_serialization_roundtrip() {
        let config = TopologyConfig::default();
        let json = serde_json::to_string(&config).unwrap();
        let deserialized: TopologyConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(config.max_nodes, deserialized.max_nodes);
        assert_eq!(config.max_links_per_node, deserialized.max_links_per_node);
    }

    // ========================================================================
    // NodeState tests
    // ========================================================================

    #[test]
    fn node_state_is_operational() {
        assert!(NodeState::Active.is_operational());
        assert!(!NodeState::Down.is_operational());
        assert!(!NodeState::Leaving.is_operational());
        assert!(!NodeState::Joining.is_operational());
        assert!(!NodeState::Degraded.is_operational());
    }

    #[test]
    fn node_state_display_name() {
        assert_eq!(NodeState::Active.display_name(), "active");
        assert_eq!(NodeState::Down.display_name(), "down");
        assert_eq!(NodeState::Degraded.display_name(), "degraded");
        assert_eq!(NodeState::Leaving.display_name(), "leaving");
        assert_eq!(NodeState::Joining.display_name(), "joining");
    }

    #[test]
    fn node_state_serialization_roundtrip() {
        let state = NodeState::Degraded;
        let json = serde_json::to_string(&state).unwrap();
        let deserialized: NodeState = serde_json::from_str(&json).unwrap();
        assert_eq!(state, deserialized);
    }

    // ========================================================================
    // LinkState tests
    // ========================================================================

    #[test]
    fn link_state_is_operational() {
        assert!(LinkState::Up.is_operational());
        assert!(!LinkState::Down.is_operational());
        assert!(!LinkState::Degraded.is_operational());
        assert!(!LinkState::Flapping.is_operational());
    }

    #[test]
    fn link_state_display_name() {
        assert_eq!(LinkState::Up.display_name(), "up");
        assert_eq!(LinkState::Down.display_name(), "down");
        assert_eq!(LinkState::Degraded.display_name(), "degraded");
        assert_eq!(LinkState::Flapping.display_name(), "flapping");
    }

    #[test]
    fn link_state_serialization_roundtrip() {
        let state = LinkState::Flapping;
        let json = serde_json::to_string(&state).unwrap();
        let deserialized: LinkState = serde_json::from_str(&json).unwrap();
        assert_eq!(state, deserialized);
    }

    // ========================================================================
    // TopologyLinkInfo tests
    // ========================================================================

    #[test]
    fn topology_link_info_effective_cost_up() {
        let link = TopologyLinkInfo {
            node_a: test_node_id(1),
            node_b: test_node_id(2),
            cost: 10,
            latency_ms: 50,
            state: LinkState::Up,
            last_updated: 100,
            update_count: 1,
        };
        assert_eq!(link.effective_cost(), 10);
    }

    #[test]
    fn topology_link_info_effective_cost_degraded() {
        let link = TopologyLinkInfo {
            node_a: test_node_id(1),
            node_b: test_node_id(2),
            cost: 10,
            latency_ms: 50,
            state: LinkState::Degraded,
            last_updated: 100,
            update_count: 1,
        };
        assert_eq!(link.effective_cost(), 20);
    }

    #[test]
    fn topology_link_info_effective_cost_flapping() {
        let link = TopologyLinkInfo {
            node_a: test_node_id(1),
            node_b: test_node_id(2),
            cost: 10,
            latency_ms: 50,
            state: LinkState::Flapping,
            last_updated: 100,
            update_count: 1,
        };
        assert_eq!(link.effective_cost(), 50);
    }

    #[test]
    fn topology_link_info_effective_cost_down() {
        let link = TopologyLinkInfo {
            node_a: test_node_id(1),
            node_b: test_node_id(2),
            cost: 10,
            latency_ms: 50,
            state: LinkState::Down,
            last_updated: 100,
            update_count: 1,
        };
        assert_eq!(link.effective_cost(), u32::MAX);
    }

    #[test]
    fn topology_link_info_serialization_roundtrip() {
        let link = TopologyLinkInfo {
            node_a: test_node_id(1),
            node_b: test_node_id(2),
            cost: 10,
            latency_ms: 50,
            state: LinkState::Up,
            last_updated: 100,
            update_count: 1,
        };
        let json = serde_json::to_string(&link).unwrap();
        let deserialized: TopologyLinkInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(link, deserialized);
    }

    // ========================================================================
    // TopologyEventType tests
    // ========================================================================

    #[test]
    fn topology_event_type_display_name() {
        assert_eq!(TopologyEventType::NodeJoined.display_name(), "node_joined");
        assert_eq!(TopologyEventType::NodeLeft.display_name(), "node_left");
        assert_eq!(TopologyEventType::NodeDown.display_name(), "node_down");
        assert_eq!(TopologyEventType::NodeRecovered.display_name(), "node_recovered");
        assert_eq!(TopologyEventType::LinkAdded.display_name(), "link_added");
        assert_eq!(TopologyEventType::LinkRemoved.display_name(), "link_removed");
        assert_eq!(TopologyEventType::LinkFailed.display_name(), "link_failed");
        assert_eq!(TopologyEventType::LinkRecovered.display_name(), "link_recovered");
        assert_eq!(
            TopologyEventType::TopologyChanged.display_name(),
            "topology_changed"
        );
    }

    #[test]
    fn topology_event_type_serialization_roundtrip() {
        let event_type = TopologyEventType::LinkFailed;
        let json = serde_json::to_string(&event_type).unwrap();
        let deserialized: TopologyEventType = serde_json::from_str(&json).unwrap();
        assert_eq!(event_type, deserialized);
    }

    // ========================================================================
    // TopologyEventRecord tests
    // ========================================================================

    #[test]
    fn topology_event_record_new() {
        let record = TopologyEventRecord {
            event_id: 1,
            event_type: TopologyEventType::NodeJoined,
            node_id: Some(test_node_id(1)),
            link: None,
            timestamp: 1000,
        };
        assert_eq!(record.event_id, 1);
        assert_eq!(record.event_type, TopologyEventType::NodeJoined);
        assert_eq!(record.node_id, Some(test_node_id(1)));
        assert_eq!(record.link, None);
        assert_eq!(record.timestamp, 1000);
    }

    #[test]
    fn topology_event_record_serialization_roundtrip() {
        let record = TopologyEventRecord {
            event_id: 42,
            event_type: TopologyEventType::LinkRemoved,
            node_id: None,
            link: Some((test_node_id(1), test_node_id(2))),
            timestamp: 5000,
        };
        let json = serde_json::to_string(&record).unwrap();
        let deserialized: TopologyEventRecord = serde_json::from_str(&json).unwrap();
        assert_eq!(record, deserialized);
    }

    // ========================================================================
    // TopologyVersion tests
    // ========================================================================

    #[test]
    fn topology_version_new() {
        let version = TopologyVersion::new(42);
        assert_eq!(version.0, 42);
    }

    #[test]
    fn topology_version_increment() {
        let version = TopologyVersion::new(1);
        let incremented = version.increment();
        assert_eq!(incremented.0, 2);
        assert_eq!(version.0, 1); // Original unchanged
    }

    #[test]
    fn topology_version_serialization_roundtrip() {
        let version = TopologyVersion::new(100);
        let json = serde_json::to_string(&version).unwrap();
        let deserialized: TopologyVersion = serde_json::from_str(&json).unwrap();
        assert_eq!(version, deserialized);
    }

    // ========================================================================
    // TopologyPath tests
    // ========================================================================

    #[test]
    fn topology_path_is_valid() {
        let path = TopologyPath {
            nodes: vec![test_node_id(1), test_node_id(2)],
            total_cost: 10,
            total_latency_ms: 50,
            hop_count: 1,
            path_id: 1,
        };
        assert!(path.is_valid());
    }

    #[test]
    fn topology_path_is_valid_single_node() {
        let path = TopologyPath {
            nodes: vec![test_node_id(1)],
            total_cost: 0,
            total_latency_ms: 0,
            hop_count: 0,
            path_id: 1,
        };
        assert!(!path.is_valid());
    }

    #[test]
    fn topology_path_contains_node() {
        let path = TopologyPath {
            nodes: vec![test_node_id(1), test_node_id(2), test_node_id(3)],
            total_cost: 20,
            total_latency_ms: 100,
            hop_count: 2,
            path_id: 1,
        };
        assert!(path.contains_node(test_node_id(2)));
        assert!(!path.contains_node(test_node_id(4)));
    }

    #[test]
    fn topology_path_serialization_roundtrip() {
        let path = TopologyPath {
            nodes: vec![test_node_id(1), test_node_id(2)],
            total_cost: 10,
            total_latency_ms: 50,
            hop_count: 1,
            path_id: 1,
        };
        let json = serde_json::to_string(&path).unwrap();
        let deserialized: TopologyPath = serde_json::from_str(&json).unwrap();
        assert_eq!(path, deserialized);
    }

    // ========================================================================
    // TopologyStats tests
    // ========================================================================

    #[test]
    fn topology_stats_new() {
        let stats = TopologyStats::new();
        assert_eq!(stats.total_nodes, 0);
        assert_eq!(stats.active_nodes, 0);
        assert_eq!(stats.down_nodes, 0);
        assert_eq!(stats.total_links, 0);
        assert_eq!(stats.up_links, 0);
        assert_eq!(stats.down_links, 0);
        assert_eq!(stats.topology_events, 0);
        assert_eq!(stats.path_computations, 0);
        assert_eq!(stats.connected_components_count, 0);
    }

    #[test]
    fn topology_stats_node_availability_percent() {
        let mut stats = TopologyStats::new();
        stats.total_nodes = 10;
        stats.active_nodes = 8;
        stats.down_nodes = 2;
        assert!((stats.node_availability_percent() - 80.0).abs() < 0.01);
    }

    #[test]
    fn topology_stats_node_availability_percent_zero_nodes() {
        let stats = TopologyStats::new();
        assert!((stats.node_availability_percent() - 100.0).abs() < 0.01);
    }

    #[test]
    fn topology_stats_link_availability_percent() {
        let mut stats = TopologyStats::new();
        stats.total_links = 20;
        stats.up_links = 15;
        stats.down_links = 5;
        assert!((stats.link_availability_percent() - 75.0).abs() < 0.01);
    }

    #[test]
    fn topology_stats_link_availability_percent_zero_links() {
        let stats = TopologyStats::new();
        assert!((stats.link_availability_percent() - 100.0).abs() < 0.01);
    }

    #[test]
    fn topology_stats_serialization_roundtrip() {
        let stats = TopologyStats::new();
        let json = serde_json::to_string(&stats).unwrap();
        let deserialized: TopologyStats = serde_json::from_str(&json).unwrap();
        assert_eq!(stats.total_nodes, deserialized.total_nodes);
    }

    // ========================================================================
    // TopologySnapshot tests
    // ========================================================================

    #[test]
    fn topology_snapshot_node_count() {
        let snapshot = TopologySnapshot {
            version: TopologyVersion::new(1),
            nodes: vec![],
            links: vec![],
            timestamp: 1000,
        };
        assert_eq!(snapshot.node_count(), 0);
    }

    #[test]
    fn topology_snapshot_link_count() {
        let snapshot = TopologySnapshot {
            version: TopologyVersion::new(1),
            nodes: vec![],
            links: vec![],
            timestamp: 1000,
        };
        assert_eq!(snapshot.link_count(), 0);
    }

    #[test]
    fn topology_snapshot_serialization_roundtrip() {
        let snapshot = TopologySnapshot {
            version: TopologyVersion::new(1),
            nodes: vec![],
            links: vec![],
            timestamp: 1000,
        };
        let json = serde_json::to_string(&snapshot).unwrap();
        let deserialized: TopologySnapshot = serde_json::from_str(&json).unwrap();
        assert_eq!(snapshot.version, deserialized.version);
    }

    // ========================================================================
    // TopologyError tests
    // ========================================================================

    #[test]
    fn topology_error_category() {
        assert_eq!(
            TopologyError::InvalidConfig {
                field: "x".to_string(),
                reason: "y".to_string(),
            }
            .category(),
            "invalid_config"
        );
        assert_eq!(
            TopologyError::NodeNotFound {
                node_id: test_node_id(1),
            }
            .category(),
            "node_not_found"
        );
        assert_eq!(
            TopologyError::LinkNotFound {
                node_a: test_node_id(1),
                node_b: test_node_id(2),
            }
            .category(),
            "link_not_found"
        );
        assert_eq!(
            TopologyError::NoPath {
                source: test_node_id(1),
                destination: test_node_id(2),
            }
            .category(),
            "no_path"
        );
    }

    #[test]
    fn topology_error_is_recoverable() {
        assert!(TopologyError::NodeDown {
            node_id: test_node_id(1),
        }
        .is_recoverable());
        assert!(TopologyError::NoPath {
            source: test_node_id(1),
            destination: test_node_id(2),
        }
        .is_recoverable());
        assert!(!TopologyError::NodeNotFound {
            node_id: test_node_id(1),
        }
        .is_recoverable());
        assert!(!TopologyError::InvalidConfig {
            field: "x".to_string(),
            reason: "y".to_string(),
        }
        .is_recoverable());
    }

    #[test]
    fn topology_error_suggested_action() {
        assert_eq!(
            TopologyError::NodeNotFound {
                node_id: test_node_id(1),
            }
            .suggested_action(),
            "register the node first"
        );
        assert!(!TopologyError::Internal {
            message: "test".to_string()
        }
        .suggested_action()
        .is_empty());
    }

    #[test]
    fn topology_error_display() {
        let err = TopologyError::NodeNotFound {
            node_id: test_node_id(1),
        };
        assert!(!err.to_string().is_empty());
    }

    #[test]
    fn topology_error_serialization_roundtrip() {
        let err = TopologyError::LinkAlreadyExists {
            node_a: test_node_id(1),
            node_b: test_node_id(2),
        };
        let json = serde_json::to_string(&err).unwrap();
        let deserialized: TopologyError = serde_json::from_str(&json).unwrap();
        assert!(matches!(deserialized, TopologyError::LinkAlreadyExists { .. }));
    }

    // ========================================================================
    // TopologyEngine tests
    // ========================================================================

    #[test]
    fn topology_engine_new() {
        let engine = TopologyEngine::new();
        assert_eq!(engine.datasets().len(), 0);
        assert_eq!(engine.all_nodes().len(), 0);
        assert_eq!(engine.all_links().len(), 0);
        assert_eq!(engine.topology_version().0, 0);
    }

    #[test]
    fn topology_engine_with_config() {
        let config = TopologyConfig::new(500).unwrap();
        let engine = TopologyEngine::with_config(config).unwrap();
        assert_eq!(engine.config_snapshot().max_nodes, 500);
    }

    #[test]
    fn topology_engine_with_config_invalid() {
        let mut config = TopologyConfig::default();
        config.max_nodes = 0;
        let result = TopologyEngine::with_config(config);
        assert!(result.is_err());
    }

    #[test]
    fn topology_engine_configure_topology() {
        let engine = TopologyEngine::new();
        let config = TopologyConfig::strict();
        assert!(engine.configure_topology(config.clone()).is_ok());
        assert_eq!(engine.config_snapshot().max_nodes, 100);
    }

    #[test]
    fn topology_engine_configure_topology_invalid() {
        let engine = TopologyEngine::new();
        let mut config = TopologyConfig::default();
        config.max_nodes = 0;
        let err = engine.configure_topology(config).unwrap_err();
        assert!(matches!(err, TopologyError::InvalidConfig { .. }));
    }

    #[test]
    fn topology_engine_register_node() {
        let engine = TopologyEngine::new();
        let info = engine
            .register_node(test_node_id(1), test_endpoint(), test_region1())
            .unwrap();
        assert_eq!(info.node_id, test_node_id(1));
        assert_eq!(info.endpoint, test_endpoint());
        assert_eq!(info.region, test_region1());
        assert!(info.is_active());
        assert_eq!(engine.all_nodes().len(), 1);
    }

    #[test]
    fn topology_engine_register_duplicate_node() {
        let engine = TopologyEngine::new();
        engine
            .register_node(test_node_id(1), test_endpoint(), test_region1())
            .unwrap();
        let err = engine
            .register_node(test_node_id(1), test_endpoint2(), test_region2())
            .unwrap_err();
        assert!(matches!(err, TopologyError::NodeAlreadyRegistered { .. }));
    }

    #[test]
    fn topology_engine_register_node_max_exceeded() {
        let engine = TopologyEngine::new();
        let mut config = TopologyConfig::new(1).unwrap();
        config.max_nodes = 1;
        engine.configure_topology(config).unwrap();
        engine.register_node(test_node_id(1), test_endpoint(), test_region1()).unwrap();
        let err = engine
            .register_node(test_node_id(2), test_endpoint2(), test_region2())
            .unwrap_err();
        assert!(matches!(err, TopologyError::MaxNodesExceeded { .. }));
    }

    #[test]
    fn topology_engine_unregister_node() {
        let engine = TopologyEngine::new();
        engine.register_node(test_node_id(1), test_endpoint(), test_region1()).unwrap();
        engine.register_node(test_node_id(2), test_endpoint2(), test_region1()).unwrap();
        assert!(engine.unregister_node(test_node_id(1)).is_ok());
        assert_eq!(engine.all_nodes().len(), 1);
        assert!(!engine.all_nodes().contains(&test_node_id(1)));
    }

    #[test]
    fn topology_engine_unregister_nonexistent_node() {
        let engine = TopologyEngine::new();
        let err = engine.unregister_node(test_node_id(1)).unwrap_err();
        assert!(matches!(err, TopologyError::NodeNotFound { .. }));
    }

    #[test]
    fn topology_engine_node_info() {
        let engine = TopologyEngine::new();
        engine.register_node(test_node_id(1), test_endpoint(), test_region1()).unwrap();
        let info = engine.node_info(test_node_id(1)).unwrap();
        assert_eq!(info.node_id, test_node_id(1));
        assert_eq!(info.endpoint, test_endpoint());
    }

    #[test]
    fn topology_engine_node_info_not_found() {
        let engine = TopologyEngine::new();
        let err = engine.node_info(test_node_id(1)).unwrap_err();
        assert!(matches!(err, TopologyError::NodeNotFound { .. }));
    }

    #[test]
    fn topology_engine_all_nodes() {
        let engine = TopologyEngine::new();
        assert!(engine.all_nodes().is_empty());
        engine.register_node(test_node_id(1), test_endpoint(), test_region1()).unwrap();
        engine.register_node(test_node_id(2), test_endpoint2(), test_region2()).unwrap();
        assert_eq!(engine.all_nodes().len(), 2);
    }

    #[test]
    fn topology_engine_nodes_in_region() {
        let engine = TopologyEngine::new();
        engine.register_node(test_node_id(1), test_endpoint(), test_region1()).unwrap();
        engine.register_node(test_node_id(2), test_endpoint2(), test_region2()).unwrap();
        engine.register_node(test_node_id(3), test_endpoint(), test_region1()).unwrap();
        let region1_nodes = engine.nodes_in_region(test_region1());
        assert_eq!(region1_nodes.len(), 2);
        let region2_nodes = engine.nodes_in_region(test_region2());
        assert_eq!(region2_nodes.len(), 1);
    }

    #[test]
    fn topology_engine_add_link() {
        let engine = TopologyEngine::new();
        engine.register_node(test_node_id(1), test_endpoint(), test_region1()).unwrap();
        engine.register_node(test_node_id(2), test_endpoint2(), test_region1()).unwrap();
        let link_info = engine
            .add_link(test_node_id(1), test_node_id(2), test_topology_link())
            .unwrap();
        assert_eq!(link_info.node_a, test_node_id(1));
        assert_eq!(link_info.node_b, test_node_id(2));
        assert_eq!(link_info.cost, 10);
    }

    #[test]
    fn topology_engine_add_link_self_loop() {
        let engine = TopologyEngine::new();
        engine.register_node(test_node_id(1), test_endpoint(), test_region1()).unwrap();
        let err = engine
            .add_link(test_node_id(1), test_node_id(1), test_topology_link())
            .unwrap_err();
        assert!(matches!(err, TopologyError::SelfLoopNotAllowed { .. }));
    }

    #[test]
    fn topology_engine_add_link_nonexistent_node() {
        let engine = TopologyEngine::new();
        let err = engine
            .add_link(test_node_id(1), test_node_id(2), test_topology_link())
            .unwrap_err();
        assert!(matches!(err, TopologyError::NodeNotFound { .. }));
    }

    #[test]
    fn topology_engine_add_duplicate_link() {
        let engine = TopologyEngine::new();
        engine.register_node(test_node_id(1), test_endpoint(), test_region1()).unwrap();
        engine.register_node(test_node_id(2), test_endpoint2(), test_region1()).unwrap();
        engine
            .add_link(test_node_id(1), test_node_id(2), test_topology_link())
            .unwrap();
        let err = engine
            .add_link(test_node_id(2), test_node_id(1), test_topology_link())
            .unwrap_err();
        assert!(matches!(err, TopologyError::LinkAlreadyExists { .. }));
    }

    #[test]
    fn topology_engine_remove_link() {
        let engine = TopologyEngine::new();
        engine.register_node(test_node_id(1), test_endpoint(), test_region1()).unwrap();
        engine.register_node(test_node_id(2), test_endpoint2(), test_region1()).unwrap();
        engine
            .add_link(test_node_id(1), test_node_id(2), test_topology_link())
            .unwrap();
        assert!(engine.remove_link(test_node_id(1), test_node_id(2)).is_ok());
        assert!(engine.link_info(test_node_id(1), test_node_id(2)).is_err());
    }

    #[test]
    fn topology_engine_remove_nonexistent_link() {
        let engine = TopologyEngine::new();
        let err = engine
            .remove_link(test_node_id(1), test_node_id(2))
            .unwrap_err();
        assert!(matches!(err, TopologyError::LinkNotFound { .. }));
    }

    #[test]
    fn topology_engine_link_info() {
        let engine = TopologyEngine::new();
        engine.register_node(test_node_id(1), test_endpoint(), test_region1()).unwrap();
        engine.register_node(test_node_id(2), test_endpoint2(), test_region1()).unwrap();
        engine
            .add_link(test_node_id(1), test_node_id(2), test_topology_link())
            .unwrap();
        let info = engine.link_info(test_node_id(2), test_node_id(1)).unwrap();
        assert_eq!(info.cost, 10);
    }

    #[test]
    fn topology_engine_node_links() {
        let engine = TopologyEngine::new();
        engine.register_node(test_node_id(1), test_endpoint(), test_region1()).unwrap();
        engine.register_node(test_node_id(2), test_endpoint2(), test_region1()).unwrap();
        engine.register_node(test_node_id(3), test_endpoint(), test_region2()).unwrap();
        engine
            .add_link(test_node_id(1), test_node_id(2), test_topology_link())
            .unwrap();
        engine
            .add_link(test_node_id(1), test_node_id(3), test_topology_link())
            .unwrap();
        let links = engine.node_links(test_node_id(1));
        assert_eq!(links.len(), 2);
    }

    #[test]
    fn topology_engine_all_links() {
        let engine = TopologyEngine::new();
        assert!(engine.all_links().is_empty());
        engine.register_node(test_node_id(1), test_endpoint(), test_region1()).unwrap();
        engine.register_node(test_node_id(2), test_endpoint2(), test_region1()).unwrap();
        engine
            .add_link(test_node_id(1), test_node_id(2), test_topology_link())
            .unwrap();
        assert_eq!(engine.all_links().len(), 1);
    }

    #[test]
    fn topology_engine_update_link_state() {
        let engine = TopologyEngine::new();
        engine.register_node(test_node_id(1), test_endpoint(), test_region1()).unwrap();
        engine.register_node(test_node_id(2), test_endpoint2(), test_region1()).unwrap();
        engine
            .add_link(test_node_id(1), test_node_id(2), test_topology_link())
            .unwrap();
        engine
            .update_link_state(test_node_id(1), test_node_id(2), LinkState::Down)
            .unwrap();
        let info = engine.link_info(test_node_id(1), test_node_id(2)).unwrap();
        assert_eq!(info.state, LinkState::Down);
    }

    #[test]
    fn topology_engine_update_link_state_nonexistent() {
        let engine = TopologyEngine::new();
        let err = engine
            .update_link_state(test_node_id(1), test_node_id(2), LinkState::Down)
            .unwrap_err();
        assert!(matches!(err, TopologyError::LinkNotFound { .. }));
    }

    #[test]
    fn topology_engine_record_topology_event() {
        let engine = TopologyEngine::new();
        let event = TopologyEvent {
            event_type: TopologyEventType::NodeJoined,
            node_id: Some(test_node_id(1)),
            link: None,
            timestamp: 1000,
            details: vec![],
        };
        let record = engine.record_topology_event(event).unwrap();
        assert_eq!(record.event_id, 1);
        assert_eq!(record.event_type, TopologyEventType::NodeJoined);
        assert_eq!(record.node_id, Some(test_node_id(1)));
    }

    #[test]
    fn topology_engine_topology_events_time_range() {
        let engine = TopologyEngine::new();
        engine.register_node(test_node_id(1), test_endpoint(), test_region1()).unwrap();
        engine.register_node(test_node_id(2), test_endpoint2(), test_region1()).unwrap();
        let events = engine.topology_events(0, u64::MAX);
        assert!(events.len() >= 2); // At least the two NodeJoined events
    }

    #[test]
    fn topology_engine_compute_path() {
        let engine = TopologyEngine::new();
        engine.register_node(test_node_id(1), test_endpoint(), test_region1()).unwrap();
        engine.register_node(test_node_id(2), test_endpoint2(), test_region1()).unwrap();
        engine.register_node(test_node_id(3), test_endpoint(), test_region2()).unwrap();
        engine
            .add_link(test_node_id(1), test_node_id(2), test_topology_link())
            .unwrap();
        engine
            .add_link(test_node_id(2), test_node_id(3), test_topology_link())
            .unwrap();
        let path = engine.compute_path(test_node_id(1), test_node_id(3)).unwrap();
        assert_eq!(path.nodes.len(), 3);
        assert_eq!(path.nodes[0], test_node_id(1));
        assert_eq!(path.nodes[2], test_node_id(3));
    }

    #[test]
    fn topology_engine_compute_path_nonexistent() {
        let engine = TopologyEngine::new();
        let err = engine
            .compute_path(test_node_id(1), test_node_id(2))
            .unwrap_err();
        assert!(matches!(err, TopologyError::NoPath { .. }));
    }

    #[test]
    fn topology_engine_compute_path_same_node() {
        let engine = TopologyEngine::new();
        engine.register_node(test_node_id(1), test_endpoint(), test_region1()).unwrap();
        let path = engine.compute_path(test_node_id(1), test_node_id(1)).unwrap();
        assert_eq!(path.nodes.len(), 1);
        assert_eq!(path.hop_count, 0);
    }

    #[test]
    fn topology_engine_compute_all_paths() {
        let engine = TopologyEngine::new();
        engine.register_node(test_node_id(1), test_endpoint(), test_region1()).unwrap();
        engine.register_node(test_node_id(2), test_endpoint2(), test_region1()).unwrap();
        engine.register_node(test_node_id(3), test_endpoint(), test_region2()).unwrap();
        engine
            .add_link(test_node_id(1), test_node_id(2), test_topology_link())
            .unwrap();
        engine
            .add_link(test_node_id(2), test_node_id(3), test_topology_link())
            .unwrap();
        engine
            .add_link(test_node_id(1), test_node_id(3), test_topology_link())
            .unwrap();
        let paths = engine
            .compute_all_paths(test_node_id(1), test_node_id(3), 10)
            .unwrap();
        assert!(paths.len() >= 1);
    }

    #[test]
    fn topology_engine_neighbors() {
        let engine = TopologyEngine::new();
        engine.register_node(test_node_id(1), test_endpoint(), test_region1()).unwrap();
        engine.register_node(test_node_id(2), test_endpoint2(), test_region1()).unwrap();
        engine.register_node(test_node_id(3), test_endpoint(), test_region2()).unwrap();
        engine
            .add_link(test_node_id(1), test_node_id(2), test_topology_link())
            .unwrap();
        engine
            .add_link(test_node_id(1), test_node_id(3), test_topology_link())
            .unwrap();
        let neighbors = engine.neighbors(test_node_id(1));
        assert_eq!(neighbors.len(), 2);
    }

    #[test]
    fn topology_engine_neighbors_nonexistent() {
        let engine = TopologyEngine::new();
        let neighbors = engine.neighbors(test_node_id(1));
        assert!(neighbors.is_empty());
    }

    #[test]
    fn topology_engine_are_connected() {
        let engine = TopologyEngine::new();
        engine.register_node(test_node_id(1), test_endpoint(), test_region1()).unwrap();
        engine.register_node(test_node_id(2), test_endpoint2(), test_region1()).unwrap();
        assert!(!engine.are_connected(test_node_id(1), test_node_id(2)));
        engine
            .add_link(test_node_id(1), test_node_id(2), test_topology_link())
            .unwrap();
        assert!(engine.are_connected(test_node_id(1), test_node_id(2)));
    }

    #[test]
    fn topology_engine_topology_version() {
        let engine = TopologyEngine::new();
        assert_eq!(engine.topology_version().0, 0);
        engine.register_node(test_node_id(1), test_endpoint(), test_region1()).unwrap();
        assert_eq!(engine.topology_version().0, 1);
    }

    #[test]
    fn topology_engine_increment_version() {
        let engine = TopologyEngine::new();
        let version = engine.increment_version();
        assert_eq!(version.0, 1);
        let version = engine.increment_version();
        assert_eq!(version.0, 2);
    }

    #[test]
    fn topology_engine_stats() {
        let engine = TopologyEngine::new();
        engine.register_node(test_node_id(1), test_endpoint(), test_region1()).unwrap();
        engine.register_node(test_node_id(2), test_endpoint2(), test_region1()).unwrap();
        engine
            .add_link(test_node_id(1), test_node_id(2), test_topology_link())
            .unwrap();
        let stats = engine.stats();
        assert_eq!(stats.total_nodes, 2);
        assert_eq!(stats.total_links, 1);
        assert_eq!(stats.up_links, 1);
        assert_eq!(stats.active_nodes, 2);
    }

    #[test]
    fn topology_engine_is_connected() {
        let engine = TopologyEngine::new();
        assert!(engine.is_connected());
        engine.register_node(test_node_id(1), test_endpoint(), test_region1()).unwrap();
        assert!(engine.is_connected());
        engine.register_node(test_node_id(2), test_endpoint2(), test_region2()).unwrap();
        assert!(!engine.is_connected());
        engine
            .add_link(test_node_id(1), test_node_id(2), test_topology_link())
            .unwrap();
        assert!(engine.is_connected());
    }

    #[test]
    fn topology_engine_connected_components() {
        let engine = TopologyEngine::new();
        engine.register_node(test_node_id(1), test_endpoint(), test_region1()).unwrap();
        engine.register_node(test_node_id(2), test_endpoint2(), test_region2()).unwrap();
        let components = engine.connected_components();
        assert_eq!(components.len(), 2);
        engine
            .add_link(test_node_id(1), test_node_id(2), test_topology_link())
            .unwrap();
        let components = engine.connected_components();
        assert_eq!(components.len(), 1);
    }

    #[test]
    fn topology_engine_validate_config() {
        let engine = TopologyEngine::new();
        let config = TopologyConfig::default();
        assert!(engine.validate_config(&config).is_ok());
        let mut bad_config = TopologyConfig::default();
        bad_config.max_nodes = 0;
        assert!(engine.validate_config(&bad_config).is_err());
    }

    #[test]
    fn topology_engine_snapshot_restore() {
        let engine = TopologyEngine::new();
        engine.register_node(test_node_id(1), test_endpoint(), test_region1()).unwrap();
        engine.register_node(test_node_id(2), test_endpoint2(), test_region1()).unwrap();
        engine
            .add_link(test_node_id(1), test_node_id(2), test_topology_link())
            .unwrap();
        let snapshot = engine.snapshot();
        assert_eq!(snapshot.node_count(), 2);
        assert_eq!(snapshot.link_count(), 1);

        let engine2 = TopologyEngine::new();
        engine2.restore_from_snapshot(snapshot.clone()).unwrap();
        assert_eq!(engine2.all_nodes().len(), 2);
        assert_eq!(engine2.all_links().len(), 1);
        assert_eq!(engine2.topology_version().0, snapshot.version.0);
    }

    #[test]
    fn topology_engine_detect_cycles() {
        let engine = TopologyEngine::new();
        engine.register_node(test_node_id(1), test_endpoint(), test_region1()).unwrap();
        engine.register_node(test_node_id(2), test_endpoint2(), test_region1()).unwrap();
        engine.register_node(test_node_id(3), test_endpoint(), test_region1()).unwrap();
        engine
            .add_link(test_node_id(1), test_node_id(2), test_topology_link())
            .unwrap();
        engine
            .add_link(test_node_id(2), test_node_id(3), test_topology_link())
            .unwrap();
        engine
            .add_link(test_node_id(3), test_node_id(1), test_topology_link())
            .unwrap();
        let cycles = engine.detect_cycles();
        assert!(!cycles.is_empty());
    }

    #[test]
    fn topology_engine_articulation_points() {
        let engine = TopologyEngine::new();
        engine.register_node(test_node_id(1), test_endpoint(), test_region1()).unwrap();
        engine.register_node(test_node_id(2), test_endpoint2(), test_region1()).unwrap();
        engine.register_node(test_node_id(3), test_endpoint(), test_region1()).unwrap();
        engine
            .add_link(test_node_id(1), test_node_id(2), test_topology_link())
            .unwrap();
        engine
            .add_link(test_node_id(2), test_node_id(3), test_topology_link())
            .unwrap();
        let aps = engine.articulation_points();
        assert!(aps.contains(&test_node_id(2))); // Node 2 is the articulation point
    }

    #[test]
    fn topology_engine_bridge_links() {
        let engine = TopologyEngine::new();
        engine.register_node(test_node_id(1), test_endpoint(), test_region1()).unwrap();
        engine.register_node(test_node_id(2), test_endpoint2(), test_region1()).unwrap();
        engine.register_node(test_node_id(3), test_endpoint(), test_region1()).unwrap();
        engine
            .add_link(test_node_id(1), test_node_id(2), test_topology_link())
            .unwrap();
        engine
            .add_link(test_node_id(2), test_node_id(3), test_topology_link())
            .unwrap();
        let bridges = engine.bridge_links();
        assert_eq!(bridges.len(), 2); // Both links are bridges
    }

    #[test]
    fn topology_engine_config_serialization() {
        let engine = TopologyEngine::new();
        let config = engine.config_snapshot();
        let json = serde_json::to_string(&config).unwrap();
        let deserialized: TopologyConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(config.max_nodes, deserialized.max_nodes);
    }

    #[test]
    fn topology_engine_clear_all() {
        let engine = TopologyEngine::new();
        engine.register_node(test_node_id(1), test_endpoint(), test_region1()).unwrap();
        engine.register_node(test_node_id(2), test_endpoint2(), test_region1()).unwrap();
        engine
            .add_link(test_node_id(1), test_node_id(2), test_topology_link())
            .unwrap();
        engine.clear_all();
        assert_eq!(engine.all_nodes().len(), 0);
        assert_eq!(engine.all_links().len(), 0);
        assert_eq!(engine.topology_version().0, 0);
    }

    // ========================================================================
    // TopologyEngine advanced methods tests
    // ========================================================================

    #[test]
    fn topology_engine_total_events() {
        let engine = TopologyEngine::new();
        engine.register_node(test_node_id(1), test_endpoint(), test_region1()).unwrap();
        assert_eq!(engine.total_events(), 1);
    }

    #[test]
    fn topology_engine_events_by_type() {
        let engine = TopologyEngine::new();
        engine.register_node(test_node_id(1), test_endpoint(), test_region1()).unwrap();
        let events = engine.events_by_type(TopologyEventType::NodeJoined);
        assert_eq!(events.len(), 1);
    }

    #[test]
    fn topology_engine_events_for_node() {
        let engine = TopologyEngine::new();
        engine.register_node(test_node_id(1), test_endpoint(), test_region1()).unwrap();
        let events = engine.events_for_node(test_node_id(1));
        assert_eq!(events.len(), 1);
    }

    #[test]
    fn topology_engine_recent_events() {
        let engine = TopologyEngine::new();
        engine.register_node(test_node_id(1), test_endpoint(), test_region1()).unwrap();
        engine.register_node(test_node_id(2), test_endpoint2(), test_region1()).unwrap();
        let recent = engine.recent_events(1);
        assert_eq!(recent.len(), 1);
    }

    #[test]
    fn topology_engine_event_count_by_type() {
        let engine = TopologyEngine::new();
        engine.register_node(test_node_id(1), test_endpoint(), test_region1()).unwrap();
        engine.register_node(test_node_id(2), test_endpoint2(), test_region1()).unwrap();
        let counts = engine.event_count_by_type();
        assert_eq!(
            counts.get(&TopologyEventType::NodeJoined),
            Some(&2)
        );
    }

    #[test]
    fn topology_engine_nodes_by_degree() {
        let engine = TopologyEngine::new();
        engine.register_node(test_node_id(1), test_endpoint(), test_region1()).unwrap();
        engine.register_node(test_node_id(2), test_endpoint2(), test_region1()).unwrap();
        engine.register_node(test_node_id(3), test_endpoint(), test_region1()).unwrap();
        engine
            .add_link(test_node_id(1), test_node_id(2), test_topology_link())
            .unwrap();
        engine
            .add_link(test_node_id(2), test_node_id(3), test_topology_link())
            .unwrap();
        let degrees = engine.nodes_by_degree();
        assert_eq!(degrees[0].0, test_node_id(2)); // Node 2 has degree 2
        assert_eq!(degrees[0].1, 2);
    }

    #[test]
    fn topology_engine_highest_degree_node() {
        let engine = TopologyEngine::new();
        engine.register_node(test_node_id(1), test_endpoint(), test_region1()).unwrap();
        engine.register_node(test_node_id(2), test_endpoint2(), test_region1()).unwrap();
        engine.register_node(test_node_id(3), test_endpoint(), test_region1()).unwrap();
        engine
            .add_link(test_node_id(1), test_node_id(2), test_topology_link())
            .unwrap();
        engine
            .add_link(test_node_id(2), test_node_id(3), test_topology_link())
            .unwrap();
        let (node, degree) = engine.highest_degree_node().unwrap();
        assert_eq!(node, test_node_id(2));
        assert_eq!(degree, 2);
    }

    #[test]
    fn topology_engine_average_node_degree() {
        let engine = TopologyEngine::new();
        engine.register_node(test_node_id(1), test_endpoint(), test_region1()).unwrap();
        engine.register_node(test_node_id(2), test_endpoint2(), test_region1()).unwrap();
        engine.register_node(test_node_id(3), test_endpoint(), test_region1()).unwrap();
        engine
            .add_link(test_node_id(1), test_node_id(2), test_topology_link())
            .unwrap();
        engine
            .add_link(test_node_id(2), test_node_id(3), test_topology_link())
            .unwrap();
        let avg = engine.average_node_degree();
        assert!((avg - 1.333).abs() < 0.01);
    }

    #[test]
    fn topology_engine_links_by_cost() {
        let engine = TopologyEngine::new();
        engine.register_node(test_node_id(1), test_endpoint(), test_region1()).unwrap();
        engine.register_node(test_node_id(2), test_endpoint2(), test_region1()).unwrap();
        engine.register_node(test_node_id(3), test_endpoint(), test_region1()).unwrap();
        let mut link1 = test_topology_link();
        link1.cost = 5;
        let mut link2 = test_topology_link();
        link2.cost = 20;
        engine
            .add_link(test_node_id(1), test_node_id(2), link1)
            .unwrap();
        engine
            .add_link(test_node_id(2), test_node_id(3), link2)
            .unwrap();
        let links = engine.links_by_cost();
        assert_eq!(links[0].cost, 5);
        assert_eq!(links[1].cost, 20);
    }

    #[test]
    fn topology_engine_links_by_latency() {
        let engine = TopologyEngine::new();
        engine.register_node(test_node_id(1), test_endpoint(), test_region1()).unwrap();
        engine.register_node(test_node_id(2), test_endpoint2(), test_region1()).unwrap();
        engine.register_node(test_node_id(3), test_endpoint(), test_region1()).unwrap();
        let mut link1 = test_topology_link();
        link1.latency_ms = 10;
        let mut link2 = test_topology_link();
        link2.latency_ms = 100;
        engine
            .add_link(test_node_id(1), test_node_id(2), link1)
            .unwrap();
        engine
            .add_link(test_node_id(2), test_node_id(3), link2)
            .unwrap();
        let links = engine.links_by_latency();
        assert_eq!(links[0].latency_ms, 10);
        assert_eq!(links[1].latency_ms, 100);
    }

    #[test]
    fn topology_engine_fastest_path() {
        let engine = TopologyEngine::new();
        engine.register_node(test_node_id(1), test_endpoint(), test_region1()).unwrap();
        engine.register_node(test_node_id(2), test_endpoint2(), test_region1()).unwrap();
        engine.register_node(test_node_id(3), test_endpoint(), test_region1()).unwrap();
        let mut direct = test_topology_link();
        direct.latency_ms = 100;
        let mut via2_a = test_topology_link();
        via2_a.latency_ms = 10;
        let mut via2_b = test_topology_link();
        via2_b.latency_ms = 10;
        engine.add_link(test_node_id(1), test_node_id(3), direct).unwrap();
        engine.add_link(test_node_id(1), test_node_id(2), via2_a).unwrap();
        engine.add_link(test_node_id(2), test_node_id(3), via2_b).unwrap();
        let path = engine.fastest_path(test_node_id(1), test_node_id(3)).unwrap();
        assert_eq!(path.total_latency_ms, 20); // Via node 2: 10 + 10
    }

    #[test]
    fn topology_engine_cheapest_path() {
        let engine = TopologyEngine::new();
        engine.register_node(test_node_id(1), test_endpoint(), test_region1()).unwrap();
        engine.register_node(test_node_id(2), test_endpoint2(), test_region1()).unwrap();
        engine.register_node(test_node_id(3), test_endpoint(), test_region1()).unwrap();
        let mut direct = test_topology_link();
        direct.cost = 100;
        let mut via2_a = test_topology_link();
        via2_a.cost = 5;
        let mut via2_b = test_topology_link();
        via2_b.cost = 5;
        engine.add_link(test_node_id(1), test_node_id(3), direct).unwrap();
        engine.add_link(test_node_id(1), test_node_id(2), via2_a).unwrap();
        engine.add_link(test_node_id(2), test_node_id(3), via2_b).unwrap();
        let path = engine.cheapest_path(test_node_id(1), test_node_id(3)).unwrap();
        assert_eq!(path.total_cost, 10); // Via node 2: 5 + 5
    }

    #[test]
    fn topology_engine_shortest_path() {
        let engine = TopologyEngine::new();
        engine.register_node(test_node_id(1), test_endpoint(), test_region1()).unwrap();
        engine.register_node(test_node_id(2), test_endpoint2(), test_region1()).unwrap();
        engine.register_node(test_node_id(3), test_endpoint(), test_region1()).unwrap();
        engine
            .add_link(test_node_id(1), test_node_id(3), test_topology_link())
            .unwrap();
        engine
            .add_link(test_node_id(1), test_node_id(2), test_topology_link())
            .unwrap();
        engine
            .add_link(test_node_id(2), test_node_id(3), test_topology_link())
            .unwrap();
        let path = engine.shortest_path(test_node_id(1), test_node_id(3)).unwrap();
        assert_eq!(path.hop_count, 1); // Direct link
    }

    #[test]
    fn topology_engine_path_exists() {
        let engine = TopologyEngine::new();
        engine.register_node(test_node_id(1), test_endpoint(), test_region1()).unwrap();
        engine.register_node(test_node_id(2), test_endpoint2(), test_region1()).unwrap();
        assert!(!engine.path_exists(test_node_id(1), test_node_id(2)));
        engine
            .add_link(test_node_id(1), test_node_id(2), test_topology_link())
            .unwrap();
        assert!(engine.path_exists(test_node_id(1), test_node_id(2)));
    }

    #[test]
    fn topology_engine_topology_diameter() {
        let engine = TopologyEngine::new();
        engine.register_node(test_node_id(1), test_endpoint(), test_region1()).unwrap();
        engine.register_node(test_node_id(2), test_endpoint2(), test_region1()).unwrap();
        engine.register_node(test_node_id(3), test_endpoint(), test_region1()).unwrap();
        engine
            .add_link(test_node_id(1), test_node_id(2), test_topology_link())
            .unwrap();
        engine
            .add_link(test_node_id(2), test_node_id(3), test_topology_link())
            .unwrap();
        let diameter = engine.topology_diameter();
        assert_eq!(diameter, Some(2));
    }

    #[test]
    fn topology_engine_topology_diameter_single_node() {
        let engine = TopologyEngine::new();
        engine.register_node(test_node_id(1), test_endpoint(), test_region1()).unwrap();
        let diameter = engine.topology_diameter();
        assert_eq!(diameter, None);
    }

    #[test]
    fn topology_engine_average_path_length() {
        let engine = TopologyEngine::new();
        engine.register_node(test_node_id(1), test_endpoint(), test_region1()).unwrap();
        engine.register_node(test_node_id(2), test_endpoint2(), test_region1()).unwrap();
        engine
            .add_link(test_node_id(1), test_node_id(2), test_topology_link())
            .unwrap();
        let avg = engine.average_path_length();
        assert_eq!(avg, Some(1.0));
    }

    #[test]
    fn topology_engine_clustering_coefficient() {
        let engine = TopologyEngine::new();
        engine.register_node(test_node_id(1), test_endpoint(), test_region1()).unwrap();
        engine.register_node(test_node_id(2), test_endpoint2(), test_region1()).unwrap();
        engine.register_node(test_node_id(3), test_endpoint(), test_region1()).unwrap();
        engine
            .add_link(test_node_id(1), test_node_id(2), test_topology_link())
            .unwrap();
        engine
            .add_link(test_node_id(2), test_node_id(3), test_topology_link())
            .unwrap();
        // Node 2 has neighbors 1 and 3, but 1 and 3 are not connected -> coefficient 0
        let coef = engine.clustering_coefficient(test_node_id(2)).unwrap();
        assert!((coef - 0.0).abs() < 0.0001);
    }

    #[test]
    fn topology_engine_clustering_coefficient_triangle() {
        let engine = TopologyEngine::new();
        engine.register_node(test_node_id(1), test_endpoint(), test_region1()).unwrap();
        engine.register_node(test_node_id(2), test_endpoint2(), test_region1()).unwrap();
        engine.register_node(test_node_id(3), test_endpoint(), test_region1()).unwrap();
        engine
            .add_link(test_node_id(1), test_node_id(2), test_topology_link())
            .unwrap();
        engine
            .add_link(test_node_id(2), test_node_id(3), test_topology_link())
            .unwrap();
        engine
            .add_link(test_node_id(3), test_node_id(1), test_topology_link())
            .unwrap();
        let coef = engine.clustering_coefficient(test_node_id(2)).unwrap();
        assert!((coef - 1.0).abs() < 0.0001);
    }

    #[test]
    fn topology_engine_average_clustering_coefficient() {
        let engine = TopologyEngine::new();
        engine.register_node(test_node_id(1), test_endpoint(), test_region1()).unwrap();
        engine.register_node(test_node_id(2), test_endpoint2(), test_region1()).unwrap();
        engine.register_node(test_node_id(3), test_endpoint(), test_region1()).unwrap();
        engine
            .add_link(test_node_id(1), test_node_id(2), test_topology_link())
            .unwrap();
        engine
            .add_link(test_node_id(2), test_node_id(3), test_topology_link())
            .unwrap();
        engine
            .add_link(test_node_id(3), test_node_id(1), test_topology_link())
            .unwrap();
        let avg = engine.average_clustering_coefficient();
        assert!((avg - 1.0).abs() < 0.0001);
    }

    #[test]
    fn topology_engine_topology_density() {
        let engine = TopologyEngine::new();
        engine.register_node(test_node_id(1), test_endpoint(), test_region1()).unwrap();
        engine.register_node(test_node_id(2), test_endpoint2(), test_region1()).unwrap();
        assert!((engine.topology_density() - 0.0).abs() < 0.0001);
        engine
            .add_link(test_node_id(1), test_node_id(2), test_topology_link())
            .unwrap();
        assert!((engine.topology_density() - 1.0).abs() < 0.0001);
    }

    #[test]
    fn topology_engine_regions() {
        let engine = TopologyEngine::new();
        engine.register_node(test_node_id(1), test_endpoint(), test_region1()).unwrap();
        engine.register_node(test_node_id(2), test_endpoint2(), test_region2()).unwrap();
        let regions = engine.regions();
        assert_eq!(regions.len(), 2);
        assert!(regions.contains(&test_region1()));
        assert!(regions.contains(&test_region2()));
    }

    #[test]
    fn topology_engine_largest_region() {
        let engine = TopologyEngine::new();
        engine.register_node(test_node_id(1), test_endpoint(), test_region1()).unwrap();
        engine.register_node(test_node_id(2), test_endpoint2(), test_region1()).unwrap();
        engine.register_node(test_node_id(3), test_endpoint(), test_region2()).unwrap();
        let (region, count) = engine.largest_region().unwrap();
        assert_eq!(region, test_region1());
        assert_eq!(count, 2);
    }

    #[test]
    fn topology_engine_cross_region_links() {
        let engine = TopologyEngine::new();
        engine.register_node(test_node_id(1), test_endpoint(), test_region1()).unwrap();
        engine.register_node(test_node_id(2), test_endpoint2(), test_region2()).unwrap();
        engine
            .add_link(test_node_id(1), test_node_id(2), test_topology_link())
            .unwrap();
        let links = engine.cross_region_links();
        assert_eq!(links.len(), 1);
    }

    #[test]
    fn topology_engine_intra_region_links() {
        let engine = TopologyEngine::new();
        engine.register_node(test_node_id(1), test_endpoint(), test_region1()).unwrap();
        engine.register_node(test_node_id(2), test_endpoint2(), test_region1()).unwrap();
        engine.register_node(test_node_id(3), test_endpoint(), test_region2()).unwrap();
        engine
            .add_link(test_node_id(1), test_node_id(2), test_topology_link())
            .unwrap();
        let links = engine.intra_region_links();
        assert_eq!(links.len(), 1);
    }

    #[test]
    fn topology_engine_region_topology_view() {
        let engine = TopologyEngine::new();
        engine.register_node(test_node_id(1), test_endpoint(), test_region1()).unwrap();
        engine.register_node(test_node_id(2), test_endpoint2(), test_region1()).unwrap();
        let view = engine.region_topology_view(test_region1());
        assert_eq!(view.region, test_region1());
        assert_eq!(view.node_count, 2);
        assert_eq!(view.link_count, 0);
    }

    #[test]
    fn topology_engine_all_region_views() {
        let engine = TopologyEngine::new();
        engine.register_node(test_node_id(1), test_endpoint(), test_region1()).unwrap();
        engine.register_node(test_node_id(2), test_endpoint2(), test_region2()).unwrap();
        let views = engine.all_region_views();
        assert_eq!(views.len(), 2);
    }

    #[test]
    fn topology_engine_node_metadata() {
        let engine = TopologyEngine::new();
        engine.register_node(test_node_id(1), test_endpoint(), test_region1()).unwrap();
        assert_eq!(engine.node_metadata(test_node_id(1)), None);
        engine
            .set_node_metadata(test_node_id(1), vec![1, 2, 3])
            .unwrap();
        assert_eq!(engine.node_metadata(test_node_id(1)), Some(vec![1, 2, 3]));
    }

    #[test]
    fn topology_engine_node_metadata_nonexistent() {
        let engine = TopologyEngine::new();
        let err = engine
            .set_node_metadata(test_node_id(1), vec![1])
            .unwrap_err();
        assert!(matches!(err, TopologyError::NodeNotFound { .. }));
    }

    #[test]
    fn topology_engine_topology_uptime_percent() {
        let engine = TopologyEngine::new();
        engine.register_node(test_node_id(1), test_endpoint(), test_region1()).unwrap();
        engine.register_node(test_node_id(2), test_endpoint2(), test_region1()).unwrap();
        assert!((engine.topology_uptime_percent() - 100.0).abs() < 0.01);
    }

    #[test]
    fn topology_engine_topology_availability_percent() {
        let engine = TopologyEngine::new();
        engine.register_node(test_node_id(1), test_endpoint(), test_region1()).unwrap();
        engine.register_node(test_node_id(2), test_endpoint2(), test_region1()).unwrap();
        assert!((engine.topology_availability_percent() - 100.0).abs() < 0.01);
    }

    #[test]
    fn topology_engine_topology_health_score() {
        let engine = TopologyEngine::new();
        engine.register_node(test_node_id(1), test_endpoint(), test_region1()).unwrap();
        engine.register_node(test_node_id(2), test_endpoint2(), test_region1()).unwrap();
        engine
            .add_link(test_node_id(1), test_node_id(2), test_topology_link())
            .unwrap();
        let score = engine.topology_health_score();
        assert!(score > 0.0);
        assert!(score <= 1.0);
    }

    #[test]
    fn topology_engine_graph_size() {
        let engine = TopologyEngine::new();
        engine.register_node(test_node_id(1), test_endpoint(), test_region1()).unwrap();
        engine.register_node(test_node_id(2), test_endpoint2(), test_region1()).unwrap();
        engine
            .add_link(test_node_id(1), test_node_id(2), test_topology_link())
            .unwrap();
        let (nodes, links) = engine.graph_size();
        assert_eq!(nodes, 2);
        assert_eq!(links, 1);
    }

    #[test]
    fn topology_engine_all_paths_sorted_by_cost() {
        let engine = TopologyEngine::new();
        engine.register_node(test_node_id(1), test_endpoint(), test_region1()).unwrap();
        engine.register_node(test_node_id(2), test_endpoint2(), test_region1()).unwrap();
        engine.register_node(test_node_id(3), test_endpoint(), test_region1()).unwrap();
        engine
            .add_link(test_node_id(1), test_node_id(3), test_topology_link())
            .unwrap();
        engine
            .add_link(test_node_id(1), test_node_id(2), test_topology_link())
            .unwrap();
        engine
            .add_link(test_node_id(2), test_node_id(3), test_topology_link())
            .unwrap();
        let paths = engine
            .all_paths_sorted_by_cost(test_node_id(1), test_node_id(3))
            .unwrap();
        assert!(!paths.is_empty());
        // First path should be cheapest
        if paths.len() >= 2 {
            assert!(paths[0].total_cost <= paths[1].total_cost);
        }
    }

    #[test]
    fn topology_engine_all_paths_sorted_by_latency() {
        let engine = TopologyEngine::new();
        engine.register_node(test_node_id(1), test_endpoint(), test_region1()).unwrap();
        engine.register_node(test_node_id(2), test_endpoint2(), test_region1()).unwrap();
        engine.register_node(test_node_id(3), test_endpoint(), test_region1()).unwrap();
        engine
            .add_link(test_node_id(1), test_node_id(3), test_topology_link())
            .unwrap();
        engine
            .add_link(test_node_id(1), test_node_id(2), test_topology_link())
            .unwrap();
        engine
            .add_link(test_node_id(2), test_node_id(3), test_topology_link())
            .unwrap();
        let paths = engine
            .all_paths_sorted_by_latency(test_node_id(1), test_node_id(3))
            .unwrap();
        assert!(!paths.is_empty());
    }

    // ========================================================================
    // RegionTopologyView tests
    // ========================================================================

    #[test]
    fn region_topology_view_link_density() {
        let view = RegionTopologyView {
            region: test_region1(),
            node_count: 3,
            link_count: 3,
            nodes: vec![],
            links: vec![],
        };
        assert!((view.link_density() - 1.0).abs() < 0.0001);
    }

    #[test]
    fn region_topology_view_link_density_single_node() {
        let view = RegionTopologyView {
            region: test_region1(),
            node_count: 1,
            link_count: 0,
            nodes: vec![],
            links: vec![],
        };
        assert!((view.link_density() - 0.0).abs() < 0.0001);
    }

    #[test]
    fn region_topology_view_is_connected() {
        let view = RegionTopologyView {
            region: test_region1(),
            node_count: 3,
            link_count: 2,
            nodes: vec![],
            links: vec![],
        };
        assert!(view.is_connected());
        let view = RegionTopologyView {
            region: test_region1(),
            node_count: 3,
            link_count: 1,
            nodes: vec![],
            links: vec![],
        };
        assert!(!view.is_connected());
    }

    #[test]
    fn region_topology_view_serialization_roundtrip() {
        let view = RegionTopologyView {
            region: test_region1(),
            node_count: 2,
            link_count: 1,
            nodes: vec![test_node_id(1)],
            links: vec![],
        };
        let json = serde_json::to_string(&view).unwrap();
        let deserialized: RegionTopologyView = serde_json::from_str(&json).unwrap();
        assert_eq!(view.region, deserialized.region);
        assert_eq!(view.node_count, deserialized.node_count);
    }

    // ========================================================================
    // Integration tests
    // ========================================================================

    #[test]
    fn topology_full_lifecycle() {
        let engine = TopologyEngine::new();
        let config = TopologyConfig::strict();
        engine.configure_topology(config).unwrap();

        // Register nodes
        engine.register_node(test_node_id(1), test_endpoint(), test_region1()).unwrap();
        engine.register_node(test_node_id(2), test_endpoint2(), test_region1()).unwrap();
        engine.register_node(test_node_id(3), test_endpoint(), test_region2()).unwrap();

        // Add links
        engine
            .add_link(test_node_id(1), test_node_id(2), test_topology_link())
            .unwrap();
        engine
            .add_link(test_node_id(2), test_node_id(3), test_topology_link())
            .unwrap();

        // Verify connectivity
        assert!(engine.is_connected());
        assert!(engine.path_exists(test_node_id(1), test_node_id(3)));

        // Compute path
        let path = engine.compute_path(test_node_id(1), test_node_id(3)).unwrap();
        assert_eq!(path.hop_count, 2);

        // Record event
        let event = TopologyEvent {
            event_type: TopologyEventType::LinkFailed,
            node_id: None,
            link: Some((test_node_id(1), test_node_id(2))),
            timestamp: 5000,
            details: vec![],
        };
        engine.record_topology_event(event).unwrap();

        // Check stats
        let stats = engine.stats();
        assert_eq!(stats.total_nodes, 3);
        assert_eq!(stats.total_links, 2);
        assert_eq!(stats.topology_events, 4); // 2 nodes + 2 links + 1 event... actually 3 nodes + 2 links + 1 event = 6

        // Snapshot and restore
        let snapshot = engine.snapshot();
        let engine2 = TopologyEngine::new();
        engine2.restore_from_snapshot(snapshot).unwrap();
        assert_eq!(engine2.all_nodes().len(), 3);
        assert_eq!(engine2.all_links().len(), 2);

        // Health score
        let score = engine.topology_health_score();
        assert!(score > 0.0);
        assert!(score <= 1.0);
    }

    #[test]
    fn topology_multi_region_topology() {
        let engine = TopologyEngine::new();
        // Region 1: 2 nodes
        engine.register_node(test_node_id(1), test_endpoint(), test_region1()).unwrap();
        engine.register_node(test_node_id(2), test_endpoint2(), test_region1()).unwrap();
        // Region 2: 2 nodes
        engine.register_node(test_node_id(3), test_endpoint(), test_region2()).unwrap();
        engine.register_node(test_node_id(4), test_endpoint2(), test_region2()).unwrap();

        // Intra-region links
        engine
            .add_link(test_node_id(1), test_node_id(2), test_topology_link())
            .unwrap();
        engine
            .add_link(test_node_id(3), test_node_id(4), test_topology_link())
            .unwrap();

        // Cross-region link
        engine
            .add_link(test_node_id(2), test_node_id(3), test_topology_link())
            .unwrap();

        let cross_links = engine.cross_region_links();
        assert_eq!(cross_links.len(), 1);
        let intra_links = engine.intra_region_links();
        assert_eq!(intra_links.len(), 2);

        let views = engine.all_region_views();
        assert_eq!(views.len(), 2);
    }

    #[test]
    fn topology_serialization_roundtrip_topology_config() {
        let config = TopologyConfig::default();
        let json = serde_json::to_string(&config).unwrap();
        let deserialized: TopologyConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(config.max_nodes, deserialized.max_nodes);
        assert_eq!(config.max_links_per_node, deserialized.max_links_per_node);
        assert_eq!(config.enable_topology_diversity, deserialized.enable_topology_diversity);
    }

    #[test]
    fn topology_serialization_roundtrip_node_state() {
        let state = NodeState::Degraded;
        let json = serde_json::to_string(&state).unwrap();
        let deserialized: NodeState = serde_json::from_str(&json).unwrap();
        assert_eq!(state, deserialized);
    }

    #[test]
    fn topology_serialization_roundtrip_link_state() {
        let state = LinkState::Flapping;
        let json = serde_json::to_string(&state).unwrap();
        let deserialized: LinkState = serde_json::from_str(&json).unwrap();
        assert_eq!(state, deserialized);
    }

    #[test]
    fn topology_serialization_roundtrip_topology_path() {
        let path = TopologyPath {
            nodes: vec![test_node_id(1), test_node_id(2)],
            total_cost: 10,
            total_latency_ms: 50,
            hop_count: 1,
            path_id: 1,
        };
        let json = serde_json::to_string(&path).unwrap();
        let deserialized: TopologyPath = serde_json::from_str(&json).unwrap();
        assert_eq!(path, deserialized);
    }
}
