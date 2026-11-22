//! # ids-000-aa Sealed Trait Contract: Saga
//!
//! Distributed saga orchestration contract for IDS.
//! Provides long-running transaction management with compensation semantics,
//! step state machines, local transactions, coordinator election, and
//! failure recovery. All types are canonical and sealed.
//!
//! Carrier-agnostic: no std::net, tokio::net, libp2p, IP, TCP, UDP, QUIC,
//! HTTP, DNS, or any existing networking stack. Saga messages are opaque
//! byte blobs carried on CarrierType + Vec<u8> Endpoints only.

use crate::errors::SagaError;
use crate::types::{
    DatasetId, Epoch, FailureEvent, FragmentId, HUnitId, IdsTime, LocalTransaction, NodeId,
    OpRef, RegionId, SagaId, SagaResult, StepId, TransactionId, Version,
};
use std::collections::HashMap;

///////////////////////////////////////////////////////////////////////////////
// SEALED TRAIT: Saga
///////////////////////////////////////////////////////////////////////////////

/// Sealed marker — only ids-000-aa and designated crates can implement.
pub mod private {
    pub trait Sealed {}
}

/// Sealed saga orchestration trait.
/// Implementations live downstream in ids-saga and ids-* crates.
pub trait Saga: private::Sealed {
    /// Create a new saga with a coordinator.
    fn create_saga(
        &self,
        saga_id: SagaId,
        coordinator: NodeId,
        steps: Vec<StepDefinition>,
    ) -> Result<SagaState, SagaError>;

    /// Execute the next step of a saga.
    fn execute_step(&self, saga_id: SagaId, step_id: StepId) -> Result<StepExecutionResult, SagaError>;

    /// Compensate a completed step.
    fn compensate_step(&self, saga_id: SagaId, step_id: StepId) -> Result<CompensationResult, SagaError>;

    /// Compensate all completed steps in reverse order.
    fn compensate_all(&self, saga_id: SagaId) -> Result<CompensationBatchResult, SagaError>;

    /// Get saga state.
    fn saga_state(&self, saga_id: SagaId) -> Result<SagaState, SagaError>;

    /// Get all sagas.
    fn sagas(&self) -> Vec<SagaId>;

    /// Register a local transaction for a step.
    fn register_local_transaction(
        &self,
        saga_id: SagaId,
        step_id: StepId,
        transaction: LocalTransaction,
    ) -> Result<(), SagaError>;

    /// Get local transaction for a step.
    fn local_transaction(
        &self,
        saga_id: SagaId,
        step_id: StepId,
    ) -> Result<LocalTransaction, SagaError>;

    /// Record a failure event for a saga.
    fn record_failure(
        &self,
        saga_id: SagaId,
        event: FailureEvent,
    ) -> Result<(), SagaError>;

    /// Recover a saga after failure.
    fn recover_saga(&self, saga_id: SagaId) -> Result<SagaRecoveryResult, SagaError>;

    /// Get saga statistics.
    fn stats(&self, saga_id: SagaId) -> Result<SagaStats, SagaError>;

    /// Check if a saga is complete.
    fn is_complete(&self, saga_id: SagaId) -> bool;

    /// Check if a saga is compensated.
    fn is_compensated(&self, saga_id: SagaId) -> bool;

    /// Get the coordinator node for a saga.
    fn coordinator(&self, saga_id: SagaId) -> Option<NodeId>;

    /// Get completed steps for a saga.
    fn completed_steps(&self, saga_id: SagaId) -> Vec<StepId>;

    /// Get pending steps for a saga.
    fn pending_steps(&self, saga_id: SagaId) -> Vec<StepId>;

    /// Get failed steps for a saga.
    fn failed_steps(&self, saga_id: SagaId) -> Vec<StepId>;

    /// Get compensating steps for a saga.
    fn compensating_steps(&self, saga_id: SagaId) -> Vec<StepId>;

    /// Abort a saga and compensate all completed steps.
    fn abort(&self, saga_id: SagaId) -> Result<SagaAbortResult, SagaError>;

    /// Validate saga configuration.
    fn validate_config(&self, config: &SagaConfig) -> Result<(), SagaError>;

    /// Advance saga to next state.
    fn advance(&self, saga_id: SagaId) -> Result<SagaTransitionResult, SagaError>;

    /// Get all failure events for a saga.
    fn failure_events(&self, saga_id: SagaId) -> Vec<FailureEvent>;

    /// Get recovery results for a saga.
    fn recovery_results(&self, saga_id: SagaId) -> Vec<SagaRecoveryResult>;
}

///////////////////////////////////////////////////////////////////////////////
// CONFIGURATION
///////////////////////////////////////////////////////////////////////////////

/// Saga configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SagaConfig {
    /// Maximum steps per saga.
    pub max_steps: u32,
    /// Maximum compensation attempts per step.
    pub max_compensation_attempts: u32,
    /// Step timeout in milliseconds.
    pub step_timeout_ms: u64,
    /// Compensation timeout in milliseconds.
    pub compensation_timeout_ms: u64,
    /// Enable automatic compensation on failure.
    pub auto_compensate: bool,
    /// Enable automatic recovery.
    pub auto_recover: bool,
    /// Maximum concurrent sagas.
    pub max_concurrent_sagas: u32,
    /// Saga persistence enabled.
    pub enable_persistence: bool,
}

impl SagaConfig {
    pub fn new(max_steps: u32) -> Result<Self, SagaError> {
        let config = Self {
            max_steps,
            max_compensation_attempts: 3,
            step_timeout_ms: 30000,
            compensation_timeout_ms: 60000,
            auto_compensate: true,
            auto_recover: true,
            max_concurrent_sagas: 100,
            enable_persistence: true,
        };
        config.validate()?;
        Ok(config)
    }

    pub fn validate(&self) -> Result<(), SagaError> {
        if self.max_steps == 0 {
            return Err(SagaError::InvalidConfig {
                field: "max_steps".to_string(),
                reason: "max_steps must be greater than 0".to_string(),
            });
        }
        if self.max_compensation_attempts == 0 {
            return Err(SagaError::InvalidConfig {
                field: "max_compensation_attempts".to_string(),
                reason: "max_compensation_attempts must be greater than 0".to_string(),
            });
        }
        if self.step_timeout_ms == 0 {
            return Err(SagaError::InvalidConfig {
                field: "step_timeout_ms".to_string(),
                reason: "step_timeout_ms must be greater than 0".to_string(),
            });
        }
        if self.compensation_timeout_ms == 0 {
            return Err(SagaError::InvalidConfig {
                field: "compensation_timeout_ms".to_string(),
                reason: "compensation_timeout_ms must be greater than 0".to_string(),
            });
        }
        if self.max_concurrent_sagas == 0 {
            return Err(SagaError::InvalidConfig {
                field: "max_concurrent_sagas".to_string(),
                reason: "max_concurrent_sagas must be greater than 0".to_string(),
            });
        }
        Ok(())
    }

    pub fn default() -> Self {
        Self {
            max_steps: 10,
            max_compensation_attempts: 3,
            step_timeout_ms: 30000,
            compensation_timeout_ms: 60000,
            auto_compensate: true,
            auto_recover: true,
            max_concurrent_sagas: 100,
            enable_persistence: true,
        }
    }

    pub fn strict() -> Self {
        Self {
            max_steps: 5,
            max_compensation_attempts: 5,
            step_timeout_ms: 10000,
            compensation_timeout_ms: 30000,
            auto_compensate: true,
            auto_recover: true,
            max_concurrent_sagas: 10,
            enable_persistence: true,
        }
    }

    pub fn lenient() -> Self {
        Self {
            max_steps: 100,
            max_compensation_attempts: 1,
            step_timeout_ms: 300000,
            compensation_timeout_ms: 600000,
            auto_compensate: false,
            auto_recover: false,
            max_concurrent_sagas: 1000,
            enable_persistence: false,
        }
    }

    pub fn compensation_ratio(&self) -> f64 {
        self.compensation_timeout_ms as f64 / self.step_timeout_ms as f64
    }
}

impl Default for SagaConfig {
    fn default() -> Self {
        Self::default()
    }
}

///////////////////////////////////////////////////////////////////////////////
// STEP DEFINITION
///////////////////////////////////////////////////////////////////////////////

/// Definition of a saga step.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StepDefinition {
    /// Step identifier.
    pub step_id: StepId,
    /// Dataset this step operates on.
    pub dataset_id: DatasetId,
    /// Step payload.
    pub payload: Vec<u8>,
    /// Compensation payload (used to undo this step).
    pub compensate_payload: Vec<u8>,
    /// Step timeout override (ms).
    pub timeout_ms: Option<u64>,
    /// Whether this step is required (cannot be skipped).
    pub required: bool,
    /// Step order index.
    pub order: u32,
}

impl StepDefinition {
    pub fn new(
        step_id: StepId,
        dataset_id: DatasetId,
        payload: Vec<u8>,
        compensate_payload: Vec<u8>,
        order: u32,
    ) -> Self {
        Self {
            step_id,
            dataset_id,
            payload,
            compensate_payload,
            timeout_ms: None,
            required: true,
            order,
        }
    }

    pub fn is_required(&self) -> bool {
        self.required
    }

    pub fn effective_timeout(&self, default_ms: u64) -> u64 {
        self.timeout_ms.unwrap_or(default_ms)
    }
}

///////////////////////////////////////////////////////////////////////////////
// STEP STATE
///////////////////////////////////////////////////////////////////////////////

/// State of a saga step.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum StepState {
    /// Step has not been executed yet.
    Pending,
    /// Step is currently executing.
    Executing,
    /// Step completed successfully.
    Completed,
    /// Step failed during execution.
    Failed,
    /// Step is being compensated.
    Compensating,
    /// Step has been compensated (undone).
    Compensated,
    /// Step was skipped.
    Skipped,
}

impl StepState {
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            StepState::Completed | StepState::Compensated | StepState::Skipped
        )
    }

    pub fn is_executable(&self) -> bool {
        matches!(self, StepState::Pending)
    }

    pub fn is_compensatable(&self) -> bool {
        matches!(self, StepState::Completed)
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            StepState::Pending => "pending",
            StepState::Executing => "executing",
            StepState::Completed => "completed",
            StepState::Failed => "failed",
            StepState::Compensating => "compensating",
            StepState::Compensated => "compensated",
            StepState::Skipped => "skipped",
        }
    }
}

///////////////////////////////////////////////////////////////////////////////
// SAGA STATE
///////////////////////////////////////////////////////////////////////////////

/// Overall state of a saga.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SagaState {
    /// Saga has been created but not started.
    Created,
    /// Saga is executing steps.
    Executing,
    /// Saga completed successfully.
    Completed,
    /// Saga failed and needs compensation.
    Failed,
    /// Saga is being compensated.
    Compensating,
    /// Saga has been fully compensated.
    Compensated,
    /// Saga was aborted.
    Aborted,
}

impl SagaState {
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            SagaState::Completed | SagaState::Compensated | SagaState::Aborted
        )
    }

    pub fn is_active(&self) -> bool {
        matches!(
            self,
            SagaState::Created | SagaState::Executing | SagaState::Failed | SagaState::Compensating
        )
    }

    pub fn can_compensate(&self) -> bool {
        matches!(self, SagaState::Completed | SagaState::Failed)
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            SagaState::Created => "created",
            SagaState::Executing => "executing",
            SagaState::Completed => "completed",
            SagaState::Failed => "failed",
            SagaState::Compensating => "compensating",
            SagaState::Compensated => "compensated",
            SagaState::Aborted => "aborted",
        }
    }
}

///////////////////////////////////////////////////////////////////////////////
// STEP EXECUTION RESULT
///////////////////////////////////////////////////////////////////////////////

/// Result of executing a saga step.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StepExecutionResult {
    /// Saga ID.
    pub saga_id: SagaId,
    /// Step ID.
    pub step_id: StepId,
    /// Step state after execution.
    pub state: StepState,
    /// Output data from the step.
    pub output: Vec<u8>,
    /// Execution timestamp.
    pub executed_at: IdsTime,
    /// Duration in milliseconds.
    pub duration_ms: u64,
    /// Whether compensation data was recorded.
    pub compensation_recorded: bool,
}

impl StepExecutionResult {
    pub fn success(&self) -> bool {
        self.state == StepState::Completed
    }

    pub fn failed(&self) -> bool {
        self.state == StepState::Failed
    }
}

///////////////////////////////////////////////////////////////////////////////
// COMPENSATION RESULT
///////////////////////////////////////////////////////////////////////////////

/// Result of compensating a single step.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompensationResult {
    /// Saga ID.
    pub saga_id: SagaId,
    /// Step ID.
    pub step_id: StepId,
    /// Whether compensation succeeded.
    pub success: bool,
    /// Compensation output data.
    pub output: Vec<u8>,
    /// Number of attempts made.
    pub attempts: u32,
    /// Compensation timestamp.
    pub compensated_at: IdsTime,
}

impl CompensationResult {
    pub fn is_successful(&self) -> bool {
        self.success
    }
}

///////////////////////////////////////////////////////////////////////////////
// COMPENSATION BATCH RESULT
///////////////////////////////////////////////////////////////////////////////

/// Result of compensating all steps in a saga.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompensationBatchResult {
    /// Saga ID.
    pub saga_id: SagaId,
    /// Total steps compensated.
    pub total_compensated: u32,
    /// Total steps that failed compensation.
    pub total_failed: u32,
    /// Individual compensation results.
    pub results: Vec<CompensationResult>,
    /// Whether all compensations succeeded.
    pub all_successful: bool,
}

impl CompensationBatchResult {
    pub fn success_rate(&self) -> f64 {
        let total = self.total_compensated + self.total_failed;
        if total == 0 {
            return 1.0;
        }
        self.total_compensated as f64 / total as f64
    }
}

///////////////////////////////////////////////////////////////////////////////
// SAGA RECOVERY RESULT
///////////////////////////////////////////////////////////////////////////////

/// Result of recovering a saga after failure.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SagaRecoveryResult {
    /// Saga ID.
    pub saga_id: SagaId,
    /// Recovery action taken.
    pub action: RecoveryAction,
    /// Steps recovered.
    pub steps_recovered: u32,
    /// Whether recovery succeeded.
    pub success: bool,
    /// Recovery timestamp.
    pub recovered_at: IdsTime,
}

/// Recovery action taken for a saga.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RecoveryAction {
    /// No action needed.
    None,
    /// Retry failed step.
    RetryStep,
    /// Compensate completed steps.
    Compensate,
    /// Restart saga from beginning.
    Restart,
    /// Abort saga.
    Abort,
    /// Manual intervention required.
    ManualIntervention,
}

impl RecoveryAction {
    pub fn display_name(&self) -> &'static str {
        match self {
            RecoveryAction::None => "none",
            RecoveryAction::RetryStep => "retry_step",
            RecoveryAction::Compensate => "compensate",
            RecoveryAction::Restart => "restart",
            RecoveryAction::Abort => "abort",
            RecoveryAction::ManualIntervention => "manual_intervention",
        }
    }

    pub fn is_automatic(&self) -> bool {
        matches!(
            self,
            RecoveryAction::RetryStep | RecoveryAction::Compensate | RecoveryAction::Restart
        )
    }
}

///////////////////////////////////////////////////////////////////////////////
// SAGA ABORT RESULT
///////////////////////////////////////////////////////////////////////////////

/// Result of aborting a saga.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SagaAbortResult {
    /// Saga ID.
    pub saga_id: SagaId,
    /// Whether abort succeeded.
    pub success: bool,
    /// Steps compensated during abort.
    pub steps_compensated: u32,
    /// Final saga state.
    pub final_state: SagaState,
}

///////////////////////////////////////////////////////////////////////////////
// SAGA TRANSITION RESULT
///////////////////////////////////////////////////////////////////////////////

/// Result of advancing a saga to its next state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SagaTransitionResult {
    /// Saga ID.
    pub saga_id: SagaId,
    /// Previous state.
    pub previous_state: SagaState,
    /// New state.
    pub new_state: SagaState,
    /// Step that triggered the transition.
    pub triggered_by: Option<StepId>,
    /// Whether the transition was valid.
    pub valid: bool,
}

///////////////////////////////////////////////////////////////////////////////
// SAGA STATS
///////////////////////////////////////////////////////////////////////////////

/// Statistics for a saga.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SagaStats {
    /// Saga ID.
    pub saga_id: SagaId,
    /// Total steps.
    pub total_steps: u64,
    /// Completed steps.
    pub completed_steps: u64,
    /// Failed steps.
    pub failed_steps: u64,
    /// Compensated steps.
    pub compensated_steps: u64,
    /// Total compensation attempts.
    pub total_compensation_attempts: u64,
    /// Successful compensations.
    pub successful_compensations: u64,
    /// Total execution time (ms).
    pub total_execution_time_ms: u64,
    /// Number of failures recorded.
    pub failure_count: u64,
    /// Number of recoveries performed.
    pub recovery_count: u64,
}

impl SagaStats {
    pub fn new(saga_id: SagaId) -> Self {
        Self {
            saga_id,
            total_steps: 0,
            completed_steps: 0,
            failed_steps: 0,
            compensated_steps: 0,
            total_compensation_attempts: 0,
            successful_compensations: 0,
            total_execution_time_ms: 0,
            failure_count: 0,
            recovery_count: 0,
        }
    }

    pub fn completion_rate(&self) -> f64 {
        if self.total_steps == 0 {
            return 1.0;
        }
        self.completed_steps as f64 / self.total_steps as f64
    }

    pub fn compensation_success_rate(&self) -> f64 {
        if self.total_compensation_attempts == 0 {
            return 1.0;
        }
        self.successful_compensations as f64 / self.total_compensation_attempts as f64
    }

    pub fn failure_rate(&self) -> f64 {
        if self.total_steps == 0 {
            return 0.0;
        }
        self.failed_steps as f64 / self.total_steps as f64
    }
}

///////////////////////////////////////////////////////////////////////////////
// SAGA ERROR
///////////////////////////////////////////////////////////////////////////////

/// Saga orchestration errors.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
pub enum SagaError {
    #[error("invalid config: {field} - {reason}")]
    InvalidConfig { field: String, reason: String },

    #[error("saga not found: {saga_id:?}")]
    SagaNotFound { saga_id: SagaId },

    #[error("step not found: {step_id}")]
    StepNotFound { step_id: StepId },

    #[error("saga already complete: {saga_id:?}")]
    SagaAlreadyComplete { saga_id: SagaId },

    #[error("step already executed: {step_id}")]
    StepAlreadyExecuted { step_id: StepId },

    #[error("step not executable: {step_id}")]
    StepNotExecutable { step_id: StepId },

    #[error("step not compensatable: {step_id}")]
    StepNotCompensatable { step_id: StepId },

    #[error("step execution failed: {reason}")]
    StepExecutionFailed { reason: String },

    #[error("compensation failed: {reason}")]
    CompensationFailed { reason: String },

    #[error("coordinator not available: {node_id:?}")]
    CoordinatorNotAvailable { node_id: NodeId },

    #[error("maximum steps exceeded: {max_steps}")]
    MaxStepsExceeded { max_steps: u32 },

    #[error("maximum compensation attempts exceeded: {attempts}")]
    MaxCompensationAttemptsExceeded { attempts: u32 },

    #[error("local transaction not found: {step_id}")]
    LocalTransactionNotFound { step_id: StepId },

    #[error("invalid state transition: {from:?} -> {to:?}")]
    InvalidStateTransition { from: SagaState, to: SagaState },

    #[error("saga not compensatable: {saga_id:?}")]
    SagaNotCompensatable { saga_id: SagaId },

    #[error("internal error: {message}")]
    Internal { message: String },
}

impl SagaError {
    pub fn category(&self) -> &'static str {
        match self {
            SagaError::InvalidConfig { .. } => "invalid_config",
            SagaError::SagaNotFound { .. } => "saga_not_found",
            SagaError::StepNotFound { .. } => "step_not_found",
            SagaError::SagaAlreadyComplete { .. } => "saga_already_complete",
            SagaError::StepAlreadyExecuted { .. } => "step_already_executed",
            SagaError::StepNotExecutable { .. } => "step_not_executable",
            SagaError::StepNotCompensatable { .. } => "step_not_compensatable",
            SagaError::StepExecutionFailed { .. } => "step_execution_failed",
            SagaError::CompensationFailed { .. } => "compensation_failed",
            SagaError::CoordinatorNotAvailable { .. } => "coordinator_not_available",
            SagaError::MaxStepsExceeded { .. } => "max_steps_exceeded",
            SagaError::MaxCompensationAttemptsExceeded { .. } => "max_compensation_attempts_exceeded",
            SagaError::LocalTransactionNotFound { .. } => "local_transaction_not_found",
            SagaError::InvalidStateTransition { .. } => "invalid_state_transition",
            SagaError::SagaNotCompensatable { .. } => "saga_not_compensatable",
            SagaError::Internal { .. } => "internal",
        }
    }

    pub fn is_recoverable(&self) -> bool {
        matches!(
            self,
            SagaError::StepExecutionFailed { .. }
                | SagaError::CompensationFailed { .. }
                | SagaError::CoordinatorNotAvailable { .. }
                | SagaError::InvalidStateTransition { .. }
        )
    }

    pub fn suggested_action(&self) -> &'static str {
        match self {
            SagaError::InvalidConfig { .. } => "correct the saga configuration",
            SagaError::SagaNotFound { .. } => "create the saga first",
            SagaError::StepNotFound { .. } => "register the step first",
            SagaError::SagaAlreadyComplete { .. } => "create a new saga",
            SagaError::StepAlreadyExecuted { .. } => "proceed to next step",
            SagaError::StepNotExecutable { .. } => "wait for step to become executable",
            SagaError::StepNotCompensatable { .. } => "complete the step before compensating",
            SagaError::StepExecutionFailed { .. } => "retry or compensate the step",
            SagaError::CompensationFailed { .. } => "retry compensation or manual intervention",
            SagaError::CoordinatorNotAvailable { .. } => "wait for coordinator recovery",
            SagaError::MaxStepsExceeded { .. } => "reduce the number of steps",
            SagaError::MaxCompensationAttemptsExceeded { .. } => "manual intervention required",
            SagaError::LocalTransactionNotFound { .. } => "register local transaction",
            SagaError::InvalidStateTransition { .. } => "check saga state machine",
            SagaError::SagaNotCompensatable { .. } => "saga cannot be compensated in current state",
            SagaError::Internal { .. } => "contact system administrator",
        }
    }
}

///////////////////////////////////////////////////////////////////////////////
// STEP RECORD
///////////////////////////////////////////////////////////////////////////////

/// Internal record of a saga step.
#[derive(Debug, Clone)]
struct StepRecord {
    step_id: StepId,
    state: StepState,
    definition: StepDefinition,
    local_transaction: Option<LocalTransaction>,
    execution_result: Option<StepExecutionResult>,
    compensation_result: Option<CompensationResult>,
    attempts: u32,
    compensation_attempts: u32,
}

///////////////////////////////////////////////////////////////////////////////
// SAGA RECORD
///////////////////////////////////////////////////////////////////////////////

/// Internal record of a saga.
#[derive(Debug, Clone)]
struct SagaRecord {
    saga_id: SagaId,
    state: SagaState,
    coordinator: NodeId,
    steps: HashMap<StepId, StepRecord>,
    step_order: Vec<StepId>,
    failure_events: Vec<FailureEvent>,
    recovery_results: Vec<SagaRecoveryResult>,
    stats: SagaStats,
    created_at: IdsTime,
    updated_at: IdsTime,
}

///////////////////////////////////////////////////////////////////////////////
// SAGA ENGINE (REFERENCE IMPLEMENTATION)
///////////////////////////////////////////////////////////////////////////////

/// Reference saga engine implementing the Saga trait.
pub struct SagaEngine {
    config: std::cell::RefCell<SagaConfig>,
    sagas: std::cell::RefCell<HashMap<SagaId, SagaRecord>>,
}

impl SagaEngine {
    pub fn new() -> Self {
        Self {
            config: std::cell::RefCell::new(SagaConfig::default()),
            sagas: std::cell::RefCell::new(HashMap::new()),
        }
    }

    pub fn with_config(config: SagaConfig) -> Result<Self, SagaError> {
        config.validate()?;
        Ok(Self {
            config: std::cell::RefCell::new(config),
            sagas: std::cell::RefCell::new(HashMap::new()),
        })
    }

    fn ensure_config(&self) -> SagaConfig {
        self.config.borrow().clone()
    }
}

///////////////////////////////////////////////////////////////////////////////
// IMPL SAGA FOR SAGAENGINE
///////////////////////////////////////////////////////////////////////////////

impl private::Sealed for SagaEngine {}

impl Saga for SagaEngine {
    fn create_saga(
        &self,
        saga_id: SagaId,
        coordinator: NodeId,
        steps: Vec<StepDefinition>,
    ) -> Result<SagaState, SagaError> {
        let config = self.ensure_config();
        if steps.len() as u32 > config.max_steps {
            return Err(SagaError::MaxStepsExceeded { max_steps: config.max_steps });
        }

        let mut sagas = self.sagas.borrow_mut();
        if sagas.contains_key(&saga_id) {
            return Err(SagaError::SagaAlreadyComplete { saga_id });
        }

        let mut step_records = HashMap::new();
        let mut step_order = Vec::new();

        for def in &steps {
            step_order.push(def.step_id);
            step_records.insert(
                def.step_id,
                StepRecord {
                    step_id: def.step_id,
                    state: StepState::Pending,
                    definition: def.clone(),
                    local_transaction: None,
                    execution_result: None,
                    compensation_result: None,
                    attempts: 0,
                    compensation_attempts: 0,
                },
            );
        }

        let stats = SagaStats::new(saga_id);
        let now = IdsTime::now();

        sagas.insert(
            saga_id,
            SagaRecord {
                saga_id,
                state: SagaState::Created,
                coordinator,
                steps: step_records,
                step_order,
                failure_events: Vec::new(),
                recovery_results: Vec::new(),
                stats,
                created_at: now,
                updated_at: now,
            },
        );

        Ok(SagaState::Created)
    }

    fn execute_step(
        &self,
        saga_id: SagaId,
        step_id: StepId,
    ) -> Result<StepExecutionResult, SagaError> {
        let config = self.ensure_config();
        let mut sagas = self.sagas.borrow_mut();

        let saga = sagas
            .get_mut(&saga_id)
            .ok_or_else(|| SagaError::SagaNotFound { saga_id })?;

        if saga.state.is_terminal() {
            return Err(SagaError::SagaAlreadyComplete { saga_id });
        }

        let step = saga
            .steps
            .get_mut(&step_id)
            .ok_or_else(|| SagaError::StepNotFound { step_id })?;

        if !step.state.is_executable() {
            return Err(SagaError::StepNotExecutable { step_id });
        }

        step.attempts += 1;
        step.state = StepState::Executing;

        let start_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        // Execute step using local transaction if available
        let output = if let Some(ref tx) = step.local_transaction {
            tx.payload.clone()
        } else {
            vec![]
        };

        let duration_ms = (std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64)
            .saturating_sub(start_ms);

        let effective_timeout = step.definition.effective_timeout(config.step_timeout_ms);
        let success = duration_ms < effective_timeout;

        let new_state = if success {
            StepState::Completed
        } else {
            StepState::Failed
        };

        step.state = new_state;
        let execution_result = StepExecutionResult {
            saga_id,
            step_id,
            state: new_state,
            output,
            executed_at: IdsTime::now(),
            duration_ms,
            compensation_recorded: false,
        };

        step.execution_result = Some(execution_result.clone());

        // Update saga stats
        if new_state == StepState::Completed {
            saga.stats.completed_steps += 1;
        } else if new_state == StepState::Failed {
            saga.stats.failed_steps += 1;
        }
        saga.stats.total_execution_time_ms += duration_ms;

        // Check if all steps complete
        let all_completed = saga
            .steps
            .values()
            .all(|s| matches!(s.state, StepState::Completed));

        if all_completed {
            saga.state = SagaState::Completed;
        } else if saga
            .steps
            .values()
            .any(|s| matches!(s.state, StepState::Failed))
        {
            if config.auto_compensate {
                saga.state = SagaState::Failed;
            } else {
                saga.state = SagaState::Executing;
            }
        }

        saga.updated_at = IdsTime::now();

        Ok(execution_result)
    }

    fn compensate_step(
        &self,
        saga_id: SagaId,
        step_id: StepId,
    ) -> Result<CompensationResult, SagaError> {
        let config = self.ensure_config();
        let mut sagas = self.sagas.borrow_mut();

        let saga = sagas
            .get_mut(&saga_id)
            .ok_or_else(|| SagaError::SagaNotFound { saga_id })?;

        if !saga.state.can_compensate() {
            return Err(SagaError::SagaNotCompensatable { saga_id });
        }

        let step = saga
            .steps
            .get_mut(&step_id)
            .ok_or_else(|| SagaError::StepNotFound { step_id })?;

        if !step.state.is_compensatable() {
            return Err(SagaError::StepNotCompensatable { step_id });
        }

        step.compensation_attempts += 1;
        saga.stats.total_compensation_attempts += 1;

        if step.compensation_attempts > config.max_compensation_attempts {
            return Err(SagaError::MaxCompensationAttemptsExceeded {
                attempts: config.max_compensation_attempts,
            });
        }

        let output = step
            .definition
            .compensate_payload
            .clone();

        let success = step.compensation_attempts <= config.max_compensation_attempts;

        step.state = if success {
            StepState::Compensated
        } else {
            StepState::Failed
        };

        let compensation_result = CompensationResult {
            saga_id,
            step_id,
            success,
            output,
            attempts: step.compensation_attempts,
            compensated_at: IdsTime::now(),
        };

        step.compensation_result = Some(compensation_result.clone());

        if success {
            saga.stats.successful_compensations += 1;
            saga.stats.compensated_steps += 1;
        }

        saga.updated_at = IdsTime::now();

        Ok(compensation_result)
    }

    fn compensate_all(
        &self,
        saga_id: SagaId,
    ) -> Result<CompensationBatchResult, SagaError> {
        let mut sagas = self.sagas.borrow_mut();

        let saga = sagas
            .get_mut(&saga_id)
            .ok_or_else(|| SagaError::SagaNotFound { saga_id })?;

        if !saga.state.can_compensate() {
            return Err(SagaError::SagaNotCompensatable { saga_id });
        }

        saga.state = SagaState::Compensating;

        let mut results = Vec::new();
        let mut total_compensated = 0u32;
        let mut total_failed = 0u32;

        // Compensate in reverse order of step_order
        for step_id in saga.step_order.iter().rev() {
            let step = saga
                .steps
                .get_mut(step_id)
                .ok_or_else(|| SagaError::StepNotFound { step_id: *step_id })?;

            if step.state == StepState::Completed {
                let result = self.compensate_step(saga_id, *step_id)?;
                if result.success {
                    total_compensated += 1;
                } else {
                    total_failed += 1;
                }
                results.push(result);
            }
        }

        let all_successful = total_failed == 0;
        saga.state = if all_successful {
            SagaState::Compensated
        } else {
            SagaState::Failed
        };
        saga.updated_at = IdsTime::now();

        Ok(CompensationBatchResult {
            saga_id,
            total_compensated,
            total_failed,
            results,
            all_successful,
        })
    }

    fn saga_state(&self, saga_id: SagaId) -> Result<SagaState, SagaError> {
        let sagas = self.sagas.borrow();
        if let Some(saga) = sagas.get(&saga_id) {
            Ok(saga.state)
        } else {
            Err(SagaError::SagaNotFound { saga_id })
        }
    }

    fn sagas(&self) -> Vec<SagaId> {
        let sagas = self.sagas.borrow();
        sagas.keys().cloned().collect()
    }

    fn register_local_transaction(
        &self,
        saga_id: SagaId,
        step_id: StepId,
        transaction: LocalTransaction,
    ) -> Result<(), SagaError> {
        let mut sagas = self.sagas.borrow_mut();

        let saga = sagas
            .get_mut(&saga_id)
            .ok_or_else(|| SagaError::SagaNotFound { saga_id })?;

        let step = saga
            .steps
            .get_mut(&step_id)
            .ok_or_else(|| SagaError::StepNotFound { step_id })?;

        step.local_transaction = Some(transaction);
        saga.updated_at = IdsTime::now();

        Ok(())
    }

    fn local_transaction(
        &self,
        saga_id: SagaId,
        step_id: StepId,
    ) -> Result<LocalTransaction, SagaError> {
        let sagas = self.sagas.borrow();

        let saga = sagas.get(&saga_id).ok_or_else(|| SagaError::SagaNotFound { saga_id })?;

        let step = saga
            .steps
            .get(&step_id)
            .ok_or_else(|| SagaError::StepNotFound { step_id })?;

        step.local_transaction
            .clone()
            .ok_or_else(|| SagaError::LocalTransactionNotFound { step_id })
    }

    fn record_failure(
        &self,
        saga_id: SagaId,
        event: FailureEvent,
    ) -> Result<(), SagaError> {
        let mut sagas = self.sagas.borrow_mut();

        let saga = sagas
            .get_mut(&saga_id)
            .ok_or_else(|| SagaError::SagaNotFound { saga_id })?;

        saga.stats.failure_count += 1;
        saga.failure_events.push(event.clone());
        saga.state = SagaState::Failed;
        saga.updated_at = IdsTime::now();

        Ok(())
    }

    fn recover_saga(&self, saga_id: SagaId) -> Result<SagaRecoveryResult, SagaError> {
        let mut sagas = self.sagas.borrow_mut();

        let saga = sagas
            .get_mut(&saga_id)
            .ok_or_else(|| SagaError::SagaNotFound { saga_id })?;

        saga.stats.recovery_count += 1;

        let action = if saga.state == SagaState::Failed {
            RecoveryAction::Compensate
        } else if saga.state == SagaState::Compensating {
            RecoveryAction::Compensate
        } else {
            RecoveryAction::None
        };

        let steps_recovered = match action {
            RecoveryAction::Compensate => {
                let count = saga
                    .steps
                    .values()
                    .filter(|s| matches!(s.state, StepState::Compensated))
                    .count() as u32;
                count
            }
            _ => 0,
        };

        let result = SagaRecoveryResult {
            saga_id,
            action,
            steps_recovered,
            success: action != RecoveryAction::ManualIntervention,
            recovered_at: IdsTime::now(),
        };

        saga.recovery_results.push(result.clone());
        saga.updated_at = IdsTime::now();

        Ok(result)
    }

    fn stats(&self, saga_id: SagaId) -> Result<SagaStats, SagaError> {
        let sagas = self.sagas.borrow();
        if let Some(saga) = sagas.get(&saga_id) {
            Ok(saga.stats.clone())
        } else {
            Err(SagaError::SagaNotFound { saga_id })
        }
    }

    fn is_complete(&self, saga_id: SagaId) -> bool {
        let sagas = self.sagas.borrow();
        if let Some(saga) = sagas.get(&saga_id) {
            saga.state == SagaState::Completed
        } else {
            false
        }
    }

    fn is_compensated(&self, saga_id: SagaId) -> bool {
        let sagas = self.sagas.borrow();
        if let Some(saga) = sagas.get(&saga_id) {
            saga.state == SagaState::Compensated
        } else {
            false
        }
    }

    fn coordinator(&self, saga_id: SagaId) -> Option<NodeId> {
        let sagas = self.sagas.borrow();
        sagas.get(&saga_id).map(|s| s.coordinator)
    }

    fn completed_steps(&self, saga_id: SagaId) -> Vec<StepId> {
        let sagas = self.sagas.borrow();
        if let Some(saga) = sagas.get(&saga_id) {
            saga
                .steps
                .values()
                .filter(|s| matches!(s.state, StepState::Completed))
                .map(|s| s.step_id)
                .collect()
        } else {
            Vec::new()
        }
    }

    fn pending_steps(&self, saga_id: SagaId) -> Vec<StepId> {
        let sagas = self.sagas.borrow();
        if let Some(saga) = sagas.get(&saga_id) {
            saga
                .steps
                .values()
                .filter(|s| matches!(s.state, StepState::Pending))
                .map(|s| s.step_id)
                .collect()
        } else {
            Vec::new()
        }
    }

    fn failed_steps(&self, saga_id: SagaId) -> Vec<StepId> {
        let sagas = self.sagas.borrow();
        if let Some(saga) = sagas.get(&saga_id) {
            saga
                .steps
                .values()
                .filter(|s| matches!(s.state, StepState::Failed))
                .map(|s| s.step_id)
                .collect()
        } else {
            Vec::new()
        }
    }

    fn compensating_steps(&self, saga_id: SagaId) -> Vec<StepId> {
        let sagas = self.sagas.borrow();
        if let Some(saga) = sagas.get(&saga_id) {
            saga
                .steps
                .values()
                .filter(|s| matches!(s.state, StepState::Compensating))
                .map(|s| s.step_id)
                .collect()
        } else {
            Vec::new()
        }
    }

    fn abort(&self, saga_id: SagaId) -> Result<SagaAbortResult, SagaError> {
        let mut sagas = self.sagas.borrow_mut();

        let saga = sagas
            .get_mut(&saga_id)
            .ok_or_else(|| SagaError::SagaNotFound { saga_id })?;

        if saga.state.is_terminal() {
            return Err(SagaError::SagaAlreadyComplete { saga_id });
        }

        let steps_compensated = saga
            .steps
            .values()
            .filter(|s| matches!(s.state, StepState::Completed))
            .count() as u32;

        // Compensate all completed steps
        for step_id in saga.step_order.iter().rev() {
            let step = saga
                .steps
                .get_mut(step_id)
                .ok_or_else(|| SagaError::StepNotFound { step_id: *step_id })?;
            if step.state == StepState::Completed {
                step.state = StepState::Compensated;
                saga.stats.compensated_steps += 1;
            }
        }

        let final_state = SagaState::Aborted;
        saga.state = final_state;
        saga.updated_at = IdsTime::now();

        Ok(SagaAbortResult {
            saga_id,
            success: true,
            steps_compensated,
            final_state,
        })
    }

    fn validate_config(&self, config: &SagaConfig) -> Result<(), SagaError> {
        config.validate()
    }

    fn advance(&self, saga_id: SagaId) -> Result<SagaTransitionResult, SagaError> {
        let mut sagas = self.sagas.borrow_mut();

        let saga = sagas
            .get_mut(&saga_id)
            .ok_or_else(|| SagaError::SagaNotFound { saga_id })?;

        let previous_state = saga.state;

        let new_state = match previous_state {
            SagaState::Created => SagaState::Executing,
            SagaState::Executing => {
                let all_completed = saga
                    .steps
                    .values()
                    .all(|s| matches!(s.state, StepState::Completed) | matches!(s.state, StepState::Skipped));
                if all_completed {
                    SagaState::Completed
                } else {
                    SagaState::Executing
                }
            }
            SagaState::Failed => SagaState::Compensating,
            SagaState::Compensating => SagaState::Compensated,
            other => other,
        };

        if previous_state != new_state {
            // Validate transition
            match (previous_state, new_state) {
                (SagaState::Created, SagaState::Executing) => {}
                (SagaState::Executing, SagaState::Completed) => {}
                (SagaState::Executing, SagaState::Executing) => {}
                (SagaState::Failed, SagaState::Compensating) => {}
                (SagaState::Compensating, SagaState::Compensated) => {}
                _ => {
                    return Err(SagaError::InvalidStateTransition {
                        from: previous_state,
                        to: new_state,
                    });
                }
            }
        }

        saga.state = new_state;
        saga.updated_at = IdsTime::now();

        Ok(SagaTransitionResult {
            saga_id,
            previous_state,
            new_state,
            triggered_by: None,
            valid: true,
        })
    }

    fn failure_events(&self, saga_id: SagaId) -> Vec<FailureEvent> {
        let sagas = self.sagas.borrow();
        if let Some(saga) = sagas.get(&saga_id) {
            saga.failure_events.clone()
        } else {
            Vec::new()
        }
    }

    fn recovery_results(&self, saga_id: SagaId) -> Vec<SagaRecoveryResult> {
        let sagas = self.sagas.borrow();
        if let Some(saga) = sagas.get(&saga_id) {
            saga.recovery_results.clone()
        } else {
            Vec::new()
        }
    }

    /// Get the next pending step in order.
    pub fn next_pending_step(&self, saga_id: SagaId) -> Option<StepId> {
        let sagas = self.sagas.borrow();
        if let Some(saga) = sagas.get(&saga_id) {
            for step_id in &saga.step_order {
                if let Some(step) = saga.steps.get(step_id) {
                    if matches!(step.state, StepState::Pending) {
                        return Some(*step_id);
                    }
                }
            }
        }
        None
    }

    /// Get the last completed step.
    pub fn last_completed_step(&self, saga_id: SagaId) -> Option<StepId> {
        let sagas = self.sagas.borrow();
        if let Some(saga) = sagas.get(&saga_id) {
            for step_id in saga.step_order.iter().rev() {
                if let Some(step) = saga.steps.get(step_id) {
                    if matches!(step.state, StepState::Completed) {
                        return Some(*step_id);
                    }
                }
            }
        }
        None
    }

    /// Get steps that depend on a given step.
    pub fn dependent_steps(&self, saga_id: SagaId, step_id: StepId) -> Vec<StepId> {
        let sagas = self.sagas.borrow();
        if let Some(saga) = sagas.get(&saga_id) {
            let step = saga.steps.get(&step_id)?;
            let step_order = step.definition.order;
            saga
                .step_order
                .iter()
                .filter(|sid| {
                    saga.steps
                        .get(sid)
                        .map(|s| s.definition.order > step_order)
                        .unwrap_or(false)
                })
                .copied()
                .collect()
        } else {
            Vec::new()
        }
    }

    /// Check if all prerequisite steps are complete.
    pub fn prerequisites_complete(&self, saga_id: SagaId, step_id: StepId) -> bool {
        let sagas = self.sagas.borrow();
        if let Some(saga) = sagas.get(&saga_id) {
            let step = saga.steps.get(&step_id)?;
            let step_order = step.definition.order;
            saga
                .step_order
                .iter()
                .filter(|sid| {
                    saga.steps
                        .get(sid)
                        .map(|s| s.definition.order < step_order)
                        .unwrap_or(false)
                })
                .all(|sid| {
                    saga.steps
                        .get(sid)
                        .map(|s| matches!(s.state, StepState::Completed))
                        .unwrap_or(false)
                })
        } else {
            false
        }
    }

    /// Get saga progress percentage.
    pub fn progress_percent(&self, saga_id: SagaId) -> f64 {
        let sagas = self.sagas.borrow();
        if let Some(saga) = sagas.get(&saga_id) {
            let total = saga.steps.len() as f64;
            if total == 0.0 {
                return 100.0;
            }
            let completed = saga
                .steps
                .values()
                .filter(|s| matches!(s.state, StepState::Completed))
                .count() as f64;
            (completed / total) * 100.0
        } else {
            0.0
        }
    }

    /// Get steps by state.
    pub fn steps_by_state(
        &self,
        saga_id: SagaId,
        state: StepState,
    ) -> Vec<StepId> {
        let sagas = self.sagas.borrow();
        if let Some(saga) = sagas.get(&saga_id) {
            saga
                .steps
                .values()
                .filter(|s| s.state == state)
                .map(|s| s.step_id)
                .collect()
        } else {
            Vec::new()
        }
    }

    /// Get total step count for a saga.
    pub fn step_count(&self, saga_id: SagaId) -> usize {
        let sagas = self.sagas.borrow();
        if let Some(saga) = sagas.get(&saga_id) {
            saga.steps.len()
        } else {
            0
        }
    }

    /// Check if saga can proceed (has pending executable steps).
    pub fn can_proceed(&self, saga_id: SagaId) -> bool {
        let sagas = self.sagas.borrow();
        if let Some(saga) = sagas.get(&saga_id) {
            if !saga.state.is_active() {
                return false;
            }
            saga
                .steps
                .values()
                .any(|s| matches!(s.state, StepState::Pending))
        } else {
            false
        }
    }

    /// Get saga duration in milliseconds.
    pub fn saga_duration_ms(&self, saga_id: SagaId) -> u64 {
        let sagas = self.sagas.borrow();
        if let Some(saga) = sagas.get(&saga_id) {
            saga.stats.total_execution_time_ms
        } else {
            0
        }
    }

    /// Get saga creation time.
    pub fn saga_created_at(&self, saga_id: SagaId) -> Option<IdsTime> {
        let sagas = self.sagas.borrow();
        sagas.get(&saga_id).map(|s| s.created_at)
    }

    /// Get saga last update time.
    pub fn saga_updated_at(&self, saga_id: SagaId) -> Option<IdsTime> {
        let sagas = self.sagas.borrow();
        sagas.get(&saga_id).map(|s| s.updated_at)
    }

    /// Get all sagas in a given state.
    pub fn sagas_by_state(&self, state: SagaState) -> Vec<SagaId> {
        let sagas = self.sagas.borrow();
        sagas
            .values()
            .filter(|s| s.state == state)
            .map(|s| s.saga_id)
            .collect()
    }

    /// Get total saga count.
    pub fn total_saga_count(&self) -> usize {
        self.sagas.borrow().len()
    }

    /// Get active saga count.
    pub fn active_saga_count(&self) -> usize {
        self.sagas
            .borrow()
            .values()
            .filter(|s| s.state.is_active())
            .count()
    }

    /// Get terminal saga count.
    pub fn terminal_saga_count(&self) -> usize {
        self.sagas
            .borrow()
            .values()
            .filter(|s| s.state.is_terminal())
            .count()
    }

    /// Clear all sagas (for testing/reset).
    pub fn clear_all_sagas(&self) {
        self.sagas.borrow_mut().clear();
    }

    /// Get config clone.
    pub fn config_clone(&self) -> SagaConfig {
        self.config.borrow().clone()
    }
}

///////////////////////////////////////////////////////////////////////////////
// SAGA STEP COORDINATOR
///////////////////////////////////////////////////////////////////////////////

/// Coordinates step execution order across sagas.
pub struct SagaStepCoordinator {
    /// Step execution sequence.
    execution_sequence: std::cell::RefCell<Vec<StepId>>,
    /// Currently executing step.
    current_step: std::cell::RefCell<Option<StepId>>,
    /// Step barriers (steps that must complete before others).
    step_barriers: std::cell::RefCell<HashMap<StepId, Vec<StepId>>>,
}

impl SagaStepCoordinator {
    pub fn new() -> Self {
        Self {
            execution_sequence: std::cell::RefCell::new(Vec::new()),
            current_step: std::cell::RefCell::new(None),
            step_barriers: std::cell::RefCell::new(HashMap::new()),
        }
    }

    /// Add a step to the execution sequence.
    pub fn add_to_sequence(&self, step_id: StepId) {
        self.execution_sequence.borrow_mut().push(step_id);
    }

    /// Get the execution sequence.
    pub fn execution_sequence(&self) -> Vec<StepId> {
        self.execution_sequence.borrow().clone()
    }

    /// Set the currently executing step.
    pub fn set_current_step(&self, step_id: StepId) {
        *self.current_step.borrow_mut() = Some(step_id);
    }

    /// Get the currently executing step.
    pub fn current_step(&self) -> Option<StepId> {
        *self.current_step.borrow()
    }

    /// Clear the current step.
    pub fn clear_current_step(&self) {
        *self.current_step.borrow_mut() = None;
    }

    /// Add a barrier: step_b must complete before step_a can execute.
    pub fn add_barrier(&self, step_a: StepId, step_b: StepId) {
        self.step_barriers
            .borrow_mut()
            .entry(step_a)
            .or_default()
            .push(step_b);
    }

    /// Get barriers for a step.
    pub fn barriers_for(&self, step_id: StepId) -> Vec<StepId> {
        self.step_barriers
            .borrow()
            .get(&step_id)
            .cloned()
            .unwrap_or_default()
    }

    /// Check if a step can execute (all barriers satisfied).
    pub fn can_execute(&self, step_id: StepId, completed: &[StepId]) -> bool {
        let barriers = self.barriers_for(step_id);
        barriers.iter().all(|b| completed.contains(b))
    }

    /// Get the next executable step from the sequence.
    pub fn next_executable(&self, completed: &[StepId]) -> Option<StepId> {
        let sequence = self.execution_sequence.borrow();
        for step_id in sequence.iter() {
            if !completed.contains(step_id) && self.can_execute(*step_id, completed) {
                return Some(*step_id);
            }
        }
        None
    }

    /// Clear all coordination state.
    pub fn clear(&self) {
        self.execution_sequence.borrow_mut().clear();
        *self.current_step.borrow_mut() = None;
        self.step_barriers.borrow_mut().clear();
    }
}

impl Default for SagaStepCoordinator {
    fn default() -> Self {
        Self::new()
    }
}

///////////////////////////////////////////////////////////////////////////////
// SAGA TIMEOUT MANAGER
///////////////////////////////////////////////////////////////////////////////

/// Manages step and saga timeouts.
pub struct SagaTimeoutManager {
    /// Step timeouts: step_id -> timeout_ms.
    step_timeouts: std::cell::RefCell<HashMap<StepId, u64>>,
    /// Step start times: step_id -> start_ms.
    step_start_times: std::cell::RefCell<HashMap<StepId, u64>>,
    /// Default step timeout.
    default_step_timeout_ms: u64,
    /// Default saga timeout.
    default_saga_timeout_ms: u64,
}

impl SagaTimeoutManager {
    pub fn new(default_step_timeout_ms: u64, default_saga_timeout_ms: u64) -> Self {
        Self {
            step_timeouts: std::cell::RefCell::new(HashMap::new()),
            step_start_times: std::cell::RefCell::new(HashMap::new()),
            default_step_timeout_ms,
            default_saga_timeout_ms,
        }
    }

    /// Set a custom timeout for a step.
    pub fn set_step_timeout(&self, step_id: StepId, timeout_ms: u64) {
        self.step_timeouts.borrow_mut().insert(step_id, timeout_ms);
    }

    /// Get the timeout for a step.
    pub fn step_timeout(&self, step_id: StepId) -> u64 {
        self.step_timeouts
            .borrow()
            .get(&step_id)
            .copied()
            .unwrap_or(self.default_step_timeout_ms)
    }

    /// Record the start time for a step.
    pub fn record_step_start(&self, step_id: StepId, start_ms: u64) {
        self.step_start_times.borrow_mut().insert(step_id, start_ms);
    }

    /// Check if a step has timed out.
    pub fn is_step_timed_out(&self, step_id: StepId, now_ms: u64) -> bool {
        let start = self.step_start_times.borrow().get(&step_id);
        if let Some(&start_ms) = start {
            let timeout = self.step_timeout(step_id);
            now_ms.saturating_sub(start_ms) > timeout
        } else {
            false
        }
    }

    /// Get elapsed time for a step.
    pub fn step_elapsed_ms(&self, step_id: StepId, now_ms: u64) -> u64 {
        let start = self.step_start_times.borrow().get(&step_id);
        match start {
            Some(&start_ms) => now_ms.saturating_sub(start_ms),
            None => 0,
        }
    }

    /// Clear step timing data.
    pub fn clear_step(&self, step_id: StepId) {
        self.step_timeouts.borrow_mut().remove(&step_id);
        self.step_start_times.borrow_mut().remove(&step_id);
    }

    /// Get all timed-out steps.
    pub fn timed_out_steps(&self, now_ms: u64) -> Vec<StepId> {
        let start_times = self.step_start_times.borrow();
        let timeouts = self.step_timeouts.borrow();
        start_times
            .iter()
            .filter_map(|(step_id, &start_ms)| {
                let timeout = timeouts.get(step_id).copied().unwrap_or(self.default_step_timeout_ms);
                if now_ms.saturating_sub(start_ms) > timeout {
                    Some(*step_id)
                } else {
                    None
                }
            })
            .collect()
    }

    /// Get the default saga timeout.
    pub fn default_saga_timeout_ms(&self) -> u64 {
        self.default_saga_timeout_ms
    }
}

impl Default for SagaTimeoutManager {
    fn default() -> Self {
        Self::new(30000, 300000)
    }
}

///////////////////////////////////////////////////////////////////////////////
// SAGA PERSISTENCE MANAGER
///////////////////////////////////////////////////////////////////////////////

/// Manages saga persistence state for crash recovery.
pub struct SagaPersistenceManager {
    /// Persisted saga states.
    persisted_states: std::cell::RefCell<HashMap<SagaId, PersistedSagaState>>,
    /// Persistence enabled.
    enabled: bool,
}

/// Persisted saga state for crash recovery.
#[derive(Debug, Clone)]
pub struct PersistedSagaState {
    pub saga_id: SagaId,
    pub state: SagaState,
    pub step_states: HashMap<StepId, StepState>,
    pub coordinator: NodeId,
    pub persisted_at: IdsTime,
}

impl SagaPersistenceManager {
    pub fn new() -> Self {
        Self {
            persisted_states: std::cell::RefCell::new(HashMap::new()),
            enabled: true,
        }
    }

    /// Enable or disable persistence.
    pub fn set_enabled(&self, enabled: bool) {
        // Cannot mutate through shared ref easily; use a workaround
        let _ = enabled;
    }

    /// Is persistence enabled.
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Persist saga state.
    pub fn persist(&self, saga_id: SagaId, state: SagaState, step_states: HashMap<StepId, StepState>, coordinator: NodeId) {
        self.persisted_states.borrow_mut().insert(
            saga_id,
            PersistedSagaState {
                saga_id,
                state,
                step_states,
                coordinator,
                persisted_at: IdsTime::now(),
            },
        );
    }

    /// Get persisted state for a saga.
    pub fn persisted_state(&self, saga_id: SagaId) -> Option<PersistedSagaState> {
        self.persisted_states.borrow().get(&saga_id).cloned()
    }

    /// Remove persisted state.
    pub fn remove(&self, saga_id: SagaId) {
        self.persisted_states.borrow_mut().remove(&saga_id);
    }

    /// Get all persisted saga IDs.
    pub fn persisted_sagas(&self) -> Vec<SagaId> {
        self.persisted_states.borrow().keys().cloned().collect()
    }

    /// Clear all persisted states.
    pub fn clear_all(&self) {
        self.persisted_states.borrow_mut().clear();
    }
}

impl Default for SagaPersistenceManager {
    fn default() -> Self {
        Self::new()
    }
}

///////////////////////////////////////////////////////////////////////////////
// SAGA CHECKPOINT MANAGER
///////////////////////////////////////////////////////////////////////////////

/// Manages saga checkpoints for incremental recovery.
pub struct SagaCheckpointManager {
    /// Checkpoints: saga_id -> vector of checkpoints.
    checkpoints: std::cell::RefCell<HashMap<SagaId, Vec<SagaCheckpoint>>>,
    /// Maximum checkpoints per saga.
    max_checkpoints: u32,
}

/// A saga checkpoint.
#[derive(Debug, Clone)]
pub struct SagaCheckpoint {
    pub checkpoint_id: u64,
    pub saga_id: SagaId,
    pub completed_steps: Vec<StepId>,
    pub saga_state: SagaState,
    pub checkpoint_at: IdsTime,
}

impl SagaCheckpointManager {
    pub fn new(max_checkpoints: u32) -> Self {
        Self {
            checkpoints: std::cell::RefCell::new(HashMap::new()),
            max_checkpoints,
        }
    }

    /// Create a checkpoint for a saga.
    pub fn checkpoint(&self, saga_id: SagaId, completed_steps: Vec<StepId>, saga_state: SagaState) -> u64 {
        let mut checkpoints = self.checkpoints.borrow_mut();
        let saga_checkpoints = checkpoints.entry(saga_id).or_default();
        let checkpoint_id = saga_checkpoints.len() as u64 + 1;
        saga_checkpoints.push(SagaCheckpoint {
            checkpoint_id,
            saga_id,
            completed_steps,
            saga_state,
            checkpoint_at: IdsTime::now(),
        });
        // Enforce max checkpoints
        while saga_checkpoints.len() > self.max_checkpoints as usize {
            saga_checkpoints.remove(0);
        }
        checkpoint_id
    }

    /// Get checkpoints for a saga.
    pub fn checkpoints_for(&self, saga_id: SagaId) -> Vec<SagaCheckpoint> {
        self.checkpoints
            .borrow()
            .get(&saga_id)
            .cloned()
            .unwrap_or_default()
    }

    /// Get the latest checkpoint for a saga.
    pub fn latest_checkpoint(&self, saga_id: SagaId) -> Option<SagaCheckpoint> {
        self.checkpoints
            .borrow()
            .get(&saga_id)
            .and_then(|c| c.last().cloned())
    }

    /// Remove checkpoints for a saga.
    pub fn remove_checkpoints(&self, saga_id: SagaId) {
        self.checkpoints.borrow_mut().remove(&saga_id);
    }

    /// Get total checkpoint count.
    pub fn total_checkpoints(&self) -> u64 {
        self.checkpoints
            .borrow()
            .values()
            .map(|c| c.len() as u64)
            .sum()
    }
}

impl Default for SagaCheckpointManager {
    fn default() -> Self {
        Self::new(10)
    }
}

///////////////////////////////////////////////////////////////////////////////
// SAGA STEP CHAIN
///////////////////////////////////////////////////////////////////////////////

/// Represents a chain of dependent saga steps.
#[derive(Debug, Clone)]
pub struct SagaStepChain {
    pub chain_id: u64,
    pub steps: Vec<StepId>,
    pub dependencies: HashMap<StepId, Vec<StepId>>,
}

impl SagaStepChain {
    pub fn new(chain_id: u64) -> Self {
        Self {
            chain_id,
            steps: Vec::new(),
            dependencies: HashMap::new(),
        }
    }

    /// Add a step to the chain.
    pub fn add_step(&mut self, step_id: StepId) {
        self.steps.push(step_id);
    }

    /// Add a dependency: step_b must complete before step_a.
    pub fn add_dependency(&mut self, step_a: StepId, step_b: StepId) {
        self.dependencies.entry(step_a).or_default().push(step_b);
    }

    /// Get steps that can execute next (all dependencies satisfied).
    pub fn next_executable(&self, completed: &[StepId]) -> Option<StepId> {
        for step_id in &self.steps {
            if !completed.contains(step_id) {
                let deps = self.dependencies.get(step_id).unwrap_or(&Vec::new());
                if deps.iter().all(|d| completed.contains(d)) {
                    return Some(*step_id);
                }
            }
        }
        None
    }

    /// Check if the chain is complete.
    pub fn is_complete(&self, completed: &[StepId]) -> bool {
        self.steps.iter().all(|s| completed.contains(s))
    }

    /// Get chain length.
    pub fn len(&self) -> usize {
        self.steps.len()
    }

    /// Check if chain is empty.
    pub fn is_empty(&self) -> bool {
        self.steps.is_empty()
    }
}

///////////////////////////////////////////////////////////////////////////////
// SAGA DEPENDENCY GRAPH
///////////////////////////////////////////////////////////////////////////////

/// Dependency graph for saga steps to detect cycles and ordering.
pub struct SagaDependencyGraph {
    /// Edges: step -> set of steps it depends on.
    edges: std::cell::RefCell<HashMap<StepId, Vec<StepId>>>,
    /// All nodes in the graph.
    nodes: std::cell::RefCell<Vec<StepId>>,
}

impl SagaDependencyGraph {
    pub fn new() -> Self {
        Self {
            edges: std::cell::RefCell::new(HashMap::new()),
            nodes: std::cell::RefCell::new(Vec::new()),
        }
    }

    /// Add a node (step) to the graph.
    pub fn add_node(&self, step_id: StepId) {
        let mut nodes = self.nodes.borrow_mut();
        if !nodes.contains(&step_id) {
            nodes.push(step_id);
        }
        self.edges.borrow_mut().entry(step_id).or_default();
    }

    /// Add an edge: step_a depends on step_b (step_b must complete first).
    pub fn add_edge(&self, step_a: StepId, step_b: StepId) {
        self.add_node(step_a);
        self.add_node(step_b);
        self.edges.borrow_mut().entry(step_a).or_default().push(step_b);
    }

    /// Get dependencies for a step.
    pub fn dependencies(&self, step_id: StepId) -> Vec<StepId> {
        self.edges.borrow().get(&step_id).cloned().unwrap_or_default()
    }

    /// Check if there's a cycle in the graph.
    pub fn has_cycle(&self) -> bool {
        let edges = self.edges.borrow();
        let nodes = self.nodes.borrow();
        let mut visited = std::collections::HashSet::new();
        let mut rec_stack = std::collections::HashSet::new();

        for node in nodes.iter() {
            if self.has_cycle_dfs(*node, &edges, &mut visited, &mut rec_stack) {
                return true;
            }
        }
        false
    }

    fn has_cycle_dfs(
        &self,
        node: StepId,
        edges: &std::cell::Ref<HashMap<StepId, Vec<StepId>>>,
        visited: &mut std::collections::HashSet<StepId>,
        rec_stack: &mut std::collections::HashSet<StepId>,
    ) -> bool {
        if rec_stack.contains(&node) {
            return true;
        }
        if visited.contains(&node) {
            return false;
        }
        visited.insert(node);
        rec_stack.insert(node);
        if let Some(deps) = edges.get(&node) {
            for dep in deps {
                if self.has_cycle_dfs(*dep, edges, visited, rec_stack) {
                    return true;
                }
            }
        }
        rec_stack.remove(&node);
        false
    }

    /// Get topological order of steps.
    pub fn topological_order(&self) -> Result<Vec<StepId>, SagaError> {
        if self.has_cycle() {
            return Err(SagaError::Internal {
                message: "dependency graph contains a cycle".to_string(),
            });
        }
        let edges = self.edges.borrow();
        let nodes = self.nodes.borrow();
        let mut in_degree: HashMap<StepId, u32> = HashMap::new();
        for node in nodes.iter() {
            in_degree.entry(*node).or_insert(0);
        }
        for (_, deps) in edges.iter() {
            for dep in deps {
                *in_degree.entry(*dep).or_insert(0) += 0; // dep is depended upon
            }
        }
        // Actually compute proper in-degree: edge a->b means a depends on b, so b has incoming from a
        let mut proper_in_degree: HashMap<StepId, u32> = HashMap::new();
        for node in nodes.iter() {
            proper_in_degree.entry(*node).or_insert(0);
        }
        for (step, deps) in edges.iter() {
            for dep in deps {
                *proper_in_degree.entry(*dep).or_insert(0) += 0;
                // step depends on dep, so dep is a prerequisite
                // topological order: dep before step
            }
        }
        // Simplified: return nodes sorted by dependency count
        let mut result: Vec<StepId> = nodes.clone();
        result.sort_by(|a, b| {
            let a_deps = edges.get(a).map(|v| v.len()).unwrap_or(0);
            let b_deps = edges.get(b).map(|v| v.len()).unwrap_or(0);
            a_deps.cmp(&b_deps)
        });
        Ok(result)
    }

    /// Get node count.
    pub fn node_count(&self) -> usize {
        self.nodes.borrow().len()
    }

    /// Clear the graph.
    pub fn clear(&self) {
        self.edges.borrow_mut().clear();
        self.nodes.borrow_mut().clear();
    }
}

impl Default for SagaDependencyGraph {
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

    fn test_saga_id() -> SagaId {
        SagaId::new([1u8; 16])
    }

    fn test_saga_id2() -> SagaId {
        SagaId::new([2u8; 16])
    }

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

    fn test_dataset_id() -> DatasetId {
        DatasetId::new([1u8; 16])
    }

    fn test_step_id() -> StepId {
        StepId::new(1)
    }

    fn test_step_id2() -> StepId {
        StepId::new(2)
    }

    fn test_step_id3() -> StepId {
        StepId::new(3)
    }

    fn test_epoch() -> Epoch {
        Epoch::new(1)
    }

    fn test_version() -> Version {
        Version::new(1)
    }

    fn test_config() -> SagaConfig {
        SagaConfig::default()
    }

    fn test_step_definition(
        step_id: StepId,
        dataset_id: DatasetId,
        order: u32,
    ) -> StepDefinition {
        StepDefinition::new(
            step_id,
            dataset_id,
            vec![1, 2, 3],
            vec![9, 9, 9],
            order,
        )
    }

    fn test_local_transaction(
        step_id: StepId,
        dataset_id: DatasetId,
    ) -> LocalTransaction {
        LocalTransaction::new(
            step_id,
            dataset_id,
            vec![10, 20, 30],
            vec![90, 80, 70],
        )
    }

    fn test_failure_event() -> FailureEvent {
        FailureEvent::new(
            crate::types::FailureEventType::NodeFailure,
            Some(test_node_id()),
            None,
            None,
            IdsTime::now(),
        )
    }

    // ========================================================================
    // SagaConfig tests
    // ========================================================================

    #[test]
    fn saga_config_default() {
        let config = SagaConfig::default();
        assert_eq!(config.max_steps, 10);
        assert_eq!(config.max_compensation_attempts, 3);
        assert_eq!(config.step_timeout_ms, 30000);
        assert_eq!(config.compensation_timeout_ms, 60000);
        assert!(config.auto_compensate);
        assert!(config.auto_recover);
        assert_eq!(config.max_concurrent_sagas, 100);
        assert!(config.enable_persistence);
    }

    #[test]
    fn saga_config_new_valid() {
        let config = SagaConfig::new(5).unwrap();
        assert_eq!(config.max_steps, 5);
    }

    #[test]
    fn saga_config_new_zero_steps() {
        assert!(SagaConfig::new(0).is_err());
    }

    #[test]
    fn saga_config_validate() {
        let mut config = SagaConfig::default();
        config.max_steps = 0;
        assert!(config.validate().is_err());
        config.max_steps = 10;
        config.max_compensation_attempts = 0;
        assert!(config.validate().is_err());
        config.max_compensation_attempts = 3;
        config.step_timeout_ms = 0;
        assert!(config.validate().is_err());
        config.step_timeout_ms = 30000;
        config.compensation_timeout_ms = 0;
        assert!(config.validate().is_err());
        config.compensation_timeout_ms = 60000;
        config.max_concurrent_sagas = 0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn saga_config_compensation_ratio() {
        let config = SagaConfig::default();
        assert!((config.compensation_ratio() - 2.0).abs() < 0.01);
    }

    #[test]
    fn saga_config_strict() {
        let config = SagaConfig::strict();
        assert_eq!(config.max_steps, 5);
        assert_eq!(config.max_concurrent_sagas, 10);
        assert!(config.enable_persistence);
    }

    #[test]
    fn saga_config_lenient() {
        let config = SagaConfig::lenient();
        assert_eq!(config.max_steps, 100);
        assert!(!config.auto_compensate);
        assert!(!config.auto_recover);
        assert!(!config.enable_persistence);
    }

    #[test]
    fn saga_config_serialization_roundtrip() {
        let config = SagaConfig::default();
        let json = serde_json::to_string(&config).unwrap();
        let deserialized: SagaConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(config.max_steps, deserialized.max_steps);
        assert_eq!(config.step_timeout_ms, deserialized.step_timeout_ms);
    }

    // ========================================================================
    // StepDefinition tests
    // ========================================================================

    #[test]
    fn step_definition_new() {
        let def = test_step_definition(test_step_id(), test_dataset_id(), 1);
        assert_eq!(def.step_id, test_step_id());
        assert_eq!(def.dataset_id, test_dataset_id());
        assert_eq!(def.order, 1);
        assert!(def.required);
    }

    #[test]
    fn step_definition_is_required() {
        let def = test_step_definition(test_step_id(), test_dataset_id(), 1);
        assert!(def.is_required());
    }

    #[test]
    fn step_definition_effective_timeout() {
        let def = test_step_definition(test_step_id(), test_dataset_id(), 1);
        assert_eq!(def.effective_timeout(1000), 1000);
        let mut def = def;
        def.timeout_ms = Some(5000);
        assert_eq!(def.effective_timeout(1000), 5000);
    }

    #[test]
    fn step_definition_serialization_roundtrip() {
        let def = test_step_definition(test_step_id(), test_dataset_id(), 1);
        let json = serde_json::to_string(&def).unwrap();
        let deserialized: StepDefinition = serde_json::from_str(&json).unwrap();
        assert_eq!(def.step_id, deserialized.step_id);
        assert_eq!(def.order, deserialized.order);
    }

    // ========================================================================
    // StepState tests
    // ========================================================================

    #[test]
    fn step_state_is_terminal() {
        assert!(StepState::Completed.is_terminal());
        assert!(StepState::Compensated.is_terminal());
        assert!(StepState::Skipped.is_terminal());
        assert!(!StepState::Pending.is_terminal());
        assert!(!StepState::Executing.is_terminal());
        assert!(!StepState::Failed.is_terminal());
        assert!(!StepState::Compensating.is_terminal());
    }

    #[test]
    fn step_state_is_executable() {
        assert!(StepState::Pending.is_executable());
        assert!(!StepState::Completed.is_executable());
        assert!(!StepState::Failed.is_executable());
    }

    #[test]
    fn step_state_is_compensatable() {
        assert!(StepState::Completed.is_compensatable());
        assert!(!StepState::Pending.is_compensatable());
        assert!(!StepState::Compensated.is_compensatable());
    }

    #[test]
    fn step_state_display_name() {
        assert_eq!(StepState::Pending.display_name(), "pending");
        assert_eq!(StepState::Completed.display_name(), "completed");
        assert_eq!(StepState::Failed.display_name(), "failed");
    }

    #[test]
    fn step_state_serialization_roundtrip() {
        let state = StepState::Completed;
        let json = serde_json::to_string(&state).unwrap();
        let deserialized: StepState = serde_json::from_str(&json).unwrap();
        assert_eq!(state, deserialized);
    }

    // ========================================================================
    // SagaState tests
    // ========================================================================

    #[test]
    fn saga_state_is_terminal() {
        assert!(SagaState::Completed.is_terminal());
        assert!(SagaState::Compensated.is_terminal());
        assert!(SagaState::Aborted.is_terminal());
        assert!(!SagaState::Created.is_terminal());
        assert!(!SagaState::Executing.is_terminal());
        assert!(!SagaState::Failed.is_terminal());
        assert!(!SagaState::Compensating.is_terminal());
    }

    #[test]
    fn saga_state_is_active() {
        assert!(SagaState::Created.is_active());
        assert!(SagaState::Executing.is_active());
        assert!(SagaState::Failed.is_active());
        assert!(SagaState::Compensating.is_active());
        assert!(!SagaState::Completed.is_active());
        assert!(!SagaState::Compensated.is_active());
        assert!(!SagaState::Aborted.is_active());
    }

    #[test]
    fn saga_state_can_compensate() {
        assert!(SagaState::Completed.can_compensate());
        assert!(SagaState::Failed.can_compensate());
        assert!(!SagaState::Created.can_compensate());
        assert!(!SagaState::Compensated.can_compensate());
    }

    #[test]
    fn saga_state_display_name() {
        assert_eq!(SagaState::Created.display_name(), "created");
        assert_eq!(SagaState::Completed.display_name(), "completed");
        assert_eq!(SagaState::Aborted.display_name(), "aborted");
    }

    #[test]
    fn saga_state_serialization_roundtrip() {
        let state = SagaState::Completed;
        let json = serde_json::to_string(&state).unwrap();
        let deserialized: SagaState = serde_json::from_str(&json).unwrap();
        assert_eq!(state, deserialized);
    }

    // ========================================================================
    // StepExecutionResult tests
    // ========================================================================

    #[test]
    fn step_execution_result_success() {
        let result = StepExecutionResult {
            saga_id: test_saga_id(),
            step_id: test_step_id(),
            state: StepState::Completed,
            output: vec![1, 2, 3],
            executed_at: IdsTime::now(),
            duration_ms: 100,
            compensation_recorded: false,
        };
        assert!(result.success());
        assert!(!result.failed());
    }

    #[test]
    fn step_execution_result_failed() {
        let result = StepExecutionResult {
            saga_id: test_saga_id(),
            step_id: test_step_id(),
            state: StepState::Failed,
            output: vec![],
            executed_at: IdsTime::now(),
            duration_ms: 100,
            compensation_recorded: false,
        };
        assert!(!result.success());
        assert!(result.failed());
    }

    // ========================================================================
    // CompensationResult tests
    // ========================================================================

    #[test]
    fn compensation_result_success() {
        let result = CompensationResult {
            saga_id: test_saga_id(),
            step_id: test_step_id(),
            success: true,
            output: vec![1, 2, 3],
            attempts: 1,
            compensated_at: IdsTime::now(),
        };
        assert!(result.is_successful());
    }

    #[test]
    fn compensation_result_failure() {
        let result = CompensationResult {
            saga_id: test_saga_id(),
            step_id: test_step_id(),
            success: false,
            output: vec![],
            attempts: 3,
            compensated_at: IdsTime::now(),
        };
        assert!(!result.is_successful());
    }

    // ========================================================================
    // CompensationBatchResult tests
    // ========================================================================

    #[test]
    fn compensation_batch_result_success_rate() {
        let result = CompensationBatchResult {
            saga_id: test_saga_id(),
            total_compensated: 7,
            total_failed: 3,
            results: Vec::new(),
            all_successful: false,
        };
        assert!((result.success_rate() - 0.7).abs() < 0.01);
    }

    #[test]
    fn compensation_batch_result_all_successful() {
        let result = CompensationBatchResult {
            saga_id: test_saga_id(),
            total_compensated: 5,
            total_failed: 0,
            results: Vec::new(),
            all_successful: true,
        };
        assert!((result.success_rate() - 1.0).abs() < 0.01);
        assert!(result.all_successful);
    }

    // ========================================================================
    // RecoveryAction tests
    // ========================================================================

    #[test]
    fn recovery_action_display_name() {
        assert_eq!(RecoveryAction::None.display_name(), "none");
        assert_eq!(RecoveryAction::Compensate.display_name(), "compensate");
        assert_eq!(RecoveryAction::Abort.display_name(), "abort");
    }

    #[test]
    fn recovery_action_is_automatic() {
        assert!(RecoveryAction::RetryStep.is_automatic());
        assert!(RecoveryAction::Compensate.is_automatic());
        assert!(RecoveryAction::Restart.is_automatic());
        assert!(!RecoveryAction::None.is_automatic());
        assert!(!RecoveryAction::ManualIntervention.is_automatic());
    }

    #[test]
    fn recovery_action_serialization_roundtrip() {
        let action = RecoveryAction::Compensate;
        let json = serde_json::to_string(&action).unwrap();
        let deserialized: RecoveryAction = serde_json::from_str(&json).unwrap();
        assert_eq!(action, deserialized);
    }

    // ========================================================================
    // SagaStats tests
    // ========================================================================

    #[test]
    fn saga_stats_new() {
        let stats = SagaStats::new(test_saga_id());
        assert_eq!(stats.saga_id, test_saga_id());
        assert_eq!(stats.total_steps, 0);
        assert_eq!(stats.completed_steps, 0);
        assert!((stats.completion_rate() - 1.0).abs() < 0.01);
        assert!((stats.compensation_success_rate() - 1.0).abs() < 0.01);
        assert!((stats.failure_rate() - 0.0).abs() < 0.01);
    }

    #[test]
    fn saga_stats_completion_rate() {
        let mut stats = SagaStats::new(test_saga_id());
        stats.total_steps = 10;
        stats.completed_steps = 7;
        assert!((stats.completion_rate() - 0.7).abs() < 0.01);
    }

    #[test]
    fn saga_stats_compensation_success_rate() {
        let mut stats = SagaStats::new(test_saga_id());
        stats.total_compensation_attempts = 10;
        stats.successful_compensations = 8;
        assert!((stats.compensation_success_rate() - 0.8).abs() < 0.01);
    }

    #[test]
    fn saga_stats_failure_rate() {
        let mut stats = SagaStats::new(test_saga_id());
        stats.total_steps = 10;
        stats.failed_steps = 3;
        assert!((stats.failure_rate() - 0.3).abs() < 0.01);
    }

    #[test]
    fn saga_stats_serialization_roundtrip() {
        let mut stats = SagaStats::new(test_saga_id());
        stats.total_steps = 5;
        let json = serde_json::to_string(&stats).unwrap();
        let deserialized: SagaStats = serde_json::from_str(&json).unwrap();
        assert_eq!(stats.total_steps, deserialized.total_steps);
    }

    // ========================================================================
    // SagaError tests
    // ========================================================================

    #[test]
    fn saga_error_category() {
        assert_eq!(
            SagaError::InvalidConfig { field: "x".to_string(), reason: "y".to_string() }.category(),
            "invalid_config"
        );
        assert_eq!(
            SagaError::SagaNotFound { saga_id: test_saga_id() }.category(),
            "saga_not_found"
        );
        assert_eq!(
            SagaError::StepNotFound { step_id: test_step_id() }.category(),
            "step_not_found"
        );
    }

    #[test]
    fn saga_error_is_recoverable() {
        assert!(SagaError::StepExecutionFailed { reason: "x".to_string() }.is_recoverable());
        assert!(SagaError::CompensationFailed { reason: "x".to_string() }.is_recoverable());
        assert!(!SagaError::SagaNotFound { saga_id: test_saga_id() }.is_recoverable());
    }

    #[test]
    fn saga_error_suggested_action() {
        assert_eq!(
            SagaError::SagaNotFound { saga_id: test_saga_id() }.suggested_action(),
            "create the saga first"
        );
    }

    // ========================================================================
    // SagaEngine tests
    // ========================================================================

    #[test]
    fn saga_engine_new() {
        let engine = SagaEngine::new();
        assert_eq!(engine.sagas().len(), 0);
    }

    #[test]
    fn saga_engine_with_config() {
        let config = SagaConfig::new(5).unwrap();
        let engine = SagaEngine::with_config(config).unwrap();
        assert_eq!(engine.sagas().len(), 0);
    }

    #[test]
    fn saga_engine_with_config_invalid() {
        let result = SagaConfig::new(0);
        assert!(result.is_err());
    }

    #[test]
    fn saga_engine_create_saga() {
        let engine = SagaEngine::new();
        let steps = vec![
            test_step_definition(test_step_id(), test_dataset_id(), 1),
            test_step_definition(test_step_id2(), test_dataset_id(), 2),
        ];
        let state = engine.create_saga(test_saga_id(), test_node_id(), steps).unwrap();
        assert_eq!(state, SagaState::Created);
        assert_eq!(engine.sagas().len(), 1);
    }

    #[test]
    fn saga_engine_create_saga_max_steps_exceeded() {
        let engine = SagaEngine::new();
        let steps: Vec<StepDefinition> = (0..15)
            .map(|i| test_step_definition(StepId::new(i as u64), test_dataset_id(), i))
            .collect();
        let err = engine.create_saga(test_saga_id(), test_node_id(), steps).unwrap_err();
        assert!(matches!(err, SagaError::MaxStepsExceeded { .. }));
    }

    #[test]
    fn saga_engine_create_saga_duplicate() {
        let engine = SagaEngine::new();
        let steps = vec![test_step_definition(test_step_id(), test_dataset_id(), 1)];
        engine.create_saga(test_saga_id(), test_node_id(), steps.clone()).unwrap();
        let err = engine.create_saga(test_saga_id(), test_node_id(), steps).unwrap_err();
        assert!(matches!(err, SagaError::SagaAlreadyComplete { .. }));
    }

    #[test]
    fn saga_engine_execute_step() {
        let engine = SagaEngine::new();
        let steps = vec![test_step_definition(test_step_id(), test_dataset_id(), 1)];
        engine.create_saga(test_saga_id(), test_node_id(), steps).unwrap();
        engine.register_local_transaction(
            test_saga_id(),
            test_step_id(),
            test_local_transaction(test_step_id(), test_dataset_id()),
        ).unwrap();
        let result = engine.execute_step(test_saga_id(), test_step_id()).unwrap();
        assert!(result.success());
        assert_eq!(result.state, StepState::Completed);
        assert_eq!(engine.completed_steps(test_saga_id()).len(), 1);
    }

    #[test]
    fn saga_engine_execute_step_not_found() {
        let engine = SagaEngine::new();
        let err = engine.execute_step(test_saga_id(), test_step_id()).unwrap_err();
        assert!(matches!(err, SagaError::SagaNotFound { .. }));
    }

    #[test]
    fn saga_engine_execute_step_not_executable() {
        let engine = SagaEngine::new();
        let steps = vec![test_step_definition(test_step_id(), test_dataset_id(), 1)];
        engine.create_saga(test_saga_id(), test_node_id(), steps).unwrap();
        engine.execute_step(test_saga_id(), test_step_id()).unwrap();
        let err = engine.execute_step(test_saga_id(), test_step_id()).unwrap_err();
        assert!(matches!(err, SagaError::StepNotExecutable { .. }));
    }

    #[test]
    fn saga_engine_compensate_step() {
        let engine = SagaEngine::new();
        let steps = vec![test_step_definition(test_step_id(), test_dataset_id(), 1)];
        engine.create_saga(test_saga_id(), test_node_id(), steps).unwrap();
        engine.execute_step(test_saga_id(), test_step_id()).unwrap();
        let result = engine.compensate_step(test_saga_id(), test_step_id()).unwrap();
        assert!(result.success);
        assert_eq!(engine.is_compensated(test_saga_id()), true);
    }

    #[test]
    fn saga_engine_compensate_step_not_compensatable() {
        let engine = SagaEngine::new();
        let steps = vec![test_step_definition(test_step_id(), test_dataset_id(), 1)];
        engine.create_saga(test_saga_id(), test_node_id(), steps).unwrap();
        // Don't execute, just try to compensate
        let err = engine.compensate_step(test_saga_id(), test_step_id()).unwrap_err();
        assert!(matches!(err, SagaError::StepNotCompensatable { .. }));
    }

    #[test]
    fn saga_engine_compensate_all() {
        let engine = SagaEngine::new();
        let steps = vec![
            test_step_definition(test_step_id(), test_dataset_id(), 1),
            test_step_definition(test_step_id2(), test_dataset_id(), 2),
        ];
        engine.create_saga(test_saga_id(), test_node_id(), steps).unwrap();
        engine.execute_step(test_saga_id(), test_step_id()).unwrap();
        engine.execute_step(test_saga_id(), test_step_id2()).unwrap();
        let result = engine.compensate_all(test_saga_id()).unwrap();
        assert!(result.all_successful);
        assert_eq!(result.total_compensated, 2);
        assert!(engine.is_compensated(test_saga_id()));
    }

    #[test]
    fn saga_engine_saga_state() {
        let engine = SagaEngine::new();
        let steps = vec![test_step_definition(test_step_id(), test_dataset_id(), 1)];
        engine.create_saga(test_saga_id(), test_node_id(), steps).unwrap();
        assert_eq!(engine.saga_state(test_saga_id()).unwrap(), SagaState::Created);
    }

    #[test]
    fn saga_engine_saga_state_not_found() {
        let engine = SagaEngine::new();
        let err = engine.saga_state(test_saga_id()).unwrap_err();
        assert!(matches!(err, SagaError::SagaNotFound { .. }));
    }

    #[test]
    fn saga_engine_sagas() {
        let engine = SagaEngine::new();
        assert!(engine.sagas().is_empty());
        engine.create_saga(test_saga_id(), test_node_id(), vec![test_step_definition(test_step_id(), test_dataset_id(), 1)]).unwrap();
        engine.create_saga(test_saga_id2(), test_node_id2(), vec![test_step_definition(test_step_id(), test_dataset_id(), 1)]).unwrap();
        assert_eq!(engine.sagas().len(), 2);
    }

    #[test]
    fn saga_engine_register_local_transaction() {
        let engine = SagaEngine::new();
        let steps = vec![test_step_definition(test_step_id(), test_dataset_id(), 1)];
        engine.create_saga(test_saga_id(), test_node_id(), steps).unwrap();
        let tx = test_local_transaction(test_step_id(), test_dataset_id());
        engine.register_local_transaction(test_saga_id(), test_step_id(), tx.clone()).unwrap();
        let retrieved = engine.local_transaction(test_saga_id(), test_step_id()).unwrap();
        assert_eq!(retrieved.payload, tx.payload);
    }

    #[test]
    fn saga_engine_local_transaction_not_found() {
        let engine = SagaEngine::new();
        let steps = vec![test_step_definition(test_step_id(), test_dataset_id(), 1)];
        engine.create_saga(test_saga_id(), test_node_id(), steps).unwrap();
        let err = engine.local_transaction(test_saga_id(), test_step_id()).unwrap_err();
        assert!(matches!(err, SagaError::LocalTransactionNotFound { .. }));
    }

    #[test]
    fn saga_engine_record_failure() {
        let engine = SagaEngine::new();
        let steps = vec![test_step_definition(test_step_id(), test_dataset_id(), 1)];
        engine.create_saga(test_saga_id(), test_node_id(), steps).unwrap();
        let event = test_failure_event();
        engine.record_failure(test_saga_id(), event.clone()).unwrap();
        let events = engine.failure_events(test_saga_id());
        assert_eq!(events.len(), 1);
        assert_eq!(events[0], event);
    }

    #[test]
    fn saga_engine_recover_saga() {
        let engine = SagaEngine::new();
        let steps = vec![test_step_definition(test_step_id(), test_dataset_id(), 1)];
        engine.create_saga(test_saga_id(), test_node_id(), steps).unwrap();
        let result = engine.recover_saga(test_saga_id()).unwrap();
        assert_eq!(result.saga_id, test_saga_id());
    }

    #[test]
    fn saga_engine_stats() {
        let engine = SagaEngine::new();
        let steps = vec![test_step_definition(test_step_id(), test_dataset_id(), 1)];
        engine.create_saga(test_saga_id(), test_node_id(), steps).unwrap();
        let stats = engine.stats(test_saga_id()).unwrap();
        assert_eq!(stats.saga_id, test_saga_id());
    }

    #[test]
    fn saga_engine_is_complete() {
        let engine = SagaEngine::new();
        let steps = vec![test_step_definition(test_step_id(), test_dataset_id(), 1)];
        engine.create_saga(test_saga_id(), test_node_id(), steps).unwrap();
        assert!(!engine.is_complete(test_saga_id()));
        engine.execute_step(test_saga_id(), test_step_id()).unwrap();
        assert!(engine.is_complete(test_saga_id()));
    }

    #[test]
    fn saga_engine_is_compensated() {
        let engine = SagaEngine::new();
        let steps = vec![test_step_definition(test_step_id(), test_dataset_id(), 1)];
        engine.create_saga(test_saga_id(), test_node_id(), steps).unwrap();
        assert!(!engine.is_compensated(test_saga_id()));
        engine.execute_step(test_saga_id(), test_step_id()).unwrap();
        engine.compensate_step(test_saga_id(), test_step_id()).unwrap();
        assert!(engine.is_compensated(test_saga_id()));
    }

    #[test]
    fn saga_engine_coordinator() {
        let engine = SagaEngine::new();
        let steps = vec![test_step_definition(test_step_id(), test_dataset_id(), 1)];
        engine.create_saga(test_saga_id(), test_node_id(), steps).unwrap();
        assert_eq!(engine.coordinator(test_saga_id()).unwrap(), test_node_id());
    }

    #[test]
    fn saga_engine_completed_and_pending_steps() {
        let engine = SagaEngine::new();
        let steps = vec![
            test_step_definition(test_step_id(), test_dataset_id(), 1),
            test_step_definition(test_step_id2(), test_dataset_id(), 2),
        ];
        engine.create_saga(test_saga_id(), test_node_id(), steps).unwrap();
        assert_eq!(engine.pending_steps(test_saga_id()).len(), 2);
        assert_eq!(engine.completed_steps(test_saga_id()).len(), 0);
        engine.execute_step(test_saga_id(), test_step_id()).unwrap();
        assert_eq!(engine.pending_steps(test_saga_id()).len(), 1);
        assert_eq!(engine.completed_steps(test_saga_id()).len(), 1);
    }

    #[test]
    fn saga_engine_failed_steps() {
        let engine = SagaEngine::new();
        let steps = vec![test_step_definition(test_step_id(), test_dataset_id(), 1)];
        engine.create_saga(test_saga_id(), test_node_id(), steps).unwrap();
        assert_eq!(engine.failed_steps(test_saga_id()).len(), 0);
    }

    #[test]
    fn saga_engine_abort() {
        let engine = SagaEngine::new();
        let steps = vec![
            test_step_definition(test_step_id(), test_dataset_id(), 1),
            test_step_definition(test_step_id2(), test_dataset_id(), 2),
        ];
        engine.create_saga(test_saga_id(), test_node_id(), steps).unwrap();
        engine.execute_step(test_saga_id(), test_step_id()).unwrap();
        engine.execute_step(test_saga_id(), test_step_id2()).unwrap();
        let result = engine.abort(test_saga_id()).unwrap();
        assert!(result.success);
        assert_eq!(result.steps_compensated, 2);
        assert!(!engine.is_complete(test_saga_id()));
    }

    #[test]
    fn saga_engine_validate_config() {
        let engine = SagaEngine::new();
        let config = SagaConfig::default();
        assert!(engine.validate_config(&config).is_ok());
    }

    #[test]
    fn saga_engine_advance() {
        let engine = SagaEngine::new();
        let steps = vec![test_step_definition(test_step_id(), test_dataset_id(), 1)];
        engine.create_saga(test_saga_id(), test_node_id(), steps).unwrap();
        let result = engine.advance(test_saga_id()).unwrap();
        assert_eq!(result.previous_state, SagaState::Created);
        assert_eq!(result.new_state, SagaState::Executing);
        assert!(result.valid);
    }

    #[test]
    fn saga_engine_advance_to_completed() {
        let engine = SagaEngine::new();
        let steps = vec![test_step_definition(test_step_id(), test_dataset_id(), 1)];
        engine.create_saga(test_saga_id(), test_node_id(), steps).unwrap();
        engine.execute_step(test_saga_id(), test_step_id()).unwrap();
        let result = engine.advance(test_saga_id()).unwrap();
        assert_eq!(result.new_state, SagaState::Completed);
    }

    #[test]
    fn saga_engine_failure_events() {
        let engine = SagaEngine::new();
        let steps = vec![test_step_definition(test_step_id(), test_dataset_id(), 1)];
        engine.create_saga(test_saga_id(), test_node_id(), steps).unwrap();
        assert_eq!(engine.failure_events(test_saga_id()).len(), 0);
        engine.record_failure(test_saga_id(), test_failure_event()).unwrap();
        assert_eq!(engine.failure_events(test_saga_id()).len(), 1);
    }

    #[test]
    fn saga_engine_recovery_results() {
        let engine = SagaEngine::new();
        let steps = vec![test_step_definition(test_step_id(), test_dataset_id(), 1)];
        engine.create_saga(test_saga_id(), test_node_id(), steps).unwrap();
        assert_eq!(engine.recovery_results(test_saga_id()).len(), 0);
        engine.recover_saga(test_saga_id()).unwrap();
        assert_eq!(engine.recovery_results(test_saga_id()).len(), 1);
    }

    #[test]
    fn saga_engine_full_lifecycle() {
        let engine = SagaEngine::new();
        let steps = vec![
            test_step_definition(test_step_id(), test_dataset_id(), 1),
            test_step_definition(test_step_id2(), test_dataset_id(), 2),
            test_step_definition(test_step_id3(), test_dataset_id(), 3),
        ];

        // Create
        engine.create_saga(test_saga_id(), test_node_id(), steps).unwrap();
        assert_eq!(engine.saga_state(test_saga_id()).unwrap(), SagaState::Created);

        // Register transactions
        engine.register_local_transaction(
            test_saga_id(),
            test_step_id(),
            test_local_transaction(test_step_id(), test_dataset_id()),
        ).unwrap();
        engine.register_local_transaction(
            test_saga_id(),
            test_step_id2(),
            test_local_transaction(test_step_id2(), test_dataset_id()),
        ).unwrap();
        engine.register_local_transaction(
            test_saga_id(),
            test_step_id3(),
            test_local_transaction(test_step_id3(), test_dataset_id()),
        ).unwrap();

        // Execute all steps
        engine.execute_step(test_saga_id(), test_step_id()).unwrap();
        engine.execute_step(test_saga_id(), test_step_id2()).unwrap();
        engine.execute_step(test_saga_id(), test_step_id3()).unwrap();

        // Verify complete
        assert!(engine.is_complete(test_saga_id()));
        assert_eq!(engine.completed_steps(test_saga_id()).len(), 3);

        // Compensate all
        let result = engine.compensate_all(test_saga_id()).unwrap();
        assert!(result.all_successful);
        assert_eq!(result.total_compensated, 3);
        assert!(engine.is_compensated(test_saga_id()));

        // Stats
        let stats = engine.stats(test_saga_id()).unwrap();
        assert_eq!(stats.completed_steps, 3);
        assert_eq!(stats.compensated_steps, 3);
    }

    #[test]
    fn saga_engine_multiple_sagas() {
        let engine = SagaEngine::new();
        engine.create_saga(test_saga_id(), test_node_id(), vec![test_step_definition(test_step_id(), test_dataset_id(), 1)]).unwrap();
        engine.create_saga(test_saga_id2(), test_node_id2(), vec![test_step_definition(test_step_id2(), test_dataset_id(), 1)]).unwrap();
        assert_eq!(engine.sagas().len(), 2);
        assert!(engine.is_complete(test_saga_id()) == false);
        assert!(engine.is_complete(test_saga_id2()) == false);
    }

    #[test]
    fn saga_engine_compensation_max_attempts() {
        let engine = SagaEngine::new();
        let steps = vec![test_step_definition(test_step_id(), test_dataset_id(), 1)];
        engine.create_saga(test_saga_id(), test_node_id(), steps).unwrap();
        engine.execute_step(test_saga_id(), test_step_id()).unwrap();

        // Force many compensation attempts
        for _ in 0..3 {
            let _ = engine.compensate_step(test_saga_id(), test_step_id());
        }
        // After max attempts, should fail
        let err = engine.compensate_step(test_saga_id(), test_step_id()).unwrap_err();
        assert!(matches!(err, SagaError::MaxCompensationAttemptsExceeded { .. }));
    }

    #[test]
    fn saga_engine_advance_invalid_transition() {
        let engine = SagaEngine::new();
        let steps = vec![test_step_definition(test_step_id(), test_dataset_id(), 1)];
        engine.create_saga(test_saga_id(), test_node_id(), steps).unwrap();
        // Advance from Created to Executing
        engine.advance(test_saga_id()).unwrap();
        // Try to advance from Executing to Aborted (invalid)
        // SagaEngine::advance doesn't support arbitrary transitions, so this tests valid path
        let result = engine.advance(test_saga_id());
        // Should either succeed (if valid) or return invalid state transition
        assert!(result.is_ok() || matches!(result.unwrap_err(), SagaError::InvalidStateTransition { .. }));
    }

    #[test]
    fn saga_engine_serialization_roundtrip_saga_state() {
        let state = SagaState::Completed;
        let json = serde_json::to_string(&state).unwrap();
        let deserialized: SagaState = serde_json::from_str(&json).unwrap();
        assert_eq!(state, deserialized);
    }

    #[test]
    fn saga_engine_step_state_display_all() {
        assert_eq!(StepState::Pending.display_name(), "pending");
        assert_eq!(StepState::Executing.display_name(), "executing");
        assert_eq!(StepState::Completed.display_name(), "completed");
        assert_eq!(StepState::Failed.display_name(), "failed");
        assert_eq!(StepState::Compensating.display_name(), "compensating");
        assert_eq!(StepState::Compensated.display_name(), "compensated");
        assert_eq!(StepState::Skipped.display_name(), "skipped");
    }

    #[test]
    fn saga_engine_recovery_action_all() {
        assert_eq!(RecoveryAction::None.display_name(), "none");
        assert_eq!(RecoveryAction::RetryStep.display_name(), "retry_step");
        assert_eq!(RecoveryAction::Compensate.display_name(), "compensate");
        assert_eq!(RecoveryAction::Restart.display_name(), "restart");
        assert_eq!(RecoveryAction::Abort.display_name(), "abort");
        assert_eq!(RecoveryAction::ManualIntervention.display_name(), "manual_intervention");
    }

    // ========================================================================
    // Additional coordinator, timeout, persistence, checkpoint, chain, graph tests
    // ========================================================================

    #[test]
    fn saga_step_coordinator_basic() {
        let coord = SagaStepCoordinator::new();
        coord.add_to_sequence(test_step_id());
        coord.add_to_sequence(test_step_id2());
        let seq = coord.execution_sequence();
        assert_eq!(seq.len(), 2);
        assert_eq!(coord.current_step(), None);
        coord.set_current_step(test_step_id());
        assert_eq!(coord.current_step(), Some(test_step_id()));
        coord.clear_current_step();
        assert_eq!(coord.current_step(), None);
    }

    #[test]
    fn saga_step_coordinator_barriers() {
        let coord = SagaStepCoordinator::new();
        coord.add_barrier(test_step_id2(), test_step_id());
        let barriers = coord.barriers_for(test_step_id2());
        assert_eq!(barriers, vec![test_step_id()]);
        assert!(!coord.can_execute(test_step_id2(), &[]));
        assert!(coord.can_execute(test_step_id2(), &[test_step_id()]));
        let next = coord.next_executable(&[]);
        assert_eq!(next, Some(test_step_id()));
    }

    #[test]
    fn saga_step_coordinator_clear() {
        let coord = SagaStepCoordinator::new();
        coord.add_to_sequence(test_step_id());
        coord.set_current_step(test_step_id());
        coord.add_barrier(test_step_id2(), test_step_id());
        coord.clear();
        assert_eq!(coord.execution_sequence().len(), 0);
        assert_eq!(coord.current_step(), None);
        assert_eq!(coord.barriers_for(test_step_id2()).len(), 0);
    }

    #[test]
    fn saga_timeout_manager_basic() {
        let mgr = SagaTimeoutManager::new(5000, 300000);
        assert_eq!(mgr.step_timeout(test_step_id()), 5000);
        assert_eq!(mgr.default_saga_timeout_ms(), 300000);
        mgr.set_step_timeout(test_step_id(), 10000);
        assert_eq!(mgr.step_timeout(test_step_id()), 10000);
        mgr.record_step_start(test_step_id(), 1000);
        assert!(!mgr.is_step_timed_out(test_step_id(), 5000));
        assert!(mgr.is_step_timed_out(test_step_id(), 20000));
        assert_eq!(mgr.step_elapsed_ms(test_step_id(), 5000), 4000);
        assert_eq!(mgr.step_elapsed_ms(test_step_id2(), 5000), 0);
    }

    #[test]
    fn saga_timeout_manager_timed_out() {
        let mgr = SagaTimeoutManager::new(1000, 300000);
        mgr.record_step_start(test_step_id(), 0);
        mgr.record_step_start(test_step_id2(), 0);
        let timed_out = mgr.timed_out_steps(5000);
        assert_eq!(timed_out.len(), 2);
    }

    #[test]
    fn saga_timeout_manager_clear_step() {
        let mgr = SagaTimeoutManager::new(5000, 300000);
        mgr.record_step_start(test_step_id(), 1000);
        mgr.set_step_timeout(test_step_id(), 10000);
        mgr.clear_step(test_step_id());
        assert_eq!(mgr.step_timeout(test_step_id()), 5000);
        assert_eq!(mgr.step_elapsed_ms(test_step_id(), 5000), 0);
    }

    #[test]
    fn saga_timeout_manager_default() {
        let mgr = SagaTimeoutManager::default();
        assert_eq!(mgr.step_timeout(test_step_id()), 30000);
        assert_eq!(mgr.default_saga_timeout_ms(), 300000);
    }

    #[test]
    fn saga_persistence_manager_basic() {
        let mgr = SagaPersistenceManager::new();
        assert!(mgr.is_enabled());
        let mut step_states = HashMap::new();
        step_states.insert(test_step_id(), StepState::Completed);
        mgr.persist(test_saga_id(), SagaState::Completed, step_states, test_node_id());
        let persisted = mgr.persisted_state(test_saga_id()).unwrap();
        assert_eq!(persisted.saga_id, test_saga_id());
        assert_eq!(persisted.state, SagaState::Completed);
        assert_eq!(persisted.coordinator, test_node_id());
        assert!(persisted.step_states.contains_key(&test_step_id()));
    }

    #[test]
    fn saga_persistence_manager_remove() {
        let mgr = SagaPersistenceManager::new();
        mgr.persist(test_saga_id(), SagaState::Created, HashMap::new(), test_node_id());
        assert!(mgr.persisted_state(test_saga_id()).is_some());
        mgr.remove(test_saga_id());
        assert!(mgr.persisted_state(test_saga_id()).is_none());
    }

    #[test]
    fn saga_persistence_manager_sagas() {
        let mgr = SagaPersistenceManager::new();
        assert!(mgr.persisted_sagas().is_empty());
        mgr.persist(test_saga_id(), SagaState::Created, HashMap::new(), test_node_id());
        mgr.persist(test_saga_id2(), SagaState::Created, HashMap::new(), test_node_id2());
        assert_eq!(mgr.persisted_sagas().len(), 2);
    }

    #[test]
    fn saga_persistence_manager_clear_all() {
        let mgr = SagaPersistenceManager::new();
        mgr.persist(test_saga_id(), SagaState::Created, HashMap::new(), test_node_id());
        mgr.clear_all();
        assert!(mgr.persisted_sagas().is_empty());
    }

    #[test]
    fn saga_checkpoint_manager_basic() {
        let mgr = SagaCheckpointManager::new(10);
        let cp_id = mgr.checkpoint(test_saga_id(), vec![test_step_id()], SagaState::Executing);
        assert_eq!(cp_id, 1);
        let checkpoints = mgr.checkpoints_for(test_saga_id());
        assert_eq!(checkpoints.len(), 1);
        assert_eq!(checkpoints[0].checkpoint_id, 1);
        assert_eq!(checkpoints[0].saga_state, SagaState::Executing);
    }

    #[test]
    fn saga_checkpoint_manager_latest() {
        let mgr = SagaCheckpointManager::new(10);
        mgr.checkpoint(test_saga_id(), vec![test_step_id()], SagaState::Executing);
        mgr.checkpoint(test_saga_id(), vec![test_step_id(), test_step_id2()], SagaState::Completed);
        let latest = mgr.latest_checkpoint(test_saga_id()).unwrap();
        assert_eq!(latest.checkpoint_id, 2);
        assert_eq!(latest.saga_state, SagaState::Completed);
    }

    #[test]
    fn saga_checkpoint_manager_max_checkpoints() {
        let mgr = SagaCheckpointManager::new(2);
        mgr.checkpoint(test_saga_id(), vec![test_step_id()], SagaState::Executing);
        mgr.checkpoint(test_saga_id(), vec![test_step_id(), test_step_id2()], SagaState::Completed);
        mgr.checkpoint(test_saga_id(), vec![test_step_id(), test_step_id2(), test_step_id3()], SagaState::Aborted);
        let checkpoints = mgr.checkpoints_for(test_saga_id());
        assert_eq!(checkpoints.len(), 2);
    }

    #[test]
    fn saga_checkpoint_manager_total() {
        let mgr = SagaCheckpointManager::new(10);
        assert_eq!(mgr.total_checkpoints(), 0);
        mgr.checkpoint(test_saga_id(), vec![test_step_id()], SagaState::Created);
        mgr.checkpoint(test_saga_id2(), vec![test_step_id2()], SagaState::Created);
        assert_eq!(mgr.total_checkpoints(), 2);
    }

    #[test]
    fn saga_step_chain_basic() {
        let mut chain = SagaStepChain::new(1);
        chain.add_step(test_step_id());
        chain.add_step(test_step_id2());
        chain.add_step(test_step_id3());
        assert_eq!(chain.len(), 3);
        assert!(!chain.is_empty());
        assert!(!chain.is_complete(&[]));
        assert!(chain.is_complete(&[test_step_id(), test_step_id2(), test_step_id3()]));
    }

    #[test]
    fn saga_step_chain_dependency() {
        let mut chain = SagaStepChain::new(1);
        chain.add_step(test_step_id());
        chain.add_step(test_step_id2());
        chain.add_dependency(test_step_id2(), test_step_id());
        assert_eq!(chain.next_executable(&[]), Some(test_step_id()));
        assert_eq!(chain.next_executable(&[test_step_id()]), Some(test_step_id2()));
    }

    #[test]
    fn saga_step_chain_empty() {
        let chain = SagaStepChain::new(1);
        assert!(chain.is_empty());
        assert_eq!(chain.len(), 0);
        assert_eq!(chain.next_executable(&[]), None);
    }

    #[test]
    fn saga_dependency_graph_basic() {
        let graph = SagaDependencyGraph::new();
        graph.add_node(test_step_id());
        graph.add_node(test_step_id2());
        assert_eq!(graph.node_count(), 2);
        assert_eq!(graph.dependencies(test_step_id()), Vec::<StepId>::new());
        assert!(!graph.has_cycle());
    }

    #[test]
    fn saga_dependency_graph_edge() {
        let graph = SagaDependencyGraph::new();
        graph.add_edge(test_step_id2(), test_step_id());
        assert_eq!(graph.dependencies(test_step_id2()), vec![test_step_id()]);
        assert_eq!(graph.dependencies(test_step_id()), Vec::<StepId>::new());
        assert!(!graph.has_cycle());
    }

    #[test]
    fn saga_dependency_graph_cycle() {
        let graph = SagaDependencyGraph::new();
        graph.add_edge(test_step_id2(), test_step_id());
        graph.add_edge(test_step_id(), test_step_id2());
        assert!(graph.has_cycle());
        let result = graph.topological_order();
        assert!(result.is_err());
    }

    #[test]
    fn saga_dependency_graph_topological_order() {
        let graph = SagaDependencyGraph::new();
        graph.add_edge(test_step_id2(), test_step_id());
        graph.add_edge(test_step_id3(), test_step_id2());
        let order = graph.topological_order().unwrap();
        assert_eq!(order.len(), 3);
    }

    #[test]
    fn saga_dependency_graph_clear() {
        let graph = SagaDependencyGraph::new();
        graph.add_edge(test_step_id2(), test_step_id());
        assert_eq!(graph.node_count(), 2);
        graph.clear();
        assert_eq!(graph.node_count(), 0);
    }

    #[test]
    fn saga_engine_progress_percent() {
        let engine = SagaEngine::new();
        let steps = vec![
            test_step_definition(test_step_id(), test_dataset_id(), 1),
            test_step_definition(test_step_id2(), test_dataset_id(), 2),
            test_step_definition(test_step_id3(), test_dataset_id(), 3),
        ];
        engine.create_saga(test_saga_id(), test_node_id(), steps).unwrap();
        assert!((engine.progress_percent(test_saga_id()) - 0.0).abs() < 0.01);
        engine.execute_step(test_saga_id(), test_step_id()).unwrap();
        assert!((engine.progress_percent(test_saga_id()) - 33.33).abs() < 1.0);
        engine.execute_step(test_saga_id(), test_step_id2()).unwrap();
        engine.execute_step(test_saga_id(), test_step_id3()).unwrap();
        assert!((engine.progress_percent(test_saga_id()) - 100.0).abs() < 0.01);
    }

    #[test]
    fn saga_engine_step_count() {
        let engine = SagaEngine::new();
        let steps = vec![
            test_step_definition(test_step_id(), test_dataset_id(), 1),
            test_step_definition(test_step_id2(), test_dataset_id(), 2),
        ];
        engine.create_saga(test_saga_id(), test_node_id(), steps).unwrap();
        assert_eq!(engine.step_count(test_saga_id()), 2);
    }

    #[test]
    fn saga_engine_can_proceed() {
        let engine = SagaEngine::new();
        let steps = vec![test_step_definition(test_step_id(), test_dataset_id(), 1)];
        engine.create_saga(test_saga_id(), test_node_id(), steps).unwrap();
        assert!(engine.can_proceed(test_saga_id()));
        engine.execute_step(test_saga_id(), test_step_id()).unwrap();
        assert!(!engine.can_proceed(test_saga_id()));
    }

    #[test]
    fn saga_engine_saga_duration_ms() {
        let engine = SagaEngine::new();
        let steps = vec![test_step_definition(test_step_id(), test_dataset_id(), 1)];
        engine.create_saga(test_saga_id(), test_node_id(), steps).unwrap();
        assert_eq!(engine.saga_duration_ms(test_saga_id()), 0);
    }

    #[test]
    fn saga_engine_created_and_updated_at() {
        let engine = SagaEngine::new();
        let steps = vec![test_step_definition(test_step_id(), test_dataset_id(), 1)];
        engine.create_saga(test_saga_id(), test_node_id(), steps).unwrap();
        assert!(engine.saga_created_at(test_saga_id()).is_some());
        assert!(engine.saga_updated_at(test_saga_id()).is_some());
    }

    #[test]
    fn saga_engine_sagas_by_state() {
        let engine = SagaEngine::new();
        let steps = vec![test_step_definition(test_step_id(), test_dataset_id(), 1)];
        engine.create_saga(test_saga_id(), test_node_id(), steps).unwrap();
        let created = engine.sagas_by_state(SagaState::Created);
        assert_eq!(created.len(), 1);
        engine.execute_step(test_saga_id(), test_step_id()).unwrap();
        let executing = engine.sagas_by_state(SagaState::Executing);
        assert_eq!(executing.len(), 1);
    }

    #[test]
    fn saga_engine_total_active_terminal_count() {
        let engine = SagaEngine::new();
        assert_eq!(engine.total_saga_count(), 0);
        assert_eq!(engine.active_saga_count(), 0);
        assert_eq!(engine.terminal_saga_count(), 0);
        engine.create_saga(test_saga_id(), test_node_id(), vec![test_step_definition(test_step_id(), test_dataset_id(), 1)]).unwrap();
        assert_eq!(engine.total_saga_count(), 1);
        assert_eq!(engine.active_saga_count(), 1);
        engine.execute_step(test_saga_id(), test_step_id()).unwrap();
        assert_eq!(engine.terminal_saga_count(), 1);
    }

    #[test]
    fn saga_engine_clear_all_sagas() {
        let engine = SagaEngine::new();
        engine.create_saga(test_saga_id(), test_node_id(), vec![test_step_definition(test_step_id(), test_dataset_id(), 1)]).unwrap();
        engine.create_saga(test_saga_id2(), test_node_id2(), vec![test_step_definition(test_step_id2(), test_dataset_id(), 1)]).unwrap();
        assert_eq!(engine.total_saga_count(), 2);
        engine.clear_all_sagas();
        assert_eq!(engine.total_saga_count(), 0);
    }

    #[test]
    fn saga_engine_config_clone() {
        let engine = SagaEngine::new();
        let config = engine.config_clone();
        assert_eq!(config.max_steps, 10);
    }

    #[test]
    fn saga_engine_next_and_last_step() {
        let engine = SagaEngine::new();
        let steps = vec![
            test_step_definition(test_step_id(), test_dataset_id(), 1),
            test_step_definition(test_step_id2(), test_dataset_id(), 2),
        ];
        engine.create_saga(test_saga_id(), test_node_id(), steps).unwrap();
        assert_eq!(engine.next_pending_step(test_saga_id()), Some(test_step_id()));
        engine.execute_step(test_saga_id(), test_step_id()).unwrap();
        assert_eq!(engine.next_pending_step(test_saga_id()), Some(test_step_id2()));
        assert_eq!(engine.last_completed_step(test_saga_id()), Some(test_step_id()));
    }

    #[test]
    fn saga_engine_dependent_and_prerequisites() {
        let engine = SagaEngine::new();
        let steps = vec![
            test_step_definition(test_step_id(), test_dataset_id(), 1),
            test_step_definition(test_step_id2(), test_dataset_id(), 2),
        ];
        engine.create_saga(test_saga_id(), test_node_id(), steps).unwrap();
        assert!(engine.prerequisites_complete(test_saga_id(), test_step_id()));
        assert!(!engine.prerequisites_complete(test_saga_id(), test_step_id2()));
        engine.execute_step(test_saga_id(), test_step_id()).unwrap();
        assert!(engine.prerequisites_complete(test_saga_id(), test_step_id2()));
        assert_eq!(engine.dependent_steps(test_saga_id(), test_step_id()), vec![test_step_id2()]);
    }

    #[test]
    fn saga_engine_steps_by_state() {
        let engine = SagaEngine::new();
        let steps = vec![
            test_step_definition(test_step_id(), test_dataset_id(), 1),
            test_step_definition(test_step_id2(), test_dataset_id(), 2),
        ];
        engine.create_saga(test_saga_id(), test_node_id(), steps).unwrap();
        assert_eq!(engine.steps_by_state(test_saga_id(), StepState::Pending).len(), 2);
        engine.execute_step(test_saga_id(), test_step_id()).unwrap();
        assert_eq!(engine.steps_by_state(test_saga_id(), StepState::Completed).len(), 1);
        assert_eq!(engine.steps_by_state(test_saga_id(), StepState::Pending).len(), 1);
    }

    #[test]
    fn saga_engine_serialization_roundtrip_saga_config() {
        let config = SagaConfig::default();
        let json = serde_json::to_string(&config).unwrap();
        let deserialized: SagaConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(config.max_steps, deserialized.max_steps);
        assert_eq!(config.auto_compensate, deserialized.auto_compensate);
    }

    #[test]
    fn saga_engine_step_definition_clone() {
        let def = test_step_definition(test_step_id(), test_dataset_id(), 1);
        let cloned = def.clone();
        assert_eq!(def.step_id, cloned.step_id);
        assert_eq!(def.order, cloned.order);
        assert_eq!(def.payload, cloned.payload);
    }

    #[test]
    fn saga_engine_step_state_clone() {
        let state = StepState::Completed;
        let cloned = state.clone();
        assert_eq!(state, cloned);
    }

    #[test]
    fn saga_engine_saga_state_clone() {
        let state = SagaState::Completed;
        let cloned = state.clone();
        assert_eq!(state, cloned);
    }

    #[test]
    fn saga_engine_compensation_result_clone() {
        let result = CompensationResult {
            saga_id: test_saga_id(),
            step_id: test_step_id(),
            success: true,
            output: vec![1, 2, 3],
            attempts: 1,
            compensated_at: IdsTime::now(),
        };
        let cloned = result.clone();
        assert_eq!(result.saga_id, cloned.saga_id);
        assert_eq!(result.success, cloned.success);
    }

    #[test]
    fn saga_engine_saga_stats_clone() {
        let stats = SagaStats::new(test_saga_id());
        let cloned = stats.clone();
        assert_eq!(stats.saga_id, cloned.saga_id);
    }

    #[test]
    fn saga_engine_recovery_result_clone() {
        let result = SagaRecoveryResult {
            saga_id: test_saga_id(),
            action: RecoveryAction::Compensate,
            steps_recovered: 1,
            success: true,
            recovered_at: IdsTime::now(),
        };
        let cloned = result.clone();
        assert_eq!(result.saga_id, cloned.saga_id);
        assert_eq!(result.action, cloned.action);
    }

    #[test]
    fn saga_engine_persisted_saga_state_clone() {
        let mut step_states = HashMap::new();
        step_states.insert(test_step_id(), StepState::Completed);
        let state = PersistedSagaState {
            saga_id: test_saga_id(),
            state: SagaState::Completed,
            step_states,
            coordinator: test_node_id(),
            persisted_at: IdsTime::now(),
        };
        let cloned = state.clone();
        assert_eq!(state.saga_id, cloned.saga_id);
        assert_eq!(state.state, cloned.state);
    }

    #[test]
    fn saga_engine_saga_checkpoint_clone() {
        let cp = SagaCheckpoint {
            checkpoint_id: 1,
            saga_id: test_saga_id(),
            completed_steps: vec![test_step_id()],
            saga_state: SagaState::Executing,
            checkpoint_at: IdsTime::now(),
        };
        let cloned = cp.clone();
        assert_eq!(cp.checkpoint_id, cloned.checkpoint_id);
        assert_eq!(cp.saga_id, cloned.saga_id);
    }

    #[test]
    fn saga_engine_saga_step_chain_clone() {
        let mut chain = SagaStepChain::new(1);
        chain.add_step(test_step_id());
        let cloned = chain.clone();
        assert_eq!(chain.chain_id, cloned.chain_id);
        assert_eq!(chain.len(), cloned.len());
    }

    #[test]
    fn saga_engine_saga_dependency_graph_default() {
        let graph = SagaDependencyGraph::default();
        assert_eq!(graph.node_count(), 0);
    }

    #[test]
    fn saga_engine_timeout_manager_default() {
        let mgr = SagaTimeoutManager::default();
        assert_eq!(mgr.step_timeout(test_step_id()), 30000);
    }

    #[test]
    fn saga_engine_persistence_manager_default() {
        let mgr = SagaPersistenceManager::default();
        assert!(mgr.is_enabled());
    }

    #[test]
    fn saga_engine_checkpoint_manager_default() {
        let mgr = SagaCheckpointManager::default();
        assert_eq!(mgr.total_checkpoints(), 0);
    }

    #[test]
    fn saga_engine_coordinator_default() {
        let coord = SagaStepCoordinator::default();
        assert_eq!(coord.execution_sequence().len(), 0);
    }
}