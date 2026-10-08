//! Mossignal authoring, execution, and owned semantic inspection.
//!
//! Graph slices describe structural possibility, including delayed paths.
//! Current explanations follow recorded support; their transition cause remains
//! separate when the output stays unchanged. Owned observations retain their
//! provenance after subsequent transactions.
//!
//! ```
//! use mossignal::{Explain, NetworkBuilder, RuntimePolicy, TimeDomainId, Transaction};
//! use mossignal::key::{ExternalInputKey, ExternalOutputKey};
//! use mossignal::metadata::DiagnosticMeta;
//! use mossignal::signal::{Level, LogicLevel};
//! use mossignal::time::Time;
//!
//! let mut builder = NetworkBuilder::<()>::new(TimeDomainId::from_u128(1));
//! let input = ExternalInputKey::<Level>::from_u128(2);
//! let output = ExternalOutputKey::<Level>::from_u128(3);
//! let signal = builder.add_level_input(input, DiagnosticMeta::default()).unwrap();
//! let inverted = builder.not(signal).unwrap();
//! builder.add_level_output(output, inverted, DiagnosticMeta::default()).unwrap();
//! let network = builder.finish().require_artifact().unwrap()
//!     .compile().require_artifact().unwrap();
//! let paths = network.slice_affecting(output.into()).unwrap();
//! assert!(!paths.subjects().is_empty());
//! let policy = RuntimePolicy::builder()
//!     .max_internal_reactions(100).max_evaluated_operations(1000)
//!     .max_pending_events(100).max_events_created_per_transaction(100)
//!     .max_required_provenance_growth(1000).build().unwrap();
//! let mut machine = network.spawn(policy);
//! let snapshot = network.input_snapshot().set(input, LogicLevel::Low).unwrap()
//!     .finish().unwrap();
//! let result = machine.apply(Transaction::initialize(
//!     Time::from_ticks(0), machine.revision(), snapshot)).unwrap();
//! let observation = machine.inspect_output(output).unwrap();
//! assert_eq!(observation.level, Some(LogicLevel::High));
//! let explanation = machine.explain(Explain::CurrentOutput(output.into())).unwrap();
//! assert!(!explanation.current_support.is_empty());
//! let event = result.explain_output_event(0).unwrap();
//! assert!(!event.causal.edges.is_empty());
//! ```

pub mod authored;
pub mod binding;
pub mod builder;
mod cbor_decode;
mod compile;
mod diagnostic_inspection;
pub mod diagnostics;
mod episode;
mod graph;
pub mod identity;
mod input;
mod inspection;
pub mod key;
mod machine;
pub mod metadata;
mod migration;
mod module;
mod node_schema;
mod patch;
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
pub use diagnostic_inspection::DiagnosticScope;
pub use diagnostics::{ConflictControls, ConflictEvidence, DiagnosticOccurrence};
pub use episode::{
    ActiveDiagnosticEpisode, DiagnosticConditionKey, DiagnosticEpisodeChange,
    DiagnosticEpisodeChangeKind, DiagnosticEpisodeId,
};
pub use graph::{GraphElement, GraphQueryFailure, GraphSubjectRef, NetworkSlice, Region, RegionId};
pub use identity::{
    ExecutionStateDigest, InputSchemaFingerprint, ModuleFingerprint, NetworkFingerprint,
    ObservableStateDigest, SnapshotDigest, TimeDomainId,
};
pub use input::{
    InputBuildFailure, InputDelta, InputDeltaBuilder, InputSnapshot, InputSnapshotBuilder,
};
pub use inspection::{
    CausalExplanation, Explain, ExplainedObservation, Explanation, ExplanationEdge,
    InputPortInspection, InspectionFailure, ModuleBehavior, NodeInspection, NodeStateInspection,
    OutputEventValue, OutputInspection, OutputPortInspection, PendingEventInspection,
    PendingPayload, RetentionStatus,
};
pub use machine::{
    DiagnosticEpisodeInspectionFailure, EdgeDetectorDefinitionInspection, EdgeDetectorInspection,
    EdgeDetectorInspectionFailure, ForecastState, InertialDelayDefinitionInspection,
    InertialDelayInspection, InertialDelayInspectionFailure,
    LevelSetResetLatchDefinitionInspection, LevelSetResetLatchInspection,
    LevelSetResetLatchInspectionFailure, Machine, MachineStatus, ModuleInputInspection,
    ModuleInspection, ModuleInspectionFailure, ModuleNodeInspection, ModuleOutputInspection,
    ModulePendingPulseDelayInspection, NetworkRevision, PendingEventKey,
    PendingInertialDelayInspection, PendingPeriodicBoundaryInspection, PendingPulseDelayInspection,
    PendingTransportDelayInspection, PeriodicDefinitionInspection, PeriodicInspection,
    PeriodicInspectionFailure, PulseDelayDefinitionInspection, PulseDelayInspection,
    PulseDelayInspectionFailure, PulseSetResetLatchDefinitionInspection,
    PulseSetResetLatchInspection, PulseSetResetLatchInspectionFailure,
    SampleHoldDefinitionInspection, SampleHoldInspection, SampleHoldInspectionFailure, Schedule,
    ScheduleFailure, ToggleDefinitionInspection, ToggleInspection, ToggleInspectionFailure,
    TransportDelayDefinitionInspection, TransportDelayInspection, TransportDelayInspectionFailure,
};
pub use migration::{
    EpisodeMigrationRecord, EpisodeOutcome, EventMigrationRecord, EventOutcome,
    InputMigrationRecord, InputOutcome, InternalMigrationRecord, MigrationReport,
    ModuleMigrationRecord, OutputMigrationRecord, OutputOutcome, ProvenanceMigrationRecord,
    ProvenanceOutcome, ReconfigurationPolicy, SemanticLossRecord, StateMigrationRecord,
    StateOutcome, SubjectMigrationRecord,
};
pub use module::{
    DefinitionGraphView, ModuleDef, ModuleInputIter, ModuleOrigin, ModuleOutputIter, NodeSubject,
    PulsePortSubject, QualifiedConnectionRef, QualifiedInPortRef, QualifiedModuleRef,
    QualifiedNodeRef,
};
pub use patch::{
    ArtifactInvalidation, ConditionalArm, Continuity, EndpointChange, EndpointRebinding,
    EpisodeRule, EventRule, ExternalInputPlan, ExternalOutputPlan, HierarchicalSubjectRef,
    InertialDelayMigration, InputValuationPlan, InternalSubjectPlan, LossClass, ModuleContinuity,
    ModuleInternalReassociation, ModuleMigrationDirective, ModuleNodeMigrationDirective,
    NetworkPatch, NetworkPatchBuilder, NodeMigrationDirective, OutputBaselinePlan,
    OverdueMigrationPolicy, PatchBuildFailure, PatchOperation, PatchOperationIter, PendingArm,
    PendingWorkRule, PeriodicMigration, PotentialSemanticLoss, PreparedPatch, ProvenanceRule,
    PulseDelayMigration, RegionChange, RegionChangeKind, StateCompatibility, StaticMigrationPlan,
    StructuralSubjectRef, SubjectPlan, SubjectReassociation, TransportDelayMigration,
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
    CauseInspection, CauseLookupFailure, CauseRef, ForecastBasis, ForecastResult, OutputEvent,
    ProvenanceSubject, ProvenanceView, PulseContribution, ReconfigurationFailure, RuntimeFailure,
    RuntimeFailureEvidence, Transaction, TransactionBuildFailure, TransactionResult,
};
pub use validation::{NetworkDefinitionGraphView, ValidatedNetwork};

pub use time::ReactionStamp;
