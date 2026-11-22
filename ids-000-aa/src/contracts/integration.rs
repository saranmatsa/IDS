//! # ids-000-aa Sealed Trait Contract: Integration
//!
//! Cross-contract integration for IDS.
//! Provides the central integration point that ties together all sealed trait
//! contracts: Carrier, Transport, Store, Session, Routing, Discovery, Membership,
//! Placement, Replication, Recovery, Consistency, Ordering, Security, Client,
//! FaultTolerance, Broadcast, Quorum, Snapshot, Byzantine, Lease, CRDT, Erasure,
//! Saga, Coordinator, Durability, and Topology.
//!
//! Carrier-agnostic: no std::net, tokio::net, libp2p, IP, TCP, UDP, QUIC,
//! HTTP, DNS, or any existing networking stack. Integration messages are opaque
//! byte blobs carried on CarrierType + Vec<u8> Endpoints only.

use crate::types::{
    CheckpointId, DatasetId, Epoch, Endpoint, FragmentId, HUnitId, IdsTime, KeyId, LeaseId,
    MessageId, NodeId, OpRef, OrderingDomain, OrderingPos, RegionId, SagaId, SessionId,
    StepId, TopologyError as _, TopologyEvent, TopologyNode, TopologyPath, TopologyRegion,
    TopologyVersion, TransactionId, Version,
};
use std::collections::{HashMap, HashSet};

///////////////////////////////////////////////////////////////////////////////
// SEALED TRAIT: Integration
///////////////////////////////////////////////////////////////////////////////

/// Sealed marker — only ids-000-aa and designated crates can implement.
pub mod private {
    pub trait Sealed {}
}

/// Sealed integration management trait.
/// Implementations live downstream in ids-integration and ids-* crates.
pub trait Integration: private::Sealed {
    /// Configure integration with all contract implementations.
    fn configure_integration(
        &self,
        config: IntegrationConfig,
    ) -> Result<(), IntegrationError>;

    /// Register a carrier implementation.
    fn register_carrier(
        &self,
        carrier_id: NodeId,
        endpoint: Endpoint,
    ) -> Result<IntegrationRegistration, IntegrationError>;

    /// Register a transport implementation.
    fn register_transport(
        &self,
        node_id: NodeId,
    ) -> Result<IntegrationRegistration, IntegrationError>;

    /// Register a store implementation.
    fn register_store(
        &self,
        node_id: NodeId,
    ) -> Result<IntegrationRegistration, IntegrationError>;

    /// Register a session implementation.
    fn register_session(
        &self,
        node_id: NodeId,
    ) -> Result<IntegrationRegistration, IntegrationError>;

    /// Register a routing implementation.
    fn register_routing(
        &self,
        node_id: NodeId,
    ) -> Result<IntegrationRegistration, IntegrationError>;

    /// Register a discovery implementation.
    fn register_discovery(
        &self,
        node_id: NodeId,
    ) -> Result<IntegrationRegistration, IntegrationError>;

    /// Register a membership implementation.
    fn register_membership(
        &self,
        node_id: NodeId,
    ) -> Result<IntegrationRegistration, IntegrationError>;

    /// Register a placement implementation.
    fn register_placement(
        &self,
        node_id: NodeId,
    ) -> Result<IntegrationRegistration, IntegrationError>;

    /// Register a replication implementation.
    fn register_replication(
        &self,
        node_id: NodeId,
    ) -> Result<IntegrationRegistration, IntegrationError>;

    /// Register a recovery implementation.
    fn register_recovery(
        &self,
        node_id: NodeId,
    ) -> Result<IntegrationRegistration, IntegrationError>;

    /// Register a consistency implementation.
    fn register_consistency(
        &self,
        node_id: NodeId,
    ) -> Result<IntegrationRegistration, IntegrationError>;

    /// Register an ordering implementation.
    fn register_ordering(
        &self,
        node_id: NodeId,
    ) -> Result<IntegrationRegistration, IntegrationError>;

    /// Register a security implementation.
    fn register_security(
        &self,
        node_id: NodeId,
    ) -> Result<IntegrationRegistration, IntegrationError>;

    /// Register a client implementation.
    fn register_client(
        &self,
        node_id: NodeId,
    ) -> Result<IntegrationRegistration, IntegrationError>;

    /// Register a fault tolerance implementation.
    fn register_fault_tolerance(
        &self,
        node_id: NodeId,
    ) -> Result<IntegrationRegistration, IntegrationError>;

    /// Register a broadcast implementation.
    fn register_broadcast(
        &self,
        node_id: NodeId,
    ) -> Result<IntegrationRegistration, IntegrationError>;

    /// Register a quorum implementation.
    fn register_quorum(
        &self,
        node_id: NodeId,
    ) -> Result<IntegrationRegistration, IntegrationError>;

    /// Register a snapshot implementation.
    fn register_snapshot(
        &self,
        node_id: NodeId,
    ) -> Result<IntegrationRegistration, IntegrationError>;

    /// Register a byzantine implementation.
    fn register_byzantine(
        &self,
        node_id: NodeId,
    ) -> Result<IntegrationRegistration, IntegrationError>;

    /// Register a lease implementation.
    fn register_lease(
        &self,
        node_id: NodeId,
    ) -> Result<IntegrationRegistration, IntegrationError>;

    /// Register a CRDT implementation.
    fn register_crdt(
        &self,
        node_id: NodeId,
    ) -> Result<IntegrationRegistration, IntegrationError>;

    /// Register an erasure implementation.
    fn register_erasure(
        &self,
        node_id: NodeId,
    ) -> Result<IntegrationRegistration, IntegrationError>;

    /// Register a saga implementation.
    fn register_saga(
        &self,
        node_id: NodeId,
    ) -> Result<IntegrationRegistration, IntegrationError>;

    /// Register a coordinator implementation.
    fn register_coordinator(
        &self,
        node_id: NodeId,
    ) -> Result<IntegrationRegistration, IntegrationError>;

    /// Register a durability implementation.
    fn register_durability(
        &self,
        node_id: NodeId,
    ) -> Result<IntegrationRegistration, IntegrationError>;

    /// Register a topology implementation.
    fn register_topology(
        &self,
        node_id: NodeId,
    ) -> Result<IntegrationRegistration, IntegrationError>;

    /// Start integration for a node.
    fn start_node(&self, node_id: NodeId) -> Result<IntegrationStartResult, IntegrationError>;

    /// Stop integration for a node.
    fn stop_node(&self, node_id: NodeId) -> Result<IntegrationStopResult, IntegrationError>;

    /// Execute a cross-contract operation.
    fn execute_integrated_operation(
        &self,
        operation: IntegratedOperation,
    ) -> Result<IntegratedOperationResult, IntegrationError>;

    /// Get registration status for a node.
    fn registration_status(
        &self,
        node_id: NodeId,
    ) -> Result<NodeRegistrationStatus, IntegrationError>;

    /// Get all registered nodes.
    fn registered_nodes(&self) -> Vec<NodeId>;

    /// Get integration events.
    fn integration_events(&self) -> Vec<IntegrationEvent>;

    /// Get integration statistics.
    fn stats(&self) -> IntegrationStats;

    /// Check if integration is healthy.
    fn is_healthy(&self) -> bool;

    /// Validate integration configuration.
    fn validate_config(&self, config: &IntegrationConfig) -> Result<(), IntegrationError>;

    /// Get integration snapshot.
    fn snapshot(&self) -> IntegrationSnapshot;

    /// Restore integration from snapshot.
    fn restore_from_snapshot(
        &self,
        snapshot: IntegrationSnapshot,
    ) -> Result<(), IntegrationError>;

    /// Shutdown all integration.
    fn shutdown_all(&self) -> Result<IntegrationShutdownResult, IntegrationError>;

    /// Get contract count by type.
    fn contract_count_by_type(&self) -> HashMap<String, u32>;

    /// Get node contract coverage.
    fn node_contract_coverage(&self, node_id: NodeId) -> Result<ContractCoverage, IntegrationError>;
}

///////////////////////////////////////////////////////////////////////////////
// CONFIGURATION
///////////////////////////////////////////////////////////////////////////////

/// Integration configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IntegrationConfig {
    /// Maximum number of registered nodes.
    pub max_nodes: u32,
    /// Maximum number of contracts per node.
    pub max_contracts_per_node: u32,
    /// Enable cross-contract operation chaining.
    pub enable_operation_chaining: bool,
    /// Operation chain timeout (ms).
    pub operation_chain_timeout_ms: u64,
    /// Enable automatic node startup.
    pub enable_auto_startup: bool,
    /// Node startup timeout (ms).
    pub node_startup_timeout_ms: u64,
    /// Enable health checking.
    pub enable_health_checking: bool,
    /// Health check interval (ms).
    pub health_check_interval_ms: u64,
    /// Enable event recording.
    pub enable_event_recording: bool,
    /// Maximum recorded events.
    pub max_recorded_events: u32,
    /// Enable snapshot/restore.
    pub enable_snapshot_restore: bool,
    /// Snapshot interval (ms).
    pub snapshot_interval_ms: u64,
    /// Enable strict contract validation.
    pub enable_strict_validation: bool,
}

impl IntegrationConfig {
    pub fn new(max_nodes: u32) -> Result<Self, IntegrationError> {
        let config = Self {
            max_nodes,
            max_contracts_per_node: 25,
            enable_operation_chaining: true,
            operation_chain_timeout_ms: 30000,
            enable_auto_startup: true,
            node_startup_timeout_ms: 10000,
            enable_health_checking: true,
            health_check_interval_ms: 5000,
            enable_event_recording: true,
            max_recorded_events: 10000,
            enable_snapshot_restore: true,
            snapshot_interval_ms: 60000,
            enable_strict_validation: true,
        };
        config.validate()?;
        Ok(config)
    }

    pub fn validate(&self) -> Result<(), IntegrationError> {
        if self.max_nodes == 0 {
            return Err(IntegrationError::InvalidConfig {
                field: "max_nodes".to_string(),
                reason: "max_nodes must be greater than 0".to_string(),
            });
        }
        if self.max_contracts_per_node == 0 {
            return Err(IntegrationError::InvalidConfig {
                field: "max_contracts_per_node".to_string(),
                reason: "max_contracts_per_node must be greater than 0".to_string(),
            });
        }
        if self.operation_chain_timeout_ms == 0 {
            return Err(IntegrationError::InvalidConfig {
                field: "operation_chain_timeout_ms".to_string(),
                reason: "operation_chain_timeout_ms must be greater than 0".to_string(),
            });
        }
        if self.node_startup_timeout_ms == 0 {
            return Err(IntegrationError::InvalidConfig {
                field: "node_startup_timeout_ms".to_string(),
                reason: "node_startup_timeout_ms must be greater than 0".to_string(),
            });
        }
        if self.health_check_interval_ms == 0 {
            return Err(IntegrationError::InvalidConfig {
                field: "health_check_interval_ms".to_string(),
                reason: "health_check_interval_ms must be greater than 0".to_string(),
            });
        }
        if self.max_recorded_events == 0 {
            return Err(IntegrationError::InvalidConfig {
                field: "max_recorded_events".to_string(),
                reason: "max_recorded_events must be greater than 0".to_string(),
            });
        }
        if self.snapshot_interval_ms == 0 {
            return Err(IntegrationError::InvalidConfig {
                field: "snapshot_interval_ms".to_string(),
                reason: "snapshot_interval_ms must be greater than 0".to_string(),
            });
        }
        Ok(())
    }

    pub fn default() -> Self {
        Self {
            max_nodes: 1000,
            max_contracts_per_node: 25,
            enable_operation_chaining: true,
            operation_chain_timeout_ms: 30000,
            enable_auto_startup: true,
            node_startup_timeout_ms: 10000,
            enable_health_checking: true,
            health_check_interval_ms: 5000,
            enable_event_recording: true,
            max_recorded_events: 10000,
            enable_snapshot_restore: true,
            snapshot_interval_ms: 60000,
            enable_strict_validation: true,
        }
    }

    pub fn strict() -> Self {
        Self {
            max_nodes: 100,
            max_contracts_per_node: 10,
            enable_operation_chaining: true,
            operation_chain_timeout_ms: 5000,
            enable_auto_startup: false,
            node_startup_timeout_ms: 5000,
            enable_health_checking: true,
            health_check_interval_ms: 1000,
            enable_event_recording: true,
            max_recorded_events: 1000,
            enable_snapshot_restore: true,
            snapshot_interval_ms: 10000,
            enable_strict_validation: true,
        }
    }

    pub fn lenient() -> Self {
        Self {
            max_nodes: 10000,
            max_contracts_per_node: 100,
            enable_operation_chaining: false,
            operation_chain_timeout_ms: 300000,
            enable_auto_startup: true,
            node_startup_timeout_ms: 60000,
            enable_health_checking: false,
            health_check_interval_ms: 60000,
            enable_event_recording: false,
            max_recorded_events: 100000,
            enable_snapshot_restore: false,
            snapshot_interval_ms: 300000,
            enable_strict_validation: false,
        }
    }

    pub fn contract_density(&self) -> f64 {
        if self.max_nodes == 0 {
            return 0.0;
        }
        self.max_contracts_per_node as f64 / self.max_nodes as f64 * 100.0
    }
}

impl Default for IntegrationConfig {
    fn default() -> Self {
        Self::default()
    }
}

///////////////////////////////////////////////////////////////////////////////
// REGISTRATION
///////////////////////////////////////////////////////////////////////////////

/// Registration result for a contract on a node.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IntegrationRegistration {
    pub node_id: NodeId,
    pub contract_type: ContractType,
    pub registered_at: u64,
    pub registration_id: u64,
    pub status: RegistrationStatus,
}

/// Types of contracts that can be registered.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ContractType {
    Carrier,
    Transport,
    Store,
    Session,
    Routing,
    Discovery,
    Membership,
    Placement,
    Replication,
    Recovery,
    Consistency,
    Ordering,
    Security,
    Client,
    FaultTolerance,
    Broadcast,
    Quorum,
    Snapshot,
    Byzantine,
    Lease,
    CRDT,
    Erasure,
    Saga,
    Coordinator,
    Durability,
    Topology,
}

impl ContractType {
    pub fn as_str(&self) -> &'static str {
        match self {
            ContractType::Carrier => "carrier",
            ContractType::Transport => "transport",
            ContractType::Store => "store",
            ContractType::Session => "session",
            ContractType::Routing => "routing",
            ContractType::Discovery => "discovery",
            ContractType::Membership => "membership",
            ContractType::Placement => "placement",
            ContractType::Replication => "replication",
            ContractType::Recovery => "recovery",
            ContractType::Consistency => "consistency",
            ContractType::Ordering => "ordering",
            ContractType::Security => "security",
            ContractType::Client => "client",
            ContractType::FaultTolerance => "fault_tolerance",
            ContractType::Broadcast => "broadcast",
            ContractType::Quorum => "quorum",
            ContractType::Snapshot => "snapshot",
            ContractType::Byzantine => "byzantine",
            ContractType::Lease => "lease",
            ContractType::CRDT => "crdt",
            ContractType::Erasure => "erasure",
            ContractType::Saga => "saga",
            ContractType::Coordinator => "coordinator",
            ContractType::Durability => "durability",
            ContractType::Topology => "topology",
        }
    }

    pub fn all_types() -> Vec<ContractType> {
        vec![
            ContractType::Carrier,
            ContractType::Transport,
            ContractType::Store,
            ContractType::Session,
            ContractType::Routing,
            ContractType::Discovery,
            ContractType::Membership,
            ContractType::Placement,
            ContractType::Replication,
            ContractType::Recovery,
            ContractType::Consistency,
            ContractType::Ordering,
            ContractType::Security,
            ContractType::Client,
            ContractType::FaultTolerance,
            ContractType::Broadcast,
            ContractType::Quorum,
            ContractType::Snapshot,
            ContractType::Byzantine,
            ContractType::Lease,
            ContractType::CRDT,
            ContractType::Erasure,
            ContractType::Saga,
            ContractType::Coordinator,
            ContractType::Durability,
            ContractType::Topology,
        ]
    }
}

/// Registration status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RegistrationStatus {
    Pending,
    Active,
    Degraded,
    Failed,
    Stopped,
}

impl RegistrationStatus {
    pub fn is_active(&self) -> bool {
        matches!(self, RegistrationStatus::Active)
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            RegistrationStatus::Pending => "pending",
            RegistrationStatus::Active => "active",
            RegistrationStatus::Degraded => "degraded",
            RegistrationStatus::Failed => "failed",
            RegistrationStatus::Stopped => "stopped",
        }
    }
}

/// Node registration status summary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodeRegistrationStatus {
    pub node_id: NodeId,
    pub registered_contracts: HashMap<ContractType, RegistrationStatus>,
    pub active_contract_count: u32,
    pub total_contract_count: u32,
    pub is_fully_registered: bool,
    pub last_registration_at: u64,
}

impl NodeRegistrationStatus {
    pub fn coverage_percent(&self) -> f64 {
        if self.total_contract_count == 0 {
            return 100.0;
        }
        self.active_contract_count as f64 / self.total_contract_count as f64 * 100.0
    }

    pub fn has_contract(&self, contract_type: ContractType) -> bool {
        self.registered_contracts
            .get(&contract_type)
            .map(|s| s.is_active())
            .unwrap_or(false)
    }
}

/// Contract coverage for a node.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContractCoverage {
    pub node_id: NodeId,
    pub active_contracts: Vec<ContractType>,
    pub missing_contracts: Vec<ContractType>,
    pub coverage_percent: f64,
    pub is_complete: bool,
}

///////////////////////////////////////////////////////////////////////////////
// INTEGRATED OPERATION
///////////////////////////////////////////////////////////////////////////////

/// A cross-contract operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IntegratedOperation {
    pub operation_id: u64,
    pub operation_type: IntegratedOperationType,
    pub source_node: NodeId,
    pub target_nodes: Vec<NodeId>,
    pub dataset_id: Option<DatasetId>,
    pub contract_chain: Vec<ContractType>,
    pub payload: Vec<u8>,
    pub timeout_ms: u64,
    pub priority: u8,
    pub created_at: u64,
}

/// Types of integrated operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum IntegratedOperationType {
    Write,
    Read,
    Delete,
    Replicate,
    Recover,
    Broadcast,
    Commit,
    Abort,
    Snapshot,
    Restore,
    Discover,
    Join,
    Leave,
    HealthCheck,
    Rebalance,
}

impl IntegratedOperationType {
    pub fn as_str(&self) -> &'static str {
        match self {
            IntegratedOperationType::Write => "write",
            IntegratedOperationType::Read => "read",
            IntegratedOperationType::Delete => "delete",
            IntegratedOperationType::Replicate => "replicate",
            IntegratedOperationType::Recover => "recover",
            IntegratedOperationType::Broadcast => "broadcast",
            IntegratedOperationType::Commit => "commit",
            IntegratedOperationType::Abort => "abort",
            IntegratedOperationType::Snapshot => "snapshot",
            IntegratedOperationType::Restore => "restore",
            IntegratedOperationType::Discover => "discover",
            IntegratedOperationType::Join => "join",
            IntegratedOperationType::Leave => "leave",
            IntegratedOperationType::HealthCheck => "health_check",
            IntegratedOperationType::Rebalance => "rebalance",
        }
    }

    pub fn required_contracts(&self) -> Vec<ContractType> {
        match self {
            IntegratedOperationType::Write => {
                vec![
                    ContractType::Security,
                    ContractType::Store,
                    ContractType::Replication,
                    ContractType::Consistency,
                    ContractType::Ordering,
                    ContractType::Durability,
                ]
            }
            IntegratedOperationType::Read => {
                vec![
                    ContractType::Security,
                    ContractType::Store,
                    ContractType::Consistency,
                    ContractType::Routing,
                ]
            }
            IntegratedOperationType::Delete => {
                vec![
                    ContractType::Security,
                    ContractType::Store,
                    ContractType::Replication,
                    ContractType::Consistency,
                    ContractType::Ordering,
                ]
            }
            IntegratedOperationType::Replicate => {
                vec![
                    ContractType::Replication,
                    ContractType::Transport,
                    ContractType::Consistency,
                    ContractType::Durability,
                ]
            }
            IntegratedOperationType::Recover => {
                vec![
                    ContractType::Recovery,
                    ContractType::Store,
                    ContractType::Replication,
                    ContractType::FaultTolerance,
                ]
            }
            IntegratedOperationType::Broadcast => {
                vec![ContractType::Broadcast, ContractType::Transport, ContractType::Membership]
            }
            IntegratedOperationType::Commit => {
                vec![
                    ContractType::Quorum,
                    ContractType::Ordering,
                    ContractType::Consistency,
                    ContractType::Durability,
                ]
            }
            IntegratedOperationType::Abort => {
                vec![ContractType::Saga, ContractType::Ordering, ContractType::Consistency]
            }
            IntegratedOperationType::Snapshot => {
                vec![ContractType::Snapshot, ContractType::Store, ContractType::Durability]
            }
            IntegratedOperationType::Restore => {
                vec![ContractType::Snapshot, ContractType::Store, ContractType::Recovery]
            }
            IntegratedOperationType::Discover => {
                vec![ContractType::Discovery, ContractType::Membership, ContractType::Topology]
            }
            IntegratedOperationType::Join => {
                vec![
                    ContractType::Membership,
                    ContractType::Discovery,
                    ContractType::Topology,
                    ContractType::Security,
                ]
            }
            IntegratedOperationType::Leave => {
                vec![
                    ContractType::Membership,
                    ContractType::Topology,
                    ContractType::Replication,
                ]
            }
            IntegratedOperationType::HealthCheck => {
                vec![
                    ContractType::FaultTolerance,
                    ContractType::HealthCheck as _,
                ]
            }
            IntegratedOperationType::Rebalance => {
                vec![
                    ContractType::Placement,
                    ContractType::Replication,
                    ContractType::Transport,
                ]
            }
        }
    }
}

// Workaround for HealthCheck variant not existing — using FaultTolerance instead
type HealthCheck = FaultTolerance;

/// Result of an integrated operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IntegratedOperationResult {
    pub operation_id: u64,
    pub success: bool,
    pub contract_results: HashMap<ContractType, ContractExecutionResult>,
    pub total_duration_ms: u64,
    pub completed_at: u64,
    pub error: Option<IntegrationError>,
}

/// Result of executing a single contract within an operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContractExecutionResult {
    pub contract_type: ContractType,
    pub success: bool,
    pub duration_ms: u64,
    pub output: Vec<u8>,
    pub error: Option<String>,
}

impl ContractExecutionResult {
    pub fn success(duration_ms: u64, output: Vec<u8>) -> Self {
        Self {
            contract_type: ContractType::Carrier,
            success: true,
            duration_ms,
            output,
            error: None,
        }
    }

    pub fn failure(duration_ms: u64, error: String) -> Self {
        Self {
            contract_type: ContractType::Carrier,
            success: false,
            duration_ms,
            output: Vec::new(),
            error: Some(error),
        }
    }
}

/// Start result for a node.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IntegrationStartResult {
    pub node_id: NodeId,
    pub started_at: u64,
    pub contracts_started: u32,
    pub contracts_failed: u32,
    pub startup_duration_ms: u64,
}

/// Stop result for a node.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IntegrationStopResult {
    pub node_id: NodeId,
    pub stopped_at: u64,
    pub contracts_stopped: u32,
    pub stop_duration_ms: u64,
}

/// Shutdown result for all nodes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IntegrationShutdownResult {
    pub nodes_stopped: u32,
    pub contracts_stopped: u32,
    pub shutdown_duration_ms: u64,
    pub completed_at: u64,
}

///////////////////////////////////////////////////////////////////////////////
// EVENTS
///////////////////////////////////////////////////////////////////////////////

/// An integration event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IntegrationEvent {
    pub event_id: u64,
    pub event_type: IntegrationEventType,
    pub node_id: Option<NodeId>,
    pub contract_type: Option<ContractType>,
    pub timestamp: u64,
    pub details: Vec<u8>,
}

/// Types of integration events.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum IntegrationEventType {
    ContractRegistered,
    ContractUnregistered,
    NodeStarted,
    NodeStopped,
    OperationStarted,
    OperationCompleted,
    OperationFailed,
    ContractStarted,
    ContractStopped,
    ContractFailed,
    HealthCheckPassed,
    HealthCheckFailed,
    SnapshotCreated,
    SnapshotRestored,
    IntegrationError,
}

impl IntegrationEventType {
    pub fn as_str(&self) -> &'static str {
        match self {
            IntegrationEventType::ContractRegistered => "contract_registered",
            IntegrationEventType::ContractUnregistered => "contract_unregistered",
            IntegrationEventType::NodeStarted => "node_started",
            IntegrationEventType::NodeStopped => "node_stopped",
            IntegrationEventType::OperationStarted => "operation_started",
            IntegrationEventType::OperationCompleted => "operation_completed",
            IntegrationEventType::OperationFailed => "operation_failed",
            IntegrationEventType::ContractStarted => "contract_started",
            IntegrationEventType::ContractStopped => "contract_stopped",
            IntegrationEventType::ContractFailed => "contract_failed",
            IntegrationEventType::HealthCheckPassed => "health_check_passed",
            IntegrationEventType::HealthCheckFailed => "health_check_failed",
            IntegrationEventType::SnapshotCreated => "snapshot_created",
            IntegrationEventType::SnapshotRestored => "snapshot_restored",
            IntegrationEventType::IntegrationError => "integration_error",
        }
    }
}

///////////////////////////////////////////////////////////////////////////////
// STATS
///////////////////////////////////////////////////////////////////////////////

/// Integration statistics.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IntegrationStats {
    pub total_nodes: u32,
    pub active_nodes: u32,
    pub stopped_nodes: u32,
    pub total_contracts: u32,
    pub active_contracts: u32,
    pub total_operations: u64,
    pub successful_operations: u64,
    pub failed_operations: u64,
    pub total_contract_executions: u64,
    pub successful_contract_executions: u64,
    pub failed_contract_executions: u64,
    pub total_events: u64,
    pub average_operation_duration_ms: f64,
    pub uptime_percent: f64,
}

impl IntegrationStats {
    pub fn new() -> Self {
        Self {
            total_nodes: 0,
            active_nodes: 0,
            stopped_nodes: 0,
            total_contracts: 0,
            active_contracts: 0,
            total_operations: 0,
            successful_operations: 0,
            failed_operations: 0,
            total_contract_executions: 0,
            successful_contract_executions: 0,
            failed_contract_executions: 0,
            total_events: 0,
            average_operation_duration_ms: 0.0,
            uptime_percent: 100.0,
        }
    }

    pub fn operation_success_rate(&self) -> f64 {
        if self.total_operations == 0 {
            return 100.0;
        }
        self.successful_operations as f64 / self.total_operations as f64 * 100.0
    }

    pub fn contract_execution_success_rate(&self) -> f64 {
        if self.total_contract_executions == 0 {
            return 100.0;
        }
        self.successful_contract_executions as f64 / self.total_contract_executions as f64
            * 100.0
    }

    pub fn node_availability_percent(&self) -> f64 {
        if self.total_nodes == 0 {
            return 100.0;
        }
        self.active_nodes as f64 / self.total_nodes as f64 * 100.0
    }
}

///////////////////////////////////////////////////////////////////////////////
// SNAPSHOT
///////////////////////////////////////////////////////////////////////////////

/// Integration snapshot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IntegrationSnapshot {
    pub version: u64,
    pub timestamp: u64,
    pub nodes: Vec<NodeId>,
    pub registrations: Vec<IntegrationRegistration>,
    pub stats: IntegrationStats,
}

impl IntegrationSnapshot {
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    pub fn registration_count(&self) -> usize {
        self.registrations.len()
    }
}

///////////////////////////////////////////////////////////////////////////////
// ERROR
///////////////////////////////////////////////////////////////////////////////

/// Integration management errors.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
pub enum IntegrationError {
    #[error("invalid config: {field} - {reason}")]
    InvalidConfig { field: String, reason: String },

    #[error("node not found: {node_id:?}")]
    NodeNotFound { node_id: NodeId },

    #[error("node already registered: {node_id:?}")]
    NodeAlreadyRegistered { node_id: NodeId },

    #[error("contract not registered: {contract:?} on {node_id:?}")]
    ContractNotRegistered {
        contract: ContractType,
        node_id: NodeId,
    },

    #[error("contract already registered: {contract:?} on {node_id:?}")]
    ContractAlreadyRegistered {
        contract: ContractType,
        node_id: NodeId,
    },

    #[error("maximum nodes exceeded: {max_nodes}")]
    MaxNodesExceeded { max_nodes: u32 },

    #[error("maximum contracts per node exceeded: {node_id:?}")]
    MaxContractsPerNodeExceeded { node_id: NodeId },

    #[error("operation failed: {operation_id}")]
    OperationFailed { operation_id: u64 },

    #[error("operation timeout: {operation_id}")]
    OperationTimeout { operation_id: u64 },

    #[error("insufficient contracts: {missing:?}")]
    InsufficientContracts { missing: Vec<ContractType> },

    #[error("node not started: {node_id:?}")]
    NodeNotStarted { node_id: NodeId },

    #[error("snapshot version mismatch: expected {expected}, got {got}")]
    SnapshotVersionMismatch { expected: u64, got: u64 },

    #[error("internal error: {message}")]
    Internal { message: String },
}

impl IntegrationError {
    pub fn category(&self) -> &'static str {
        match self {
            IntegrationError::InvalidConfig { .. } => "invalid_config",
            IntegrationError::NodeNotFound { .. } => "node_not_found",
            IntegrationError::NodeAlreadyRegistered { .. } => "node_already_registered",
            IntegrationError::ContractNotRegistered { .. } => "contract_not_registered",
            IntegrationError::ContractAlreadyRegistered { .. } => "contract_already_registered",
            IntegrationError::MaxNodesExceeded { .. } => "max_nodes_exceeded",
            IntegrationError::MaxContractsPerNodeExceeded { .. } => "max_contracts_per_node_exceeded",
            IntegrationError::OperationFailed { .. } => "operation_failed",
            IntegrationError::OperationTimeout { .. } => "operation_timeout",
            IntegrationError::InsufficientContracts { .. } => "insufficient_contracts",
            IntegrationError::NodeNotStarted { .. } => "node_not_started",
            IntegrationError::SnapshotVersionMismatch { .. } => "snapshot_version_mismatch",
            IntegrationError::Internal { .. } => "internal",
        }
    }

    pub fn is_recoverable(&self) -> bool {
        matches!(
            self,
            IntegrationError::OperationFailed { .. }
                | IntegrationError::OperationTimeout { .. }
                | IntegrationError::InsufficientContracts { .. }
                | IntegrationError::NodeNotStarted { .. }
        )
    }

    pub fn suggested_action(&self) -> &'static str {
        match self {
            IntegrationError::InvalidConfig { .. } => "correct the integration configuration",
            IntegrationError::NodeNotFound { .. } => "register the node first",
            IntegrationError::NodeAlreadyRegistered { .. } => "use existing registration",
            IntegrationError::ContractNotRegistered { .. } => "register the contract first",
            IntegrationError::ContractAlreadyRegistered { .. } => "use existing contract registration",
            IntegrationError::MaxNodesExceeded { .. } => "increase max_nodes or remove nodes",
            IntegrationError::MaxContractsPerNodeExceeded { .. } => {
                "remove some contracts or increase limit"
            }
            IntegrationError::OperationFailed { .. } => "retry the operation",
            IntegrationError::OperationTimeout { .. } => "increase timeout or retry",
            IntegrationError::InsufficientContracts { .. } => "register missing contracts",
            IntegrationError::NodeNotStarted { .. } => "start the node first",
            IntegrationError::SnapshotVersionMismatch { .. } => "use matching snapshot version",
            IntegrationError::Internal { .. } => "contact system administrator",
        }
    }
}

///////////////////////////////////////////////////////////////////////////////
// INTERNAL STATE
///////////////////////////////////////////////////////////////////////////////

/// Internal registration record.
#[derive(Debug, Clone)]
struct InternalRegistration {
    node_id: NodeId,
    contract_type: ContractType,
    registered_at: u64,
    registration_id: u64,
    status: RegistrationStatus,
}

/// Internal node state.
#[derive(Debug, Clone)]
struct InternalNodeState {
    node_id: NodeId,
    endpoint: Option<Endpoint>,
    registrations: HashMap<ContractType, InternalRegistration>,
    started_at: Option<u64>,
    stopped_at: Option<u64>,
}

/// Internal integration state.
#[derive(Debug, Clone)]
struct IntegrationGraph {
    nodes: HashMap<NodeId, InternalNodeState>,
    registrations: Vec<IntegrationRegistration>,
    events: Vec<IntegrationEvent>,
    event_counter: u64,
    registration_counter: u64,
    operation_counter: u64,
    version: u64,
}

impl IntegrationGraph {
    fn new() -> Self {
        Self {
            nodes: HashMap::new(),
            registrations: Vec::new(),
            events: Vec::new(),
            event_counter: 0,
            registration_counter: 0,
            operation_counter: 0,
            version: 0,
        }
    }

    fn add_event(
        &mut self,
        event_type: IntegrationEventType,
        node_id: Option<NodeId>,
        contract_type: Option<ContractType>,
    ) -> u64 {
        self.event_counter += 1;
        let record = IntegrationEvent {
            event_id: self.event_counter,
            event_type,
            node_id,
            contract_type,
            timestamp: 0,
            details: Vec::new(),
        };
        self.events.push(record.clone());
        self.event_counter
    }
}

///////////////////////////////////////////////////////////////////////////////
// INTEGRATION ENGINE (REFERENCE IMPLEMENTATION)
///////////////////////////////////////////////////////////////////////////////

/// Reference integration engine implementing the Integration trait.
pub struct IntegrationEngine {
    config: std::cell::RefCell<IntegrationConfig>,
    graph: std::cell::RefCell<IntegrationGraph>,
}

impl IntegrationEngine {
    pub fn new() -> Self {
        Self {
            config: std::cell::RefCell::new(IntegrationConfig::default()),
            graph: std::cell::RefCell::new(IntegrationGraph::new()),
        }
    }

    pub fn with_config(config: IntegrationConfig) -> Result<Self, IntegrationError> {
        config.validate()?;
        Ok(Self {
            config: std::cell::RefCell::new(config),
            graph: std::cell::RefCell::new(IntegrationGraph::new()),
        })
    }

    fn ensure_config(&self) -> IntegrationConfig {
        self.config.borrow().clone()
    }
}

///////////////////////////////////////////////////////////////////////////////
// IMPL INTEGRATION FOR INTEGRATIONENGINE
///////////////////////////////////////////////////////////////////////////////

impl private::Sealed for IntegrationEngine {}

impl Integration for IntegrationEngine {
    fn configure_integration(
        &self,
        config: IntegrationConfig,
    ) -> Result<(), IntegrationError> {
        config.validate()?;
        *self.config.borrow_mut() = config;
        Ok(())
    }

    fn register_carrier(
        &self,
        carrier_id: NodeId,
        endpoint: Endpoint,
    ) -> Result<IntegrationRegistration, IntegrationError> {
        self.register_contract(carrier_id, ContractType::Carrier, endpoint)
    }

    fn register_transport(
        &self,
        node_id: NodeId,
    ) -> Result<IntegrationRegistration, IntegrationError> {
        self.register_contract(node_id, ContractType::Transport, Endpoint::default())
    }

    fn register_store(
        &self,
        node_id: NodeId,
    ) -> Result<IntegrationRegistration, IntegrationError> {
        self.register_contract(node_id, ContractType::Store, Endpoint::default())
    }

    fn register_session(
        &self,
        node_id: NodeId,
    ) -> Result<IntegrationRegistration, IntegrationError> {
        self.register_contract(node_id, ContractType::Session, Endpoint::default())
    }

    fn register_routing(
        &self,
        node_id: NodeId,
    ) -> Result<IntegrationRegistration, IntegrationError> {
        self.register_contract(node_id, ContractType::Routing, Endpoint::default())
    }

    fn register_discovery(
        &self,
        node_id: NodeId,
    ) -> Result<IntegrationRegistration, IntegrationError> {
        self.register_contract(node_id, ContractType::Discovery, Endpoint::default())
    }

    fn register_membership(
        &self,
        node_id: NodeId,
    ) -> Result<IntegrationRegistration, IntegrationError> {
        self.register_contract(node_id, ContractType::Membership, Endpoint::default())
    }

    fn register_placement(
        &self,
        node_id: NodeId,
    ) -> Result<IntegrationRegistration, IntegrationError> {
        self.register_contract(node_id, ContractType::Placement, Endpoint::default())
    }

    fn register_replication(
        &self,
        node_id: NodeId,
    ) -> Result<IntegrationRegistration, IntegrationError> {
        self.register_contract(node_id, ContractType::Replication, Endpoint::default())
    }

    fn register_recovery(
        &self,
        node_id: NodeId,
    ) -> Result<IntegrationRegistration, IntegrationError> {
        self.register_contract(node_id, ContractType::Recovery, Endpoint::default())
    }

    fn register_consistency(
        &self,
        node_id: NodeId,
    ) -> Result<IntegrationRegistration, IntegrationError> {
        self.register_contract(node_id, ContractType::Consistency, Endpoint::default())
    }

    fn register_ordering(
        &self,
        node_id: NodeId,
    ) -> Result<IntegrationRegistration, IntegrationError> {
        self.register_contract(node_id, ContractType::Ordering, Endpoint::default())
    }

    fn register_security(
        &self,
        node_id: NodeId,
    ) -> Result<IntegrationRegistration, IntegrationError> {
        self.register_contract(node_id, ContractType::Security, Endpoint::default())
    }

    fn register_client(
        &self,
        node_id: NodeId,
    ) -> Result<IntegrationRegistration, IntegrationError> {
        self.register_contract(node_id, ContractType::Client, Endpoint::default())
    }

    fn register_fault_tolerance(
        &self,
        node_id: NodeId,
    ) -> Result<IntegrationRegistration, IntegrationError> {
        self.register_contract(node_id, ContractType::FaultTolerance, Endpoint::default())
    }

    fn register_broadcast(
        &self,
        node_id: NodeId,
    ) -> Result<IntegrationRegistration, IntegrationError> {
        self.register_contract(node_id, ContractType::Broadcast, Endpoint::default())
    }

    fn register_quorum(
        &self,
        node_id: NodeId,
    ) -> Result<IntegrationRegistration, IntegrationError> {
        self.register_contract(node_id, ContractType::Quorum, Endpoint::default())
    }

    fn register_snapshot(
        &self,
        node_id: NodeId,
    ) -> Result<IntegrationRegistration, IntegrationError> {
        self.register_contract(node_id, ContractType::Snapshot, Endpoint::default())
    }

    fn register_byzantine(
        &self,
        node_id: NodeId,
    ) -> Result<IntegrationRegistration, IntegrationError> {
        self.register_contract(node_id, ContractType::Byzantine, Endpoint::default())
    }

    fn register_lease(
        &self,
        node_id: NodeId,
    ) -> Result<IntegrationRegistration, IntegrationError> {
        self.register_contract(node_id, ContractType::Lease, Endpoint::default())
    }

    fn register_crdt(
        &self,
        node_id: NodeId,
    ) -> Result<IntegrationRegistration, IntegrationError> {
        self.register_contract(node_id, ContractType::CRDT, Endpoint::default())
    }

    fn register_erasure(
        &self,
        node_id: NodeId,
    ) -> Result<IntegrationRegistration, IntegrationError> {
        self.register_contract(node_id, ContractType::Erasure, Endpoint::default())
    }

    fn register_saga(
        &self,
        node_id: NodeId,
    ) -> Result<IntegrationRegistration, IntegrationError> {
        self.register_contract(node_id, ContractType::Saga, Endpoint::default())
    }

    fn register_coordinator(
        &self,
        node_id: NodeId,
    ) -> Result<IntegrationRegistration, IntegrationError> {
        self.register_contract(node_id, ContractType::Coordinator, Endpoint::default())
    }

    fn register_durability(
        &self,
        node_id: NodeId,
    ) -> Result<IntegrationRegistration, IntegrationError> {
        self.register_contract(node_id, ContractType::Durability, Endpoint::default())
    }

    fn register_topology(
        &self,
        node_id: NodeId,
    ) -> Result<IntegrationRegistration, IntegrationError> {
        self.register_contract(node_id, ContractType::Topology, Endpoint::default())
    }

    fn start_node(&self, node_id: NodeId) -> Result<IntegrationStartResult, IntegrationError> {
        let mut graph = self.graph.borrow_mut();
        let config = self.config.borrow();

        let node = graph
            .nodes
            .get_mut(&node_id)
            .ok_or_else(|| IntegrationError::NodeNotFound { node_id })?;

        if node.started_at.is_some() {
            return Err(IntegrationError::NodeAlreadyRegistered { node_id });
        }

        let started_at = 0u64;
        node.started_at = Some(started_at);

        let active_count = node
            .registrations
            .values()
            .filter(|r| r.status == RegistrationStatus::Active)
            .count() as u32;
        let failed_count = node
            .registrations
            .values()
            .filter(|r| r.status == RegistrationStatus::Failed)
            .count() as u32;

        graph.add_event(
            IntegrationEventType::NodeStarted,
            Some(node_id),
            None,
        );
        graph.version += 1;

        drop(config);

        Ok(IntegrationStartResult {
            node_id,
            started_at,
            contracts_started: active_count,
            contracts_failed: failed_count,
            startup_duration_ms: 0,
        })
    }

    fn stop_node(&self, node_id: NodeId) -> Result<IntegrationStopResult, IntegrationError> {
        let mut graph = self.graph.borrow_mut();

        let node = graph
            .nodes
            .get_mut(&node_id)
            .ok_or_else(|| IntegrationError::NodeNotFound { node_id })?;

        if node.started_at.is_none() {
            return Err(IntegrationError::NodeNotStarted { node_id });
        }

        let stopped_at = 0u64;
        node.stopped_at = Some(stopped_at);

        let contracts_stopped = node.registrations.len() as u32;

        graph.add_event(
            IntegrationEventType::NodeStopped,
            Some(node_id),
            None,
        );
        graph.version += 1;

        Ok(IntegrationStopResult {
            node_id,
            stopped_at,
            contracts_stopped,
            stop_duration_ms: 0,
        })
    }

    fn execute_integrated_operation(
        &self,
        operation: IntegratedOperation,
    ) -> Result<IntegratedOperationResult, IntegrationError> {
        let config = self.config.borrow();
        let mut graph = self.graph.borrow_mut();

        if !graph.nodes.contains_key(&operation.source_node) {
            return Err(IntegrationError::NodeNotFound {
                node_id: operation.source_node,
            });
        }

        graph.operation_counter += 1;
        let operation_id = graph.operation_counter;

        // Check if required contracts are registered on source node
        let required = operation.operation_type.required_contracts();
        let source_node = graph
            .nodes
            .get(&operation.source_node)
            .ok_or_else(|| IntegrationError::NodeNotFound {
                node_id: operation.source_node,
            })?;

        let mut missing: Vec<ContractType> = Vec::new();
        for &contract_type in &required {
            if !source_node
                .registrations
                .contains_key(&contract_type)
            {
                missing.push(contract_type);
            }
        }

        if !missing.is_empty() {
            return Err(IntegrationError::InsufficientContracts { missing });
        }

        // Execute each contract in the chain
        let mut contract_results = HashMap::new();
        let mut total_duration_ms: u64 = 0;
        let mut all_success = true;

        for &contract_type in &operation.contract_chain {
            let start_ms = 0u64;
            let result = if source_node.registrations.contains_key(&contract_type) {
                ContractExecutionResult::success(1, vec![1, 2, 3])
            } else {
                all_success = false;
                ContractExecutionResult::failure(1, "contract not registered".to_string())
            };
            total_duration_ms = total_duration_ms.saturating_add(1);
            contract_results.insert(contract_type, result);
        }

        graph.add_event(
            if all_success {
                IntegrationEventType::OperationCompleted
            } else {
                IntegrationEventType::OperationFailed
            },
            Some(operation.source_node),
            None,
        );
        graph.version += 1;

        drop(config);

        Ok(IntegratedOperationResult {
            operation_id,
            success: all_success,
            contract_results,
            total_duration_ms,
            completed_at: 0,
            error: if all_success { None } else { Some(IntegrationError::OperationFailed { operation_id }) },
        })
    }

    fn registration_status(
        &self,
        node_id: NodeId,
    ) -> Result<NodeRegistrationStatus, IntegrationError> {
        let graph = self.graph.borrow();
        let node = graph
            .nodes
            .get(&node_id)
            .ok_or_else(|| IntegrationError::NodeNotFound { node_id })?;

        let mut registered_contracts = HashMap::new();
        let mut active_count = 0u32;
        let mut total_count = 0u32;

        for (contract_type, registration) in &node.registrations {
            registered_contracts.insert(*contract_type, registration.status);
            if registration.status == RegistrationStatus::Active {
                active_count += 1;
            }
            total_count += 1;
        }

        let last_registration = node
            .registrations
            .values()
            .map(|r| r.registered_at)
            .max()
            .unwrap_or(0);

        Ok(NodeRegistrationStatus {
            node_id,
            registered_contracts,
            active_contract_count: active_count,
            total_contract_count: total_count,
            is_fully_registered: active_count == total_count && total_count > 0,
            last_registration_at: last_registration,
        })
    }

    fn registered_nodes(&self) -> Vec<NodeId> {
        let graph = self.graph.borrow();
        graph.nodes.keys().cloned().collect()
    }

    fn integration_events(&self) -> Vec<IntegrationEvent> {
        let graph = self.graph.borrow();
        graph.events.clone()
    }

    fn stats(&self) -> IntegrationStats {
        let graph = self.graph.borrow();
        let total_nodes = graph.nodes.len() as u32;
        let active_nodes = graph
            .nodes
            .values()
            .filter(|n| n.started_at.is_some() && n.stopped_at.is_none())
            .count() as u32;
        let stopped_nodes = graph
            .nodes
            .values()
            .filter(|n| n.stopped_at.is_some())
            .count() as u32;

        let total_contracts = graph.registrations.len() as u32;
        let active_contracts = graph
            .registrations
            .iter()
            .filter(|r| r.status == RegistrationStatus::Active)
            .count() as u32;

        IntegrationStats {
            total_nodes,
            active_nodes,
            stopped_nodes,
            total_contracts,
            active_contracts,
            total_operations: graph.operation_counter,
            successful_operations: 0,
            failed_operations: 0,
            total_contract_executions: 0,
            successful_contract_executions: 0,
            failed_contract_executions: 0,
            total_events: graph.event_counter,
            average_operation_duration_ms: 0.0,
            uptime_percent: if total_nodes == 0 {
                100.0
            } else {
                active_nodes as f64 / total_nodes as f64 * 100.0
            },
        }
    }

    fn is_healthy(&self) -> bool {
        let graph = self.graph.borrow();
        if graph.nodes.is_empty() {
            return true;
        }
        let active_count = graph
            .nodes
            .values()
            .filter(|n| n.started_at.is_some() && n.stopped_at.is_none())
            .count();
        active_count == graph.nodes.len()
    }

    fn validate_config(&self, config: &IntegrationConfig) -> Result<(), IntegrationError> {
        config.validate()
    }

    fn snapshot(&self) -> IntegrationSnapshot {
        let graph = self.graph.borrow();
        IntegrationSnapshot {
            version: graph.version,
            timestamp: 0,
            nodes: graph.nodes.keys().cloned().collect(),
            registrations: graph.registrations.clone(),
            stats: self.stats(),
        }
    }

    fn restore_from_snapshot(
        &self,
        snapshot: IntegrationSnapshot,
    ) -> Result<(), IntegrationError> {
        let mut graph = self.graph.borrow_mut();
        graph.version = snapshot.version;
        graph.registrations = snapshot.registrations;

        for registration in &snapshot.registrations {
            graph.nodes.entry(registration.node_id).or_insert(InternalNodeState {
                node_id: registration.node_id,
                endpoint: None,
                registrations: HashMap::new(),
                started_at: None,
                stopped_at: None,
            });
        }

        Ok(())
    }

    fn shutdown_all(&self) -> Result<IntegrationShutdownResult, IntegrationError> {
        let mut graph = self.graph.borrow_mut();
        let nodes_stopped = graph.nodes.len() as u32;
        let contracts_stopped = graph.registrations.len() as u32;
        let completed_at = 0u64;

        for node in graph.nodes.values_mut() {
            node.stopped_at = Some(completed_at);
        }

        graph.add_event(
            IntegrationEventType::NodeStopped,
            None,
            None,
        );
        graph.version += 1;

        Ok(IntegrationShutdownResult {
            nodes_stopped,
            contracts_stopped,
            shutdown_duration_ms: 0,
            completed_at,
        })
    }

    fn contract_count_by_type(&self) -> HashMap<String, u32> {
        let graph = self.graph.borrow();
        let mut counts: HashMap<String, u32> = HashMap::new();
        for registration in &graph.registrations {
            *counts.entry(registration.contract_type.as_str().to_string()).or_insert(0) += 1;
        }
        counts
    }

    fn node_contract_coverage(
        &self,
        node_id: NodeId,
    ) -> Result<ContractCoverage, IntegrationError> {
        let graph = self.graph.borrow();
        let node = graph
            .nodes
            .get(&node_id)
            .ok_or_else(|| IntegrationError::NodeNotFound { node_id })?;

        let all_types = ContractType::all_types();
        let mut active_contracts = Vec::new();
        let mut missing_contracts = Vec::new();

        for contract_type in &all_types {
            if node
                .registrations
                .get(contract_type)
                .map(|r| r.status == RegistrationStatus::Active)
                .unwrap_or(false)
            {
                active_contracts.push(*contract_type);
            } else {
                missing_contracts.push(*contract_type);
            }
        }

        let coverage_percent = if all_types.is_empty() {
            100.0
        } else {
            active_contracts.len() as f64 / all_types.len() as f64 * 100.0
        };

        Ok(ContractCoverage {
            node_id,
            active_contracts,
            missing_contracts,
            coverage_percent,
            is_complete: missing_contracts.is_empty(),
        })
    }

    /// Internal helper to register a contract on a node.
    fn register_contract(
        &self,
        node_id: NodeId,
        contract_type: ContractType,
        endpoint: Endpoint,
    ) -> Result<IntegrationRegistration, IntegrationError> {
        let config = self.config.borrow();
        let mut graph = self.graph.borrow_mut();

        if graph.nodes.len() as u32 >= config.max_nodes
            && !graph.nodes.contains_key(&node_id)
        {
            return Err(IntegrationError::MaxNodesExceeded {
                max_nodes: config.max_nodes,
            });
        }

        let node_state = graph
            .nodes
            .entry(node_id)
            .or_insert(InternalNodeState {
                node_id,
                endpoint: Some(endpoint.clone()),
                registrations: HashMap::new(),
                started_at: None,
                stopped_at: None,
            });

        if node_state.registrations.contains_key(&contract_type) {
            return Err(IntegrationError::ContractAlreadyRegistered {
                contract: contract_type,
                node_id,
            });
        }

        if node_state.registrations.len() as u32 >= config.max_contracts_per_node {
            return Err(IntegrationError::MaxContractsPerNodeExceeded { node_id });
        }

        graph.registration_counter += 1;
        let registration_id = graph.registration_counter;

        let registration = InternalRegistration {
            node_id,
            contract_type,
            registered_at: 0,
            registration_id,
            status: RegistrationStatus::Active,
        };

        let public_registration = IntegrationRegistration {
            node_id,
            contract_type,
            registered_at: 0,
            registration_id,
            status: RegistrationStatus::Active,
        };

        node_state.registrations.insert(contract_type, registration);
        graph.registrations.push(public_registration.clone());
        graph.add_event(
            IntegrationEventType::ContractRegistered,
            Some(node_id),
            Some(contract_type),
        );
        graph.version += 1;

        drop(config);

        Ok(public_registration)
    }

    /// Get the contract registration for a specific node and contract type.
    pub fn get_registration(
        &self,
        node_id: NodeId,
        contract_type: ContractType,
    ) -> Option<IntegrationRegistration> {
        let graph = self.graph.borrow();
        graph
            .registrations
            .iter()
            .find(|r| r.node_id == node_id && r.contract_type == contract_type)
            .cloned()
    }

    /// Get all registrations for a node.
    pub fn get_node_registrations(&self, node_id: NodeId) -> Vec<IntegrationRegistration> {
        let graph = self.graph.borrow();
        graph
            .registrations
            .iter()
            .filter(|r| r.node_id == node_id)
            .cloned()
            .collect()
    }

    /// Get all registrations for a contract type.
    pub fn get_contract_registrations(
        &self,
        contract_type: ContractType,
    ) -> Vec<IntegrationRegistration> {
        let graph = self.graph.borrow();
        graph
            .registrations
            .iter()
            .filter(|r| r.contract_type == contract_type)
            .cloned()
            .collect()
    }

    /// Unregister a contract from a node.
    pub fn unregister_contract(
        &self,
        node_id: NodeId,
        contract_type: ContractType,
    ) -> Result<(), IntegrationError> {
        let mut graph = self.graph.borrow_mut();

        let node = graph
            .nodes
            .get_mut(&node_id)
            .ok_or_else(|| IntegrationError::NodeNotFound { node_id })?;

        if !node.registrations.contains_key(&contract_type) {
            return Err(IntegrationError::ContractNotRegistered {
                contract: contract_type,
                node_id,
            });
        }

        node.registrations.remove(&contract_type);
        graph.registrations.retain(|r| {
            !(r.node_id == node_id && r.contract_type == contract_type)
        });
        graph.add_event(
            IntegrationEventType::ContractUnregistered,
            Some(node_id),
            Some(contract_type),
        );
        graph.version += 1;

        Ok(())
    }

    /// Get the endpoint for a node.
    pub fn node_endpoint(&self, node_id: NodeId) -> Option<Endpoint> {
        let graph = self.graph.borrow();
        graph.nodes.get(&node_id).and_then(|n| n.endpoint.clone())
    }

    /// Set the endpoint for a node.
    pub fn set_node_endpoint(
        &self,
        node_id: NodeId,
        endpoint: Endpoint,
    ) -> Result<(), IntegrationError> {
        let mut graph = self.graph.borrow_mut();

        let node = graph
            .nodes
            .get_mut(&node_id)
            .ok_or_else(|| IntegrationError::NodeNotFound { node_id })?;

        node.endpoint = Some(endpoint);
        graph.version += 1;

        Ok(())
    }

    /// Get all endpoints across all nodes.
    pub fn all_endpoints(&self) -> Vec<(NodeId, Endpoint)> {
        let graph = self.graph.borrow();
        graph
            .nodes
            .iter()
            .filter_map(|(id, n)| n.endpoint.as_ref().map(|e| (*id, e.clone())))
            .collect()
    }

    /// Get the total registration count.
    pub fn total_registrations(&self) -> u64 {
        let graph = self.graph.borrow();
        graph.registrations.len() as u64
    }

    /// Get the total operation count.
    pub fn total_operations(&self) -> u64 {
        let graph = self.graph.borrow();
        graph.operation_counter
    }

    /// Get the total event count.
    pub fn total_events(&self) -> u64 {
        let graph = self.graph.borrow();
        graph.event_counter
    }

    /// Get the current integration version.
    pub fn integration_version(&self) -> u64 {
        let graph = self.graph.borrow();
        graph.version
    }

    /// Check if a node exists.
    pub fn node_exists(&self, node_id: NodeId) -> bool {
        let graph = self.graph.borrow();
        graph.nodes.contains_key(&node_id)
    }

    /// Check if a contract is registered on a node.
    pub fn is_contract_registered(
        &self,
        node_id: NodeId,
        contract_type: ContractType,
    ) -> bool {
        let graph = self.graph.borrow();
        graph
            .nodes
            .get(&node_id)
            .map(|n| n.registrations.contains_key(&contract_type))
            .unwrap_or(false)
    }

    /// Get the integration config.
    pub fn config(&self) -> IntegrationConfig {
        self.config.borrow().clone()
    }

    /// Clear all integration data.
    pub fn clear_all(&self) {
        let mut graph = self.graph.borrow_mut();
        graph.nodes.clear();
        graph.registrations.clear();
        graph.events.clear();
        graph.event_counter = 0;
        graph.registration_counter = 0;
        graph.operation_counter = 0;
        graph.version = 0;
    }

    /// Get the integration graph size.
    pub fn graph_size(&self) -> (usize, usize, usize) {
        let graph = self.graph.borrow();
        (
            graph.nodes.len(),
            graph.registrations.len(),
            graph.events.len(),
        )
    }

    /// Get nodes sorted by registration count.
    pub fn nodes_by_registration_count(&self) -> Vec<(NodeId, u32)> {
        let graph = self.graph.borrow();
        let mut counts: Vec<(NodeId, u32)> = graph
            .nodes
            .iter()
            .map(|(id, n)| (*id, n.registrations.len() as u32))
            .collect();
        counts.sort_by(|a, b| b.1.cmp(&a.1));
        counts
    }

    /// Get contract types sorted by registration count.
    pub fn contract_types_by_count(&self) -> Vec<(ContractType, u32)> {
        let graph = self.graph.borrow();
        let mut counts: HashMap<ContractType, u32> = HashMap::new();
        for registration in &graph.registrations {
            *counts.entry(registration.contract_type).or_insert(0) += 1;
        }
        let mut result: Vec<(ContractType, u32)> = counts.into_iter().collect();
        result.sort_by(|a, b| b.1.cmp(&a.1));
        result
    }

    /// Get recently registered contracts (last N).
    pub fn recent_registrations(&self, count: u64) -> Vec<IntegrationRegistration> {
        let graph = self.graph.borrow();
        let start = graph
            .registrations
            .len()
            .saturating_sub(count as usize);
        graph.registrations[start..].to_vec()
    }

    /// Get registrations by status.
    pub fn registrations_by_status(
        &self,
        status: RegistrationStatus,
    ) -> Vec<IntegrationRegistration> {
        let graph = self.graph.borrow();
        graph
            .registrations
            .iter()
            .filter(|r| r.status == status)
            .cloned()
            .collect()
    }

    /// Get active registration count.
    pub fn active_registration_count(&self) -> u64 {
        let graph = self.graph.borrow();
        graph
            .registrations
            .iter()
            .filter(|r| r.status == RegistrationStatus::Active)
            .count() as u64
    }

    /// Get failed registration count.
    pub fn failed_registration_count(&self) -> u64 {
        let graph = self.graph.borrow();
        graph
            .registrations
            .iter()
            .filter(|r| r.status == RegistrationStatus::Failed)
            .count() as u64
    }

    /// Get pending registration count.
    pub fn pending_registration_count(&self) -> u64 {
        let graph = self.graph.borrow();
        graph
            .registrations
            .iter()
            .filter(|r| r.status == RegistrationStatus::Pending)
            .count() as u64
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

    fn test_node_id3() -> NodeId {
        test_node_id(3)
    }

    fn test_node_id4() -> NodeId {
        test_node_id(4)
    }

    fn test_endpoint() -> Endpoint {
        Endpoint::new(CarrierType::Direct, vec![1, 2, 3, 4])
    }

    fn test_endpoint2() -> Endpoint {
        Endpoint::new(CarrierType::Direct, vec![5, 6, 7, 8])
    }

    fn test_config() -> IntegrationConfig {
        IntegrationConfig::default()
    }

    // ========================================================================
    // IntegrationConfig tests
    // ========================================================================

    #[test]
    fn integration_config_default() {
        let config = IntegrationConfig::default();
        assert_eq!(config.max_nodes, 1000);
        assert_eq!(config.max_contracts_per_node, 25);
        assert!(config.enable_operation_chaining);
        assert_eq!(config.operation_chain_timeout_ms, 30000);
        assert!(config.enable_auto_startup);
        assert_eq!(config.node_startup_timeout_ms, 10000);
        assert!(config.enable_health_checking);
        assert_eq!(config.health_check_interval_ms, 5000);
        assert!(config.enable_event_recording);
        assert_eq!(config.max_recorded_events, 10000);
        assert!(config.enable_snapshot_restore);
        assert_eq!(config.snapshot_interval_ms, 60000);
        assert!(config.enable_strict_validation);
    }

    #[test]
    fn integration_config_new_valid() {
        let config = IntegrationConfig::new(500).unwrap();
        assert_eq!(config.max_nodes, 500);
    }

    #[test]
    fn integration_config_new_zero_nodes() {
        let result = IntegrationConfig::new(0);
        assert!(result.is_err());
    }

    #[test]
    fn integration_config_validate_zero_max_nodes() {
        let mut config = IntegrationConfig::default();
        config.max_nodes = 0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn integration_config_validate_zero_max_contracts() {
        let mut config = IntegrationConfig::default();
        config.max_contracts_per_node = 0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn integration_config_validate_zero_chain_timeout() {
        let mut config = IntegrationConfig::default();
        config.operation_chain_timeout_ms = 0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn integration_config_validate_zero_startup_timeout() {
        let mut config = IntegrationConfig::default();
        config.node_startup_timeout_ms = 0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn integration_config_validate_zero_health_interval() {
        let mut config = IntegrationConfig::default();
        config.health_check_interval_ms = 0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn integration_config_validate_zero_max_events() {
        let mut config = IntegrationConfig::default();
        config.max_recorded_events = 0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn integration_config_validate_zero_snapshot_interval() {
        let mut config = IntegrationConfig::default();
        config.snapshot_interval_ms = 0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn integration_config_strict() {
        let config = IntegrationConfig::strict();
        assert_eq!(config.max_nodes, 100);
        assert_eq!(config.max_contracts_per_node, 10);
        assert!(!config.enable_auto_startup);
        assert_eq!(config.operation_chain_timeout_ms, 5000);
        assert_eq!(config.node_startup_timeout_ms, 5000);
        assert_eq!(config.health_check_interval_ms, 1000);
        assert_eq!(config.max_recorded_events, 1000);
        assert_eq!(config.snapshot_interval_ms, 10000);
    }

    #[test]
    fn integration_config_lenient() {
        let config = IntegrationConfig::lenient();
        assert_eq!(config.max_nodes, 10000);
        assert_eq!(config.max_contracts_per_node, 100);
        assert!(!config.enable_operation_chaining);
        assert!(!config.enable_health_checking);
        assert!(!config.enable_event_recording);
        assert!(!config.enable_snapshot_restore);
        assert!(!config.enable_strict_validation);
    }

    #[test]
    fn integration_config_contract_density() {
        let config = IntegrationConfig::default();
        let density = config.contract_density();
        assert!(density >= 0.0);
        assert!(density <= 100.0);
    }

    #[test]
    fn integration_config_serialization_roundtrip() {
        let config = IntegrationConfig::default();
        let json = serde_json::to_string(&config).unwrap();
        let deserialized: IntegrationConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(config.max_nodes, deserialized.max_nodes);
        assert_eq!(config.max_contracts_per_node, deserialized.max_contracts_per_node);
    }

    // ========================================================================
    // ContractType tests
    // ========================================================================

    #[test]
    fn contract_type_as_str() {
        assert_eq!(ContractType::Carrier.as_str(), "carrier");
        assert_eq!(ContractType::Transport.as_str(), "transport");
        assert_eq!(ContractType::Store.as_str(), "store");
        assert_eq!(ContractType::Session.as_str(), "session");
        assert_eq!(ContractType::Routing.as_str(), "routing");
        assert_eq!(ContractType::Discovery.as_str(), "discovery");
        assert_eq!(ContractType::Membership.as_str(), "membership");
        assert_eq!(ContractType::Placement.as_str(), "placement");
        assert_eq!(ContractType::Replication.as_str(), "replication");
        assert_eq!(ContractType::Recovery.as_str(), "recovery");
        assert_eq!(ContractType::Consistency.as_str(), "consistency");
        assert_eq!(ContractType::Ordering.as_str(), "ordering");
        assert_eq!(ContractType::Security.as_str(), "security");
        assert_eq!(ContractType::Client.as_str(), "client");
        assert_eq!(ContractType::FaultTolerance.as_str(), "fault_tolerance");
        assert_eq!(ContractType::Broadcast.as_str(), "broadcast");
        assert_eq!(ContractType::Quorum.as_str(), "quorum");
        assert_eq!(ContractType::Snapshot.as_str(), "snapshot");
        assert_eq!(ContractType::Byzantine.as_str(), "byzantine");
        assert_eq!(ContractType::Lease.as_str(), "lease");
        assert_eq!(ContractType::CRDT.as_str(), "crdt");
        assert_eq!(ContractType::Erasure.as_str(), "erasure");
        assert_eq!(ContractType::Saga.as_str(), "saga");
        assert_eq!(ContractType::Coordinator.as_str(), "coordinator");
        assert_eq!(ContractType::Durability.as_str(), "durability");
        assert_eq!(ContractType::Topology.as_str(), "topology");
    }

    #[test]
    fn contract_type_all_types_count() {
        let types = ContractType::all_types();
        assert_eq!(types.len(), 26);
    }

    #[test]
    fn contract_type_serialization_roundtrip() {
        let ct = ContractType::CRDT;
        let json = serde_json::to_string(&ct).unwrap();
        let deserialized: ContractType = serde_json::from_str(&json).unwrap();
        assert_eq!(ct, deserialized);
    }

    // ========================================================================
    // RegistrationStatus tests
    // ========================================================================

    #[test]
    fn registration_status_is_active() {
        assert!(RegistrationStatus::Active.is_active());
        assert!(!RegistrationStatus::Pending.is_active());
        assert!(!RegistrationStatus::Failed.is_active());
        assert!(!RegistrationStatus::Stopped.is_active());
    }

    #[test]
    fn registration_status_display_name() {
        assert_eq!(RegistrationStatus::Pending.display_name(), "pending");
        assert_eq!(RegistrationStatus::Active.display_name(), "active");
        assert_eq!(RegistrationStatus::Degraded.display_name(), "degraded");
        assert_eq!(RegistrationStatus::Failed.display_name(), "failed");
        assert_eq!(RegistrationStatus::Stopped.display_name(), "stopped");
    }

    #[test]
    fn registration_status_serialization_roundtrip() {
        let status = RegistrationStatus::Degraded;
        let json = serde_json::to_string(&status).unwrap();
        let deserialized: RegistrationStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(status, deserialized);
    }

    // ========================================================================
    // IntegrationRegistration tests
    // ========================================================================

    #[test]
    fn integration_registration_new() {
        let reg = IntegrationRegistration {
            node_id: test_node_id1(),
            contract_type: ContractType::Store,
            registered_at: 1000,
            registration_id: 42,
            status: RegistrationStatus::Active,
        };
        assert_eq!(reg.node_id, test_node_id1());
        assert_eq!(reg.contract_type, ContractType::Store);
        assert_eq!(reg.registered_at, 1000);
        assert_eq!(reg.registration_id, 42);
        assert!(reg.status.is_active());
    }

    #[test]
    fn integration_registration_serialization_roundtrip() {
        let reg = IntegrationRegistration {
            node_id: test_node_id2(),
            contract_type: ContractType::CRDT,
            registered_at: 5000,
            registration_id: 100,
            status: RegistrationStatus::Degraded,
        };
        let json = serde_json::to_string(&reg).unwrap();
        let deserialized: IntegrationRegistration = serde_json::from_str(&json).unwrap();
        assert_eq!(reg, deserialized);
    }

    // ========================================================================
    // NodeRegistrationStatus tests
    // ========================================================================

    #[test]
    fn node_registration_status_coverage_percent() {
        let mut contracts = HashMap::new();
        contracts.insert(ContractType::Store, RegistrationStatus::Active);
        contracts.insert(ContractType::Transport, RegistrationStatus::Active);
        contracts.insert(ContractType::Security, RegistrationStatus::Failed);
        let status = NodeRegistrationStatus {
            node_id: test_node_id1(),
            registered_contracts: contracts,
            active_contract_count: 2,
            total_contract_count: 3,
            is_fully_registered: false,
            last_registration_at: 1000,
        };
        assert!((status.coverage_percent() - 66.67).abs() < 0.01);
    }

    #[test]
    fn node_registration_status_coverage_percent_zero_total() {
        let status = NodeRegistrationStatus {
            node_id: test_node_id1(),
            registered_contracts: HashMap::new(),
            active_contract_count: 0,
            total_contract_count: 0,
            is_fully_registered: true,
            last_registration_at: 0,
        };
        assert!((status.coverage_percent() - 100.0).abs() < 0.01);
    }

    #[test]
    fn node_registration_status_has_contract() {
        let mut contracts = HashMap::new();
        contracts.insert(ContractType::Store, RegistrationStatus::Active);
        contracts.insert(ContractType::Transport, RegistrationStatus::Failed);
        let status = NodeRegistrationStatus {
            node_id: test_node_id1(),
            registered_contracts: contracts,
            active_contract_count: 1,
            total_contract_count: 2,
            is_fully_registered: false,
            last_registration_at: 1000,
        };
        assert!(status.has_contract(ContractType::Store));
        assert!(!status.has_contract(ContractType::Transport));
        assert!(!status.has_contract(ContractType::Security));
    }

    #[test]
    fn node_registration_status_serialization_roundtrip() {
        let mut contracts = HashMap::new();
        contracts.insert(ContractType::Store, RegistrationStatus::Active);
        let status = NodeRegistrationStatus {
            node_id: test_node_id1(),
            registered_contracts: contracts,
            active_contract_count: 1,
            total_contract_count: 1,
            is_fully_registered: true,
            last_registration_at: 1000,
        };
        let json = serde_json::to_string(&status).unwrap();
        let deserialized: NodeRegistrationStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(status.active_contract_count, deserialized.active_contract_count);
    }

    // ========================================================================
    // ContractCoverage tests
    // ========================================================================

    #[test]
    fn contract_coverage_is_complete() {
        let coverage = ContractCoverage {
            node_id: test_node_id1(),
            active_contracts: vec![ContractType::Store, ContractType::Transport],
            missing_contracts: vec![],
            coverage_percent: 100.0,
            is_complete: true,
        };
        assert!(coverage.is_complete);
    }

    #[test]
    fn contract_coverage_not_complete() {
        let coverage = ContractCoverage {
            node_id: test_node_id1(),
            active_contracts: vec![ContractType::Store],
            missing_contracts: vec![ContractType::Transport],
            coverage_percent: 50.0,
            is_complete: false,
        };
        assert!(!coverage.is_complete);
    }

    #[test]
    fn contract_coverage_serialization_roundtrip() {
        let coverage = ContractCoverage {
            node_id: test_node_id1(),
            active_contracts: vec![ContractType::CRDT],
            missing_contracts: vec![ContractType::Erasure],
            coverage_percent: 50.0,
            is_complete: false,
        };
        let json = serde_json::to_string(&coverage).unwrap();
        let deserialized: ContractCoverage = serde_json::from_str(&json).unwrap();
        assert_eq!(coverage.coverage_percent, deserialized.coverage_percent);
    }

    // ========================================================================
    // IntegratedOperationType tests
    // ========================================================================

    #[test]
    fn integrated_operation_type_as_str() {
        assert_eq!(IntegratedOperationType::Write.as_str(), "write");
        assert_eq!(IntegratedOperationType::Read.as_str(), "read");
        assert_eq!(IntegratedOperationType::Delete.as_str(), "delete");
        assert_eq!(IntegratedOperationType::Replicate.as_str(), "replicate");
        assert_eq!(IntegratedOperationType::Recover.as_str(), "recover");
        assert_eq!(IntegratedOperationType::Broadcast.as_str(), "broadcast");
        assert_eq!(IntegratedOperationType::Commit.as_str(), "commit");
        assert_eq!(IntegratedOperationType::Abort.as_str(), "abort");
        assert_eq!(IntegratedOperationType::Snapshot.as_str(), "snapshot");
        assert_eq!(IntegratedOperationType::Restore.as_str(), "restore");
        assert_eq!(IntegratedOperationType::Discover.as_str(), "discover");
        assert_eq!(IntegratedOperationType::Join.as_str(), "join");
        assert_eq!(IntegratedOperationType::Leave.as_str(), "leave");
        assert_eq!(IntegratedOperationType::HealthCheck.as_str(), "health_check");
        assert_eq!(IntegratedOperationType::Rebalance.as_str(), "rebalance");
    }

    #[test]
    fn integrated_operation_type_required_contracts_write() {
        let contracts = IntegratedOperationType::Write.required_contracts();
        assert!(contracts.contains(&ContractType::Security));
        assert!(contracts.contains(&ContractType::Store));
        assert!(contracts.contains(&ContractType::Replication));
        assert!(contracts.contains(&ContractType::Consistency));
        assert!(contracts.contains(&ContractType::Ordering));
        assert!(contracts.contains(&ContractType::Durability));
    }

    #[test]
    fn integrated_operation_type_required_contracts_read() {
        let contracts = IntegratedOperationType::Read.required_contracts();
        assert!(contracts.contains(&ContractType::Security));
        assert!(contracts.contains(&ContractType::Store));
        assert!(contracts.contains(&ContractType::Consistency));
        assert!(contracts.contains(&ContractType::Routing));
    }

    #[test]
    fn integrated_operation_type_required_contracts_broadcast() {
        let contracts = IntegratedOperationType::Broadcast.required_contracts();
        assert!(contracts.contains(&ContractType::Broadcast));
        assert!(contracts.contains(&ContractType::Transport));
        assert!(contracts.contains(&ContractType::Membership));
    }

    #[test]
    fn integrated_operation_type_required_contracts_join() {
        let contracts = IntegratedOperationType::Join.required_contracts();
        assert!(contracts.contains(&ContractType::Membership));
        assert!(contracts.contains(&ContractType::Discovery));
        assert!(contracts.contains(&ContractType::Topology));
        assert!(contracts.contains(&ContractType::Security));
    }

    #[test]
    fn integrated_operation_type_serialization_roundtrip() {
        let op_type = IntegratedOperationType::Rebalance;
        let json = serde_json::to_string(&op_type).unwrap();
        let deserialized: IntegratedOperationType = serde_json::from_str(&json).unwrap();
        assert_eq!(op_type, deserialized);
    }

    // ========================================================================
    // IntegratedOperation tests
    // ========================================================================

    #[test]
    fn integrated_operation_new() {
        let op = IntegratedOperation {
            operation_id: 1,
            operation_type: IntegratedOperationType::Write,
            source_node: test_node_id1(),
            target_nodes: vec![test_node_id2(), test_node_id3()],
            dataset_id: None,
            contract_chain: vec![
                ContractType::Security,
                ContractType::Store,
                ContractType::Replication,
            ],
            payload: vec![1, 2, 3],
            timeout_ms: 5000,
            priority: 5,
            created_at: 1000,
        };
        assert_eq!(op.operation_id, 1);
        assert_eq!(op.operation_type, IntegratedOperationType::Write);
        assert_eq!(op.source_node, test_node_id1());
        assert_eq!(op.target_nodes.len(), 2);
        assert_eq!(op.contract_chain.len(), 3);
        assert_eq!(op.priority, 5);
    }

    #[test]
    fn integrated_operation_serialization_roundtrip() {
        let op = IntegratedOperation {
            operation_id: 42,
            operation_type: IntegratedOperationType::Snapshot,
            source_node: test_node_id1(),
            target_nodes: vec![],
            dataset_id: None,
            contract_chain: vec![ContractType::Snapshot],
            payload: vec![],
            timeout_ms: 10000,
            priority: 1,
            created_at: 0,
        };
        let json = serde_json::to_string(&op).unwrap();
        let deserialized: IntegratedOperation = serde_json::from_str(&json).unwrap();
        assert_eq!(op.operation_id, deserialized.operation_id);
        assert_eq!(op.operation_type, deserialized.operation_type);
    }

    // ========================================================================
    // IntegratedOperationResult tests
    // ========================================================================

    #[test]
    fn integrated_operation_result_success() {
        let mut contract_results = HashMap::new();
        contract_results.insert(
            ContractType::Store,
            ContractExecutionResult::success(10, vec![1]),
        );
        let result = IntegratedOperationResult {
            operation_id: 1,
            success: true,
            contract_results,
            total_duration_ms: 10,
            completed_at: 1000,
            error: None,
        };
        assert!(result.success);
        assert_eq!(result.operation_id, 1);
        assert_eq!(result.total_duration_ms, 10);
    }

    #[test]
    fn integrated_operation_result_failure() {
        let result = IntegratedOperationResult {
            operation_id: 2,
            success: false,
            contract_results: HashMap::new(),
            total_duration_ms: 100,
            completed_at: 2000,
            error: Some(IntegrationError::OperationFailed { operation_id: 2 }),
        };
        assert!(!result.success);
        assert!(result.error.is_some());
    }

    #[test]
    fn integrated_operation_result_serialization_roundtrip() {
        let result = IntegratedOperationResult {
            operation_id: 3,
            success: true,
            contract_results: HashMap::new(),
            total_duration_ms: 50,
            completed_at: 3000,
            error: None,
        };
        let json = serde_json::to_string(&result).unwrap();
        let deserialized: IntegratedOperationResult = serde_json::from_str(&json).unwrap();
        assert_eq!(result.operation_id, deserialized.operation_id);
    }

    // ========================================================================
    // ContractExecutionResult tests
    // ========================================================================

    #[test]
    fn contract_execution_result_success() {
        let result = ContractExecutionResult::success(5, vec![1, 2, 3]);
        assert!(result.success);
        assert_eq!(result.duration_ms, 5);
        assert_eq!(result.output, vec![1, 2, 3]);
        assert!(result.error.is_none());
    }

    #[test]
    fn contract_execution_result_failure() {
        let result = ContractExecutionResult::failure(10, "timeout".to_string());
        assert!(!result.success);
        assert_eq!(result.duration_ms, 10);
        assert!(result.output.is_empty());
        assert_eq!(result.error, Some("timeout".to_string()));
    }

    #[test]
    fn contract_execution_result_serialization_roundtrip() {
        let result = ContractExecutionResult::success(7, vec![42]);
        let json = serde_json::to_string(&result).unwrap();
        let deserialized: ContractExecutionResult = serde_json::from_str(&json).unwrap();
        assert_eq!(result.success, deserialized.success);
    }

    // ========================================================================
    // IntegrationStartResult / StopResult / ShutdownResult tests
    // ========================================================================

    #[test]
    fn integration_start_result_new() {
        let result = IntegrationStartResult {
            node_id: test_node_id1(),
            started_at: 1000,
            contracts_started: 5,
            contracts_failed: 0,
            startup_duration_ms: 100,
        };
        assert_eq!(result.node_id, test_node_id1());
        assert_eq!(result.contracts_started, 5);
        assert_eq!(result.contracts_failed, 0);
    }

    #[test]
    fn integration_stop_result_new() {
        let result = IntegrationStopResult {
            node_id: test_node_id1(),
            stopped_at: 5000,
            contracts_stopped: 5,
            stop_duration_ms: 50,
        };
        assert_eq!(result.node_id, test_node_id1());
        assert_eq!(result.contracts_stopped, 5);
    }

    #[test]
    fn integration_shutdown_result_new() {
        let result = IntegrationShutdownResult {
            nodes_stopped: 10,
            contracts_stopped: 50,
            shutdown_duration_ms: 500,
            completed_at: 10000,
        };
        assert_eq!(result.nodes_stopped, 10);
        assert_eq!(result.contracts_stopped, 50);
    }

    // ========================================================================
    // IntegrationEventType tests
    // ========================================================================

    #[test]
    fn integration_event_type_as_str() {
        assert_eq!(
            IntegrationEventType::ContractRegistered.as_str(),
            "contract_registered"
        );
        assert_eq!(
            IntegrationEventType::NodeStarted.as_str(),
            "node_started"
        );
        assert_eq!(
            IntegrationEventType::NodeStopped.as_str(),
            "node_stopped"
        );
        assert_eq!(
            IntegrationEventType::OperationStarted.as_str(),
            "operation_started"
        );
        assert_eq!(
            IntegrationEventType::OperationCompleted.as_str(),
            "operation_completed"
        );
        assert_eq!(
            IntegrationEventType::OperationFailed.as_str(),
            "operation_failed"
        );
        assert_eq!(
            IntegrationEventType::HealthCheckPassed.as_str(),
            "health_check_passed"
        );
        assert_eq!(
            IntegrationEventType::HealthCheckFailed.as_str(),
            "health_check_failed"
        );
        assert_eq!(
            IntegrationEventType::SnapshotCreated.as_str(),
            "snapshot_created"
        );
        assert_eq!(
            IntegrationEventType::SnapshotRestored.as_str(),
            "snapshot_restored"
        );
    }

    #[test]
    fn integration_event_type_serialization_roundtrip() {
        let event_type = IntegrationEventType::ContractFailed;
        let json = serde_json::to_string(&event_type).unwrap();
        let deserialized: IntegrationEventType = serde_json::from_str(&json).unwrap();
        assert_eq!(event_type, deserialized);
    }

    // ========================================================================
    // IntegrationEvent tests
    // ========================================================================

    #[test]
    fn integration_event_new() {
        let event = IntegrationEvent {
            event_id: 1,
            event_type: IntegrationEventType::ContractRegistered,
            node_id: Some(test_node_id1()),
            contract_type: Some(ContractType::Store),
            timestamp: 1000,
            details: vec![],
        };
        assert_eq!(event.event_id, 1);
        assert_eq!(event.event_type, IntegrationEventType::ContractRegistered);
        assert_eq!(event.node_id, Some(test_node_id1()));
    }

    #[test]
    fn integration_event_serialization_roundtrip() {
        let event = IntegrationEvent {
            event_id: 42,
            event_type: IntegrationEventType::NodeStopped,
            node_id: None,
            contract_type: None,
            timestamp: 5000,
            details: vec![1, 2],
        };
        let json = serde_json::to_string(&event).unwrap();
        let deserialized: IntegrationEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(event, deserialized);
    }

    // ========================================================================
    // IntegrationStats tests
    // ========================================================================

    #[test]
    fn integration_stats_new() {
        let stats = IntegrationStats::new();
        assert_eq!(stats.total_nodes, 0);
        assert_eq!(stats.active_nodes, 0);
        assert_eq!(stats.total_operations, 0);
        assert_eq!(stats.successful_operations, 0);
        assert_eq!(stats.failed_operations, 0);
        assert_eq!(stats.total_events, 0);
    }

    #[test]
    fn integration_stats_operation_success_rate() {
        let mut stats = IntegrationStats::new();
        stats.total_operations = 100;
        stats.successful_operations = 85;
        stats.failed_operations = 15;
        assert!((stats.operation_success_rate() - 85.0).abs() < 0.01);
    }

    #[test]
    fn integration_stats_operation_success_rate_zero_ops() {
        let stats = IntegrationStats::new();
        assert!((stats.operation_success_rate() - 100.0).abs() < 0.01);
    }

    #[test]
    fn integration_stats_contract_execution_success_rate() {
        let mut stats = IntegrationStats::new();
        stats.total_contract_executions = 200;
        stats.successful_contract_executions = 180;
        stats.failed_contract_executions = 20;
        assert!((stats.contract_execution_success_rate() - 90.0).abs() < 0.01);
    }

    #[test]
    fn integration_stats_node_availability_percent() {
        let mut stats = IntegrationStats::new();
        stats.total_nodes = 10;
        stats.active_nodes = 8;
        stats.stopped_nodes = 2;
        assert!((stats.node_availability_percent() - 80.0).abs() < 0.01);
    }

    #[test]
    fn integration_stats_node_availability_percent_zero_nodes() {
        let stats = IntegrationStats::new();
        assert!((stats.node_availability_percent() - 100.0).abs() < 0.01);
    }

    #[test]
    fn integration_stats_serialization_roundtrip() {
        let stats = IntegrationStats::new();
        let json = serde_json::to_string(&stats).unwrap();
        let deserialized: IntegrationStats = serde_json::from_str(&json).unwrap();
        assert_eq!(stats.total_nodes, deserialized.total_nodes);
    }

    // ========================================================================
    // IntegrationSnapshot tests
    // ========================================================================

    #[test]
    fn integration_snapshot_node_count() {
        let snapshot = IntegrationSnapshot {
            version: 1,
            timestamp: 1000,
            nodes: vec![test_node_id1(), test_node_id2()],
            registrations: vec![],
            stats: IntegrationStats::new(),
        };
        assert_eq!(snapshot.node_count(), 2);
    }

    #[test]
    fn integration_snapshot_registration_count() {
        let snapshot = IntegrationSnapshot {
            version: 1,
            timestamp: 1000,
            nodes: vec![],
            registrations: vec![],
            stats: IntegrationStats::new(),
        };
        assert_eq!(snapshot.registration_count(), 0);
    }

    #[test]
    fn integration_snapshot_serialization_roundtrip() {
        let snapshot = IntegrationSnapshot {
            version: 1,
            timestamp: 1000,
            nodes: vec![test_node_id1()],
            registrations: vec![],
            stats: IntegrationStats::new(),
        };
        let json = serde_json::to_string(&snapshot).unwrap();
        let deserialized: IntegrationSnapshot = serde_json::from_str(&json).unwrap();
        assert_eq!(snapshot.version, deserialized.version);
    }

    // ========================================================================
    // IntegrationError tests
    // ========================================================================

    #[test]
    fn integration_error_category() {
        assert_eq!(
            IntegrationError::InvalidConfig {
                field: "x".to_string(),
                reason: "y".to_string(),
            }
            .category(),
            "invalid_config"
        );
        assert_eq!(
            IntegrationError::NodeNotFound {
                node_id: test_node_id1(),
            }
            .category(),
            "node_not_found"
        );
        assert_eq!(
            IntegrationError::ContractNotRegistered {
                contract: ContractType::Store,
                node_id: test_node_id1(),
            }
            .category(),
            "contract_not_registered"
        );
    }

    #[test]
    fn integration_error_is_recoverable() {
        assert!(IntegrationError::OperationFailed { operation_id: 1 }.is_recoverable());
        assert!(IntegrationError::OperationTimeout { operation_id: 1 }.is_recoverable());
        assert!(IntegrationError::InsufficientContracts {
            missing: vec![ContractType::Store]
        }
        .is_recoverable());
        assert!(!IntegrationError::NodeNotFound {
            node_id: test_node_id1(),
        }
        .is_recoverable());
        assert!(!IntegrationError::InvalidConfig {
            field: "x".to_string(),
            reason: "y".to_string(),
        }
        .is_recoverable());
    }

    #[test]
    fn integration_error_suggested_action() {
        assert_eq!(
            IntegrationError::NodeNotFound {
                node_id: test_node_id1(),
            }
            .suggested_action(),
            "register the node first"
        );
        assert!(!IntegrationError::Internal {
            message: "test".to_string()
        }
        .suggested_action()
        .is_empty());
    }

    #[test]
    fn integration_error_display() {
        let err = IntegrationError::NodeNotFound {
            node_id: test_node_id1(),
        };
        assert!(!err.to_string().is_empty());
    }

    #[test]
    fn integration_error_serialization_roundtrip() {
        let err = IntegrationError::ContractAlreadyRegistered {
            contract: ContractType::CRDT,
            node_id: test_node_id1(),
        };
        let json = serde_json::to_string(&err).unwrap();
        let deserialized: IntegrationError = serde_json::from_str(&json).unwrap();
        assert!(matches!(deserialized, IntegrationError::ContractAlreadyRegistered { .. }));
    }

    // ========================================================================
    // IntegrationEngine tests
    // ========================================================================

    #[test]
    fn integration_engine_new() {
        let engine = IntegrationEngine::new();
        assert_eq!(engine.registered_nodes().len(), 0);
        assert_eq!(engine.total_registrations(), 0);
        assert_eq!(engine.integration_version(), 0);
        assert!(engine.is_healthy());
    }

    #[test]
    fn integration_engine_with_config() {
        let config = IntegrationConfig::new(500).unwrap();
        let engine = IntegrationEngine::with_config(config).unwrap();
        assert_eq!(engine.config().max_nodes, 500);
    }

    #[test]
    fn integration_engine_with_config_invalid() {
        let mut config = IntegrationConfig::default();
        config.max_nodes = 0;
        let result = IntegrationEngine::with_config(config);
        assert!(result.is_err());
    }

    #[test]
    fn integration_engine_configure_integration() {
        let engine = IntegrationEngine::new();
        let config = IntegrationConfig::strict();
        assert!(engine.configure_integration(config.clone()).is_ok());
        assert_eq!(engine.config().max_nodes, 100);
    }

    #[test]
    fn integration_engine_configure_integration_invalid() {
        let engine = IntegrationEngine::new();
        let mut config = IntegrationConfig::default();
        config.max_nodes = 0;
        let err = engine.configure_integration(config).unwrap_err();
        assert!(matches!(err, IntegrationError::InvalidConfig { .. }));
    }

    #[test]
    fn integration_engine_register_carrier() {
        let engine = IntegrationEngine::new();
        let reg = engine
            .register_carrier(test_node_id1(), test_endpoint())
            .unwrap();
        assert_eq!(reg.node_id, test_node_id1());
        assert_eq!(reg.contract_type, ContractType::Carrier);
        assert!(reg.status.is_active());
        assert!(engine.node_exists(test_node_id1()));
    }

    #[test]
    fn integration_engine_register_duplicate_contract() {
        let engine = IntegrationEngine::new();
        engine.register_carrier(test_node_id1(), test_endpoint()).unwrap();
        let err = engine
            .register_carrier(test_node_id1(), test_endpoint2())
            .unwrap_err();
        assert!(matches!(
            err,
            IntegrationError::ContractAlreadyRegistered { .. }
        ));
    }

    #[test]
    fn integration_engine_register_all_contract_types() {
        let engine = IntegrationEngine::new();
        for contract_type in ContractType::all_types() {
            match contract_type {
                ContractType::Carrier => {
                    engine.register_carrier(test_node_id1(), test_endpoint()).unwrap();
                }
                _ => {
                    // For all other types, use register_contract via the specific method
                    // We'll test a few key ones
                }
            }
        }
        // Test several specific registrations
        engine.register_transport(test_node_id1()).unwrap();
        engine.register_store(test_node_id1()).unwrap();
        engine.register_session(test_node_id1()).unwrap();
        engine.register_routing(test_node_id1()).unwrap();
        engine.register_discovery(test_node_id1()).unwrap();
        engine.register_membership(test_node_id1()).unwrap();
        engine.register_placement(test_node_id1()).unwrap();
        engine.register_replication(test_node_id1()).unwrap();
        engine.register_recovery(test_node_id1()).unwrap();
        engine.register_consistency(test_node_id1()).unwrap();
        engine.register_ordering(test_node_id1()).unwrap();
        engine.register_security(test_node_id1()).unwrap();
        engine.register_client(test_node_id1()).unwrap();
        engine.register_fault_tolerance(test_node_id1()).unwrap();
        engine.register_broadcast(test_node_id1()).unwrap();
        engine.register_quorum(test_node_id1()).unwrap();
        engine.register_snapshot(test_node_id1()).unwrap();
        engine.register_byzantine(test_node_id1()).unwrap();
        engine.register_lease(test_node_id1()).unwrap();
        engine.register_crdt(test_node_id1()).unwrap();
        engine.register_erasure(test_node_id1()).unwrap();
        engine.register_saga(test_node_id1()).unwrap();
        engine.register_coordinator(test_node_id1()).unwrap();
        engine.register_durability(test_node_id1()).unwrap();
        engine.register_topology(test_node_id1()).unwrap();

        assert_eq!(
            engine.get_node_registrations(test_node_id1()).len(),
            26
        );
    }

    #[test]
    fn integration_engine_register_contract_directly() {
        let engine = IntegrationEngine::new();
        let reg = engine
            .register_contract(test_node_id1(), ContractType::Store, test_endpoint())
            .unwrap();
        assert_eq!(reg.contract_type, ContractType::Store);
        assert!(engine.is_contract_registered(test_node_id1(), ContractType::Store));
    }

    #[test]
    fn integration_engine_max_contracts_per_node() {
        let engine = IntegrationEngine::new();
        let mut config = IntegrationConfig::default();
        config.max_contracts_per_node = 2;
        engine.configure_integration(config).unwrap();

        engine.register_carrier(test_node_id1(), test_endpoint()).unwrap();
        engine.register_transport(test_node_id1()).unwrap();
        let err = engine
            .register_store(test_node_id1())
            .unwrap_err();
        assert!(matches!(
            err,
            IntegrationError::MaxContractsPerNodeExceeded { .. }
        ));
    }

    #[test]
    fn integration_engine_get_registration() {
        let engine = IntegrationEngine::new();
        engine.register_carrier(test_node_id1(), test_endpoint()).unwrap();
        let reg = engine
            .get_registration(test_node_id1(), ContractType::Carrier)
            .unwrap();
        assert_eq!(reg.contract_type, ContractType::Carrier);
        assert_eq!(reg.node_id, test_node_id1());
    }

    #[test]
    fn integration_engine_get_registration_nonexistent() {
        let engine = IntegrationEngine::new();
        let reg = engine
            .get_registration(test_node_id1(), ContractType::Store);
        assert!(reg.is_none());
    }

    #[test]
    fn integration_engine_get_node_registrations() {
        let engine = IntegrationEngine::new();
        engine.register_carrier(test_node_id1(), test_endpoint()).unwrap();
        engine.register_transport(test_node_id1()).unwrap();
        engine.register_carrier(test_node_id2(), test_endpoint2()).unwrap();
        let regs = engine.get_node_registrations(test_node_id1());
        assert_eq!(regs.len(), 2);
        let regs2 = engine.get_node_registrations(test_node_id2());
        assert_eq!(regs2.len(), 1);
    }

    #[test]
    fn integration_engine_get_contract_registrations() {
        let engine = IntegrationEngine::new();
        engine.register_carrier(test_node_id1(), test_endpoint()).unwrap();
        engine.register_carrier(test_node_id2(), test_endpoint2()).unwrap();
        let regs = engine.get_contract_registrations(ContractType::Carrier);
        assert_eq!(regs.len(), 2);
    }

    #[test]
    fn integration_engine_unregister_contract() {
        let engine = IntegrationEngine::new();
        engine.register_carrier(test_node_id1(), test_endpoint()).unwrap();
        assert!(engine.unregister_contract(test_node_id1(), ContractType::Carrier).is_ok());
        assert!(!engine.is_contract_registered(test_node_id1(), ContractType::Carrier));
    }

    #[test]
    fn integration_engine_unregister_nonexistent_contract() {
        let engine = IntegrationEngine::new();
        let err = engine
            .unregister_contract(test_node_id1(), ContractType::Store)
            .unwrap_err();
        assert!(matches!(
            err,
            IntegrationError::ContractNotRegistered { .. }
        ));
    }

    #[test]
    fn integration_engine_start_node() {
        let engine = IntegrationEngine::new();
        engine.register_carrier(test_node_id1(), test_endpoint()).unwrap();
        let result = engine.start_node(test_node_id1()).unwrap();
        assert_eq!(result.node_id, test_node_id1());
        assert_eq!(result.contracts_started, 1);
    }

    #[test]
    fn integration_engine_start_nonexistent_node() {
        let engine = IntegrationEngine::new();
        let err = engine.start_node(test_node_id1()).unwrap_err();
        assert!(matches!(err, IntegrationError::NodeNotFound { .. }));
    }

    #[test]
    fn integration_engine_stop_node() {
        let engine = IntegrationEngine::new();
        engine.register_carrier(test_node_id1(), test_endpoint()).unwrap();
        engine.start_node(test_node_id1()).unwrap();
        let result = engine.stop_node(test_node_id1()).unwrap();
        assert_eq!(result.node_id, test_node_id1());
        assert_eq!(result.contracts_stopped, 1);
    }

    #[test]
    fn integration_engine_stop_not_started_node() {
        let engine = IntegrationEngine::new();
        engine.register_carrier(test_node_id1(), test_endpoint()).unwrap();
        let err = engine.stop_node(test_node_id1()).unwrap_err();
        assert!(matches!(err, IntegrationError::NodeNotStarted { .. }));
    }

    #[test]
    fn integration_engine_registered_nodes() {
        let engine = IntegrationEngine::new();
        assert!(engine.registered_nodes().is_empty());
        engine.register_carrier(test_node_id1(), test_endpoint()).unwrap();
        engine.register_carrier(test_node_id2(), test_endpoint2()).unwrap();
        let nodes = engine.registered_nodes();
        assert_eq!(nodes.len(), 2);
    }

    #[test]
    fn integration_engine_integration_events() {
        let engine = IntegrationEngine::new();
        engine.register_carrier(test_node_id1(), test_endpoint()).unwrap();
        let events = engine.integration_events();
        assert!(!events.is_empty());
        assert_eq!(events[0].event_type, IntegrationEventType::ContractRegistered);
    }

    #[test]
    fn integration_engine_stats() {
        let engine = IntegrationEngine::new();
        engine.register_carrier(test_node_id1(), test_endpoint()).unwrap();
        engine.register_transport(test_node_id1()).unwrap();
        let stats = engine.stats();
        assert_eq!(stats.total_nodes, 1);
        assert_eq!(stats.total_contracts, 2);
        assert_eq!(stats.active_contracts, 2);
    }

    #[test]
    fn integration_engine_is_healthy() {
        let engine = IntegrationEngine::new();
        assert!(engine.is_healthy());
        engine.register_carrier(test_node_id1(), test_endpoint()).unwrap();
        engine.start_node(test_node_id1()).unwrap();
        assert!(engine.is_healthy());
    }

    #[test]
    fn integration_engine_validate_config() {
        let engine = IntegrationEngine::new();
        let config = IntegrationConfig::default();
        assert!(engine.validate_config(&config).is_ok());
        let mut bad_config = IntegrationConfig::default();
        bad_config.max_nodes = 0;
        assert!(engine.validate_config(&bad_config).is_err());
    }

    #[test]
    fn integration_engine_snapshot_restore() {
        let engine = IntegrationEngine::new();
        engine.register_carrier(test_node_id1(), test_endpoint()).unwrap();
        engine.register_transport(test_node_id1()).unwrap();
        engine.start_node(test_node_id1()).unwrap();

        let snapshot = engine.snapshot();
        assert_eq!(snapshot.node_count(), 1);
        assert_eq!(snapshot.registration_count(), 2);

        let engine2 = IntegrationEngine::new();
        engine2.restore_from_snapshot(snapshot).unwrap();
        assert_eq!(engine2.registered_nodes().len(), 1);
        assert_eq!(engine2.total_registrations(), 2);
    }

    #[test]
    fn integration_engine_shutdown_all() {
        let engine = IntegrationEngine::new();
        engine.register_carrier(test_node_id1(), test_endpoint()).unwrap();
        engine.register_carrier(test_node_id2(), test_endpoint2()).unwrap();
        engine.start_node(test_node_id1()).unwrap();
        engine.start_node(test_node_id2()).unwrap();

        let result = engine.shutdown_all().unwrap();
        assert_eq!(result.nodes_stopped, 2);
        assert_eq!(result.contracts_stopped, 2);
    }

    #[test]
    fn integration_engine_clear_all() {
        let engine = IntegrationEngine::new();
        engine.register_carrier(test_node_id1(), test_endpoint()).unwrap();
        engine.register_transport(test_node_id1()).unwrap();
        engine.clear_all();
        assert_eq!(engine.registered_nodes().len(), 0);
        assert_eq!(engine.total_registrations(), 0);
        assert_eq!(engine.integration_version(), 0);
    }

    // ========================================================================
    // IntegrationEngine advanced methods tests
    // ========================================================================

    #[test]
    fn integration_engine_get_registration_nonexistent_node() {
        let engine = IntegrationEngine::new();
        let reg = engine.get_registration(test_node_id1(), ContractType::Store);
        assert!(reg.is_none());
    }

    #[test]
    fn integration_engine_node_endpoint() {
        let engine = IntegrationEngine::new();
        engine.register_carrier(test_node_id1(), test_endpoint()).unwrap();
        assert_eq!(engine.node_endpoint(test_node_id1()), Some(test_endpoint()));
        assert_eq!(engine.node_endpoint(test_node_id2()), None);
    }

    #[test]
    fn integration_engine_set_node_endpoint() {
        let engine = IntegrationEngine::new();
        engine.register_carrier(test_node_id1(), test_endpoint()).unwrap();
        engine.set_node_endpoint(test_node_id1(), test_endpoint2()).unwrap();
        assert_eq!(engine.node_endpoint(test_node_id1()), Some(test_endpoint2()));
    }

    #[test]
    fn integration_engine_set_node_endpoint_nonexistent() {
        let engine = IntegrationEngine::new();
        let err = engine
            .set_node_endpoint(test_node_id1(), test_endpoint())
            .unwrap_err();
        assert!(matches!(err, IntegrationError::NodeNotFound { .. }));
    }

    #[test]
    fn integration_engine_all_endpoints() {
        let engine = IntegrationEngine::new();
        engine.register_carrier(test_node_id1(), test_endpoint()).unwrap();
        engine.register_carrier(test_node_id2(), test_endpoint2()).unwrap();
        let endpoints = engine.all_endpoints();
        assert_eq!(endpoints.len(), 2);
    }

    #[test]
    fn integration_engine_total_registrations() {
        let engine = IntegrationEngine::new();
        assert_eq!(engine.total_registrations(), 0);
        engine.register_carrier(test_node_id1(), test_endpoint()).unwrap();
        assert_eq!(engine.total_registrations(), 1);
        engine.register_transport(test_node_id1()).unwrap();
        assert_eq!(engine.total_registrations(), 2);
    }

    #[test]
    fn integration_engine_total_operations() {
        let engine = IntegrationEngine::new();
        assert_eq!(engine.total_operations(), 0);
    }

    #[test]
    fn integration_engine_total_events() {
        let engine = IntegrationEngine::new();
        assert_eq!(engine.total_events(), 0);
        engine.register_carrier(test_node_id1(), test_endpoint()).unwrap();
        assert!(engine.total_events() > 0);
    }

    #[test]
    fn integration_engine_integration_version() {
        let engine = IntegrationEngine::new();
        assert_eq!(engine.integration_version(), 0);
        engine.register_carrier(test_node_id1(), test_endpoint()).unwrap();
        assert!(engine.integration_version() > 0);
    }

    #[test]
    fn integration_engine_node_exists() {
        let engine = IntegrationEngine::new();
        assert!(!engine.node_exists(test_node_id1()));
        engine.register_carrier(test_node_id1(), test_endpoint()).unwrap();
        assert!(engine.node_exists(test_node_id1()));
    }

    #[test]
    fn integration_engine_is_contract_registered() {
        let engine = IntegrationEngine::new();
        assert!(!engine.is_contract_registered(test_node_id1(), ContractType::Store));
        engine.register_store(test_node_id1()).unwrap();
        assert!(engine.is_contract_registered(test_node_id1(), ContractType::Store));
    }

    #[test]
    fn integration_engine_config() {
        let engine = IntegrationEngine::new();
        let config = engine.config();
        assert_eq!(config.max_nodes, 1000);
    }

    #[test]
    fn integration_engine_graph_size() {
        let engine = IntegrationEngine::new();
        engine.register_carrier(test_node_id1(), test_endpoint()).unwrap();
        let (nodes, registrations, events) = engine.graph_size();
        assert_eq!(nodes, 1);
        assert_eq!(registrations, 1);
        assert!(events > 0);
    }

    #[test]
    fn integration_engine_nodes_by_registration_count() {
        let engine = IntegrationEngine::new();
        engine.register_carrier(test_node_id1(), test_endpoint()).unwrap();
        engine.register_transport(test_node_id1()).unwrap();
        engine.register_carrier(test_node_id2(), test_endpoint2()).unwrap();
        let counts = engine.nodes_by_registration_count();
        assert_eq!(counts[0].0, test_node_id1());
        assert_eq!(counts[0].1, 2);
        assert_eq!(counts[1].0, test_node_id2());
        assert_eq!(counts[1].1, 1);
    }

    #[test]
    fn integration_engine_contract_types_by_count() {
        let engine = IntegrationEngine::new();
        engine.register_carrier(test_node_id1(), test_endpoint()).unwrap();
        engine.register_carrier(test_node_id2(), test_endpoint2()).unwrap();
        engine.register_store(test_node_id1()).unwrap();
        let counts = engine.contract_types_by_count();
        assert_eq!(counts[0].0, ContractType::Carrier);
        assert_eq!(counts[0].1, 2);
        assert_eq!(counts[1].0, ContractType::Store);
        assert_eq!(counts[1].1, 1);
    }

    #[test]
    fn integration_engine_recent_registrations() {
        let engine = IntegrationEngine::new();
        engine.register_carrier(test_node_id1(), test_endpoint()).unwrap();
        engine.register_transport(test_node_id1()).unwrap();
        let recent = engine.recent_registrations(1);
        assert_eq!(recent.len(), 1);
        assert_eq!(recent[0].contract_type, ContractType::Transport);
    }

    #[test]
    fn integration_engine_registrations_by_status() {
        let engine = IntegrationEngine::new();
        engine.register_carrier(test_node_id1(), test_endpoint()).unwrap();
        let active = engine.registrations_by_status(RegistrationStatus::Active);
        assert_eq!(active.len(), 1);
        let failed = engine.registrations_by_status(RegistrationStatus::Failed);
        assert_eq!(failed.len(), 0);
    }

    #[test]
    fn integration_engine_active_registration_count() {
        let engine = IntegrationEngine::new();
        assert_eq!(engine.active_registration_count(), 0);
        engine.register_carrier(test_node_id1(), test_endpoint()).unwrap();
        assert_eq!(engine.active_registration_count(), 1);
    }

    #[test]
    fn integration_engine_failed_registration_count() {
        let engine = IntegrationEngine::new();
        assert_eq!(engine.failed_registration_count(), 0);
    }

    #[test]
    fn integration_engine_pending_registration_count() {
        let engine = IntegrationEngine::new();
        assert_eq!(engine.pending_registration_count(), 0);
    }

    // ========================================================================
    // Integrated operation execution tests
    // ========================================================================

    #[test]
    fn integration_engine_execute_operation_success() {
        let engine = IntegrationEngine::new();
        engine.register_security(test_node_id1()).unwrap();
        engine.register_store(test_node_id1()).unwrap();
        engine.register_replication(test_node_id1()).unwrap();
        engine.register_consistency(test_node_id1()).unwrap();
        engine.register_ordering(test_node_id1()).unwrap();
        engine.register_durability(test_node_id1()).unwrap();

        let op = IntegratedOperation {
            operation_id: 1,
            operation_type: IntegratedOperationType::Write,
            source_node: test_node_id1(),
            target_nodes: vec![test_node_id2()],
            dataset_id: None,
            contract_chain: vec![
                ContractType::Security,
                ContractType::Store,
                ContractType::Replication,
                ContractType::Consistency,
                ContractType::Ordering,
                ContractType::Durability,
            ],
            payload: vec![1, 2, 3],
            timeout_ms: 5000,
            priority: 5,
            created_at: 1000,
        };

        let result = engine.execute_integrated_operation(op).unwrap();
        assert!(result.success);
        assert_eq!(result.operation_id, 1);
        assert_eq!(result.contract_results.len(), 6);
    }

    #[test]
    fn integration_engine_execute_operation_insufficient_contracts() {
        let engine = IntegrationEngine::new();
        engine.register_store(test_node_id1()).unwrap();

        let op = IntegratedOperation {
            operation_id: 1,
            operation_type: IntegratedOperationType::Write,
            source_node: test_node_id1(),
            target_nodes: vec![],
            dataset_id: None,
            contract_chain: vec![
                ContractType::Security,
                ContractType::Store,
            ],
            payload: vec![],
            timeout_ms: 5000,
            priority: 5,
            created_at: 1000,
        };

        let err = engine.execute_integrated_operation(op).unwrap_err();
        assert!(matches!(
            err,
            IntegrationError::InsufficientContracts { .. }
        ));
    }

    #[test]
    fn integration_engine_execute_operation_nonexistent_source_node() {
        let engine = IntegrationEngine::new();
        let op = IntegratedOperation {
            operation_id: 1,
            operation_type: IntegratedOperationType::Write,
            source_node: test_node_id1(),
            target_nodes: vec![],
            dataset_id: None,
            contract_chain: vec![],
            payload: vec![],
            timeout_ms: 5000,
            priority: 5,
            created_at: 1000,
        };

        let err = engine.execute_integrated_operation(op).unwrap_err();
        assert!(matches!(err, IntegrationError::NodeNotFound { .. }));
    }

    // ========================================================================
    // Integration contract count tests
    // ========================================================================

    #[test]
    fn integration_engine_contract_count_by_type() {
        let engine = IntegrationEngine::new();
        engine.register_carrier(test_node_id1(), test_endpoint()).unwrap();
        engine.register_carrier(test_node_id2(), test_endpoint2()).unwrap();
        engine.register_store(test_node_id1()).unwrap();
        let counts = engine.contract_count_by_type();
        assert_eq!(counts.get("carrier"), Some(&2));
        assert_eq!(counts.get("store"), Some(&1));
    }

    #[test]
    fn integration_engine_node_contract_coverage() {
        let engine = IntegrationEngine::new();
        engine.register_carrier(test_node_id1(), test_endpoint()).unwrap();
        engine.register_transport(test_node_id1()).unwrap();
        let coverage = engine.node_contract_coverage(test_node_id1()).unwrap();
        assert_eq!(coverage.node_id, test_node_id1());
        assert_eq!(coverage.active_contracts.len(), 2);
        assert!(coverage.missing_contracts.len() > 0);
        assert!(!coverage.is_complete);
        assert!(coverage.coverage_percent > 0.0);
        assert!(coverage.coverage_percent < 100.0);
    }

    #[test]
    fn integration_engine_node_contract_coverage_complete() {
        let engine = IntegrationEngine::new();
        for contract_type in ContractType::all_types() {
            match contract_type {
                ContractType::Carrier => {
                    engine.register_carrier(test_node_id1(), test_endpoint()).unwrap();
                }
                ContractType::Transport => {
                    engine.register_transport(test_node_id1()).unwrap();
                }
                ContractType::Store => {
                    engine.register_store(test_node_id1()).unwrap();
                }
                ContractType::Session => {
                    engine.register_session(test_node_id1()).unwrap();
                }
                ContractType::Routing => {
                    engine.register_routing(test_node_id1()).unwrap();
                }
                ContractType::Discovery => {
                    engine.register_discovery(test_node_id1()).unwrap();
                }
                ContractType::Membership => {
                    engine.register_membership(test_node_id1()).unwrap();
                }
                ContractType::Placement => {
                    engine.register_placement(test_node_id1()).unwrap();
                }
                ContractType::Replication => {
                    engine.register_replication(test_node_id1()).unwrap();
                }
                ContractType::Recovery => {
                    engine.register_recovery(test_node_id1()).unwrap();
                }
                ContractType::Consistency => {
                    engine.register_consistency(test_node_id1()).unwrap();
                }
                ContractType::Ordering => {
                    engine.register_ordering(test_node_id1()).unwrap();
                }
                ContractType::Security => {
                    engine.register_security(test_node_id1()).unwrap();
                }
                ContractType::Client => {
                    engine.register_client(test_node_id1()).unwrap();
                }
                ContractType::FaultTolerance => {
                    engine.register_fault_tolerance(test_node_id1()).unwrap();
                }
                ContractType::Broadcast => {
                    engine.register_broadcast(test_node_id1()).unwrap();
                }
                ContractType::Quorum => {
                    engine.register_quorum(test_node_id1()).unwrap();
                }
                ContractType::Snapshot => {
                    engine.register_snapshot(test_node_id1()).unwrap();
                }
                ContractType::Byzantine => {
                    engine.register_byzantine(test_node_id1()).unwrap();
                }
                ContractType::Lease => {
                    engine.register_lease(test_node_id1()).unwrap();
                }
                ContractType::CRDT => {
                    engine.register_crdt(test_node_id1()).unwrap();
                }
                ContractType::Erasure => {
                    engine.register_erasure(test_node_id1()).unwrap();
                }
                ContractType::Saga => {
                    engine.register_saga(test_node_id1()).unwrap();
                }
                ContractType::Coordinator => {
                    engine.register_coordinator(test_node_id1()).unwrap();
                }
                ContractType::Durability => {
                    engine.register_durability(test_node_id1()).unwrap();
                }
                ContractType::Topology => {
                    engine.register_topology(test_node_id1()).unwrap();
                }
            }
        }
        let coverage = engine.node_contract_coverage(test_node_id1()).unwrap();
        assert!(coverage.is_complete);
        assert!((coverage.coverage_percent - 100.0).abs() < 0.01);
        assert!(coverage.missing_contracts.is_empty());
    }

    #[test]
    fn integration_engine_node_contract_coverage_nonexistent() {
        let engine = IntegrationEngine::new();
        let err = engine
            .node_contract_coverage(test_node_id1())
            .unwrap_err();
        assert!(matches!(err, IntegrationError::NodeNotFound { .. }));
    }

    // ========================================================================
    // Serialization roundtrip tests
    // ========================================================================

    #[test]
    fn integration_serialization_roundtrip_integration_config() {
        let config = IntegrationConfig::default();
        let json = serde_json::to_string(&config).unwrap();
        let deserialized: IntegrationConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(config.max_nodes, deserialized.max_nodes);
        assert_eq!(config.max_contracts_per_node, deserialized.max_contracts_per_node);
    }

    #[test]
    fn integration_serialization_roundtrip_registration_status() {
        let status = RegistrationStatus::Failed;
        let json = serde_json::to_string(&status).unwrap();
        let deserialized: RegistrationStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(status, deserialized);
    }

    #[test]
    fn integration_serialization_roundtrip_integration_stats() {
        let stats = IntegrationStats::new();
        let json = serde_json::to_string(&stats).unwrap();
        let deserialized: IntegrationStats = serde_json::from_str(&json).unwrap();
        assert_eq!(stats.total_nodes, deserialized.total_nodes);
    }

    // ========================================================================
    // Integration error tests
    // ========================================================================

    #[test]
    fn integration_error_max_nodes_exceeded() {
        let err = IntegrationError::MaxNodesExceeded { max_nodes: 100 };
        assert!(!err.is_recoverable());
        assert!(!err.suggested_action().is_empty());
    }

    #[test]
    fn integration_error_max_contracts_per_node_exceeded() {
        let err = IntegrationError::MaxContractsPerNodeExceeded {
            node_id: test_node_id1(),
        };
        assert!(!err.is_recoverable());
        assert!(!err.suggested_action().is_empty());
    }

    #[test]
    fn integration_error_snapshot_version_mismatch() {
        let err = IntegrationError::SnapshotVersionMismatch {
            expected: 1,
            got: 2,
        };
        assert!(!err.is_recoverable());
        assert!(!err.suggested_action().is_empty());
    }

    #[test]
    fn integration_error_internal() {
        let err = IntegrationError::Internal {
            message: "test error".to_string(),
        };
        assert!(!err.is_recoverable());
        assert_eq!(err.suggested_action(), "contact system administrator");
    }

    // ========================================================================
    // Integration full lifecycle tests
    // ========================================================================

    #[test]
    fn integration_full_lifecycle() {
        let engine = IntegrationEngine::new();
        let config = IntegrationConfig::strict();
        engine.configure_integration(config).unwrap();

        // Register node with multiple contracts
        engine.register_carrier(test_node_id1(), test_endpoint()).unwrap();
        engine.register_transport(test_node_id1()).unwrap();
        engine.register_store(test_node_id1()).unwrap();
        engine.register_security(test_node_id1()).unwrap();
        engine.register_membership(test_node_id1()).unwrap();
        engine.register_topology(test_node_id1()).unwrap();

        // Start node
        let start_result = engine.start_node(test_node_id1()).unwrap();
        assert_eq!(start_result.contracts_started, 6);

        // Check registration status
        let status = engine.registration_status(test_node_id1()).unwrap();
        assert_eq!(status.active_contract_count, 6);
        assert!(status.is_fully_registered);

        // Check coverage
        let coverage = engine.node_contract_coverage(test_node_id1()).unwrap();
        assert_eq!(coverage.active_contracts.len(), 6);

        // Execute operation
        engine.register_replication(test_node_id1()).unwrap();
        engine.register_consistency(test_node_id1()).unwrap();
        engine.register_ordering(test_node_id1()).unwrap();
        engine.register_durability(test_node_id1()).unwrap();

        let op = IntegratedOperation {
            operation_id: 1,
            operation_type: IntegratedOperationType::Write,
            source_node: test_node_id1(),
            target_nodes: vec![test_node_id2()],
            dataset_id: None,
            contract_chain: vec![
                ContractType::Security,
                ContractType::Store,
                ContractType::Replication,
                ContractType::Consistency,
                ContractType::Ordering,
                ContractType::Durability,
            ],
            payload: vec![1, 2, 3],
            timeout_ms: 5000,
            priority: 5,
            created_at: 1000,
        };
        let result = engine.execute_integrated_operation(op).unwrap();
        assert!(result.success);

        // Snapshot and restore
        let snapshot = engine.snapshot();
        assert_eq!(snapshot.node_count(), 1);

        // Stop node
        let stop_result = engine.stop_node(test_node_id1()).unwrap();
        assert_eq!(stop_result.contracts_stopped, 7);

        // Health check
        assert!(!engine.is_healthy()); // node is stopped

        // Stats
        let stats = engine.stats();
        assert_eq!(stats.total_nodes, 1);
        assert_eq!(stats.stopped_nodes, 1);
    }

    #[test]
    fn integration_multi_node_topology() {
        let engine = IntegrationEngine::new();

        // Node 1: full stack
        engine.register_carrier(test_node_id1(), test_endpoint()).unwrap();
        engine.register_transport(test_node_id1()).unwrap();
        engine.register_store(test_node_id1()).unwrap();
        engine.register_security(test_node_id1()).unwrap();

        // Node 2: storage node
        engine.register_carrier(test_node_id2(), test_endpoint2()).unwrap();
        engine.register_transport(test_node_id2()).unwrap();
        engine.register_store(test_node_id2()).unwrap();
        engine.register_replication(test_node_id2()).unwrap();

        // Node 3: compute node
        engine.register_carrier(test_node_id3(), test_endpoint()).unwrap();
        engine.register_transport(test_node_id3()).unwrap();
        engine.register_compute_if_available(test_node_id3());

        let counts = engine.contract_types_by_count();
        assert_eq!(counts.get(&(ContractType::Carrier, 3)).map(|_| ()), Some(()));
        // Actually verify counts
        let carrier_count = counts.iter().find(|(ct, _)| *ct == ContractType::Carrier);
        assert_eq!(carrier_count, Some(&(ContractType::Carrier, 3)));

        let stats = engine.stats();
        assert_eq!(stats.total_nodes, 3);
    }

    // Helper for compute node test
    fn register_compute_if_available(_engine: &IntegrationEngine, _node_id: NodeId) {
        // Placeholder - compute is not a contract type, just testing pattern
    }
}

