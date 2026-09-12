//! Core value types for Mossignal.

pub mod authored;
pub mod binding;
pub mod builder;
mod compile;
pub mod diagnostics;
mod episode;
pub mod identity;
mod input;
pub mod key;
mod machine;
pub mod metadata;
mod module;
mod node_schema;
mod policy;
pub mod signal;
pub mod standard;
pub mod time;
mod transaction;

mod validation;

pub use authored::{
    ConflictPolicy, EdgeConfig, EdgeDetectorKind, EdgeInitialization, EdgeObservation,
    LevelSetResetConfig, ModuleBinding, ModuleBindingSet, ModuleInstanceDef, PulseDelayConfig,
    PulseSetResetConfig, ToggleConfig,
};
pub use binding::{
    BindingFailure, BindingSet, BindingSetBuilder, BoundApplyFailure, BoundMachine,
    BoundOutputFailure, BoundTransactionResult, InputObservation, InputProjectionFailure,
    InputProjector, ProjectedOutputEvent,
};
pub use builder::{
    AddedModuleInstance, AddedNode, AddedStandardModule, AuthoringFailure, KeyedModuleInput,
    ModuleBuilder, ModuleInstanceBuilder, NetworkBuilder, PulseRouteOutputs, Signal,
};
pub use compile::CompiledNetwork;
pub use diagnostics::{ConflictControls, ConflictEvidence, DiagnosticOccurrence};
pub use episode::{
    ActiveDiagnosticEpisode, DiagnosticConditionKey, DiagnosticEpisodeChange,
    DiagnosticEpisodeChangeKind, DiagnosticEpisodeId,
};
pub use identity::{InputSchemaFingerprint, ModuleFingerprint, NetworkFingerprint, TimeDomainId};
pub use input::{
    InputBuildFailure, InputDelta, InputDeltaBuilder, InputSnapshot, InputSnapshotBuilder,
};
pub use machine::{
    DiagnosticEpisodeInspectionFailure, EdgeDetectorDefinitionInspection, EdgeDetectorInspection,
    EdgeDetectorInspectionFailure, LevelSetResetLatchDefinitionInspection,
    LevelSetResetLatchInspection, LevelSetResetLatchInspectionFailure, Machine, MachineStatus,
    ModuleInputInspection, ModuleInspection, ModuleInspectionFailure, ModuleNodeInspection,
    ModuleOutputInspection, ModulePendingPulseDelayInspection, NetworkRevision, PendingEventKey,
    PendingPulseDelayInspection, PulseDelayDefinitionInspection, PulseDelayInspection,
    PulseDelayInspectionFailure, PulseSetResetLatchDefinitionInspection,
    PulseSetResetLatchInspection, PulseSetResetLatchInspectionFailure, Schedule, ScheduleFailure,
    ToggleDefinitionInspection, ToggleInspection, ToggleInspectionFailure,
};
pub use module::{
    DefinitionGraphView, ModuleDef, ModuleInputIter, ModuleOrigin, ModuleOutputIter, NodeSubject,
    PulsePortSubject, QualifiedConnectionRef, QualifiedInPortRef, QualifiedModuleRef,
    QualifiedNodeRef,
};
pub use policy::{
    PolicyFailure, RuntimePolicy, RuntimePolicyBuilder, RuntimePolicyId, RuntimePolicyLimit,
};
pub use standard::{
    AllEqualDependency, AllEqualExplanation, AllEqualInspection, AtMostDependency,
    AtMostExplanation, AtMostInspection, CatalogueFailure, ExactlyDependency, ExactlyExplanation,
    ExactlyInspection, StandardCatalogue, StandardCatalogueVersion, StandardEnumValue,
    StandardInternalCategory, StandardInternalRole, StandardModuleAvailability,
    StandardModuleCategory, StandardModuleDeclaration, StandardModuleDescriptor,
    StandardModuleExpansionFingerprint, StandardModuleExpansionVersion, StandardModuleId,
    StandardModuleIdError, StandardModuleRef, StandardModuleRequest, StandardModuleSemanticVersion,
    StandardParameterAssignment, StandardParameterKey, StandardParameterKind,
    StandardParameterSchema, StandardParameterValue, StandardPortSchema, StandardPublicDependency,
    all_equal_result_key, at_most_result_key, exactly_result_key,
};
pub use transaction::{
    CauseInspection, CauseLookupFailure, CauseRef, OutputEvent, ProvenanceSubject, ProvenanceView,
    PulseContribution, RuntimeFailure, RuntimeFailureEvidence, Transaction, TransactionResult,
};
pub use validation::{NetworkDefinitionGraphView, ValidatedNetwork};
