//! Mutable machine lifecycle over an immutable compiled topology.

use crate::authored::{
    ConflictPolicy, EdgeDetectorKind, EdgeInitialization, EdgeObservation, FirstEmissionPolicy,
    NodeKind, ReenablePhasePolicy,
};
use crate::compile::CompiledNetwork;
use crate::diagnostics::{
    DiagnosticCode, InspectionEvidence, InspectionSubjectKind, LifecycleEvidence,
    OperationSubjectRef, Problem, ProblemEvidence, Responsibility, Severity, SubjectRef,
};
use crate::identity::{
    ExecutionStateDigest, ModuleFingerprint, NetworkFingerprint, ObservableStateDigest,
};
use crate::key::{
    AnyModuleInputKey, AnyModuleOutputKey, ExternalInputKey, ExternalOutputKey, ModuleInstanceKey,
    NodeKey,
};
use crate::module::{
    ModuleOrigin, NodeSubject, QualifiedConnectionRef, QualifiedModuleRef, QualifiedNodeRef,
};
use crate::policy::{RuntimePolicy, RuntimePolicyId};
use crate::signal::{Level, LogicLevel, PulseCount};
use crate::standard::{
    AllEqualInspection, AtMostInspection, ExactlyInspection, StandardInternalCategory,
    StandardModuleDeclaration, all_equal_result_key, at_most_result_key, exactly_result_key,
};
use crate::time::{NonZeroSpan, ReactionStamp, Time};
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
    pub(crate) stimulus: ReactionStamp<D>,
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

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct PendingTransportDelay<D> {
    pub(crate) key: PendingEventKey,
    pub(crate) node: NodeKey,
    pub(crate) stimulus: ReactionStamp<D>,
    pub(crate) origin: Time<D>,
    pub(crate) deadline: Time<D>,
    pub(crate) target: LogicLevel,
    pub(crate) revision: NetworkRevision,
    pub(crate) cause: CauseRef,
}

impl<D> Clone for PendingTransportDelay<D> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<D> Copy for PendingTransportDelay<D> {}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct PendingInertialDelay<D> {
    pub(crate) key: PendingEventKey,
    pub(crate) node: NodeKey,
    pub(crate) stimulus: ReactionStamp<D>,
    pub(crate) origin: Time<D>,
    pub(crate) deadline: Time<D>,
    pub(crate) target: LogicLevel,
    pub(crate) revision: NetworkRevision,
    pub(crate) cause: CauseRef,
}

impl<D> Clone for PendingInertialDelay<D> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<D> Copy for PendingInertialDelay<D> {}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct PendingPeriodicBoundary<D> {
    pub(crate) key: PendingEventKey,
    pub(crate) node: NodeKey,
    pub(crate) stimulus: ReactionStamp<D>,
    pub(crate) origin: Time<D>,
    pub(crate) deadline: Time<D>,
    pub(crate) anchor: Time<D>,
    pub(crate) ordinal: u64,
    pub(crate) first_emission: FirstEmissionPolicy,
    pub(crate) reenable_phase: ReenablePhasePolicy,
    pub(crate) revision: NetworkRevision,
    pub(crate) cause: CauseRef,
}

impl<D> Clone for PendingPeriodicBoundary<D> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<D> Copy for PendingPeriodicBoundary<D> {}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum PendingEvent<D> {
    PulseDelay(PendingPulseDelay<D>),
    TransportDelay(PendingTransportDelay<D>),
    Inertial(PendingInertialDelay<D>),
    Periodic(PendingPeriodicBoundary<D>),
}

impl<D> Clone for PendingEvent<D> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<D> Copy for PendingEvent<D> {}

impl<D> PendingEvent<D> {
    pub(crate) fn stimulus(self) -> ReactionStamp<D> {
        match self {
            Self::PulseDelay(e) => e.stimulus,
            Self::TransportDelay(e) => e.stimulus,
            Self::Inertial(e) => e.stimulus,
            Self::Periodic(e) => e.stimulus,
        }
    }
    pub(crate) fn identity(
        self,
    ) -> (
        PendingEventKey,
        NodeKey,
        Time<D>,
        Time<D>,
        NetworkRevision,
        CauseRef,
    ) {
        match self {
            Self::PulseDelay(event) => (
                event.key,
                event.node,
                event.origin,
                event.deadline,
                event.revision,
                event.cause,
            ),
            Self::TransportDelay(event) => (
                event.key,
                event.node,
                event.origin,
                event.deadline,
                event.revision,
                event.cause,
            ),
            Self::Inertial(event) => (
                event.key,
                event.node,
                event.origin,
                event.deadline,
                event.revision,
                event.cause,
            ),
            Self::Periodic(event) => (
                event.key,
                event.node,
                event.origin,
                event.deadline,
                event.revision,
                event.cause,
            ),
        }
    }

    pub(crate) const fn kind_name(self) -> &'static str {
        match self {
            Self::PulseDelay(_) => "pulse_delay",
            Self::TransportDelay(_) => "transport_delay",
            Self::Inertial(_) => "inertial_delay",
            Self::Periodic(_) => "periodic",
        }
    }
}

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
    stimulus: ReactionStamp<D>,
    origin: Time<D>,
    deadline: Time<D>,
    count: PulseCount,
    revision: NetworkRevision,
    cause: CauseRef,
}

impl<D> PendingPulseDelayInspection<D> {
    /// Returns the immutable originating occurrence, independent of retiming.
    #[must_use]
    pub const fn origin_stamp(&self) -> ReactionStamp<D> {
        self.stimulus
    }

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
    provenance: ProvenanceView<D>,
}

/// Structural information available for one compiled TransportDelay.
pub struct TransportDelayDefinitionInspection<D> {
    node: NodeKey,
    delay: NonZeroSpan<D>,
    initial: LogicLevel,
}

impl<D> TransportDelayDefinitionInspection<D> {
    #[must_use]
    pub const fn node(&self) -> NodeKey {
        self.node
    }

    #[must_use]
    pub const fn delay(&self) -> NonZeroSpan<D> {
        self.delay
    }

    #[must_use]
    pub const fn initial(&self) -> LogicLevel {
        self.initial
    }
}

/// One owned observation of a pending TransportDelay transition.
pub struct PendingTransportDelayInspection<D> {
    event: PendingEventKey,
    node: NodeKey,
    stimulus: ReactionStamp<D>,
    origin: Time<D>,
    deadline: Time<D>,
    target: LogicLevel,
    revision: NetworkRevision,
    cause: CauseRef,
}

impl<D> Clone for PendingTransportDelayInspection<D> {
    fn clone(&self) -> Self {
        Self {
            event: self.event,
            node: self.node,
            stimulus: self.stimulus,
            origin: self.origin,
            deadline: self.deadline,
            target: self.target,
            revision: self.revision,
            cause: self.cause,
        }
    }
}

impl<D> PendingTransportDelayInspection<D> {
    /// Returns the immutable originating occurrence, independent of retiming.
    #[must_use]
    pub const fn origin_stamp(&self) -> ReactionStamp<D> {
        self.stimulus
    }

    #[must_use]
    pub const fn event(&self) -> PendingEventKey {
        self.event
    }

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
    pub const fn target(&self) -> LogicLevel {
        self.target
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

/// One owned ready-machine observation of TransportDelay state and pending work.
pub struct TransportDelayInspection<D> {
    node: NodeSubject,
    delay: NonZeroSpan<D>,
    initial: LogicLevel,
    remembered_input: LogicLevel,
    committed: LogicLevel,
    input: LogicLevel,
    revision: NetworkRevision,
    at: Time<D>,
    latest_transition: CauseRef,
    current_support: CauseRef,
    pending: Vec<PendingTransportDelayInspection<D>>,
    next_deadline: Option<Time<D>>,
    provenance: ProvenanceView<D>,
}

/// Structural information available for one compiled InertialDelay.
pub struct InertialDelayDefinitionInspection<D> {
    node: NodeKey,
    delay: NonZeroSpan<D>,
    initial: LogicLevel,
}

impl<D> InertialDelayDefinitionInspection<D> {
    #[must_use]
    pub const fn node(&self) -> NodeKey {
        self.node
    }

    #[must_use]
    pub const fn delay(&self) -> NonZeroSpan<D> {
        self.delay
    }

    #[must_use]
    pub const fn initial(&self) -> LogicLevel {
        self.initial
    }
}

/// One owned observation of a pending InertialDelay candidate.
pub struct PendingInertialDelayInspection<D> {
    event: PendingEventKey,
    node: NodeKey,
    stimulus: ReactionStamp<D>,
    origin: Time<D>,
    deadline: Time<D>,
    target: LogicLevel,
    revision: NetworkRevision,
    cause: CauseRef,
}

impl<D> Clone for PendingInertialDelayInspection<D> {
    fn clone(&self) -> Self {
        Self {
            event: self.event,
            node: self.node,
            stimulus: self.stimulus,
            origin: self.origin,
            deadline: self.deadline,
            target: self.target,
            revision: self.revision,
            cause: self.cause,
        }
    }
}

impl<D> PendingInertialDelayInspection<D> {
    /// Returns the immutable originating occurrence, independent of retiming.
    #[must_use]
    pub const fn origin_stamp(&self) -> ReactionStamp<D> {
        self.stimulus
    }

    #[must_use]
    pub const fn event(&self) -> PendingEventKey {
        self.event
    }
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
    pub const fn target(&self) -> LogicLevel {
        self.target
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

/// One owned ready-machine observation of InertialDelay state and candidate work.
pub struct InertialDelayInspection<D> {
    node: NodeSubject,
    delay: NonZeroSpan<D>,
    initial: LogicLevel,
    remembered_input: LogicLevel,
    committed: LogicLevel,
    input: LogicLevel,
    revision: NetworkRevision,
    at: Time<D>,
    latest_transition: CauseRef,
    current_support: CauseRef,
    pending: Option<PendingInertialDelayInspection<D>>,
    next_deadline: Option<Time<D>>,
    last_cancellation: Option<CauseRef>,
    provenance: ProvenanceView<D>,
}

impl<D> Clone for InertialDelayInspection<D> {
    fn clone(&self) -> Self {
        Self {
            node: self.node.clone(),
            delay: self.delay,
            initial: self.initial,
            remembered_input: self.remembered_input,
            committed: self.committed,
            input: self.input,
            revision: self.revision,
            at: self.at,
            latest_transition: self.latest_transition,
            current_support: self.current_support,
            pending: self.pending.clone(),
            next_deadline: self.next_deadline,
            last_cancellation: self.last_cancellation,
            provenance: self.provenance.clone(),
        }
    }
}

impl<D> InertialDelayInspection<D> {
    #[must_use]
    pub const fn node(&self) -> &NodeSubject {
        &self.node
    }
    #[must_use]
    pub const fn delay(&self) -> NonZeroSpan<D> {
        self.delay
    }
    #[must_use]
    pub const fn initial(&self) -> LogicLevel {
        self.initial
    }
    #[must_use]
    pub const fn remembered_input(&self) -> LogicLevel {
        self.remembered_input
    }
    #[must_use]
    pub const fn committed(&self) -> LogicLevel {
        self.committed
    }
    #[must_use]
    pub const fn output(&self) -> LogicLevel {
        self.committed
    }
    #[must_use]
    pub const fn input(&self) -> LogicLevel {
        self.input
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
    pub const fn latest_transition(&self) -> CauseRef {
        self.latest_transition
    }
    #[must_use]
    pub const fn current_support(&self) -> CauseRef {
        self.current_support
    }
    #[must_use]
    pub const fn pending(&self) -> Option<&PendingInertialDelayInspection<D>> {
        self.pending.as_ref()
    }
    #[must_use]
    pub const fn next_deadline(&self) -> Option<Time<D>> {
        self.next_deadline
    }
    #[must_use]
    pub const fn last_cancellation(&self) -> Option<CauseRef> {
        self.last_cancellation
    }
    #[must_use]
    pub const fn provenance(&self) -> &ProvenanceView<D> {
        &self.provenance
    }
}

/// A structural or lifecycle failure to inspect one node as InertialDelay.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InertialDelayInspectionFailure {
    UnknownNode(NodeKey),
    NotInertialDelay(NodeKey),
    NotInitialized,
}

impl InertialDelayInspectionFailure {
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
                inspection_unknown(node, InspectionSubjectKind::InertialDelay)
            }
            Self::NotInertialDelay(node) => {
                inspection_wrong_kind(node, InspectionSubjectKind::InertialDelay)
            }
            Self::NotInitialized => {
                lifecycle_not_initialized(OperationSubjectRef::MachineLifecycle)
            }
        }
    }
}

/// Structural information available for one compiled Periodic source.
pub struct PeriodicDefinitionInspection<D> {
    node: NodeKey,
    period: NonZeroSpan<D>,
    first_emission: FirstEmissionPolicy,
    reenable_phase: ReenablePhasePolicy,
}

impl<D> PeriodicDefinitionInspection<D> {
    #[must_use]
    pub const fn node(&self) -> NodeKey {
        self.node
    }
    #[must_use]
    pub const fn period(&self) -> NonZeroSpan<D> {
        self.period
    }
    #[must_use]
    pub const fn first_emission(&self) -> FirstEmissionPolicy {
        self.first_emission
    }
    #[must_use]
    pub const fn reenable_phase(&self) -> ReenablePhasePolicy {
        self.reenable_phase
    }
}

/// One owned observation of a pending Periodic phase boundary.
pub struct PendingPeriodicBoundaryInspection<D> {
    event: PendingEventKey,
    node: NodeSubject,
    stimulus: ReactionStamp<D>,
    origin: Time<D>,
    deadline: Time<D>,
    anchor: Time<D>,
    ordinal: u64,
    first_emission: FirstEmissionPolicy,
    reenable_phase: ReenablePhasePolicy,
    revision: NetworkRevision,
    cause: CauseRef,
}

impl<D> Clone for PendingPeriodicBoundaryInspection<D> {
    fn clone(&self) -> Self {
        Self {
            event: self.event,
            node: self.node.clone(),
            stimulus: self.stimulus,
            origin: self.origin,
            deadline: self.deadline,
            anchor: self.anchor,
            ordinal: self.ordinal,
            first_emission: self.first_emission,
            reenable_phase: self.reenable_phase,
            revision: self.revision,
            cause: self.cause,
        }
    }
}

impl<D> PendingPeriodicBoundaryInspection<D> {
    /// Returns the immutable originating occurrence, independent of retiming.
    #[must_use]
    pub const fn origin_stamp(&self) -> ReactionStamp<D> {
        self.stimulus
    }

    #[must_use]
    pub const fn event(&self) -> PendingEventKey {
        self.event
    }
    #[must_use]
    pub const fn node(&self) -> &NodeSubject {
        &self.node
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
    pub const fn anchor(&self) -> Time<D> {
        self.anchor
    }
    #[must_use]
    pub const fn ordinal(&self) -> u64 {
        self.ordinal
    }
    #[must_use]
    pub const fn first_emission(&self) -> FirstEmissionPolicy {
        self.first_emission
    }
    #[must_use]
    pub const fn reenable_phase(&self) -> ReenablePhasePolicy {
        self.reenable_phase
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

/// One owned ready-machine observation of Periodic phase and pending work.
pub struct PeriodicInspection<D> {
    node: NodeSubject,
    period: NonZeroSpan<D>,
    first_emission: FirstEmissionPolicy,
    reenable_phase: ReenablePhasePolicy,
    remembered_enable: LogicLevel,
    enable: LogicLevel,
    anchor: Option<Time<D>>,
    phase_origin: Option<ReactionStamp<D>>,
    settled_boundary: Option<Time<D>>,
    revision: NetworkRevision,
    at: Time<D>,
    current_support: CauseRef,
    anchor_cause: Option<CauseRef>,
    pending: Option<PendingPeriodicBoundaryInspection<D>>,
    next_deadline: Option<Time<D>>,
    last_cancellation: Option<CauseRef>,
    provenance: ProvenanceView<D>,
}

impl<D> Clone for PeriodicInspection<D> {
    fn clone(&self) -> Self {
        Self {
            node: self.node.clone(),
            period: self.period,
            first_emission: self.first_emission,
            reenable_phase: self.reenable_phase,
            remembered_enable: self.remembered_enable,
            enable: self.enable,
            anchor: self.anchor,
            phase_origin: self.phase_origin,
            settled_boundary: self.settled_boundary,
            revision: self.revision,
            at: self.at,
            current_support: self.current_support,
            anchor_cause: self.anchor_cause,
            pending: self.pending.clone(),
            next_deadline: self.next_deadline,
            last_cancellation: self.last_cancellation,
            provenance: self.provenance.clone(),
        }
    }
}

impl<D> PeriodicInspection<D> {
    #[must_use]
    pub const fn node(&self) -> &NodeSubject {
        &self.node
    }
    #[must_use]
    pub const fn period(&self) -> NonZeroSpan<D> {
        self.period
    }
    #[must_use]
    pub const fn first_emission(&self) -> FirstEmissionPolicy {
        self.first_emission
    }
    #[must_use]
    pub const fn reenable_phase(&self) -> ReenablePhasePolicy {
        self.reenable_phase
    }
    #[must_use]
    pub const fn remembered_enable(&self) -> LogicLevel {
        self.remembered_enable
    }
    #[must_use]
    pub const fn enable(&self) -> LogicLevel {
        self.enable
    }
    #[must_use]
    pub const fn anchor(&self) -> Option<Time<D>> {
        self.anchor
    }
    /// Returns the occurrence that established the current phase.
    #[must_use]
    pub const fn phase_origin(&self) -> Option<ReactionStamp<D>> {
        self.phase_origin
    }
    /// Returns the most recent emitted or suppressed boundary of this phase.
    #[must_use]
    pub const fn settled_boundary(&self) -> Option<Time<D>> {
        self.settled_boundary
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
    pub const fn current_support(&self) -> CauseRef {
        self.current_support
    }
    #[must_use]
    pub const fn anchor_cause(&self) -> Option<CauseRef> {
        self.anchor_cause
    }
    #[must_use]
    pub const fn pending(&self) -> Option<&PendingPeriodicBoundaryInspection<D>> {
        self.pending.as_ref()
    }
    #[must_use]
    pub const fn next_deadline(&self) -> Option<Time<D>> {
        self.next_deadline
    }
    #[must_use]
    pub const fn last_cancellation(&self) -> Option<CauseRef> {
        self.last_cancellation
    }
    #[must_use]
    pub const fn provenance(&self) -> &ProvenanceView<D> {
        &self.provenance
    }
}

/// A structural or lifecycle failure to inspect one node as Periodic.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PeriodicInspectionFailure {
    UnknownNode(NodeKey),
    NotPeriodic(NodeKey),
    NotInitialized,
}

impl PeriodicInspectionFailure {
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
            Self::UnknownNode(node) => inspection_unknown(node, InspectionSubjectKind::Periodic),
            Self::NotPeriodic(node) => inspection_wrong_kind(node, InspectionSubjectKind::Periodic),
            Self::NotInitialized => {
                lifecycle_not_initialized(OperationSubjectRef::MachineLifecycle)
            }
        }
    }
}

impl<D> Clone for TransportDelayInspection<D> {
    fn clone(&self) -> Self {
        Self {
            node: self.node.clone(),
            delay: self.delay,
            initial: self.initial,
            remembered_input: self.remembered_input,
            committed: self.committed,
            input: self.input,
            revision: self.revision,
            at: self.at,
            latest_transition: self.latest_transition,
            current_support: self.current_support,
            pending: self.pending.clone(),
            next_deadline: self.next_deadline,
            provenance: self.provenance.clone(),
        }
    }
}

impl<D> TransportDelayInspection<D> {
    #[must_use]
    pub const fn node(&self) -> &NodeSubject {
        &self.node
    }
    #[must_use]
    pub const fn delay(&self) -> NonZeroSpan<D> {
        self.delay
    }
    #[must_use]
    pub const fn initial(&self) -> LogicLevel {
        self.initial
    }
    #[must_use]
    pub const fn remembered_input(&self) -> LogicLevel {
        self.remembered_input
    }
    #[must_use]
    pub const fn committed(&self) -> LogicLevel {
        self.committed
    }
    #[must_use]
    pub const fn output(&self) -> LogicLevel {
        self.committed
    }
    #[must_use]
    pub const fn input(&self) -> LogicLevel {
        self.input
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
    pub const fn latest_transition(&self) -> CauseRef {
        self.latest_transition
    }
    #[must_use]
    pub const fn current_support(&self) -> CauseRef {
        self.current_support
    }
    #[must_use]
    pub fn pending(&self) -> &[PendingTransportDelayInspection<D>] {
        &self.pending
    }
    #[must_use]
    pub const fn next_deadline(&self) -> Option<Time<D>> {
        self.next_deadline
    }
    #[must_use]
    pub const fn provenance(&self) -> &ProvenanceView<D> {
        &self.provenance
    }
}

/// A structural or lifecycle failure to inspect one node as TransportDelay.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransportDelayInspectionFailure {
    UnknownNode(NodeKey),
    NotTransportDelay(NodeKey),
    NotInitialized,
}

impl TransportDelayInspectionFailure {
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
                inspection_unknown(node, InspectionSubjectKind::TransportDelay)
            }
            Self::NotTransportDelay(node) => {
                inspection_wrong_kind(node, InspectionSubjectKind::TransportDelay)
            }
            Self::NotInitialized => {
                lifecycle_not_initialized(OperationSubjectRef::MachineLifecycle)
            }
        }
    }
}

impl<D> PulseDelayInspection<D> {
    /// Retains the provenance resolving every cause in this owned observation.
    #[must_use]
    pub const fn provenance(&self) -> &ProvenanceView<D> {
        &self.provenance
    }

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
    stimulus: ReactionStamp<D>,
    origin: Time<D>,
    deadline: Time<D>,
    count: PulseCount,
    cause: CauseRef,
}

impl<D> ModulePendingPulseDelayInspection<D> {
    /// Returns immutable originating occurrence.
    #[must_use]
    pub const fn origin_stamp(&self) -> ReactionStamp<D> {
        self.stimulus
    }
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
    pulse_set_reset_state: Option<LogicLevel>,
    pulse_set_reset_cause: Option<CauseRef>,
    level_set_reset_state: Option<LogicLevel>,
    level_set_reset_cause: Option<CauseRef>,
    sample_hold: Option<SampleHoldInspection<D>>,
    transport_delay: Option<TransportDelayInspection<D>>,
    inertial_delay: Option<InertialDelayInspection<D>>,
    periodic: Option<PeriodicInspection<D>>,
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
    /// Returns committed stored state for a pulse set/reset latch.
    #[must_use]
    pub const fn pulse_set_reset_state(&self) -> Option<LogicLevel> {
        self.pulse_set_reset_state
    }
    /// Returns the retained cause of the latest state-establishing latch control.
    #[must_use]
    pub const fn pulse_set_reset_cause(&self) -> Option<CauseRef> {
        self.pulse_set_reset_cause
    }
    /// Returns committed stored state for a level set/reset latch.
    #[must_use]
    pub const fn level_set_reset_state(&self) -> Option<LogicLevel> {
        self.level_set_reset_state
    }
    /// Returns its latest state-establishing cause.
    #[must_use]
    pub const fn level_set_reset_cause(&self) -> Option<CauseRef> {
        self.level_set_reset_cause
    }
    /// Returns this instance's owned held-state and capture evidence, when initialized.
    #[must_use]
    pub const fn sample_hold(&self) -> Option<&SampleHoldInspection<D>> {
        self.sample_hold.as_ref()
    }
    #[must_use]
    pub const fn transport_delay(&self) -> Option<&TransportDelayInspection<D>> {
        self.transport_delay.as_ref()
    }
    #[must_use]
    pub const fn inertial_delay(&self) -> Option<&InertialDelayInspection<D>> {
        self.inertial_delay.as_ref()
    }
    #[must_use]
    pub const fn periodic(&self) -> Option<&PeriodicInspection<D>> {
        self.periodic.as_ref()
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
    stateful_standard: Option<crate::standard::StatefulStandardInspection<D>>,
    revision: NetworkRevision,
    at: Option<Time<D>>,
    inputs: Vec<ModuleInputInspection>,
    outputs: Vec<ModuleOutputInspection>,
    nodes: Vec<ModuleNodeInspection<D>>,
    connections: Vec<QualifiedConnectionRef>,
    modules: Vec<QualifiedModuleRef>,
    provenance: ProvenanceView<D>,
}

impl<D> ModuleInspection<D> {
    /// Retains the provenance resolving every cause in this owned observation.
    #[must_use]
    pub const fn provenance(&self) -> &ProvenanceView<D> {
        &self.provenance
    }

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
    /// Returns aggregate state and explicitly historical reaction facts for a stateful standard module.
    #[must_use]
    pub fn stateful_standard(&self) -> Option<&crate::standard::StatefulStandardInspection<D>> {
        self.stateful_standard.as_ref()
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
                    requested.clone(),
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

// SPEC: docs/specs/contracts/ordered-reactions.yaml "singular-inertial-and-once-only-phase"
// Cadence/anchor and phase identity are separate; settled includes suppressed boundaries.
#[derive(Debug)]
pub(crate) struct PeriodicPhase<D> {
    pub(crate) anchor: Time<D>,
    pub(crate) origin: ReactionStamp<D>,
    pub(crate) settled: Option<Time<D>>,
}
impl<D> Clone for PeriodicPhase<D> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<D> Copy for PeriodicPhase<D> {}
impl<D> PartialEq for PeriodicPhase<D> {
    fn eq(&self, other: &Self) -> bool {
        self.anchor == other.anchor && self.origin == other.origin && self.settled == other.settled
    }
}
impl<D> Eq for PeriodicPhase<D> {}

pub(crate) struct MachineStore<D> {
    // Explicit last-reaction history, never input to network evaluation.
    pub(crate) standard_history:
        BTreeMap<QualifiedModuleRef, crate::standard::stateful::StandardHistory>,
    pub(crate) status: MachineStatus<D>,
    pub(crate) last_reaction: Option<ReactionStamp<D>>,
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
    pub(crate) stored_levels: Vec<LogicLevel>,
    pub(crate) toggle_inversion_causes: BTreeMap<NodeKey, CauseRef>,
    pub(crate) establishment_causes: BTreeMap<NodeKey, CauseRef>,
    pub(crate) transport_transition_causes: BTreeMap<NodeKey, CauseRef>,
    pub(crate) inertial_cancellation_causes: BTreeMap<NodeKey, CauseRef>,
    pub(crate) periodic_anchors: BTreeMap<NodeKey, PeriodicPhase<D>>,
    pub(crate) periodic_anchor_causes: BTreeMap<NodeKey, CauseRef>,
    pub(crate) periodic_cancellation_causes: BTreeMap<NodeKey, CauseRef>,
    pub(crate) active_episodes: crate::episode::ActiveEpisodes<D>,
    pub(crate) pending_events: BTreeMap<Time<D>, Vec<PendingEvent<D>>>,
    pub(crate) next_pending_event_serial: u64,
}

impl<D> Clone for MachineStore<D> {
    fn clone(&self) -> Self {
        Self {
            standard_history: self.standard_history.clone(),
            status: self.status,
            last_reaction: self.last_reaction,
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
            stored_levels: self.stored_levels.clone(),
            toggle_inversion_causes: self.toggle_inversion_causes.clone(),
            establishment_causes: self.establishment_causes.clone(),
            transport_transition_causes: self.transport_transition_causes.clone(),
            inertial_cancellation_causes: self.inertial_cancellation_causes.clone(),
            periodic_anchors: self.periodic_anchors.clone(),
            periodic_anchor_causes: self.periodic_anchor_causes.clone(),
            periodic_cancellation_causes: self.periodic_cancellation_causes.clone(),
            active_episodes: self.active_episodes.clone(),
            pending_events: self.pending_events.clone(),
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

/// Read-only unpublished candidate produced by [`Machine::forecast`](crate::Machine::forecast).
///
/// ```compile_fail
/// use mossignal::{ForecastState, Transaction};
/// fn commit<D>(state: &mut ForecastState<D>, transaction: Transaction<D>) {
///     let _ = state.apply(transaction);
/// }
/// ```
pub struct ForecastState<D> {
    // SPEC: docs/specs/contracts/transaction-forecast.yaml "hypothetical-only"
    // No apply, mutation, or conversion into the live machine.
    pub(crate) machine: Machine<D>,
}

impl<D> ForecastState<D> {
    pub(crate) fn from_candidate(machine: Machine<D>) -> Self {
        Self { machine }
    }

    /// Returns the candidate lifecycle state.
    #[must_use]
    pub fn status(&self) -> MachineStatus<D> {
        self.machine.status()
    }

    /// Returns whether the candidate has committed initialization.
    #[must_use]
    pub fn is_initialized(&self) -> bool {
        self.machine.is_initialized()
    }

    /// Returns the candidate logical time, or `None` before initialization.
    #[must_use]
    pub fn now(&self) -> Option<Time<D>> {
        self.machine.now()
    }

    /// Returns the candidate last occurrence without modifying the live machine.
    #[must_use]
    pub fn last_reaction(&self) -> Option<ReactionStamp<D>> {
        self.machine.last_reaction()
    }

    /// Returns the candidate topology revision.
    #[must_use]
    pub fn revision(&self) -> NetworkRevision {
        self.machine.revision()
    }

    /// Returns the semantic fingerprint of the candidate topology.
    #[must_use]
    pub fn fingerprint(&self) -> NetworkFingerprint {
        self.machine.fingerprint()
    }

    /// Returns the candidate execution-state digest.
    #[must_use]
    pub fn execution_state_digest(&self) -> ExecutionStateDigest {
        self.machine.execution_state_digest()
    }

    /// Returns the candidate observable-state digest.
    #[must_use]
    pub fn observable_state_digest(&self) -> ObservableStateDigest {
        self.machine.observable_state_digest()
    }

    /// Returns an owned snapshot of the unpublished candidate.
    ///
    /// Encoding or restoring that snapshot is a separate explicit operation.
    #[must_use]
    pub fn snapshot(&self) -> crate::persistence::MachineSnapshot<D> {
        self.machine.snapshot()
    }

    /// Returns the immutable compiled topology installed in the candidate.
    #[must_use]
    pub fn compiled(&self) -> &CompiledNetwork<D> {
        self.machine.compiled()
    }

    /// Returns the runtime policy the candidate was forecast under.
    #[must_use]
    pub fn runtime_policy(&self) -> &RuntimePolicy {
        self.machine.runtime_policy()
    }

    /// Returns the semantic identity of that runtime policy.
    #[must_use]
    pub fn runtime_policy_id(&self) -> RuntimePolicyId {
        self.machine.runtime_policy_id()
    }

    /// Returns the least pending deadline without changing the candidate.
    pub fn next_deadline(&self) -> Result<Option<Time<D>>, ScheduleFailure> {
        self.machine.next_deadline()
    }

    /// Returns whether the candidate is dormant or must be called at a deadline.
    pub fn schedule(&self) -> Result<Schedule<D>, ScheduleFailure> {
        self.machine.schedule()
    }

    /// Projects the same PulseDelay definition as [`Machine::inspect_pulse_delay_definition`].
    pub fn inspect_pulse_delay_definition(
        &self,
        node: NodeKey,
    ) -> Result<PulseDelayDefinitionInspection<D>, PulseDelayInspectionFailure> {
        self.machine.inspect_pulse_delay_definition(node)
    }

    /// Projects the same PulseDelay state as [`Machine::inspect_pulse_delay`].
    pub fn inspect_pulse_delay(
        &self,
        node: NodeKey,
    ) -> Result<PulseDelayInspection<D>, PulseDelayInspectionFailure> {
        self.machine.inspect_pulse_delay(node)
    }

    /// Projects the same TransportDelay definition as [`Machine::inspect_transport_delay_definition`].
    pub fn inspect_transport_delay_definition(
        &self,
        node: NodeKey,
    ) -> Result<TransportDelayDefinitionInspection<D>, TransportDelayInspectionFailure> {
        self.machine.inspect_transport_delay_definition(node)
    }

    /// Projects the same TransportDelay state as [`Machine::inspect_transport_delay`].
    pub fn inspect_transport_delay(
        &self,
        node: NodeKey,
    ) -> Result<TransportDelayInspection<D>, TransportDelayInspectionFailure> {
        self.machine.inspect_transport_delay(node)
    }

    /// Projects the same InertialDelay definition as [`Machine::inspect_inertial_delay_definition`].
    pub fn inspect_inertial_delay_definition(
        &self,
        node: NodeKey,
    ) -> Result<InertialDelayDefinitionInspection<D>, InertialDelayInspectionFailure> {
        self.machine.inspect_inertial_delay_definition(node)
    }

    /// Projects the same InertialDelay state as [`Machine::inspect_inertial_delay`].
    pub fn inspect_inertial_delay(
        &self,
        node: NodeKey,
    ) -> Result<InertialDelayInspection<D>, InertialDelayInspectionFailure> {
        self.machine.inspect_inertial_delay(node)
    }

    /// Projects the same Periodic definition as [`Machine::inspect_periodic_definition`].
    pub fn inspect_periodic_definition(
        &self,
        node: NodeKey,
    ) -> Result<PeriodicDefinitionInspection<D>, PeriodicInspectionFailure> {
        self.machine.inspect_periodic_definition(node)
    }

    /// Projects the same Periodic state as [`Machine::inspect_periodic`].
    pub fn inspect_periodic(
        &self,
        node: NodeKey,
    ) -> Result<PeriodicInspection<D>, PeriodicInspectionFailure> {
        self.machine.inspect_periodic(node)
    }

    /// Projects the same module inspection as [`Machine::inspect_module`].
    pub fn inspect_module(
        &self,
        instance: ModuleInstanceKey,
    ) -> Result<ModuleInspection<D>, ModuleInspectionFailure> {
        self.machine.inspect_module(instance)
    }

    /// Projects the same qualified-module inspection as [`Machine::inspect_qualified_module`].
    pub fn inspect_qualified_module(
        &self,
        module: QualifiedModuleRef,
    ) -> Result<ModuleInspection<D>, ModuleInspectionFailure> {
        self.machine.inspect_qualified_module(module)
    }

    /// Projects the same edge-detector definition as [`Machine::inspect_edge_detector_definition`].
    pub fn inspect_edge_detector_definition(
        &self,
        node: NodeKey,
    ) -> Result<EdgeDetectorDefinitionInspection, EdgeDetectorInspectionFailure> {
        self.machine.inspect_edge_detector_definition(node)
    }

    /// Projects the same edge-detector state as [`Machine::inspect_edge_detector`].
    pub fn inspect_edge_detector(
        &self,
        node: NodeKey,
    ) -> Result<EdgeDetectorInspection<D>, EdgeDetectorInspectionFailure> {
        self.machine.inspect_edge_detector(node)
    }

    /// Projects the same Toggle definition as [`Machine::inspect_toggle_definition`].
    pub fn inspect_toggle_definition(
        &self,
        node: NodeKey,
    ) -> Result<ToggleDefinitionInspection, ToggleInspectionFailure> {
        self.machine.inspect_toggle_definition(node)
    }

    /// Projects the same Toggle state as [`Machine::inspect_toggle`].
    pub fn inspect_toggle(
        &self,
        node: NodeKey,
    ) -> Result<ToggleInspection<D>, ToggleInspectionFailure> {
        self.machine.inspect_toggle(node)
    }

    /// Projects the same pulse latch definition as [`Machine::inspect_pulse_set_reset_latch_definition`].
    pub fn inspect_pulse_set_reset_latch_definition(
        &self,
        node: NodeKey,
    ) -> Result<PulseSetResetLatchDefinitionInspection, PulseSetResetLatchInspectionFailure> {
        self.machine.inspect_pulse_set_reset_latch_definition(node)
    }

    /// Projects the same pulse latch state as [`Machine::inspect_pulse_set_reset_latch`].
    pub fn inspect_pulse_set_reset_latch(
        &self,
        node: NodeKey,
    ) -> Result<PulseSetResetLatchInspection<D>, PulseSetResetLatchInspectionFailure> {
        self.machine.inspect_pulse_set_reset_latch(node)
    }

    /// Projects the same level latch definition as [`Machine::inspect_level_set_reset_latch_definition`].
    pub fn inspect_level_set_reset_latch_definition(
        &self,
        node: NodeKey,
    ) -> Result<LevelSetResetLatchDefinitionInspection, LevelSetResetLatchInspectionFailure> {
        self.machine.inspect_level_set_reset_latch_definition(node)
    }

    /// Projects the same level latch state as [`Machine::inspect_level_set_reset_latch`].
    pub fn inspect_level_set_reset_latch(
        &self,
        node: NodeKey,
    ) -> Result<LevelSetResetLatchInspection<D>, LevelSetResetLatchInspectionFailure> {
        self.machine.inspect_level_set_reset_latch(node)
    }

    /// Projects the same SampleHold definition as [`Machine::inspect_sample_hold_definition`].
    pub fn inspect_sample_hold_definition(
        &self,
        node: NodeKey,
    ) -> Result<SampleHoldDefinitionInspection, SampleHoldInspectionFailure> {
        self.machine.inspect_sample_hold_definition(node)
    }

    /// Projects the same SampleHold state as [`Machine::inspect_sample_hold`].
    pub fn inspect_sample_hold(
        &self,
        node: NodeKey,
    ) -> Result<SampleHoldInspection<D>, SampleHoldInspectionFailure> {
        self.machine.inspect_sample_hold(node)
    }
}

impl<D> fmt::Debug for ForecastState<D> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ForecastState")
            .field("status", &self.status())
            .field("revision", &self.revision())
            .field("execution_state_digest", &self.execution_state_digest())
            .finish()
    }
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
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EdgeDetectorInspection<D> {
    node: NodeKey,
    detector: EdgeDetectorKind,
    initialization: EdgeInitialization,
    committed: EdgeObservation,
    input: LogicLevel,
    observation_cause: CauseRef,
    revision: NetworkRevision,
    at: Time<D>,
    provenance: ProvenanceView<D>,
}

impl<D> EdgeDetectorInspection<D> {
    /// Retains the provenance resolving every cause in this owned observation.
    #[must_use]
    pub const fn provenance(&self) -> &ProvenanceView<D> {
        &self.provenance
    }

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
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToggleInspection<D> {
    node: NodeKey,
    initial: LogicLevel,
    committed: LogicLevel,
    revision: NetworkRevision,
    at: Time<D>,
    latest_inversion: Option<CauseRef>,
    provenance: ProvenanceView<D>,
}

impl<D> ToggleInspection<D> {
    /// Retains the provenance resolving every cause in this owned observation.
    #[must_use]
    pub const fn provenance(&self) -> &ProvenanceView<D> {
        &self.provenance
    }

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

/// Structural information available for one compiled pulse set/reset latch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PulseSetResetLatchDefinitionInspection {
    node: NodeKey,
    initial: LogicLevel,
    conflict: ConflictPolicy,
}

impl PulseSetResetLatchDefinitionInspection {
    /// Returns the stable node key.
    #[must_use]
    pub const fn node(&self) -> NodeKey {
        self.node
    }
    /// Returns the declared initial level.
    #[must_use]
    pub const fn initial(&self) -> LogicLevel {
        self.initial
    }
    /// Returns the simultaneous-control conflict policy.
    #[must_use]
    pub const fn conflict(&self) -> ConflictPolicy {
        self.conflict
    }
}

/// One owned observation of committed pulse set/reset latch state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PulseSetResetLatchInspection<D> {
    node: NodeKey,
    initial: LogicLevel,
    conflict: ConflictPolicy,
    committed: LogicLevel,
    revision: NetworkRevision,
    at: Time<D>,
    latest_establishment: CauseRef,
    provenance: ProvenanceView<D>,
}

impl<D> PulseSetResetLatchInspection<D> {
    /// Retains the provenance resolving every cause in this owned observation.
    #[must_use]
    pub const fn provenance(&self) -> &ProvenanceView<D> {
        &self.provenance
    }

    /// Returns the stable node key.
    #[must_use]
    pub const fn node(&self) -> NodeKey {
        self.node
    }
    /// Returns the declared initial level.
    #[must_use]
    pub const fn initial(&self) -> LogicLevel {
        self.initial
    }
    /// Returns the simultaneous-control conflict policy.
    #[must_use]
    pub const fn conflict(&self) -> ConflictPolicy {
        self.conflict
    }
    /// Returns the committed stored level.
    #[must_use]
    pub const fn committed(&self) -> LogicLevel {
        self.committed
    }
    /// Returns the observed topology revision.
    #[must_use]
    pub const fn revision(&self) -> NetworkRevision {
        self.revision
    }
    /// Returns the logical time of the committed observation.
    #[must_use]
    pub const fn at(&self) -> Time<D> {
        self.at
    }
    /// Returns the cause that most recently established the stored level.
    #[must_use]
    pub const fn latest_establishment(&self) -> CauseRef {
        self.latest_establishment
    }
}

/// A structural or lifecycle failure to inspect one pulse set/reset latch.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PulseSetResetLatchInspectionFailure {
    /// The node key is not part of the compiled topology.
    UnknownNode(NodeKey),
    /// The node exists but is not a pulse set/reset latch.
    NotPulseSetResetLatch(NodeKey),
    /// Runtime state is unavailable before machine initialization.
    NotInitialized,
}

impl PulseSetResetLatchInspectionFailure {
    /// Returns the exact catalogue code for this failure.
    #[must_use]
    pub fn code(self) -> DiagnosticCode {
        self.problem::<()>().code()
    }
    /// Returns the catalogue-fixed severity.
    #[must_use]
    pub fn severity(self) -> Severity {
        self.code().severity()
    }
    /// Returns the catalogue-fixed responsibility.
    #[must_use]
    pub fn responsibility(self) -> Responsibility {
        self.code().responsibility()
    }
    /// Projects the failure into the common problem kernel.
    #[must_use]
    pub fn problem<D>(self) -> Problem<D> {
        match self {
            Self::UnknownNode(node) => {
                inspection_unknown(node, InspectionSubjectKind::PulseSetResetLatch)
            }
            Self::NotPulseSetResetLatch(node) => {
                inspection_wrong_kind(node, InspectionSubjectKind::PulseSetResetLatch)
            }
            Self::NotInitialized => {
                lifecycle_not_initialized(OperationSubjectRef::MachineLifecycle)
            }
        }
    }
}

/// Structural information available for one compiled level set/reset latch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LevelSetResetLatchDefinitionInspection {
    node: NodeKey,
    initial: LogicLevel,
    conflict: ConflictPolicy,
}

impl LevelSetResetLatchDefinitionInspection {
    /// Returns the stable node key.
    #[must_use]
    pub const fn node(&self) -> NodeKey {
        self.node
    }
    /// Returns the declared initial level.
    #[must_use]
    pub const fn initial(&self) -> LogicLevel {
        self.initial
    }
    /// Returns the simultaneous-control conflict policy.
    #[must_use]
    pub const fn conflict(&self) -> ConflictPolicy {
        self.conflict
    }
}

/// One owned observation of committed level set/reset latch state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LevelSetResetLatchInspection<D> {
    node: NodeKey,
    initial: LogicLevel,
    conflict: ConflictPolicy,
    committed: LogicLevel,
    set: LogicLevel,
    reset: LogicLevel,
    revision: NetworkRevision,
    at: Time<D>,
    latest_establishment: CauseRef,
    provenance: ProvenanceView<D>,
}

impl<D> LevelSetResetLatchInspection<D> {
    /// Retains the provenance resolving every cause in this owned observation.
    #[must_use]
    pub const fn provenance(&self) -> &ProvenanceView<D> {
        &self.provenance
    }

    /// Returns the stable node key.
    #[must_use]
    pub const fn node(&self) -> NodeKey {
        self.node
    }
    /// Returns the declared initial level.
    #[must_use]
    pub const fn initial(&self) -> LogicLevel {
        self.initial
    }
    /// Returns the simultaneous-control conflict policy.
    #[must_use]
    pub const fn conflict(&self) -> ConflictPolicy {
        self.conflict
    }
    /// Returns the committed stored level.
    #[must_use]
    pub const fn committed(&self) -> LogicLevel {
        self.committed
    }
    /// Returns the fully settled current set control.
    #[must_use]
    pub const fn set(&self) -> LogicLevel {
        self.set
    }
    /// Returns the fully settled current reset control.
    #[must_use]
    pub const fn reset(&self) -> LogicLevel {
        self.reset
    }
    /// Returns the observed topology revision.
    #[must_use]
    pub const fn revision(&self) -> NetworkRevision {
        self.revision
    }
    /// Returns the logical time of the committed observation.
    #[must_use]
    pub const fn at(&self) -> Time<D> {
        self.at
    }
    /// Returns the cause that most recently established the stored level.
    #[must_use]
    pub const fn latest_establishment(&self) -> CauseRef {
        self.latest_establishment
    }
}

/// A structural or lifecycle failure to inspect one level set/reset latch.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LevelSetResetLatchInspectionFailure {
    /// The node key is not part of the compiled topology.
    UnknownNode(NodeKey),
    /// The node exists but is not a level set/reset latch.
    NotLevelSetResetLatch(NodeKey),
    /// Runtime state is unavailable before machine initialization.
    NotInitialized,
}

impl LevelSetResetLatchInspectionFailure {
    /// Returns the exact catalogue code for this failure.
    #[must_use]
    pub fn code(self) -> DiagnosticCode {
        self.problem::<()>().code()
    }
    /// Returns the catalogue-fixed severity.
    #[must_use]
    pub fn severity(self) -> Severity {
        self.code().severity()
    }
    /// Returns the catalogue-fixed responsibility.
    #[must_use]
    pub fn responsibility(self) -> Responsibility {
        self.code().responsibility()
    }
    /// Projects the failure into the common problem kernel.
    #[must_use]
    pub fn problem<D>(self) -> Problem<D> {
        match self {
            Self::UnknownNode(node) => {
                inspection_unknown(node, InspectionSubjectKind::LevelSetResetLatch)
            }
            Self::NotLevelSetResetLatch(node) => {
                inspection_wrong_kind(node, InspectionSubjectKind::LevelSetResetLatch)
            }
            Self::NotInitialized => {
                lifecycle_not_initialized(OperationSubjectRef::MachineLifecycle)
            }
        }
    }
}

/// Structural information available for one compiled SampleHold.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SampleHoldDefinitionInspection {
    node: NodeKey,
    initial: LogicLevel,
}

impl SampleHoldDefinitionInspection {
    /// Returns the stable node key.
    #[must_use]
    pub const fn node(&self) -> NodeKey {
        self.node
    }
    /// Returns the declared initial level.
    #[must_use]
    pub const fn initial(&self) -> LogicLevel {
        self.initial
    }
}

/// An owned held-value observation with immutable current and establishment provenance.
/// Sample pulse counts appear only in the retained causal evidence of their reaction.
pub struct SampleHoldInspection<D> {
    node: NodeSubject,
    initial: LogicLevel,
    committed: LogicLevel,
    value: LogicLevel,
    revision: NetworkRevision,
    at: Time<D>,
    latest_establishment: CauseRef,
    current_support: CauseRef,
    provenance: ProvenanceView<D>,
}

impl<D> Clone for SampleHoldInspection<D> {
    fn clone(&self) -> Self {
        Self {
            node: self.node.clone(),
            initial: self.initial,
            committed: self.committed,
            value: self.value,
            revision: self.revision,
            at: self.at,
            latest_establishment: self.latest_establishment,
            current_support: self.current_support,
            provenance: self.provenance.clone(),
        }
    }
}

impl<D> fmt::Debug for SampleHoldInspection<D> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SampleHoldInspection")
            .field("node", &self.node)
            .field("initial", &self.initial)
            .field("committed", &self.committed)
            .field("value", &self.value)
            .field("revision", &self.revision)
            .field("at_ticks", &self.at.ticks())
            .field("latest_establishment", &self.latest_establishment)
            .field("current_support", &self.current_support)
            .finish_non_exhaustive()
    }
}

impl<D> SampleHoldInspection<D> {
    /// Returns the direct or fully qualified stable owner.
    #[must_use]
    pub const fn node(&self) -> &NodeSubject {
        &self.node
    }
    /// Returns the declared initial held value.
    #[must_use]
    pub const fn initial(&self) -> LogicLevel {
        self.initial
    }
    /// Returns the committed held value, also the settled output.
    #[must_use]
    pub const fn committed(&self) -> LogicLevel {
        self.committed
    }
    /// Returns the current settled Level input, which may differ from the held value.
    #[must_use]
    pub const fn value(&self) -> LogicLevel {
        self.value
    }
    /// Returns the revision of this observation.
    #[must_use]
    pub const fn revision(&self) -> NetworkRevision {
        self.revision
    }
    /// Returns the logical time of this observation.
    #[must_use]
    pub const fn at(&self) -> Time<D> {
        self.at
    }
    /// Returns the last capture or initial establishment, resolved by this owned view.
    #[must_use]
    pub const fn latest_establishment(&self) -> CauseRef {
        self.latest_establishment
    }
    /// Returns the current reaction's support, resolved by this owned view.
    #[must_use]
    pub const fn current_support(&self) -> CauseRef {
        self.current_support
    }
    /// Retains both causes even after later machine transactions.
    #[must_use]
    pub const fn provenance(&self) -> &ProvenanceView<D> {
        &self.provenance
    }
}

/// A structural or lifecycle failure to inspect one SampleHold.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SampleHoldInspectionFailure {
    /// The node key is not part of the compiled topology.
    UnknownNode(NodeKey),
    /// The node exists but is not a SampleHold.
    NotSampleHold(NodeKey),
    /// Runtime state is unavailable before machine initialization.
    NotInitialized,
}

impl SampleHoldInspectionFailure {
    /// Returns the exact catalogue code for this failure.
    #[must_use]
    pub fn code(self) -> DiagnosticCode {
        self.problem::<()>().code()
    }
    /// Returns the catalogue-fixed severity.
    #[must_use]
    pub fn severity(self) -> Severity {
        self.code().severity()
    }
    /// Returns the catalogue-fixed responsibility.
    #[must_use]
    pub fn responsibility(self) -> Responsibility {
        self.code().responsibility()
    }
    /// Projects the failure into the common problem kernel.
    #[must_use]
    pub fn problem<D>(self) -> Problem<D> {
        match self {
            Self::UnknownNode(node) => inspection_unknown(node, InspectionSubjectKind::SampleHold),
            Self::NotSampleHold(node) => {
                inspection_wrong_kind(node, InspectionSubjectKind::SampleHold)
            }
            Self::NotInitialized => {
                lifecycle_not_initialized(OperationSubjectRef::MachineLifecycle)
            }
        }
    }
}

/// A lifecycle failure to inspect active runtime diagnostic episodes.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticEpisodeInspectionFailure {
    /// Active conditions are unavailable before initialization.
    NotInitialized,
}
impl DiagnosticEpisodeInspectionFailure {
    /// Returns the exact catalogue code.
    #[must_use]
    pub const fn code(self) -> DiagnosticCode {
        DiagnosticCode::LifecycleNotInitialized
    }
    /// Returns the catalogue severity.
    #[must_use]
    pub fn severity(self) -> Severity {
        self.code().severity()
    }
    /// Returns the catalogue responsibility.
    #[must_use]
    pub fn responsibility(self) -> Responsibility {
        self.code().responsibility()
    }
    /// Returns the structured lifecycle problem.
    #[must_use]
    pub fn problem<D>(self) -> Problem<D> {
        lifecycle_not_initialized(OperationSubjectRef::MachineLifecycle)
    }
}

impl<D> Machine<D> {
    /// Inspects active conditions in deterministic stable-owner order.
    /// Each owned record retains the provenance needed to explain its current evidence.
    pub fn active_diagnostic_episodes(
        &self,
    ) -> Result<Vec<crate::ActiveDiagnosticEpisode<D>>, DiagnosticEpisodeInspectionFailure> {
        if !self.is_initialized() {
            return Err(DiagnosticEpisodeInspectionFailure::NotInitialized);
        }
        Ok(self.store.active_episodes.values().cloned().collect())
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
        requested.clone(),
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
        requested.clone(),
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
        let stored_levels = compiled.initial_stored_levels();
        Self {
            compiled,
            policy,
            store: MachineStore {
                standard_history: BTreeMap::new(),
                status: MachineStatus::AwaitingInitialization,
                last_reaction: None,
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
                stored_levels,
                toggle_inversion_causes: BTreeMap::new(),
                establishment_causes: BTreeMap::new(),
                transport_transition_causes: BTreeMap::new(),
                inertial_cancellation_causes: BTreeMap::new(),
                periodic_anchors: BTreeMap::new(),
                periodic_anchor_causes: BTreeMap::new(),
                periodic_cancellation_causes: BTreeMap::new(),
                active_episodes: BTreeMap::new(),
                pending_events: BTreeMap::new(),
                next_pending_event_serial: 0,
            },
        }
    }

    pub(crate) fn duplicate_for_forecast(&self) -> Self {
        // SPEC: docs/specs/contracts/transaction-forecast.yaml "shared-transition"
        // Forecast calls ordinary apply on this private copy. Machine stays uncloneable.
        Self {
            compiled: self.compiled.clone(),
            policy: self.policy.clone(),
            store: self.store.clone(),
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

    /// Returns the last committed occurrence, or absence before initialization.
    #[must_use]
    pub const fn last_reaction(&self) -> Option<ReactionStamp<D>> {
        self.store.last_reaction
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

    /// Returns the execution-state digest of the committed machine.
    ///
    /// The query is a pure projection and does not change the machine.
    ///
    /// ```compile_fail
    /// use mossignal::{ExecutionStateDigest, ObservableStateDigest};
    /// fn accepts(_: ExecutionStateDigest) {}
    /// fn reject(value: ObservableStateDigest) {
    ///     accepts(value);
    /// }
    /// ```
    #[must_use]
    pub fn execution_state_digest(&self) -> ExecutionStateDigest {
        crate::state_digest::execution_state_digest(self)
    }

    /// Returns the observable-state digest of the committed machine.
    ///
    /// The query is a pure projection and does not change the machine.
    #[must_use]
    pub fn observable_state_digest(&self) -> ObservableStateDigest {
        crate::state_digest::observable_state_digest(self)
    }

    /// Returns an owned snapshot of this committed machine.
    ///
    /// Creation reads one complete committed version. It does not borrow or
    /// mutate the machine. Encoding the returned value is
    /// [`encode_snapshot`](crate::encode_snapshot).
    ///
    /// ```compile_fail
    /// use mossignal::{ExecutionStateDigest, Machine, SnapshotDigest};
    /// fn accepts(_: SnapshotDigest) {}
    /// fn reject<D>(machine: &Machine<D>) {
    ///     accepts(machine.execution_state_digest());
    /// }
    /// ```
    #[must_use]
    pub fn snapshot(&self) -> crate::persistence::MachineSnapshot<D> {
        crate::persistence::snapshot_from_machine(self)
    }

    #[cfg(test)]
    pub(crate) fn reverse_pending_batches_for_test(&mut self) {
        for batch in self.store.pending_events.values_mut() {
            batch.reverse();
        }
    }

    #[cfg(test)]
    pub(crate) fn pending_keys_in_storage_order(&self) -> Vec<u64> {
        self.store
            .pending_events
            .values()
            .flat_map(|batch| batch.iter().map(|event| event.identity().0.value()))
            .collect()
    }

    #[cfg(test)]
    pub(crate) fn reverse_provenance_supporters_for_test(&mut self)
    where
        D: Clone,
    {
        if let Some(view) = &mut self.store.provenance {
            view.reverse_unordered_supporters();
        }
    }

    #[cfg(test)]
    pub(crate) fn supporter_ordinals_for_test(&self) -> Vec<u32> {
        match &self.store.provenance {
            Some(view) => view.supporter_ordinals(),
            None => Vec::new(),
        }
    }

    #[cfg(test)]
    pub(crate) fn clear_standard_history_for_test(&mut self) {
        self.store.standard_history.clear();
    }

    #[cfg(test)]
    pub(crate) fn standard_history_len_for_test(&self) -> usize {
        self.store.standard_history.len()
    }

    #[cfg(test)]
    pub(crate) fn set_output_baseline_for_test(
        &mut self,
        output: ExternalOutputKey<Level>,
        level: LogicLevel,
    ) {
        self.store.output_baselines.insert(output, level);
    }

    #[cfg(test)]
    pub(crate) fn rebuild_episodes_reversed_for_test(&mut self) {
        let episodes = std::mem::take(&mut self.store.active_episodes);
        let mut pairs: Vec<_> = episodes.into_iter().collect();
        pairs.reverse();
        self.store.active_episodes = pairs.into_iter().collect();
    }

    /// Returns the immutable compiled topology installed in this machine.
    #[must_use]
    pub fn compiled(&self) -> &CompiledNetwork<D> {
        &self.compiled
    }

    /// Starts an owned patch bound to this machine's current revision.
    ///
    /// The builder does not borrow the machine after it is returned.
    #[must_use]
    pub fn patch(&self) -> crate::patch::NetworkPatchBuilder<D> {
        self.compiled.patch(self.store.revision)
    }

    /// Prepares a patch when its base revision is this machine's current revision.
    ///
    /// The check reads the revision, then delegates to the compiled topology.
    /// Preparation does not read or mutate runtime state.
    pub fn prepare_patch(
        &self,
        patch: crate::patch::NetworkPatch<D>,
    ) -> crate::diagnostics::Report<crate::patch::PreparedPatch<D>, D>
    where
        D: PartialEq,
    {
        if patch.base_revision() != self.revision() {
            return crate::patch::revision_mismatch(
                self.compiled.network_key(),
                patch.base_revision(),
                self.revision(),
            );
        }
        self.compiled.prepare_patch(patch)
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
        Ok(self.store.pending_events.keys().next().copied())
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
            .pending_events
            .values()
            .flatten()
            .filter_map(|event| match event {
                PendingEvent::PulseDelay(event) if event.node == node => {
                    Some(PendingPulseDelayInspection {
                        event: event.key,
                        node: event.node,
                        stimulus: event.stimulus,
                        origin: event.origin,
                        deadline: event.deadline,
                        count: event.count,
                        revision: event.revision,
                        cause: event.cause,
                    })
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        pending.sort_by_key(|event| (event.deadline, event.event));
        let next_deadline = pending.first().map(PendingPulseDelayInspection::deadline);
        Ok(PulseDelayInspection {
            provenance: self
                .store
                .provenance
                .clone()
                .unwrap_or_else(|| panic!("ready inspections must retain committed provenance")),
            node,
            delay: definition.delay,
            revision: self.store.revision,
            at: now,
            pending,
            next_deadline,
        })
    }

    /// Returns TransportDelay's immutable definition in either lifecycle phase.
    pub fn inspect_transport_delay_definition(
        &self,
        node: NodeKey,
    ) -> Result<TransportDelayDefinitionInspection<D>, TransportDelayInspectionFailure> {
        if self.compiled.qualified_node(node).is_some() {
            return Err(TransportDelayInspectionFailure::UnknownNode(node));
        }
        let Some((delay, _, _, initial)) = self.compiled.transport_delay(node) else {
            return Err(if self.compiled.contains_node(node) {
                TransportDelayInspectionFailure::NotTransportDelay(node)
            } else {
                TransportDelayInspectionFailure::UnknownNode(node)
            });
        };
        Ok(TransportDelayDefinitionInspection {
            node,
            delay,
            initial,
        })
    }

    /// Returns owned committed state and pending transition facts for a direct TransportDelay.
    pub fn inspect_transport_delay(
        &self,
        node: NodeKey,
    ) -> Result<TransportDelayInspection<D>, TransportDelayInspectionFailure> {
        self.inspect_transport_delay_definition(node)?;
        if !self.is_initialized() {
            return Err(TransportDelayInspectionFailure::NotInitialized);
        }
        self.transport_delay_observation(node)
            .ok_or(TransportDelayInspectionFailure::NotTransportDelay(node))
    }

    pub(crate) fn transport_delay_observation(
        &self,
        node: NodeKey,
    ) -> Option<TransportDelayInspection<D>> {
        let MachineStatus::Ready { now } = self.store.status else {
            return None;
        };
        let (delay, remembered_slot, output_slot, initial) = self.compiled.transport_delay(node)?;
        let input_operation = self.compiled.transport_delay_input_operation(node)?;
        let operation = self.compiled.node_operation(node)?;
        let current_support = *self.store.operation_causes.get(operation)?;
        let latest_transition = self
            .store
            .transport_transition_causes
            .get(&node)
            .copied()
            .unwrap_or(current_support);
        let mut pending = self
            .store
            .pending_events
            .values()
            .flatten()
            .filter_map(|event| match event {
                PendingEvent::TransportDelay(event) if event.node == node => {
                    Some(PendingTransportDelayInspection {
                        event: event.key,
                        node: event.node,
                        stimulus: event.stimulus,
                        origin: event.origin,
                        deadline: event.deadline,
                        target: event.target,
                        revision: event.revision,
                        cause: event.cause,
                    })
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        pending.sort_by_key(|event| (event.deadline, event.event));
        let next_deadline = pending
            .first()
            .map(PendingTransportDelayInspection::deadline);
        Some(TransportDelayInspection {
            node: self.compiled.node_subject(node),
            delay,
            initial,
            remembered_input: *self.store.stored_levels.get(remembered_slot.value())?,
            committed: *self.store.stored_levels.get(output_slot.value())?,
            input: self
                .store
                .operation_levels
                .get(input_operation)?
                .as_ref()
                .copied()?,
            revision: self.store.revision,
            at: now,
            latest_transition,
            current_support,
            pending,
            next_deadline,
            provenance: self.store.provenance.as_ref()?.clone(),
        })
    }

    /// Returns InertialDelay's immutable definition in either lifecycle phase.
    pub fn inspect_inertial_delay_definition(
        &self,
        node: NodeKey,
    ) -> Result<InertialDelayDefinitionInspection<D>, InertialDelayInspectionFailure> {
        if self.compiled.qualified_node(node).is_some() {
            return Err(InertialDelayInspectionFailure::UnknownNode(node));
        }
        let Some((delay, _, _, initial)) = self.compiled.inertial_delay(node) else {
            return Err(if self.compiled.contains_node(node) {
                InertialDelayInspectionFailure::NotInertialDelay(node)
            } else {
                InertialDelayInspectionFailure::UnknownNode(node)
            });
        };
        Ok(InertialDelayDefinitionInspection {
            node,
            delay,
            initial,
        })
    }

    /// Returns owned committed state and the optional InertialDelay candidate.
    pub fn inspect_inertial_delay(
        &self,
        node: NodeKey,
    ) -> Result<InertialDelayInspection<D>, InertialDelayInspectionFailure> {
        self.inspect_inertial_delay_definition(node)?;
        if !self.is_initialized() {
            return Err(InertialDelayInspectionFailure::NotInitialized);
        }
        self.inertial_delay_observation(node)
            .ok_or(InertialDelayInspectionFailure::NotInertialDelay(node))
    }

    pub(crate) fn inertial_delay_observation(
        &self,
        node: NodeKey,
    ) -> Option<InertialDelayInspection<D>> {
        let MachineStatus::Ready { now } = self.store.status else {
            return None;
        };
        let (delay, remembered_slot, output_slot, initial) = self.compiled.inertial_delay(node)?;
        let input_operation = self.compiled.inertial_delay_input_operation(node)?;
        let operation = self.compiled.node_operation(node)?;
        let current_support = *self.store.operation_causes.get(operation)?;
        let latest_transition = self
            .store
            .transport_transition_causes
            .get(&node)
            .copied()
            .unwrap_or(current_support);
        let pending = self
            .store
            .pending_events
            .values()
            .flatten()
            .find_map(|event| match event {
                PendingEvent::Inertial(event) if event.node == node => {
                    Some(PendingInertialDelayInspection {
                        event: event.key,
                        node: event.node,
                        stimulus: event.stimulus,
                        origin: event.origin,
                        deadline: event.deadline,
                        target: event.target,
                        revision: event.revision,
                        cause: event.cause,
                    })
                }
                _ => None,
            });
        let next_deadline = pending
            .as_ref()
            .map(PendingInertialDelayInspection::deadline);
        Some(InertialDelayInspection {
            node: self.compiled.node_subject(node),
            delay,
            initial,
            remembered_input: *self.store.stored_levels.get(remembered_slot.value())?,
            committed: *self.store.stored_levels.get(output_slot.value())?,
            input: self
                .store
                .operation_levels
                .get(input_operation)?
                .as_ref()
                .copied()?,
            revision: self.store.revision,
            at: now,
            latest_transition,
            current_support,
            pending,
            next_deadline,
            last_cancellation: self.store.inertial_cancellation_causes.get(&node).copied(),
            provenance: self.store.provenance.as_ref()?.clone(),
        })
    }

    /// Returns Periodic's immutable definition in either lifecycle phase.
    pub fn inspect_periodic_definition(
        &self,
        node: NodeKey,
    ) -> Result<PeriodicDefinitionInspection<D>, PeriodicInspectionFailure> {
        if self.compiled.qualified_node(node).is_some() {
            return Err(PeriodicInspectionFailure::UnknownNode(node));
        }
        let Some((period, first_emission, reenable_phase, _)) = self.compiled.periodic(node) else {
            return Err(if self.compiled.contains_node(node) {
                PeriodicInspectionFailure::NotPeriodic(node)
            } else {
                PeriodicInspectionFailure::UnknownNode(node)
            });
        };
        Ok(PeriodicDefinitionInspection {
            node,
            period,
            first_emission,
            reenable_phase,
        })
    }

    /// Returns committed phase and pending boundary facts for one Periodic source.
    pub fn inspect_periodic(
        &self,
        node: NodeKey,
    ) -> Result<PeriodicInspection<D>, PeriodicInspectionFailure> {
        self.inspect_periodic_definition(node)?;
        if !self.is_initialized() {
            return Err(PeriodicInspectionFailure::NotInitialized);
        }
        self.periodic_observation(node)
            .ok_or(PeriodicInspectionFailure::NotPeriodic(node))
    }

    pub(crate) fn periodic_observation(&self, node: NodeKey) -> Option<PeriodicInspection<D>> {
        let MachineStatus::Ready { now } = self.store.status else {
            return None;
        };
        let (period, first_emission, reenable_phase, previous_enable) =
            self.compiled.periodic(node)?;
        let enable_operation = self.compiled.periodic_enable_operation(node)?;
        let operation = self.compiled.node_operation(node)?;
        let current_support = *self.store.operation_causes.get(operation)?;
        let pending = self
            .store
            .pending_events
            .values()
            .flatten()
            .find_map(|event| match event {
                PendingEvent::Periodic(event) if event.node == node => {
                    Some(PendingPeriodicBoundaryInspection {
                        event: event.key,
                        node: self.compiled.node_subject(event.node),
                        stimulus: event.stimulus,
                        origin: event.origin,
                        deadline: event.deadline,
                        anchor: event.anchor,
                        ordinal: event.ordinal,
                        first_emission: event.first_emission,
                        reenable_phase: event.reenable_phase,
                        revision: event.revision,
                        cause: event.cause,
                    })
                }
                _ => None,
            });
        let next_deadline = pending
            .as_ref()
            .map(PendingPeriodicBoundaryInspection::deadline);
        Some(PeriodicInspection {
            phase_origin: self
                .store
                .periodic_anchors
                .get(&node)
                .map(|phase| phase.origin),
            settled_boundary: self
                .store
                .periodic_anchors
                .get(&node)
                .and_then(|phase| phase.settled),
            node: self.compiled.node_subject(node),
            period,
            first_emission,
            reenable_phase,
            remembered_enable: *self.store.stored_levels.get(previous_enable.value())?,
            enable: self
                .store
                .operation_levels
                .get(enable_operation)?
                .as_ref()
                .copied()?,
            anchor: self
                .store
                .periodic_anchors
                .get(&node)
                .map(|phase| phase.anchor),
            revision: self.store.revision,
            at: now,
            current_support,
            anchor_cause: self.store.periodic_anchor_causes.get(&node).copied(),
            pending,
            next_deadline,
            last_cancellation: self.store.periodic_cancellation_causes.get(&node).copied(),
            provenance: self.store.provenance.as_ref()?.clone(),
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
                .and_then(|(slot, _)| self.store.stored_levels.get(slot.value()).copied())
                .filter(|_| self.is_initialized());
            let toggle_inversion = self.store.toggle_inversion_causes.get(&flat).copied();
            let pulse_set_reset_state = self
                .compiled
                .pulse_set_reset_state_slot(flat)
                .and_then(|(slot, _, _)| self.store.stored_levels.get(slot.value()).copied())
                .filter(|_| self.is_initialized());
            let pulse_set_reset_cause = self
                .store
                .establishment_causes
                .get(&flat)
                .copied()
                .filter(|_| pulse_set_reset_state.is_some());
            let level_set_reset_state = self
                .compiled
                .level_set_reset_state_slot(flat)
                .and_then(|(slot, _, _)| self.store.stored_levels.get(slot.value()).copied())
                .filter(|_| self.is_initialized());
            let level_set_reset_cause = self
                .store
                .establishment_causes
                .get(&flat)
                .copied()
                .filter(|_| level_set_reset_state.is_some());
            let mut pending = self
                .store
                .pending_events
                .values()
                .flatten()
                .filter_map(|event| match event {
                    PendingEvent::PulseDelay(event) if event.node == flat => {
                        Some(ModulePendingPulseDelayInspection {
                            event: event.key,
                            stimulus: event.stimulus,
                            origin: event.origin,
                            deadline: event.deadline,
                            count: event.count,
                            cause: event.cause,
                        })
                    }
                    _ => None,
                })
                .collect::<Vec<_>>();
            pending.sort_by_key(|event| (event.deadline, event.event));
            nodes.push(ModuleNodeInspection {
                node: qualified.clone(),
                standard_role: owner_definition
                    .standard_declaration()
                    .and_then(|declaration| {
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
                pulse_set_reset_state,
                pulse_set_reset_cause,
                level_set_reset_state,
                level_set_reset_cause,
                sample_hold: self.sample_hold_observation(flat),
                transport_delay: self.transport_delay_observation(flat),
                inertial_delay: self.inertial_delay_observation(flat),
                periodic: self.periodic_observation(flat),
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
        let stateful_standard = crate::standard::stateful::inspect(self, &module);
        Ok(ModuleInspection {
            provenance: self
                .store
                .provenance
                .clone()
                .unwrap_or_else(|| panic!("ready inspections must retain committed provenance")),
            stateful_standard,
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
            provenance: self
                .store
                .provenance
                .clone()
                .unwrap_or_else(|| panic!("ready inspections must retain committed provenance")),
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
            .stored_levels
            .get(slot.value())
            .copied()
            .ok_or(ToggleInspectionFailure::NotToggle(node))?;
        Ok(ToggleInspection {
            provenance: self
                .store
                .provenance
                .clone()
                .unwrap_or_else(|| panic!("ready inspections must retain committed provenance")),
            node,
            initial: definition.initial,
            committed,
            revision: self.store.revision,
            at: now,
            latest_inversion: self.store.toggle_inversion_causes.get(&node).copied(),
        })
    }

    /// Returns declared pulse set/reset latch state and policy before or after initialization.
    pub fn inspect_pulse_set_reset_latch_definition(
        &self,
        node: NodeKey,
    ) -> Result<PulseSetResetLatchDefinitionInspection, PulseSetResetLatchInspectionFailure> {
        if self.compiled.qualified_node(node).is_some() {
            return Err(PulseSetResetLatchInspectionFailure::UnknownNode(node));
        }
        let Some((_, initial, conflict)) = self.compiled.pulse_set_reset_state_slot(node) else {
            return Err(if self.compiled.contains_node(node) {
                PulseSetResetLatchInspectionFailure::NotPulseSetResetLatch(node)
            } else {
                PulseSetResetLatchInspectionFailure::UnknownNode(node)
            });
        };
        Ok(PulseSetResetLatchDefinitionInspection {
            node,
            initial,
            conflict,
        })
    }

    /// Returns an owned observation of committed pulse set/reset latch state.
    pub fn inspect_pulse_set_reset_latch(
        &self,
        node: NodeKey,
    ) -> Result<PulseSetResetLatchInspection<D>, PulseSetResetLatchInspectionFailure> {
        let definition = self.inspect_pulse_set_reset_latch_definition(node)?;
        let MachineStatus::Ready { now } = self.store.status else {
            return Err(PulseSetResetLatchInspectionFailure::NotInitialized);
        };
        let (slot, _, _) = self.compiled.pulse_set_reset_state_slot(node).ok_or(
            PulseSetResetLatchInspectionFailure::NotPulseSetResetLatch(node),
        )?;
        let committed = self.store.stored_levels.get(slot.value()).copied().ok_or(
            PulseSetResetLatchInspectionFailure::NotPulseSetResetLatch(node),
        )?;
        let latest_establishment = self.store.establishment_causes.get(&node).copied().ok_or(
            PulseSetResetLatchInspectionFailure::NotPulseSetResetLatch(node),
        )?;
        Ok(PulseSetResetLatchInspection {
            provenance: self
                .store
                .provenance
                .clone()
                .unwrap_or_else(|| panic!("ready inspections must retain committed provenance")),
            node,
            initial: definition.initial,
            conflict: definition.conflict,
            committed,
            revision: self.store.revision,
            at: now,
            latest_establishment,
        })
    }
    /// Returns declared level set/reset latch state and policy before or after initialization.
    pub fn inspect_level_set_reset_latch_definition(
        &self,
        node: NodeKey,
    ) -> Result<LevelSetResetLatchDefinitionInspection, LevelSetResetLatchInspectionFailure> {
        if self.compiled.qualified_node(node).is_some() {
            return Err(LevelSetResetLatchInspectionFailure::UnknownNode(node));
        }
        let Some((_, initial, conflict)) = self.compiled.level_set_reset_state_slot(node) else {
            return Err(if self.compiled.contains_node(node) {
                LevelSetResetLatchInspectionFailure::NotLevelSetResetLatch(node)
            } else {
                LevelSetResetLatchInspectionFailure::UnknownNode(node)
            });
        };
        Ok(LevelSetResetLatchDefinitionInspection {
            node,
            initial,
            conflict,
        })
    }

    /// Returns an owned observation of committed level set/reset latch state.
    pub fn inspect_level_set_reset_latch(
        &self,
        node: NodeKey,
    ) -> Result<LevelSetResetLatchInspection<D>, LevelSetResetLatchInspectionFailure> {
        let definition = self.inspect_level_set_reset_latch_definition(node)?;
        let MachineStatus::Ready { now } = self.store.status else {
            return Err(LevelSetResetLatchInspectionFailure::NotInitialized);
        };
        let (slot, _, _) = self.compiled.level_set_reset_state_slot(node).ok_or(
            LevelSetResetLatchInspectionFailure::NotLevelSetResetLatch(node),
        )?;
        let committed = self.store.stored_levels.get(slot.value()).copied().ok_or(
            LevelSetResetLatchInspectionFailure::NotLevelSetResetLatch(node),
        )?;
        let latest_establishment = self.store.establishment_causes.get(&node).copied().ok_or(
            LevelSetResetLatchInspectionFailure::NotLevelSetResetLatch(node),
        )?;
        let (set, reset) = self
            .compiled
            .level_set_reset_controls(node, &self.store.operation_levels)
            .ok_or(LevelSetResetLatchInspectionFailure::NotLevelSetResetLatch(
                node,
            ))?;
        Ok(LevelSetResetLatchInspection {
            provenance: self
                .store
                .provenance
                .clone()
                .unwrap_or_else(|| panic!("ready inspections must retain committed provenance")),
            set,
            reset,
            node,
            initial: definition.initial,
            conflict: definition.conflict,
            committed,
            revision: self.store.revision,
            at: now,
            latest_establishment,
        })
    }
    /// Returns declared SampleHold state before or after initialization.
    pub fn inspect_sample_hold_definition(
        &self,
        node: NodeKey,
    ) -> Result<SampleHoldDefinitionInspection, SampleHoldInspectionFailure> {
        if self.compiled.qualified_node(node).is_some() {
            return Err(SampleHoldInspectionFailure::UnknownNode(node));
        }
        let Some((_, initial)) = self.compiled.sample_hold_state_slot(node) else {
            return Err(if self.compiled.contains_node(node) {
                SampleHoldInspectionFailure::NotSampleHold(node)
            } else {
                SampleHoldInspectionFailure::UnknownNode(node)
            });
        };
        Ok(SampleHoldDefinitionInspection { node, initial })
    }

    /// Returns an owned observation of one direct SampleHold; module nodes are available through module inspection.
    pub fn inspect_sample_hold(
        &self,
        node: NodeKey,
    ) -> Result<SampleHoldInspection<D>, SampleHoldInspectionFailure> {
        self.inspect_sample_hold_definition(node)?;
        if !self.is_initialized() {
            return Err(SampleHoldInspectionFailure::NotInitialized);
        }
        Ok(self.sample_hold_observation(node).unwrap_or_else(|| {
            panic!("initialized SampleHold must retain its stored value and both causal roots")
        }))
    }

    pub(crate) fn sample_hold_observation(&self, node: NodeKey) -> Option<SampleHoldInspection<D>> {
        let MachineStatus::Ready { now } = self.store.status else {
            return None;
        };
        let (slot, initial) = self.compiled.sample_hold_state_slot(node)?;
        let value_operation = self.compiled.sample_hold_value_operation(node)?;
        Some(SampleHoldInspection {
            node: self.compiled.node_subject(node),
            initial,
            committed: *self.store.stored_levels.get(slot.value())?,
            value: self
                .store
                .operation_levels
                .get(value_operation)
                .copied()
                .flatten()?,
            revision: self.store.revision,
            at: now,
            latest_establishment: *self.store.establishment_causes.get(&node)?,
            current_support: *self
                .store
                .operation_causes
                .get(self.compiled.node_operation(node)?)?,
            provenance: self.store.provenance.as_ref()?.clone(),
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
