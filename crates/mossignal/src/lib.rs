//! Core value types for Mossignal.

pub mod authored;
pub mod builder;
mod compile;
pub mod diagnostics;
pub mod identity;
mod input;
pub mod key;
mod machine;
pub mod metadata;
mod module;
mod policy;
pub mod signal;
pub mod time;
mod transaction;

mod validation;

pub use authored::{
    ModuleBinding, ModuleBindingSet, ModuleInstanceDef, PulseDelayConfig, ToggleConfig,
};
pub use builder::{
    AddedModuleInstance, AddedNode, AuthoringFailure, ModuleBuilder, ModuleInstanceBuilder,
    NetworkBuilder, Signal,
};
pub use compile::CompiledNetwork;
pub use identity::{InputSchemaFingerprint, ModuleFingerprint, NetworkFingerprint, TimeDomainId};
pub use input::{
    InputBuildFailure, InputDelta, InputDeltaBuilder, InputSnapshot, InputSnapshotBuilder,
};
pub use machine::{
    Machine, MachineStatus, NetworkRevision, PendingEventKey, PendingPulseDelayInspection,
    PulseDelayDefinitionInspection, PulseDelayInspection, PulseDelayInspectionFailure, Schedule,
    ScheduleFailure, ToggleDefinitionInspection, ToggleInspection, ToggleInspectionFailure,
};
pub use module::{
    DefinitionGraphView, ModuleDef, ModuleInputIter, ModuleOrigin, ModuleOutputIter,
    QualifiedConnectionRef, QualifiedNodeRef,
};
pub use policy::{
    PolicyFailure, RuntimePolicy, RuntimePolicyBuilder, RuntimePolicyId, RuntimePolicyLimit,
};
pub use transaction::{
    CauseInspection, CauseLookupFailure, CauseRef, OutputEvent, ProvenanceSubject, ProvenanceView,
    PulseContribution, RuntimeFailure, RuntimeFailureEvidence, Transaction, TransactionResult,
};
pub use validation::{NetworkDefinitionGraphView, ValidatedNetwork};
