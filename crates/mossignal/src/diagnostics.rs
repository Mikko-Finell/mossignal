//! Structured, catalogue-backed diagnostic findings and artifact reports.
//!
//! The opening catalogue is intentionally small.  Its types are the common
//! representation used by later graph construction and validation modules.

use crate::authored::{
    ConflictPolicy, EdgeInitialization, FirstEmissionPolicy, InputPortRole, OutputPortRole,
    ReenablePhasePolicy,
};
use crate::identity::{InputSchemaFingerprint, ModuleFingerprint, NetworkFingerprint};
use crate::key::{
    AnyExternalInputKey, AnyExternalOutputKey, AnyInPortKey, AnyModuleInputKey, AnyModuleOutputKey,
    AnyOutPortKey, ConnectionKey, ModuleInputKey, ModuleInstanceKey, NetworkKey, NodeKey,
};
use crate::machine::NetworkRevision;
use crate::metadata::OriginRef;
use crate::module::QualifiedNodeRef;
use crate::signal::{LogicLevel, PulseCount, SignalKind};
use crate::standard::{StandardModuleRef, StandardParameterKey, StandardParameterKind};
use crate::time::Time;
use core::cmp::Ordering;
use core::marker::PhantomData;

/// A stable subject to which a diagnostic condition applies.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum SubjectRef {
    /// An authored network.
    Network(NetworkKey),
    /// The immutable built-in standard catalogue.
    StandardCatalogue,
    /// A validated reusable module definition.
    ModuleDefinition(ModuleFingerprint),
    /// A typed public module input.
    ModuleInput(AnyModuleInputKey),
    /// A typed public module output.
    ModuleOutput(AnyModuleOutputKey),
    /// One stable module instance.
    ModuleInstance(ModuleInstanceKey),
    /// One exact public input of one module instance.
    ModuleInstanceInput(ModuleInstanceKey, AnyModuleInputKey),
    /// One exact public output of one module instance.
    ModuleInstanceOutput(ModuleInstanceKey, AnyModuleOutputKey),
    /// An authored node.
    Node(NodeKey),
    /// An authored module-local node qualified by its complete instance path.
    QualifiedNode(QualifiedNodeRef),
    /// An authored node input port.
    InPort(AnyInPortKey),
    /// An authored node output port.
    OutPort(AnyOutPortKey),
    /// An authored connection.
    Connection(ConnectionKey),
    /// An authored external input.
    ExternalInput(AnyExternalInputKey),
    /// An authored external output.
    ExternalOutput(AnyExternalOutputKey),
    /// One compiled-network-bound application binding slot.
    Binding(BindingSubjectRef),
    /// One non-structural operation boundary with no durable graph key.
    Operation(OperationSubjectRef),
}

/// Stable semantic identity for an operation boundary that has no structural key.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum OperationSubjectRef {
    Authoring,
    InputConstruction,
    InputProjection,
    OutputProjection,
    KeyProjection,
    LogicalTime,
    PulseCount,
    RuntimePolicy,
    MachineLifecycle,
    MachineTransaction,
    ProvenanceView,
    StandardModuleIdentifier,
    Persistence,
}

/// Stable identity for one non-structural application binding slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum BindingSubjectRef {
    Input {
        fingerprint: NetworkFingerprint,
        endpoint: AnyExternalInputKey,
    },
    Output {
        fingerprint: NetworkFingerprint,
        endpoint: AnyExternalOutputKey,
    },
}

/// The semantic role of a stable subject in the current-reaction graph.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ReactionRole {
    ExternalInput,
    ModuleInput,
    NodeOperation,
    NodeOutput,
    ModuleOutput,
    ExternalOutput,
}

/// A stable current-reaction graph member used in cycle diagnostics.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ReactionMemberRef {
    pub subject: SubjectRef,
    pub role: ReactionRole,
}

/// One stable dependency step in a current-reaction cycle witness.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct CurrentReactionCycleStep {
    pub source: ReactionMemberRef,
    pub dependency: SubjectRef,
    pub target: ReactionMemberRef,
}

impl SubjectRef {
    fn ordering_key(&self) -> (u8, SubjectPayload) {
        match self {
            Self::Network(key) => (0, SubjectPayload::Direct(key.as_u128())),
            Self::StandardCatalogue => (1, SubjectPayload::Direct(0)),
            Self::ModuleDefinition(fingerprint) => {
                (3, SubjectPayload::Fingerprint(fingerprint.as_bytes()))
            }
            Self::ModuleInput(key) => (5, SubjectPayload::ModuleInput(*key)),
            Self::ModuleOutput(key) => (6, SubjectPayload::ModuleOutput(*key)),
            Self::ModuleInstance(key) => (7, SubjectPayload::Direct(key.as_u128())),
            Self::ModuleInstanceInput(instance, key) => {
                (8, SubjectPayload::InstanceInput(*instance, *key))
            }
            Self::ModuleInstanceOutput(instance, key) => {
                (9, SubjectPayload::InstanceOutput(*instance, *key))
            }
            Self::Node(key) => (10, SubjectPayload::Direct(key.as_u128())),
            Self::QualifiedNode(node) => (10, SubjectPayload::QualifiedNode(node.clone())),
            Self::InPort(key) => (11, SubjectPayload::InPort(*key)),
            Self::OutPort(key) => (12, SubjectPayload::OutPort(*key)),
            Self::Connection(key) => (13, SubjectPayload::Direct(key.as_u128())),
            Self::ExternalInput(key) => (14, SubjectPayload::ExternalInput(*key)),
            Self::ExternalOutput(key) => (15, SubjectPayload::ExternalOutput(*key)),
            Self::Binding(binding) => (16, SubjectPayload::Binding(*binding)),
            Self::Operation(operation) => (17, SubjectPayload::Operation(*operation)),
        }
    }

    pub(crate) fn cmp_canonical(&self, other: &Self) -> Ordering {
        self.ordering_key().cmp(&other.ordering_key())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum SubjectPayload {
    Direct(u128),
    Fingerprint([u8; 32]),
    QualifiedNode(QualifiedNodeRef),
    ModuleInput(AnyModuleInputKey),
    ModuleOutput(AnyModuleOutputKey),
    InstanceInput(ModuleInstanceKey, AnyModuleInputKey),
    InstanceOutput(ModuleInstanceKey, AnyModuleOutputKey),
    InPort(AnyInPortKey),
    OutPort(AnyOutPortKey),
    ExternalInput(AnyExternalInputKey),
    ExternalOutput(AnyExternalOutputKey),
    Binding(BindingSubjectRef),
    Operation(OperationSubjectRef),
}

/// The severity fixed by a catalogue entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    Info,
    Warning,
    Error,
}

impl Severity {
    fn rank(self) -> u8 {
        match self {
            Self::Error => 0,
            Self::Warning => 1,
            Self::Info => 2,
        }
    }
}

/// The party or boundary responsible for a catalogue condition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Responsibility {
    Advisory,
    CallerInput,
    SemanticRejection,
    Compatibility,
    ResourceLimit,
    CorruptData,
    UnsupportedFeature,
    ExternalIntegration,
    LibraryDefect,
}

/// A delivery form permitted by a catalogue entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProblemDelivery {
    ReportFinding,
    OperationFailure,
    RuntimeOccurrence,
    /// A continuous condition retained in semantic machine state.
    PersistentEpisode,
    InternalDefect,
}

/// The catalogue evidence family fixed for one diagnostic code.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvidenceSchema {
    ForeignArtifact,
    KeyConflict,
    MissingReference,
    Direction,
    KindMismatch,
    DriverConflict,
    Arity,
    Parameter,
    StaticQuality,
    CurrentReactionCycle,
    ModuleSchema,
    Hierarchy,
    StandardModule,
    Binding,
    Lifecycle,
    RevisionMismatch,
    Time,
    Budget,
    InputObservation,
    InputSchema,
    Provenance,
    Conflict,
    InternalInvariant,
    CanonicalEncoding,
    VersionCompatibility,
    DigestMismatch,
    DigestCollision,
    ArtifactIdentity,
    StateSchema,
    PendingEvent,
    DiagnosticEpisode,
    Replay,
    PatchEdit,
    Migration,
    SemanticLoss,
    StaleArtifact,
}

/// The opening catalogue's structured identifiers.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum DiagnosticCode {
    AuthoringForeignSignal,
    ValidationDuplicateKey,
    ValidationMissingNode,
    ValidationMissingPort,
    ValidationMissingEndpoint,
    ValidationInvalidDirection,
    ValidationSignalKindMismatch,
    ValidationInvalidParameter,
    ValidationUnsupportedMultipleDrivers,
    ValidationMissingRequiredInput,
    ValidationInvalidFixedArity,
    ValidationInvalidVariadicArity,
    ValidationDuplicateSource,
    ValidationEmptyVariadicNode,
    ValidationUnaryDegenerateNode,
    ValidationConstantResultNode,
    ValidationCurrentReactionCycle,
    ValidationInvalidModuleBinding,
    ValidationMalformedHierarchy,
    ValidationHierarchyCycle,
    StandardModuleUnknownId,
    StandardModuleUnsupportedVersion,
    StandardModuleMissingParameter,
    StandardModuleUnexpectedParameter,
    StandardModuleParameterKindMismatch,
    StandardModuleInvalidParameter,
    StandardModuleInterfaceMismatch,
    StandardModuleInternalKeyCollision,
    StandardModuleCatalogueInvariant,
    StandardModuleEmptyVariadic,
    StandardModuleUnaryDegenerate,
    StandardModuleImpossibleThreshold,
    StandardModuleConstantResult,
    StandardModuleDuplicateSource,
    BindingUnknownEndpoint,
    BindingWrongSignalKind,
    BindingDuplicateEndpoint,
    BindingDuplicateExternalKey,
    BindingAmbiguousExternalKey,
    BindingMissingRequiredBinding,
    BindingWrongNetwork,
    BindingStaleSchema,
    LifecycleNotInitialized,
    LifecycleAlreadyInitialized,
    LifecycleDeltaBeforeInitialization,
    RuntimeStaleRevision,
    RuntimeStaleExecutionState,
    RuntimeTimeRegression,
    RuntimeReactionOrderOverflow,
    RuntimeTimeOverflow,
    RuntimeInvalidTimeSubtraction,
    RuntimeZeroSpanNotAllowed,
    RuntimePulseCountOverflow,
    RuntimePolicyMissingLimit,
    RuntimePolicyInvalidLimit,
    RuntimeBudgetExceeded,
    RuntimePulseLatchConflictRetained,
    RuntimePulseLatchConflictRejected,
    RuntimeLevelLatchConflictRetained,
    RuntimeLevelLatchConflictRejected,
    InputUnknownEndpoint,
    InputWrongSignalKind,
    InputDuplicateObservation,
    InputConflictingObservation,
    InputMissingRequiredLevel,
    InputWrongNetwork,
    InputForeignSchema,
    InputStaleSchema,
    InspectionUnknownSubject,
    InspectionWrongSubjectKind,
    InspectionPendingEventNotFound,
    ExplanationUnknownSubject,
    ExplanationForeignCause,
    ExplanationInvalidCause,
    InternalDiagnosticEvidenceConflict,
    PersistenceInvalidPrefix,
    PersistenceTruncatedArtifact,
    PersistenceTrailingBytes,
    PersistenceNoncanonicalEncoding,
    PersistenceMalformedEnvelope,
    PersistenceUnknownArtifactKind,
    PersistenceUnknownSchemaField,
    PersistenceUnknownSchemaVariant,
    PersistenceIntegrityDigestMismatch,
    PersistenceDecodeLimitExceeded,
    PersistenceUnsupportedVersion,
    PersistenceWrongTimeDomain,
    PersistenceNetworkIdentityMismatch,
    PersistenceFingerprintMismatch,
    PersistenceTopologyRevisionMismatch,
    PersistenceRuntimePolicyMismatch,
    PersistenceLifecycleShapeInvalid,
    PersistenceStateSchemaMismatch,
    PersistenceUnknownSubject,
    PersistencePendingEventInvalid,
    PersistenceEventIdentityStateInvalid,
    PersistenceDiagnosticEpisodeInvalid,
    PersistenceDiagnosticSchemaInvalid,
    PersistenceSettledStateInconsistent,
    PersistenceExecutionDigestMismatch,
    PersistenceObservableDigestMismatch,
    PersistenceSnapshotDigestMismatch,
    PersistenceDigestCollision,
    PersistenceProvenanceMissingPredecessor,
    PersistenceProvenanceDigestMismatch,
    PersistenceProvenanceCycle,
    PersistenceProvenanceInvalidSubject,
    PersistenceProvenanceInvalidRole,
    PersistenceProvenanceIncompleteRootClosure,
    PersistenceProvenanceConflictingRecord,
    PersistenceProvenanceFalseCheckpoint,
    StandardModuleExpansionMismatch,
    ReplayStartingExecutionDigestMismatch,
    ReplayStartingObservableDigestMismatch,
    ReplayExpectedRevisionMismatch,
    ReplayRuntimePolicyMismatch,
    ReplayTimeDomainMismatch,
    ReplayNetworkFingerprintMismatch,
    ReplayLogsNotConcatenable,
    ReplayPatchPreparationDiverged,
    ReplayResultingExecutionDigestMismatch,
    ReplayResultingObservableDigestMismatch,
    ReplayFrameMissing,
    ReplayFrameReordered,
    ReplayFrameDuplicated,
    ReconfigurationForeignArtifact,
    ReconfigurationDuplicateOperation,
    ReconfigurationConflictingEdit,
    ReconfigurationInvalidReplacementKey,
    ReconfigurationContradictoryHierarchy,
    ReconfigurationInvalidReassociation,
    ReconfigurationBaseFingerprintMismatch,
    ReconfigurationBaseRevisionMismatch,
    ReconfigurationUnknownBaseSubject,
    ReconfigurationNonInjectiveReassociation,
    ReconfigurationIncompatibleMigrationDirective,
    ReconfigurationIncompleteTemporalMigrationPolicy,
    ReconfigurationUnsupportedCrossKindMigration,
    ReconfigurationAmbiguousEventMigration,
    ReconfigurationConflictingMigratedTransitions,
    ReconfigurationInvalidTargetInputSchema,
    ReconfigurationConditionalSemanticLoss,
    ReconfigurationUnavoidableSemanticLoss,
    ReconfigurationEmptyPatch,
    ReconfigurationStalePreparedPatch,
    ReconfigurationTargetInputSchemaMismatch,
    ReconfigurationStateMigrationRejected,
    ReconfigurationPendingEventMigrationRejected,
    ReconfigurationRequirePreserveFailed,
    ReconfigurationEpisodeMigrationRejected,
    ReconfigurationProvenanceMigrationRejected,
    ReconfigurationStateLossRejected,
    StandardModuleNoncanonicalInternalEdit,
}

#[derive(Clone, Copy)]
struct CodeSpecification {
    code: &'static str,
    severity: Severity,
    responsibility: Responsibility,
    evidence_schema: EvidenceSchema,
    delivery: DeliverySet,
}

#[derive(Clone, Copy)]
struct DeliverySet {
    report_finding: bool,
    operation_failure: bool,
    runtime_occurrence: bool,
    persistent_episode: bool,
    internal_defect: bool,
}

/// Why a related semantic subject is included in a problem.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum RelatedSubjectRole {
    Source,
    Target,
    Owner,
    Driver,
    ConflictingDriver,
    ConflictingClaim,
    MissingReference,
    ExpectedSubject,
    ActualSubject,
    BaseSubject,
    TargetSubject,
    MigrationSource,
    MigrationTarget,
    CyclePredecessor,
    CycleSuccessor,
    Supporter,
    Blocker,
    InvalidatedArtifact,
}

/// A typed relationship to another subject involved in a problem.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelatedSubject {
    pub role: RelatedSubjectRole,
    pub subject: SubjectRef,
}

/// The fixed node port group that an arity condition addresses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum FixedArityRole {
    Input,
    Output,
}

/// A lossless projection of one authored claim that conflicts on a stable key.
///
/// This keeps duplicate-key evidence independent of caller record order while
/// retaining the authored facts needed to distinguish conflicting claims.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum DuplicateClaim {
    Node {
        key: NodeKey,
        kind: DuplicateNodeKind,
        inputs: Vec<AnyInPortKey>,
        input_roles: Vec<InputPortRole>,
        outputs: Vec<AnyOutPortKey>,
        output_roles: Vec<OutputPortRole>,
        origin: Option<OriginRef>,
    },
    InPort {
        key: AnyInPortKey,
        owner: NodeKey,
        origin: Option<OriginRef>,
    },
    OutPort {
        key: AnyOutPortKey,
        owner: NodeKey,
        origin: Option<OriginRef>,
    },
    Connection {
        key: ConnectionKey,
        source: SubjectRef,
        target: SubjectRef,
        origin: Option<OriginRef>,
    },
    ExternalInput {
        key: AnyExternalInputKey,
        origin: Option<OriginRef>,
    },
    ExternalOutput {
        key: AnyExternalOutputKey,
        source: SubjectRef,
        origin: Option<OriginRef>,
    },
    ModuleInput {
        key: AnyModuleInputKey,
        origin: Option<OriginRef>,
    },
    ModuleOutput {
        key: AnyModuleOutputKey,
        origin: Option<OriginRef>,
    },
    ModuleInstance {
        key: ModuleInstanceKey,
        module: ModuleFingerprint,
        parent: Option<ModuleInstanceKey>,
        bindings: Vec<(AnyModuleInputKey, SubjectRef)>,
        origin: Option<OriginRef>,
    },
}

/// The restricted node-kind facts retained by a duplicate node claim.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum DuplicateNodeKind {
    Constant(LogicLevel),
    Not,
    All,
    Any,
    Parity,
    AtLeast(u64),
    Select,
    Merge,
    Coalesce,
    Zip,
    PulseGate,
    PulseSelect,
    PulseRoute,
    RisingEdge(EdgeInitialization),
    FallingEdge(EdgeInitialization),
    AnyEdge(EdgeInitialization),
    Toggle(LogicLevel),
    PulseSetResetLatch(LogicLevel, ConflictPolicy),
    LevelSetResetLatch(LogicLevel, ConflictPolicy),
    SampleHold(LogicLevel),
    PulseDelay(u64),
    TransportDelay(u64, LogicLevel),
    InertialDelay(u64, LogicLevel),
    Periodic(u64, FirstEmissionPolicy, ReenablePhasePolicy),
}

/// The stable identity of one required fixed input that is absent.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RequiredInputRef {
    /// An authored input port exists but has no driver.
    Port(SubjectRef),
    /// A fixed semantic input role has no corresponding authored port.
    Role(InputPortRole),
}

/// The exact public-interface defect represented by module-binding evidence.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModuleBindingIssue {
    Missing,
    Duplicate,
    Invalid,
}

/// A safe, machine-readable correction.  The opening validation catalogue has
/// no unambiguous automatic correction, so no constructors are exposed yet.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Suggestion {}

/// Catalogue evidence shared by application-binding conditions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BindingEvidence {
    pub network: NetworkKey,
    pub fingerprint: NetworkFingerprint,
    pub revision: NetworkRevision,
    pub endpoint: Option<BindingSubjectRef>,
    pub conflicting: Vec<BindingSubjectRef>,
    pub missing: Vec<BindingSubjectRef>,
    pub expected_kind: Option<SignalKind>,
}

/// Evidence shared by lifecycle operation failures.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LifecycleEvidence {
    pub operation: OperationSubjectRef,
    pub current_time_ticks: Option<u64>,
}

/// A stable node identity retained without private runtime positions.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NodeEvidence {
    Node(NodeKey),
    Qualified {
        instances: Vec<ModuleInstanceKey>,
        node: NodeKey,
    },
}

/// The checked logical-time operation represented by [`TimeEvidence`].
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeOperation {
    ReactionOrderIncrement,
    TimeAddition,
    NonZeroTimeAddition,
    SpanAddition,
    DurationSubtraction,
    TransactionAdvance,
    PulseDelayDeadline,
    TransportDelayDeadline,
    InertialDelayDeadline,
    PeriodicDeadline,
    /// Checked arithmetic while finalizing a topology replacement.
    Reconfiguration,
}

/// Exact operands and relation for a logical-time condition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimeEvidence {
    pub owner: Option<NodeEvidence>,
    pub operation: TimeOperation,
    pub left_ticks: u64,
    pub right_ticks: u64,
}

/// Evidence for a runtime-policy or scalar-domain parameter condition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParameterEvidence {
    pub owner: Option<NodeEvidence>,
    pub parameter: &'static str,
    pub expected_domain: &'static str,
    pub encountered: Option<u64>,
    pub operands: Vec<u64>,
}

/// One exact value supplied as an external input observation.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputObservationValue {
    Level(LogicLevel),
    Pulse(PulseCount),
}

/// Evidence shared by input-construction and input-projection conditions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InputObservationEvidence {
    pub endpoint: Option<AnyExternalInputKey>,
    pub expected_kind: Option<SignalKind>,
    pub actual_kind: Option<SignalKind>,
    pub observations: Vec<InputObservationValue>,
    pub missing: Vec<AnyExternalInputKey>,
}

/// Evidence for use of an input artifact against the wrong schema context.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InputSchemaEvidence {
    pub expected_network: Option<NetworkKey>,
    pub actual_network: Option<NetworkKey>,
    pub expected_fingerprint: Option<NetworkFingerprint>,
    pub actual_fingerprint: Option<NetworkFingerprint>,
    pub expected_schema: Option<InputSchemaFingerprint>,
    pub actual_schema: Option<InputSchemaFingerprint>,
}

/// The closed subject category required by an inspection projection.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InspectionSubjectKind {
    Node,
    PulseDelay,
    TransportDelay,
    InertialDelay,
    Periodic,
    EdgeDetector,
    Toggle,
    PulseSetResetLatch,
    LevelSetResetLatch,
    SampleHold,
    Module,
    LevelOutput,
    GraphElement(crate::GraphElement),
    OutputEvent(usize),
    SignalKind(SignalKind),
}

/// Evidence for a missing or wrong-kind inspected subject.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InspectionEvidence {
    pub requested: SubjectRef,
    pub qualified_path: Vec<ModuleInstanceKey>,
    pub expected: InspectionSubjectKind,
    pub actual: Option<InspectionSubjectKind>,
}

/// Evidence for resolving one opaque cause through a provenance view.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProvenanceEvidence {
    pub expected_scope: [u8; 32],
    pub actual_scope: [u8; 32],
    pub ordinal: u32,
}

/// Evidence for a topology-revision compatibility rejection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RevisionMismatchEvidence {
    pub expected: NetworkRevision,
    pub actual: NetworkRevision,
}

/// Evidence for a named runtime budget rejection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BudgetEvidence {
    pub budget: &'static str,
    pub limit: u64,
    pub consumed: u64,
}

/// The exact controls of a set/reset conflict, without erasing signal kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConflictControls {
    /// Complete simultaneous pulse counts.
    Pulse { set: PulseCount, reset: PulseCount },
    /// Fully settled current levels.
    Level { set: LogicLevel, reset: LogicLevel },
}

/// Exact structured evidence for one set/reset conflict.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConflictEvidence {
    /// The stable direct or module-qualified primitive subject.
    pub node: NodeEvidence,
    /// The configured conflict law.
    pub policy: ConflictPolicy,
    /// The previous stored level read by the reaction.
    pub previous: LogicLevel,
    /// The exact simultaneous controls, retaining their signal kind.
    pub controls: ConflictControls,
    /// The exact logical tick at which the conflict settled.
    pub at_ticks: u64,
    /// The producing occurrence order at that exact physical time.
    pub reaction_order: u64,
    /// The topology revision under which the conflict settled.
    pub revision: NetworkRevision,
}

/// Canonical-encoding failure for one persistence artifact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CanonicalEncodingEvidence {
    /// Artifact kind when the envelope identified one.
    pub artifact_kind: Option<String>,
    /// Byte offset or structural path, when one is known.
    pub path: String,
    /// Catalogue canonical-violation kind.
    pub violation: &'static str,
    /// Encountered token, field, length, or ordering fact.
    pub encountered: String,
}

/// Version-vector incompatibility for one persistence artifact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VersionCompatibilityEvidence {
    /// Artifact kind being decoded or restored.
    pub artifact_kind: String,
    /// Compatibility stage that rejected the component.
    pub stage: &'static str,
    /// Version-vector component name.
    pub component: String,
    /// Version found in the artifact.
    pub encountered: String,
    /// Version required by the current support rule.
    pub required: String,
    /// Whether a representation upgrader exists for this component.
    pub upgrader_exists: bool,
}

/// Identity mismatch between an artifact and the caller-supplied context.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtifactIdentityEvidence {
    /// Artifact kind being checked.
    pub artifact_kind: String,
    /// Compatibility stage that rejected the identity.
    pub stage: &'static str,
    /// Expected time domain, network, fingerprint, revision, or policy.
    pub expected: String,
    /// Identity found in the artifact.
    pub actual: String,
}

/// A prepared topology replacement that no longer matches the live machine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StaleArtifactEvidence {
    /// Network key expected by the prepared patch.
    pub expected_network: String,
    /// Network key installed on the machine.
    pub actual_network: String,
    /// Prepared base revision.
    pub expected_revision: u64,
    /// Machine revision.
    pub actual_revision: u64,
    /// Prepared base fingerprint.
    pub expected_fingerprint: String,
    /// Machine fingerprint.
    pub actual_fingerprint: String,
    /// Prepared time domain.
    pub expected_time_domain: String,
    /// Machine time domain.
    pub actual_time_domain: String,
}

/// One state, event, or preservation rejection during finalization.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MigrationEvidence {
    /// Stable subject the rule rejected.
    pub subject: SubjectRef,
    /// State or event fact that selected the rejection.
    pub fact: String,
    /// Migration rule that selected the rejection.
    pub rule: String,
}

/// One realized semantic loss forbidden by the caller policy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SemanticLossEvidence {
    /// Stable subject that lost the fact.
    pub subject: SubjectRef,
    /// Lost fact.
    pub fact: String,
    /// Rule or removal that caused the loss.
    pub rule: String,
    /// Whether the loss was conditional on pre-patch state.
    pub conditional: bool,
}

/// Recomputed digest disagreement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DigestMismatchEvidence {
    /// Digest family that disagreed.
    pub kind: &'static str,
    /// Digest claimed by the artifact.
    pub expected: String,
    /// Digest recomputed from canonical content.
    pub actual: String,
    /// Artifact or machine context of the comparison.
    pub context: String,
}

/// Two different canonical records sharing one typed digest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DigestCollisionEvidence {
    /// Digest family.
    pub kind: &'static str,
    /// Domain separation label.
    pub domain: &'static str,
    /// Shared digest value.
    pub digest: String,
    /// First canonical record.
    pub first: String,
    /// Second conflicting canonical record.
    pub second: String,
    /// Artifact context.
    pub context: String,
}

/// Persisted state that does not match the compiled state schema.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateSchemaEvidence {
    /// Stable state owner, when one was named.
    pub owner: String,
    /// Schema required by the compiled topology.
    pub expected: String,
    /// Schema or value found in the artifact.
    pub encountered: String,
}

/// A persisted subject that the compiled topology does not contain.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MissingSubjectEvidence {
    /// Stable subject text from the artifact.
    pub subject: String,
}

/// Pending-event identity, payload, or allocator failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingEventEvidence {
    /// Public pending-event serial, when one was present.
    pub event: Option<u64>,
    /// Stable owner text.
    pub owner: String,
    /// Originating logical time, when present.
    pub origin: Option<u64>,
    /// Deadline, when present.
    pub deadline: Option<u64>,
    /// Expected invariant that failed.
    pub detail: String,
}

/// Persisted diagnostic-episode identity or schema failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiagnosticEpisodeEvidence {
    /// Episode identity text.
    pub identity: String,
    /// Catalogue code text from the artifact.
    pub code: String,
    /// Stable owner text.
    pub owner: String,
    /// Condition discriminator, when present.
    pub discriminator: Option<u64>,
    /// Beginning logical time, when present.
    pub began_at: Option<u64>,
    /// Last material-change logical time, when present.
    pub last_material_change: Option<u64>,
}

/// Provenance-graph corruption found while restoring a snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PersistenceProvenanceEvidence {
    /// Cause digest or checkpoint text.
    pub digest: String,
    /// Record kind, when known.
    pub record_kind: String,
    /// Semantic subject text.
    pub subject: String,
    /// Predecessor digest, when the failure names one.
    pub predecessor: String,
    /// Predecessor role, when the failure names one.
    pub role: String,
    /// Required root that failed to close.
    pub root: String,
    /// Checkpoint boundary claimed by the artifact.
    pub checkpoint: String,
    /// Conflicting canonical bytes, when two records collide.
    pub conflict: String,
}

/// Settled values that disagree with the reference evaluation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettledStateEvidence {
    /// Stable subject or fact that disagreed.
    pub detail: String,
}

/// Localization for one replay checkpoint or frame failure.
///
/// Absent facts are empty strings or `None`. The frame index is location
/// context and is not a separate catalogue subject.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplayEvidence {
    /// Log identity when the failure names one.
    pub log_id: String,
    /// Frame identity when the failure names one.
    pub frame_id: String,
    /// Zero-based frame position when the failure is inside a sequence.
    pub frame_index: Option<u64>,
    /// Requested logical time when the frame names one.
    pub logical_time: Option<u64>,
    /// Revision expected by the log or frame.
    pub expected_revision: Option<u64>,
    /// Revision found on the machine.
    pub actual_revision: Option<u64>,
    /// Expected network fingerprint, when the failure compares one.
    pub expected_fingerprint: String,
    /// Fingerprint found on the machine or artifact.
    pub actual_fingerprint: String,
    /// Expected runtime-policy identity, when the failure compares one.
    pub expected_policy: String,
    /// Policy identity found on the machine or artifact.
    pub actual_policy: String,
    /// Expected execution-state digest.
    pub expected_execution_digest: String,
    /// Execution-state digest found on the machine.
    pub actual_execution_digest: String,
    /// Expected observable-state digest.
    pub expected_observable_digest: String,
    /// Observable-state digest found on the machine.
    pub actual_observable_digest: String,
    /// Expected time-domain identity, when the failure compares one.
    pub expected_time_domain: String,
    /// Time-domain identity found on the machine or artifact.
    pub actual_time_domain: String,
    /// Catalogue code of a nested failure when one is being located.
    pub underlying_code: String,
}

impl ReplayEvidence {
    pub(crate) fn new() -> Self {
        Self {
            log_id: String::new(),
            frame_id: String::new(),
            frame_index: None,
            logical_time: None,
            expected_revision: None,
            actual_revision: None,
            expected_fingerprint: String::new(),
            actual_fingerprint: String::new(),
            expected_policy: String::new(),
            actual_policy: String::new(),
            expected_execution_digest: String::new(),
            actual_execution_digest: String::new(),
            expected_observable_digest: String::new(),
            actual_observable_digest: String::new(),
            expected_time_domain: String::new(),
            actual_time_domain: String::new(),
            underlying_code: String::new(),
        }
    }
}

/// Structured evidence for one exact opening catalogue code.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProblemEvidence<D> {
    AuthoringForeignSignal {
        expected_builder: u64,
        actual_builder: u64,
        marker: PhantomData<fn() -> D>,
    },
    ValidationDuplicateKey {
        key: SubjectRef,
        claims: Vec<DuplicateClaim>,
        marker: PhantomData<fn() -> D>,
    },
    ValidationMissingNode {
        missing: NodeKey,
        marker: PhantomData<fn() -> D>,
    },
    ValidationMissingPort {
        missing: SubjectRef,
        expected_kind: SignalKind,
        marker: PhantomData<fn() -> D>,
    },
    ValidationMissingEndpoint {
        missing: SubjectRef,
        expected_kind: SignalKind,
        marker: PhantomData<fn() -> D>,
    },
    ValidationInvalidDirection {
        source: SubjectRef,
        target: SubjectRef,
        marker: PhantomData<fn() -> D>,
    },
    ValidationSignalKindMismatch {
        source: SubjectRef,
        target: SubjectRef,
        source_kind: SignalKind,
        target_kind: SignalKind,
        marker: PhantomData<fn() -> D>,
    },
    ValidationInvalidParameter {
        parameter: &'static str,
        encountered: String,
        marker: PhantomData<fn() -> D>,
    },
    ValidationUnsupportedMultipleDrivers {
        drivers: Vec<SubjectRef>,
        marker: PhantomData<fn() -> D>,
    },
    ValidationMissingRequiredInput {
        required: RequiredInputRef,
        expected_kind: SignalKind,
        marker: PhantomData<fn() -> D>,
    },
    ValidationInvalidFixedArity {
        role: FixedArityRole,
        ports: Vec<SubjectRef>,
        expected: usize,
        encountered: usize,
        marker: PhantomData<fn() -> D>,
    },
    ValidationInvalidVariadicArity {
        ports: Vec<SubjectRef>,
        minimum: usize,
        encountered: usize,
        marker: PhantomData<fn() -> D>,
    },
    ValidationDuplicateSource {
        source: SubjectRef,
        ports: Vec<SubjectRef>,
        marker: PhantomData<fn() -> D>,
    },
    ValidationEmptyVariadicNode {
        ports: Vec<SubjectRef>,
        marker: PhantomData<fn() -> D>,
    },
    ValidationUnaryDegenerateNode {
        ports: Vec<SubjectRef>,
        marker: PhantomData<fn() -> D>,
    },
    ValidationConstantResultNode {
        inputs: Vec<SubjectRef>,
        result: LogicLevel,
        marker: PhantomData<fn() -> D>,
    },
    ValidationCurrentReactionCycle {
        members: Vec<ReactionMemberRef>,
        witness: Vec<CurrentReactionCycleStep>,
        marker: PhantomData<fn() -> D>,
    },
    ValidationInvalidModuleBinding {
        instance: ModuleInstanceKey,
        input: AnyModuleInputKey,
        issue: ModuleBindingIssue,
        sources: Vec<SubjectRef>,
        marker: PhantomData<fn() -> D>,
    },
    ValidationMalformedHierarchy {
        instance: ModuleInstanceKey,
        parent: ModuleInstanceKey,
        marker: PhantomData<fn() -> D>,
    },
    ValidationHierarchyCycle {
        instances: Vec<ModuleInstanceKey>,
        marker: PhantomData<fn() -> D>,
    },
    StandardModuleUnknownId {
        module_ref: StandardModuleRef,
        marker: PhantomData<fn() -> D>,
    },
    StandardModuleUnsupportedVersion {
        module_ref: StandardModuleRef,
        marker: PhantomData<fn() -> D>,
    },
    StandardModuleMissingParameter {
        module_ref: StandardModuleRef,
        parameter: StandardParameterKey,
        marker: PhantomData<fn() -> D>,
    },
    StandardModuleUnexpectedParameter {
        module_ref: StandardModuleRef,
        parameter: StandardParameterKey,
        marker: PhantomData<fn() -> D>,
    },
    StandardModuleParameterKindMismatch {
        module_ref: StandardModuleRef,
        parameter: StandardParameterKey,
        expected: StandardParameterKind,
        encountered: StandardParameterKind,
        marker: PhantomData<fn() -> D>,
    },
    StandardModuleInvalidParameter {
        module_ref: StandardModuleRef,
        parameter: StandardParameterKey,
        marker: PhantomData<fn() -> D>,
    },
    StandardModuleInterfaceMismatch {
        module_ref: StandardModuleRef,
        inputs: Vec<AnyModuleInputKey>,
        marker: PhantomData<fn() -> D>,
    },
    StandardModuleInternalKeyCollision {
        module_ref: StandardModuleRef,
        key: u128,
        marker: PhantomData<fn() -> D>,
    },
    StandardModuleCatalogueInvariant {
        module_ref: StandardModuleRef,
        detail: String,
        marker: PhantomData<fn() -> D>,
    },
    StandardModuleEmptyVariadic {
        module_ref: StandardModuleRef,
        inputs: Vec<ModuleInputKey<crate::signal::Level>>,
        marker: PhantomData<fn() -> D>,
    },
    StandardModuleUnaryDegenerate {
        module_ref: StandardModuleRef,
        inputs: Vec<ModuleInputKey<crate::signal::Level>>,
        marker: PhantomData<fn() -> D>,
    },
    StandardModuleImpossibleThreshold {
        module_ref: StandardModuleRef,
        arity: usize,
        threshold: u64,
        marker: PhantomData<fn() -> D>,
    },
    StandardModuleConstantResult {
        module_ref: StandardModuleRef,
        result: LogicLevel,
        marker: PhantomData<fn() -> D>,
    },
    StandardModuleDuplicateSource {
        module_ref: StandardModuleRef,
        source: SubjectRef,
        inputs: Vec<ModuleInputKey<crate::signal::Level>>,
        marker: PhantomData<fn() -> D>,
    },
    BindingUnknownEndpoint {
        evidence: BindingEvidence,
        marker: PhantomData<fn() -> D>,
    },
    BindingWrongSignalKind {
        evidence: BindingEvidence,
        marker: PhantomData<fn() -> D>,
    },
    BindingDuplicateEndpoint {
        evidence: BindingEvidence,
        marker: PhantomData<fn() -> D>,
    },
    BindingDuplicateExternalKey {
        evidence: BindingEvidence,
        marker: PhantomData<fn() -> D>,
    },
    BindingAmbiguousExternalKey {
        evidence: BindingEvidence,
        marker: PhantomData<fn() -> D>,
    },
    BindingMissingRequiredBinding {
        evidence: BindingEvidence,
        marker: PhantomData<fn() -> D>,
    },
    BindingWrongNetwork {
        evidence: BindingEvidence,
        marker: PhantomData<fn() -> D>,
    },
    BindingStaleSchema {
        evidence: BindingEvidence,
        marker: PhantomData<fn() -> D>,
    },
    LifecycleNotInitialized {
        evidence: LifecycleEvidence,
        marker: PhantomData<fn() -> D>,
    },
    LifecycleAlreadyInitialized {
        evidence: LifecycleEvidence,
        marker: PhantomData<fn() -> D>,
    },
    LifecycleDeltaBeforeInitialization {
        evidence: LifecycleEvidence,
        marker: PhantomData<fn() -> D>,
    },
    RuntimeStaleRevision {
        evidence: RevisionMismatchEvidence,
        marker: PhantomData<fn() -> D>,
    },
    RuntimeStaleExecutionState {
        evidence: DigestMismatchEvidence,
        marker: PhantomData<fn() -> D>,
    },
    RuntimeTimeRegression {
        evidence: TimeEvidence,
        marker: PhantomData<fn() -> D>,
    },
    RuntimeReactionOrderOverflow {
        evidence: TimeEvidence,
        marker: PhantomData<fn() -> D>,
    },
    RuntimeTimeOverflow {
        evidence: TimeEvidence,
        marker: PhantomData<fn() -> D>,
    },
    RuntimeInvalidTimeSubtraction {
        evidence: TimeEvidence,
        marker: PhantomData<fn() -> D>,
    },
    RuntimeZeroSpanNotAllowed {
        evidence: ParameterEvidence,
        marker: PhantomData<fn() -> D>,
    },
    RuntimePulseCountOverflow {
        evidence: ParameterEvidence,
        marker: PhantomData<fn() -> D>,
    },
    RuntimePolicyMissingLimit {
        evidence: ParameterEvidence,
        marker: PhantomData<fn() -> D>,
    },
    RuntimePolicyInvalidLimit {
        evidence: ParameterEvidence,
        marker: PhantomData<fn() -> D>,
    },
    RuntimeBudgetExceeded {
        evidence: BudgetEvidence,
        marker: PhantomData<fn() -> D>,
    },
    RuntimePulseLatchConflictRetained {
        evidence: ConflictEvidence,
        marker: PhantomData<fn() -> D>,
    },
    RuntimePulseLatchConflictRejected {
        evidence: ConflictEvidence,
        marker: PhantomData<fn() -> D>,
    },
    RuntimeLevelLatchConflictRetained {
        evidence: ConflictEvidence,
        marker: PhantomData<fn() -> D>,
    },
    RuntimeLevelLatchConflictRejected {
        evidence: ConflictEvidence,
        marker: PhantomData<fn() -> D>,
    },
    InputUnknownEndpoint {
        evidence: InputObservationEvidence,
        marker: PhantomData<fn() -> D>,
    },
    InputWrongSignalKind {
        evidence: InputObservationEvidence,
        marker: PhantomData<fn() -> D>,
    },
    InputDuplicateObservation {
        evidence: InputObservationEvidence,
        marker: PhantomData<fn() -> D>,
    },
    InputConflictingObservation {
        evidence: InputObservationEvidence,
        marker: PhantomData<fn() -> D>,
    },
    InputMissingRequiredLevel {
        evidence: InputObservationEvidence,
        marker: PhantomData<fn() -> D>,
    },
    InputWrongNetwork {
        evidence: InputSchemaEvidence,
        marker: PhantomData<fn() -> D>,
    },
    InputForeignSchema {
        evidence: InputSchemaEvidence,
        marker: PhantomData<fn() -> D>,
    },
    InputStaleSchema {
        evidence: InputSchemaEvidence,
        marker: PhantomData<fn() -> D>,
    },
    InspectionUnknownSubject {
        evidence: InspectionEvidence,
        marker: PhantomData<fn() -> D>,
    },
    InspectionWrongSubjectKind {
        evidence: InspectionEvidence,
        marker: PhantomData<fn() -> D>,
    },
    InspectionPendingEventNotFound {
        evidence: PendingEventEvidence,
        marker: PhantomData<fn() -> D>,
    },
    ExplanationUnknownSubject {
        evidence: InspectionEvidence,
        marker: PhantomData<fn() -> D>,
    },
    ExplanationForeignCause {
        evidence: ProvenanceEvidence,
        marker: PhantomData<fn() -> D>,
    },
    ExplanationInvalidCause {
        evidence: ProvenanceEvidence,
        marker: PhantomData<fn() -> D>,
    },
    InternalDiagnosticEvidenceConflict {
        conflicting_code: DiagnosticCode,
        conflicting_primary: SubjectRef,
        marker: PhantomData<fn() -> D>,
    },
    PersistenceInvalidPrefix {
        evidence: CanonicalEncodingEvidence,
        marker: PhantomData<fn() -> D>,
    },
    PersistenceTruncatedArtifact {
        evidence: CanonicalEncodingEvidence,
        marker: PhantomData<fn() -> D>,
    },
    PersistenceTrailingBytes {
        evidence: CanonicalEncodingEvidence,
        marker: PhantomData<fn() -> D>,
    },
    PersistenceNoncanonicalEncoding {
        evidence: CanonicalEncodingEvidence,
        marker: PhantomData<fn() -> D>,
    },
    PersistenceMalformedEnvelope {
        evidence: CanonicalEncodingEvidence,
        marker: PhantomData<fn() -> D>,
    },
    PersistenceUnknownArtifactKind {
        evidence: VersionCompatibilityEvidence,
        marker: PhantomData<fn() -> D>,
    },
    PersistenceUnknownSchemaField {
        evidence: VersionCompatibilityEvidence,
        marker: PhantomData<fn() -> D>,
    },
    PersistenceUnknownSchemaVariant {
        evidence: VersionCompatibilityEvidence,
        marker: PhantomData<fn() -> D>,
    },
    PersistenceIntegrityDigestMismatch {
        evidence: DigestMismatchEvidence,
        marker: PhantomData<fn() -> D>,
    },
    PersistenceDecodeLimitExceeded {
        evidence: BudgetEvidence,
        marker: PhantomData<fn() -> D>,
    },
    PersistenceUnsupportedVersion {
        evidence: VersionCompatibilityEvidence,
        marker: PhantomData<fn() -> D>,
    },
    PersistenceWrongTimeDomain {
        evidence: ArtifactIdentityEvidence,
        marker: PhantomData<fn() -> D>,
    },
    PersistenceNetworkIdentityMismatch {
        evidence: ArtifactIdentityEvidence,
        marker: PhantomData<fn() -> D>,
    },
    PersistenceFingerprintMismatch {
        evidence: ArtifactIdentityEvidence,
        marker: PhantomData<fn() -> D>,
    },
    PersistenceTopologyRevisionMismatch {
        evidence: ArtifactIdentityEvidence,
        marker: PhantomData<fn() -> D>,
    },
    PersistenceRuntimePolicyMismatch {
        evidence: ArtifactIdentityEvidence,
        marker: PhantomData<fn() -> D>,
    },
    PersistenceLifecycleShapeInvalid {
        evidence: ParameterEvidence,
        marker: PhantomData<fn() -> D>,
    },
    PersistenceStateSchemaMismatch {
        evidence: StateSchemaEvidence,
        marker: PhantomData<fn() -> D>,
    },
    PersistenceUnknownSubject {
        evidence: MissingSubjectEvidence,
        marker: PhantomData<fn() -> D>,
    },
    PersistencePendingEventInvalid {
        evidence: PendingEventEvidence,
        marker: PhantomData<fn() -> D>,
    },
    PersistenceEventIdentityStateInvalid {
        evidence: PendingEventEvidence,
        marker: PhantomData<fn() -> D>,
    },
    PersistenceDiagnosticEpisodeInvalid {
        evidence: DiagnosticEpisodeEvidence,
        marker: PhantomData<fn() -> D>,
    },
    PersistenceDiagnosticSchemaInvalid {
        evidence: DiagnosticEpisodeEvidence,
        marker: PhantomData<fn() -> D>,
    },
    PersistenceSettledStateInconsistent {
        evidence: SettledStateEvidence,
        marker: PhantomData<fn() -> D>,
    },
    PersistenceExecutionDigestMismatch {
        evidence: DigestMismatchEvidence,
        marker: PhantomData<fn() -> D>,
    },
    PersistenceObservableDigestMismatch {
        evidence: DigestMismatchEvidence,
        marker: PhantomData<fn() -> D>,
    },
    PersistenceSnapshotDigestMismatch {
        evidence: DigestMismatchEvidence,
        marker: PhantomData<fn() -> D>,
    },
    PersistenceDigestCollision {
        evidence: DigestCollisionEvidence,
        marker: PhantomData<fn() -> D>,
    },
    PersistenceProvenanceMissingPredecessor {
        evidence: PersistenceProvenanceEvidence,
        marker: PhantomData<fn() -> D>,
    },
    PersistenceProvenanceDigestMismatch {
        evidence: PersistenceProvenanceEvidence,
        marker: PhantomData<fn() -> D>,
    },
    PersistenceProvenanceCycle {
        evidence: PersistenceProvenanceEvidence,
        marker: PhantomData<fn() -> D>,
    },
    PersistenceProvenanceInvalidSubject {
        evidence: PersistenceProvenanceEvidence,
        marker: PhantomData<fn() -> D>,
    },
    PersistenceProvenanceInvalidRole {
        evidence: PersistenceProvenanceEvidence,
        marker: PhantomData<fn() -> D>,
    },
    PersistenceProvenanceIncompleteRootClosure {
        evidence: PersistenceProvenanceEvidence,
        marker: PhantomData<fn() -> D>,
    },
    PersistenceProvenanceConflictingRecord {
        evidence: PersistenceProvenanceEvidence,
        marker: PhantomData<fn() -> D>,
    },
    PersistenceProvenanceFalseCheckpoint {
        evidence: PersistenceProvenanceEvidence,
        marker: PhantomData<fn() -> D>,
    },
    StandardModuleExpansionMismatch {
        module_ref: StandardModuleRef,
        detail: String,
        marker: PhantomData<fn() -> D>,
    },
    ReplayStartingExecutionDigestMismatch {
        evidence: ReplayEvidence,
        marker: PhantomData<fn() -> D>,
    },
    ReplayStartingObservableDigestMismatch {
        evidence: ReplayEvidence,
        marker: PhantomData<fn() -> D>,
    },
    ReplayExpectedRevisionMismatch {
        evidence: ReplayEvidence,
        marker: PhantomData<fn() -> D>,
    },
    ReplayRuntimePolicyMismatch {
        evidence: ReplayEvidence,
        marker: PhantomData<fn() -> D>,
    },
    ReplayTimeDomainMismatch {
        evidence: ReplayEvidence,
        marker: PhantomData<fn() -> D>,
    },
    ReplayNetworkFingerprintMismatch {
        evidence: ReplayEvidence,
        marker: PhantomData<fn() -> D>,
    },
    ReplayLogsNotConcatenable {
        evidence: ReplayEvidence,
        marker: PhantomData<fn() -> D>,
    },
    ReplayPatchPreparationDiverged {
        evidence: ReplayEvidence,
        marker: PhantomData<fn() -> D>,
    },
    ReplayResultingExecutionDigestMismatch {
        evidence: ReplayEvidence,
        marker: PhantomData<fn() -> D>,
    },
    ReplayResultingObservableDigestMismatch {
        evidence: ReplayEvidence,
        marker: PhantomData<fn() -> D>,
    },
    ReplayFrameMissing {
        evidence: ReplayEvidence,
        marker: PhantomData<fn() -> D>,
    },
    ReplayFrameReordered {
        evidence: ReplayEvidence,
        marker: PhantomData<fn() -> D>,
    },
    ReplayFrameDuplicated {
        evidence: ReplayEvidence,
        marker: PhantomData<fn() -> D>,
    },
    ReconfigurationForeignArtifact {
        marker: PhantomData<fn() -> D>,
    },
    ReconfigurationDuplicateOperation {
        marker: PhantomData<fn() -> D>,
    },
    ReconfigurationConflictingEdit {
        marker: PhantomData<fn() -> D>,
    },
    ReconfigurationInvalidReplacementKey {
        marker: PhantomData<fn() -> D>,
    },
    ReconfigurationInvalidReassociation {
        source: SubjectRef,
        target: SubjectRef,
        marker: PhantomData<fn() -> D>,
    },
    ReconfigurationContradictoryHierarchy {
        marker: PhantomData<fn() -> D>,
    },
    ReconfigurationBaseFingerprintMismatch {
        marker: PhantomData<fn() -> D>,
    },
    ReconfigurationBaseRevisionMismatch {
        expected: NetworkRevision,
        actual: NetworkRevision,
        marker: PhantomData<fn() -> D>,
    },
    ReconfigurationUnknownBaseSubject {
        marker: PhantomData<fn() -> D>,
    },
    ReconfigurationNonInjectiveReassociation {
        marker: PhantomData<fn() -> D>,
    },
    ReconfigurationIncompatibleMigrationDirective {
        marker: PhantomData<fn() -> D>,
    },
    ReconfigurationIncompleteTemporalMigrationPolicy {
        marker: PhantomData<fn() -> D>,
    },
    ReconfigurationUnsupportedCrossKindMigration {
        marker: PhantomData<fn() -> D>,
    },
    ReconfigurationAmbiguousEventMigration {
        marker: PhantomData<fn() -> D>,
    },
    ReconfigurationConflictingMigratedTransitions {
        evidence: PendingEventEvidence,
        marker: PhantomData<fn() -> D>,
    },
    ReconfigurationInvalidTargetInputSchema {
        marker: PhantomData<fn() -> D>,
    },
    ReconfigurationConditionalSemanticLoss {
        fact: &'static str,
        rule: &'static str,
        marker: PhantomData<fn() -> D>,
    },
    ReconfigurationUnavoidableSemanticLoss {
        fact: &'static str,
        rule: &'static str,
        marker: PhantomData<fn() -> D>,
    },
    ReconfigurationEmptyPatch {
        marker: PhantomData<fn() -> D>,
    },
    ReconfigurationStalePreparedPatch {
        evidence: StaleArtifactEvidence,
        marker: PhantomData<fn() -> D>,
    },
    ReconfigurationTargetInputSchemaMismatch {
        evidence: InputSchemaEvidence,
        marker: PhantomData<fn() -> D>,
    },
    ReconfigurationStateMigrationRejected {
        evidence: MigrationEvidence,
        marker: PhantomData<fn() -> D>,
    },
    ReconfigurationPendingEventMigrationRejected {
        evidence: MigrationEvidence,
        marker: PhantomData<fn() -> D>,
    },
    ReconfigurationRequirePreserveFailed {
        evidence: MigrationEvidence,
        marker: PhantomData<fn() -> D>,
    },
    ReconfigurationEpisodeMigrationRejected {
        evidence: DiagnosticEpisodeEvidence,
        marker: PhantomData<fn() -> D>,
    },
    ReconfigurationProvenanceMigrationRejected {
        evidence: ProvenanceEvidence,
        marker: PhantomData<fn() -> D>,
    },
    ReconfigurationStateLossRejected {
        evidence: SemanticLossEvidence,
        marker: PhantomData<fn() -> D>,
    },
    StandardModuleNoncanonicalInternalEdit {
        instance: ModuleInstanceKey,
        subject: SubjectRef,
        marker: PhantomData<fn() -> D>,
    },
}

impl<D> ProblemEvidence<D> {
    #[must_use]
    pub fn standard_module_unknown_id(module_ref: StandardModuleRef) -> Self {
        Self::StandardModuleUnknownId {
            module_ref,
            marker: PhantomData,
        }
    }
    #[must_use]
    pub fn standard_module_unsupported_version(module_ref: StandardModuleRef) -> Self {
        Self::StandardModuleUnsupportedVersion {
            module_ref,
            marker: PhantomData,
        }
    }
    #[must_use]
    pub fn standard_module_missing_parameter(
        module_ref: StandardModuleRef,
        parameter: StandardParameterKey,
    ) -> Self {
        Self::StandardModuleMissingParameter {
            module_ref,
            parameter,
            marker: PhantomData,
        }
    }
    #[must_use]
    pub fn standard_module_unexpected_parameter(
        module_ref: StandardModuleRef,
        parameter: StandardParameterKey,
    ) -> Self {
        Self::StandardModuleUnexpectedParameter {
            module_ref,
            parameter,
            marker: PhantomData,
        }
    }
    #[must_use]
    pub fn standard_module_parameter_kind_mismatch(
        module_ref: StandardModuleRef,
        parameter: StandardParameterKey,
        expected: StandardParameterKind,
        encountered: StandardParameterKind,
    ) -> Self {
        Self::StandardModuleParameterKindMismatch {
            module_ref,
            parameter,
            expected,
            encountered,
            marker: PhantomData,
        }
    }
    #[must_use]
    pub fn standard_module_invalid_parameter(
        module_ref: StandardModuleRef,
        parameter: StandardParameterKey,
    ) -> Self {
        Self::StandardModuleInvalidParameter {
            module_ref,
            parameter,
            marker: PhantomData,
        }
    }
    #[must_use]
    pub fn standard_module_interface_mismatch(
        module_ref: StandardModuleRef,
        inputs: Vec<AnyModuleInputKey>,
    ) -> Self {
        Self::StandardModuleInterfaceMismatch {
            module_ref,
            inputs,
            marker: PhantomData,
        }
    }
    #[must_use]
    pub fn standard_module_internal_key_collision(
        module_ref: StandardModuleRef,
        key: u128,
    ) -> Self {
        Self::StandardModuleInternalKeyCollision {
            module_ref,
            key,
            marker: PhantomData,
        }
    }
    #[must_use]
    pub fn standard_module_catalogue_invariant(
        module_ref: StandardModuleRef,
        detail: impl Into<String>,
    ) -> Self {
        Self::StandardModuleCatalogueInvariant {
            module_ref,
            detail: detail.into(),
            marker: PhantomData,
        }
    }
    #[must_use]
    pub fn standard_module_empty_variadic(
        module_ref: StandardModuleRef,
        inputs: Vec<ModuleInputKey<crate::signal::Level>>,
    ) -> Self {
        Self::StandardModuleEmptyVariadic {
            module_ref,
            inputs,
            marker: PhantomData,
        }
    }
    #[must_use]
    pub fn standard_module_unary_degenerate(
        module_ref: StandardModuleRef,
        inputs: Vec<ModuleInputKey<crate::signal::Level>>,
    ) -> Self {
        Self::StandardModuleUnaryDegenerate {
            module_ref,
            inputs,
            marker: PhantomData,
        }
    }
    #[must_use]
    pub fn standard_module_impossible_threshold(
        module_ref: StandardModuleRef,
        arity: usize,
        threshold: u64,
    ) -> Self {
        Self::StandardModuleImpossibleThreshold {
            module_ref,
            arity,
            threshold,
            marker: PhantomData,
        }
    }
    #[must_use]
    pub fn standard_module_constant_result(
        module_ref: StandardModuleRef,
        result: LogicLevel,
    ) -> Self {
        Self::StandardModuleConstantResult {
            module_ref,
            result,
            marker: PhantomData,
        }
    }
    #[must_use]
    pub fn standard_module_duplicate_source(
        module_ref: StandardModuleRef,
        source: SubjectRef,
        inputs: Vec<ModuleInputKey<crate::signal::Level>>,
    ) -> Self {
        Self::StandardModuleDuplicateSource {
            module_ref,
            source,
            inputs,
            marker: PhantomData,
        }
    }

    /// Evidence for a duplicate structural key condition.
    #[must_use]
    pub fn duplicate_key(key: SubjectRef, claims: Vec<DuplicateClaim>) -> Self {
        Self::ValidationDuplicateKey {
            key,
            claims,
            marker: PhantomData,
        }
    }

    /// Evidence for a missing node reference.
    #[must_use]
    pub fn missing_node(missing: NodeKey) -> Self {
        Self::ValidationMissingNode {
            missing,
            marker: PhantomData,
        }
    }

    /// Evidence for a missing input or output port reference.
    #[must_use]
    pub fn missing_port(missing: SubjectRef, expected_kind: SignalKind) -> Self {
        Self::ValidationMissingPort {
            missing,
            expected_kind,
            marker: PhantomData,
        }
    }

    /// Evidence for a missing external endpoint reference.
    #[must_use]
    pub fn missing_endpoint(missing: SubjectRef, expected_kind: SignalKind) -> Self {
        Self::ValidationMissingEndpoint {
            missing,
            expected_kind,
            marker: PhantomData,
        }
    }

    /// Evidence for an invalid connection direction.
    #[must_use]
    pub fn invalid_direction(source: SubjectRef, target: SubjectRef) -> Self {
        Self::ValidationInvalidDirection {
            source,
            target,
            marker: PhantomData,
        }
    }

    /// Evidence for incompatible connection signal kinds.
    #[must_use]
    pub fn signal_kind_mismatch(
        source: SubjectRef,
        target: SubjectRef,
        source_kind: SignalKind,
        target_kind: SignalKind,
    ) -> Self {
        Self::ValidationSignalKindMismatch {
            source,
            target,
            source_kind,
            target_kind,
            marker: PhantomData,
        }
    }

    /// Evidence for a target input's conflicting drivers.
    #[must_use]
    pub fn unsupported_multiple_drivers(drivers: Vec<SubjectRef>) -> Self {
        Self::ValidationUnsupportedMultipleDrivers {
            drivers,
            marker: PhantomData,
        }
    }

    /// Evidence for an absent required input.
    #[must_use]
    pub fn missing_required_input(required: SubjectRef, expected_kind: SignalKind) -> Self {
        Self::ValidationMissingRequiredInput {
            required: RequiredInputRef::Port(required),
            expected_kind,
            marker: PhantomData,
        }
    }

    /// Evidence for an absent fixed semantic input role.
    #[must_use]
    pub fn missing_required_input_role(required: InputPortRole, expected_kind: SignalKind) -> Self {
        Self::ValidationMissingRequiredInput {
            required: RequiredInputRef::Role(required),
            expected_kind,
            marker: PhantomData,
        }
    }

    /// Evidence for a fixed port-group arity mismatch.
    #[must_use]
    pub fn invalid_fixed_arity(
        role: FixedArityRole,
        ports: Vec<SubjectRef>,
        expected: usize,
        encountered: usize,
    ) -> Self {
        Self::ValidationInvalidFixedArity {
            role,
            ports,
            expected,
            encountered,
            marker: PhantomData,
        }
    }

    /// Evidence for a variadic port group outside its semantic arity domain.
    #[must_use]
    pub fn invalid_variadic_arity(
        ports: Vec<SubjectRef>,
        minimum: usize,
        encountered: usize,
    ) -> Self {
        Self::ValidationInvalidVariadicArity {
            ports,
            minimum,
            encountered,
            marker: PhantomData,
        }
    }

    /// Evidence that one source feeds several stable ports of a variadic node.
    #[must_use]
    pub fn duplicate_source(source: SubjectRef, ports: Vec<SubjectRef>) -> Self {
        Self::ValidationDuplicateSource {
            source,
            ports,
            marker: PhantomData,
        }
    }

    /// Evidence that a total variadic node is using its empty law.
    #[must_use]
    pub fn empty_variadic_node(ports: Vec<SubjectRef>) -> Self {
        Self::ValidationEmptyVariadicNode {
            ports,
            marker: PhantomData,
        }
    }

    /// Evidence that a variadic node reduces to its unary law.
    #[must_use]
    pub fn unary_degenerate_node(ports: Vec<SubjectRef>) -> Self {
        Self::ValidationUnaryDegenerateNode {
            ports,
            marker: PhantomData,
        }
    }

    /// Evidence that node parameters and arity make its result constant.
    #[must_use]
    pub fn constant_result_node(inputs: Vec<SubjectRef>, result: LogicLevel) -> Self {
        Self::ValidationConstantResultNode {
            inputs,
            result,
            marker: PhantomData,
        }
    }

    /// Evidence for one cyclic current-reaction SCC.
    #[must_use]
    pub fn current_reaction_cycle(
        members: Vec<ReactionMemberRef>,
        witness: Vec<CurrentReactionCycleStep>,
    ) -> Self {
        Self::ValidationCurrentReactionCycle {
            members,
            witness,
            marker: PhantomData,
        }
    }

    /// Evidence for a missing, duplicate, unknown, wrong-kind, or invalid module binding.
    #[must_use]
    pub fn invalid_module_binding(
        instance: ModuleInstanceKey,
        input: AnyModuleInputKey,
        sources: Vec<SubjectRef>,
    ) -> Self {
        let issue = if sources.is_empty() {
            ModuleBindingIssue::Missing
        } else if sources.len() > 1 {
            ModuleBindingIssue::Duplicate
        } else {
            ModuleBindingIssue::Invalid
        };
        Self::ValidationInvalidModuleBinding {
            instance,
            input,
            issue,
            sources,
            marker: PhantomData,
        }
    }

    /// Evidence for one missing module-instance parent.
    #[must_use]
    pub fn malformed_hierarchy(instance: ModuleInstanceKey, parent: ModuleInstanceKey) -> Self {
        Self::ValidationMalformedHierarchy {
            instance,
            parent,
            marker: PhantomData,
        }
    }

    /// Evidence for one cyclic module-containment component.
    #[must_use]
    pub fn hierarchy_cycle(instances: Vec<ModuleInstanceKey>) -> Self {
        Self::ValidationHierarchyCycle {
            instances,
            marker: PhantomData,
        }
    }

    fn canonicalize(&mut self) {
        let canonicalize = |subjects: &mut Vec<SubjectRef>| {
            subjects.sort_by(SubjectRef::cmp_canonical);
            subjects.dedup();
        };
        match self {
            // SPEC: docs/specs/contracts/diagnostic-collections.yaml
            // "initial-duplicate-key" — claims are a multiset, not a set.
            Self::ValidationDuplicateKey { claims, .. } => claims.sort(),
            Self::ValidationUnsupportedMultipleDrivers { drivers, .. } => canonicalize(drivers),
            Self::ValidationInvalidFixedArity { ports, .. }
            | Self::ValidationInvalidVariadicArity { ports, .. } => canonicalize(ports),
            Self::ValidationDuplicateSource { ports, .. }
            | Self::ValidationEmptyVariadicNode { ports, .. }
            | Self::ValidationUnaryDegenerateNode { ports, .. }
            | Self::ValidationConstantResultNode { inputs: ports, .. } => canonicalize(ports),
            Self::ValidationCurrentReactionCycle { members, .. } => {
                members.sort();
                members.dedup();
            }
            Self::ValidationInvalidModuleBinding { sources, .. } => canonicalize(sources),
            Self::ValidationHierarchyCycle { instances, .. } => {
                instances.sort();
                instances.dedup();
            }
            Self::StandardModuleInterfaceMismatch { inputs, .. } => {
                inputs.sort();
                inputs.dedup();
            }
            Self::StandardModuleEmptyVariadic { inputs, .. }
            | Self::StandardModuleUnaryDegenerate { inputs, .. }
            | Self::StandardModuleDuplicateSource { inputs, .. } => {
                inputs.sort();
                inputs.dedup();
            }
            Self::InputMissingRequiredLevel { evidence, .. } => {
                evidence.missing.sort();
                evidence.missing.dedup();
            }
            _ => {}
        }
    }
}

// SPEC: docs/specs/contracts/diagnostic-problem-model.yaml
// "authoritative-registry" — code spelling, classification, delivery, and
// evidence association are declared together so they cannot drift apart.
macro_rules! runtime_occurrence_delivery {
    () => {
        false
    };
    ($allowed:expr) => {
        $allowed
    };
}

macro_rules! opening_diagnostic_registry {
    ($( $code:ident, $evidence:pat, $spelling:literal, $severity:ident, $responsibility:ident, $schema:ident, $report_finding:expr, $operation_failure:expr, $internal_defect:expr $(, $runtime_occurrence:expr $(, $persistent_episode:expr)?)?; )+) => {
        impl DiagnosticCode {
            /// Every code implemented by the current catalogue slice.
            pub const ALL: &'static [Self] = &[$(Self::$code,)+];

            /// Returns the stable dotted spelling of this catalogue entry.
            #[must_use]
            pub const fn as_str(self) -> &'static str {
                self.specification().code
            }

            /// Returns the catalogue-fixed severity of this condition.
            #[must_use]
            pub const fn severity(self) -> Severity {
                self.specification().severity
            }

            /// Returns the catalogue-fixed responsibility of this condition.
            #[must_use]
            pub const fn responsibility(self) -> Responsibility {
                self.specification().responsibility
            }

            /// Returns the exact evidence family fixed by the catalogue.
            #[must_use]
            pub const fn evidence_schema(self) -> EvidenceSchema {
                self.specification().evidence_schema
            }

            /// Returns whether the catalogue permits this delivery form.
            #[must_use]
            pub const fn allows_delivery(self, delivery: ProblemDelivery) -> bool {
                let allowed = self.specification().delivery;
                match delivery {
                    ProblemDelivery::ReportFinding => allowed.report_finding,
                    ProblemDelivery::OperationFailure => allowed.operation_failure,
                    ProblemDelivery::RuntimeOccurrence => allowed.runtime_occurrence,
                    ProblemDelivery::PersistentEpisode => allowed.persistent_episode,
                    ProblemDelivery::InternalDefect => allowed.internal_defect,
                }
            }

            const fn specification(self) -> CodeSpecification {
                match self {
                    $(Self::$code => CodeSpecification {
                        code: $spelling,
                        severity: Severity::$severity,
                        responsibility: Responsibility::$responsibility,
                        evidence_schema: EvidenceSchema::$schema,
                        delivery: DeliverySet {
                            report_finding: $report_finding,
                            operation_failure: $operation_failure,
                            runtime_occurrence: runtime_occurrence_delivery!($($runtime_occurrence)?),
                            persistent_episode: runtime_occurrence_delivery!($($($persistent_episode)?)?),
                            internal_defect: $internal_defect,
                        },
                    },)+
                }
            }
        }

        impl<D> ProblemEvidence<D> {
            /// Returns the one catalogue code paired with this exact evidence variant.
            #[must_use]
            pub const fn code(&self) -> DiagnosticCode {
                match self {
                    $($evidence => DiagnosticCode::$code,)+
                }
            }
        }
    };
}

opening_diagnostic_registry! {
    AuthoringForeignSignal, Self::AuthoringForeignSignal { .. }, "authoring.foreign_signal", Error, CallerInput, ForeignArtifact, false, true, false;
    ValidationDuplicateKey, Self::ValidationDuplicateKey { .. }, "validation.duplicate_key", Error, CallerInput, KeyConflict, true, true, false;
    ValidationMissingNode, Self::ValidationMissingNode { .. }, "validation.missing_node", Error, CallerInput, MissingReference, true, false, false;
    ValidationMissingPort, Self::ValidationMissingPort { .. }, "validation.missing_port", Error, CallerInput, MissingReference, true, false, false;
    ValidationMissingEndpoint, Self::ValidationMissingEndpoint { .. }, "validation.missing_endpoint", Error, CallerInput, MissingReference, true, true, false;
    ValidationInvalidDirection, Self::ValidationInvalidDirection { .. }, "validation.invalid_direction", Error, CallerInput, Direction, true, true, false;
    ValidationSignalKindMismatch, Self::ValidationSignalKindMismatch { .. }, "validation.signal_kind_mismatch", Error, CallerInput, KindMismatch, true, true, false;
    ValidationInvalidParameter, Self::ValidationInvalidParameter { .. }, "validation.invalid_parameter", Error, CallerInput, Parameter, true, true, false;
    ValidationUnsupportedMultipleDrivers, Self::ValidationUnsupportedMultipleDrivers { .. }, "validation.unsupported_multiple_drivers", Error, CallerInput, DriverConflict, true, false, false;
    ValidationMissingRequiredInput, Self::ValidationMissingRequiredInput { .. }, "validation.missing_required_input", Error, CallerInput, MissingReference, true, false, false;
    ValidationInvalidFixedArity, Self::ValidationInvalidFixedArity { .. }, "validation.invalid_fixed_arity", Error, CallerInput, Arity, true, false, false;
    ValidationInvalidVariadicArity, Self::ValidationInvalidVariadicArity { .. }, "validation.invalid_variadic_arity", Error, CallerInput, Arity, true, false, false;
    ValidationDuplicateSource, Self::ValidationDuplicateSource { .. }, "validation.duplicate_source", Warning, CallerInput, StaticQuality, true, false, false;
    ValidationEmptyVariadicNode, Self::ValidationEmptyVariadicNode { .. }, "validation.empty_variadic_node", Warning, Advisory, StaticQuality, true, false, false;
    ValidationUnaryDegenerateNode, Self::ValidationUnaryDegenerateNode { .. }, "validation.unary_degenerate_node", Warning, Advisory, StaticQuality, true, false, false;
    ValidationConstantResultNode, Self::ValidationConstantResultNode { .. }, "validation.constant_result_node", Warning, Advisory, StaticQuality, true, false, false;
    ValidationCurrentReactionCycle, Self::ValidationCurrentReactionCycle { .. }, "validation.current_reaction_cycle", Error, CallerInput, CurrentReactionCycle, true, false, false;
    ValidationInvalidModuleBinding, Self::ValidationInvalidModuleBinding { .. }, "validation.invalid_module_binding", Error, CallerInput, ModuleSchema, true, true, false;
    ValidationMalformedHierarchy, Self::ValidationMalformedHierarchy { .. }, "validation.malformed_hierarchy", Error, CallerInput, Hierarchy, true, true, false;
    ValidationHierarchyCycle, Self::ValidationHierarchyCycle { .. }, "validation.hierarchy_cycle", Error, CallerInput, Hierarchy, true, false, false;
    StandardModuleUnknownId, Self::StandardModuleUnknownId { .. }, "standard_module.unknown_id", Error, UnsupportedFeature, StandardModule, true, true, false;
    StandardModuleUnsupportedVersion, Self::StandardModuleUnsupportedVersion { .. }, "standard_module.unsupported_version", Error, Compatibility, StandardModule, true, true, false;
    StandardModuleMissingParameter, Self::StandardModuleMissingParameter { .. }, "standard_module.missing_parameter", Error, CallerInput, StandardModule, true, true, false;
    StandardModuleUnexpectedParameter, Self::StandardModuleUnexpectedParameter { .. }, "standard_module.unexpected_parameter", Error, CallerInput, StandardModule, true, true, false;
    StandardModuleParameterKindMismatch, Self::StandardModuleParameterKindMismatch { .. }, "standard_module.parameter_kind_mismatch", Error, CallerInput, StandardModule, true, true, false;
    StandardModuleInvalidParameter, Self::StandardModuleInvalidParameter { .. }, "standard_module.invalid_parameter", Error, CallerInput, StandardModule, true, true, false;
    StandardModuleInterfaceMismatch, Self::StandardModuleInterfaceMismatch { .. }, "standard_module.interface_mismatch", Error, CallerInput, StandardModule, true, true, false;
    StandardModuleInternalKeyCollision, Self::StandardModuleInternalKeyCollision { .. }, "standard_module.internal_key_collision", Error, LibraryDefect, StandardModule, false, true, true;
    StandardModuleCatalogueInvariant, Self::StandardModuleCatalogueInvariant { .. }, "standard_module.catalogue_invariant", Error, LibraryDefect, StandardModule, false, true, true;
    StandardModuleEmptyVariadic, Self::StandardModuleEmptyVariadic { .. }, "standard_module.empty_variadic", Warning, Advisory, StandardModule, true, false, false;
    StandardModuleUnaryDegenerate, Self::StandardModuleUnaryDegenerate { .. }, "standard_module.unary_degenerate", Warning, Advisory, StandardModule, true, false, false;
    StandardModuleImpossibleThreshold, Self::StandardModuleImpossibleThreshold { .. }, "standard_module.impossible_threshold", Warning, Advisory, StandardModule, true, false, false;
    StandardModuleConstantResult, Self::StandardModuleConstantResult { .. }, "standard_module.constant_result", Warning, Advisory, StandardModule, true, false, false;
    StandardModuleDuplicateSource, Self::StandardModuleDuplicateSource { .. }, "standard_module.duplicate_source", Warning, CallerInput, StandardModule, true, false, false;
    BindingUnknownEndpoint, Self::BindingUnknownEndpoint { .. }, "binding.unknown_endpoint", Error, CallerInput, Binding, true, true, false;
    BindingWrongSignalKind, Self::BindingWrongSignalKind { .. }, "binding.wrong_signal_kind", Error, CallerInput, Binding, true, true, false;
    BindingDuplicateEndpoint, Self::BindingDuplicateEndpoint { .. }, "binding.duplicate_endpoint", Error, CallerInput, Binding, true, true, false;
    BindingDuplicateExternalKey, Self::BindingDuplicateExternalKey { .. }, "binding.duplicate_external_key", Error, CallerInput, Binding, true, true, false;
    BindingAmbiguousExternalKey, Self::BindingAmbiguousExternalKey { .. }, "binding.ambiguous_external_key", Error, CallerInput, Binding, true, true, false;
    BindingMissingRequiredBinding, Self::BindingMissingRequiredBinding { .. }, "binding.missing_required_binding", Error, CallerInput, Binding, true, true, false;
    BindingWrongNetwork, Self::BindingWrongNetwork { .. }, "binding.wrong_network", Error, Compatibility, Binding, false, true, false;
    BindingStaleSchema, Self::BindingStaleSchema { .. }, "binding.stale_schema", Error, Compatibility, Binding, false, true, false;
    LifecycleNotInitialized, Self::LifecycleNotInitialized { .. }, "lifecycle.not_initialized", Error, CallerInput, Lifecycle, false, true, false;
    LifecycleAlreadyInitialized, Self::LifecycleAlreadyInitialized { .. }, "lifecycle.already_initialized", Error, CallerInput, Lifecycle, false, true, false;
    LifecycleDeltaBeforeInitialization, Self::LifecycleDeltaBeforeInitialization { .. }, "lifecycle.delta_before_initialization", Error, CallerInput, Lifecycle, false, true, false;
    RuntimeStaleRevision, Self::RuntimeStaleRevision { .. }, "runtime.stale_revision", Error, Compatibility, RevisionMismatch, false, true, false;
    RuntimeStaleExecutionState, Self::RuntimeStaleExecutionState { .. }, "runtime.stale_execution_state", Error, Compatibility, DigestMismatch, false, true, false;
    RuntimeTimeRegression, Self::RuntimeTimeRegression { .. }, "runtime.time_regression", Error, CallerInput, Time, false, true, false;
    RuntimeReactionOrderOverflow, Self::RuntimeReactionOrderOverflow { .. }, "runtime.reaction_order_overflow", Error, SemanticRejection, Time, false, true, false;
    RuntimeTimeOverflow, Self::RuntimeTimeOverflow { .. }, "runtime.time_overflow", Error, SemanticRejection, Time, false, true, false;
    RuntimeInvalidTimeSubtraction, Self::RuntimeInvalidTimeSubtraction { .. }, "runtime.invalid_time_subtraction", Error, CallerInput, Time, false, true, false;
    RuntimeZeroSpanNotAllowed, Self::RuntimeZeroSpanNotAllowed { .. }, "runtime.zero_span_not_allowed", Error, CallerInput, Parameter, false, true, false;
    RuntimePulseCountOverflow, Self::RuntimePulseCountOverflow { .. }, "runtime.pulse_count_overflow", Error, SemanticRejection, Parameter, false, true, false;
    RuntimePolicyMissingLimit, Self::RuntimePolicyMissingLimit { .. }, "runtime.policy_missing_limit", Error, CallerInput, Parameter, false, true, false;
    RuntimePolicyInvalidLimit, Self::RuntimePolicyInvalidLimit { .. }, "runtime.policy_invalid_limit", Error, CallerInput, Parameter, false, true, false;
    RuntimeBudgetExceeded, Self::RuntimeBudgetExceeded { .. }, "runtime.budget_exceeded", Error, ResourceLimit, Budget, false, true, false;
    RuntimePulseLatchConflictRetained, Self::RuntimePulseLatchConflictRetained { .. }, "runtime.pulse_latch_conflict_retained", Warning, Advisory, Conflict, false, false, false, true;
    RuntimePulseLatchConflictRejected, Self::RuntimePulseLatchConflictRejected { .. }, "runtime.pulse_latch_conflict_rejected", Error, SemanticRejection, Conflict, false, true, false;
    RuntimeLevelLatchConflictRetained, Self::RuntimeLevelLatchConflictRetained { .. }, "runtime.level_latch_conflict_retained", Warning, Advisory, Conflict, false, false, false, false, true;
    RuntimeLevelLatchConflictRejected, Self::RuntimeLevelLatchConflictRejected { .. }, "runtime.level_latch_conflict_rejected", Error, SemanticRejection, Conflict, false, true, false;
    InputUnknownEndpoint, Self::InputUnknownEndpoint { .. }, "input.unknown_endpoint", Error, CallerInput, InputObservation, false, true, false;
    InputWrongSignalKind, Self::InputWrongSignalKind { .. }, "input.wrong_signal_kind", Error, CallerInput, InputObservation, false, true, false;
    InputDuplicateObservation, Self::InputDuplicateObservation { .. }, "input.duplicate_observation", Error, CallerInput, InputObservation, false, true, false;
    InputConflictingObservation, Self::InputConflictingObservation { .. }, "input.conflicting_observation", Error, CallerInput, InputObservation, false, true, false;
    InputMissingRequiredLevel, Self::InputMissingRequiredLevel { .. }, "input.missing_required_level", Error, CallerInput, InputObservation, false, true, false;
    InputWrongNetwork, Self::InputWrongNetwork { .. }, "input.wrong_network", Error, Compatibility, InputSchema, false, true, false;
    InputForeignSchema, Self::InputForeignSchema { .. }, "input.foreign_schema", Error, Compatibility, InputSchema, false, true, false;
    InputStaleSchema, Self::InputStaleSchema { .. }, "input.stale_schema", Error, Compatibility, InputSchema, false, true, false;
    InspectionUnknownSubject, Self::InspectionUnknownSubject { .. }, "inspection.unknown_subject", Error, CallerInput, MissingReference, false, true, false;
    InspectionWrongSubjectKind, Self::InspectionWrongSubjectKind { .. }, "inspection.wrong_subject_kind", Error, CallerInput, KindMismatch, false, true, false;
    InspectionPendingEventNotFound, Self::InspectionPendingEventNotFound { .. }, "inspection.pending_event_not_found", Error, CallerInput, PendingEvent, false, true, false;
    ExplanationUnknownSubject, Self::ExplanationUnknownSubject { .. }, "explanation.unknown_subject", Error, CallerInput, MissingReference, false, true, false;
    ExplanationForeignCause, Self::ExplanationForeignCause { .. }, "explanation.foreign_cause", Error, Compatibility, Provenance, false, true, false;
    ExplanationInvalidCause, Self::ExplanationInvalidCause { .. }, "explanation.invalid_cause", Error, CallerInput, Provenance, false, true, false;
    InternalDiagnosticEvidenceConflict, Self::InternalDiagnosticEvidenceConflict { .. }, "internal.diagnostic_evidence_conflict", Error, LibraryDefect, InternalInvariant, false, false, true;
    PersistenceInvalidPrefix, Self::PersistenceInvalidPrefix { .. }, "persistence.invalid_prefix", Error, CorruptData, CanonicalEncoding, false, true, false;
    PersistenceTruncatedArtifact, Self::PersistenceTruncatedArtifact { .. }, "persistence.truncated_artifact", Error, CorruptData, CanonicalEncoding, false, true, false;
    PersistenceTrailingBytes, Self::PersistenceTrailingBytes { .. }, "persistence.trailing_bytes", Error, CorruptData, CanonicalEncoding, false, true, false;
    PersistenceNoncanonicalEncoding, Self::PersistenceNoncanonicalEncoding { .. }, "persistence.noncanonical_encoding", Error, CorruptData, CanonicalEncoding, false, true, false;
    PersistenceMalformedEnvelope, Self::PersistenceMalformedEnvelope { .. }, "persistence.malformed_envelope", Error, CorruptData, CanonicalEncoding, false, true, false;
    PersistenceUnknownArtifactKind, Self::PersistenceUnknownArtifactKind { .. }, "persistence.unknown_artifact_kind", Error, UnsupportedFeature, VersionCompatibility, false, true, false;
    PersistenceUnknownSchemaField, Self::PersistenceUnknownSchemaField { .. }, "persistence.unknown_schema_field", Error, Compatibility, VersionCompatibility, false, true, false;
    PersistenceUnknownSchemaVariant, Self::PersistenceUnknownSchemaVariant { .. }, "persistence.unknown_schema_variant", Error, Compatibility, VersionCompatibility, false, true, false;
    PersistenceIntegrityDigestMismatch, Self::PersistenceIntegrityDigestMismatch { .. }, "persistence.integrity_digest_mismatch", Error, CorruptData, DigestMismatch, false, true, false;
    PersistenceDecodeLimitExceeded, Self::PersistenceDecodeLimitExceeded { .. }, "persistence.decode_limit_exceeded", Error, ResourceLimit, Budget, false, true, false;
    PersistenceUnsupportedVersion, Self::PersistenceUnsupportedVersion { .. }, "persistence.unsupported_version", Error, Compatibility, VersionCompatibility, false, true, false;
    PersistenceWrongTimeDomain, Self::PersistenceWrongTimeDomain { .. }, "persistence.wrong_time_domain", Error, Compatibility, ArtifactIdentity, false, true, false;
    PersistenceNetworkIdentityMismatch, Self::PersistenceNetworkIdentityMismatch { .. }, "persistence.network_identity_mismatch", Error, Compatibility, ArtifactIdentity, false, true, false;
    PersistenceFingerprintMismatch, Self::PersistenceFingerprintMismatch { .. }, "persistence.fingerprint_mismatch", Error, Compatibility, ArtifactIdentity, false, true, false;
    PersistenceTopologyRevisionMismatch, Self::PersistenceTopologyRevisionMismatch { .. }, "persistence.topology_revision_mismatch", Error, Compatibility, ArtifactIdentity, false, true, false;
    PersistenceRuntimePolicyMismatch, Self::PersistenceRuntimePolicyMismatch { .. }, "persistence.runtime_policy_mismatch", Error, Compatibility, ArtifactIdentity, false, true, false;
    PersistenceLifecycleShapeInvalid, Self::PersistenceLifecycleShapeInvalid { .. }, "persistence.lifecycle_shape_invalid", Error, CorruptData, Parameter, false, true, false;
    PersistenceStateSchemaMismatch, Self::PersistenceStateSchemaMismatch { .. }, "persistence.state_schema_mismatch", Error, Compatibility, StateSchema, false, true, false;
    PersistenceUnknownSubject, Self::PersistenceUnknownSubject { .. }, "persistence.unknown_subject", Error, CorruptData, MissingReference, false, true, false;
    PersistencePendingEventInvalid, Self::PersistencePendingEventInvalid { .. }, "persistence.pending_event_invalid", Error, CorruptData, PendingEvent, false, true, false;
    PersistenceEventIdentityStateInvalid, Self::PersistenceEventIdentityStateInvalid { .. }, "persistence.event_identity_state_invalid", Error, CorruptData, PendingEvent, false, true, false;
    PersistenceDiagnosticEpisodeInvalid, Self::PersistenceDiagnosticEpisodeInvalid { .. }, "persistence.diagnostic_episode_invalid", Error, CorruptData, DiagnosticEpisode, false, true, false;
    PersistenceDiagnosticSchemaInvalid, Self::PersistenceDiagnosticSchemaInvalid { .. }, "persistence.diagnostic_schema_invalid", Error, CorruptData, DiagnosticEpisode, false, true, false;
    PersistenceSettledStateInconsistent, Self::PersistenceSettledStateInconsistent { .. }, "persistence.settled_state_inconsistent", Error, CorruptData, InternalInvariant, false, true, false;
    PersistenceExecutionDigestMismatch, Self::PersistenceExecutionDigestMismatch { .. }, "persistence.execution_digest_mismatch", Error, CorruptData, DigestMismatch, false, true, false;
    PersistenceObservableDigestMismatch, Self::PersistenceObservableDigestMismatch { .. }, "persistence.observable_digest_mismatch", Error, CorruptData, DigestMismatch, false, true, false;
    PersistenceSnapshotDigestMismatch, Self::PersistenceSnapshotDigestMismatch { .. }, "persistence.snapshot_digest_mismatch", Error, CorruptData, DigestMismatch, false, true, false;
    PersistenceDigestCollision, Self::PersistenceDigestCollision { .. }, "persistence.digest_collision", Error, CorruptData, DigestCollision, false, true, true;
    PersistenceProvenanceMissingPredecessor, Self::PersistenceProvenanceMissingPredecessor { .. }, "persistence.provenance_missing_predecessor", Error, CorruptData, Provenance, false, true, false;
    PersistenceProvenanceDigestMismatch, Self::PersistenceProvenanceDigestMismatch { .. }, "persistence.provenance_digest_mismatch", Error, CorruptData, Provenance, false, true, false;
    PersistenceProvenanceCycle, Self::PersistenceProvenanceCycle { .. }, "persistence.provenance_cycle", Error, CorruptData, Provenance, false, true, false;
    PersistenceProvenanceInvalidSubject, Self::PersistenceProvenanceInvalidSubject { .. }, "persistence.provenance_invalid_subject", Error, CorruptData, Provenance, false, true, false;
    PersistenceProvenanceInvalidRole, Self::PersistenceProvenanceInvalidRole { .. }, "persistence.provenance_invalid_role", Error, CorruptData, Provenance, false, true, false;
    PersistenceProvenanceIncompleteRootClosure, Self::PersistenceProvenanceIncompleteRootClosure { .. }, "persistence.provenance_incomplete_root_closure", Error, CorruptData, Provenance, false, true, false;
    PersistenceProvenanceConflictingRecord, Self::PersistenceProvenanceConflictingRecord { .. }, "persistence.provenance_conflicting_record", Error, CorruptData, Provenance, false, true, false;
    PersistenceProvenanceFalseCheckpoint, Self::PersistenceProvenanceFalseCheckpoint { .. }, "persistence.provenance_false_checkpoint", Error, CorruptData, Provenance, false, true, false;
    StandardModuleExpansionMismatch, Self::StandardModuleExpansionMismatch { .. }, "standard_module.expansion_mismatch", Error, CorruptData, StandardModule, true, true, false;
    ReplayStartingExecutionDigestMismatch, Self::ReplayStartingExecutionDigestMismatch { .. }, "replay.starting_execution_digest_mismatch", Error, Compatibility, Replay, false, true, false;
    ReplayStartingObservableDigestMismatch, Self::ReplayStartingObservableDigestMismatch { .. }, "replay.starting_observable_digest_mismatch", Error, Compatibility, Replay, false, true, false;
    ReplayExpectedRevisionMismatch, Self::ReplayExpectedRevisionMismatch { .. }, "replay.expected_revision_mismatch", Error, Compatibility, Replay, false, true, false;
    ReplayRuntimePolicyMismatch, Self::ReplayRuntimePolicyMismatch { .. }, "replay.runtime_policy_mismatch", Error, Compatibility, Replay, false, true, false;
    ReplayTimeDomainMismatch, Self::ReplayTimeDomainMismatch { .. }, "replay.time_domain_mismatch", Error, Compatibility, Replay, false, true, false;
    ReplayNetworkFingerprintMismatch, Self::ReplayNetworkFingerprintMismatch { .. }, "replay.network_fingerprint_mismatch", Error, Compatibility, Replay, false, true, false;
    ReplayLogsNotConcatenable, Self::ReplayLogsNotConcatenable { .. }, "replay.logs_not_concatenable", Error, Compatibility, Replay, false, true, false;
    ReplayPatchPreparationDiverged, Self::ReplayPatchPreparationDiverged { .. }, "replay.patch_preparation_diverged", Error, Compatibility, Replay, false, true, false;
    ReplayResultingExecutionDigestMismatch, Self::ReplayResultingExecutionDigestMismatch { .. }, "replay.resulting_execution_digest_mismatch", Error, CorruptData, Replay, false, true, false;
    ReplayResultingObservableDigestMismatch, Self::ReplayResultingObservableDigestMismatch { .. }, "replay.resulting_observable_digest_mismatch", Error, CorruptData, Replay, false, true, false;
    ReplayFrameMissing, Self::ReplayFrameMissing { .. }, "replay.frame_missing", Error, CorruptData, Replay, false, true, false;
    ReplayFrameReordered, Self::ReplayFrameReordered { .. }, "replay.frame_reordered", Error, CorruptData, Replay, false, true, false;
    ReplayFrameDuplicated, Self::ReplayFrameDuplicated { .. }, "replay.frame_duplicated", Error, CorruptData, Replay, false, true, false;
    ReconfigurationForeignArtifact, Self::ReconfigurationForeignArtifact { .. }, "reconfiguration.foreign_artifact", Error, CallerInput, ForeignArtifact, false, true, false;
    ReconfigurationDuplicateOperation, Self::ReconfigurationDuplicateOperation { .. }, "reconfiguration.duplicate_operation", Error, CallerInput, PatchEdit, false, true, false;
    ReconfigurationConflictingEdit, Self::ReconfigurationConflictingEdit { .. }, "reconfiguration.conflicting_edit", Error, CallerInput, PatchEdit, false, true, false;
    ReconfigurationInvalidReplacementKey, Self::ReconfigurationInvalidReplacementKey { .. }, "reconfiguration.invalid_replacement_key", Error, CallerInput, PatchEdit, false, true, false;
    ReconfigurationContradictoryHierarchy, Self::ReconfigurationContradictoryHierarchy { .. }, "reconfiguration.contradictory_hierarchy", Error, CallerInput, PatchEdit, false, true, false;
    ReconfigurationInvalidReassociation, Self::ReconfigurationInvalidReassociation { .. }, "reconfiguration.invalid_reassociation", Error, CallerInput, PatchEdit, true, true, false;
    ReconfigurationBaseFingerprintMismatch, Self::ReconfigurationBaseFingerprintMismatch { .. }, "reconfiguration.base_fingerprint_mismatch", Error, Compatibility, ArtifactIdentity, true, true, false;
    ReconfigurationBaseRevisionMismatch, Self::ReconfigurationBaseRevisionMismatch { .. }, "reconfiguration.base_revision_mismatch", Error, Compatibility, RevisionMismatch, true, true, false;
    ReconfigurationUnknownBaseSubject, Self::ReconfigurationUnknownBaseSubject { .. }, "reconfiguration.unknown_base_subject", Error, CallerInput, MissingReference, true, false, false;
    ReconfigurationNonInjectiveReassociation, Self::ReconfigurationNonInjectiveReassociation { .. }, "reconfiguration.non_injective_reassociation", Error, CallerInput, PatchEdit, true, false, false;
    ReconfigurationIncompatibleMigrationDirective, Self::ReconfigurationIncompatibleMigrationDirective { .. }, "reconfiguration.incompatible_migration_directive", Error, CallerInput, Migration, true, false, false;
    ReconfigurationIncompleteTemporalMigrationPolicy, Self::ReconfigurationIncompleteTemporalMigrationPolicy { .. }, "reconfiguration.incomplete_temporal_migration_policy", Error, CallerInput, Migration, true, false, false;
    ReconfigurationUnsupportedCrossKindMigration, Self::ReconfigurationUnsupportedCrossKindMigration { .. }, "reconfiguration.unsupported_cross_kind_migration", Error, UnsupportedFeature, Migration, true, false, false;
    ReconfigurationAmbiguousEventMigration, Self::ReconfigurationAmbiguousEventMigration { .. }, "reconfiguration.ambiguous_event_migration", Error, CallerInput, PendingEvent, true, true, false;
    ReconfigurationConflictingMigratedTransitions, Self::ReconfigurationConflictingMigratedTransitions { .. }, "reconfiguration.conflicting_migrated_transitions", Error, SemanticRejection, PendingEvent, false, true, false;
    ReconfigurationInvalidTargetInputSchema, Self::ReconfigurationInvalidTargetInputSchema { .. }, "reconfiguration.invalid_target_input_schema", Error, CallerInput, InputSchema, true, false, false;
    ReconfigurationConditionalSemanticLoss, Self::ReconfigurationConditionalSemanticLoss { .. }, "reconfiguration.conditional_semantic_loss", Warning, Advisory, SemanticLoss, true, false, false;
    ReconfigurationUnavoidableSemanticLoss, Self::ReconfigurationUnavoidableSemanticLoss { .. }, "reconfiguration.unavoidable_semantic_loss", Warning, Advisory, SemanticLoss, true, false, false;
    ReconfigurationEmptyPatch, Self::ReconfigurationEmptyPatch { .. }, "reconfiguration.empty_patch", Error, CallerInput, PatchEdit, true, false, false;
    ReconfigurationStalePreparedPatch, Self::ReconfigurationStalePreparedPatch { .. }, "reconfiguration.stale_prepared_patch", Error, Compatibility, StaleArtifact, false, true, false;
    ReconfigurationTargetInputSchemaMismatch, Self::ReconfigurationTargetInputSchemaMismatch { .. }, "reconfiguration.target_input_schema_mismatch", Error, Compatibility, InputSchema, false, true, false;
    ReconfigurationStateMigrationRejected, Self::ReconfigurationStateMigrationRejected { .. }, "reconfiguration.state_migration_rejected", Error, SemanticRejection, Migration, false, true, false;
    ReconfigurationPendingEventMigrationRejected, Self::ReconfigurationPendingEventMigrationRejected { .. }, "reconfiguration.pending_event_migration_rejected", Error, SemanticRejection, Migration, false, true, false;
    ReconfigurationRequirePreserveFailed, Self::ReconfigurationRequirePreserveFailed { .. }, "reconfiguration.require_preserve_failed", Error, SemanticRejection, Migration, false, true, false;
    ReconfigurationEpisodeMigrationRejected, Self::ReconfigurationEpisodeMigrationRejected { .. }, "reconfiguration.episode_migration_rejected", Error, SemanticRejection, DiagnosticEpisode, false, true, false;
    ReconfigurationProvenanceMigrationRejected, Self::ReconfigurationProvenanceMigrationRejected { .. }, "reconfiguration.provenance_migration_rejected", Error, SemanticRejection, Provenance, false, true, false;
    ReconfigurationStateLossRejected, Self::ReconfigurationStateLossRejected { .. }, "reconfiguration.state_loss_rejected", Error, SemanticRejection, SemanticLoss, false, true, false;
    StandardModuleNoncanonicalInternalEdit, Self::StandardModuleNoncanonicalInternalEdit { .. }, "standard_module.noncanonical_internal_edit", Error, CallerInput, StandardModule, true, false, false;
}

/// One structured, catalogue-valid problem record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Problem<D> {
    code: DiagnosticCode,
    primary: SubjectRef,
    related: Vec<RelatedSubject>,
    evidence: ProblemEvidence<D>,
    suggestions: Vec<Suggestion>,
}

impl<D> Problem<D> {
    /// Creates a problem only when its code and evidence are the catalogue pair.
    pub(crate) fn new(
        primary: SubjectRef,
        mut related: Vec<RelatedSubject>,
        mut evidence: ProblemEvidence<D>,
    ) -> Self {
        evidence.canonicalize();
        related.sort_by(|left, right| {
            left.role
                .cmp(&right.role)
                .then_with(|| left.subject.cmp_canonical(&right.subject))
        });
        related.dedup_by(|left, right| left.role == right.role && left.subject == right.subject);
        Self {
            code: evidence.code(),
            primary,
            related,
            evidence,
            suggestions: Vec::new(),
        }
    }

    fn evidence_conflict(
        conflicting_code: DiagnosticCode,
        conflicting_primary: SubjectRef,
    ) -> Self {
        Self::new(
            conflicting_primary.clone(),
            Vec::new(),
            ProblemEvidence::InternalDiagnosticEvidenceConflict {
                conflicting_code,
                conflicting_primary,
                marker: PhantomData,
            },
        )
    }
    #[must_use]
    pub const fn code(&self) -> DiagnosticCode {
        self.code
    }
    #[must_use]
    pub const fn severity(&self) -> Severity {
        self.code.specification().severity
    }
    #[must_use]
    pub const fn responsibility(&self) -> Responsibility {
        self.code.specification().responsibility
    }
    #[must_use]
    pub const fn primary(&self) -> &SubjectRef {
        &self.primary
    }
    #[must_use]
    pub fn related(&self) -> &[RelatedSubject] {
        &self.related
    }
    #[must_use]
    pub const fn evidence(&self) -> &ProblemEvidence<D> {
        &self.evidence
    }
    #[must_use]
    pub fn suggestions(&self) -> &[Suggestion] {
        &self.suggestions
    }
}

/// A problem permitted as a report finding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic<D> {
    problem: Problem<D>,
}

impl<D> Diagnostic<D> {
    /// Converts a catalogue-valid report problem into a report finding.
    pub fn new(problem: Problem<D>) -> Result<Self, Box<Problem<D>>> {
        if problem.code.allows_delivery(ProblemDelivery::ReportFinding) {
            Ok(Self { problem })
        } else {
            Err(Box::new(problem))
        }
    }
    #[must_use]
    pub const fn problem(&self) -> &Problem<D> {
        &self.problem
    }
    #[must_use]
    pub fn into_problem(self) -> Problem<D> {
        self.problem
    }
}

/// One catalogue-valid transient runtime condition published by a committed reaction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiagnosticOccurrence<D> {
    problem: Problem<D>,
    at: crate::ReactionStamp<D>,
    revision: NetworkRevision,
}

impl<D> DiagnosticOccurrence<D> {
    /// Creates an occurrence only for a code whose catalogue delivery permits it.
    pub fn new(
        problem: Problem<D>,
        at: crate::ReactionStamp<D>,
        revision: NetworkRevision,
    ) -> Result<Self, Box<Problem<D>>> {
        let evidence_is_coherent = match problem.evidence() {
            ProblemEvidence::RuntimePulseLatchConflictRetained { evidence, .. } => {
                evidence.policy == ConflictPolicy::RetainAndDiagnose
                    && matches!(evidence.controls, ConflictControls::Pulse { set, reset } if set.is_positive() && reset.is_positive())
                    && evidence.at_ticks == at.time().ticks()
                    && evidence.reaction_order == at.order()
                    && evidence.revision == revision
            }
            _ => true,
        };
        if evidence_is_coherent
            && problem
                .code()
                .allows_delivery(ProblemDelivery::RuntimeOccurrence)
        {
            Ok(Self {
                problem,
                at,
                revision,
            })
        } else {
            Err(Box::new(problem))
        }
    }

    /// Returns the producing occurrence.
    #[must_use]
    pub const fn stamp(&self) -> crate::ReactionStamp<D> {
        self.at
    }

    /// Returns the complete catalogue-backed problem.
    #[must_use]
    pub const fn problem(&self) -> &Problem<D> {
        &self.problem
    }

    /// Returns the exact reaction time of the condition.
    #[must_use]
    pub const fn at(&self) -> Time<D> {
        self.at.time()
    }

    /// Returns the topology revision under which the condition settled.
    #[must_use]
    pub const fn revision(&self) -> NetworkRevision {
        self.revision
    }
}

/// A deterministic owned collection of report findings.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DiagnosticSet<D> {
    findings: Vec<Diagnostic<D>>,
    internal_defects: Vec<Problem<D>>,
}

impl<D> DiagnosticSet<D> {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            findings: Vec::new(),
            internal_defects: Vec::new(),
        }
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.findings.len()
    }
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.findings.is_empty()
    }
    pub fn iter(&self) -> impl Iterator<Item = &Diagnostic<D>> {
        self.findings.iter()
    }
    /// Returns internal invariant failures discovered while merging findings.
    #[must_use]
    pub fn internal_defects(&self) -> &[Problem<D>] {
        &self.internal_defects
    }
    #[must_use]
    pub fn has_severity(&self, severity: Severity) -> bool {
        self.findings
            .iter()
            .any(|finding| finding.problem().severity() == severity)
            || self
                .internal_defects
                .iter()
                .any(|defect| defect.severity() == severity)
    }
    pub fn insert(&mut self, diagnostic: Diagnostic<D>)
    where
        D: PartialEq,
    {
        if let Some(existing) = self
            .findings
            .iter()
            .position(|old| same_condition(old.problem(), diagnostic.problem()))
        {
            let code = self.findings[existing].problem.code;
            let primary = self.findings[existing].problem.primary.clone();
            let result = merge_evidence(
                &mut self.findings[existing].problem.evidence,
                diagnostic.problem.evidence,
            );
            match result {
                Ok(()) => {}
                Err(()) => self.record_evidence_conflict(code, primary),
            }
            self.findings.sort_by(compare_diagnostics);
            return;
        }
        self.findings.push(diagnostic);
        self.findings.sort_by(compare_diagnostics);
    }

    pub(crate) fn insert_internal_defect(&mut self, problem: Problem<D>) {
        if problem
            .code()
            .allows_delivery(ProblemDelivery::InternalDefect)
        {
            self.internal_defects.push(problem);
            self.internal_defects.sort_by(|left, right| {
                left.primary
                    .cmp_canonical(&right.primary)
                    .then_with(|| left.code.as_str().cmp(right.code.as_str()))
            });
        }
    }

    fn record_evidence_conflict(&mut self, code: DiagnosticCode, primary: SubjectRef) {
        if self.internal_defects.iter().any(|defect| {
            defect.code == DiagnosticCode::InternalDiagnosticEvidenceConflict
                && defect.primary == primary
                && matches!(
                    defect.evidence,
                    ProblemEvidence::InternalDiagnosticEvidenceConflict {
                        conflicting_code,
                        ..
                    } if conflicting_code == code
                )
        }) {
            return;
        }
        self.internal_defects
            .push(Problem::evidence_conflict(code, primary));
        self.internal_defects.sort_by(|left, right| {
            left.primary
                .cmp_canonical(&right.primary)
                .then_with(|| left.code.as_str().cmp(right.code.as_str()))
        });
    }
}

impl<D> IntoIterator for DiagnosticSet<D> {
    type Item = Diagnostic<D>;
    type IntoIter = std::vec::IntoIter<Diagnostic<D>>;
    fn into_iter(self) -> Self::IntoIter {
        self.findings.into_iter()
    }
}

fn same_condition<D>(left: &Problem<D>, right: &Problem<D>) -> bool {
    left.code == right.code
        && left.primary == right.primary
        && condition_discriminator(&left.evidence) == condition_discriminator(&right.evidence)
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ConditionDiscriminator {
    Subject(SubjectRef),
    SubjectAndKind(SubjectRef, SignalKind),
    Subjects(SubjectRef, SubjectRef),
    Required(RequiredInputRef, SignalKind),
    Arity(FixedArityRole, usize, usize),
    DuplicateSource(SubjectRef),
    VariadicArity(usize),
    ConstantResult(LogicLevel),
    Empty,
    Internal(DiagnosticCode, SubjectRef),
    Cycle(Vec<ReactionMemberRef>),
    Instances(Vec<ModuleInstanceKey>),
    StandardModule(StandardModuleRef),
    StandardParameter(StandardModuleRef, StandardParameterKey),
    StandardArity(StandardModuleRef, usize, u64),
    StandardInputs(StandardModuleRef, Vec<AnyModuleInputKey>),
    StandardSource(StandardModuleRef, SubjectRef),
    StandardDetail(StandardModuleRef, String),
    Binding(DiagnosticCode, Option<BindingSubjectRef>),
    ModuleBinding(ModuleInstanceKey, AnyModuleInputKey, ModuleBindingIssue),
    Text(&'static str, String),
    Operation(DiagnosticCode),
}

fn condition_discriminator<D>(evidence: &ProblemEvidence<D>) -> ConditionDiscriminator {
    match evidence {
        ProblemEvidence::AuthoringForeignSignal { .. } => {
            ConditionDiscriminator::Operation(DiagnosticCode::AuthoringForeignSignal)
        }
        ProblemEvidence::ValidationDuplicateKey { key, .. } => {
            ConditionDiscriminator::Subject(key.clone())
        }
        ProblemEvidence::ValidationMissingNode { missing, .. } => {
            ConditionDiscriminator::Subject(SubjectRef::Node(*missing))
        }
        ProblemEvidence::ValidationMissingPort {
            missing,
            expected_kind,
            ..
        }
        | ProblemEvidence::ValidationMissingEndpoint {
            missing,
            expected_kind,
            ..
        } => ConditionDiscriminator::SubjectAndKind(missing.clone(), *expected_kind),
        ProblemEvidence::ValidationInvalidDirection { source, target, .. } => {
            ConditionDiscriminator::Subjects(source.clone(), target.clone())
        }
        ProblemEvidence::ValidationSignalKindMismatch { .. }
        | ProblemEvidence::ValidationUnsupportedMultipleDrivers { .. } => {
            ConditionDiscriminator::Empty
        }
        ProblemEvidence::ValidationInvalidParameter {
            parameter,
            encountered,
            ..
        } => ConditionDiscriminator::Text(parameter, encountered.clone()),
        ProblemEvidence::ValidationMissingRequiredInput {
            required,
            expected_kind,
            ..
        } => ConditionDiscriminator::Required(required.clone(), *expected_kind),
        ProblemEvidence::ValidationInvalidFixedArity {
            role,
            expected,
            encountered,
            ..
        } => ConditionDiscriminator::Arity(*role, *expected, *encountered),
        ProblemEvidence::ValidationInvalidVariadicArity {
            minimum,
            encountered,
            ..
        } => ConditionDiscriminator::Arity(FixedArityRole::Input, *minimum, *encountered),
        ProblemEvidence::ValidationDuplicateSource { source, .. } => {
            ConditionDiscriminator::DuplicateSource(source.clone())
        }
        ProblemEvidence::ValidationEmptyVariadicNode { ports, .. } => {
            ConditionDiscriminator::VariadicArity(ports.len())
        }
        ProblemEvidence::ValidationUnaryDegenerateNode { ports, .. } => {
            ConditionDiscriminator::VariadicArity(ports.len())
        }
        ProblemEvidence::ValidationConstantResultNode { result, .. } => {
            ConditionDiscriminator::ConstantResult(*result)
        }
        ProblemEvidence::ValidationCurrentReactionCycle { members, .. } => {
            ConditionDiscriminator::Cycle(members.clone())
        }
        ProblemEvidence::ValidationInvalidModuleBinding {
            instance,
            input,
            issue,
            ..
        } => ConditionDiscriminator::ModuleBinding(*instance, *input, *issue),
        ProblemEvidence::ValidationMalformedHierarchy {
            instance, parent, ..
        } => ConditionDiscriminator::Subjects(
            SubjectRef::ModuleInstance(*instance),
            SubjectRef::ModuleInstance(*parent),
        ),
        ProblemEvidence::ValidationHierarchyCycle { instances, .. } => {
            ConditionDiscriminator::Instances(instances.clone())
        }
        ProblemEvidence::StandardModuleUnknownId { module_ref, .. }
        | ProblemEvidence::StandardModuleUnsupportedVersion { module_ref, .. }
        | ProblemEvidence::StandardModuleConstantResult { module_ref, .. } => {
            ConditionDiscriminator::StandardModule(module_ref.clone())
        }
        ProblemEvidence::StandardModuleMissingParameter {
            module_ref,
            parameter,
            ..
        }
        | ProblemEvidence::StandardModuleUnexpectedParameter {
            module_ref,
            parameter,
            ..
        }
        | ProblemEvidence::StandardModuleParameterKindMismatch {
            module_ref,
            parameter,
            ..
        }
        | ProblemEvidence::StandardModuleInvalidParameter {
            module_ref,
            parameter,
            ..
        } => ConditionDiscriminator::StandardParameter(module_ref.clone(), parameter.clone()),
        ProblemEvidence::StandardModuleInterfaceMismatch {
            module_ref, inputs, ..
        } => ConditionDiscriminator::StandardInputs(module_ref.clone(), inputs.clone()),
        ProblemEvidence::StandardModuleInternalKeyCollision {
            module_ref, key, ..
        } => ConditionDiscriminator::StandardDetail(module_ref.clone(), format!("key:{key:032x}")),
        ProblemEvidence::StandardModuleCatalogueInvariant {
            module_ref, detail, ..
        } => ConditionDiscriminator::StandardDetail(module_ref.clone(), detail.clone()),
        ProblemEvidence::StandardModuleEmptyVariadic {
            module_ref, inputs, ..
        }
        | ProblemEvidence::StandardModuleUnaryDegenerate {
            module_ref, inputs, ..
        } => ConditionDiscriminator::StandardInputs(
            module_ref.clone(),
            inputs
                .iter()
                .copied()
                .map(AnyModuleInputKey::from)
                .collect(),
        ),
        ProblemEvidence::StandardModuleImpossibleThreshold {
            module_ref,
            arity,
            threshold,
            ..
        } => ConditionDiscriminator::StandardArity(module_ref.clone(), *arity, *threshold),
        ProblemEvidence::StandardModuleDuplicateSource {
            module_ref, source, ..
        } => ConditionDiscriminator::StandardSource(module_ref.clone(), source.clone()),
        ProblemEvidence::BindingUnknownEndpoint { evidence, .. } => {
            ConditionDiscriminator::Binding(
                DiagnosticCode::BindingUnknownEndpoint,
                evidence.endpoint,
            )
        }
        ProblemEvidence::BindingWrongSignalKind { evidence, .. } => {
            ConditionDiscriminator::Binding(
                DiagnosticCode::BindingWrongSignalKind,
                evidence.endpoint,
            )
        }
        ProblemEvidence::BindingDuplicateEndpoint { evidence, .. } => {
            ConditionDiscriminator::Binding(
                DiagnosticCode::BindingDuplicateEndpoint,
                evidence.endpoint,
            )
        }
        ProblemEvidence::BindingDuplicateExternalKey { evidence, .. } => {
            ConditionDiscriminator::Binding(
                DiagnosticCode::BindingDuplicateExternalKey,
                evidence.endpoint,
            )
        }
        ProblemEvidence::BindingAmbiguousExternalKey { evidence, .. } => {
            ConditionDiscriminator::Binding(
                DiagnosticCode::BindingAmbiguousExternalKey,
                evidence.endpoint,
            )
        }
        ProblemEvidence::BindingMissingRequiredBinding { evidence, .. } => {
            ConditionDiscriminator::Binding(
                DiagnosticCode::BindingMissingRequiredBinding,
                evidence.endpoint,
            )
        }
        ProblemEvidence::BindingWrongNetwork { evidence, .. } => {
            ConditionDiscriminator::Binding(DiagnosticCode::BindingWrongNetwork, evidence.endpoint)
        }
        ProblemEvidence::BindingStaleSchema { evidence, .. } => {
            ConditionDiscriminator::Binding(DiagnosticCode::BindingStaleSchema, evidence.endpoint)
        }
        ProblemEvidence::LifecycleNotInitialized { .. } => {
            ConditionDiscriminator::Operation(DiagnosticCode::LifecycleNotInitialized)
        }
        ProblemEvidence::LifecycleAlreadyInitialized { .. } => {
            ConditionDiscriminator::Operation(DiagnosticCode::LifecycleAlreadyInitialized)
        }
        ProblemEvidence::LifecycleDeltaBeforeInitialization { .. } => {
            ConditionDiscriminator::Operation(DiagnosticCode::LifecycleDeltaBeforeInitialization)
        }
        ProblemEvidence::RuntimeStaleRevision { .. } => {
            ConditionDiscriminator::Operation(DiagnosticCode::RuntimeStaleRevision)
        }
        ProblemEvidence::RuntimeStaleExecutionState { .. } => {
            ConditionDiscriminator::Operation(DiagnosticCode::RuntimeStaleExecutionState)
        }
        ProblemEvidence::RuntimeTimeRegression { .. } => {
            ConditionDiscriminator::Operation(DiagnosticCode::RuntimeTimeRegression)
        }
        ProblemEvidence::RuntimeReactionOrderOverflow { .. } => {
            ConditionDiscriminator::Operation(DiagnosticCode::RuntimeReactionOrderOverflow)
        }
        ProblemEvidence::RuntimeTimeOverflow { .. } => {
            ConditionDiscriminator::Operation(DiagnosticCode::RuntimeTimeOverflow)
        }
        ProblemEvidence::RuntimeInvalidTimeSubtraction { .. } => {
            ConditionDiscriminator::Operation(DiagnosticCode::RuntimeInvalidTimeSubtraction)
        }
        ProblemEvidence::RuntimeZeroSpanNotAllowed { .. } => {
            ConditionDiscriminator::Operation(DiagnosticCode::RuntimeZeroSpanNotAllowed)
        }
        ProblemEvidence::RuntimePulseCountOverflow { .. } => {
            ConditionDiscriminator::Operation(DiagnosticCode::RuntimePulseCountOverflow)
        }
        ProblemEvidence::RuntimePolicyMissingLimit { .. } => {
            ConditionDiscriminator::Operation(DiagnosticCode::RuntimePolicyMissingLimit)
        }
        ProblemEvidence::RuntimePolicyInvalidLimit { .. } => {
            ConditionDiscriminator::Operation(DiagnosticCode::RuntimePolicyInvalidLimit)
        }
        ProblemEvidence::RuntimeBudgetExceeded { .. } => {
            ConditionDiscriminator::Operation(DiagnosticCode::RuntimeBudgetExceeded)
        }
        ProblemEvidence::RuntimePulseLatchConflictRetained { .. } => {
            ConditionDiscriminator::Operation(DiagnosticCode::RuntimePulseLatchConflictRetained)
        }
        ProblemEvidence::RuntimePulseLatchConflictRejected { .. } => {
            ConditionDiscriminator::Operation(DiagnosticCode::RuntimePulseLatchConflictRejected)
        }
        ProblemEvidence::RuntimeLevelLatchConflictRetained { .. } => {
            ConditionDiscriminator::Operation(DiagnosticCode::RuntimeLevelLatchConflictRetained)
        }
        ProblemEvidence::RuntimeLevelLatchConflictRejected { .. } => {
            ConditionDiscriminator::Operation(DiagnosticCode::RuntimeLevelLatchConflictRejected)
        }
        ProblemEvidence::InputUnknownEndpoint { .. } => {
            ConditionDiscriminator::Operation(DiagnosticCode::InputUnknownEndpoint)
        }
        ProblemEvidence::InputWrongSignalKind { .. } => {
            ConditionDiscriminator::Operation(DiagnosticCode::InputWrongSignalKind)
        }
        ProblemEvidence::InputDuplicateObservation { .. } => {
            ConditionDiscriminator::Operation(DiagnosticCode::InputDuplicateObservation)
        }
        ProblemEvidence::InputConflictingObservation { .. } => {
            ConditionDiscriminator::Operation(DiagnosticCode::InputConflictingObservation)
        }
        ProblemEvidence::InputMissingRequiredLevel { .. } => {
            ConditionDiscriminator::Operation(DiagnosticCode::InputMissingRequiredLevel)
        }
        ProblemEvidence::InputWrongNetwork { .. } => {
            ConditionDiscriminator::Operation(DiagnosticCode::InputWrongNetwork)
        }
        ProblemEvidence::InputForeignSchema { .. } => {
            ConditionDiscriminator::Operation(DiagnosticCode::InputForeignSchema)
        }
        ProblemEvidence::InputStaleSchema { .. } => {
            ConditionDiscriminator::Operation(DiagnosticCode::InputStaleSchema)
        }
        ProblemEvidence::InspectionUnknownSubject { .. } => {
            ConditionDiscriminator::Operation(DiagnosticCode::InspectionUnknownSubject)
        }
        ProblemEvidence::InspectionWrongSubjectKind { .. } => {
            ConditionDiscriminator::Operation(DiagnosticCode::InspectionWrongSubjectKind)
        }
        ProblemEvidence::InspectionPendingEventNotFound { .. } => {
            ConditionDiscriminator::Operation(DiagnosticCode::InspectionPendingEventNotFound)
        }
        ProblemEvidence::ExplanationUnknownSubject { .. } => {
            ConditionDiscriminator::Operation(DiagnosticCode::ExplanationUnknownSubject)
        }
        ProblemEvidence::ExplanationForeignCause { .. } => {
            ConditionDiscriminator::Operation(DiagnosticCode::ExplanationForeignCause)
        }
        ProblemEvidence::ExplanationInvalidCause { .. } => {
            ConditionDiscriminator::Operation(DiagnosticCode::ExplanationInvalidCause)
        }
        ProblemEvidence::InternalDiagnosticEvidenceConflict {
            conflicting_code,
            conflicting_primary,
            ..
        } => ConditionDiscriminator::Internal(*conflicting_code, conflicting_primary.clone()),
        ProblemEvidence::PersistenceInvalidPrefix { .. }
        | ProblemEvidence::PersistenceTruncatedArtifact { .. }
        | ProblemEvidence::PersistenceTrailingBytes { .. }
        | ProblemEvidence::PersistenceNoncanonicalEncoding { .. }
        | ProblemEvidence::PersistenceMalformedEnvelope { .. }
        | ProblemEvidence::PersistenceUnknownArtifactKind { .. }
        | ProblemEvidence::PersistenceUnknownSchemaField { .. }
        | ProblemEvidence::PersistenceUnknownSchemaVariant { .. }
        | ProblemEvidence::PersistenceIntegrityDigestMismatch { .. }
        | ProblemEvidence::PersistenceDecodeLimitExceeded { .. }
        | ProblemEvidence::PersistenceUnsupportedVersion { .. }
        | ProblemEvidence::PersistenceWrongTimeDomain { .. }
        | ProblemEvidence::PersistenceNetworkIdentityMismatch { .. }
        | ProblemEvidence::PersistenceFingerprintMismatch { .. }
        | ProblemEvidence::PersistenceTopologyRevisionMismatch { .. }
        | ProblemEvidence::PersistenceRuntimePolicyMismatch { .. }
        | ProblemEvidence::PersistenceLifecycleShapeInvalid { .. }
        | ProblemEvidence::PersistenceStateSchemaMismatch { .. }
        | ProblemEvidence::PersistenceUnknownSubject { .. }
        | ProblemEvidence::PersistencePendingEventInvalid { .. }
        | ProblemEvidence::PersistenceEventIdentityStateInvalid { .. }
        | ProblemEvidence::PersistenceDiagnosticEpisodeInvalid { .. }
        | ProblemEvidence::PersistenceDiagnosticSchemaInvalid { .. }
        | ProblemEvidence::PersistenceSettledStateInconsistent { .. }
        | ProblemEvidence::PersistenceExecutionDigestMismatch { .. }
        | ProblemEvidence::PersistenceObservableDigestMismatch { .. }
        | ProblemEvidence::PersistenceSnapshotDigestMismatch { .. }
        | ProblemEvidence::PersistenceDigestCollision { .. }
        | ProblemEvidence::PersistenceProvenanceMissingPredecessor { .. }
        | ProblemEvidence::PersistenceProvenanceDigestMismatch { .. }
        | ProblemEvidence::PersistenceProvenanceCycle { .. }
        | ProblemEvidence::PersistenceProvenanceInvalidSubject { .. }
        | ProblemEvidence::PersistenceProvenanceInvalidRole { .. }
        | ProblemEvidence::PersistenceProvenanceIncompleteRootClosure { .. }
        | ProblemEvidence::PersistenceProvenanceConflictingRecord { .. }
        | ProblemEvidence::PersistenceProvenanceFalseCheckpoint { .. } => {
            ConditionDiscriminator::Operation(evidence.code())
        }
        ProblemEvidence::ReplayStartingExecutionDigestMismatch { .. }
        | ProblemEvidence::ReplayStartingObservableDigestMismatch { .. }
        | ProblemEvidence::ReplayExpectedRevisionMismatch { .. }
        | ProblemEvidence::ReplayRuntimePolicyMismatch { .. }
        | ProblemEvidence::ReplayTimeDomainMismatch { .. }
        | ProblemEvidence::ReplayNetworkFingerprintMismatch { .. }
        | ProblemEvidence::ReplayLogsNotConcatenable { .. }
        | ProblemEvidence::ReplayPatchPreparationDiverged { .. }
        | ProblemEvidence::ReplayResultingExecutionDigestMismatch { .. }
        | ProblemEvidence::ReplayResultingObservableDigestMismatch { .. }
        | ProblemEvidence::ReplayFrameMissing { .. }
        | ProblemEvidence::ReplayFrameReordered { .. }
        | ProblemEvidence::ReplayFrameDuplicated { .. } => {
            ConditionDiscriminator::Operation(evidence.code())
        }
        ProblemEvidence::StandardModuleExpansionMismatch {
            module_ref, detail, ..
        } => ConditionDiscriminator::StandardDetail(module_ref.clone(), detail.clone()),
        ProblemEvidence::ReconfigurationConditionalSemanticLoss { fact, rule, .. }
        | ProblemEvidence::ReconfigurationUnavoidableSemanticLoss { fact, rule, .. } => {
            ConditionDiscriminator::Text(fact, (*rule).to_owned())
        }
        ProblemEvidence::ReconfigurationInvalidReassociation { source, target, .. } => {
            ConditionDiscriminator::Subjects(source.clone(), target.clone())
        }
        ProblemEvidence::StandardModuleNoncanonicalInternalEdit { subject, .. } => {
            ConditionDiscriminator::Subject(subject.clone())
        }
        ProblemEvidence::ReconfigurationBaseRevisionMismatch {
            expected, actual, ..
        } => ConditionDiscriminator::Text(
            "revision",
            format!("{:016x}:{:016x}", expected.value(), actual.value()),
        ),
        ProblemEvidence::ReconfigurationForeignArtifact { .. }
        | ProblemEvidence::ReconfigurationDuplicateOperation { .. }
        | ProblemEvidence::ReconfigurationConflictingEdit { .. }
        | ProblemEvidence::ReconfigurationInvalidReplacementKey { .. }
        | ProblemEvidence::ReconfigurationContradictoryHierarchy { .. }
        | ProblemEvidence::ReconfigurationBaseFingerprintMismatch { .. }
        | ProblemEvidence::ReconfigurationUnknownBaseSubject { .. }
        | ProblemEvidence::ReconfigurationNonInjectiveReassociation { .. }
        | ProblemEvidence::ReconfigurationIncompatibleMigrationDirective { .. }
        | ProblemEvidence::ReconfigurationIncompleteTemporalMigrationPolicy { .. }
        | ProblemEvidence::ReconfigurationUnsupportedCrossKindMigration { .. }
        | ProblemEvidence::ReconfigurationAmbiguousEventMigration { .. }
        | ProblemEvidence::ReconfigurationConflictingMigratedTransitions { .. }
        | ProblemEvidence::ReconfigurationInvalidTargetInputSchema { .. }
        | ProblemEvidence::ReconfigurationEmptyPatch { .. }
        | ProblemEvidence::ReconfigurationStalePreparedPatch { .. }
        | ProblemEvidence::ReconfigurationTargetInputSchemaMismatch { .. }
        | ProblemEvidence::ReconfigurationStateMigrationRejected { .. }
        | ProblemEvidence::ReconfigurationPendingEventMigrationRejected { .. }
        | ProblemEvidence::ReconfigurationRequirePreserveFailed { .. }
        | ProblemEvidence::ReconfigurationEpisodeMigrationRejected { .. }
        | ProblemEvidence::ReconfigurationProvenanceMigrationRejected { .. }
        | ProblemEvidence::ReconfigurationStateLossRejected { .. } => {
            ConditionDiscriminator::Operation(evidence.code())
        }
    }
}

fn merge_evidence<D: PartialEq>(
    existing: &mut ProblemEvidence<D>,
    incoming: ProblemEvidence<D>,
) -> Result<(), ()> {
    match (existing, incoming) {
        (
            ProblemEvidence::ValidationUnsupportedMultipleDrivers { drivers, .. },
            ProblemEvidence::ValidationUnsupportedMultipleDrivers {
                drivers: incoming, ..
            },
        ) => {
            drivers.extend(incoming);
            drivers.sort_by(SubjectRef::cmp_canonical);
            drivers.dedup();
            Ok(())
        }
        (
            ProblemEvidence::ValidationInvalidModuleBinding { sources, .. },
            ProblemEvidence::ValidationInvalidModuleBinding {
                sources: incoming, ..
            },
        ) => {
            // SPEC: docs/specs/contracts/module-instantiation-hierarchy.yaml
            // "complete-dynamic-binding-validation" — retain every independently found source.
            sources.extend(incoming);
            sources.sort_by(SubjectRef::cmp_canonical);
            sources.dedup();
            Ok(())
        }
        (
            ProblemEvidence::ValidationInvalidFixedArity { ports, .. },
            ProblemEvidence::ValidationInvalidFixedArity {
                ports: incoming, ..
            },
        )
        | (
            ProblemEvidence::ValidationInvalidVariadicArity { ports, .. },
            ProblemEvidence::ValidationInvalidVariadicArity {
                ports: incoming, ..
            },
        ) => {
            ports.extend(incoming);
            ports.sort_by(SubjectRef::cmp_canonical);
            ports.dedup();
            Ok(())
        }
        (existing, incoming)
            if matches!(
                existing,
                ProblemEvidence::ValidationCurrentReactionCycle { .. }
            ) && *existing == incoming =>
        {
            Ok(())
        }
        (existing, incoming) if *existing == incoming => Ok(()),
        _ => Err(()),
    }
}
fn compare_diagnostics<D>(left: &Diagnostic<D>, right: &Diagnostic<D>) -> Ordering {
    let left = left.problem();
    let right = right.problem();
    left.severity()
        .rank()
        .cmp(&right.severity().rank())
        .then_with(|| left.code.as_str().cmp(right.code.as_str()))
        .then_with(|| left.primary.cmp_canonical(&right.primary))
        .then_with(|| {
            compare_discriminators(
                condition_discriminator(&left.evidence),
                condition_discriminator(&right.evidence),
            )
        })
}

fn compare_discriminators(left: ConditionDiscriminator, right: ConditionDiscriminator) -> Ordering {
    fn tag(value: &ConditionDiscriminator) -> u8 {
        match value {
            ConditionDiscriminator::Subject(_) => 0,
            ConditionDiscriminator::SubjectAndKind(_, _) => 1,
            ConditionDiscriminator::Subjects(_, _) => 2,
            ConditionDiscriminator::Required(_, _) => 3,
            ConditionDiscriminator::Arity(_, _, _) => 4,
            ConditionDiscriminator::DuplicateSource(_) => 5,
            ConditionDiscriminator::VariadicArity(_) => 6,
            ConditionDiscriminator::ConstantResult(_) => 7,
            ConditionDiscriminator::Empty => 8,
            ConditionDiscriminator::Internal(_, _) => 9,
            ConditionDiscriminator::Cycle(_) => 10,
            ConditionDiscriminator::Instances(_) => 11,
            ConditionDiscriminator::StandardModule(_) => 12,
            ConditionDiscriminator::StandardParameter(_, _) => 13,
            ConditionDiscriminator::StandardArity(_, _, _) => 14,
            ConditionDiscriminator::StandardInputs(_, _) => 15,
            ConditionDiscriminator::StandardSource(_, _) => 16,
            ConditionDiscriminator::StandardDetail(_, _) => 17,
            ConditionDiscriminator::Binding(_, _) => 18,
            ConditionDiscriminator::Text(_, _) => 19,
            ConditionDiscriminator::Operation(_) => 20,
            ConditionDiscriminator::ModuleBinding(_, _, _) => 21,
        }
    }
    tag(&left)
        .cmp(&tag(&right))
        .then_with(|| match (left, right) {
            (ConditionDiscriminator::Subject(left), ConditionDiscriminator::Subject(right)) => {
                left.cmp_canonical(&right)
            }
            (
                ConditionDiscriminator::SubjectAndKind(left_subject, left_kind),
                ConditionDiscriminator::SubjectAndKind(right_subject, right_kind),
            ) => left_subject
                .cmp_canonical(&right_subject)
                .then_with(|| left_kind.cmp(&right_kind)),
            (
                ConditionDiscriminator::Subjects(left_first, left_second),
                ConditionDiscriminator::Subjects(right_first, right_second),
            ) => left_first
                .cmp_canonical(&right_first)
                .then_with(|| left_second.cmp_canonical(&right_second)),
            (
                ConditionDiscriminator::Required(left_required, left_kind),
                ConditionDiscriminator::Required(right_required, right_kind),
            ) => compare_required_inputs(left_required, right_required)
                .then_with(|| left_kind.cmp(&right_kind)),
            (
                ConditionDiscriminator::Arity(left_role, left_expected, left_encountered),
                ConditionDiscriminator::Arity(right_role, right_expected, right_encountered),
            ) => left_role
                .cmp(&right_role)
                .then_with(|| left_expected.cmp(&right_expected))
                .then_with(|| left_encountered.cmp(&right_encountered)),
            (
                ConditionDiscriminator::DuplicateSource(left),
                ConditionDiscriminator::DuplicateSource(right),
            ) => left.cmp_canonical(&right),
            (
                ConditionDiscriminator::VariadicArity(left),
                ConditionDiscriminator::VariadicArity(right),
            ) => left.cmp(&right),
            (
                ConditionDiscriminator::ConstantResult(left),
                ConditionDiscriminator::ConstantResult(right),
            ) => left.cmp(&right),
            (
                ConditionDiscriminator::Internal(left_code, left_subject),
                ConditionDiscriminator::Internal(right_code, right_subject),
            ) => left_code
                .cmp(&right_code)
                .then_with(|| left_subject.cmp_canonical(&right_subject)),
            (ConditionDiscriminator::Cycle(left), ConditionDiscriminator::Cycle(right)) => {
                left.cmp(&right)
            }
            (ConditionDiscriminator::Instances(left), ConditionDiscriminator::Instances(right)) => {
                left.cmp(&right)
            }
            (
                ConditionDiscriminator::StandardModule(left),
                ConditionDiscriminator::StandardModule(right),
            ) => left.cmp(&right),
            (
                ConditionDiscriminator::StandardParameter(left_module, left_parameter),
                ConditionDiscriminator::StandardParameter(right_module, right_parameter),
            ) => (left_module, left_parameter).cmp(&(right_module, right_parameter)),
            (
                ConditionDiscriminator::StandardArity(left_module, left_arity, left_threshold),
                ConditionDiscriminator::StandardArity(right_module, right_arity, right_threshold),
            ) => (left_module, left_arity, left_threshold).cmp(&(
                right_module,
                right_arity,
                right_threshold,
            )),
            (
                ConditionDiscriminator::StandardInputs(left_module, left_inputs),
                ConditionDiscriminator::StandardInputs(right_module, right_inputs),
            ) => (left_module, left_inputs).cmp(&(right_module, right_inputs)),
            (
                ConditionDiscriminator::StandardSource(left_module, left_source),
                ConditionDiscriminator::StandardSource(right_module, right_source),
            ) => left_module
                .cmp(&right_module)
                .then_with(|| left_source.cmp_canonical(&right_source)),
            (
                ConditionDiscriminator::StandardDetail(left_module, left_detail),
                ConditionDiscriminator::StandardDetail(right_module, right_detail),
            ) => (left_module, left_detail).cmp(&(right_module, right_detail)),
            (
                ConditionDiscriminator::Binding(left_code, left_subject),
                ConditionDiscriminator::Binding(right_code, right_subject),
            ) => (left_code, left_subject).cmp(&(right_code, right_subject)),
            (
                ConditionDiscriminator::Text(left_key, left_value),
                ConditionDiscriminator::Text(right_key, right_value),
            ) => (left_key, left_value).cmp(&(right_key, right_value)),
            (ConditionDiscriminator::Operation(left), ConditionDiscriminator::Operation(right)) => {
                left.cmp(&right)
            }
            (
                ConditionDiscriminator::ModuleBinding(left_instance, left_input, left_issue),
                ConditionDiscriminator::ModuleBinding(right_instance, right_input, right_issue),
            ) => (
                left_instance,
                left_input,
                module_binding_issue_tag(left_issue),
            )
                .cmp(&(
                    right_instance,
                    right_input,
                    module_binding_issue_tag(right_issue),
                )),
            _ => Ordering::Equal,
        })
}

const fn module_binding_issue_tag(issue: ModuleBindingIssue) -> u8 {
    match issue {
        ModuleBindingIssue::Missing => 0,
        ModuleBindingIssue::Duplicate => 1,
        ModuleBindingIssue::Invalid => 2,
    }
}

fn compare_required_inputs(left: RequiredInputRef, right: RequiredInputRef) -> Ordering {
    match (left, right) {
        (RequiredInputRef::Port(left), RequiredInputRef::Port(right)) => left.cmp_canonical(&right),
        (RequiredInputRef::Role(left), RequiredInputRef::Role(right)) => left.cmp(&right),
        (RequiredInputRef::Port(_), RequiredInputRef::Role(_)) => Ordering::Less,
        (RequiredInputRef::Role(_), RequiredInputRef::Port(_)) => Ordering::Greater,
    }
}

/// An artifact together with all independently collectable findings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report<T, D> {
    artifact: Option<T>,
    diagnostics: DiagnosticSet<D>,
}

impl<T, D> Report<T, D> {
    /// Creates a report, suppressing the artifact when an error remains.
    #[must_use]
    pub fn new(artifact: Option<T>, diagnostics: DiagnosticSet<D>) -> Self {
        Self {
            artifact: if diagnostics.has_severity(Severity::Error) {
                None
            } else {
                artifact
            },
            diagnostics,
        }
    }
    #[must_use]
    pub const fn artifact(&self) -> Option<&T> {
        self.artifact.as_ref()
    }
    #[must_use]
    pub const fn diagnostics(&self) -> &DiagnosticSet<D> {
        &self.diagnostics
    }
    #[must_use]
    pub fn has_errors(&self) -> bool {
        self.diagnostics.has_severity(Severity::Error)
    }
    #[must_use]
    pub fn has_warnings(&self) -> bool {
        self.diagnostics.has_severity(Severity::Warning)
    }
    #[must_use]
    pub fn into_parts(self) -> (Option<T>, DiagnosticSet<D>) {
        (self.artifact, self.diagnostics)
    }
    pub fn require_artifact(self) -> Result<T, ReportFailure<D>> {
        match self.artifact {
            Some(value) => Ok(value),
            None => Err(ReportFailure {
                diagnostics: self.diagnostics,
            }),
        }
    }
}

/// A report without an artifact, retaining every collected finding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReportFailure<D> {
    diagnostics: DiagnosticSet<D>,
}

impl<D> ReportFailure<D> {
    #[must_use]
    pub const fn diagnostics(&self) -> &DiagnosticSet<D> {
        &self.diagnostics
    }
    #[must_use]
    pub fn into_diagnostics(self) -> DiagnosticSet<D> {
        self.diagnostics
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::key::{AnyInPortKey, InPortKey};
    use crate::signal::Level;
    use std::collections::BTreeSet;
    use std::fmt::Write as _;

    #[test]
    fn implemented_catalogue_matches_the_reviewable_golden_registry() {
        let mut spellings = BTreeSet::new();
        let mut rendered = String::new();
        for code in DiagnosticCode::ALL {
            assert!(
                spellings.insert(code.as_str()),
                "duplicate catalogue spelling"
            );
            writeln!(
                rendered,
                "{}|{:?}|{:?}|{:?}|{}|{}|{}|{}|{}",
                code.as_str(),
                code.severity(),
                code.responsibility(),
                code.evidence_schema(),
                code.allows_delivery(ProblemDelivery::ReportFinding),
                code.allows_delivery(ProblemDelivery::OperationFailure),
                code.allows_delivery(ProblemDelivery::RuntimeOccurrence),
                code.allows_delivery(ProblemDelivery::PersistentEpisode),
                code.allows_delivery(ProblemDelivery::InternalDefect),
            )
            .unwrap_or_else(|_| unreachable!("writing to String cannot fail"));
        }
        assert_eq!(
            rendered,
            include_str!("../tests/golden/diagnostic_catalogue.txt")
        );
    }

    #[test]
    fn public_failure_leaf_inventory_is_unique_complete_and_operation_backed() {
        let mut leaves = BTreeSet::new();
        for line in include_str!("../tests/golden/public_failure_inventory.txt").lines() {
            let Some((leaf, mapping)) = line.split_once('|') else {
                panic!("failure inventory row must contain one separator: {line}");
            };
            assert!(leaves.insert(leaf), "duplicate public failure leaf: {leaf}");
            if mapping == "delegate" || mapping == "aggregate" {
                continue;
            }
            let code = DiagnosticCode::ALL
                .iter()
                .copied()
                .find(|code| code.as_str() == mapping)
                .unwrap_or_else(|| panic!("unregistered failure code: {mapping}"));
            assert!(
                code.allows_delivery(ProblemDelivery::OperationFailure),
                "public failure leaf uses a code that forbids failure delivery: {leaf}"
            );
        }
        assert_eq!(leaves.len(), 194);
    }

    fn missing<D>(node: u128, missing: u128) -> Diagnostic<D> {
        Diagnostic::new(Problem::new(
            SubjectRef::Node(NodeKey::from_u128(node)),
            Vec::new(),
            ProblemEvidence::ValidationMissingNode {
                missing: NodeKey::from_u128(missing),
                marker: PhantomData,
            },
        ))
        .unwrap_or_else(|_| unreachable!("validation code is reportable"))
    }
    #[test]
    fn subject_order_is_tag_then_payload() {
        assert_eq!(
            SubjectRef::Network(NetworkKey::from_u128(99))
                .cmp_canonical(&SubjectRef::Node(NodeKey::from_u128(0))),
            Ordering::Less
        );
        assert_eq!(
            SubjectRef::InPort(AnyInPortKey::from(InPortKey::<Level>::from_u128(1))).cmp_canonical(
                &SubjectRef::InPort(AnyInPortKey::from(InPortKey::<Level>::from_u128(2)))
            ),
            Ordering::Less
        );
    }
    #[test]
    fn reports_retain_findings_and_block_errors() {
        let mut set = DiagnosticSet::new();
        set.insert(missing::<()>(1, 2));
        let report = Report::new(Some(7), set);
        assert_eq!(report.artifact(), None);
        assert_eq!(
            report
                .require_artifact()
                .err()
                .map(|failure| failure.diagnostics().len()),
            Some(1)
        );
    }
    #[test]
    fn equal_detection_deduplicates() {
        let mut set = DiagnosticSet::new();
        set.insert(missing::<()>(1, 2));
        set.insert(missing::<()>(1, 2));
        assert_eq!(set.len(), 1);
    }

    #[test]
    fn distinct_missing_references_on_one_owner_do_not_collapse() {
        let mut set = DiagnosticSet::new();
        set.insert(missing::<()>(1, 2));
        set.insert(missing::<()>(1, 3));
        assert_eq!(set.len(), 2);
    }

    #[test]
    fn duplicate_claims_remain_a_canonical_multiset() {
        let evidence = ProblemEvidence::<()>::duplicate_key(
            SubjectRef::Node(NodeKey::from_u128(7)),
            vec![
                DuplicateClaim::Node {
                    key: NodeKey::from_u128(3),
                    kind: DuplicateNodeKind::Not,
                    inputs: Vec::new(),
                    input_roles: Vec::new(),
                    outputs: Vec::new(),
                    output_roles: Vec::new(),
                    origin: None,
                },
                DuplicateClaim::Node {
                    key: NodeKey::from_u128(3),
                    kind: DuplicateNodeKind::Not,
                    inputs: Vec::new(),
                    input_roles: Vec::new(),
                    outputs: Vec::new(),
                    output_roles: Vec::new(),
                    origin: None,
                },
                DuplicateClaim::Node {
                    key: NodeKey::from_u128(2),
                    kind: DuplicateNodeKind::Not,
                    inputs: Vec::new(),
                    input_roles: Vec::new(),
                    outputs: Vec::new(),
                    output_roles: Vec::new(),
                    origin: None,
                },
            ],
        );
        let problem = Problem::new(
            SubjectRef::Network(NetworkKey::from_u128(1)),
            Vec::new(),
            evidence,
        );
        match problem.evidence() {
            ProblemEvidence::ValidationDuplicateKey { claims, .. } => assert_eq!(
                claims,
                &[
                    DuplicateClaim::Node {
                        key: NodeKey::from_u128(2),
                        kind: DuplicateNodeKind::Not,
                        inputs: Vec::new(),
                        input_roles: Vec::new(),
                        outputs: Vec::new(),
                        output_roles: Vec::new(),
                        origin: None,
                    },
                    DuplicateClaim::Node {
                        key: NodeKey::from_u128(3),
                        kind: DuplicateNodeKind::Not,
                        inputs: Vec::new(),
                        input_roles: Vec::new(),
                        outputs: Vec::new(),
                        output_roles: Vec::new(),
                        origin: None,
                    },
                    DuplicateClaim::Node {
                        key: NodeKey::from_u128(3),
                        kind: DuplicateNodeKind::Not,
                        inputs: Vec::new(),
                        input_roles: Vec::new(),
                        outputs: Vec::new(),
                        output_roles: Vec::new(),
                        origin: None,
                    },
                ]
            ),
            _ => unreachable!("duplicate-key evidence was constructed"),
        }
    }

    #[test]
    fn driver_evidence_merges_as_a_canonical_set() {
        let primary = SubjectRef::InPort(AnyInPortKey::from(InPortKey::<Level>::from_u128(1)));
        let make = |drivers| {
            Diagnostic::new(Problem::new(
                primary.clone(),
                Vec::new(),
                ProblemEvidence::<()>::unsupported_multiple_drivers(drivers),
            ))
            .unwrap_or_else(|_| unreachable!("validation code is reportable"))
        };
        let mut set = DiagnosticSet::new();
        set.insert(make(vec![SubjectRef::Node(NodeKey::from_u128(3))]));
        set.insert(make(vec![
            SubjectRef::Node(NodeKey::from_u128(2)),
            SubjectRef::Node(NodeKey::from_u128(3)),
        ]));
        assert_eq!(set.len(), 1);
        match set
            .iter()
            .next()
            .map(Diagnostic::problem)
            .map(Problem::evidence)
        {
            Some(ProblemEvidence::ValidationUnsupportedMultipleDrivers { drivers, .. }) => {
                assert_eq!(
                    drivers,
                    &[
                        SubjectRef::Node(NodeKey::from_u128(2)),
                        SubjectRef::Node(NodeKey::from_u128(3)),
                    ]
                );
            }
            _ => unreachable!("merged driver evidence is retained"),
        }
    }

    #[test]
    fn contradictory_exact_evidence_records_an_internal_defect() {
        let primary = SubjectRef::Connection(ConnectionKey::from_u128(1));
        let make = |source| {
            Diagnostic::new(Problem::new(
                primary.clone(),
                Vec::new(),
                ProblemEvidence::<()>::signal_kind_mismatch(
                    source,
                    SubjectRef::Node(NodeKey::from_u128(2)),
                    SignalKind::Level,
                    SignalKind::Pulse,
                ),
            ))
            .unwrap_or_else(|_| unreachable!("validation code is reportable"))
        };
        let mut set = DiagnosticSet::new();
        set.insert(make(SubjectRef::Node(NodeKey::from_u128(3))));
        set.insert(make(SubjectRef::Node(NodeKey::from_u128(4))));
        assert_eq!(set.len(), 1);
        assert_eq!(set.internal_defects().len(), 1);
        assert_eq!(
            set.internal_defects()[0].code(),
            DiagnosticCode::InternalDiagnosticEvidenceConflict
        );
        assert_eq!(Report::new(Some(7), set).artifact(), None);
    }

    #[test]
    fn cycle_detection_is_idempotent_and_conflicting_witnesses_are_defects() {
        let first = ReactionMemberRef {
            subject: SubjectRef::Node(NodeKey::from_u128(1)),
            role: ReactionRole::NodeOperation,
        };
        let second = ReactionMemberRef {
            subject: SubjectRef::Node(NodeKey::from_u128(2)),
            role: ReactionRole::NodeOperation,
        };
        let forward = CurrentReactionCycleStep {
            source: first.clone(),
            dependency: SubjectRef::Connection(ConnectionKey::from_u128(1)),
            target: second.clone(),
        };
        let backward = CurrentReactionCycleStep {
            source: second.clone(),
            dependency: SubjectRef::Connection(ConnectionKey::from_u128(2)),
            target: first.clone(),
        };
        let make = |members, witness| {
            Diagnostic::new(Problem::new(
                SubjectRef::Network(NetworkKey::from_u128(9)),
                Vec::new(),
                ProblemEvidence::<()>::current_reaction_cycle(members, witness),
            ))
            .unwrap_or_else(|_| unreachable!("cycle validation is reportable"))
        };
        let mut set = DiagnosticSet::new();
        set.insert(make(
            vec![first.clone(), second.clone()],
            vec![forward.clone(), backward.clone()],
        ));
        set.insert(make(
            vec![second.clone(), first.clone()],
            vec![forward.clone(), backward.clone()],
        ));

        assert_eq!(set.len(), 1);
        assert!(set.internal_defects().is_empty());

        set.insert(make(
            vec![first.clone(), second.clone()],
            vec![backward.clone(), forward.clone()],
        ));
        assert_eq!(set.len(), 1);
        assert_eq!(set.internal_defects().len(), 1);
        assert_eq!(
            set.internal_defects()[0].code(),
            DiagnosticCode::InternalDiagnosticEvidenceConflict
        );
    }

    #[test]
    fn occurrence_construction_enforces_delivery_and_exact_conflict_pairing() {
        let revision = NetworkRevision::from_value(3);
        let at = Time::<()>::from_ticks(5);
        let conflict = ConflictEvidence {
            node: NodeEvidence::Node(NodeKey::from_u128(7)),
            policy: ConflictPolicy::RetainAndDiagnose,
            previous: LogicLevel::Low,
            controls: ConflictControls::Pulse {
                set: PulseCount::ONE,
                reset: PulseCount::new(2),
            },
            reaction_order: 0,
            at_ticks: at.ticks(),
            revision,
        };
        let retained = Problem::new(
            SubjectRef::Node(NodeKey::from_u128(7)),
            Vec::new(),
            ProblemEvidence::RuntimePulseLatchConflictRetained {
                evidence: conflict.clone(),
                marker: PhantomData,
            },
        );
        assert!(
            DiagnosticOccurrence::new(retained, crate::ReactionStamp::from_parts(at, 0), revision)
                .is_ok()
        );

        let wrong_control_kind = Problem::new(
            SubjectRef::Node(NodeKey::from_u128(7)),
            Vec::new(),
            ProblemEvidence::RuntimePulseLatchConflictRetained {
                evidence: ConflictEvidence {
                    controls: ConflictControls::Level {
                        set: LogicLevel::High,
                        reset: LogicLevel::High,
                    },
                    ..conflict.clone()
                },
                marker: PhantomData,
            },
        );
        assert!(
            DiagnosticOccurrence::new(
                wrong_control_kind,
                crate::ReactionStamp::from_parts(at, 0),
                revision
            )
            .is_err()
        );

        let wrong_time = Problem::new(
            SubjectRef::Node(NodeKey::from_u128(7)),
            Vec::new(),
            ProblemEvidence::RuntimePulseLatchConflictRetained {
                evidence: ConflictEvidence {
                    reaction_order: 0,
                    at_ticks: 4,
                    ..conflict.clone()
                },
                marker: PhantomData,
            },
        );
        assert!(
            DiagnosticOccurrence::new(
                wrong_time,
                crate::ReactionStamp::from_parts(at, 0),
                revision
            )
            .is_err()
        );

        let wrong_policy = Problem::new(
            SubjectRef::Node(NodeKey::from_u128(7)),
            Vec::new(),
            ProblemEvidence::RuntimePulseLatchConflictRetained {
                evidence: ConflictEvidence {
                    policy: ConflictPolicy::SetDominant,
                    ..conflict.clone()
                },
                marker: PhantomData,
            },
        );
        assert!(
            DiagnosticOccurrence::new(
                wrong_policy,
                crate::ReactionStamp::from_parts(at, 0),
                revision
            )
            .is_err()
        );

        let rejected = Problem::new(
            SubjectRef::Node(NodeKey::from_u128(7)),
            Vec::new(),
            ProblemEvidence::RuntimePulseLatchConflictRejected {
                evidence: conflict,
                marker: PhantomData,
            },
        );
        assert!(
            DiagnosticOccurrence::new(rejected, crate::ReactionStamp::from_parts(at, 0), revision)
                .is_err()
        );

        let operation_failure = Problem::new(
            SubjectRef::Operation(OperationSubjectRef::MachineTransaction),
            Vec::new(),
            ProblemEvidence::RuntimeBudgetExceeded {
                evidence: BudgetEvidence {
                    budget: "test",
                    limit: 1,
                    consumed: 2,
                },
                marker: PhantomData,
            },
        );
        assert!(
            DiagnosticOccurrence::new(
                operation_failure,
                crate::ReactionStamp::from_parts(at, 0),
                revision
            )
            .is_err()
        );
    }
}
