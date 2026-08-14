//! Mutable machine lifecycle over an immutable compiled topology.

use crate::authored::{EdgeDetectorKind, EdgeInitialization, EdgeObservation, NodeKind};
use crate::compile::CompiledNetwork;
use crate::diagnostics::{
    DiagnosticCode, InspectionEvidence, InspectionSubjectKind, LifecycleEvidence,
    OperationSubjectRef, Problem, ProblemEvidence, Responsibility, Severity, SubjectRef,
};
use crate::identity::{ModuleFingerprint, NetworkFingerprint};
use crate::key::{
    AnyModuleInputKey, AnyModuleOutputKey, ExternalInputKey, ExternalOutputKey, ModuleInstanceKey,
    NodeKey,
};
use crate::module::{ModuleOrigin, QualifiedConnectionRef, QualifiedModuleRef, QualifiedNodeRef};
use crate::policy::{RuntimePolicy, RuntimePolicyId};
use crate::signal::{Level, LogicLevel, PulseCount};
use crate::standard::{
    AllEqualInspection, AtMostInspection, ExactlyInspection, StandardInternalCategory,
    StandardModuleDeclaration, all_equal_result_key, at_most_result_key, exactly_result_key,
};
use crate::time::{NonZeroSpan, Time};
use crate::transaction::{CauseRef, ProvenanceView};
use core::fmt;
use core::marker::PhantomData;
use std::collections::BTreeMap;

/// The opaque machine-local revision of the currently installed topology.
///
/// A revision is distinct from the semantic network fingerprint. Its initial
/// numeric representation is private and is not a persistence-format promise.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NetworkRevision(u64);

/// A stable machine-local identity for one pending temporal obligation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PendingEventKey(u64);

impl PendingEventKey {
    pub(crate) const fn from_serial(value: u64) -> Self {
        Self(value)
    }
    /// Returns the machine-local monotonically allocated serial.
    #[must_use]
    pub const fn value(self) -> u64 {
        self.0
    }
}

/// The next caller-visible temporal wakeup state of a ready machine.
#[non_exhaustive]
pub enum Schedule<D> {
    Dormant,
    WakeAt(Time<D>),
}

impl<D> Clone for Schedule<D> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<D> Copy for Schedule<D> {}

impl<D> PartialEq for Schedule<D> {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Dormant, Self::Dormant) => true,
            (Self::WakeAt(left), Self::WakeAt(right)) => left == right,
            _ => false,
        }
    }
}

impl<D> Eq for Schedule<D> {}

impl<D> fmt::Debug for Schedule<D> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Dormant => formatter.write_str("Dormant"),
            Self::WakeAt(deadline) => formatter.debug_tuple("WakeAt").field(deadline).finish(),
        }
    }
}

/// Failure to access ready-only scheduling state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScheduleFailure {
    NotInitialized,
}

impl ScheduleFailure {
    #[must_use]
    pub const fn code(self) -> DiagnosticCode {
        DiagnosticCode::LifecycleNotInitialized
    }
    #[must_use]
    pub const fn severity(self) -> Severity {
        self.code().severity()
    }
    #[must_use]
    pub const fn responsibility(self) -> Responsibility {
        self.code().responsibility()
    }
    #[must_use]
    pub fn problem<D>(self) -> Problem<D> {
        lifecycle_not_initialized(OperationSubjectRef::MachineLifecycle)
    }
}

impl fmt::Display for ScheduleFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("schedule is unavailable before machine initialization")
    }
}

impl std::error::Error for ScheduleFailure {}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct PendingPulseDelay<D> {
    pub(crate) key: PendingEventKey,
    pub(crate) node: NodeKey,
    pub(crate) origin: Time<D>,
    pub(crate) deadline: Time<D>,
    pub(crate) count: PulseCount,
    pub(crate) revision: NetworkRevision,
    pub(crate) cause: CauseRef,
}

impl<D> Clone for PendingPulseDelay<D> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<D> Copy for PendingPulseDelay<D> {}

/// Structural information available for one compiled PulseDelay.
pub struct PulseDelayDefinitionInspection<D> {
    node: NodeKey,
    delay: NonZeroSpan<D>,
}

impl<D> PulseDelayDefinitionInspection<D> {
    #[must_use]
    pub const fn node(&self) -> NodeKey {
        self.node
    }

    #[must_use]
    pub const fn delay(&self) -> NonZeroSpan<D> {
        self.delay
    }
}

/// One owned observation of a pending PulseDelay group.
pub struct PendingPulseDelayInspection<D> {
    event: PendingEventKey,
    node: NodeKey,
    origin: Time<D>,
    deadline: Time<D>,
    count: PulseCount,
    revision: NetworkRevision,
    cause: CauseRef,
}

impl<D> PendingPulseDelayInspection<D> {
    #[must_use]
    pub const fn event(&self) -> PendingEventKey {
        self.event
    }

    /// Returns the stable identity of the PulseDelay that owns this obligation.
    #[must_use]
    pub const fn node(&self) -> NodeKey {
        self.node
    }

    #[must_use]
    pub const fn origin(&self) -> Time<D> {
        self.origin
    }

    #[must_use]
    pub const fn deadline(&self) -> Time<D> {
        self.deadline
    }

    #[must_use]
    pub const fn count(&self) -> PulseCount {
        self.count
    }

    #[must_use]
    pub const fn revision(&self) -> NetworkRevision {
        self.revision
    }

    #[must_use]
    pub const fn cause(&self) -> CauseRef {
        self.cause
    }
}

/// One owned ready-machine observation of PulseDelay pending work.
pub struct PulseDelayInspection<D> {
    node: NodeKey,
    delay: NonZeroSpan<D>,
    revision: NetworkRevision,
    at: Time<D>,
    pending: Vec<PendingPulseDelayInspection<D>>,
    next_deadline: Option<Time<D>>,
}

impl<D> PulseDelayInspection<D> {
    #[must_use]
    pub const fn node(&self) -> NodeKey {
        self.node
    }

    #[must_use]
    pub const fn delay(&self) -> NonZeroSpan<D> {
        self.delay
    }

    #[must_use]
    pub const fn revision(&self) -> NetworkRevision {
        self.revision
    }

    #[must_use]
    pub const fn at(&self) -> Time<D> {
        self.at
    }

    #[must_use]
    pub fn pending(&self) -> &[PendingPulseDelayInspection<D>] {
        &self.pending
    }

    #[must_use]
    pub const fn next_deadline(&self) -> Option<Time<D>> {
        self.next_deadline
    }
}

/// A structural or lifecycle failure to inspect one node as PulseDelay.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PulseDelayInspectionFailure {
    UnknownNode(NodeKey),
    NotPulseDelay(NodeKey),
    NotInitialized,
}

impl PulseDelayInspectionFailure {
    #[must_use]
    pub fn code(self) -> DiagnosticCode {
        self.problem::<()>().code()
    }
    #[must_use]
    pub fn severity(self) -> Severity {
        self.code().severity()
    }
    #[must_use]
    pub fn responsibility(self) -> Responsibility {
        self.code().responsibility()
    }
    #[must_use]
    pub fn problem<D>(self) -> Problem<D> {
        match self {
            Self::UnknownNode(node) => inspection_unknown(node, InspectionSubjectKind::PulseDelay),
            Self::NotPulseDelay(node) => {
                inspection_wrong_kind(node, InspectionSubjectKind::PulseDelay)
            }
            Self::NotInitialized => {
                lifecycle_not_initialized(OperationSubjectRef::MachineLifecycle)
            }
        }
    }
}

/// One public module input and its committed Level value, when applicable.
///
/// Pulse activity is reaction-scoped and is published through transaction
/// results rather than retained by module inspection.
///
/// ```compile_fail
/// use mossignal::ModuleInputInspection;
///
/// fn retained_pulse(inspection: &ModuleInputInspection) {
///     let _ = inspection.pulse();
/// }
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModuleInputInspection {
    key: AnyModuleInputKey,
    level: Option<LogicLevel>,
}

impl ModuleInputInspection {
    #[must_use]
    pub const fn key(&self) -> AnyModuleInputKey {
        self.key
    }

    /// Returns the committed Level value, or `None` for Pulse ports and before initialization.
    #[must_use]
    pub const fn level(&self) -> Option<LogicLevel> {
        self.level
    }
}

/// One public module output and its committed Level value, when applicable.
///
/// Pulse activity is reaction-scoped and is published through transaction
/// results rather than retained by module inspection.
///
/// ```compile_fail
/// use mossignal::ModuleOutputInspection;
///
/// fn retained_pulse(inspection: &ModuleOutputInspection) {
///     let _ = inspection.pulse();
/// }
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModuleOutputInspection {
    key: AnyModuleOutputKey,
    level: Option<LogicLevel>,
}

impl ModuleOutputInspection {
    #[must_use]
    pub const fn key(&self) -> AnyModuleOutputKey {
        self.key
    }

    /// Returns the committed Level value, or `None` for Pulse ports and before initialization.
    #[must_use]
    pub const fn level(&self) -> Option<LogicLevel> {
        self.level
    }
}

/// One qualified pending temporal obligation owned by a module-local PulseDelay.
pub struct ModulePendingPulseDelayInspection<D> {
    event: PendingEventKey,
    origin: Time<D>,
    deadline: Time<D>,
    count: PulseCount,
    cause: CauseRef,
}

impl<D> ModulePendingPulseDelayInspection<D> {
    #[must_use]
    pub const fn event(&self) -> PendingEventKey {
        self.event
    }
    #[must_use]
    pub const fn origin(&self) -> Time<D> {
        self.origin
    }
    #[must_use]
    pub const fn deadline(&self) -> Time<D> {
        self.deadline
    }
    #[must_use]
    pub const fn count(&self) -> PulseCount {
        self.count
    }
    #[must_use]
    pub const fn cause(&self) -> CauseRef {
        self.cause
    }
}

/// Owned persistent runtime facts for one expanded module-local primitive occurrence.
///
/// Reaction-scoped Pulse outputs are not retained by this inspection. Observe
/// externally published pulse activity through the owning transaction result.
///
/// ```compile_fail
/// use mossignal::ModuleNodeInspection;
///
/// fn retained_pulse<D>(inspection: &ModuleNodeInspection<D>) {
///     let _ = inspection.pulse();
/// }
/// ```
pub struct ModuleNodeInspection<D> {
    node: QualifiedNodeRef,
    standard_role: Option<String>,
    kind: NodeKind<D>,
    level: Option<LogicLevel>,
    cause: Option<CauseRef>,
    edge_observation: Option<EdgeObservation>,
    edge_observation_cause: Option<CauseRef>,
    toggle_state: Option<LogicLevel>,
    toggle_inversion: Option<CauseRef>,
    pending: Vec<ModulePendingPulseDelayInspection<D>>,
}

impl<D> ModuleNodeInspection<D> {
    #[must_use]
    pub const fn node(&self) -> &QualifiedNodeRef {
        &self.node
    }
    /// Returns the permanent canonical role for a standard-module internal node.
    #[must_use]
    pub fn standard_role(&self) -> Option<&str> {
        self.standard_role.as_deref()
    }
    #[must_use]
    pub const fn kind(&self) -> &NodeKind<D> {
        &self.kind
    }
    #[must_use]
    pub const fn level(&self) -> Option<LogicLevel> {
        self.level
    }
    /// Returns the current causal support for this primitive occurrence.
    #[must_use]
    pub const fn cause(&self) -> Option<CauseRef> {
        self.cause
    }
    /// Returns the committed remembered observation for an edge detector.
    #[must_use]
    pub const fn edge_observation(&self) -> Option<EdgeObservation> {
        self.edge_observation
    }
    /// Returns the retained cause of that committed observation.
    #[must_use]
    pub const fn edge_observation_cause(&self) -> Option<CauseRef> {
        self.edge_observation_cause
    }
    #[must_use]
    pub const fn toggle_state(&self) -> Option<LogicLevel> {
        self.toggle_state
    }
    #[must_use]
    pub const fn toggle_inversion(&self) -> Option<CauseRef> {
        self.toggle_inversion
    }
    #[must_use]
    pub fn pending(&self) -> &[ModulePendingPulseDelayInspection<D>] {
        &self.pending
    }
}

/// An owned public-boundary and expanded-internal observation of one user module.
pub struct ModuleInspection<D> {
    module: QualifiedModuleRef,
    origin: ModuleOrigin<D>,
    fingerprint: ModuleFingerprint,
    standard_declaration: Option<StandardModuleDeclaration<D>>,
    exactly: Option<ExactlyInspection>,
    at_most: Option<AtMostInspection>,
    all_equal: Option<AllEqualInspection>,
    revision: NetworkRevision,
    at: Option<Time<D>>,
    inputs: Vec<ModuleInputInspection>,
    outputs: Vec<ModuleOutputInspection>,
    nodes: Vec<ModuleNodeInspection<D>>,
    connections: Vec<QualifiedConnectionRef>,
    modules: Vec<QualifiedModuleRef>,
}

impl<D> ModuleInspection<D> {
    #[must_use]
    pub const fn module(&self) -> &QualifiedModuleRef {
        &self.module
    }
    #[must_use]
    pub const fn origin(&self) -> &ModuleOrigin<D> {
        &self.origin
    }
    #[must_use]
    pub const fn fingerprint(&self) -> ModuleFingerprint {
        self.fingerprint
    }
    #[must_use]
    pub const fn standard_declaration(&self) -> Option<&StandardModuleDeclaration<D>> {
        self.standard_declaration.as_ref()
    }
    #[must_use]
    pub const fn exactly(&self) -> Option<&ExactlyInspection> {
        self.exactly.as_ref()
    }
    #[must_use]
    pub const fn at_most(&self) -> Option<&AtMostInspection> {
        self.at_most.as_ref()
    }
    #[must_use]
    pub const fn all_equal(&self) -> Option<&AllEqualInspection> {
        self.all_equal.as_ref()
    }
    #[must_use]
    pub const fn revision(&self) -> NetworkRevision {
        self.revision
    }
    #[must_use]
    pub const fn at(&self) -> Option<Time<D>> {
        self.at
    }
    #[must_use]
    pub fn inputs(&self) -> &[ModuleInputInspection] {
        &self.inputs
    }
    #[must_use]
    pub fn outputs(&self) -> &[ModuleOutputInspection] {
        &self.outputs
    }
    #[must_use]
    pub fn nodes(&self) -> &[ModuleNodeInspection<D>] {
        &self.nodes
    }
    #[must_use]
    pub fn connections(&self) -> &[QualifiedConnectionRef] {
        &self.connections
    }
    #[must_use]
    pub fn modules(&self) -> &[QualifiedModuleRef] {
        &self.modules
    }
}

/// Failure to inspect one retained qualified module instance.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModuleInspectionFailure {
    /// The requested qualified module is absent from the compiled topology.
    UnknownModule(QualifiedModuleRef),
    /// Current module runtime facts are unavailable before initialization.
    NotInitialized,
}

impl ModuleInspectionFailure {
    #[must_use]
    pub fn code(&self) -> DiagnosticCode {
        self.problem::<()>().code()
    }
    #[must_use]
    pub fn severity(&self) -> Severity {
        self.code().severity()
    }
    #[must_use]
    pub fn responsibility(&self) -> Responsibility {
        self.code().responsibility()
    }
    #[must_use]
    pub fn problem<D>(&self) -> Problem<D> {
        match self {
            Self::UnknownModule(module) => {
                let requested = SubjectRef::ModuleInstance(module.instance());
                Problem::new(
                    requested,
                    Vec::new(),
                    ProblemEvidence::InspectionUnknownSubject {
                        evidence: InspectionEvidence {
                            requested,
                            qualified_path: module.instances().to_vec(),
                            expected: InspectionSubjectKind::Module,
                            actual: None,
                        },
                        marker: PhantomData,
                    },
                )
            }
            Self::NotInitialized => {
                lifecycle_not_initialized(OperationSubjectRef::MachineLifecycle)
            }
        }
    }
}

impl ModuleInspectionFailure {
    /// Returns the absent module identity for an unknown-module failure.
    #[must_use]
    pub const fn module(&self) -> Option<&QualifiedModuleRef> {
        match self {
            Self::UnknownModule(module) => Some(module),
            Self::NotInitialized => None,
        }
    }
}

impl fmt::Display for ModuleInspectionFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownModule(_) => {
                formatter.write_str("the qualified module is absent from the compiled topology")
            }
            Self::NotInitialized => {
                formatter.write_str("module runtime inspection requires an initialized machine")
            }
        }
    }
}

impl std::error::Error for ModuleInspectionFailure {}

impl NetworkRevision {
    const INITIAL: Self = Self(0);

    pub(crate) const fn initial() -> Self {
        Self::INITIAL
    }

    #[cfg(test)]
    pub(crate) const fn from_value(value: u64) -> Self {
        Self(value)
    }

    pub(crate) const fn value(self) -> u64 {
        self.0
    }
}

impl fmt::Debug for NetworkRevision {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "NetworkRevision({self})")
    }
}

impl fmt::Display for NetworkRevision {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{:016x}", self.0)
    }
}

/// The runtime lifecycle state of a [`Machine`].
#[non_exhaustive]
pub enum MachineStatus<D> {
    /// The machine has not yet received a complete initializing input snapshot.
    AwaitingInitialization,
    /// The machine has committed initialization and has a current logical time.
    Ready { now: Time<D> },
}

impl<D> Clone for MachineStatus<D> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<D> Copy for MachineStatus<D> {}

impl<D> PartialEq for MachineStatus<D> {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::AwaitingInitialization, Self::AwaitingInitialization) => true,
            (Self::Ready { now: left }, Self::Ready { now: right }) => left == right,
            _ => false,
        }
    }
}

impl<D> Eq for MachineStatus<D> {}

impl<D> fmt::Debug for MachineStatus<D> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AwaitingInitialization => formatter.write_str("AwaitingInitialization"),
            Self::Ready { now } => formatter.debug_struct("Ready").field("now", now).finish(),
        }
    }
}

pub(crate) struct MachineStore<D> {
    pub(crate) status: MachineStatus<D>,
    pub(crate) revision: NetworkRevision,
    pub(crate) external_levels: BTreeMap<ExternalInputKey<Level>, LogicLevel>,
    pub(crate) settled_levels: Vec<LogicLevel>,
    pub(crate) operation_levels: Vec<Option<LogicLevel>>,
    pub(crate) operation_causes: Vec<CauseRef>,
    pub(crate) output_baselines: BTreeMap<ExternalOutputKey<Level>, LogicLevel>,
    pub(crate) input_causes: BTreeMap<ExternalInputKey<Level>, CauseRef>,
    pub(crate) output_causes: BTreeMap<ExternalOutputKey<Level>, CauseRef>,
    pub(crate) provenance: Option<ProvenanceView<D>>,
    pub(crate) edge_observations: Vec<EdgeObservation>,
    pub(crate) edge_observation_causes: BTreeMap<NodeKey, CauseRef>,
    pub(crate) toggle_states: Vec<LogicLevel>,
    pub(crate) toggle_inversion_causes: BTreeMap<NodeKey, CauseRef>,
    pub(crate) pending_pulse_delays: BTreeMap<Time<D>, Vec<PendingPulseDelay<D>>>,
    pub(crate) next_pending_event_serial: u64,
}

impl<D> Clone for MachineStore<D> {
    fn clone(&self) -> Self {
        Self {
            status: self.status,
            revision: self.revision,
            external_levels: self.external_levels.clone(),
            settled_levels: self.settled_levels.clone(),
            operation_levels: self.operation_levels.clone(),
            operation_causes: self.operation_causes.clone(),
            output_baselines: self.output_baselines.clone(),
            input_causes: self.input_causes.clone(),
            output_causes: self.output_causes.clone(),
            provenance: self.provenance.clone(),
            edge_observations: self.edge_observations.clone(),
            edge_observation_causes: self.edge_observation_causes.clone(),
            toggle_states: self.toggle_states.clone(),
            toggle_inversion_causes: self.toggle_inversion_causes.clone(),
            pending_pulse_delays: self.pending_pulse_delays.clone(),
            next_pending_event_serial: self.next_pending_event_serial,
        }
    }
}

/// One mutable semantic execution instance of a compiled network.
///
/// The canonical type carries lifecycle at runtime, so both uninitialized and
/// ready machines have the same `Machine<D>` type. A newly spawned machine has
/// no current time or settled runtime values.
///
/// `Machine` deliberately does not provide ordinary cloning:
///
/// ```compile_fail
/// use mossignal::Machine;
///
/// fn clone_machine<D>(machine: &Machine<D>) -> Machine<D> {
///     machine.clone()
/// }
/// ```
pub struct Machine<D> {
    pub(crate) compiled: CompiledNetwork<D>,
    pub(crate) policy: RuntimePolicy,
    // SPEC: docs/specs/processor_and_runtime_architecture.md §6 "Machine lifecycle states"
    // Absence before initialization is lifecycle state, never fabricated Low values.
    pub(crate) store: MachineStore<D>,
}

/// Structural information available for one compiled edge detector in every lifecycle phase.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EdgeDetectorDefinitionInspection {
    node: NodeKey,
    detector: EdgeDetectorKind,
    initialization: EdgeInitialization,
}

impl EdgeDetectorDefinitionInspection {
    /// Returns the detector's stable node identity.
    #[must_use]
    pub const fn node(&self) -> NodeKey {
        self.node
    }

    /// Returns which transition law this node applies.
    #[must_use]
    pub const fn detector(&self) -> EdgeDetectorKind {
        self.detector
    }

    /// Returns the explicit first-reaction observation policy.
    #[must_use]
    pub const fn initialization(&self) -> EdgeInitialization {
        self.initialization
    }
}

/// One owned observation of an initialized edge detector's persistent facts.
///
/// An emitted edge Pulse is reaction-scoped and appears only in the committed
/// transaction result; it is not retained as a current node output.
///
/// ```compile_fail
/// use mossignal::EdgeDetectorInspection;
///
/// fn retained_output<D>(inspection: &EdgeDetectorInspection<D>) {
///     let _ = inspection.output();
/// }
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EdgeDetectorInspection<D> {
    node: NodeKey,
    detector: EdgeDetectorKind,
    initialization: EdgeInitialization,
    committed: EdgeObservation,
    input: LogicLevel,
    observation_cause: CauseRef,
    revision: NetworkRevision,
    at: Time<D>,
}

impl<D> EdgeDetectorInspection<D> {
    #[must_use]
    pub const fn node(&self) -> NodeKey {
        self.node
    }
    #[must_use]
    pub const fn detector(&self) -> EdgeDetectorKind {
        self.detector
    }
    #[must_use]
    pub const fn initialization(&self) -> EdgeInitialization {
        self.initialization
    }
    /// Returns the currently committed remembered observation.
    #[must_use]
    pub const fn committed(&self) -> EdgeObservation {
        self.committed
    }
    /// Returns the settled current Level input from the committed reaction.
    #[must_use]
    pub const fn input(&self) -> LogicLevel {
        self.input
    }
    /// Returns the retained cause of the committed observation.
    #[must_use]
    pub const fn observation_cause(&self) -> CauseRef {
        self.observation_cause
    }
    #[must_use]
    pub const fn revision(&self) -> NetworkRevision {
        self.revision
    }
    #[must_use]
    pub const fn at(&self) -> Time<D> {
        self.at
    }
}

/// A structural or lifecycle failure to inspect one node as an edge detector.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EdgeDetectorInspectionFailure {
    /// The requested stable node key is absent from this topology.
    UnknownNode(NodeKey),
    /// The requested stable node exists but is not an edge detector.
    NotEdgeDetector(NodeKey),
    /// Runtime state was requested before initialization committed.
    NotInitialized,
}

impl EdgeDetectorInspectionFailure {
    #[must_use]
    pub fn code(self) -> DiagnosticCode {
        self.problem::<()>().code()
    }
    #[must_use]
    pub fn severity(self) -> Severity {
        self.code().severity()
    }
    #[must_use]
    pub fn responsibility(self) -> Responsibility {
        self.code().responsibility()
    }
    #[must_use]
    pub fn problem<D>(self) -> Problem<D> {
        match self {
            Self::UnknownNode(node) => {
                inspection_unknown(node, InspectionSubjectKind::EdgeDetector)
            }
            Self::NotEdgeDetector(node) => {
                inspection_wrong_kind(node, InspectionSubjectKind::EdgeDetector)
            }
            Self::NotInitialized => {
                lifecycle_not_initialized(OperationSubjectRef::MachineLifecycle)
            }
        }
    }
}

/// Structural information available for one compiled Toggle in every lifecycle phase.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ToggleDefinitionInspection {
    node: NodeKey,
    initial: LogicLevel,
}

impl ToggleDefinitionInspection {
    /// Returns the inspected Toggle's stable node identity.
    #[must_use]
    pub const fn node(&self) -> NodeKey {
        self.node
    }

    /// Returns the declared level used as previous state by the first reaction.
    #[must_use]
    pub const fn initial(&self) -> LogicLevel {
        self.initial
    }
}

/// One owned observation of committed Toggle state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ToggleInspection<D> {
    node: NodeKey,
    initial: LogicLevel,
    committed: LogicLevel,
    revision: NetworkRevision,
    at: Time<D>,
    latest_inversion: Option<CauseRef>,
}

impl<D> ToggleInspection<D> {
    /// Returns the inspected Toggle's stable node identity.
    #[must_use]
    pub const fn node(&self) -> NodeKey {
        self.node
    }
    /// Returns the Toggle's immutable declared initial level.
    #[must_use]
    pub const fn initial(&self) -> LogicLevel {
        self.initial
    }
    /// Returns the currently committed stored level.
    #[must_use]
    pub const fn committed(&self) -> LogicLevel {
        self.committed
    }
    /// Returns the topology revision at which this observation was made.
    #[must_use]
    pub const fn revision(&self) -> NetworkRevision {
        self.revision
    }
    /// Returns the committed reaction time of this observation.
    #[must_use]
    pub const fn at(&self) -> Time<D> {
        self.at
    }
    /// Returns the retained cause of the most recent odd-count inversion, if any.
    #[must_use]
    pub const fn latest_inversion(&self) -> Option<CauseRef> {
        self.latest_inversion
    }
}

/// A structural failure to inspect one node as a Toggle.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToggleInspectionFailure {
    /// The requested stable node key is absent from this topology.
    UnknownNode(NodeKey),
    /// The requested stable node exists but is not a Toggle.
    NotToggle(NodeKey),
    /// Runtime state was requested before initialization committed.
    NotInitialized,
}

impl ToggleInspectionFailure {
    #[must_use]
    pub fn code(self) -> DiagnosticCode {
        self.problem::<()>().code()
    }
    #[must_use]
    pub fn severity(self) -> Severity {
        self.code().severity()
    }
    #[must_use]
    pub fn responsibility(self) -> Responsibility {
        self.code().responsibility()
    }
    #[must_use]
    pub fn problem<D>(self) -> Problem<D> {
        match self {
            Self::UnknownNode(node) => inspection_unknown(node, InspectionSubjectKind::Toggle),
            Self::NotToggle(node) => inspection_wrong_kind(node, InspectionSubjectKind::Toggle),
            Self::NotInitialized => {
                lifecycle_not_initialized(OperationSubjectRef::MachineLifecycle)
            }
        }
    }
}

fn lifecycle_not_initialized<D>(operation: OperationSubjectRef) -> Problem<D> {
    Problem::new(
        SubjectRef::Operation(operation),
        Vec::new(),
        ProblemEvidence::LifecycleNotInitialized {
            evidence: LifecycleEvidence {
                operation,
                current_time_ticks: None,
            },
            marker: PhantomData,
        },
    )
}

fn inspection_unknown<D>(node: NodeKey, expected: InspectionSubjectKind) -> Problem<D> {
    let requested = SubjectRef::Node(node);
    Problem::new(
        requested,
        Vec::new(),
        ProblemEvidence::InspectionUnknownSubject {
            evidence: InspectionEvidence {
                requested,
                qualified_path: Vec::new(),
                expected,
                actual: None,
            },
            marker: PhantomData,
        },
    )
}

fn inspection_wrong_kind<D>(node: NodeKey, expected: InspectionSubjectKind) -> Problem<D> {
    let requested = SubjectRef::Node(node);
    Problem::new(
        requested,
        Vec::new(),
        ProblemEvidence::InspectionWrongSubjectKind {
            evidence: InspectionEvidence {
                requested,
                qualified_path: Vec::new(),
                expected,
                actual: Some(InspectionSubjectKind::Node),
            },
            marker: PhantomData,
        },
    )
}

impl<D> Machine<D> {
    pub(crate) fn new(compiled: CompiledNetwork<D>, policy: RuntimePolicy) -> Self {
        let edge_observations = compiled.initial_edge_observations();
        let toggle_states = compiled.initial_toggle_states();
        Self {
            compiled,
            policy,
            store: MachineStore {
                status: MachineStatus::AwaitingInitialization,
                revision: NetworkRevision::INITIAL,
                external_levels: BTreeMap::new(),
                settled_levels: Vec::new(),
                operation_levels: Vec::new(),
                operation_causes: Vec::new(),
                output_baselines: BTreeMap::new(),
                input_causes: BTreeMap::new(),
                output_causes: BTreeMap::new(),
                provenance: None,
                edge_observations,
                edge_observation_causes: BTreeMap::new(),
                toggle_states,
                toggle_inversion_causes: BTreeMap::new(),
                pending_pulse_delays: BTreeMap::new(),
                next_pending_event_serial: 0,
            },
        }
    }

    /// Returns the current runtime lifecycle state.
    #[must_use]
    pub fn status(&self) -> MachineStatus<D> {
        self.store.status
    }

    /// Returns whether the machine has committed a successful initialization.
    #[must_use]
    pub fn is_initialized(&self) -> bool {
        matches!(self.store.status, MachineStatus::Ready { .. })
    }

    /// Returns the current logical time, or `None` before initialization.
    #[must_use]
    pub fn now(&self) -> Option<Time<D>> {
        match self.store.status {
            MachineStatus::AwaitingInitialization => None,
            MachineStatus::Ready { now } => Some(now),
        }
    }

    /// Returns the machine-local revision of the installed topology.
    #[must_use]
    pub fn revision(&self) -> NetworkRevision {
        self.store.revision
    }

    /// Returns the semantic fingerprint of the installed compiled topology.
    #[must_use]
    pub fn fingerprint(&self) -> NetworkFingerprint {
        self.compiled.fingerprint()
    }

    /// Returns the immutable compiled topology installed in this machine.
    #[must_use]
    pub fn compiled(&self) -> &CompiledNetwork<D> {
        &self.compiled
    }

    /// Returns the exact validated runtime policy associated at spawning.
    #[must_use]
    pub fn runtime_policy(&self) -> &RuntimePolicy {
        &self.policy
    }

    /// Returns the semantic identity of the associated runtime policy.
    #[must_use]
    pub fn runtime_policy_id(&self) -> RuntimePolicyId {
        self.policy.id()
    }

    /// Returns the least pending deadline without changing machine state.
    pub fn next_deadline(&self) -> Result<Option<Time<D>>, ScheduleFailure> {
        if !self.is_initialized() {
            return Err(ScheduleFailure::NotInitialized);
        }
        Ok(self.store.pending_pulse_delays.keys().next().copied())
    }

    /// Returns whether a ready machine is dormant or must be called at a deadline.
    pub fn schedule(&self) -> Result<Schedule<D>, ScheduleFailure> {
        match self.next_deadline()? {
            Some(deadline) => Ok(Schedule::WakeAt(deadline)),
            None => Ok(Schedule::Dormant),
        }
    }

    /// Returns PulseDelay's immutable definition in either lifecycle phase.
    pub fn inspect_pulse_delay_definition(
        &self,
        node: NodeKey,
    ) -> Result<PulseDelayDefinitionInspection<D>, PulseDelayInspectionFailure> {
        if self.compiled.qualified_node(node).is_some() {
            return Err(PulseDelayInspectionFailure::UnknownNode(node));
        }
        let Some(delay) = self.compiled.pulse_delay(node) else {
            return Err(if self.compiled.contains_node(node) {
                PulseDelayInspectionFailure::NotPulseDelay(node)
            } else {
                PulseDelayInspectionFailure::UnknownNode(node)
            });
        };
        Ok(PulseDelayDefinitionInspection { node, delay })
    }

    /// Returns stable pending-group facts for one PulseDelay on a ready machine.
    pub fn inspect_pulse_delay(
        &self,
        node: NodeKey,
    ) -> Result<PulseDelayInspection<D>, PulseDelayInspectionFailure> {
        let definition = self.inspect_pulse_delay_definition(node)?;
        let MachineStatus::Ready { now } = self.store.status else {
            return Err(PulseDelayInspectionFailure::NotInitialized);
        };
        let mut pending = self
            .store
            .pending_pulse_delays
            .values()
            .flatten()
            .filter(|event| event.node == node)
            .map(|event| PendingPulseDelayInspection {
                event: event.key,
                node: event.node,
                origin: event.origin,
                deadline: event.deadline,
                count: event.count,
                revision: event.revision,
                cause: event.cause,
            })
            .collect::<Vec<_>>();
        pending.sort_by_key(|event| (event.deadline, event.event));
        let next_deadline = pending.first().map(PendingPulseDelayInspection::deadline);
        Ok(PulseDelayInspection {
            node,
            delay: definition.delay,
            revision: self.store.revision,
            at: now,
            pending,
            next_deadline,
        })
    }

    /// Returns an owned observation of one top-level user-module instance.
    pub fn inspect_module(
        &self,
        instance: ModuleInstanceKey,
    ) -> Result<ModuleInspection<D>, ModuleInspectionFailure> {
        let module = match QualifiedModuleRef::from_instances(vec![instance]) {
            Some(module) => module,
            None => panic!("one explicit instance key must form a qualified module identity"),
        };
        self.inspect_qualified_module(module)
    }

    /// Returns an owned observation of one exact qualified user-module instance.
    pub fn inspect_qualified_module(
        &self,
        module: QualifiedModuleRef,
    ) -> Result<ModuleInspection<D>, ModuleInspectionFailure> {
        let Some(definition) = self.compiled.module(&module) else {
            return Err(ModuleInspectionFailure::UnknownModule(module));
        };
        if !self.is_initialized() {
            return Err(ModuleInspectionFailure::NotInitialized);
        }
        let level_at = |operation: Option<usize>| {
            operation
                .and_then(|index| self.store.operation_levels.get(index))
                .copied()
                .flatten()
        };
        let inputs: Vec<ModuleInputInspection> = definition
            .inputs()
            .map(|input| ModuleInputInspection {
                key: input.key(),
                level: level_at(self.compiled.module_input_operation(&module, input.key())),
            })
            .collect();
        let outputs: Vec<ModuleOutputInspection> = definition
            .outputs()
            .map(|output| ModuleOutputInspection {
                key: output.key(),
                level: level_at(self.compiled.module_output_operation(&module, output.key())),
            })
            .collect();
        let mut nodes = Vec::new();
        for (qualified, flat) in self.compiled.qualified_nodes_under(&module) {
            let Some(owner) = QualifiedModuleRef::from_instances(qualified.instances().to_vec())
            else {
                panic!("qualified module-local node must retain its owning instance path");
            };
            let Some(owner_definition) = self.compiled.module(&owner) else {
                panic!("compiled qualified node owner must retain its module definition");
            };
            let Some(kind) = owner_definition
                .graph()
                .nodes()
                .iter()
                .find(|node| node.key() == qualified.node())
                .map(|node| node.kind().clone())
            else {
                panic!("compiled qualified node must retain its module-local definition");
            };
            let edge_observation = self
                .compiled
                .edge_state_slot(flat)
                .and_then(|(slot, _, _)| self.store.edge_observations.get(slot.value()).copied())
                .filter(|_| self.is_initialized());
            let edge_observation_cause = self.store.edge_observation_causes.get(&flat).copied();
            let toggle_state = self
                .compiled
                .toggle_state_slot(flat)
                .and_then(|(slot, _)| self.store.toggle_states.get(slot.value()).copied())
                .filter(|_| self.is_initialized());
            let toggle_inversion = self.store.toggle_inversion_causes.get(&flat).copied();
            let mut pending = self
                .store
                .pending_pulse_delays
                .values()
                .flatten()
                .filter(|event| event.node == flat)
                .map(|event| ModulePendingPulseDelayInspection {
                    event: event.key,
                    origin: event.origin,
                    deadline: event.deadline,
                    count: event.count,
                    cause: event.cause,
                })
                .collect::<Vec<_>>();
            pending.sort_by_key(|event| (event.deadline, event.event));
            nodes.push(ModuleNodeInspection {
                node: qualified.clone(),
                standard_role: definition.standard_declaration().and_then(|declaration| {
                    declaration
                        .internal_roles()
                        .find(|role| {
                            role.category() == StandardInternalCategory::Node
                                && role.key() == qualified.node().as_u128()
                        })
                        .map(|role| role.role().to_owned())
                }),
                kind,
                level: level_at(self.compiled.node_operation(flat)),
                cause: self
                    .compiled
                    .node_operation(flat)
                    .and_then(|index| self.store.operation_causes.get(index))
                    .copied(),
                edge_observation,
                edge_observation_cause,
                toggle_state,
                toggle_inversion,
                pending,
            });
        }
        let connections = self
            .compiled
            .qualified_connections_under(&module)
            .cloned()
            .collect();
        let modules = self
            .compiled
            .qualified_modules_under(&module)
            .filter(|candidate| *candidate != &module)
            .cloned()
            .collect();
        let standard_declaration = definition.standard_declaration().cloned();
        let exactly = standard_declaration.as_ref().and_then(|declaration| {
            let threshold = declaration.exactly_threshold()?;
            let levels: Option<Vec<_>> = declaration
                .variadic_inputs()
                .map(|key| {
                    inputs
                        .iter()
                        .find(|input| input.key() == AnyModuleInputKey::Level(key))
                        .and_then(ModuleInputInspection::level)
                        .map(|level| (key, level))
                })
                .collect();
            let result = outputs
                .iter()
                .find(|output| output.key() == AnyModuleOutputKey::Level(exactly_result_key()))
                .and_then(ModuleOutputInspection::level)?;
            Some(ExactlyInspection::new(
                threshold,
                levels?.into_iter(),
                result,
            ))
        });
        let at_most = standard_declaration.as_ref().and_then(|declaration| {
            let threshold = declaration.at_most_threshold()?;
            let levels: Option<Vec<_>> = declaration
                .variadic_inputs()
                .map(|key| {
                    inputs
                        .iter()
                        .find(|input| input.key() == AnyModuleInputKey::Level(key))
                        .and_then(ModuleInputInspection::level)
                        .map(|level| (key, level))
                })
                .collect();
            let result = outputs
                .iter()
                .find(|output| output.key() == AnyModuleOutputKey::Level(at_most_result_key()))
                .and_then(ModuleOutputInspection::level)?;
            Some(AtMostInspection::new(
                threshold,
                levels?.into_iter(),
                result,
            ))
        });
        let all_equal = standard_declaration.as_ref().and_then(|declaration| {
            declaration.all_equal_dependency()?;
            let levels: Option<Vec<_>> = declaration
                .variadic_inputs()
                .map(|key| {
                    inputs
                        .iter()
                        .find(|input| input.key() == AnyModuleInputKey::Level(key))
                        .and_then(ModuleInputInspection::level)
                        .map(|level| (key, level))
                })
                .collect();
            let result = outputs
                .iter()
                .find(|output| output.key() == AnyModuleOutputKey::Level(all_equal_result_key()))
                .and_then(ModuleOutputInspection::level)?;
            Some(AllEqualInspection::new(levels?.into_iter(), result))
        });
        Ok(ModuleInspection {
            module,
            origin: definition.origin().clone(),
            fingerprint: definition.fingerprint(),
            standard_declaration,
            exactly,
            at_most,
            all_equal,
            revision: self.store.revision,
            at: self.now(),
            inputs,
            outputs,
            nodes,
            connections,
            modules,
        })
    }

    /// Returns one authoritative external level after initialization.
    #[must_use]
    pub fn external_level(&self, input: ExternalInputKey<Level>) -> Option<LogicLevel> {
        if !self.is_initialized() {
            return None;
        }
        self.store.external_levels.get(&input).copied()
    }

    /// Returns one established external level-output baseline after initialization.
    #[must_use]
    pub fn output_level(&self, output: ExternalOutputKey<Level>) -> Option<LogicLevel> {
        if !self.is_initialized() {
            return None;
        }
        self.store.output_baselines.get(&output).copied()
    }

    /// Returns the current retained cause of an established external level output.
    #[must_use]
    pub fn output_cause(&self, output: ExternalOutputKey<Level>) -> Option<CauseRef> {
        if !self.is_initialized() {
            return None;
        }
        self.store.output_causes.get(&output).copied()
    }

    /// Returns an edge detector's immutable definition in either lifecycle phase.
    pub fn inspect_edge_detector_definition(
        &self,
        node: NodeKey,
    ) -> Result<EdgeDetectorDefinitionInspection, EdgeDetectorInspectionFailure> {
        if self.compiled.qualified_node(node).is_some() {
            return Err(EdgeDetectorInspectionFailure::UnknownNode(node));
        }
        let Some((_, detector, initial)) = self.compiled.edge_state_slot(node) else {
            return Err(if self.compiled.contains_node(node) {
                EdgeDetectorInspectionFailure::NotEdgeDetector(node)
            } else {
                EdgeDetectorInspectionFailure::UnknownNode(node)
            });
        };
        let initialization = match initial {
            EdgeObservation::Unestablished => EdgeInitialization::Baseline,
            EdgeObservation::Established(level) => EdgeInitialization::Assume(level),
        };
        Ok(EdgeDetectorDefinitionInspection {
            node,
            detector,
            initialization,
        })
    }

    /// Returns committed state, the current Level input, and retained provenance.
    pub fn inspect_edge_detector(
        &self,
        node: NodeKey,
    ) -> Result<EdgeDetectorInspection<D>, EdgeDetectorInspectionFailure> {
        let definition = self.inspect_edge_detector_definition(node)?;
        let MachineStatus::Ready { now } = self.store.status else {
            return Err(EdgeDetectorInspectionFailure::NotInitialized);
        };
        let (slot, _, _) = self
            .compiled
            .edge_state_slot(node)
            .ok_or(EdgeDetectorInspectionFailure::NotEdgeDetector(node))?;
        let committed = self
            .store
            .edge_observations
            .get(slot.value())
            .copied()
            .ok_or(EdgeDetectorInspectionFailure::NotEdgeDetector(node))?;
        let input = self
            .compiled
            .edge_input_operation(node)
            .and_then(|index| self.store.operation_levels.get(index))
            .copied()
            .flatten()
            .ok_or(EdgeDetectorInspectionFailure::NotEdgeDetector(node))?;
        let observation_cause = self
            .store
            .edge_observation_causes
            .get(&node)
            .copied()
            .ok_or(EdgeDetectorInspectionFailure::NotEdgeDetector(node))?;
        Ok(EdgeDetectorInspection {
            node,
            detector: definition.detector,
            initialization: definition.initialization,
            committed,
            input,
            observation_cause,
            revision: self.store.revision,
            at: now,
        })
    }

    /// Returns declared Toggle state without requiring runtime initialization.
    pub fn inspect_toggle_definition(
        &self,
        node: NodeKey,
    ) -> Result<ToggleDefinitionInspection, ToggleInspectionFailure> {
        if self.compiled.qualified_node(node).is_some() {
            return Err(ToggleInspectionFailure::UnknownNode(node));
        }
        let Some((_, initial)) = self.compiled.toggle_state_slot(node) else {
            return Err(if self.compiled.contains_node(node) {
                ToggleInspectionFailure::NotToggle(node)
            } else {
                ToggleInspectionFailure::UnknownNode(node)
            });
        };
        Ok(ToggleDefinitionInspection { node, initial })
    }

    /// Returns an owned observation of committed Toggle state.
    pub fn inspect_toggle(
        &self,
        node: NodeKey,
    ) -> Result<ToggleInspection<D>, ToggleInspectionFailure> {
        let definition = self.inspect_toggle_definition(node)?;
        let MachineStatus::Ready { now } = self.store.status else {
            return Err(ToggleInspectionFailure::NotInitialized);
        };
        let (slot, _) = self
            .compiled
            .toggle_state_slot(node)
            .ok_or(ToggleInspectionFailure::NotToggle(node))?;
        let committed = self
            .store
            .toggle_states
            .get(slot.value())
            .copied()
            .ok_or(ToggleInspectionFailure::NotToggle(node))?;
        Ok(ToggleInspection {
            node,
            initial: definition.initial,
            committed,
            revision: self.store.revision,
            at: now,
            latest_inversion: self.store.toggle_inversion_causes.get(&node).copied(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TimeDomainId;
    use crate::authored::{NodeDef, NodeKind, NodePorts, UncheckedNetwork};
    use crate::key::{NetworkKey, NodeKey, OutPortKey};
    use crate::metadata::DiagnosticMeta;
    use crate::signal::{Level, LogicLevel};
    use std::collections::BTreeMap;

    #[derive(PartialEq)]
    struct Ticks;

    fn compiled() -> CompiledNetwork<Ticks> {
        let network = UncheckedNetwork::new(
            NetworkKey::from_u128(1),
            TimeDomainId::from_u128(2),
            DiagnosticMeta::default(),
            vec![NodeDef::new(
                NodeKey::from_u128(3),
                NodeKind::constant(LogicLevel::High),
                NodePorts::new(Vec::new(), vec![OutPortKey::<Level>::from_u128(4).into()]),
                DiagnosticMeta::default(),
            )],
            Vec::new(),
            Vec::new(),
            Vec::new(),
        );
        network
            .validate()
            .require_artifact()
            .unwrap_or_else(|_| panic!("fixture must validate"))
            .compile()
            .require_artifact()
            .unwrap_or_else(|_| panic!("fixture must compile"))
    }

    fn policy(values: [u64; 5]) -> RuntimePolicy {
        RuntimePolicy::builder()
            .max_internal_reactions(values[0])
            .max_evaluated_operations(values[1])
            .max_pending_events(values[2])
            .max_events_created_per_transaction(values[3])
            .max_required_provenance_growth(values[4])
            .build()
            .unwrap_or_else(|failure| panic!("complete policy must build: {failure}"))
    }

    #[test]
    fn spawn_retains_topology_and_exact_policy_without_initializing_values() {
        let compiled = compiled();
        let fingerprint = compiled.fingerprint();
        let expected_policy = policy([1, 2, 3, 4, 5]);
        let policy_id = expected_policy.id();

        let machine = compiled.spawn(expected_policy.clone());

        assert_eq!(machine.status(), MachineStatus::AwaitingInitialization);
        assert!(!machine.is_initialized());
        assert_eq!(machine.now(), None);
        assert_eq!(machine.fingerprint(), fingerprint);
        assert_eq!(machine.compiled().fingerprint(), fingerprint);
        assert_eq!(machine.runtime_policy(), &expected_policy);
        assert_eq!(machine.runtime_policy_id(), policy_id);
    }

    #[test]
    fn equivalent_spawns_have_deterministic_initial_revisions() {
        let compiled = compiled();
        let first = compiled.spawn(policy([1, 2, 3, 4, 5]));
        let second = compiled.spawn(policy([1, 2, 3, 4, 5]));

        assert_eq!(first.revision(), second.revision());
        assert_eq!(first.revision(), NetworkRevision::INITIAL);
    }

    #[test]
    fn spawned_machines_own_independent_mutable_lifecycle_stores() {
        let compiled = compiled();
        let mut first = compiled.spawn(policy([1, 2, 3, 4, 5]));
        let second = compiled.spawn(policy([1, 2, 3, 4, 5]));
        let expected_evaluation = compiled.evaluate_full(&BTreeMap::new());

        assert!(!core::ptr::eq(&first.store, &second.store));
        first.store.status = MachineStatus::Ready {
            now: Time::from_ticks(17),
        };
        first.store.revision = NetworkRevision(7);

        assert_eq!(
            first.status(),
            MachineStatus::Ready {
                now: Time::from_ticks(17)
            }
        );
        assert_eq!(first.now(), Some(Time::from_ticks(17)));
        assert_eq!(first.revision(), NetworkRevision(7));
        assert_eq!(second.status(), MachineStatus::AwaitingInitialization);
        assert_eq!(second.now(), None);
        assert_eq!(second.revision(), NetworkRevision::INITIAL);
        assert_eq!(
            first.compiled().network_key(),
            second.compiled().network_key()
        );
        assert_eq!(first.fingerprint(), second.fingerprint());
        assert_eq!(first.runtime_policy_id(), second.runtime_policy_id());
        assert_eq!(
            first.compiled().evaluate_full(&BTreeMap::new()),
            expected_evaluation
        );
        assert_eq!(
            second.compiled().evaluate_full(&BTreeMap::new()),
            expected_evaluation
        );
    }

    #[test]
    fn scheduling_and_inspection_leaves_share_the_problem_kernel() {
        fn assert_problem(code: DiagnosticCode, problem: Problem<()>) {
            assert_eq!(problem.code(), code);
            assert_eq!(problem.evidence().code(), code);
            assert_eq!(problem.severity(), code.severity());
            assert_eq!(problem.responsibility(), code.responsibility());
            assert!(code.allows_delivery(crate::diagnostics::ProblemDelivery::OperationFailure));
        }

        let node = NodeKey::from_u128(7);
        assert_problem(
            DiagnosticCode::LifecycleNotInitialized,
            ScheduleFailure::NotInitialized.problem(),
        );
        for failure in [
            PulseDelayInspectionFailure::UnknownNode(node),
            PulseDelayInspectionFailure::NotPulseDelay(node),
            PulseDelayInspectionFailure::NotInitialized,
        ] {
            assert_problem(failure.code(), failure.problem());
        }
        let module = QualifiedModuleRef::from_instances(vec![ModuleInstanceKey::from_u128(8)])
            .unwrap_or_else(|| panic!("non-empty qualified module path must construct"));
        for failure in [
            ModuleInspectionFailure::UnknownModule(module),
            ModuleInspectionFailure::NotInitialized,
        ] {
            assert_problem(failure.code(), failure.problem());
        }
        for failure in [
            EdgeDetectorInspectionFailure::UnknownNode(node),
            EdgeDetectorInspectionFailure::NotEdgeDetector(node),
            EdgeDetectorInspectionFailure::NotInitialized,
        ] {
            assert_problem(failure.code(), failure.problem());
        }
        for failure in [
            ToggleInspectionFailure::UnknownNode(node),
            ToggleInspectionFailure::NotToggle(node),
            ToggleInspectionFailure::NotInitialized,
        ] {
            assert_problem(failure.code(), failure.problem());
        }
    }
}
