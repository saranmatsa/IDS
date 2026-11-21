//! # Sealed Trait: Membership — join/leave/lifecycle (candidate→active→retired)
//!
//! Carrier-agnostic: operates on opaque Endpoints and IDs only.
//! No std::net, tokio::net, libp2p, IP, TCP, UDP, QUIC, HTTP, or DNS.
//!
//! The Membership trait defines how nodes and H-units join, leave, and
//! transition through lifecycle states in a carrier-agnostic distributed
//! system. Implementations live downstream in ids-membership and ids-* crates.

use crate::types::{
    ConsistencyProfile, DatasetId, Endpoint, HUnitId, NodeId, OrderingPos, RecordId, RegionId,
};
use crate::errors::MembershipError;

///////////////////////////////////////////////////////////////////////////////
// SEALED TRAIT: Membership
///////////////////////////////////////////////////////////////////////////////

mod private {
    pub trait Sealed {}
}

impl private::Sealed for super::MembershipEngine {}
impl private::Sealed for super::NodeLifecycle {}
impl private::Sealed for super::HUnitLifecycle {}
impl private::Sealed for super::MembershipManager {}

/// Sealed trait for node / H-unit membership lifecycle.
/// Downstream crates can call methods and use as trait bounds but cannot implement.
pub trait Membership: private::Sealed {
    /// Join a node to the network.
    fn join_node(&self, node: NodeInfo) -> Result<(), MembershipError>;

    /// Leave the network (graceful departure).
    fn leave_node(&self, node_id: &NodeId) -> Result<(), MembershipError>;

    /// Retire a node (mark as leaving, drain workloads).
    fn retire_node(&self, node_id: &NodeId) -> Result<(), MembershipError>;

    /// Get the current lifecycle state of a node.
    fn node_state(&self, node_id: &NodeId) -> Result<NodeLifecycleState, MembershipError>;

    /// Get the current lifecycle state of an H-unit.
    fn hunit_state(&self, hunit_id: &HUnitId) -> Result<HUnitLifecycleState, MembershipError>;

    /// List all nodes in a given lifecycle state.
    fn nodes_in_state(
        &self,
        state: NodeLifecycleState,
    ) -> Result<Vec<NodeInfo>, MembershipError>;

    /// List all H-units in a given lifecycle state.
    fn hunits_in_state(
        &self,
        state: HUnitLifecycleState,
    ) -> Result<Vec<HUnitInfo>, MembershipError>;

    /// Transition an H-unit to active state.
    fn activate_hunit(&self, hunit: HUnitInfo) -> Result<(), MembershipError>;

    /// Transition an H-unit to retired state.
    fn retire_hunit(&self, hunit_id: &HUnitId) -> Result<(), MembershipError>;

    /// Check if a node is a member of the network.
    fn is_member(&self, node_id: &NodeId) -> bool;

    /// Get the membership name for debugging.
    fn membership_name(&self) -> &'static str;

    /// Get the total member count.
    fn member_count(&self) -> usize;

    /// Get the active member count.
    fn active_member_count(&self) -> usize;
}

///////////////////////////////////////////////////////////////////////////////
// MEMBERSHIP ERROR
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq)]
pub enum MembershipError {
    #[error("node not found: {node_id:?}")]
    NodeNotFound { node_id: NodeId },

    #[error("H-unit not found: {hunit_id:?}")]
    HUnitNotFound { hunit_id: HUnitId },

    #[error("node already member: {node_id:?}")]
    AlreadyMember { node_id: NodeId },

    #[error("node not a member: {node_id:?}")]
    NotMember { node_id: NodeId },

    #[error("invalid lifecycle transition: {from:?} -> {to:?}")]
    InvalidTransition {
        from: NodeLifecycleState,
        to: NodeLifecycleState,
    },

    #[error("invalid H-unit lifecycle transition: {from:?} -> {to:?}")]
    InvalidHUnitTransition {
        from: HUnitLifecycleState,
        to: HUnitLifecycleState,
    },

    #[error("membership table full: max={max}")]
    MembershipTableFull { max: usize },

    #[error("leave in progress: {node_id:?}")]
    LeaveInProgress { node_id: NodeId },

    #[error("retire in progress: {node_id:?}")]
    RetireInProgress { node_id: NodeId },

    #[error("drain timeout: {node_id:?}")]
    DrainTimeout { node_id: NodeId },

    #[error("quorum not met for membership change")]
    QuorumNotMet,

    #[error("internal membership error: {message}")]
    Internal { message: String },
}

impl MembershipError {
    pub fn category(&self) -> &'static str {
        match self {
            MembershipError::NodeNotFound { .. } => "node_not_found",
            MembershipError::HUnitNotFound { .. } => "hunit_not_found",
            MembershipError::AlreadyMember { .. } => "already_member",
            MembershipError::NotMember { .. } => "not_member",
            MembershipError::InvalidTransition { .. } => "invalid_transition",
            MembershipError::InvalidHUnitTransition { .. } => "invalid_hunit_transition",
            MembershipError::MembershipTableFull { .. } => "table_full",
            MembershipError::LeaveInProgress { .. } => "leave_in_progress",
            MembershipError::RetireInProgress { .. } => "retire_in_progress",
            MembershipError::DrainTimeout { .. } => "drain_timeout",
            MembershipError::QuorumNotMet => "quorum_not_met",
            MembershipError::Internal { .. } => "internal",
        }
    }

    pub fn is_recoverable(&self) -> bool {
        matches!(
            self,
            MembershipError::LeaveInProgress { .. }
                | MembershipError::RetireInProgress { .. }
                | MembershipError::DrainTimeout { .. }
        )
    }

    pub fn suggested_action(&self) -> &'static str {
        match self {
            MembershipError::NodeNotFound { .. } => "re-join node to network",
            MembershipError::HUnitNotFound { .. } => "re-register H-unit",
            MembershipError::AlreadyMember { .. } => "use existing membership",
            MembershipError::NotMember { .. } => "join network first",
            MembershipError::InvalidTransition { .. } => "follow lifecycle ordering",
            MembershipError::InvalidHUnitTransition { .. } => "follow H-unit lifecycle ordering",
            MembershipError::MembershipTableFull { .. } => "retire stale members",
            MembershipError::LeaveInProgress { .. } => "wait for leave to complete",
            MembershipError::RetireInProgress { .. } => "wait for retire to complete",
            MembershipError::DrainTimeout { .. } => "force drain or extend timeout",
            MembershipError::QuorumNotMet => "wait for quorum",
            MembershipError::Internal { .. } => "contact support",
        }
    }
}

///////////////////////////////////////////////////////////////////////////////
// NODE LIFECYCLE STATE
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeLifecycleState {
    Candidate,
    Joining,
    Active,
    Leaving,
    Retired,
    Failed,
}

impl NodeLifecycleState {
    pub fn as_str(&self) -> &'static str {
        match self {
            NodeLifecycleState::Candidate => "candidate",
            NodeLifecycleState::Joining => "joining",
            NodeLifecycleState::Active => "active",
            NodeLifecycleState::Leaving => "leaving",
            NodeLifecycleState::Retired => "retired",
            NodeLifecycleState::Failed => "failed",
        }
    }

    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            NodeLifecycleState::Retired | NodeLifecycleState::Failed
        )
    }

    pub fn is_active(&self) -> bool {
        matches!(self, NodeLifecycleState::Active)
    }

    pub fn can_transition_to(&self, target: NodeLifecycleState) -> bool {
        match self {
            NodeLifecycleState::Candidate => {
                matches!(target, NodeLifecycleState::Joining | NodeLifecycleState::Failed)
            }
            NodeLifecycleState::Joining => {
                matches!(target, NodeLifecycleState::Active | NodeLifecycleState::Failed)
            }
            NodeLifecycleState::Active => {
                matches!(
                    target,
                    NodeLifecycleState::Leaving
                        | NodeLifecycleState::Retired
                        | NodeLifecycleState::Failed
                )
            }
            NodeLifecycleState::Leaving => {
                matches!(target, NodeLifecycleState::Retired | NodeLifecycleState::Failed)
            }
            NodeLifecycleState::Retired => matches!(target, NodeLifecycleState::Candidate),
            NodeLifecycleState::Failed => matches!(target, NodeLifecycleState::Candidate),
        }
    }
}

///////////////////////////////////////////////////////////////////////////////
// H-UNIT LIFECYCLE STATE
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HUnitLifecycleState {
    Pending,
    Active,
    Draining,
    Retired,
    Failed,
}

impl HUnitLifecycleState {
    pub fn as_str(&self) -> &'static str {
        match self {
            HUnitLifecycleState::Pending => "pending",
            HUnitLifecycleState::Active => "active",
            HUnitLifecycleState::Draining => "draining",
            HUnitLifecycleState::Retired => "retired",
            HUnitLifecycleState::Failed => "failed",
        }
    }

    pub fn is_terminal(&self) -> bool {
        matches!(self, HUnitLifecycleState::Retired | HUnitLifecycleState::Failed)
    }

    pub fn is_active(&self) -> bool {
        matches!(self, HUnitLifecycleState::Active)
    }

    pub fn can_transition_to(&self, target: HUnitLifecycleState) -> bool {
        match self {
            HUnitLifecycleState::Pending => {
                matches!(target, HUnitLifecycleState::Active | HUnitLifecycleState::Failed)
            }
            HUnitLifecycleState::Active => {
                matches!(
                    target,
                    HUnitLifecycleState::Draining
                        | HUnitLifecycleState::Retired
                        | HUnitLifecycleState::Failed
                )
            }
            HUnitLifecycleState::Draining => {
                matches!(target, HUnitLifecycleState::Retired | HUnitLifecycleState::Failed)
            }
            HUnitLifecycleState::Retired => matches!(target, HUnitLifecycleState::Pending),
            HUnitLifecycleState::Failed => matches!(target, HUnitLifecycleState::Pending),
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
    pub state: NodeLifecycleState,
    pub joined_at_ms: u64,
    pub last_heartbeat_ms: u64,
    pub capabilities: Vec<String>,
    pub metadata: Vec<(String, String)>,
}

impl NodeInfo {
    pub fn new(node_id: NodeId, endpoint: Endpoint, region_id: RegionId) -> Self {
        Self {
            node_id,
            endpoint,
            region_id,
            state: NodeLifecycleState::Candidate,
            joined_at_ms: 0,
            last_heartbeat_ms: 0,
            capabilities: Vec::new(),
            metadata: Vec::new(),
        }
    }

    pub fn with_state(mut self, state: NodeLifecycleState) -> Self {
        self.state = state;
        self
    }

    pub fn with_joined_at(mut self, joined_at_ms: u64) -> Self {
        self.joined_at_ms = joined_at_ms;
        self
    }

    pub fn with_heartbeat(mut self, last_heartbeat_ms: u64) -> Self {
        self.last_heartbeat_ms = last_heartbeat_ms;
        self
    }

    pub fn with_capabilities(mut self, capabilities: Vec<String>) -> Self {
        self.capabilities = capabilities;
        self
    }

    pub fn with_metadata(mut self, key: String, value: String) -> Self {
        self.metadata.push((key, value));
        self
    }

    pub fn is_stale(&self, now_ms: u64, threshold_ms: u64) -> bool {
        now_ms.saturating_sub(self.last_heartbeat_ms) > threshold_ms
    }

    pub fn uptime_ms(&self, now_ms: u64) -> u64 {
        now_ms.saturating_sub(self.joined_at_ms)
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
    pub state: HUnitLifecycleState,
    pub datasets: Vec<DatasetId>,
    pub activated_at_ms: u64,
    pub retired_at_ms: Option<u64>,
}

impl HUnitInfo {
    pub fn new(hunit_id: HUnitId, node_id: NodeId, region_id: RegionId) -> Self {
        Self {
            hunit_id,
            node_id,
            region_id,
            state: HUnitLifecycleState::Pending,
            datasets: Vec::new(),
            activated_at_ms: 0,
            retired_at_ms: None,
        }
    }

    pub fn with_state(mut self, state: HUnitLifecycleState) -> Self {
        self.state = state;
        self
    }

    pub fn with_datasets(mut self, datasets: Vec<DatasetId>) -> Self {
        self.datasets = datasets;
        self
    }

    pub fn with_activated_at(mut self, activated_at_ms: u64) -> Self {
        self.activated_at_ms = activated_at_ms;
        self
    }

    pub fn retire(mut self, retired_at_ms: u64) -> Self {
        self.state = HUnitLifecycleState::Retired;
        self.retired_at_ms = Some(retired_at_ms);
        self
    }

    pub fn hosts_dataset(&self, dataset_id: &DatasetId) -> bool {
        self.datasets.iter().any(|d| d == dataset_id)
    }

    pub fn is_retired(&self) -> bool {
        matches!(self.state, HUnitLifecycleState::Retired)
    }
}

///////////////////////////////////////////////////////////////////////////////
// MEMBERSHIP CONFIG
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone)]
pub struct MembershipConfig {
    pub max_members: usize,
    pub max_hunits: usize,
    pub join_timeout_ms: u64,
    pub leave_timeout_ms: u64,
    pub drain_timeout_ms: u64,
    pub heartbeat_threshold_ms: u64,
    pub quorum_required: bool,
}

impl MembershipConfig {
    pub fn new() -> Self {
        Self {
            max_members: 10000,
            max_hunits: 100000,
            join_timeout_ms: 30000,
            leave_timeout_ms: 60000,
            drain_timeout_ms: 120000,
            heartbeat_threshold_ms: 300000,
            quorum_required: true,
        }
    }

    pub fn with_max_members(mut self, max: usize) -> Self {
        self.max_members = max;
        self
    }

    pub fn with_join_timeout(mut self, ms: u64) -> Self {
        self.join_timeout_ms = ms;
        self
    }

    pub fn with_drain_timeout(mut self, ms: u64) -> Self {
        self.drain_timeout_ms = ms;
        self
    }

    pub fn with_quorum_required(mut self, required: bool) -> Self {
        self.quorum_required = required;
        self
    }
}

impl Default for MembershipConfig {
    fn default() -> Self {
        Self::new()
    }
}

///////////////////////////////////////////////////////////////////////////////
// MEMBERSHIP ENGINE
///////////////////////////////////////////////////////////////////////////////

pub struct MembershipEngine {
    pub nodes: std::cell::RefCell<std::collections::HashMap<NodeId, NodeInfo>>,
    pub hunits: std::cell::RefCell<std::collections::HashMap<HUnitId, HUnitInfo>>,
    pub config: MembershipConfig,
    pub join_count: u64,
    pub leave_count: u64,
    pub retire_count: u64,
    pub activate_count: u64,
    pub transition_count: u64,
}

impl MembershipEngine {
    pub fn new(config: MembershipConfig) -> Self {
        Self {
            nodes: std::cell::RefCell::new(std::collections::HashMap::new()),
            hunits: std::cell::RefCell::new(std::collections::HashMap::new()),
            config,
            join_count: 0,
            leave_count: 0,
            retire_count: 0,
            activate_count: 0,
            transition_count: 0,
        }
    }

    pub fn join_node(&self, mut node: NodeInfo) -> Result<(), MembershipError> {
        let mut nodes = self.nodes.borrow_mut();
        if nodes.len() >= self.config.max_members && !nodes.contains_key(&node.node_id) {
            return Err(MembershipError::MembershipTableFull {
                max: self.config.max_members,
            });
        }
        if nodes.contains_key(&node.node_id) {
            return Err(MembershipError::AlreadyMember {
                node_id: node.node_id.clone(),
            });
        }
        node.state = NodeLifecycleState::Joining;
        node.joined_at_ms = 1000;
        nodes.insert(node.node_id.clone(), node);
        drop(nodes);
        self.join_count += 1;
        Ok(())
    }

    pub fn leave_node(&self, node_id: &NodeId) -> Result<(), MembershipError> {
        let mut nodes = self.nodes.borrow_mut();
        let node = match nodes.get_mut(node_id) {
            Some(n) => n,
            None => return Err(MembershipError::NodeNotFound {
                node_id: node_id.clone(),
            }),
        };
        if !node.state.can_transition_to(NodeLifecycleState::Leaving) {
            return Err(MembershipError::InvalidTransition {
                from: node.state,
                to: NodeLifecycleState::Leaving,
            });
        }
        node.state = NodeLifecycleState::Leaving;
        drop(nodes);
        self.leave_count += 1;
        Ok(())
    }

    pub fn retire_node(&self, node_id: &NodeId) -> Result<(), MembershipError> {
        let mut nodes = self.nodes.borrow_mut();
        let node = match nodes.get_mut(node_id) {
            Some(n) => n,
            None => return Err(MembershipError::NodeNotFound {
                node_id: node_id.clone(),
            }),
        };
        if !node.state.can_transition_to(NodeLifecycleState::Retired) {
            return Err(MembershipError::InvalidTransition {
                from: node.state,
                to: NodeLifecycleState::Retired,
            });
        }
        node.state = NodeLifecycleState::Retired;
        drop(nodes);
        self.retire_count += 1;
        Ok(())
    }

    pub fn activate_node(&self, node_id: &NodeId) -> Result<(), MembershipError> {
        let mut nodes = self.nodes.borrow_mut();
        let node = match nodes.get_mut(node_id) {
            Some(n) => n,
            None => return Err(MembershipError::NodeNotFound {
                node_id: node_id.clone(),
            }),
        };
        if !node.state.can_transition_to(NodeLifecycleState::Active) {
            return Err(MembershipError::InvalidTransition {
                from: node.state,
                to: NodeLifecycleState::Active,
            });
        }
        node.state = NodeLifecycleState::Active;
        drop(nodes);
        Ok(())
    }

    pub fn fail_node(&self, node_id: &NodeId) -> Result<(), MembershipError> {
        let mut nodes = self.nodes.borrow_mut();
        let node = match nodes.get_mut(node_id) {
            Some(n) => n,
            None => return Err(MembershipError::NodeNotFound {
                node_id: node_id.clone(),
            }),
        };
        node.state = NodeLifecycleState::Failed;
        drop(nodes);
        Ok(())
    }

    pub fn node_state(&self, node_id: &NodeId) -> Result<NodeLifecycleState, MembershipError> {
        let nodes = self.nodes.borrow();
        match nodes.get(node_id) {
            Some(node) => Ok(node.state),
            None => Err(MembershipError::NodeNotFound {
                node_id: node_id.clone(),
            }),
        }
    }

    pub fn hunit_state(&self, hunit_id: &HUnitId) -> Result<HUnitLifecycleState, MembershipError> {
        let hunits = self.hunits.borrow();
        match hunits.get(hunit_id) {
            Some(hunit) => Ok(hunit.state),
            None => Err(MembershipError::HUnitNotFound {
                hunit_id: hunit_id.clone(),
            }),
        }
    }

    pub fn activate_hunit(&self, mut hunit: HUnitInfo) -> Result<(), MembershipError> {
        let mut hunits = self.hunits.borrow_mut();
        if hunits.contains_key(&hunit.hunit_id) {
            let existing = hunits.get(&hunit.hunit_id).unwrap();
            if !existing.state.can_transition_to(HUnitLifecycleState::Active) {
                return Err(MembershipError::InvalidHUnitTransition {
                    from: existing.state,
                    to: HUnitLifecycleState::Active,
                });
            }
        }
        hunit.state = HUnitLifecycleState::Active;
        hunit.activated_at_ms = 2000;
        hunits.insert(hunit.hunit_id.clone(), hunit);
        drop(hunits);
        self.activate_count += 1;
        Ok(())
    }

    pub fn retire_hunit(&self, hunit_id: &HUnitId) -> Result<(), MembershipError> {
        let mut hunits = self.hunits.borrow_mut();
        let hunit = match hunits.get_mut(hunit_id) {
            Some(h) => h,
            None => return Err(MembershipError::HUnitNotFound {
                hunit_id: hunit_id.clone(),
            }),
        };
        if !hunit.state.can_transition_to(HUnitLifecycleState::Retired) {
            return Err(MembershipError::InvalidHUnitTransition {
                from: hunit.state,
                to: HUnitLifecycleState::Retired,
            });
        }
        hunit.state = HUnitLifecycleState::Retired;
        hunit.retired_at_ms = Some(3000);
        drop(hunits);
        self.retire_count += 1;
        Ok(())
    }

    pub fn nodes_in_state(
        &self,
        state: NodeLifecycleState,
    ) -> Result<Vec<NodeInfo>, MembershipError> {
        let nodes = self.nodes.borrow();
        Ok(nodes
            .values()
            .filter(|n| n.state == state)
            .cloned()
            .collect())
    }

    pub fn hunits_in_state(
        &self,
        state: HUnitLifecycleState,
    ) -> Result<Vec<HUnitInfo>, MembershipError> {
        let hunits = self.hunits.borrow();
        Ok(hunits
            .values()
            .filter(|h| h.state == state)
            .cloned()
            .collect())
    }

    pub fn is_member(&self, node_id: &NodeId) -> bool {
        self.nodes.borrow().contains_key(node_id)
    }

    pub fn member_count(&self) -> usize {
        self.nodes.borrow().len()
    }

    pub fn active_member_count(&self) -> usize {
        self.nodes
            .borrow()
            .values()
            .filter(|n| n.state == NodeLifecycleState::Active)
            .count()
    }

    pub fn hunit_count(&self) -> usize {
        self.hunits.borrow().len()
    }

    pub fn join_count(&self) -> u64 {
        self.join_count
    }

    pub fn leave_count(&self) -> u64 {
        self.leave_count
    }

    pub fn retire_count(&self) -> u64 {
        self.retire_count
    }

    pub fn activate_count(&self) -> u64 {
        self.activate_count
    }

    pub fn transition_count(&self) -> u64 {
        self.transition_count
    }
}

impl Membership for MembershipEngine {
    fn join_node(&self, node: NodeInfo) -> Result<(), MembershipError> {
        self.join_node(node)
    }

    fn leave_node(&self, node_id: &NodeId) -> Result<(), MembershipError> {
        self.leave_node(node_id)
    }

    fn retire_node(&self, node_id: &NodeId) -> Result<(), MembershipError> {
        self.retire_node(node_id)
    }

    fn node_state(&self, node_id: &NodeId) -> Result<NodeLifecycleState, MembershipError> {
        self.node_state(node_id)
    }

    fn hunit_state(&self, hunit_id: &HUnitId) -> Result<HUnitLifecycleState, MembershipError> {
        self.hunit_state(hunit_id)
    }

    fn nodes_in_state(
        &self,
        state: NodeLifecycleState,
    ) -> Result<Vec<NodeInfo>, MembershipError> {
        self.nodes_in_state(state)
    }

    fn hunits_in_state(
        &self,
        state: HUnitLifecycleState,
    ) -> Result<Vec<HUnitInfo>, MembershipError> {
        self.hunits_in_state(state)
    }

    fn activate_hunit(&self, hunit: HUnitInfo) -> Result<(), MembershipError> {
        self.activate_hunit(hunit)
    }

    fn retire_hunit(&self, hunit_id: &HUnitId) -> Result<(), MembershipError> {
        self.retire_hunit(hunit_id)
    }

    fn is_member(&self, node_id: &NodeId) -> bool {
        self.is_member(node_id)
    }

    fn membership_name(&self) -> &'static str {
        "membership_engine"
    }

    fn member_count(&self) -> usize {
        self.member_count()
    }

    fn active_member_count(&self) -> usize {
        self.active_member_count()
    }
}

impl Default for MembershipEngine {
    fn default() -> Self {
        Self::new(MembershipConfig::default())
    }
}

///////////////////////////////////////////////////////////////////////////////
// NODE LIFECYCLE MANAGER
///////////////////////////////////////////////////////////////////////////////

pub struct NodeLifecycle {
    pub lifecycle_states: std::cell::RefCell<
        std::collections::HashMap<NodeId, NodeLifecycleState>,
    >,
    pub state_transitions: u64,
    pub failed_transitions: u64,
}

impl NodeLifecycle {
    pub fn new() -> Self {
        Self {
            lifecycle_states: std::cell::RefCell::new(std::collections::HashMap::new()),
            state_transitions: 0,
            failed_transitions: 0,
        }
    }

    pub fn set_state(&self, node_id: NodeId, state: NodeLifecycleState) {
        self.lifecycle_states
            .borrow_mut()
            .insert(node_id, state);
        self.state_transitions += 1;
    }

    pub fn get_state(&self, node_id: &NodeId) -> NodeLifecycleState {
        self.lifecycle_states
            .borrow()
            .get(node_id)
            .copied()
            .unwrap_or(NodeLifecycleState::Candidate)
    }

    pub fn can_transition(
        &self,
        node_id: &NodeId,
        target: NodeLifecycleState,
    ) -> bool {
        let current = self.get_state(node_id);
        current.can_transition_to(target)
    }

    pub fn transition(
        &self,
        node_id: NodeId,
        target: NodeLifecycleState,
    ) -> Result<(), MembershipError> {
        if !self.can_transition(&node_id, target) {
            self.failed_transitions += 1;
            return Err(MembershipError::InvalidTransition {
                from: self.get_state(&node_id),
                to: target,
            });
        }
        self.set_state(node_id, target);
        Ok(())
    }

    pub fn transition_count(&self) -> u64 {
        self.state_transitions
    }

    pub fn failed_transition_count(&self) -> u64 {
        self.failed_transitions
    }

    pub fn tracked_count(&self) -> usize {
        self.lifecycle_states.borrow().len()
    }
}

impl Default for NodeLifecycle {
    fn default() -> Self {
        Self::new()
    }
}

///////////////////////////////////////////////////////////////////////////////
// H-UNIT LIFECYCLE MANAGER
///////////////////////////////////////////////////////////////////////////////

pub struct HUnitLifecycle {
    pub lifecycle_states: std::cell::RefCell<
        std::collections::HashMap<HUnitId, HUnitLifecycleState>,
    >,
    pub state_transitions: u64,
    pub failed_transitions: u64,
}

impl HUnitLifecycle {
    pub fn new() -> Self {
        Self {
            lifecycle_states: std::cell::RefCell::new(std::collections::HashMap::new()),
            state_transitions: 0,
            failed_transitions: 0,
        }
    }

    pub fn set_state(&self, hunit_id: HUnitId, state: HUnitLifecycleState) {
        self.lifecycle_states
            .borrow_mut()
            .insert(hunit_id, state);
        self.state_transitions += 1;
    }

    pub fn get_state(&self, hunit_id: &HUnitId) -> HUnitLifecycleState {
        self.lifecycle_states
            .borrow()
            .get(hunit_id)
            .copied()
            .unwrap_or(HUnitLifecycleState::Pending)
    }

    pub fn can_transition(
        &self,
        hunit_id: &HUnitId,
        target: HUnitLifecycleState,
    ) -> bool {
        let current = self.get_state(hunit_id);
        current.can_transition_to(target)
    }

    pub fn transition(
        &self,
        hunit_id: HUnitId,
        target: HUnitLifecycleState,
    ) -> Result<(), MembershipError> {
        if !self.can_transition(&hunit_id, target) {
            self.failed_transitions += 1;
            return Err(MembershipError::InvalidHUnitTransition {
                from: self.get_state(&hunit_id),
                to: target,
            });
        }
        self.set_state(hunit_id, target);
        Ok(())
    }

    pub fn transition_count(&self) -> u64 {
        self.state_transitions
    }

    pub fn failed_transition_count(&self) -> u64 {
        self.failed_transitions
    }

    pub fn tracked_count(&self) -> usize {
        self.lifecycle_states.borrow().len()
    }
}

impl Default for HUnitLifecycle {
    fn default() -> Self {
        Self::new()
    }
}

///////////////////////////////////////////////////////////////////////////////
// MEMBERSHIP MANAGER
///////////////////////////////////////////////////////////////////////////////

pub struct MembershipManager {
    pub engine: std::cell::RefCell<MembershipEngine>,
    pub node_lifecycle: std::cell::RefCell<NodeLifecycle>,
    pub hunit_lifecycle: std::cell::RefCell<HUnitLifecycle>,
    pub membership_rounds: u64,
}

impl MembershipManager {
    pub fn new() -> Self {
        Self {
            engine: std::cell::RefCell::new(MembershipEngine::default()),
            node_lifecycle: std::cell::RefCell::new(NodeLifecycle::default()),
            hunit_lifecycle: std::cell::RefCell::new(HUnitLifecycle::default()),
            membership_rounds: 0,
        }
    }

    pub fn join_node(&self, node: NodeInfo) -> Result<(), MembershipError> {
        self.engine.borrow_mut().join_node(node)
    }

    pub fn leave_node(&self, node_id: &NodeId) -> Result<(), MembershipError> {
        self.engine.borrow_mut().leave_node(node_id)
    }

    pub fn retire_node(&self, node_id: &NodeId) -> Result<(), MembershipError> {
        self.engine.borrow_mut().retire_node(node_id)
    }

    pub fn activate_hunit(&self, hunit: HUnitInfo) -> Result<(), MembershipError> {
        self.engine.borrow_mut().activate_hunit(hunit)
    }

    pub fn retire_hunit(&self, hunit_id: &HUnitId) -> Result<(), MembershipError> {
        self.engine.borrow_mut().retire_hunit(hunit_id)
    }

    pub fn membership_round(&self) -> Result<(), MembershipError> {
        let engine = self.engine.borrow();
        let active = engine.active_member_count();
        drop(engine);
        self.membership_rounds += 1;
        if active == 0 {
            return Err(MembershipError::QuorumNotMet);
        }
        Ok(())
    }

    pub fn is_member(&self, node_id: &NodeId) -> bool {
        self.engine.borrow().is_member(node_id)
    }

    pub fn member_count(&self) -> usize {
        self.engine.borrow().member_count()
    }

    pub fn membership_rounds(&self) -> u64 {
        self.membership_rounds
    }
}

impl Default for MembershipManager {
    fn default() -> Self {
        Self::new()
    }
}

///////////////////////////////////////////////////////////////////////////////
// MEMBERSHIP SUMMARY
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MembershipSummary {
    pub total_members: usize,
    pub active_members: usize,
    pub total_hunits: usize,
    pub active_hunits: usize,
    pub join_count: u64,
    pub leave_count: u64,
}

impl MembershipSummary {
    pub fn new() -> Self {
        Self {
            total_members: 0,
            active_members: 0,
            total_hunits: 0,
            active_hunits: 0,
            join_count: 0,
            leave_count: 0,
        }
    }

    pub fn member_ratio(&self) -> f64 {
        if self.total_members == 0 {
            return 0.0;
        }
        self.active_members as f64 / self.total_members as f64
    }
}

///////////////////////////////////////////////////////////////////////////////
// MEMBERSHIP CACHE
///////////////////////////////////////////////////////////////////////////////

pub struct MembershipCache {
    pub node_state_cache: std::cell::RefCell<
        std::collections::HashMap<NodeId, NodeLifecycleState>,
    >,
    pub hunit_state_cache: std::cell::RefCell<
        std::collections::HashMap<HUnitId, HUnitLifecycleState>,
    >,
    pub max_entries: usize,
    pub hits: u64,
    pub misses: u64,
}

impl MembershipCache {
    pub fn new(max_entries: usize) -> Self {
        Self {
            node_state_cache: std::cell::RefCell::new(std::collections::HashMap::new()),
            hunit_state_cache: std::cell::RefCell::new(std::collections::HashMap::new()),
            max_entries,
            hits: 0,
            misses: 0,
        }
    }

    pub fn cache_node_state(
        &self,
        node_id: NodeId,
        state: NodeLifecycleState,
    ) {
        if self.node_state_cache.borrow().len() >= self.max_entries {
            if let Some(key) = self.node_state_cache.borrow().keys().next().cloned() {
                self.node_state_cache.borrow_mut().remove(&key);
            }
        }
        self.node_state_cache.borrow_mut().insert(node_id, state);
    }

    pub fn get_cached_node_state(
        &self,
        node_id: &NodeId,
    ) -> Option<NodeLifecycleState> {
        match self.node_state_cache.borrow().get(node_id) {
            Some(state) => {
                self.hits += 1;
                Some(*state)
            }
            None => {
                self.misses += 1;
                None
            }
        }
    }

    pub fn cache_hunit_state(
        &self,
        hunit_id: HUnitId,
        state: HUnitLifecycleState,
    ) {
        if self.hunit_state_cache.borrow().len() >= self.max_entries {
            if let Some(key) = self.hunit_state_cache.borrow().keys().next().cloned() {
                self.hunit_state_cache.borrow_mut().remove(&key);
            }
        }
        self.hunit_state_cache.borrow_mut().insert(hunit_id, state);
    }

    pub fn get_cached_hunit_state(
        &self,
        hunit_id: &HUnitId,
    ) -> Option<HUnitLifecycleState> {
        match self.hunit_state_cache.borrow().get(hunit_id) {
            Some(state) => {
                self.hits += 1;
                Some(*state)
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
        self.node_state_cache.borrow_mut().clear();
        self.hunit_state_cache.borrow_mut().clear();
    }

    pub fn entry_count(&self) -> usize {
        self.node_state_cache.borrow().len()
            + self.hunit_state_cache.borrow().len()
    }
}

impl Default for MembershipCache {
    fn default() -> Self {
        Self::new(10000)
    }
}

///////////////////////////////////////////////////////////////////////////////
// MEMBERSHIP EXPIRATION
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MembershipEntryStatus {
    Fresh,
    Stale,
    Expired,
}

impl MembershipEntryStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            MembershipEntryStatus::Fresh => "fresh",
            MembershipEntryStatus::Stale => "stale",
            MembershipEntryStatus::Expired => "expired",
        }
    }

    pub fn is_fresh(&self) -> bool {
        matches!(self, MembershipEntryStatus::Fresh)
    }

    pub fn is_expired(&self) -> bool {
        matches!(self, MembershipEntryStatus::Expired)
    }
}

pub struct MembershipExpiration {
    pub entry_ttls: std::cell::RefCell<
        std::collections::HashMap<NodeId, (u64, MembershipEntryStatus)>,
    >,
    pub default_ttl_ms: u64,
    pub expired_count: u64,
}

impl MembershipExpiration {
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
            .insert(node_id, (ttl_ms, MembershipEntryStatus::Fresh));
    }

    pub fn get_status(
        &self,
        node_id: &NodeId,
        now_ms: u64,
    ) -> MembershipEntryStatus {
        let ttl_map = self.entry_ttls.borrow();
        match ttl_map.get(node_id) {
            Some((ttl, status)) => {
                if *status == MembershipEntryStatus::Expired {
                    MembershipEntryStatus::Expired
                } else if now_ms >= *ttl {
                    MembershipEntryStatus::Expired
                } else if now_ms.saturating_sub(*ttl / 2) > *ttl {
                    MembershipEntryStatus::Stale
                } else {
                    MembershipEntryStatus::Fresh
                }
            }
            None => MembershipEntryStatus::Expired,
        }
    }

    pub fn mark_expired(&self, node_id: &NodeId) {
        self.entry_ttls
            .borrow_mut()
            .insert(node_id.clone(), (0, MembershipEntryStatus::Expired));
        self.expired_count += 1;
    }

    pub fn mark_fresh(&self, node_id: NodeId, ttl_ms: u64) {
        self.entry_ttls
            .borrow_mut()
            .insert(node_id, (ttl_ms, MembershipEntryStatus::Fresh));
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

impl Default for MembershipExpiration {
    fn default() -> Self {
        Self::new(300000)
    }
}

///////////////////////////////////////////////////////////////////////////////
// MEMBERSHIP VALIDATOR
///////////////////////////////////////////////////////////////////////////////

pub struct MembershipValidator {
    pub validations: u64,
    pub validation_failures: u64,
    pub invalid_transitions: u64,
}

impl MembershipValidator {
    pub fn new() -> Self {
        Self {
            validations: 0,
            validation_failures: 0,
            invalid_transitions: 0,
        }
    }

    pub fn validate_node_info(&mut self, node: &NodeInfo) -> bool {
        self.validations += 1;
        if node.endpoint.address.is_empty() {
            self.validation_failures += 1;
            return false;
        }
        true
    }

    pub fn validate_transition(
        &mut self,
        from: NodeLifecycleState,
        to: NodeLifecycleState,
    ) -> bool {
        self.validations += 1;
        if from.can_transition_to(to) {
            true
        } else {
            self.validation_failures += 1;
            self.invalid_transitions += 1;
            false
        }
    }

    pub fn pass_rate(&self) -> f64 {
        if self.validations == 0 {
            return 1.0;
        }
        (self.validations - self.validation_failures) as f64 / self.validations as f64
    }
}

impl Default for MembershipValidator {
    fn default() -> Self {
        Self::new()
    }
}

///////////////////////////////////////////////////////////////////////////////
// QUORUM TRACKER
///////////////////////////////////////////////////////////////////////////////

pub struct QuorumTracker {
    pub total_members: u64,
    pub voting_members: u64,
    pub required_quorum: u64,
}

impl QuorumTracker {
    pub fn new(total_members: u64, required_quorum: u64) -> Self {
        Self {
            total_members,
            voting_members: total_members,
            required_quorum,
        }
    }

    pub fn set_total_members(&mut self, count: u64) {
        self.total_members = count;
        self.voting_members = count;
    }

    pub fn set_voting_members(&mut self, count: u64) {
        self.voting_members = count;
    }

    pub fn has_quorum(&self) -> bool {
        self.voting_members >= self.required_quorum
    }

    pub fn quorum_ratio(&self) -> f64 {
        if self.required_quorum == 0 {
            return 0.0;
        }
        self.voting_members as f64 / self.required_quorum as f64
    }

    pub fn required_quorum(&self) -> u64 {
        self.required_quorum
    }
}

///////////////////////////////////////////////////////////////////////////////
// DRAIN MANAGER
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DrainStatus {
    NotDraining,
    Draining,
    Drained,
    DrainFailed,
}

impl DrainStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            DrainStatus::NotDraining => "not_draining",
            DrainStatus::Draining => "draining",
            DrainStatus::Drained => "drained",
            DrainStatus::DrainFailed => "drain_failed",
        }
    }

    pub fn is_complete(&self) -> bool {
        matches!(self, DrainStatus::Drained | DrainStatus::DrainFailed)
    }
}

pub struct DrainManager {
    pub drain_states: std::cell::RefCell<std::collections::HashMap<NodeId, DrainStatus>>,
    pub drain_started_ms: std::cell::RefCell<std::collections::HashMap<NodeId, u64>>,
    pub drain_completed_ms: std::cell::RefCell<std::collections::HashMap<NodeId, u64>>,
}

impl DrainManager {
    pub fn new() -> Self {
        Self {
            drain_states: std::cell::RefCell::new(std::collections::HashMap::new()),
            drain_started_ms: std::cell::RefCell::new(std::collections::HashMap::new()),
            drain_completed_ms: std::cell::RefCell::new(std::collections::HashMap::new()),
        }
    }

    pub fn start_drain(&self, node_id: NodeId, started_ms: u64) {
        self.drain_states
            .borrow_mut()
            .insert(node_id, DrainStatus::Draining);
        self.drain_started_ms
            .borrow_mut()
            .insert(node_id, started_ms);
    }

    pub fn complete_drain(&self, node_id: NodeId, completed_ms: u64) {
        self.drain_states
            .borrow_mut()
            .insert(node_id, DrainStatus::Drained);
        self.drain_completed_ms
            .borrow_mut()
            .insert(node_id, completed_ms);
    }

    pub fn fail_drain(&self, node_id: NodeId) {
        self.drain_states
            .borrow_mut()
            .insert(node_id, DrainStatus::DrainFailed);
    }

    pub fn get_drain_status(&self, node_id: &NodeId) -> DrainStatus {
        self.drain_states
            .borrow()
            .get(node_id)
            .copied()
            .unwrap_or(DrainStatus::NotDraining)
    }

    pub fn drain_duration_ms(&self, node_id: &NodeId) -> Option<u64> {
        let started = self.drain_started_ms.borrow().get(node_id)?;
        let completed = self.drain_completed_ms.borrow().get(node_id)?;
        Some(completed.saturating_sub(*started))
    }

    pub fn draining_count(&self) -> usize {
        self.drain_states
            .borrow()
            .values()
            .filter(|s| matches!(s, DrainStatus::Draining))
            .count()
    }
}

impl Default for DrainManager {
    fn default() -> Self {
        Self::new()
    }
}

///////////////////////////////////////////////////////////////////////////////
// GRACEFUL LEAVE
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LeaveStatus {
    NotLeaving,
    LeaveRequested,
    LeaveInProgress,
    Left,
    LeaveFailed,
}

impl LeaveStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            LeaveStatus::NotLeaving => "not_leaving",
            LeaveStatus::LeaveRequested => "leave_requested",
            LeaveStatus::LeaveInProgress => "leave_in_progress",
            LeaveStatus::Left => "left",
            LeaveStatus::LeaveFailed => "leave_failed",
        }
    }

    pub fn is_terminal(&self) -> bool {
        matches!(self, LeaveStatus::Left | LeaveStatus::LeaveFailed)
    }
}

pub struct GracefulLeave {
    pub leave_states: std::cell::RefCell<std::collections::HashMap<NodeId, LeaveStatus>>,
    pub leave_count: u64,
    pub failed_leave_count: u64,
}

impl GracefulLeave {
    pub fn new() -> Self {
        Self {
            leave_states: std::cell::RefCell::new(std::collections::HashMap::new()),
            leave_count: 0,
            failed_leave_count: 0,
        }
    }

    pub fn request_leave(&self, node_id: NodeId) {
        self.leave_states
            .borrow_mut()
            .insert(node_id, LeaveStatus::LeaveRequested);
    }

    pub fn start_leave(&self, node_id: NodeId) {
        self.leave_states
            .borrow_mut()
            .insert(node_id, LeaveStatus::LeaveInProgress);
    }

    pub fn complete_leave(&self, node_id: NodeId) {
        self.leave_states
            .borrow_mut()
            .insert(node_id, LeaveStatus::Left);
        self.leave_count += 1;
    }

    pub fn fail_leave(&self, node_id: NodeId) {
        self.leave_states
            .borrow_mut()
            .insert(node_id, LeaveStatus::LeaveFailed);
        self.failed_leave_count += 1;
    }

    pub fn get_leave_status(&self, node_id: &NodeId) -> LeaveStatus {
        self.leave_states
            .borrow()
            .get(node_id)
            .copied()
            .unwrap_or(LeaveStatus::NotLeaving)
    }

    pub fn total_leaves(&self) -> u64 {
        self.leave_count
    }

    pub fn total_failed_leaves(&self) -> u64 {
        self.failed_leave_count
    }
}

impl Default for GracefulLeave {
    fn default() -> Self {
        Self::new()
    }
}

///////////////////////////////////////////////////////////////////////////////
// REJOIN MANAGER
///////////////////////////////////////////////////////////////////////////////

pub struct RejoinManager {
    pub rejoin_attempts: std::cell::RefCell<std::collections::HashMap<NodeId, u32>>,
    pub max_rejoin_attempts: u32,
    pub successful_rejoins: u64,
    pub failed_rejoins: u64,
}

impl RejoinManager {
    pub fn new(max_rejoin_attempts: u32) -> Self {
        Self {
            rejoin_attempts: std::cell::RefCell::new(std::collections::HashMap::new()),
            max_rejoin_attempts,
            successful_rejoins: 0,
            failed_rejoins: 0,
        }
    }

    pub fn record_attempt(&self, node_id: NodeId) {
        self.rejoin_attempts
            .borrow_mut()
            .entry(node_id)
            .and_modify(|v| *v += 1)
            .or_insert(1);
    }

    pub fn can_rejoin(&self, node_id: &NodeId) -> bool {
        self.rejoin_attempts
            .borrow()
            .get(node_id)
            .map(|v| *v < self.max_rejoin_attempts)
            .unwrap_or(true)
    }

    pub fn record_success(&self, node_id: NodeId) {
        self.rejoin_attempts.borrow_mut().remove(node_id);
        self.successful_rejoins += 1;
    }

    pub fn record_failure(&self, node_id: NodeId) {
        self.failed_rejoins += 1;
    }

    pub fn attempt_count(&self, node_id: &NodeId) -> u32 {
        self.rejoin_attempts.borrow().get(node_id).copied().unwrap_or(0)
    }

    pub fn total_successful_rejoins(&self) -> u64 {
        self.successful_rejoins
    }

    pub fn total_failed_rejoins(&self) -> u64 {
        self.failed_rejoins
    }
}

impl Default for RejoinManager {
    fn default() -> Self {
        Self::new(5)
    }
}

///////////////////////////////////////////////////////////////////////////////
// MEMBERSHIP STATISTICS
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone, Default)]
pub struct MembershipStatistics {
    pub total_joins: u64,
    pub total_leaves: u64,
    pub total_retires: u64,
    pub total_activations: u64,
    pub total_transitions: u64,
    pub total_failed_transitions: u64,
}

impl MembershipStatistics {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn record_join(&mut self) {
        self.total_joins += 1;
    }

    pub fn record_leave(&mut self) {
        self.total_leaves += 1;
    }

    pub fn record_retire(&mut self) {
        self.total_retires += 1;
    }

    pub fn record_activation(&mut self) {
        self.total_activations += 1;
    }

    pub fn record_transition(&mut self, success: bool) {
        self.total_transitions += 1;
        if !success {
            self.total_failed_transitions += 1;
        }
    }

    pub fn churn_rate(&self) -> f64 {
        let total = self.total_joins + self.total_leaves;
        if total == 0 {
            return 0.0;
        }
        self.total_leaves as f64 / total as f64
    }

    pub fn transition_success_rate(&self) -> f64 {
        if self.total_transitions == 0 {
            return 1.0;
        }
        (self.total_transitions - self.total_failed_transitions) as f64
            / self.total_transitions as f64
    }
}

///////////////////////////////////////////////////////////////////////////////
// NODE HEALTH MONITOR
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeHealthMonitorStatus {
    Healthy,
    Warning,
    Critical,
    Down,
}

impl NodeHealthMonitorStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            NodeHealthMonitorStatus::Healthy => "healthy",
            NodeHealthMonitorStatus::Warning => "warning",
            NodeHealthMonitorStatus::Critical => "critical",
            NodeHealthMonitorStatus::Down => "down",
        }
    }

    pub fn is_healthy(&self) -> bool {
        matches!(self, NodeHealthMonitorStatus::Healthy)
    }
}

pub struct NodeHealthMonitor {
    pub health_states: std::cell::RefCell<
        std::collections::HashMap<NodeId, NodeHealthMonitorStatus>,
    >,
    pub health_checks: u64,
}

impl NodeHealthMonitor {
    pub fn new() -> Self {
        Self {
            health_states: std::cell::RefCell::new(std::collections::HashMap::new()),
            health_checks: 0,
        }
    }

    pub fn set_health(&self, node_id: NodeId, status: NodeHealthMonitorStatus) {
        self.health_states
            .borrow_mut()
            .insert(node_id, status);
        self.health_checks += 1;
    }

    pub fn get_health(&self, node_id: &NodeId) -> NodeHealthMonitorStatus {
        self.health_states
            .borrow()
            .get(node_id)
            .copied()
            .unwrap_or(NodeHealthMonitorStatus::Healthy)
    }

    pub fn healthy_count(&self) -> usize {
        self.health_states
            .borrow()
            .values()
            .filter(|s| matches!(s, NodeHealthMonitorStatus::Healthy))
            .count()
    }

    pub fn total_checks(&self) -> u64 {
        self.health_checks
    }

    pub fn tracked_count(&self) -> usize {
        self.health_states.borrow().len()
    }
}

impl Default for NodeHealthMonitor {
    fn default() -> Self {
        Self::new()
    }
}

///////////////////////////////////////////////////////////////////////////////
// MEMBERSHIP PROTOCOL
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MembershipProtocolVersion {
    V1,
    V2,
}

impl MembershipProtocolVersion {
    pub fn as_str(&self) -> &'static str {
        match self {
            MembershipProtocolVersion::V1 => "membership_v1",
            MembershipProtocolVersion::V2 => "membership_v2",
        }
    }

    pub fn header_size(&self) -> usize {
        match self {
            MembershipProtocolVersion::V1 => 4,
            MembershipProtocolVersion::V2 => 8,
        }
    }
}

pub struct MembershipProtocol {
    pub version: MembershipProtocolVersion,
    pub encoded_messages: u64,
    pub decoded_messages: u64,
}

impl MembershipProtocol {
    pub fn new(version: MembershipProtocolVersion) -> Self {
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

impl Default for MembershipProtocol {
    fn default() -> Self {
        Self::new(MembershipProtocolVersion::V2)
    }
}

///////////////////////////////////////////////////////////////////////////////
// MEMBERSHIP WIRE FORMAT
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MembershipWireVersion {
    V1,
    V2,
}

impl MembershipWireVersion {
    pub fn as_str(&self) -> &'static str {
        match self {
            MembershipWireVersion::V1 => "membership_wire_v1",
            MembershipWireVersion::V2 => "membership_wire_v2",
        }
    }

    pub fn header_size(&self) -> usize {
        match self {
            MembershipWireVersion::V1 => 6,
            MembershipWireVersion::V2 => 12,
        }
    }
}

pub struct MembershipWireFormat {
    pub version: MembershipWireVersion,
    pub encoded_messages: u64,
    pub decoded_messages: u64,
}

impl MembershipWireFormat {
    pub fn new(version: MembershipWireVersion) -> Self {
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

impl Default for MembershipWireFormat {
    fn default() -> Self {
        Self::new(MembershipWireVersion::V2)
    }
}

///////////////////////////////////////////////////////////////////////////////
// MEMBERSHIP FRAME
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MembershipFrame {
    pub frame_type: u8,
    pub payload: Vec<u8>,
    pub sequence: u64,
    pub checksum: u32,
}

impl MembershipFrame {
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

    pub fn is_join(&self) -> bool {
        self.frame_type == 1
    }

    pub fn is_leave(&self) -> bool {
        self.frame_type == 2
    }
}

///////////////////////////////////////////////////////////////////////////////
// MEMBERSHIP ACK
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MembershipAckState {
    Pending,
    AckReceived,
    NackReceived,
    Timeout,
}

impl MembershipAckState {
    pub fn as_str(&self) -> &'static str {
        match self {
            MembershipAckState::Pending => "pending",
            MembershipAckState::AckReceived => "ack_received",
            MembershipAckState::NackReceived => "nack_received",
            MembershipAckState::Timeout => "timeout",
        }
    }

    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            MembershipAckState::AckReceived
                | MembershipAckState::NackReceived
                | MembershipAckState::Timeout
        )
    }
}

pub struct MembershipAck {
    pub ack_states: std::cell::RefCell<std::collections::HashMap<u64, MembershipAckState>>,
    pub ack_count: u64,
    pub nack_count: u64,
    pub timeout_count: u64,
}

impl MembershipAck {
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
            .insert(seq, MembershipAckState::AckReceived);
        self.ack_count += 1;
    }

    pub fn record_nack(&self, seq: u64) {
        self.ack_states
            .borrow_mut()
            .insert(seq, MembershipAckState::NackReceived);
        self.nack_count += 1;
    }

    pub fn record_timeout(&self, seq: u64) {
        self.ack_states
            .borrow_mut()
            .insert(seq, MembershipAckState::Timeout);
        self.timeout_count += 1;
    }

    pub fn get_state(&self, seq: u64) -> Option<MembershipAckState> {
        self.ack_states.borrow().get(&seq).copied()
    }

    pub fn is_acked(&self, seq: u64) -> bool {
        matches!(
            self.get_state(seq),
            Some(MembershipAckState::AckReceived)
        )
    }

    pub fn pending_count(&self) -> usize {
        self.ack_states
            .borrow()
            .values()
            .filter(|s| matches!(s, MembershipAckState::Pending))
            .count()
    }

    pub fn total_tracked(&self) -> usize {
        self.ack_states.borrow().len()
    }
}

impl Default for MembershipAck {
    fn default() -> Self {
        Self::new()
    }
}

///////////////////////////////////////////////////////////////////////////////
// MEMBERSHIP SEQUENCING
///////////////////////////////////////////////////////////////////////////////

pub struct MembershipSequencing {
    pub next_sequence: std::cell::Cell<u64>,
    pub acknowledged: std::cell::RefCell<std::collections::HashSet<u64>>,
    pub total_sequences: u64,
}

impl MembershipSequencing {
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

impl Default for MembershipSequencing {
    fn default() -> Self {
        Self::new()
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

    // === MembershipError tests ===
    #[test]
    fn membership_error_category() {
        assert_eq!(
            MembershipError::NodeNotFound {
                node_id: test_node_id(),
            }
            .category(),
            "node_not_found"
        );
        assert_eq!(
            MembershipError::HUnitNotFound {
                hunit_id: test_hunit_id(),
            }
            .category(),
            "hunit_not_found"
        );
        assert_eq!(
            MembershipError::AlreadyMember {
                node_id: test_node_id(),
            }
            .category(),
            "already_member"
        );
        assert_eq!(
            MembershipError::NotMember {
                node_id: test_node_id(),
            }
            .category(),
            "not_member"
        );
        assert_eq!(
            MembershipError::InvalidTransition {
                from: NodeLifecycleState::Candidate,
                to: NodeLifecycleState::Active,
            }
            .category(),
            "invalid_transition"
        );
        assert_eq!(
            MembershipError::MembershipTableFull { max: 100 }.category(),
            "table_full"
        );
        assert_eq!(
            MembershipError::QuorumNotMet.category(),
            "quorum_not_met"
        );
        assert_eq!(
            MembershipError::Internal {
                message: "panic".to_string(),
            }
            .category(),
            "internal"
        );
    }

    #[test]
    fn membership_error_is_recoverable() {
        assert!(MembershipError::LeaveInProgress {
            node_id: test_node_id(),
        }
        .is_recoverable());
        assert!(MembershipError::RetireInProgress {
            node_id: test_node_id(),
        }
        .is_recoverable());
        assert!(MembershipError::DrainTimeout {
            node_id: test_node_id(),
        }
        .is_recoverable());
        assert!(!MembershipError::NodeNotFound {
            node_id: test_node_id(),
        }
        .is_recoverable());
    }

    #[test]
    fn membership_error_suggested_action() {
        assert_eq!(
            MembershipError::NodeNotFound {
                node_id: test_node_id(),
            }
            .suggested_action(),
            "re-join node to network"
        );
        assert_eq!(
            MembershipError::QuorumNotMet.suggested_action(),
            "wait for quorum"
        );
        assert_eq!(
            MembershipError::DrainTimeout {
                node_id: test_node_id(),
            }
            .suggested_action(),
            "force drain or extend timeout"
        );
        assert_eq!(
            MembershipError::AlreadyMember {
                node_id: test_node_id(),
            }
            .suggested_action(),
            "use existing membership"
        );
        assert_eq!(
            MembershipError::NotMember {
                node_id: test_node_id(),
            }
            .suggested_action(),
            "join network first"
        );
        assert_eq!(
            MembershipError::InvalidTransition {
                from: NodeLifecycleState::Candidate,
                to: NodeLifecycleState::Active,
            }
            .suggested_action(),
            "follow lifecycle ordering"
        );
    }

    // === NodeLifecycleState tests ===
    #[test]
    fn node_lifecycle_state_transitions() {
        assert!(NodeLifecycleState::Candidate.can_transition_to(NodeLifecycleState::Joining));
        assert!(!NodeLifecycleState::Candidate.can_transition_to(NodeLifecycleState::Active));
        assert!(NodeLifecycleState::Joining.can_transition_to(NodeLifecycleState::Active));
        assert!(NodeLifecycleState::Active.can_transition_to(NodeLifecycleState::Leaving));
        assert!(NodeLifecycleState::Leaving.can_transition_to(NodeLifecycleState::Retired));
        assert!(NodeLifecycleState::Retired.can_transition_to(NodeLifecycleState::Candidate));
        assert!(NodeLifecycleState::Failed.can_transition_to(NodeLifecycleState::Candidate));
    }

    #[test]
    fn node_lifecycle_state_terminal_active() {
        assert!(NodeLifecycleState::Retired.is_terminal());
        assert!(NodeLifecycleState::Failed.is_terminal());
        assert!(!NodeLifecycleState::Active.is_terminal());
        assert!(NodeLifecycleState::Active.is_active());
        assert!(!NodeLifecycleState::Candidate.is_active());
    }

    #[test]
    fn node_lifecycle_state_str() {
        assert_eq!(NodeLifecycleState::Candidate.as_str(), "candidate");
        assert_eq!(NodeLifecycleState::Joining.as_str(), "joining");
        assert_eq!(NodeLifecycleState::Active.as_str(), "active");
        assert_eq!(NodeLifecycleState::Leaving.as_str(), "leaving");
        assert_eq!(NodeLifecycleState::Retired.as_str(), "retired");
        assert_eq!(NodeLifecycleState::Failed.as_str(), "failed");
    }

    // === HUnitLifecycleState tests ===
    #[test]
    fn hunit_lifecycle_state_transitions() {
        assert!(HUnitLifecycleState::Pending.can_transition_to(HUnitLifecycleState::Active));
        assert!(!HUnitLifecycleState::Pending.can_transition_to(HUnitLifecycleState::Retired));
        assert!(HUnitLifecycleState::Active.can_transition_to(HUnitLifecycleState::Draining));
        assert!(HUnitLifecycleState::Draining.can_transition_to(HUnitLifecycleState::Retired));
        assert!(HUnitLifecycleState::Retired.can_transition_to(HUnitLifecycleState::Pending));
        assert!(HUnitLifecycleState::Failed.can_transition_to(HUnitLifecycleState::Pending));
    }

    #[test]
    fn hunit_lifecycle_state_terminal_active() {
        assert!(HUnitLifecycleState::Retired.is_terminal());
        assert!(HUnitLifecycleState::Failed.is_terminal());
        assert!(!HUnitLifecycleState::Active.is_terminal());
        assert!(HUnitLifecycleState::Active.is_active());
        assert!(!HUnitLifecycleState::Pending.is_active());
    }

    #[test]
    fn hunit_lifecycle_state_str() {
        assert_eq!(HUnitLifecycleState::Pending.as_str(), "pending");
        assert_eq!(HUnitLifecycleState::Active.as_str(), "active");
        assert_eq!(HUnitLifecycleState::Draining.as_str(), "draining");
        assert_eq!(HUnitLifecycleState::Retired.as_str(), "retired");
        assert_eq!(HUnitLifecycleState::Failed.as_str(), "failed");
    }

    // === NodeInfo tests ===
    #[test]
    fn node_info_new_and_fields() {
        let ni = test_node_info();
        assert_eq!(ni.node_id, test_node_id());
        assert_eq!(ni.endpoint, test_endpoint());
        assert_eq!(ni.region_id, test_region_id());
        assert_eq!(ni.state, NodeLifecycleState::Candidate);
        assert_eq!(ni.joined_at_ms, 0);
        assert_eq!(ni.last_heartbeat_ms, 0);
    }

    #[test]
    fn node_info_with_state_joined_heartbeat() {
        let ni = test_node_info()
            .with_state(NodeLifecycleState::Active)
            .with_joined_at(1000)
            .with_heartbeat(2000);
        assert_eq!(ni.state, NodeLifecycleState::Active);
        assert_eq!(ni.joined_at_ms, 1000);
        assert_eq!(ni.last_heartbeat_ms, 2000);
    }

    #[test]
    fn node_info_with_capabilities_metadata() {
        let ni = test_node_info()
            .with_capabilities(vec!["gossip".to_string()])
            .with_metadata("rack".to_string(), "r1".to_string());
        assert!(ni.capabilities.contains(&"gossip".to_string()));
        assert_eq!(ni.metadata.len(), 1);
        assert_eq!(ni.metadata[0].0, "rack");
        assert_eq!(ni.metadata[0].1, "r1");
    }

    #[test]
    fn node_info_is_stale_uptime() {
        let ni = test_node_info().with_heartbeat(1000);
        assert!(!ni.is_stale(2000, 300000));
        assert!(ni.is_stale(400000, 300000));
        let ni = ni.with_joined_at(1000);
        assert_eq!(ni.uptime_ms(5000), 4000);
    }

    // === HUnitInfo tests ===
    #[test]
    fn hunit_info_new_and_fields() {
        let hi = test_hunit_info();
        assert_eq!(hi.hunit_id, test_hunit_id());
        assert_eq!(hi.node_id, test_node_id());
        assert_eq!(hi.region_id, test_region_id());
        assert_eq!(hi.state, HUnitLifecycleState::Pending);
        assert!(hi.datasets.is_empty());
    }

    #[test]
    fn hunit_info_with_datasets_activated() {
        let hi = test_hunit_info()
            .with_datasets(vec![test_dataset_id()])
            .with_activated_at(5000);
        assert!(hi.hosts_dataset(&test_dataset_id()));
        assert_eq!(hi.activated_at_ms, 5000);
    }

    #[test]
    fn hunit_info_retire() {
        let hi = test_hunit_info().retire(6000);
        assert!(hi.is_retired());
        assert_eq!(hi.retired_at_ms, Some(6000));
    }

    // === MembershipConfig tests ===
    #[test]
    fn membership_config_defaults() {
        let config = MembershipConfig::default();
        assert_eq!(config.max_members, 10000);
        assert_eq!(config.max_hunits, 100000);
        assert_eq!(config.join_timeout_ms, 30000);
        assert_eq!(config.leave_timeout_ms, 60000);
        assert_eq!(config.drain_timeout_ms, 120000);
        assert_eq!(config.heartbeat_threshold_ms, 300000);
        assert!(config.quorum_required);
    }

    #[test]
    fn membership_config_with_methods() {
        let config = MembershipConfig::default()
            .with_max_members(5000)
            .with_join_timeout(10000)
            .with_drain_timeout(30000)
            .with_quorum_required(false);
        assert_eq!(config.max_members, 5000);
        assert_eq!(config.join_timeout_ms, 10000);
        assert_eq!(config.drain_timeout_ms, 30000);
        assert!(!config.quorum_required);
    }

    // === MembershipEngine tests ===
    #[test]
    fn membership_engine_join_and_leave() {
        let engine = MembershipEngine::default();
        engine.join_node(test_node_info()).unwrap();
        assert!(engine.is_member(&test_node_id()));
        assert_eq!(engine.member_count(), 1);
        engine.activate_node(&test_node_id()).unwrap();
        engine.leave_node(&test_node_id()).unwrap();
        assert_eq!(engine.leave_count(), 1);
    }

    #[test]
    fn membership_engine_join_duplicate() {
        let engine = MembershipEngine::default();
        engine.join_node(test_node_info()).unwrap();
        let err = engine.join_node(test_node_info()).unwrap_err();
        assert!(matches!(err, MembershipError::AlreadyMember { .. }));
    }

    #[test]
    fn membership_engine_leave_not_member() {
        let engine = MembershipEngine::default();
        let err = engine.leave_node(&test_node_id()).unwrap_err();
        assert!(matches!(err, MembershipError::NodeNotFound { .. }));
    }

    #[test]
    fn membership_engine_retire_node() {
        let engine = MembershipEngine::default();
        engine.join_node(test_node_info()).unwrap();
        engine.activate_node(&test_node_id()).unwrap();
        engine.retire_node(&test_node_id()).unwrap();
        assert_eq!(
            engine.node_state(&test_node_id()).unwrap(),
            NodeLifecycleState::Retired
        );
        assert_eq!(engine.retire_count(), 1);
    }

    #[test]
    fn membership_engine_fail_node() {
        let engine = MembershipEngine::default();
        engine.join_node(test_node_info()).unwrap();
        engine.fail_node(&test_node_id()).unwrap();
        assert_eq!(
            engine.node_state(&test_node_id()).unwrap(),
            NodeLifecycleState::Failed
        );
    }

    #[test]
    fn membership_engine_activate_hunit() {
        let engine = MembershipEngine::default();
        engine.activate_hunit(test_hunit_info()).unwrap();
        assert_eq!(
            engine.hunit_state(&test_hunit_id()).unwrap(),
            HUnitLifecycleState::Active
        );
        assert_eq!(engine.activate_count(), 1);
    }

    #[test]
    fn membership_engine_retire_hunit() {
        let engine = MembershipEngine::default();
        engine.activate_hunit(test_hunit_info()).unwrap();
        engine.retire_hunit(&test_hunit_id()).unwrap();
        assert_eq!(
            engine.hunit_state(&test_hunit_id()).unwrap(),
            HUnitLifecycleState::Retired
        );
    }

    #[test]
    fn membership_engine_nodes_in_state() {
        let engine = MembershipEngine::default();
        engine.join_node(test_node_info()).unwrap();
        engine.activate_node(&test_node_id()).unwrap();
        let active = engine.nodes_in_state(NodeLifecycleState::Active).unwrap();
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].node_id, test_node_id());
    }

    #[test]
    fn membership_engine_hunits_in_state() {
        let engine = MembershipEngine::default();
        engine.activate_hunit(test_hunit_info()).unwrap();
        let active = engine.hunits_in_state(HUnitLifecycleState::Active).unwrap();
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].hunit_id, test_hunit_id());
    }

    #[test]
    fn membership_engine_counts() {
        let engine = MembershipEngine::default();
        assert_eq!(engine.join_count(), 0);
        assert_eq!(engine.leave_count(), 0);
        assert_eq!(engine.retire_count(), 0);
        assert_eq!(engine.active_member_count(), 0);
    }

    #[test]
    fn membership_engine_active_member_count() {
        let engine = MembershipEngine::default();
        engine.join_node(test_node_info()).unwrap();
        engine.activate_node(&test_node_id()).unwrap();
        assert_eq!(engine.active_member_count(), 1);
    }

    // === NodeLifecycle tests ===
    #[test]
    fn node_lifecycle_set_and_get() {
        let nl = NodeLifecycle::default();
        nl.set_state(test_node_id(), NodeLifecycleState::Active);
        assert_eq!(nl.get_state(&test_node_id()), NodeLifecycleState::Active);
        assert_eq!(nl.tracked_count(), 1);
    }

    #[test]
    fn node_lifecycle_can_transition() {
        let nl = NodeLifecycle::default();
        nl.set_state(test_node_id(), NodeLifecycleState::Candidate);
        assert!(nl.can_transition(&test_node_id(), NodeLifecycleState::Joining));
        assert!(!nl.can_transition(&test_node_id(), NodeLifecycleState::Active));
    }

    #[test]
    fn node_lifecycle_transition_success_failure() {
        let nl = NodeLifecycle::default();
        nl.transition(test_node_id(), NodeLifecycleState::Joining).unwrap();
        assert!(nl
            .transition(test_node_id(), NodeLifecycleState::Active)
            .is_ok());
        let err = nl
            .transition(test_node_id(), NodeLifecycleState::Retired)
            .unwrap_err();
        assert!(matches!(err, MembershipError::InvalidTransition { .. }));
        assert_eq!(nl.failed_transition_count(), 1);
    }

    // === HUnitLifecycle tests ===
    #[test]
    fn hunit_lifecycle_set_and_get() {
        let hl = HUnitLifecycle::default();
        hl.set_state(test_hunit_id(), HUnitLifecycleState::Active);
        assert_eq!(
            hl.get_state(&test_hunit_id()),
            HUnitLifecycleState::Active
        );
        assert_eq!(hl.tracked_count(), 1);
    }

    #[test]
    fn hunit_lifecycle_transition_success_failure() {
        let hl = HUnitLifecycle::default();
        hl.transition(test_hunit_id(), HUnitLifecycleState::Active).unwrap();
        let err = hl
            .transition(test_hunit_id(), HUnitLifecycleState::Retired)
            .unwrap_err();
        assert!(matches!(
            err,
            MembershipError::InvalidHUnitTransition { .. }
        ));
        assert_eq!(hl.failed_transition_count(), 1);
    }

    // === MembershipManager tests ===
    #[test]
    fn membership_manager_join_leave_round() {
        let mm = MembershipManager::default();
        mm.join_node(test_node_info()).unwrap();
        assert!(mm.is_member(&test_node_id()));
        assert_eq!(mm.member_count(), 1);
        assert!(mm.membership_round().is_ok());
        assert_eq!(mm.membership_rounds(), 1);
    }

    #[test]
    fn membership_manager_round_quorum_not_met() {
        let mm = MembershipManager::default();
        let err = mm.membership_round().unwrap_err();
        assert!(matches!(err, MembershipError::QuorumNotMet));
    }

    #[test]
    fn membership_manager_activate_retire_hunit() {
        let mm = MembershipManager::default();
        mm.activate_hunit(test_hunit_info()).unwrap();
        mm.retire_hunit(&test_hunit_id()).unwrap();
        assert_eq!(
            mm.engine.borrow().hunit_state(&test_hunit_id()).unwrap(),
            HUnitLifecycleState::Retired
        );
    }

    // === MembershipSummary tests ===
    #[test]
    fn membership_summary_member_ratio() {
        let summary = MembershipSummary {
            total_members: 10,
            active_members: 7,
            total_hunits: 50,
            active_hunits: 45,
            join_count: 100,
            leave_count: 3,
        };
        assert!((summary.member_ratio() - 0.7).abs() < 0.01);
    }

    #[test]
    fn membership_summary_default_ratio() {
        let summary = MembershipSummary::new();
        assert_eq!(summary.member_ratio(), 0.0);
    }

    // === MembershipCache tests ===
    #[test]
    fn membership_cache_node_state() {
        let cache = MembershipCache::default();
        cache.cache_node_state(test_node_id(), NodeLifecycleState::Active);
        assert_eq!(
            cache.get_cached_node_state(&test_node_id()),
            Some(NodeLifecycleState::Active)
        );
    }

    #[test]
    fn membership_cache_node_state_miss() {
        let cache = MembershipCache::default();
        assert_eq!(cache.get_cached_node_state(&test_node_id()), None);
        assert_eq!(cache.misses, 1);
    }

    #[test]
    fn membership_cache_hunit_state() {
        let cache = MembershipCache::default();
        cache.cache_hunit_state(test_hunit_id(), HUnitLifecycleState::Active);
        assert_eq!(
            cache.get_cached_hunit_state(&test_hunit_id()),
            Some(HUnitLifecycleState::Active)
        );
    }

    #[test]
    fn membership_cache_hit_rate() {
        let cache = MembershipCache::default();
        cache.cache_node_state(test_node_id(), NodeLifecycleState::Active);
        cache.get_cached_node_state(&test_node_id());
        cache.get_cached_node_state(&NodeId::new([2; 32]));
        assert_eq!(cache.hit_rate(), 0.5);
    }

    #[test]
    fn membership_cache_clear() {
        let cache = MembershipCache::default();
        cache.cache_node_state(test_node_id(), NodeLifecycleState::Active);
        cache.clear();
        assert_eq!(cache.entry_count(), 0);
    }

    // === MembershipExpiration tests ===
    #[test]
    fn membership_expiration_set_and_status() {
        let exp = MembershipExpiration::default();
        exp.set_ttl(test_node_id(), 5000);
        assert_eq!(
            exp.get_status(&test_node_id(), 1000),
            MembershipEntryStatus::Fresh
        );
        assert_eq!(
            exp.get_status(&test_node_id(), 6000),
            MembershipEntryStatus::Expired
        );
    }

    #[test]
    fn membership_expiration_mark_expired_and_fresh() {
        let exp = MembershipExpiration::default();
        exp.mark_expired(&test_node_id());
        assert_eq!(
            exp.get_status(&test_node_id(), 1000),
            MembershipEntryStatus::Expired
        );
        assert_eq!(exp.expired_count(), 1);
        exp.mark_fresh(test_node_id(), 5000);
        assert_eq!(
            exp.get_status(&test_node_id(), 1000),
            MembershipEntryStatus::Fresh
        );
    }

    #[test]
    fn membership_expiration_default_ttl() {
        let exp = MembershipExpiration::default();
        assert_eq!(exp.default_ttl_ms(), 300000);
    }

    // === MembershipValidator tests ===
    #[test]
    fn membership_validator_validate_node_info() {
        let mut validator = MembershipValidator::new();
        let valid_node = test_node_info();
        assert!(validator.validate_node_info(&valid_node));
        assert_eq!(validator.validations, 1);
    }

    #[test]
    fn membership_validator_validate_transition() {
        let mut validator = MembershipValidator::new();
        assert!(validator.validate_transition(
            NodeLifecycleState::Candidate,
            NodeLifecycleState::Joining
        ));
        assert!(!validator.validate_transition(
            NodeLifecycleState::Candidate,
            NodeLifecycleState::Active
        ));
        assert_eq!(validator.invalid_transitions, 1);
    }

    #[test]
    fn membership_validator_pass_rate() {
        let mut validator = MembershipValidator::new();
        validator.validate_transition(
            NodeLifecycleState::Candidate,
            NodeLifecycleState::Active
        );
        validator.validate_transition(
            NodeLifecycleState::Candidate,
            NodeLifecycleState::Joining
        );
        assert!((validator.pass_rate() - 0.5).abs() < 0.01);
    }

    // === QuorumTracker tests ===
    #[test]
    fn quorum_tracker_has_quorum() {
        let mut qt = QuorumTracker::new(10, 6);
        assert!(qt.has_quorum());
        qt.set_voting_members(5);
        assert!(!qt.has_quorum());
    }

    #[test]
    fn quorum_tracker_ratio() {
        let qt = QuorumTracker::new(10, 6);
        assert!((qt.quorum_ratio() - 1.0).abs() < 0.01);
        let qt2 = QuorumTracker::new(10, 3);
        assert!((qt2.quorum_ratio() - (10.0 / 3.0)).abs() < 0.01);
    }

    // === DrainManager tests ===
    #[test]
    fn drain_manager_start_complete() {
        let dm = DrainManager::default();
        dm.start_drain(test_node_id(), 1000);
        assert_eq!(
            dm.get_drain_status(&test_node_id()),
            DrainStatus::Draining
        );
        dm.complete_drain(test_node_id(), 5000);
        assert_eq!(
            dm.get_drain_status(&test_node_id()),
            DrainStatus::Drained
        );
        assert_eq!(dm.drain_duration_ms(&test_node_id()), Some(4000));
    }

    #[test]
    fn drain_manager_fail() {
        let dm = DrainManager::default();
        dm.start_drain(test_node_id(), 1000);
        dm.fail_drain(test_node_id());
        assert_eq!(
            dm.get_drain_status(&test_node_id()),
            DrainStatus::DrainFailed
        );
        assert_eq!(dm.drain_duration_ms(&test_node_id()), None);
    }

    #[test]
    fn drain_manager_draining_count() {
        let dm = DrainManager::default();
        dm.start_drain(test_node_id(), 1000);
        dm.start_drain(NodeId::new([2; 32]), 1000);
        assert_eq!(dm.draining_count(), 2);
    }

    // === GracefulLeave tests ===
    #[test]
    fn graceful_leave_request_start_complete() {
        let gl = GracefulLeave::default();
        gl.request_leave(test_node_id());
        assert_eq!(
            gl.get_leave_status(&test_node_id()),
            LeaveStatus::LeaveRequested
        );
        gl.start_leave(test_node_id());
        assert_eq!(
            gl.get_leave_status(&test_node_id()),
            LeaveStatus::LeaveInProgress
        );
        gl.complete_leave(test_node_id());
        assert_eq!(gl.get_leave_status(&test_node_id()), LeaveStatus::Left);
        assert_eq!(gl.total_leaves(), 1);
    }

    #[test]
    fn graceful_leave_fail() {
        let gl = GracefulLeave::default();
        gl.request_leave(test_node_id());
        gl.fail_leave(test_node_id());
        assert_eq!(
            gl.get_leave_status(&test_node_id()),
            LeaveStatus::LeaveFailed
        );
        assert_eq!(gl.total_failed_leaves(), 1);
    }

    #[test]
    fn leave_status_terminal() {
        assert!(LeaveStatus::Left.is_terminal());
        assert!(LeaveStatus::LeaveFailed.is_terminal());
        assert!(!LeaveStatus::Draining.is_terminal());
        assert!(!LeaveStatus::NotLeaving.is_terminal());
    }

    // === RejoinManager tests ===
    #[test]
    fn rejoin_manager_attempts() {
        let rm = RejoinManager::new(3);
        assert!(rm.can_rejoin(&test_node_id()));
        rm.record_attempt(test_node_id());
        rm.record_attempt(test_node_id());
        assert!(rm.can_rejoin(&test_node_id()));
        rm.record_attempt(test_node_id());
        assert!(!rm.can_rejoin(&test_node_id()));
        assert_eq!(rm.attempt_count(&test_node_id()), 3);
    }

    #[test]
    fn rejoin_manager_success_failure() {
        let rm = RejoinManager::new(5);
        rm.record_attempt(test_node_id());
        rm.record_success(test_node_id());
        assert_eq!(rm.total_successful_rejoins(), 1);
        rm.record_failure(test_node_id());
        assert_eq!(rm.total_failed_rejoins(), 1);
    }

    // === MembershipStatistics tests ===
    #[test]
    fn membership_statistics_record_and_rates() {
        let mut stats = MembershipStatistics::default();
        stats.record_join();
        stats.record_leave();
        stats.record_retire();
        stats.record_activation();
        stats.record_transition(true);
        stats.record_transition(false);
        assert_eq!(stats.total_joins, 1);
        assert_eq!(stats.total_leaves, 1);
        assert_eq!(stats.total_retires, 1);
        assert_eq!(stats.total_activations, 1);
        assert_eq!(stats.total_transitions, 2);
        assert_eq!(stats.total_failed_transitions, 1);
        assert!((stats.churn_rate() - 0.5).abs() < 0.01);
        assert!((stats.transition_success_rate() - 0.5).abs() < 0.01);
    }

    // === NodeHealthMonitor tests ===
    #[test]
    fn node_health_monitor_set_and_get() {
        let nhm = NodeHealthMonitor::default();
        nhm.set_health(test_node_id(), NodeHealthMonitorStatus::Critical);
        assert_eq!(
            nhm.get_health(&test_node_id()),
            NodeHealthMonitorStatus::Critical
        );
        assert_eq!(nhm.healthy_count(), 0);
        assert_eq!(nhm.total_checks(), 1);
    }

    #[test]
    fn node_health_monitor_healthy_count() {
        let nhm = NodeHealthMonitor::default();
        nhm.set_health(test_node_id(), NodeHealthMonitorStatus::Healthy);
        nhm.set_health(NodeId::new([2; 32]), NodeHealthMonitorStatus::Down);
        assert_eq!(nhm.healthy_count(), 1);
    }

    #[test]
    fn node_health_monitor_status_str() {
        assert_eq!(NodeHealthMonitorStatus::Healthy.as_str(), "healthy");
        assert_eq!(NodeHealthMonitorStatus::Warning.as_str(), "warning");
        assert_eq!(NodeHealthMonitorStatus::Critical.as_str(), "critical");
        assert_eq!(NodeHealthMonitorStatus::Down.as_str(), "down");
        assert!(NodeHealthMonitorStatus::Healthy.is_healthy());
        assert!(!NodeHealthMonitorStatus::Down.is_healthy());
    }

    // === MembershipProtocol tests ===
    #[test]
    fn membership_protocol_version_and_header() {
        let mp = MembershipProtocol::new(MembershipProtocolVersion::V1);
        assert_eq!(mp.header_size(), 4);
        assert_eq!(mp.version.as_str(), "membership_v1");
        let mp2 = MembershipProtocol::new(MembershipProtocolVersion::V2);
        assert_eq!(mp2.header_size(), 8);
        assert_eq!(mp2.version.as_str(), "membership_v2");
    }

    #[test]
    fn membership_protocol_encoded_decoded() {
        let mut mp = MembershipProtocol::default();
        mp.record_encoded();
        mp.record_decoded();
        mp.record_encoded();
        assert_eq!(mp.total_processed(), 3);
    }

    // === MembershipWireFormat tests ===
    #[test]
    fn membership_wire_format_version_and_header() {
        let mwf = MembershipWireFormat::new(MembershipWireVersion::V1);
        assert_eq!(mwf.header_size(), 6);
        assert_eq!(mwf.version.as_str(), "membership_wire_v1");
        let mwf2 = MembershipWireFormat::new(MembershipWireVersion::V2);
        assert_eq!(mwf2.header_size(), 12);
        assert_eq!(mwf2.version.as_str(), "membership_wire_v2");
    }

    #[test]
    fn membership_wire_format_encoded_decoded() {
        let mut mwf = MembershipWireFormat::default();
        mwf.record_encoded();
        mwf.record_decoded();
        assert_eq!(mwf.total_processed(), 2);
    }

    // === MembershipFrame tests ===
    #[test]
    fn membership_frame_new_and_verify() {
        let frame = MembershipFrame::new(1, vec![1, 2, 3], 42);
        assert_eq!(frame.frame_type, 1);
        assert_eq!(frame.payload, vec![1, 2, 3]);
        assert_eq!(frame.sequence, 42);
        assert!(frame.verify_checksum());
        assert!(frame.is_join());
        assert!(!frame.is_leave());
    }

    #[test]
    fn membership_frame_leave() {
        let frame = MembershipFrame::new(2, vec![], 0);
        assert!(frame.is_leave());
        assert!(!frame.is_join());
        assert!(frame.verify_checksum());
    }

    // === MembershipAck tests ===
    #[test]
    fn membership_ack_record_and_state() {
        let ack = MembershipAck::default();
        ack.record_ack(1);
        ack.record_nack(2);
        ack.record_timeout(3);
        assert!(ack.is_acked(1));
        assert_eq!(ack.get_state(2), Some(MembershipAckState::NackReceived));
        assert_eq!(ack.get_state(3), Some(MembershipAckState::Timeout));
        assert_eq!(ack.ack_count, 1);
        assert_eq!(ack.nack_count, 1);
        assert_eq!(ack.timeout_count, 1);
        assert_eq!(ack.total_tracked(), 3);
    }

    #[test]
    fn membership_ack_state_str_and_terminal() {
        assert_eq!(MembershipAckState::Pending.as_str(), "pending");
        assert_eq!(MembershipAckState::AckReceived.as_str(), "ack_received");
        assert_eq!(MembershipAckState::NackReceived.as_str(), "nack_received");
        assert_eq!(MembershipAckState::Timeout.as_str(), "timeout");
        assert!(!MembershipAckState::Pending.is_terminal());
        assert!(MembershipAckState::AckReceived.is_terminal());
        assert!(MembershipAckState::Timeout.is_terminal());
    }

    // === MembershipSequencing tests ===
    #[test]
    fn membership_sequencing_next_and_ack() {
        let seq = MembershipSequencing::default();
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
}
