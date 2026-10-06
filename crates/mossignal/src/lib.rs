//! Core value types for Mossignal.

pub mod authored;
pub mod binding;
pub mod builder;
mod cbor_decode;
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
mod persistence;
mod policy;
mod replay;
pub mod signal;
mod snapshot_restore;
pub mod standard;
mod state_digest;
pub mod time;
mod transaction;

mod validation;

pub use authored::{
    ConflictPolicy, EdgeConfig, EdgeDetectorKind, EdgeInitialization, EdgeObservation,
    FirstEmissionPolicy, InertialDelayConfig, LevelSetResetConfig, ModuleBinding, ModuleBindingSet,
    ModuleInstanceDef, PeriodicConfig, PulseDelayConfig, PulseSetResetConfig, ReenablePhasePolicy,
    SampleHoldConfig, ToggleConfig, TransportDelayConfig,
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
pub use identity::{
    ExecutionStateDigest, InputSchemaFingerprint, ModuleFingerprint, NetworkFingerprint,
    ObservableStateDigest, SnapshotDigest, TimeDomainId,
};
pub use input::{
    InputBuildFailure, InputDelta, InputDeltaBuilder, InputSnapshot, InputSnapshotBuilder,
};
pub use machine::{
    DiagnosticEpisodeInspectionFailure, EdgeDetectorDefinitionInspection, EdgeDetectorInspection,
    EdgeDetectorInspectionFailure, InertialDelayDefinitionInspection, InertialDelayInspection,
    InertialDelayInspectionFailure, LevelSetResetLatchDefinitionInspection,
    LevelSetResetLatchInspection, LevelSetResetLatchInspectionFailure, Machine, MachineStatus,
    ModuleInputInspection, ModuleInspection, ModuleInspectionFailure, ModuleNodeInspection,
    ModuleOutputInspection, ModulePendingPulseDelayInspection, NetworkRevision, PendingEventKey,
    PendingInertialDelayInspection, PendingPeriodicBoundaryInspection, PendingPulseDelayInspection,
    PendingTransportDelayInspection, PeriodicDefinitionInspection, PeriodicInspection,
    PeriodicInspectionFailure, PulseDelayDefinitionInspection, PulseDelayInspection,
    PulseDelayInspectionFailure, PulseSetResetLatchDefinitionInspection,
    PulseSetResetLatchInspection, PulseSetResetLatchInspectionFailure,
    SampleHoldDefinitionInspection, SampleHoldInspection, SampleHoldInspectionFailure, Schedule,
    ScheduleFailure, ToggleDefinitionInspection, ToggleInspection, ToggleInspectionFailure,
    TransportDelayDefinitionInspection, TransportDelayInspection, TransportDelayInspectionFailure,
};
pub use module::{
    DefinitionGraphView, ModuleDef, ModuleInputIter, ModuleOrigin, ModuleOutputIter, NodeSubject,
    PulsePortSubject, QualifiedConnectionRef, QualifiedInPortRef, QualifiedModuleRef,
    QualifiedNodeRef,
};
pub use persistence::{
    ArtifactBytes, EncodeFailure, MachineSnapshot, PersistenceContext, encode_snapshot,
};
pub use policy::{
    PolicyFailure, RuntimePolicy, RuntimePolicyBuilder, RuntimePolicyId, RuntimePolicyLimit,
};
pub use replay::{
    RecordedTransaction, ReplayFailure, ReplayFrame, ReplayLog, ReplayLogContentDigest,
    decode_replay_frame, decode_replay_log, encode_replay_frame, encode_replay_log,
    record_replay_log,
};
pub use snapshot_restore::{DecodeFailure, DecodePolicy, RestoreFailure, decode_snapshot};
pub use standard::{
    AllEqualDependency, AllEqualExplanation, AllEqualInspection, AtMostDependency,
    AtMostExplanation, AtMostInspection, CaptureKind, CatalogueFailure, ExactlyDependency,
    ExactlyExplanation, ExactlyInspection, ResetObservation, StandardCatalogue,
    StandardCatalogueVersion, StandardEnumValue, StandardInternalCategory, StandardInternalRole,
    StandardModuleAvailability, StandardModuleCategory, StandardModuleDeclaration,
    StandardModuleDescriptor, StandardModuleExpansionFingerprint, StandardModuleExpansionVersion,
    StandardModuleId, StandardModuleIdError, StandardModuleRef, StandardModuleRequest,
    StandardModuleSemanticVersion, StandardParameterAssignment, StandardParameterKey,
    StandardParameterKind, StandardParameterSchema, StandardParameterValue, StandardPortSchema,
    StandardPublicDependency, StatefulStandardInspection, StatefulStandardReaction, StatefulWhyNot,
    all_equal_result_key, at_most_result_key, exactly_result_key,
};
pub use transaction::{
    CauseInspection, CauseLookupFailure, CauseRef, OutputEvent, ProvenanceSubject, ProvenanceView,
    PulseContribution, RuntimeFailure, RuntimeFailureEvidence, Transaction, TransactionResult,
};
pub use validation::{NetworkDefinitionGraphView, ValidatedNetwork};
