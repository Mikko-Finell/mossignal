//! Structured, catalogue-backed diagnostic findings and artifact reports.
//!
//! The opening catalogue is intentionally small.  Its types are the common
//! representation used by later graph construction and validation modules.

use crate::authored::{InputPortRole, OutputPortRole};
use crate::identity::{ModuleFingerprint, NetworkFingerprint};
use crate::key::{
    AnyExternalInputKey, AnyExternalOutputKey, AnyInPortKey, AnyModuleInputKey, AnyModuleOutputKey,
    AnyOutPortKey, ConnectionKey, ModuleInputKey, ModuleInstanceKey, NetworkKey, NodeKey,
};
use crate::machine::NetworkRevision;
use crate::metadata::OriginRef;
use crate::signal::{LogicLevel, SignalKind};
use crate::standard::{StandardModuleRef, StandardParameterKey, StandardParameterKind};
use core::cmp::Ordering;
use core::marker::PhantomData;

/// A stable subject to which a diagnostic condition applies.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ReactionMemberRef {
    pub subject: SubjectRef,
    pub role: ReactionRole,
}

/// One stable dependency step in a current-reaction cycle witness.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
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
            Self::InPort(key) => (11, SubjectPayload::InPort(*key)),
            Self::OutPort(key) => (12, SubjectPayload::OutPort(*key)),
            Self::Connection(key) => (13, SubjectPayload::Direct(key.as_u128())),
            Self::ExternalInput(key) => (14, SubjectPayload::ExternalInput(*key)),
            Self::ExternalOutput(key) => (15, SubjectPayload::ExternalOutput(*key)),
            Self::Binding(binding) => (16, SubjectPayload::Binding(*binding)),
        }
    }

    fn cmp_canonical(&self, other: &Self) -> Ordering {
        self.ordering_key().cmp(&other.ordering_key())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum SubjectPayload {
    Direct(u128),
    Fingerprint([u8; 32]),
    ModuleInput(AnyModuleInputKey),
    ModuleOutput(AnyModuleOutputKey),
    InstanceInput(ModuleInstanceKey, AnyModuleInputKey),
    InstanceOutput(ModuleInstanceKey, AnyModuleOutputKey),
    InPort(AnyInPortKey),
    OutPort(AnyOutPortKey),
    ExternalInput(AnyExternalInputKey),
    ExternalOutput(AnyExternalOutputKey),
    Binding(BindingSubjectRef),
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
    InternalDefect,
}

/// The opening catalogue's structured identifiers.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum DiagnosticCode {
    ValidationDuplicateKey,
    ValidationMissingNode,
    ValidationMissingPort,
    ValidationMissingEndpoint,
    ValidationInvalidDirection,
    ValidationSignalKindMismatch,
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
    InternalDiagnosticEvidenceConflict,
}

#[derive(Clone, Copy)]
struct CodeSpecification {
    code: &'static str,
    severity: Severity,
    responsibility: Responsibility,
    report_finding: bool,
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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
    Toggle(LogicLevel),
    PulseDelay(u64),
}

/// The stable identity of one required fixed input that is absent.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RequiredInputRef {
    /// An authored input port exists but has no driver.
    Port(SubjectRef),
    /// A fixed semantic input role has no corresponding authored port.
    Role(InputPortRole),
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

/// Structured evidence for one exact opening catalogue code.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProblemEvidence<D> {
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
    InternalDiagnosticEvidenceConflict {
        conflicting_code: DiagnosticCode,
        conflicting_primary: SubjectRef,
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
        Self::ValidationInvalidModuleBinding {
            instance,
            input,
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
            _ => {}
        }
    }
}

// SPEC: docs/specs/contracts/diagnostic-problem-model.yaml
// "authoritative-registry" — code spelling, classification, delivery, and
// evidence association are declared together so they cannot drift apart.
macro_rules! opening_diagnostic_registry {
    ($( $code:ident, $evidence:pat, $spelling:literal, $severity:ident, $responsibility:ident, $report_finding:expr; )+) => {
        impl DiagnosticCode {
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

            const fn specification(self) -> CodeSpecification {
                match self {
                    $(Self::$code => CodeSpecification {
                        code: $spelling,
                        severity: Severity::$severity,
                        responsibility: Responsibility::$responsibility,
                        report_finding: $report_finding,
                    },)+
                }
            }
        }

        impl<D> ProblemEvidence<D> {
            fn code(&self) -> DiagnosticCode {
                match self {
                    $($evidence => DiagnosticCode::$code,)+
                }
            }
        }
    };
}

opening_diagnostic_registry! {
    ValidationDuplicateKey, Self::ValidationDuplicateKey { .. }, "validation.duplicate_key", Error, CallerInput, true;
    ValidationMissingNode, Self::ValidationMissingNode { .. }, "validation.missing_node", Error, CallerInput, true;
    ValidationMissingPort, Self::ValidationMissingPort { .. }, "validation.missing_port", Error, CallerInput, true;
    ValidationMissingEndpoint, Self::ValidationMissingEndpoint { .. }, "validation.missing_endpoint", Error, CallerInput, true;
    ValidationInvalidDirection, Self::ValidationInvalidDirection { .. }, "validation.invalid_direction", Error, CallerInput, true;
    ValidationSignalKindMismatch, Self::ValidationSignalKindMismatch { .. }, "validation.signal_kind_mismatch", Error, CallerInput, true;
    ValidationUnsupportedMultipleDrivers, Self::ValidationUnsupportedMultipleDrivers { .. }, "validation.unsupported_multiple_drivers", Error, CallerInput, true;
    ValidationMissingRequiredInput, Self::ValidationMissingRequiredInput { .. }, "validation.missing_required_input", Error, CallerInput, true;
    ValidationInvalidFixedArity, Self::ValidationInvalidFixedArity { .. }, "validation.invalid_fixed_arity", Error, CallerInput, true;
    ValidationInvalidVariadicArity, Self::ValidationInvalidVariadicArity { .. }, "validation.invalid_variadic_arity", Error, CallerInput, true;
    ValidationDuplicateSource, Self::ValidationDuplicateSource { .. }, "validation.duplicate_source", Warning, CallerInput, true;
    ValidationEmptyVariadicNode, Self::ValidationEmptyVariadicNode { .. }, "validation.empty_variadic_node", Warning, Advisory, true;
    ValidationUnaryDegenerateNode, Self::ValidationUnaryDegenerateNode { .. }, "validation.unary_degenerate_node", Warning, Advisory, true;
    ValidationConstantResultNode, Self::ValidationConstantResultNode { .. }, "validation.constant_result_node", Warning, Advisory, true;
    ValidationCurrentReactionCycle, Self::ValidationCurrentReactionCycle { .. }, "validation.current_reaction_cycle", Error, CallerInput, true;
    ValidationInvalidModuleBinding, Self::ValidationInvalidModuleBinding { .. }, "validation.invalid_module_binding", Error, CallerInput, true;
    ValidationMalformedHierarchy, Self::ValidationMalformedHierarchy { .. }, "validation.malformed_hierarchy", Error, CallerInput, true;
    ValidationHierarchyCycle, Self::ValidationHierarchyCycle { .. }, "validation.hierarchy_cycle", Error, CallerInput, true;
    StandardModuleUnknownId, Self::StandardModuleUnknownId { .. }, "standard_module.unknown_id", Error, UnsupportedFeature, true;
    StandardModuleUnsupportedVersion, Self::StandardModuleUnsupportedVersion { .. }, "standard_module.unsupported_version", Error, Compatibility, true;
    StandardModuleMissingParameter, Self::StandardModuleMissingParameter { .. }, "standard_module.missing_parameter", Error, CallerInput, true;
    StandardModuleUnexpectedParameter, Self::StandardModuleUnexpectedParameter { .. }, "standard_module.unexpected_parameter", Error, CallerInput, true;
    StandardModuleParameterKindMismatch, Self::StandardModuleParameterKindMismatch { .. }, "standard_module.parameter_kind_mismatch", Error, CallerInput, true;
    StandardModuleInvalidParameter, Self::StandardModuleInvalidParameter { .. }, "standard_module.invalid_parameter", Error, CallerInput, true;
    StandardModuleInterfaceMismatch, Self::StandardModuleInterfaceMismatch { .. }, "standard_module.interface_mismatch", Error, CallerInput, true;
    StandardModuleInternalKeyCollision, Self::StandardModuleInternalKeyCollision { .. }, "standard_module.internal_key_collision", Error, LibraryDefect, true;
    StandardModuleCatalogueInvariant, Self::StandardModuleCatalogueInvariant { .. }, "standard_module.catalogue_invariant", Error, LibraryDefect, true;
    StandardModuleEmptyVariadic, Self::StandardModuleEmptyVariadic { .. }, "standard_module.empty_variadic", Warning, Advisory, true;
    StandardModuleUnaryDegenerate, Self::StandardModuleUnaryDegenerate { .. }, "standard_module.unary_degenerate", Warning, Advisory, true;
    StandardModuleImpossibleThreshold, Self::StandardModuleImpossibleThreshold { .. }, "standard_module.impossible_threshold", Warning, Advisory, true;
    StandardModuleConstantResult, Self::StandardModuleConstantResult { .. }, "standard_module.constant_result", Warning, Advisory, true;
    StandardModuleDuplicateSource, Self::StandardModuleDuplicateSource { .. }, "standard_module.duplicate_source", Warning, CallerInput, true;
    BindingUnknownEndpoint, Self::BindingUnknownEndpoint { .. }, "binding.unknown_endpoint", Error, CallerInput, true;
    BindingWrongSignalKind, Self::BindingWrongSignalKind { .. }, "binding.wrong_signal_kind", Error, CallerInput, true;
    BindingDuplicateEndpoint, Self::BindingDuplicateEndpoint { .. }, "binding.duplicate_endpoint", Error, CallerInput, true;
    BindingDuplicateExternalKey, Self::BindingDuplicateExternalKey { .. }, "binding.duplicate_external_key", Error, CallerInput, true;
    BindingAmbiguousExternalKey, Self::BindingAmbiguousExternalKey { .. }, "binding.ambiguous_external_key", Error, CallerInput, true;
    BindingMissingRequiredBinding, Self::BindingMissingRequiredBinding { .. }, "binding.missing_required_binding", Error, CallerInput, true;
    BindingWrongNetwork, Self::BindingWrongNetwork { .. }, "binding.wrong_network", Error, Compatibility, false;
    BindingStaleSchema, Self::BindingStaleSchema { .. }, "binding.stale_schema", Error, Compatibility, false;
    InternalDiagnosticEvidenceConflict, Self::InternalDiagnosticEvidenceConflict { .. }, "internal.diagnostic_evidence_conflict", Error, LibraryDefect, false;
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
        related: Vec<RelatedSubject>,
        mut evidence: ProblemEvidence<D>,
    ) -> Self {
        evidence.canonicalize();
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
            conflicting_primary,
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
        if problem.code.specification().report_finding {
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
            let primary = self.findings[existing].problem.primary;
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
}

fn condition_discriminator<D>(evidence: &ProblemEvidence<D>) -> ConditionDiscriminator {
    match evidence {
        ProblemEvidence::ValidationDuplicateKey { key, .. } => {
            ConditionDiscriminator::Subject(*key)
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
        } => ConditionDiscriminator::SubjectAndKind(*missing, *expected_kind),
        ProblemEvidence::ValidationInvalidDirection { source, target, .. } => {
            ConditionDiscriminator::Subjects(*source, *target)
        }
        ProblemEvidence::ValidationSignalKindMismatch { .. }
        | ProblemEvidence::ValidationUnsupportedMultipleDrivers { .. } => {
            ConditionDiscriminator::Empty
        }
        ProblemEvidence::ValidationMissingRequiredInput {
            required,
            expected_kind,
            ..
        } => ConditionDiscriminator::Required(*required, *expected_kind),
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
            ConditionDiscriminator::DuplicateSource(*source)
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
            instance, input, ..
        } => ConditionDiscriminator::Subject(SubjectRef::ModuleInstanceInput(*instance, *input)),
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
        } => ConditionDiscriminator::StandardSource(module_ref.clone(), *source),
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
        ProblemEvidence::InternalDiagnosticEvidenceConflict {
            conflicting_code,
            conflicting_primary,
            ..
        } => ConditionDiscriminator::Internal(*conflicting_code, *conflicting_primary),
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
            _ => Ordering::Equal,
        })
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
                primary,
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
                primary,
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
            source: first,
            dependency: SubjectRef::Connection(ConnectionKey::from_u128(1)),
            target: second,
        };
        let backward = CurrentReactionCycleStep {
            source: second,
            dependency: SubjectRef::Connection(ConnectionKey::from_u128(2)),
            target: first,
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
        set.insert(make(vec![first, second], vec![forward, backward]));
        set.insert(make(vec![second, first], vec![forward, backward]));

        assert_eq!(set.len(), 1);
        assert!(set.internal_defects().is_empty());

        set.insert(make(vec![first, second], vec![backward, forward]));
        assert_eq!(set.len(), 1);
        assert_eq!(set.internal_defects().len(), 1);
        assert_eq!(
            set.internal_defects()[0].code(),
            DiagnosticCode::InternalDiagnosticEvidenceConflict
        );
    }
}
