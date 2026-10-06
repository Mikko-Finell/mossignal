//! Restricted initialization and ready-machine Level and Pulse transactions.

use crate::authored::{ConflictPolicy, EdgeObservation};
use crate::compile::{
    EvaluationCause, EvaluationFailure, FullEvaluation, LevelLatchConflict, PulseLatchConflict,
};
use crate::diagnostics::{
    BudgetEvidence, ConflictControls, ConflictEvidence, DiagnosticCode, DiagnosticOccurrence,
    InputSchemaEvidence, LifecycleEvidence, NodeEvidence, OperationSubjectRef, ParameterEvidence,
    Problem, ProblemEvidence, ProvenanceEvidence, Responsibility, RevisionMismatchEvidence,
    Severity, SubjectRef, TimeEvidence, TimeOperation,
};
use crate::identity::{
    ExecutionStateDigest, InputSchemaFingerprint, NetworkFingerprint, ObservableStateDigest,
};
use crate::input::{InputDelta, InputSnapshot};
use crate::key::{ExternalInputKey, ExternalOutputKey, NetworkKey, NodeKey};
use crate::machine::{
    Machine, MachineStatus, NetworkRevision, PendingEvent, PendingEventKey, PendingInertialDelay,
    PendingPeriodicBoundary, PendingPulseDelay, PendingTransportDelay, Schedule,
};
use crate::module::{NodeSubject, PulsePortSubject, QualifiedNodeRef};
use crate::policy::{RuntimePolicy, RuntimePolicyLimit};
use crate::signal::{Level, LogicLevel, Pulse, PulseCount};
use crate::time::{Span, Time};
use core::fmt;
use core::marker::PhantomData;
use std::collections::BTreeMap;
use std::sync::Arc;

const PROVENANCE_VIEW_SCOPE_DOMAIN: &[u8] = b"mossignal/provenance_view_scope/v1";
type ProvenanceScope = [u8; 32];
const UNFINALIZED_PROVENANCE_SCOPE: ProvenanceScope = [0; 32];

enum TransactionKind<D> {
    Initialize(InputSnapshot<D>),
    Advance(InputDelta<D>),
}

impl<D> Clone for TransactionKind<D> {
    fn clone(&self) -> Self {
        match self {
            Self::Initialize(input) => Self::Initialize(input.clone()),
            Self::Advance(input) => Self::Advance(input.clone()),
        }
    }
}

/// An owned explicit runtime transaction.
pub struct Transaction<D> {
    at: Time<D>,
    expected_revision: NetworkRevision,
    kind: TransactionKind<D>,
}

impl<D> Clone for Transaction<D> {
    fn clone(&self) -> Self {
        Self {
            at: self.at,
            expected_revision: self.expected_revision,
            kind: self.kind.clone(),
        }
    }
}

impl<D> Transaction<D> {
    /// Constructs the initialization transaction for an uninitialized machine.
    #[must_use]
    pub const fn initialize(
        at: Time<D>,
        expected_revision: NetworkRevision,
        input: InputSnapshot<D>,
    ) -> Self {
        Self {
            at,
            expected_revision,
            kind: TransactionKind::Initialize(input),
        }
    }

    /// Constructs a ready-machine advancement transaction.
    #[must_use]
    pub const fn advance(
        at: Time<D>,
        expected_revision: NetworkRevision,
        input: InputDelta<D>,
    ) -> Self {
        Self {
            at,
            expected_revision,
            kind: TransactionKind::Advance(input),
        }
    }

    /// Returns the requested logical time.
    #[must_use]
    pub const fn requested_time(&self) -> Time<D> {
        self.at
    }

    /// Returns the exact machine revision expected by this transaction.
    #[must_use]
    pub const fn expected_revision(&self) -> NetworkRevision {
        self.expected_revision
    }

    /// Returns the initialization input when this is an initialization transaction.
    #[must_use]
    pub const fn initialization_input(&self) -> Option<&InputSnapshot<D>> {
        match &self.kind {
            TransactionKind::Initialize(input) => Some(input),
            TransactionKind::Advance(_) => None,
        }
    }

    /// Returns the level delta when this is a ready-machine transaction.
    #[must_use]
    pub const fn advance_input(&self) -> Option<&InputDelta<D>> {
        match &self.kind {
            TransactionKind::Initialize(_) => None,
            TransactionKind::Advance(input) => Some(input),
        }
    }
}

/// One structured runtime rejection with an exact catalogue-backed category.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeFailureEvidence {
    AlreadyInitialized,
    DeltaBeforeInitialization,
    TimeNotStrictlyIncreasing {
        current_ticks: u64,
        requested_ticks: u64,
    },
    StaleRevision {
        expected: NetworkRevision,
        actual: NetworkRevision,
    },
    WrongNetwork {
        expected_key: NetworkKey,
        actual_key: NetworkKey,
        expected_fingerprint: NetworkFingerprint,
        actual_fingerprint: NetworkFingerprint,
    },
    ForeignInputSchema {
        expected: InputSchemaFingerprint,
        actual: InputSchemaFingerprint,
    },
    StaleInputSchema {
        expected: InputSchemaFingerprint,
        actual: InputSchemaFingerprint,
    },
    BudgetExceeded {
        budget: RuntimePolicyLimit,
        limit: u64,
        consumed: u64,
    },
    PulseCountOverflow {
        node: NodeSubject,
        left: PulseCount,
        right: PulseCount,
    },
    TimeOverflow {
        node: NodeSubject,
        origin_ticks: u64,
        delay_ticks: u64,
    },
    TransportTimeOverflow {
        node: NodeSubject,
        origin_ticks: u64,
        delay_ticks: u64,
    },
    InertialTimeOverflow {
        node: NodeSubject,
        origin_ticks: u64,
        delay_ticks: u64,
    },
    PeriodicTimeOverflow {
        node: NodeSubject,
        origin_ticks: u64,
        period_ticks: u64,
    },
    PulseLatchConflict {
        node: NodeSubject,
        policy: ConflictPolicy,
        previous: LogicLevel,
        set_count: PulseCount,
        reset_count: PulseCount,
        at_ticks: u64,
        revision: NetworkRevision,
    },

    LevelLatchConflict {
        node: NodeSubject,
        policy: ConflictPolicy,
        previous: LogicLevel,
        set_level: LogicLevel,
        reset_level: LogicLevel,
        at_ticks: u64,
        revision: NetworkRevision,
    },
}

impl RuntimeFailureEvidence {
    /// Returns the catalogue code paired with this exact evidence leaf.
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

    /// Projects this runtime leaf into the common catalogue-backed problem model.
    #[must_use]
    pub fn problem<D>(&self) -> Problem<D> {
        let primary = SubjectRef::Operation(OperationSubjectRef::MachineTransaction);
        let evidence = match self {
            Self::AlreadyInitialized => ProblemEvidence::LifecycleAlreadyInitialized {
                evidence: LifecycleEvidence {
                    operation: OperationSubjectRef::MachineTransaction,
                    current_time_ticks: None,
                },
                marker: PhantomData,
            },
            Self::DeltaBeforeInitialization => {
                ProblemEvidence::LifecycleDeltaBeforeInitialization {
                    evidence: LifecycleEvidence {
                        operation: OperationSubjectRef::MachineTransaction,
                        current_time_ticks: None,
                    },
                    marker: PhantomData,
                }
            }
            Self::TimeNotStrictlyIncreasing {
                current_ticks,
                requested_ticks,
            } => ProblemEvidence::RuntimeTimeNotStrictlyIncreasing {
                evidence: TimeEvidence {
                    owner: None,
                    operation: TimeOperation::TransactionAdvance,
                    left_ticks: *current_ticks,
                    right_ticks: *requested_ticks,
                },
                marker: PhantomData,
            },
            Self::StaleRevision { expected, actual } => ProblemEvidence::RuntimeStaleRevision {
                evidence: RevisionMismatchEvidence {
                    expected: *expected,
                    actual: *actual,
                },
                marker: PhantomData,
            },
            Self::WrongNetwork {
                expected_key,
                actual_key,
                expected_fingerprint,
                actual_fingerprint,
            } => ProblemEvidence::InputWrongNetwork {
                evidence: InputSchemaEvidence {
                    expected_network: Some(*expected_key),
                    actual_network: Some(*actual_key),
                    expected_fingerprint: Some(*expected_fingerprint),
                    actual_fingerprint: Some(*actual_fingerprint),
                    expected_schema: None,
                    actual_schema: None,
                },
                marker: PhantomData,
            },
            Self::ForeignInputSchema { expected, actual } => ProblemEvidence::InputForeignSchema {
                evidence: InputSchemaEvidence {
                    expected_network: None,
                    actual_network: None,
                    expected_fingerprint: None,
                    actual_fingerprint: None,
                    expected_schema: Some(*expected),
                    actual_schema: Some(*actual),
                },
                marker: PhantomData,
            },
            Self::StaleInputSchema { expected, actual } => ProblemEvidence::InputStaleSchema {
                evidence: InputSchemaEvidence {
                    expected_network: None,
                    actual_network: None,
                    expected_fingerprint: None,
                    actual_fingerprint: None,
                    expected_schema: Some(*expected),
                    actual_schema: Some(*actual),
                },
                marker: PhantomData,
            },
            Self::BudgetExceeded {
                budget,
                limit,
                consumed,
            } => ProblemEvidence::RuntimeBudgetExceeded {
                evidence: BudgetEvidence {
                    budget: budget.parameter_key(),
                    limit: *limit,
                    consumed: *consumed,
                },
                marker: PhantomData,
            },
            Self::PulseCountOverflow { node, left, right } => {
                ProblemEvidence::RuntimePulseCountOverflow {
                    evidence: ParameterEvidence {
                        owner: Some(node_evidence(node)),
                        parameter: "pulse_count_sum",
                        expected_domain: "u64 sum",
                        encountered: None,
                        operands: vec![left.get(), right.get()],
                    },
                    marker: PhantomData,
                }
            }
            Self::TimeOverflow {
                node,
                origin_ticks,
                delay_ticks,
            } => ProblemEvidence::RuntimeTimeOverflow {
                evidence: TimeEvidence {
                    owner: Some(node_evidence(node)),
                    operation: TimeOperation::PulseDelayDeadline,
                    left_ticks: *origin_ticks,
                    right_ticks: *delay_ticks,
                },
                marker: PhantomData,
            },
            Self::TransportTimeOverflow {
                node,
                origin_ticks,
                delay_ticks,
            } => ProblemEvidence::RuntimeTimeOverflow {
                evidence: TimeEvidence {
                    owner: Some(node_evidence(node)),
                    operation: TimeOperation::TransportDelayDeadline,
                    left_ticks: *origin_ticks,
                    right_ticks: *delay_ticks,
                },
                marker: PhantomData,
            },
            Self::InertialTimeOverflow {
                node,
                origin_ticks,
                delay_ticks,
            } => ProblemEvidence::RuntimeTimeOverflow {
                evidence: TimeEvidence {
                    owner: Some(node_evidence(node)),
                    operation: TimeOperation::InertialDelayDeadline,
                    left_ticks: *origin_ticks,
                    right_ticks: *delay_ticks,
                },
                marker: PhantomData,
            },
            Self::PeriodicTimeOverflow {
                node,
                origin_ticks,
                period_ticks,
            } => ProblemEvidence::RuntimeTimeOverflow {
                evidence: TimeEvidence {
                    owner: Some(node_evidence(node)),
                    operation: TimeOperation::PeriodicDeadline,
                    left_ticks: *origin_ticks,
                    right_ticks: *period_ticks,
                },
                marker: PhantomData,
            },
            Self::PulseLatchConflict {
                node,
                policy,
                previous,
                set_count,
                reset_count,
                at_ticks,
                revision,
            } => {
                return Problem::new(
                    node_subject_ref(node),
                    Vec::new(),
                    ProblemEvidence::RuntimePulseLatchConflictRejected {
                        evidence: ConflictEvidence {
                            node: node_evidence(node),
                            policy: *policy,
                            previous: *previous,
                            controls: ConflictControls::Pulse {
                                set: *set_count,
                                reset: *reset_count,
                            },
                            at_ticks: *at_ticks,
                            revision: *revision,
                        },
                        marker: PhantomData,
                    },
                );
            }
            Self::LevelLatchConflict {
                node,
                policy,
                previous,
                set_level,
                reset_level,
                at_ticks,
                revision,
            } => {
                return Problem::new(
                    node_subject_ref(node),
                    Vec::new(),
                    ProblemEvidence::RuntimeLevelLatchConflictRejected {
                        evidence: ConflictEvidence {
                            node: node_evidence(node),
                            policy: *policy,
                            previous: *previous,
                            controls: ConflictControls::Level {
                                set: *set_level,
                                reset: *reset_level,
                            },
                            at_ticks: *at_ticks,
                            revision: *revision,
                        },
                        marker: PhantomData,
                    },
                );
            }
        };
        Problem::new(primary, Vec::new(), evidence)
    }
}

fn node_evidence(subject: &NodeSubject) -> NodeEvidence {
    match subject {
        NodeSubject::Node(node) => NodeEvidence::Node(*node),
        NodeSubject::Qualified(node) => NodeEvidence::Qualified {
            instances: node.instances().to_vec(),
            node: node.node(),
        },
    }
}

fn node_subject_ref(subject: &NodeSubject) -> SubjectRef {
    match subject {
        NodeSubject::Node(node) => SubjectRef::Node(*node),
        NodeSubject::Qualified(node) => SubjectRef::QualifiedNode(node.clone()),
    }
}

/// A structured rejection of one runtime transaction.
pub struct RuntimeFailure<D> {
    evidence: Box<RuntimeFailureEvidence>,
    problem: Box<Problem<D>>,
}

impl<D> RuntimeFailure<D> {
    fn new(evidence: RuntimeFailureEvidence) -> Self {
        let problem = Box::new(evidence.problem());
        Self {
            evidence: Box::new(evidence),
            problem,
        }
    }

    /// Returns the exact typed failure evidence.
    #[must_use]
    pub const fn evidence(&self) -> &RuntimeFailureEvidence {
        &self.evidence
    }

    /// Returns the exact catalogue code represented by this rejection.
    #[must_use]
    pub const fn code(&self) -> DiagnosticCode {
        self.problem.code()
    }

    /// Returns the catalogue severity for this rejection.
    #[must_use]
    pub const fn severity(&self) -> Severity {
        self.problem.severity()
    }

    /// Returns the catalogue responsibility for this rejection.
    #[must_use]
    pub const fn responsibility(&self) -> Responsibility {
        self.problem.responsibility()
    }

    /// Returns the complete common problem retained by this runtime rejection.
    #[must_use]
    pub const fn problem(&self) -> &Problem<D> {
        &self.problem
    }
}

impl<D> fmt::Debug for RuntimeFailure<D> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RuntimeFailure")
            .field("code", &self.code())
            .field("evidence", &self.evidence)
            .finish()
    }
}

impl<D> fmt::Display for RuntimeFailure<D> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "transaction rejected with {}",
            self.code().as_str()
        )
    }
}

impl<D> std::error::Error for RuntimeFailure<D> {}

/// An opaque result-scoped reference to one immutable provenance record.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CauseRef {
    scope: ProvenanceScope,
    ordinal: u32,
}

impl fmt::Debug for CauseRef {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("CauseRef(..)")
    }
}

impl CauseRef {
    pub(crate) const fn from_parts(scope: ProvenanceScope, ordinal: u32) -> Self {
        Self { scope, ordinal }
    }

    #[cfg(test)]
    pub(crate) const fn ordinal(self) -> u32 {
        self.ordinal
    }
}

pub(crate) enum ProvenanceSubjectKind<'a> {
    Node(NodeKey),
    Qualified(&'a QualifiedNodeRef),
    ExternalOutput(ExternalOutputKey<Level>),
    PulseExternalOutput(ExternalOutputKey<Pulse>),
}

impl ProvenanceSubject {
    pub(crate) fn kind(&self) -> ProvenanceSubjectKind<'_> {
        match self {
            Self::Node(node) => ProvenanceSubjectKind::Node(*node),
            Self::QualifiedNode(node) => ProvenanceSubjectKind::Qualified(node),
            Self::ExternalOutput(output) => ProvenanceSubjectKind::ExternalOutput(*output),
            Self::PulseExternalOutput(output) => {
                ProvenanceSubjectKind::PulseExternalOutput(*output)
            }
        }
    }
}

/// The semantic subject of one derived initialization cause.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProvenanceSubject {
    Node(NodeKey),
    QualifiedNode(QualifiedNodeRef),
    ExternalOutput(ExternalOutputKey<Level>),
    PulseExternalOutput(ExternalOutputKey<Pulse>),
}

/// One stable input-port contribution to a simultaneous pulse transformation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PulseContribution {
    port: PulsePortSubject,
    count: PulseCount,
    cause: CauseRef,
}

impl PulseContribution {
    #[must_use]
    pub const fn port(&self) -> &PulsePortSubject {
        &self.port
    }

    #[must_use]
    pub const fn count(&self) -> PulseCount {
        self.count
    }

    #[must_use]
    pub const fn cause(&self) -> CauseRef {
        self.cause
    }

    pub(crate) const fn restored(
        port: PulsePortSubject,
        count: PulseCount,
        cause: CauseRef,
    ) -> Self {
        Self { port, count, cause }
    }
}

#[derive(Clone)]
pub(crate) enum ProvenanceRecord<D> {
    InitializationTransaction {
        at: Time<D>,
        revision: NetworkRevision,
    },
    ReadyTransaction {
        at: Time<D>,
        revision: NetworkRevision,
    },
    ExternalObservation {
        input: ExternalInputKey<Level>,
        value: LogicLevel,
    },
    ExternalPulseObservation {
        input: ExternalInputKey<Pulse>,
        count: PulseCount,
    },
    PendingPulseDelay {
        event: PendingEventKey,
        owner: NodeSubject,
        origin: Time<D>,
        deadline: Time<D>,
        count: PulseCount,
        revision: NetworkRevision,
        supporters: Vec<CauseRef>,
    },
    PendingTransportDelay {
        event: PendingEventKey,
        owner: NodeSubject,
        origin: Time<D>,
        deadline: Time<D>,
        target: LogicLevel,
        revision: NetworkRevision,
        supporters: Vec<CauseRef>,
    },
    PendingInertialDelay {
        event: PendingEventKey,
        owner: NodeSubject,
        origin: Time<D>,
        deadline: Time<D>,
        target: LogicLevel,
        revision: NetworkRevision,
        supporters: Vec<CauseRef>,
    },
    PendingPeriodicBoundary {
        event: PendingEventKey,
        owner: NodeSubject,
        origin: Time<D>,
        deadline: Time<D>,
        anchor: Time<D>,
        ordinal: u64,
        first_emission: crate::authored::FirstEmissionPolicy,
        reenable_phase: crate::authored::ReenablePhasePolicy,
        revision: NetworkRevision,
        supporters: Vec<CauseRef>,
    },
    Derived {
        subject: ProvenanceSubject,
        supporters: Vec<CauseRef>,
    },
    PulseDerived {
        subject: ProvenanceSubject,
        contributions: Vec<PulseContribution>,
        result: PulseCount,
        supporters: Vec<CauseRef>,
    },
    PulseControlledLevel {
        subject: ProvenanceSubject,
        contributions: Vec<PulseContribution>,
        result: LogicLevel,
        supporters: Vec<CauseRef>,
    },
}

impl<D> ProvenanceRecord<D> {
    pub(crate) fn supporters(&self) -> &[CauseRef] {
        match self {
            Self::PendingPulseDelay { supporters, .. }
            | Self::PendingTransportDelay { supporters, .. }
            | Self::PendingInertialDelay { supporters, .. }
            | Self::PendingPeriodicBoundary { supporters, .. }
            | Self::Derived { supporters, .. }
            | Self::PulseDerived { supporters, .. }
            | Self::PulseControlledLevel { supporters, .. } => supporters,
            Self::InitializationTransaction { .. }
            | Self::ReadyTransaction { .. }
            | Self::ExternalObservation { .. }
            | Self::ExternalPulseObservation { .. } => &[],
        }
    }

    pub(crate) fn predecessor_causes(&self) -> Vec<CauseRef> {
        let mut causes = self.supporters().to_vec();
        match self {
            Self::PulseDerived { contributions, .. }
            | Self::PulseControlledLevel { contributions, .. } => {
                causes.extend(contributions.iter().map(PulseContribution::cause));
            }
            _ => {}
        }
        causes
    }

    #[cfg(test)]
    pub(crate) fn reverse_unordered_supporters(&mut self) {
        match self {
            Self::PendingPulseDelay { supporters, .. }
            | Self::PendingTransportDelay { supporters, .. }
            | Self::PendingInertialDelay { supporters, .. }
            | Self::PendingPeriodicBoundary { supporters, .. }
            | Self::Derived { supporters, .. }
            | Self::PulseDerived { supporters, .. }
            | Self::PulseControlledLevel { supporters, .. } => supporters.reverse(),
            Self::InitializationTransaction { .. }
            | Self::ReadyTransaction { .. }
            | Self::ExternalObservation { .. }
            | Self::ExternalPulseObservation { .. } => {}
        }
    }
}

/// A borrowed structured projection of one immutable causal record.
#[non_exhaustive]
pub enum CauseInspection<'a, D> {
    InitializationTransaction {
        at: Time<D>,
        revision: NetworkRevision,
    },
    ReadyTransaction {
        at: Time<D>,
        revision: NetworkRevision,
    },
    ExternalObservation {
        input: ExternalInputKey<Level>,
        value: LogicLevel,
    },
    ExternalPulseObservation {
        input: ExternalInputKey<Pulse>,
        count: PulseCount,
    },
    PendingPulseDelay {
        event: PendingEventKey,
        owner: &'a NodeSubject,
        origin: Time<D>,
        deadline: Time<D>,
        count: PulseCount,
        revision: NetworkRevision,
        supporters: &'a [CauseRef],
    },
    PendingTransportDelay {
        event: PendingEventKey,
        owner: &'a NodeSubject,
        origin: Time<D>,
        deadline: Time<D>,
        target: LogicLevel,
        revision: NetworkRevision,
        supporters: &'a [CauseRef],
    },
    PendingInertialDelay {
        event: PendingEventKey,
        owner: &'a NodeSubject,
        origin: Time<D>,
        deadline: Time<D>,
        target: LogicLevel,
        revision: NetworkRevision,
        supporters: &'a [CauseRef],
    },
    PendingPeriodicBoundary {
        event: PendingEventKey,
        owner: &'a NodeSubject,
        origin: Time<D>,
        deadline: Time<D>,
        anchor: Time<D>,
        ordinal: u64,
        first_emission: crate::authored::FirstEmissionPolicy,
        reenable_phase: crate::authored::ReenablePhasePolicy,
        revision: NetworkRevision,
        supporters: &'a [CauseRef],
    },
    Derived {
        subject: ProvenanceSubject,
        supporters: &'a [CauseRef],
    },
    PulseDerived {
        subject: ProvenanceSubject,
        contributions: &'a [PulseContribution],
        result: PulseCount,
        supporters: &'a [CauseRef],
    },
    PulseControlledLevel {
        subject: ProvenanceSubject,
        contributions: &'a [PulseContribution],
        result: LogicLevel,
        supporters: &'a [CauseRef],
    },
}

/// Failure to resolve a cause through the owning result's provenance view.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CauseLookupFailure {
    /// The reference belongs to a different immutable provenance view.
    ForeignCause {
        expected_scope: [u8; 32],
        actual_scope: [u8; 32],
        ordinal: u32,
    },
    /// The reference belongs to this view but its ordinal is not valid.
    InvalidCause { scope: [u8; 32], ordinal: u32 },
}

impl CauseLookupFailure {
    #[must_use]
    pub const fn code(self) -> DiagnosticCode {
        match self {
            Self::ForeignCause { .. } => DiagnosticCode::ExplanationForeignCause,
            Self::InvalidCause { .. } => DiagnosticCode::ExplanationInvalidCause,
        }
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
        let evidence = match self {
            Self::ForeignCause {
                expected_scope,
                actual_scope,
                ordinal,
            } => ProblemEvidence::ExplanationForeignCause {
                evidence: ProvenanceEvidence {
                    expected_scope,
                    actual_scope,
                    ordinal,
                },
                marker: PhantomData,
            },
            Self::InvalidCause { scope, ordinal } => ProblemEvidence::ExplanationInvalidCause {
                evidence: ProvenanceEvidence {
                    expected_scope: scope,
                    actual_scope: scope,
                    ordinal,
                },
                marker: PhantomData,
            },
        };
        Problem::new(
            SubjectRef::Operation(OperationSubjectRef::ProvenanceView),
            Vec::new(),
            evidence,
        )
    }
}

impl fmt::Display for CauseLookupFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ForeignCause { .. } => {
                formatter.write_str("cause reference belongs to another provenance view")
            }
            Self::InvalidCause { .. } => {
                formatter.write_str("cause reference does not resolve in its provenance view")
            }
        }
    }
}

impl std::error::Error for CauseLookupFailure {}

/// An immutable result-owned view of initialization derivations.
pub struct ProvenanceView<D> {
    scope: ProvenanceScope,
    records: Arc<Vec<ProvenanceRecord<D>>>,
}

struct ProvenanceBuild<D> {
    provenance: ProvenanceView<D>,
    operation_causes: Vec<CauseRef>,
    input_causes: BTreeMap<ExternalInputKey<Level>, CauseRef>,
    output_causes: BTreeMap<ExternalOutputKey<Level>, CauseRef>,
    pulse_output_causes: BTreeMap<ExternalOutputKey<Pulse>, CauseRef>,
    edge_observation_causes: BTreeMap<NodeKey, CauseRef>,
    toggle_inversion_causes: BTreeMap<NodeKey, CauseRef>,
    establishment_causes: BTreeMap<NodeKey, CauseRef>,
    transport_output_transitions: BTreeMap<NodeKey, CauseRef>,
    pulse_delay_schedules: BTreeMap<NodeKey, CauseRef>,
    transport_delay_schedules: BTreeMap<NodeKey, CauseRef>,
    inertial_delay_schedules: BTreeMap<NodeKey, CauseRef>,
    periodic_schedules: BTreeMap<NodeKey, CauseRef>,
}

struct EvaluationProvenanceInputs<'a, D> {
    compiled: &'a crate::CompiledNetwork<D>,
    levels: &'a BTreeMap<ExternalInputKey<Level>, CauseRef>,
    pulses: &'a BTreeMap<ExternalInputKey<Pulse>, CauseRef>,
    due_pulse_delays: &'a BTreeMap<NodeKey, Vec<CauseRef>>,
    due_transport_delays: &'a BTreeMap<NodeKey, Vec<CauseRef>>,
    due_inertial_delays: &'a BTreeMap<NodeKey, Vec<CauseRef>>,
    due_periodic_boundaries: &'a BTreeMap<NodeKey, Vec<CauseRef>>,
}

struct PreviousLevelOutputs<'a> {
    causes: &'a BTreeMap<ExternalOutputKey<Level>, CauseRef>,
    baselines: &'a BTreeMap<ExternalOutputKey<Level>, LogicLevel>,
}

struct PreviousStateCauses<'a> {
    edge_observations: &'a BTreeMap<NodeKey, CauseRef>,
    toggle_inversions: &'a BTreeMap<NodeKey, CauseRef>,
    establishments: &'a BTreeMap<NodeKey, CauseRef>,
    transport_transitions: &'a BTreeMap<NodeKey, CauseRef>,
    periodic_anchors: &'a BTreeMap<NodeKey, CauseRef>,
}

impl<D> Clone for ProvenanceView<D> {
    fn clone(&self) -> Self {
        Self {
            scope: self.scope,
            records: Arc::clone(&self.records),
        }
    }
}

impl<D> ProvenanceView<D> {
    /// Resolves one result-scoped reference to structured immutable evidence.
    pub fn inspect(&self, cause: CauseRef) -> Result<CauseInspection<'_, D>, CauseLookupFailure> {
        if cause.scope != self.scope {
            return Err(CauseLookupFailure::ForeignCause {
                expected_scope: self.scope,
                actual_scope: cause.scope,
                ordinal: cause.ordinal,
            });
        }
        let Some(record) = self.records.get(cause.ordinal as usize) else {
            return Err(CauseLookupFailure::InvalidCause {
                scope: self.scope,
                ordinal: cause.ordinal,
            });
        };
        Ok(match record {
            ProvenanceRecord::InitializationTransaction { at, revision } => {
                CauseInspection::InitializationTransaction {
                    at: *at,
                    revision: *revision,
                }
            }
            ProvenanceRecord::ReadyTransaction { at, revision } => {
                CauseInspection::ReadyTransaction {
                    at: *at,
                    revision: *revision,
                }
            }
            ProvenanceRecord::ExternalObservation { input, value } => {
                CauseInspection::ExternalObservation {
                    input: *input,
                    value: *value,
                }
            }
            ProvenanceRecord::ExternalPulseObservation { input, count } => {
                CauseInspection::ExternalPulseObservation {
                    input: *input,
                    count: *count,
                }
            }
            ProvenanceRecord::PendingPulseDelay {
                event,
                owner,
                origin,
                deadline,
                count,
                revision,
                supporters,
            } => CauseInspection::PendingPulseDelay {
                event: *event,
                owner,
                origin: *origin,
                deadline: *deadline,
                count: *count,
                revision: *revision,
                supporters,
            },
            ProvenanceRecord::PendingTransportDelay {
                event,
                owner,
                origin,
                deadline,
                target,
                revision,
                supporters,
            } => CauseInspection::PendingTransportDelay {
                event: *event,
                owner,
                origin: *origin,
                deadline: *deadline,
                target: *target,
                revision: *revision,
                supporters,
            },
            ProvenanceRecord::PendingInertialDelay {
                event,
                owner,
                origin,
                deadline,
                target,
                revision,
                supporters,
            } => CauseInspection::PendingInertialDelay {
                event: *event,
                owner,
                origin: *origin,
                deadline: *deadline,
                target: *target,
                revision: *revision,
                supporters,
            },
            ProvenanceRecord::PendingPeriodicBoundary {
                event,
                owner,
                origin,
                deadline,
                anchor,
                ordinal,
                first_emission,
                reenable_phase,
                revision,
                supporters,
            } => CauseInspection::PendingPeriodicBoundary {
                event: *event,
                owner,
                origin: *origin,
                deadline: *deadline,
                anchor: *anchor,
                ordinal: *ordinal,
                first_emission: *first_emission,
                reenable_phase: *reenable_phase,
                revision: *revision,
                supporters,
            },
            ProvenanceRecord::Derived {
                subject,
                supporters,
            } => CauseInspection::Derived {
                subject: subject.clone(),
                supporters,
            },
            ProvenanceRecord::PulseDerived {
                subject,
                contributions,
                result,
                supporters,
            } => CauseInspection::PulseDerived {
                subject: subject.clone(),
                contributions,
                result: *result,
                supporters,
            },
            ProvenanceRecord::PulseControlledLevel {
                subject,
                contributions,
                result,
                supporters,
            } => CauseInspection::PulseControlledLevel {
                subject: subject.clone(),
                contributions,
                result: *result,
                supporters,
            },
        })
    }

    /// Returns the number of retained immutable records.
    #[must_use]
    pub fn len(&self) -> usize {
        self.records.len()
    }

    /// Returns whether this view contains no causal records.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    pub(crate) fn records(&self) -> &[ProvenanceRecord<D>] {
        &self.records
    }

    pub(crate) const fn scope(&self) -> ProvenanceScope {
        self.scope
    }

    pub(crate) fn restored(
        network_key: crate::key::NetworkKey,
        fingerprint: crate::NetworkFingerprint,
        records: Vec<ProvenanceRecord<D>>,
    ) -> Self {
        let scope = provenance_view_scope(network_key, fingerprint, &records);
        let records = records
            .iter()
            .map(|record| remap_record(record, scope))
            .collect();
        Self {
            scope,
            records: Arc::new(records),
        }
    }

    pub(crate) fn resolve_ordinal(&self, cause: CauseRef) -> usize {
        if cause.scope != self.scope {
            panic!("committed cause must belong to its provenance view");
        }
        let ordinal = cause.ordinal as usize;
        if ordinal >= self.records.len() {
            panic!("committed cause ordinal must resolve in its provenance view");
        }
        ordinal
    }

    #[cfg(test)]
    pub(crate) fn reverse_unordered_supporters(&mut self)
    where
        D: Clone,
    {
        // Episodes retain their own view of the same records. Cloning the
        // shared vector reverses only this view's storage order.
        for record in Arc::make_mut(&mut self.records) {
            record.reverse_unordered_supporters();
        }
    }

    #[cfg(test)]
    pub(crate) fn supporter_ordinals(&self) -> Vec<u32> {
        self.records
            .iter()
            .flat_map(ProvenanceRecord::supporters)
            .map(|cause| cause.ordinal())
            .collect()
    }

    fn append_pending_pulse_delay(
        &mut self,
        pending: &PendingPulseDelay<D>,
        owner: NodeSubject,
        scheduling_cause: CauseRef,
    ) -> CauseRef {
        let Some(records) = Arc::get_mut(&mut self.records) else {
            panic!("new transaction provenance must be uniquely owned before publication");
        };
        push_record(
            self.scope,
            records,
            ProvenanceRecord::PendingPulseDelay {
                event: pending.key,
                owner,
                origin: pending.origin,
                deadline: pending.deadline,
                count: pending.count,
                revision: pending.revision,
                supporters: vec![scheduling_cause],
            },
        )
    }

    fn append_pending_transport_delay(
        &mut self,
        pending: &PendingTransportDelay<D>,
        owner: NodeSubject,
        scheduling_cause: CauseRef,
    ) -> CauseRef {
        let Some(records) = Arc::get_mut(&mut self.records) else {
            panic!("new transaction provenance must be uniquely owned before publication");
        };
        push_record(
            self.scope,
            records,
            ProvenanceRecord::PendingTransportDelay {
                event: pending.key,
                owner,
                origin: pending.origin,
                deadline: pending.deadline,
                target: pending.target,
                revision: pending.revision,
                supporters: vec![scheduling_cause],
            },
        )
    }

    fn append_pending_inertial_delay(
        &mut self,
        pending: &PendingInertialDelay<D>,
        owner: NodeSubject,
        scheduling_cause: CauseRef,
    ) -> CauseRef {
        let Some(records) = Arc::get_mut(&mut self.records) else {
            panic!("new transaction provenance must be uniquely owned before publication");
        };
        push_record(
            self.scope,
            records,
            ProvenanceRecord::PendingInertialDelay {
                event: pending.key,
                owner,
                origin: pending.origin,
                deadline: pending.deadline,
                target: pending.target,
                revision: pending.revision,
                supporters: vec![scheduling_cause],
            },
        )
    }

    fn append_pending_periodic_boundary(
        &mut self,
        pending: &PendingPeriodicBoundary<D>,
        owner: NodeSubject,
        scheduling_cause: CauseRef,
    ) -> CauseRef {
        let Some(records) = Arc::get_mut(&mut self.records) else {
            panic!("new transaction provenance must be uniquely owned before publication");
        };
        push_record(
            self.scope,
            records,
            ProvenanceRecord::PendingPeriodicBoundary {
                event: pending.key,
                owner,
                origin: pending.origin,
                deadline: pending.deadline,
                anchor: pending.anchor,
                ordinal: pending.ordinal,
                first_emission: pending.first_emission,
                reenable_phase: pending.reenable_phase,
                revision: pending.revision,
                supporters: vec![scheduling_cause],
            },
        )
    }

    fn append_inertial_cancellation(
        &mut self,
        owner: NodeSubject,
        canceled: CauseRef,
        replacement: CauseRef,
    ) -> CauseRef {
        let Some(records) = Arc::get_mut(&mut self.records) else {
            panic!("new transaction provenance must be uniquely owned before publication");
        };
        let subject = match owner {
            NodeSubject::Node(node) => ProvenanceSubject::Node(node),
            NodeSubject::Qualified(node) => ProvenanceSubject::QualifiedNode(node),
        };
        let mut supporters = vec![remap_cause(canceled, self.scope), replacement];
        supporters.sort();
        supporters.dedup();
        push_record(
            self.scope,
            records,
            ProvenanceRecord::Derived {
                subject,
                supporters,
            },
        )
    }
}

/// One committed external output event.
#[non_exhaustive]
pub enum OutputEvent<D> {
    LevelEstablished {
        output: ExternalOutputKey<Level>,
        value: LogicLevel,
        at: Time<D>,
        cause: CauseRef,
        revision: NetworkRevision,
    },
    LevelChanged {
        output: ExternalOutputKey<Level>,
        from: LogicLevel,
        to: LogicLevel,
        at: Time<D>,
        cause: CauseRef,
        revision: NetworkRevision,
    },
    Pulsed {
        output: ExternalOutputKey<Pulse>,
        count: PulseCount,
        at: Time<D>,
        cause: CauseRef,
        revision: NetworkRevision,
    },
}

impl<D> fmt::Debug for OutputEvent<D> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LevelEstablished {
                output,
                value,
                at,
                cause,
                revision,
            } => formatter
                .debug_struct("LevelEstablished")
                .field("output", output)
                .field("value", value)
                .field("at", at)
                .field("cause", cause)
                .field("revision", revision)
                .finish(),
            Self::LevelChanged {
                output,
                from,
                to,
                at,
                cause,
                revision,
            } => formatter
                .debug_struct("LevelChanged")
                .field("output", output)
                .field("from", from)
                .field("to", to)
                .field("at", at)
                .field("cause", cause)
                .field("revision", revision)
                .finish(),
            Self::Pulsed {
                output,
                count,
                at,
                cause,
                revision,
            } => formatter
                .debug_struct("Pulsed")
                .field("output", output)
                .field("count", count)
                .field("at", at)
                .field("cause", cause)
                .field("revision", revision)
                .finish(),
        }
    }
}

/// The owned immutable result of a successful transaction.
pub struct TransactionResult<D> {
    requested_time: Time<D>,
    before_revision: NetworkRevision,
    after_revision: NetworkRevision,
    before_execution_digest: ExecutionStateDigest,
    after_execution_digest: ExecutionStateDigest,
    after_observable_digest: ObservableStateDigest,
    output_events: Vec<OutputEvent<D>>,
    occurrences: Vec<DiagnosticOccurrence<D>>,
    diagnostic_episode_changes: Vec<crate::DiagnosticEpisodeChange<D>>,
    schedule: Schedule<D>,
    provenance: ProvenanceView<D>,
}

impl<D> TransactionResult<D> {
    /// Returns the logical time requested by the applied transaction.
    #[must_use]
    pub const fn requested_time(&self) -> Time<D> {
        self.requested_time
    }

    /// Returns the machine revision observed before application.
    #[must_use]
    pub const fn before_revision(&self) -> NetworkRevision {
        self.before_revision
    }

    /// Returns the machine revision retained after application.
    #[must_use]
    pub const fn after_revision(&self) -> NetworkRevision {
        self.after_revision
    }

    /// Returns the deterministic external output event stream.
    #[must_use]
    pub fn output_events(&self) -> &[OutputEvent<D>] {
        &self.output_events
    }

    /// Returns every committed transient runtime condition in reaction order.
    #[must_use]
    pub fn occurrences(&self) -> &[DiagnosticOccurrence<D>] {
        &self.occurrences
    }

    /// Returns committed episode transitions in chronological, stable-owner order.
    #[must_use]
    pub fn diagnostic_episode_changes(&self) -> &[crate::DiagnosticEpisodeChange<D>] {
        &self.diagnostic_episode_changes
    }

    /// Returns the next temporal wakeup state after this transaction.
    #[must_use]
    pub const fn schedule(&self) -> Schedule<D> {
        self.schedule
    }

    /// Returns the immutable view that resolves every event cause.
    #[must_use]
    pub const fn provenance(&self) -> &ProvenanceView<D> {
        &self.provenance
    }

    /// Returns the execution digest of the machine before this transaction published.
    #[must_use]
    pub const fn before_execution_digest(&self) -> ExecutionStateDigest {
        self.before_execution_digest
    }

    /// Returns the execution digest of the machine after this transaction published.
    #[must_use]
    pub const fn after_execution_digest(&self) -> ExecutionStateDigest {
        self.after_execution_digest
    }

    /// Returns the observable digest of the machine after this transaction published.
    #[must_use]
    pub const fn after_observable_digest(&self) -> ObservableStateDigest {
        self.after_observable_digest
    }
}

impl<D> fmt::Debug for TransactionResult<D> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("TransactionResult")
            .field("requested_time", &self.requested_time)
            .field("before_revision", &self.before_revision)
            .field("after_revision", &self.after_revision)
            .field("before_execution_digest", &self.before_execution_digest)
            .field("after_execution_digest", &self.after_execution_digest)
            .field("after_observable_digest", &self.after_observable_digest)
            .field("output_events", &self.output_events)
            .field("occurrence_count", &self.occurrences.len())
            .field("schedule", &self.schedule)
            .field("provenance_records", &self.provenance.len())
            .finish()
    }
}

impl<D> Machine<D> {
    /// Applies an owned transaction atomically.
    pub fn apply(
        &mut self,
        transaction: Transaction<D>,
    ) -> Result<TransactionResult<D>, RuntimeFailure<D>> {
        let Transaction {
            at,
            expected_revision,
            kind,
        } = transaction;
        match kind {
            TransactionKind::Initialize(input) => {
                self.apply_initialization(at, expected_revision, input)
            }
            TransactionKind::Advance(input) => self.apply_advance(at, expected_revision, input),
        }
    }

    fn apply_initialization(
        &mut self,
        at: Time<D>,
        expected_revision: NetworkRevision,
        input: InputSnapshot<D>,
    ) -> Result<TransactionResult<D>, RuntimeFailure<D>> {
        if self.is_initialized() {
            return Err(RuntimeFailure::new(
                RuntimeFailureEvidence::AlreadyInitialized,
            ));
        }
        if expected_revision != self.store.revision {
            return Err(RuntimeFailure::new(RuntimeFailureEvidence::StaleRevision {
                expected: expected_revision,
                actual: self.store.revision,
            }));
        }
        validate_snapshot_binding::<D>(&self.compiled, &input)?;

        enforce_outer_reaction_budgets::<D>(&self.policy, &self.compiled, 1)?;
        let revision = self.store.revision;
        let (levels, pulses) = input.into_parts();
        let evaluation = evaluate_reaction::<D>(
            &self.compiled,
            &levels,
            &pulses,
            &self.store.edge_observations,
            &self.store.stored_levels,
            at,
            revision,
            &BTreeMap::new(),
        )?;
        let occurrences = pulse_latch_occurrences(&self.compiled, at, revision, &evaluation);
        let mut active_episodes = self.store.active_episodes.clone();
        let mut diagnostic_episode_changes = Vec::new();
        let mut built = build_initialization_provenance(
            &self.compiled,
            revision,
            at,
            &levels,
            &pulses,
            &evaluation,
        );
        let mut created_pending_events = 0_u64;
        let mut pending_events = BTreeMap::new();
        let mut next_pending_event_serial = 0;
        let mut inertial_cancellation_causes = BTreeMap::new();
        let mut periodic_anchors = BTreeMap::new();
        let mut periodic_anchor_causes = BTreeMap::new();
        let mut periodic_cancellation_causes = BTreeMap::new();
        schedule_pulse_delays(
            PulseDelayScheduling {
                compiled: &self.compiled,
                pending: &mut pending_events,
                next_serial: &mut next_pending_event_serial,
                created_events: &mut created_pending_events,
            },
            at,
            revision,
            &evaluation,
            &built.pulse_delay_schedules,
            &built.transport_delay_schedules,
            &built.inertial_delay_schedules,
            &built.periodic_schedules,
            &mut inertial_cancellation_causes,
            &mut periodic_anchors,
            &mut periodic_anchor_causes,
            &mut periodic_cancellation_causes,
            &mut built.provenance,
            &self.policy,
        )?;
        finalize_provenance_build(
            &mut built,
            self.compiled.network_key(),
            self.compiled.fingerprint(),
        );
        let standard_history = crate::standard::stateful::observe_reaction(
            &self.compiled,
            &evaluation,
            &built.operation_causes,
            &BTreeMap::new(),
            |cause| remap_cause(cause, built.provenance.scope),
        );
        remap_cause_map(&mut inertial_cancellation_causes, built.provenance.scope);
        remap_cause_map(&mut periodic_anchor_causes, built.provenance.scope);
        remap_cause_map(&mut periodic_cancellation_causes, built.provenance.scope);
        remap_pending_causes(&mut pending_events, built.provenance.scope);
        reconcile_level_episodes(
            &self.compiled,
            at,
            revision,
            &evaluation,
            &built,
            &mut active_episodes,
            &mut diagnostic_episode_changes,
        );
        enforce_budget::<D>(
            &self.policy,
            RuntimePolicyLimit::MaxRequiredProvenanceGrowth,
            count_as_u64(built.provenance.len()),
        )?;

        let output_events = initialization_events(
            at,
            revision,
            &evaluation.external_outputs,
            &built.output_causes,
            &evaluation.pulse_outputs,
            &built.pulse_output_causes,
        );
        enforce_created_event_budget::<D>(
            &self.policy,
            created_pending_events,
            output_events
                .len()
                .saturating_add(occurrences.len())
                .saturating_add(diagnostic_episode_changes.len()),
        )?;
        let schedule = schedule_from_pending(&pending_events);
        let before_execution_digest = self.execution_state_digest();
        let provenance = built.provenance.clone();
        Ok(publish_success(
            self,
            PublicationReport {
                before_execution_digest,
                requested_time: at,
                revision,
                output_events,
                occurrences,
                diagnostic_episode_changes,
                schedule,
                provenance,
            },
            PublishedCandidate {
                standard_history,
                at,
                levels,
                evaluation,
                input_causes: built.input_causes,
                output_causes: built.output_causes,
                provenance: built.provenance,
                operation_causes: built.operation_causes,
                edge_observation_causes: built.edge_observation_causes,
                toggle_inversion_causes: built.toggle_inversion_causes,
                establishment_causes: built.establishment_causes,
                transport_transition_causes: built.transport_output_transitions,
                inertial_cancellation_causes,
                periodic_anchors,
                periodic_anchor_causes,
                periodic_cancellation_causes,
                active_episodes,
                pending_events,
                next_pending_event_serial,
            },
        ))
    }

    fn apply_advance(
        &mut self,
        at: Time<D>,
        expected_revision: NetworkRevision,
        input: InputDelta<D>,
    ) -> Result<TransactionResult<D>, RuntimeFailure<D>> {
        let MachineStatus::Ready { now } = self.store.status else {
            return Err(RuntimeFailure::new(
                RuntimeFailureEvidence::DeltaBeforeInitialization,
            ));
        };
        if expected_revision != self.store.revision {
            return Err(RuntimeFailure::new(RuntimeFailureEvidence::StaleRevision {
                expected: expected_revision,
                actual: self.store.revision,
            }));
        }
        validate_delta_binding::<D>(&self.compiled, &input)?;
        if at <= now {
            return Err(RuntimeFailure::new(
                RuntimeFailureEvidence::TimeNotStrictlyIncreasing {
                    current_ticks: now.ticks(),
                    requested_ticks: at.ticks(),
                },
            ));
        }

        let revision = self.store.revision;
        let (explicit_levels, pulses) = input.into_parts();
        let mut standard_history = self.store.standard_history.clone();
        let mut levels = self.store.external_levels.clone();
        let mut pending_events = self.store.pending_events.clone();
        let mut next_pending_event_serial = self.store.next_pending_event_serial;
        let mut edge_observations = self.store.edge_observations.clone();
        let mut stored_levels = self.store.stored_levels.clone();
        let mut output_baselines = self.store.output_baselines.clone();
        let mut input_causes = self.store.input_causes.clone();
        let mut output_causes = self.store.output_causes.clone();
        let mut edge_observation_causes = self.store.edge_observation_causes.clone();
        let mut toggle_inversion_causes = self.store.toggle_inversion_causes.clone();
        let mut establishment_causes = self.store.establishment_causes.clone();
        let mut transport_transition_causes = self.store.transport_transition_causes.clone();
        let mut inertial_cancellation_causes = self.store.inertial_cancellation_causes.clone();
        let mut periodic_anchors = self.store.periodic_anchors.clone();
        let mut periodic_anchor_causes = self.store.periodic_anchor_causes.clone();
        let mut periodic_cancellation_causes = self.store.periodic_cancellation_causes.clone();
        let mut provenance = match self.store.provenance.as_ref() {
            Some(provenance) => provenance.clone(),
            None => panic!("ready machine must retain committed provenance"),
        };
        let previous_provenance_len = provenance.len();
        let mut output_events = Vec::new();
        let mut occurrences = Vec::new();
        let mut active_episodes = self.store.active_episodes.clone();
        let mut diagnostic_episode_changes = Vec::new();
        let mut created_pending_events = 0_u64;
        let mut reaction_count = 0_u64;
        let empty_levels = BTreeMap::new();
        let empty_pulses = BTreeMap::new();

        while pending_events
            .keys()
            .next()
            .is_some_and(|deadline| *deadline < at)
        {
            let deadline = match pending_events.keys().next().copied() {
                Some(deadline) => deadline,
                None => panic!("nonempty temporal calendar must have a least deadline"),
            };
            let batch = match pending_events.remove(&deadline) {
                Some(batch) => batch,
                None => panic!("selected temporal deadline must retain its event batch"),
            };
            let due = aggregate_due::<D>(&self.compiled, batch)?;
            let internal = self
                .compiled
                .evaluate_temporal_reaction(
                    &levels,
                    &empty_pulses,
                    &edge_observations,
                    &stored_levels,
                    &due.counts,
                    &due.transport_targets,
                    &due.inertial_targets,
                    &due.periodic_ordinals,
                    deadline,
                    &periodic_anchors,
                )
                .map_err(|failure| {
                    evaluation_failure(&self.compiled, failure, deadline, revision)
                })?;
            occurrences.extend(pulse_latch_occurrences(
                &self.compiled,
                deadline,
                revision,
                &internal,
            ));
            reaction_count = reaction_count.saturating_add(1);
            enforce_outer_reaction_budgets::<D>(&self.policy, &self.compiled, reaction_count)?;

            let mut built = build_ready_provenance(
                &self.compiled,
                revision,
                deadline,
                &empty_levels,
                &empty_pulses,
                &internal,
                &provenance,
                &input_causes,
                &output_causes,
                &output_baselines,
                &edge_observation_causes,
                &toggle_inversion_causes,
                &establishment_causes,
                &transport_transition_causes,
                &periodic_anchor_causes,
                &due.causes,
                &due.transport_causes,
                &due.inertial_causes,
                &due.periodic_causes,
            );
            remap_pending_causes(&mut pending_events, built.provenance.scope);
            remap_output_event_causes(&mut output_events, built.provenance.scope);
            let mut reaction_events = changed_events(
                deadline,
                revision,
                &output_baselines,
                &internal.external_outputs,
                &built.output_causes,
                &internal.pulse_outputs,
                &built.pulse_output_causes,
            );
            output_events.append(&mut reaction_events);

            edge_observations = internal.proposed_edge_observations.clone();
            stored_levels = internal.proposed_stored_levels.clone();
            output_baselines = internal.external_outputs.clone();
            schedule_pulse_delays(
                PulseDelayScheduling {
                    compiled: &self.compiled,
                    pending: &mut pending_events,
                    next_serial: &mut next_pending_event_serial,
                    created_events: &mut created_pending_events,
                },
                deadline,
                revision,
                &internal,
                &built.pulse_delay_schedules,
                &built.transport_delay_schedules,
                &built.inertial_delay_schedules,
                &built.periodic_schedules,
                &mut inertial_cancellation_causes,
                &mut periodic_anchors,
                &mut periodic_anchor_causes,
                &mut periodic_cancellation_causes,
                &mut built.provenance,
                &self.policy,
            )?;
            finalize_provenance_build(
                &mut built,
                self.compiled.network_key(),
                self.compiled.fingerprint(),
            );
            standard_history = crate::standard::stateful::observe_reaction(
                &self.compiled,
                &internal,
                &built.operation_causes,
                &standard_history,
                |cause| remap_cause(cause, built.provenance.scope),
            );
            remap_cause_map(&mut inertial_cancellation_causes, built.provenance.scope);
            remap_cause_map(&mut periodic_anchor_causes, built.provenance.scope);
            remap_cause_map(&mut periodic_cancellation_causes, built.provenance.scope);
            remap_pending_causes(&mut pending_events, built.provenance.scope);
            remap_output_event_causes(&mut output_events, built.provenance.scope);
            reconcile_level_episodes(
                &self.compiled,
                deadline,
                revision,
                &internal,
                &built,
                &mut active_episodes,
                &mut diagnostic_episode_changes,
            );
            remap_episode_changes(&mut diagnostic_episode_changes, built.provenance.scope);
            input_causes = built.input_causes;
            output_causes = built.output_causes;
            edge_observation_causes = built.edge_observation_causes;
            toggle_inversion_causes = built.toggle_inversion_causes;
            establishment_causes = built.establishment_causes;
            transport_transition_causes = built.transport_output_transitions;
            provenance = built.provenance;
            enforce_created_event_budget::<D>(
                &self.policy,
                created_pending_events,
                output_events
                    .len()
                    .saturating_add(occurrences.len())
                    .saturating_add(diagnostic_episode_changes.len()),
            )?;
            enforce_provenance_growth::<D>(
                &self.policy,
                provenance.len(),
                previous_provenance_len,
            )?;
        }

        // Target-time external levels become authoritative only after every
        // strictly earlier internal deadline has completed on candidate state.
        levels.extend(explicit_levels.iter().map(|(key, value)| (*key, *value)));
        let due = pending_events
            .remove(&at)
            .map(|batch| aggregate_due::<D>(&self.compiled, batch))
            .transpose()?
            .unwrap_or_default();
        let evaluation = self
            .compiled
            .evaluate_temporal_reaction(
                &levels,
                &pulses,
                &edge_observations,
                &stored_levels,
                &due.counts,
                &due.transport_targets,
                &due.inertial_targets,
                &due.periodic_ordinals,
                at,
                &periodic_anchors,
            )
            .map_err(|failure| evaluation_failure(&self.compiled, failure, at, revision))?;
        occurrences.extend(pulse_latch_occurrences(
            &self.compiled,
            at,
            revision,
            &evaluation,
        ));
        reaction_count = reaction_count.saturating_add(1);
        enforce_outer_reaction_budgets::<D>(&self.policy, &self.compiled, reaction_count)?;

        let mut built = build_ready_provenance(
            &self.compiled,
            revision,
            at,
            &explicit_levels,
            &pulses,
            &evaluation,
            &provenance,
            &input_causes,
            &output_causes,
            &output_baselines,
            &edge_observation_causes,
            &toggle_inversion_causes,
            &establishment_causes,
            &transport_transition_causes,
            &periodic_anchor_causes,
            &due.causes,
            &due.transport_causes,
            &due.inertial_causes,
            &due.periodic_causes,
        );
        remap_pending_causes(&mut pending_events, built.provenance.scope);
        remap_output_event_causes(&mut output_events, built.provenance.scope);
        let mut final_events = changed_events(
            at,
            revision,
            &output_baselines,
            &evaluation.external_outputs,
            &built.output_causes,
            &evaluation.pulse_outputs,
            &built.pulse_output_causes,
        );
        output_events.append(&mut final_events);
        schedule_pulse_delays(
            PulseDelayScheduling {
                compiled: &self.compiled,
                pending: &mut pending_events,
                next_serial: &mut next_pending_event_serial,
                created_events: &mut created_pending_events,
            },
            at,
            revision,
            &evaluation,
            &built.pulse_delay_schedules,
            &built.transport_delay_schedules,
            &built.inertial_delay_schedules,
            &built.periodic_schedules,
            &mut inertial_cancellation_causes,
            &mut periodic_anchors,
            &mut periodic_anchor_causes,
            &mut periodic_cancellation_causes,
            &mut built.provenance,
            &self.policy,
        )?;
        finalize_provenance_build(
            &mut built,
            self.compiled.network_key(),
            self.compiled.fingerprint(),
        );
        standard_history = crate::standard::stateful::observe_reaction(
            &self.compiled,
            &evaluation,
            &built.operation_causes,
            &standard_history,
            |cause| remap_cause(cause, built.provenance.scope),
        );
        remap_cause_map(&mut inertial_cancellation_causes, built.provenance.scope);
        remap_cause_map(&mut periodic_anchor_causes, built.provenance.scope);
        remap_cause_map(&mut periodic_cancellation_causes, built.provenance.scope);
        remap_pending_causes(&mut pending_events, built.provenance.scope);
        remap_output_event_causes(&mut output_events, built.provenance.scope);
        reconcile_level_episodes(
            &self.compiled,
            at,
            revision,
            &evaluation,
            &built,
            &mut active_episodes,
            &mut diagnostic_episode_changes,
        );
        remap_episode_changes(&mut diagnostic_episode_changes, built.provenance.scope);
        enforce_created_event_budget::<D>(
            &self.policy,
            created_pending_events,
            output_events
                .len()
                .saturating_add(occurrences.len())
                .saturating_add(diagnostic_episode_changes.len()),
        )?;
        enforce_provenance_growth::<D>(
            &self.policy,
            built.provenance.len(),
            previous_provenance_len,
        )?;
        let schedule = schedule_from_pending(&pending_events);
        let before_execution_digest = self.execution_state_digest();
        let provenance = built.provenance.clone();
        Ok(publish_success(
            self,
            PublicationReport {
                before_execution_digest,
                requested_time: at,
                revision,
                output_events,
                occurrences,
                diagnostic_episode_changes,
                schedule,
                provenance,
            },
            PublishedCandidate {
                standard_history,
                at,
                levels,
                evaluation,
                input_causes: built.input_causes,
                output_causes: built.output_causes,
                provenance: built.provenance,
                operation_causes: built.operation_causes,
                edge_observation_causes: built.edge_observation_causes,
                toggle_inversion_causes: built.toggle_inversion_causes,
                establishment_causes: built.establishment_causes,
                transport_transition_causes: built.transport_output_transitions,
                inertial_cancellation_causes,
                periodic_anchors,
                periodic_anchor_causes,
                periodic_cancellation_causes,
                active_episodes,
                pending_events,
                next_pending_event_serial,
            },
        ))
    }
}

fn validate_snapshot_binding<D>(
    compiled: &crate::CompiledNetwork<D>,
    input: &InputSnapshot<D>,
) -> Result<(), RuntimeFailure<D>> {
    if input.network_key() != compiled.network_key() {
        return Err(RuntimeFailure::new(RuntimeFailureEvidence::WrongNetwork {
            expected_key: compiled.network_key(),
            actual_key: input.network_key(),
            expected_fingerprint: compiled.fingerprint(),
            actual_fingerprint: input.network_fingerprint(),
        }));
    }
    if input.input_schema_fingerprint() != compiled.input_schema_fingerprint() {
        return Err(RuntimeFailure::new(
            RuntimeFailureEvidence::ForeignInputSchema {
                expected: compiled.input_schema_fingerprint(),
                actual: input.input_schema_fingerprint(),
            },
        ));
    }
    if input.network_fingerprint() != compiled.fingerprint() {
        return Err(RuntimeFailure::new(RuntimeFailureEvidence::WrongNetwork {
            expected_key: compiled.network_key(),
            actual_key: input.network_key(),
            expected_fingerprint: compiled.fingerprint(),
            actual_fingerprint: input.network_fingerprint(),
        }));
    }
    Ok(())
}

fn validate_delta_binding<D>(
    compiled: &crate::CompiledNetwork<D>,
    input: &InputDelta<D>,
) -> Result<(), RuntimeFailure<D>> {
    if input.network_key() != compiled.network_key() {
        return Err(RuntimeFailure::new(RuntimeFailureEvidence::WrongNetwork {
            expected_key: compiled.network_key(),
            actual_key: input.network_key(),
            expected_fingerprint: compiled.fingerprint(),
            actual_fingerprint: input.network_fingerprint(),
        }));
    }
    let fingerprint_matches = input.network_fingerprint() == compiled.fingerprint();
    let schema_matches = input.input_schema_fingerprint() == compiled.input_schema_fingerprint();
    if !fingerprint_matches && !schema_matches {
        return Err(RuntimeFailure::new(
            RuntimeFailureEvidence::StaleInputSchema {
                expected: compiled.input_schema_fingerprint(),
                actual: input.input_schema_fingerprint(),
            },
        ));
    }
    if !schema_matches {
        return Err(RuntimeFailure::new(
            RuntimeFailureEvidence::ForeignInputSchema {
                expected: compiled.input_schema_fingerprint(),
                actual: input.input_schema_fingerprint(),
            },
        ));
    }
    if !fingerprint_matches {
        return Err(RuntimeFailure::new(RuntimeFailureEvidence::WrongNetwork {
            expected_key: compiled.network_key(),
            actual_key: input.network_key(),
            expected_fingerprint: compiled.fingerprint(),
            actual_fingerprint: input.network_fingerprint(),
        }));
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn evaluate_reaction<D>(
    compiled: &crate::CompiledNetwork<D>,
    levels: &BTreeMap<ExternalInputKey<Level>, LogicLevel>,
    pulses: &BTreeMap<ExternalInputKey<Pulse>, PulseCount>,
    previous_edge_observations: &[EdgeObservation],
    previous_stored_levels: &[LogicLevel],
    at: Time<D>,
    revision: NetworkRevision,
    periodic_anchors: &BTreeMap<NodeKey, Time<D>>,
) -> Result<FullEvaluation, RuntimeFailure<D>> {
    match compiled.evaluate_reaction_with_state(
        levels,
        pulses,
        previous_edge_observations,
        previous_stored_levels,
        at,
        periodic_anchors,
    ) {
        Ok(evaluation) => Ok(evaluation),
        Err(EvaluationFailure::PulseCountOverflow { node, left, right }) => Err(
            RuntimeFailure::new(RuntimeFailureEvidence::PulseCountOverflow {
                node: compiled.node_subject(node),
                left,
                right,
            }),
        ),
        Err(EvaluationFailure::PulseLatchConflict(conflict)) => Err(RuntimeFailure::new(
            pulse_latch_failure(compiled, conflict, at, revision),
        )),
        Err(EvaluationFailure::LevelLatchConflict(conflict)) => Err(RuntimeFailure::new(
            level_latch_failure(compiled, conflict, at, revision),
        )),
        Err(EvaluationFailure::Incomplete) => {
            panic!("validated topology and exact-bound inputs must evaluate completely")
        }
    }
}

fn evaluation_failure<D>(
    compiled: &crate::CompiledNetwork<D>,
    failure: EvaluationFailure,
    at: Time<D>,
    revision: NetworkRevision,
) -> RuntimeFailure<D> {
    match failure {
        EvaluationFailure::PulseCountOverflow { node, left, right } => {
            RuntimeFailure::new(RuntimeFailureEvidence::PulseCountOverflow {
                node: compiled.node_subject(node),
                left,
                right,
            })
        }
        EvaluationFailure::PulseLatchConflict(conflict) => {
            RuntimeFailure::new(pulse_latch_failure(compiled, conflict, at, revision))
        }
        EvaluationFailure::LevelLatchConflict(conflict) => {
            RuntimeFailure::new(level_latch_failure(compiled, conflict, at, revision))
        }
        EvaluationFailure::Incomplete => {
            panic!("validated topology and exact-bound inputs must evaluate completely")
        }
    }
}

fn pulse_latch_failure<D>(
    compiled: &crate::CompiledNetwork<D>,
    conflict: PulseLatchConflict,
    at: Time<D>,
    revision: NetworkRevision,
) -> RuntimeFailureEvidence {
    RuntimeFailureEvidence::PulseLatchConflict {
        node: compiled.node_subject(conflict.node),
        policy: conflict.policy,
        previous: conflict.previous,
        set_count: conflict.set_count,
        reset_count: conflict.reset_count,
        at_ticks: at.ticks(),
        revision,
    }
}

fn level_latch_failure<D>(
    compiled: &crate::CompiledNetwork<D>,
    conflict: LevelLatchConflict,
    at: Time<D>,
    revision: NetworkRevision,
) -> RuntimeFailureEvidence {
    RuntimeFailureEvidence::LevelLatchConflict {
        node: compiled.node_subject(conflict.node),
        policy: conflict.policy,
        previous: conflict.previous,
        set_level: conflict.set_level,
        reset_level: conflict.reset_level,
        at_ticks: at.ticks(),
        revision,
    }
}

fn pulse_latch_occurrences<D>(
    compiled: &crate::CompiledNetwork<D>,
    at: Time<D>,
    revision: NetworkRevision,
    evaluation: &FullEvaluation,
) -> Vec<DiagnosticOccurrence<D>> {
    let mut conflicts = evaluation
        .pulse_latch_conflicts
        .iter()
        .copied()
        .map(|conflict| (compiled.node_subject(conflict.node), conflict))
        .collect::<Vec<_>>();
    // Within one reaction the provisional deterministic order is the complete
    // stable direct-or-qualified primitive subject.
    conflicts.sort_by(|(left, _), (right, _)| left.cmp(right));
    conflicts
        .into_iter()
        .map(|(node, conflict)| {
            let problem = Problem::new(
                match &node {
                    NodeSubject::Node(node) => SubjectRef::Node(*node),
                    NodeSubject::Qualified(node) => SubjectRef::QualifiedNode(node.clone()),
                },
                Vec::new(),
                ProblemEvidence::RuntimePulseLatchConflictRetained {
                    evidence: ConflictEvidence {
                        node: node_evidence(&node),
                        policy: conflict.policy,
                        previous: conflict.previous,
                        controls: ConflictControls::Pulse {
                            set: conflict.set_count,
                            reset: conflict.reset_count,
                        },
                        at_ticks: at.ticks(),
                        revision,
                    },
                    marker: PhantomData,
                },
            );
            match DiagnosticOccurrence::new(problem, at, revision) {
                Ok(occurrence) => occurrence,
                Err(_) => panic!(
                    "pulse latch retained-conflict registry entry must permit occurrence delivery"
                ),
            }
        })
        .collect()
}

#[allow(clippy::too_many_arguments)]
fn reconcile_level_episodes<D>(
    compiled: &crate::CompiledNetwork<D>,
    at: Time<D>,
    revision: NetworkRevision,
    evaluation: &FullEvaluation,
    built: &ProvenanceBuild<D>,
    active: &mut crate::episode::ActiveEpisodes<D>,
    changes: &mut Vec<crate::DiagnosticEpisodeChange<D>>,
) {
    let problems = evaluation
        .level_latch_conflicts
        .iter()
        .map(|conflict| {
            let node = compiled.node_subject(conflict.node);
            Problem::new(
                match &node {
                    NodeSubject::Node(node) => SubjectRef::Node(*node),
                    NodeSubject::Qualified(node) => SubjectRef::QualifiedNode(node.clone()),
                },
                Vec::new(),
                ProblemEvidence::RuntimeLevelLatchConflictRetained {
                    evidence: ConflictEvidence {
                        node: node_evidence(&node),
                        policy: conflict.policy,
                        previous: conflict.previous,
                        controls: ConflictControls::Level {
                            set: conflict.set_level,
                            reset: conflict.reset_level,
                        },
                        at_ticks: at.ticks(),
                        revision,
                    },
                    marker: PhantomData,
                },
            )
        })
        .collect();
    let causes = evaluation
        .causes
        .iter()
        .enumerate()
        .filter_map(|(index, cause)| match cause {
            EvaluationCause::LevelSetResetLatch { node, .. } => Some((
                compiled.node_subject(*node),
                operation_cause(&built.operation_causes, index),
            )),
            _ => None,
        })
        .collect();
    crate::episode::reconcile(
        active,
        changes,
        compiled.network_key(),
        at,
        problems,
        &causes,
        &built.provenance,
    );
}

fn remap_episode_changes<D>(
    changes: &mut [crate::DiagnosticEpisodeChange<D>],
    scope: ProvenanceScope,
) {
    for change in changes {
        change.remap_cause(remap_cause(change.cause(), scope));
    }
}

#[derive(Default)]
struct DuePulseDelays {
    counts: BTreeMap<NodeKey, PulseCount>,
    causes: BTreeMap<NodeKey, Vec<CauseRef>>,
    transport_targets: BTreeMap<NodeKey, LogicLevel>,
    transport_origins: BTreeMap<NodeKey, u64>,
    transport_causes: BTreeMap<NodeKey, Vec<CauseRef>>,
    inertial_targets: BTreeMap<NodeKey, LogicLevel>,
    inertial_origins: BTreeMap<NodeKey, u64>,
    inertial_causes: BTreeMap<NodeKey, Vec<CauseRef>>,
    periodic_ordinals: BTreeMap<NodeKey, u64>,
    periodic_causes: BTreeMap<NodeKey, Vec<CauseRef>>,
}

struct PulseDelayScheduling<'a, D> {
    compiled: &'a crate::CompiledNetwork<D>,
    pending: &'a mut BTreeMap<Time<D>, Vec<PendingEvent<D>>>,
    next_serial: &'a mut u64,
    created_events: &'a mut u64,
}

fn aggregate_due<D>(
    compiled: &crate::CompiledNetwork<D>,
    batch: Vec<PendingEvent<D>>,
) -> Result<DuePulseDelays, RuntimeFailure<D>> {
    let mut due = DuePulseDelays::default();
    for event in batch {
        match event {
            PendingEvent::PulseDelay(event) => {
                let previous = due
                    .counts
                    .get(&event.node)
                    .copied()
                    .unwrap_or(PulseCount::ZERO);
                let combined = previous.checked_add(event.count).map_err(|_| {
                    RuntimeFailure::new(RuntimeFailureEvidence::PulseCountOverflow {
                        node: compiled.node_subject(event.node),
                        left: previous,
                        right: event.count,
                    })
                })?;
                due.counts.insert(event.node, combined);
                due.causes.entry(event.node).or_default().push(event.cause);
            }
            PendingEvent::TransportDelay(event) => {
                let replace = match due.transport_origins.get(&event.node).copied() {
                    None => true,
                    Some(previous) if event.origin.ticks() > previous => true,
                    Some(previous) if event.origin.ticks() < previous => false,
                    Some(_) => due
                        .transport_targets
                        .get(&event.node)
                        .is_none_or(|target| event.target > *target),
                };
                if replace {
                    due.transport_targets.insert(event.node, event.target);
                    due.transport_origins
                        .insert(event.node, event.origin.ticks());
                }
                due.transport_causes
                    .entry(event.node)
                    .or_default()
                    .push(event.cause);
            }
            PendingEvent::Inertial(event) => {
                let replace = match due.inertial_origins.get(&event.node).copied() {
                    None => true,
                    Some(previous) if event.origin.ticks() > previous => true,
                    Some(previous) if event.origin.ticks() < previous => false,
                    Some(_) => due
                        .inertial_targets
                        .get(&event.node)
                        .is_none_or(|target| event.target > *target),
                };
                if replace {
                    due.inertial_targets.insert(event.node, event.target);
                    due.inertial_origins
                        .insert(event.node, event.origin.ticks());
                }
                due.inertial_causes
                    .entry(event.node)
                    .or_default()
                    .push(event.cause);
            }
            PendingEvent::Periodic(event) => {
                due.periodic_ordinals.insert(event.node, event.ordinal);
                due.periodic_causes
                    .entry(event.node)
                    .or_default()
                    .push(event.cause);
            }
        }
    }
    for causes in due.causes.values_mut() {
        causes.sort();
        causes.dedup();
    }
    for causes in due.transport_causes.values_mut() {
        causes.sort();
        causes.dedup();
    }
    for causes in due.inertial_causes.values_mut() {
        causes.sort();
        causes.dedup();
    }
    for causes in due.periodic_causes.values_mut() {
        causes.sort();
        causes.dedup();
    }
    Ok(due)
}

#[allow(clippy::too_many_arguments)]
fn schedule_pulse_delays<D>(
    scheduling: PulseDelayScheduling<'_, D>,
    origin: Time<D>,
    revision: NetworkRevision,
    evaluation: &FullEvaluation,
    proposal_causes: &BTreeMap<NodeKey, CauseRef>,
    transport_proposal_causes: &BTreeMap<NodeKey, CauseRef>,
    inertial_proposal_causes: &BTreeMap<NodeKey, CauseRef>,
    periodic_proposal_causes: &BTreeMap<NodeKey, CauseRef>,
    cancellation_causes: &mut BTreeMap<NodeKey, CauseRef>,
    periodic_anchors: &mut BTreeMap<NodeKey, Time<D>>,
    periodic_anchor_causes: &mut BTreeMap<NodeKey, CauseRef>,
    periodic_cancellation_causes: &mut BTreeMap<NodeKey, CauseRef>,
    provenance: &mut ProvenanceView<D>,
    policy: &RuntimePolicy,
) -> Result<(), RuntimeFailure<D>> {
    enum Proposal<'a> {
        Pulse(&'a crate::compile::PulseDelayProposal),
        Transport(&'a crate::compile::TransportDelayProposal),
        Inertial(&'a crate::compile::InertialDelayProposal),
        Periodic(&'a crate::compile::PeriodicProposal),
    }
    let mut proposals = evaluation
        .pulse_delay_proposals
        .iter()
        .map(|proposal| (proposal.node, Proposal::Pulse(proposal)))
        .collect::<Vec<_>>();
    proposals.extend(
        evaluation
            .transport_delay_proposals
            .iter()
            .map(|proposal| (proposal.node, Proposal::Transport(proposal))),
    );
    proposals.extend(
        evaluation
            .inertial_delay_proposals
            .iter()
            .map(|proposal| (proposal.node, Proposal::Inertial(proposal))),
    );
    proposals.extend(
        evaluation
            .periodic_proposals
            .iter()
            .map(|proposal| (proposal.node, Proposal::Periodic(proposal))),
    );
    // Mixed temporal kinds allocate public serials by stable owner identity.
    proposals.sort_by_key(|(node, _)| *node);
    for (_, proposal) in proposals {
        match proposal {
            Proposal::Pulse(proposal) => {
                let deadline = origin
                    .checked_add(Span::from_ticks(proposal.delay_ticks))
                    .map_err(|_| {
                        RuntimeFailure::new(RuntimeFailureEvidence::TimeOverflow {
                            node: scheduling.compiled.node_subject(proposal.node),
                            origin_ticks: origin.ticks(),
                            delay_ticks: proposal.delay_ticks,
                        })
                    })?;
                let key = PendingEventKey::from_serial(*scheduling.next_serial);
                *scheduling.next_serial =
                    scheduling.next_serial.checked_add(1).ok_or_else(|| {
                        RuntimeFailure::new(RuntimeFailureEvidence::BudgetExceeded {
                            budget: RuntimePolicyLimit::MaxEventsCreatedPerTransaction,
                            limit: policy.max_events_created_per_transaction(),
                            consumed: u64::MAX,
                        })
                    })?;
                let scheduling_cause = match proposal_causes.get(&proposal.node).copied() {
                    Some(cause) => cause,
                    None => panic!("every PulseDelay proposal must retain a scheduling cause"),
                };
                let mut pending = PendingPulseDelay {
                    key,
                    node: proposal.node,
                    origin,
                    deadline,
                    count: proposal.count,
                    revision,
                    cause: scheduling_cause,
                };
                pending.cause = provenance.append_pending_pulse_delay(
                    &pending,
                    scheduling.compiled.node_subject(pending.node),
                    scheduling_cause,
                );
                scheduling
                    .pending
                    .entry(deadline)
                    .or_default()
                    .push(PendingEvent::PulseDelay(pending));
                *scheduling.created_events = scheduling.created_events.saturating_add(1);
                enforce_budget::<D>(
                    policy,
                    RuntimePolicyLimit::MaxEventsCreatedPerTransaction,
                    *scheduling.created_events,
                )?;
            }
            Proposal::Transport(proposal) => {
                let deadline = origin
                    .checked_add(Span::from_ticks(proposal.delay_ticks))
                    .map_err(|_| {
                        RuntimeFailure::new(RuntimeFailureEvidence::TransportTimeOverflow {
                            node: scheduling.compiled.node_subject(proposal.node),
                            origin_ticks: origin.ticks(),
                            delay_ticks: proposal.delay_ticks,
                        })
                    })?;
                let key = PendingEventKey::from_serial(*scheduling.next_serial);
                *scheduling.next_serial =
                    scheduling.next_serial.checked_add(1).ok_or_else(|| {
                        RuntimeFailure::new(RuntimeFailureEvidence::BudgetExceeded {
                            budget: RuntimePolicyLimit::MaxEventsCreatedPerTransaction,
                            limit: policy.max_events_created_per_transaction(),
                            consumed: u64::MAX,
                        })
                    })?;
                let scheduling_cause = match transport_proposal_causes.get(&proposal.node).copied()
                {
                    Some(cause) => cause,
                    None => panic!("every TransportDelay proposal must retain a scheduling cause"),
                };
                let mut pending = PendingTransportDelay {
                    key,
                    node: proposal.node,
                    origin,
                    deadline,
                    target: proposal.target,
                    revision,
                    cause: scheduling_cause,
                };
                pending.cause = provenance.append_pending_transport_delay(
                    &pending,
                    scheduling.compiled.node_subject(pending.node),
                    scheduling_cause,
                );
                scheduling
                    .pending
                    .entry(deadline)
                    .or_default()
                    .push(PendingEvent::TransportDelay(pending));
                *scheduling.created_events = scheduling.created_events.saturating_add(1);
                enforce_budget::<D>(
                    policy,
                    RuntimePolicyLimit::MaxEventsCreatedPerTransaction,
                    *scheduling.created_events,
                )?;
            }
            Proposal::Inertial(proposal) => {
                let scheduling_cause = match inertial_proposal_causes.get(&proposal.node).copied() {
                    Some(cause) => cause,
                    None => panic!("every InertialDelay proposal must retain a scheduling cause"),
                };
                if let Some(canceled) = remove_inertial_candidate(scheduling.pending, proposal.node)
                {
                    let cancellation = provenance.append_inertial_cancellation(
                        scheduling.compiled.node_subject(proposal.node),
                        canceled.cause,
                        scheduling_cause,
                    );
                    cancellation_causes.insert(proposal.node, cancellation);
                }
                if proposal.target == proposal.output {
                    continue;
                }
                let deadline = origin
                    .checked_add(Span::from_ticks(proposal.delay_ticks))
                    .map_err(|_| {
                        RuntimeFailure::new(RuntimeFailureEvidence::InertialTimeOverflow {
                            node: scheduling.compiled.node_subject(proposal.node),
                            origin_ticks: origin.ticks(),
                            delay_ticks: proposal.delay_ticks,
                        })
                    })?;
                let key = PendingEventKey::from_serial(*scheduling.next_serial);
                *scheduling.next_serial =
                    scheduling.next_serial.checked_add(1).ok_or_else(|| {
                        RuntimeFailure::new(RuntimeFailureEvidence::BudgetExceeded {
                            budget: RuntimePolicyLimit::MaxEventsCreatedPerTransaction,
                            limit: policy.max_events_created_per_transaction(),
                            consumed: u64::MAX,
                        })
                    })?;
                let mut pending = PendingInertialDelay {
                    key,
                    node: proposal.node,
                    origin,
                    deadline,
                    target: proposal.target,
                    revision,
                    cause: scheduling_cause,
                };
                pending.cause = provenance.append_pending_inertial_delay(
                    &pending,
                    scheduling.compiled.node_subject(pending.node),
                    scheduling_cause,
                );
                scheduling
                    .pending
                    .entry(deadline)
                    .or_default()
                    .push(PendingEvent::Inertial(pending));
                *scheduling.created_events = scheduling.created_events.saturating_add(1);
                enforce_budget::<D>(
                    policy,
                    RuntimePolicyLimit::MaxEventsCreatedPerTransaction,
                    *scheduling.created_events,
                )?;
            }
            Proposal::Periodic(proposal) => {
                // SPEC: docs/specs/contracts/periodic.yaml
                // "machine-owned-recurring-work" — retain at most one
                // strictly future boundary while enabled and no calendar work while disabled.
                let scheduling_cause = match periodic_proposal_causes.get(&proposal.node).copied() {
                    Some(cause) => cause,
                    None => panic!("every Periodic proposal must retain a scheduling cause"),
                };
                let transitioned = proposal.previous_enable != proposal.enable;
                if proposal.enable.is_low() {
                    if transitioned || proposal.due_ordinal.is_some() {
                        let canceled = remove_periodic_boundary(scheduling.pending, proposal.node)
                            .map(|event| event.cause)
                            .unwrap_or(scheduling_cause);
                        let cancellation = provenance.append_inertial_cancellation(
                            scheduling.compiled.node_subject(proposal.node),
                            canceled,
                            scheduling_cause,
                        );
                        periodic_cancellation_causes.insert(proposal.node, cancellation);
                    }
                    if transitioned
                        && proposal.reenable_phase
                            == crate::authored::ReenablePhasePolicy::RestartPhase
                    {
                        periodic_anchors.remove(&proposal.node);
                        periodic_anchor_causes.remove(&proposal.node);
                    }
                    continue;
                }

                let rising = proposal.previous_enable.is_low();
                if !rising && proposal.due_ordinal.is_none() {
                    continue;
                }
                if rising
                    && (proposal.reenable_phase
                        == crate::authored::ReenablePhasePolicy::RestartPhase
                        || !periodic_anchors.contains_key(&proposal.node))
                {
                    periodic_anchors.insert(proposal.node, origin);
                    periodic_anchor_causes.insert(proposal.node, scheduling_cause);
                }
                let anchor = match periodic_anchors.get(&proposal.node).copied() {
                    Some(anchor) => anchor,
                    None => panic!("enabled Periodic proposal must retain a phase anchor"),
                };
                let ordinal = if let Some(due) = proposal.due_ordinal {
                    due.checked_add(1)
                } else {
                    let elapsed = origin.ticks().checked_sub(anchor.ticks());
                    elapsed
                        .map(|ticks| ticks / proposal.period_ticks)
                        .and_then(|completed| completed.checked_add(1))
                }
                .ok_or_else(|| {
                    RuntimeFailure::new(RuntimeFailureEvidence::PeriodicTimeOverflow {
                        node: scheduling.compiled.node_subject(proposal.node),
                        origin_ticks: origin.ticks(),
                        period_ticks: proposal.period_ticks,
                    })
                })?;
                let offset = proposal.period_ticks.checked_mul(ordinal).ok_or_else(|| {
                    RuntimeFailure::new(RuntimeFailureEvidence::PeriodicTimeOverflow {
                        node: scheduling.compiled.node_subject(proposal.node),
                        origin_ticks: origin.ticks(),
                        period_ticks: proposal.period_ticks,
                    })
                })?;
                let deadline = anchor.checked_add(Span::from_ticks(offset)).map_err(|_| {
                    RuntimeFailure::new(RuntimeFailureEvidence::PeriodicTimeOverflow {
                        node: scheduling.compiled.node_subject(proposal.node),
                        origin_ticks: origin.ticks(),
                        period_ticks: proposal.period_ticks,
                    })
                })?;
                if deadline <= origin {
                    return Err(RuntimeFailure::new(
                        RuntimeFailureEvidence::PeriodicTimeOverflow {
                            node: scheduling.compiled.node_subject(proposal.node),
                            origin_ticks: origin.ticks(),
                            period_ticks: proposal.period_ticks,
                        },
                    ));
                }
                let key = PendingEventKey::from_serial(*scheduling.next_serial);
                *scheduling.next_serial =
                    scheduling.next_serial.checked_add(1).ok_or_else(|| {
                        RuntimeFailure::new(RuntimeFailureEvidence::BudgetExceeded {
                            budget: RuntimePolicyLimit::MaxEventsCreatedPerTransaction,
                            limit: policy.max_events_created_per_transaction(),
                            consumed: u64::MAX,
                        })
                    })?;
                let mut pending = PendingPeriodicBoundary {
                    key,
                    node: proposal.node,
                    origin,
                    deadline,
                    anchor,
                    ordinal,
                    first_emission: proposal.first_emission,
                    reenable_phase: proposal.reenable_phase,
                    revision,
                    cause: scheduling_cause,
                };
                pending.cause = provenance.append_pending_periodic_boundary(
                    &pending,
                    scheduling.compiled.node_subject(pending.node),
                    scheduling_cause,
                );
                scheduling
                    .pending
                    .entry(deadline)
                    .or_default()
                    .push(PendingEvent::Periodic(pending));
                *scheduling.created_events = scheduling.created_events.saturating_add(1);
                enforce_budget::<D>(
                    policy,
                    RuntimePolicyLimit::MaxEventsCreatedPerTransaction,
                    *scheduling.created_events,
                )?;
            }
        }
    }
    let pending_count = scheduling.pending.values().map(Vec::len).sum::<usize>();
    enforce_budget::<D>(
        policy,
        RuntimePolicyLimit::MaxPendingEvents,
        count_as_u64(pending_count),
    )?;
    Ok(())
}

fn remove_inertial_candidate<D>(
    pending: &mut BTreeMap<Time<D>, Vec<PendingEvent<D>>>,
    node: NodeKey,
) -> Option<PendingInertialDelay<D>> {
    let deadlines = pending.keys().copied().collect::<Vec<_>>();
    let mut removed = None;
    for deadline in deadlines {
        let Some(batch) = pending.get_mut(&deadline) else {
            continue;
        };
        let mut retained = Vec::with_capacity(batch.len());
        for event in batch.drain(..) {
            match event {
                PendingEvent::Inertial(candidate)
                    if candidate.node == node && removed.is_none() =>
                {
                    removed = Some(candidate);
                }
                other => retained.push(other),
            }
        }
        *batch = retained;
        if batch.is_empty() {
            pending.remove(&deadline);
        }
        if removed.is_some() {
            break;
        }
    }
    removed
}

fn remove_periodic_boundary<D>(
    pending: &mut BTreeMap<Time<D>, Vec<PendingEvent<D>>>,
    node: NodeKey,
) -> Option<PendingPeriodicBoundary<D>> {
    let deadlines = pending.keys().copied().collect::<Vec<_>>();
    let mut removed = None;
    for deadline in deadlines {
        let Some(batch) = pending.get_mut(&deadline) else {
            continue;
        };
        let mut retained = Vec::with_capacity(batch.len());
        for event in batch.drain(..) {
            match event {
                PendingEvent::Periodic(boundary) if boundary.node == node && removed.is_none() => {
                    removed = Some(boundary);
                }
                other => retained.push(other),
            }
        }
        *batch = retained;
        if batch.is_empty() {
            pending.remove(&deadline);
        }
        if removed.is_some() {
            break;
        }
    }
    removed
}

fn enforce_outer_reaction_budgets<D>(
    policy: &RuntimePolicy,
    compiled: &crate::CompiledNetwork<D>,
    reaction_count: u64,
) -> Result<(), RuntimeFailure<D>> {
    enforce_budget::<D>(
        policy,
        RuntimePolicyLimit::MaxInternalReactions,
        reaction_count,
    )?;
    let operations = reaction_count.saturating_mul(count_as_u64(compiled.operation_count()));
    enforce_budget::<D>(
        policy,
        RuntimePolicyLimit::MaxEvaluatedOperations,
        operations,
    )
}

fn enforce_created_event_budget<D>(
    policy: &RuntimePolicy,
    pending_created: u64,
    output_events: usize,
) -> Result<(), RuntimeFailure<D>> {
    enforce_budget::<D>(
        policy,
        RuntimePolicyLimit::MaxEventsCreatedPerTransaction,
        pending_created.saturating_add(count_as_u64(output_events)),
    )
}

fn enforce_provenance_growth<D>(
    policy: &RuntimePolicy,
    current_len: usize,
    previous_len: usize,
) -> Result<(), RuntimeFailure<D>> {
    enforce_budget::<D>(
        policy,
        RuntimePolicyLimit::MaxRequiredProvenanceGrowth,
        count_as_u64(current_len.saturating_sub(previous_len)),
    )
}

fn schedule_from_pending<D>(pending: &BTreeMap<Time<D>, Vec<PendingEvent<D>>>) -> Schedule<D> {
    match pending.keys().next().copied() {
        Some(deadline) => Schedule::WakeAt(deadline),
        None => Schedule::Dormant,
    }
}

fn remap_pending_causes<D>(
    pending: &mut BTreeMap<Time<D>, Vec<PendingEvent<D>>>,
    scope: ProvenanceScope,
) {
    for event in pending.values_mut().flatten() {
        match event {
            PendingEvent::PulseDelay(event) => event.cause = remap_cause(event.cause, scope),
            PendingEvent::TransportDelay(event) => event.cause = remap_cause(event.cause, scope),
            PendingEvent::Inertial(event) => event.cause = remap_cause(event.cause, scope),
            PendingEvent::Periodic(event) => event.cause = remap_cause(event.cause, scope),
        }
    }
}

fn remap_cause_map(causes: &mut BTreeMap<NodeKey, CauseRef>, scope: ProvenanceScope) {
    for cause in causes.values_mut() {
        *cause = remap_cause(*cause, scope);
    }
}

fn remap_output_event_causes<D>(events: &mut [OutputEvent<D>], scope: ProvenanceScope) {
    for event in events {
        let cause = match event {
            OutputEvent::LevelEstablished { cause, .. }
            | OutputEvent::LevelChanged { cause, .. }
            | OutputEvent::Pulsed { cause, .. } => cause,
        };
        *cause = remap_cause(*cause, scope);
    }
}

struct PublishedCandidate<D> {
    standard_history:
        BTreeMap<crate::QualifiedModuleRef, crate::standard::stateful::StandardHistory>,
    at: Time<D>,
    levels: BTreeMap<ExternalInputKey<Level>, LogicLevel>,
    evaluation: FullEvaluation,
    input_causes: BTreeMap<ExternalInputKey<Level>, CauseRef>,
    output_causes: BTreeMap<ExternalOutputKey<Level>, CauseRef>,
    provenance: ProvenanceView<D>,
    operation_causes: Vec<CauseRef>,
    edge_observation_causes: BTreeMap<NodeKey, CauseRef>,
    toggle_inversion_causes: BTreeMap<NodeKey, CauseRef>,
    establishment_causes: BTreeMap<NodeKey, CauseRef>,
    transport_transition_causes: BTreeMap<NodeKey, CauseRef>,
    inertial_cancellation_causes: BTreeMap<NodeKey, CauseRef>,
    periodic_anchors: BTreeMap<NodeKey, Time<D>>,
    periodic_anchor_causes: BTreeMap<NodeKey, CauseRef>,
    periodic_cancellation_causes: BTreeMap<NodeKey, CauseRef>,
    active_episodes: crate::episode::ActiveEpisodes<D>,
    pending_events: BTreeMap<Time<D>, Vec<PendingEvent<D>>>,
    next_pending_event_serial: u64,
}

struct PublicationReport<D> {
    before_execution_digest: ExecutionStateDigest,
    requested_time: Time<D>,
    revision: NetworkRevision,
    output_events: Vec<OutputEvent<D>>,
    occurrences: Vec<DiagnosticOccurrence<D>>,
    diagnostic_episode_changes: Vec<crate::DiagnosticEpisodeChange<D>>,
    schedule: Schedule<D>,
    provenance: ProvenanceView<D>,
}

fn publish_success<D>(
    machine: &mut Machine<D>,
    report: PublicationReport<D>,
    candidate: PublishedCandidate<D>,
) -> TransactionResult<D> {
    publish_candidate(machine, candidate);
    TransactionResult {
        requested_time: report.requested_time,
        before_revision: report.revision,
        after_revision: report.revision,
        before_execution_digest: report.before_execution_digest,
        after_execution_digest: machine.execution_state_digest(),
        after_observable_digest: machine.observable_state_digest(),
        output_events: report.output_events,
        occurrences: report.occurrences,
        diagnostic_episode_changes: report.diagnostic_episode_changes,
        schedule: report.schedule,
        provenance: report.provenance,
    }
}

fn publish_candidate<D>(machine: &mut Machine<D>, published: PublishedCandidate<D>) {
    let PublishedCandidate {
        standard_history,
        at,
        levels,
        evaluation,
        input_causes,
        output_causes,
        provenance,
        operation_causes,
        edge_observation_causes,
        toggle_inversion_causes,
        establishment_causes,
        transport_transition_causes,
        inertial_cancellation_causes,
        periodic_anchors,
        periodic_anchor_causes,
        periodic_cancellation_causes,
        active_episodes,
        pending_events,
        next_pending_event_serial,
    } = published;
    // SPEC: docs/specs/processor_and_runtime_architecture.md §50 "Reference execution strategy"
    // Every fallible step precedes replacement of the complete private candidate.
    let mut candidate = machine.store.clone();
    candidate.standard_history = standard_history;
    candidate.status = MachineStatus::Ready { now: at };
    candidate.external_levels = levels;
    candidate.settled_levels = evaluation.values;
    // SPEC: docs/specs/contracts/reaction-scoped-pulse-foundation.yaml
    // "lifecycle-transaction-integration" — only Level operation values survive publication.
    candidate.operation_levels = evaluation.operation_levels;
    candidate.operation_causes = operation_causes;
    candidate.output_baselines = evaluation.external_outputs;
    candidate.input_causes = input_causes;
    candidate.output_causes = output_causes;
    candidate.provenance = Some(provenance);
    candidate.edge_observations = evaluation.proposed_edge_observations;
    candidate.edge_observation_causes = edge_observation_causes;
    candidate.stored_levels = evaluation.proposed_stored_levels;
    candidate.toggle_inversion_causes = toggle_inversion_causes;
    candidate.establishment_causes = establishment_causes;
    candidate.transport_transition_causes = transport_transition_causes;
    candidate.inertial_cancellation_causes = inertial_cancellation_causes;
    candidate.periodic_anchors = periodic_anchors;
    candidate.periodic_anchor_causes = periodic_anchor_causes;
    candidate.periodic_cancellation_causes = periodic_cancellation_causes;
    candidate.active_episodes = active_episodes;
    candidate.pending_events = pending_events;
    candidate.next_pending_event_serial = next_pending_event_serial;
    machine.store = candidate;
}

fn count_as_u64(count: usize) -> u64 {
    u64::try_from(count).unwrap_or(u64::MAX)
}

fn enforce_budget<D>(
    policy: &RuntimePolicy,
    budget: RuntimePolicyLimit,
    consumed: u64,
) -> Result<(), RuntimeFailure<D>> {
    let limit = match budget {
        RuntimePolicyLimit::MaxInternalReactions => policy.max_internal_reactions(),
        RuntimePolicyLimit::MaxEvaluatedOperations => policy.max_evaluated_operations(),
        RuntimePolicyLimit::MaxPendingEvents => policy.max_pending_events(),
        RuntimePolicyLimit::MaxEventsCreatedPerTransaction => {
            policy.max_events_created_per_transaction()
        }
        RuntimePolicyLimit::MaxRequiredProvenanceGrowth => policy.max_required_provenance_growth(),
    };
    if consumed > limit {
        return Err(RuntimeFailure::new(
            RuntimeFailureEvidence::BudgetExceeded {
                budget,
                limit,
                consumed,
            },
        ));
    }
    Ok(())
}

fn hash_cause(hasher: &mut blake3::Hasher, cause: CauseRef) {
    hasher.update(&cause.ordinal.to_be_bytes());
}

fn hash_causes(hasher: &mut blake3::Hasher, causes: &[CauseRef]) {
    hasher.update(&count_as_u64(causes.len()).to_be_bytes());
    for cause in causes {
        hash_cause(hasher, *cause);
    }
}

fn hash_node_subject(hasher: &mut blake3::Hasher, subject: &NodeSubject) {
    match subject {
        NodeSubject::Node(node) => {
            hasher.update(&[0]);
            hasher.update(&node.as_u128().to_be_bytes());
        }
        NodeSubject::Qualified(node) => {
            hasher.update(&[1]);
            hasher.update(&count_as_u64(node.instances().len()).to_be_bytes());
            for instance in node.instances() {
                hasher.update(&instance.as_u128().to_be_bytes());
            }
            hasher.update(&node.node().as_u128().to_be_bytes());
        }
    }
}

fn hash_provenance_subject(hasher: &mut blake3::Hasher, subject: &ProvenanceSubject) {
    match subject {
        ProvenanceSubject::Node(node) => {
            hasher.update(&[0]);
            hasher.update(&node.as_u128().to_be_bytes());
        }
        ProvenanceSubject::QualifiedNode(node) => {
            hasher.update(&[1]);
            hasher.update(&count_as_u64(node.instances().len()).to_be_bytes());
            for instance in node.instances() {
                hasher.update(&instance.as_u128().to_be_bytes());
            }
            hasher.update(&node.node().as_u128().to_be_bytes());
        }
        ProvenanceSubject::ExternalOutput(output) => {
            hasher.update(&[2]);
            hasher.update(&output.as_u128().to_be_bytes());
        }
        ProvenanceSubject::PulseExternalOutput(output) => {
            hasher.update(&[3]);
            hasher.update(&output.as_u128().to_be_bytes());
        }
    }
}

fn hash_pulse_port_subject(hasher: &mut blake3::Hasher, subject: &PulsePortSubject) {
    match subject {
        PulsePortSubject::Port(port) => {
            hasher.update(&[0]);
            hasher.update(&port.as_u128().to_be_bytes());
        }
        PulsePortSubject::Qualified(port) => {
            hasher.update(&[1]);
            hasher.update(&count_as_u64(port.instances().len()).to_be_bytes());
            for instance in port.instances() {
                hasher.update(&instance.as_u128().to_be_bytes());
            }
            match port.port() {
                crate::key::AnyInPortKey::Level(key) => {
                    hasher.update(&[0]);
                    hasher.update(&key.as_u128().to_be_bytes());
                }
                crate::key::AnyInPortKey::Pulse(key) => {
                    hasher.update(&[1]);
                    hasher.update(&key.as_u128().to_be_bytes());
                }
            }
        }
    }
}

fn hash_provenance_record<D>(hasher: &mut blake3::Hasher, record: &ProvenanceRecord<D>) {
    match record {
        ProvenanceRecord::InitializationTransaction { at, revision } => {
            hasher.update(&[0]);
            hasher.update(&at.ticks().to_be_bytes());
            hasher.update(&revision.value().to_be_bytes());
        }
        ProvenanceRecord::ReadyTransaction { at, revision } => {
            hasher.update(&[1]);
            hasher.update(&at.ticks().to_be_bytes());
            hasher.update(&revision.value().to_be_bytes());
        }
        ProvenanceRecord::ExternalObservation { input, value } => {
            hasher.update(&[2]);
            hasher.update(&input.as_u128().to_be_bytes());
            hasher.update(&[u8::from(value.is_high())]);
        }
        ProvenanceRecord::ExternalPulseObservation { input, count } => {
            hasher.update(&[3]);
            hasher.update(&input.as_u128().to_be_bytes());
            hasher.update(&count.get().to_be_bytes());
        }
        ProvenanceRecord::PendingPulseDelay {
            event,
            owner,
            origin,
            deadline,
            count,
            revision,
            supporters,
        } => {
            hasher.update(&[4]);
            hasher.update(&event.value().to_be_bytes());
            hash_node_subject(hasher, owner);
            hasher.update(&origin.ticks().to_be_bytes());
            hasher.update(&deadline.ticks().to_be_bytes());
            hasher.update(&count.get().to_be_bytes());
            hasher.update(&revision.value().to_be_bytes());
            hash_causes(hasher, supporters);
        }
        ProvenanceRecord::PendingTransportDelay {
            event,
            owner,
            origin,
            deadline,
            target,
            revision,
            supporters,
        } => {
            hasher.update(&[8]);
            hasher.update(&event.value().to_be_bytes());
            hash_node_subject(hasher, owner);
            hasher.update(&origin.ticks().to_be_bytes());
            hasher.update(&deadline.ticks().to_be_bytes());
            hasher.update(&[u8::from(target.is_high())]);
            hasher.update(&revision.value().to_be_bytes());
            hash_causes(hasher, supporters);
        }
        ProvenanceRecord::PendingInertialDelay {
            event,
            owner,
            origin,
            deadline,
            target,
            revision,
            supporters,
        } => {
            hasher.update(&[9]);
            hasher.update(&event.value().to_be_bytes());
            hash_node_subject(hasher, owner);
            hasher.update(&origin.ticks().to_be_bytes());
            hasher.update(&deadline.ticks().to_be_bytes());
            hasher.update(&[u8::from(target.is_high())]);
            hasher.update(&revision.value().to_be_bytes());
            hash_causes(hasher, supporters);
        }
        ProvenanceRecord::PendingPeriodicBoundary {
            event,
            owner,
            origin,
            deadline,
            anchor,
            ordinal,
            first_emission,
            reenable_phase,
            revision,
            supporters,
        } => {
            hasher.update(&[10]);
            hasher.update(&event.value().to_be_bytes());
            hash_node_subject(hasher, owner);
            hasher.update(&origin.ticks().to_be_bytes());
            hasher.update(&deadline.ticks().to_be_bytes());
            hasher.update(&anchor.ticks().to_be_bytes());
            hasher.update(&ordinal.to_be_bytes());
            hasher.update(&[match first_emission {
                crate::authored::FirstEmissionPolicy::Immediate => 0,
                crate::authored::FirstEmissionPolicy::AfterFirstPeriod => 1,
            }]);
            hasher.update(&[match reenable_phase {
                crate::authored::ReenablePhasePolicy::RestartPhase => 0,
                crate::authored::ReenablePhasePolicy::PreservePhase => 1,
            }]);
            hasher.update(&revision.value().to_be_bytes());
            hash_causes(hasher, supporters);
        }
        ProvenanceRecord::Derived {
            subject,
            supporters,
        } => {
            hasher.update(&[5]);
            hash_provenance_subject(hasher, subject);
            hash_causes(hasher, supporters);
        }
        ProvenanceRecord::PulseDerived {
            subject,
            contributions,
            result,
            supporters,
        } => {
            hasher.update(&[6]);
            hash_provenance_subject(hasher, subject);
            hasher.update(&count_as_u64(contributions.len()).to_be_bytes());
            for contribution in contributions {
                hash_pulse_port_subject(hasher, &contribution.port);
                hasher.update(&contribution.count.get().to_be_bytes());
                hash_cause(hasher, contribution.cause);
            }
            hasher.update(&result.get().to_be_bytes());
            hash_causes(hasher, supporters);
        }
        ProvenanceRecord::PulseControlledLevel {
            subject,
            contributions,
            result,
            supporters,
        } => {
            hasher.update(&[7]);
            hash_provenance_subject(hasher, subject);
            hasher.update(&count_as_u64(contributions.len()).to_be_bytes());
            for contribution in contributions {
                hash_pulse_port_subject(hasher, &contribution.port);
                hasher.update(&contribution.count.get().to_be_bytes());
                hash_cause(hasher, contribution.cause);
            }
            hasher.update(&[u8::from(result.is_high())]);
            hash_causes(hasher, supporters);
        }
    }
}

fn provenance_view_scope<D>(
    network_key: NetworkKey,
    fingerprint: NetworkFingerprint,
    records: &[ProvenanceRecord<D>],
) -> ProvenanceScope {
    // SPEC: docs/specs/contracts/ready-level-transaction.yaml "resolvable-ready-causes"
    // The lookup authority commits to scope-neutral graph content, not transaction coordinates.
    let mut hasher = blake3::Hasher::new();
    hasher.update(PROVENANCE_VIEW_SCOPE_DOMAIN);
    hasher.update(&network_key.as_u128().to_be_bytes());
    hasher.update(&fingerprint.as_bytes());
    hasher.update(&count_as_u64(records.len()).to_be_bytes());
    for record in records {
        hash_provenance_record(&mut hasher, record);
    }
    *hasher.finalize().as_bytes()
}

fn build_initialization_provenance<D>(
    compiled: &crate::CompiledNetwork<D>,
    revision: NetworkRevision,
    at: Time<D>,
    levels: &BTreeMap<ExternalInputKey<Level>, LogicLevel>,
    pulses: &BTreeMap<ExternalInputKey<Pulse>, PulseCount>,
    evaluation: &FullEvaluation,
) -> ProvenanceBuild<D> {
    let scope = UNFINALIZED_PROVENANCE_SCOPE;
    let mut records = Vec::new();
    let transaction_cause = push_record(
        scope,
        &mut records,
        ProvenanceRecord::InitializationTransaction { at, revision },
    );
    let input_causes = levels
        .iter()
        .map(|(input, value)| {
            let cause = push_record(
                scope,
                &mut records,
                ProvenanceRecord::ExternalObservation {
                    input: *input,
                    value: *value,
                },
            );
            (*input, cause)
        })
        .collect::<BTreeMap<_, _>>();
    let pulse_input_causes = pulses
        .iter()
        .map(|(input, count)| {
            let cause = push_record(
                scope,
                &mut records,
                ProvenanceRecord::ExternalPulseObservation {
                    input: *input,
                    count: *count,
                },
            );
            (*input, cause)
        })
        .collect::<BTreeMap<_, _>>();
    let evaluation_causes = append_evaluation_provenance(
        scope,
        &mut records,
        transaction_cause,
        evaluation,
        EvaluationProvenanceInputs {
            compiled,
            levels: &input_causes,
            pulses: &pulse_input_causes,
            due_pulse_delays: &BTreeMap::new(),
            due_transport_delays: &BTreeMap::new(),
            due_inertial_delays: &BTreeMap::new(),
            due_periodic_boundaries: &BTreeMap::new(),
        },
        None,
        None,
    );

    ProvenanceBuild {
        provenance: ProvenanceView {
            scope,
            records: Arc::new(records),
        },
        operation_causes: evaluation_causes.operation_causes,
        input_causes,
        output_causes: evaluation_causes.level_outputs,
        pulse_output_causes: evaluation_causes.pulse_outputs,
        edge_observation_causes: evaluation_causes.edge_observations,
        toggle_inversion_causes: evaluation_causes.toggle_inversions,
        establishment_causes: evaluation_causes.establishments,
        transport_output_transitions: evaluation_causes.transport_output_transitions,
        pulse_delay_schedules: evaluation_causes.pulse_delay_schedules,
        transport_delay_schedules: evaluation_causes.transport_delay_schedules,
        inertial_delay_schedules: evaluation_causes.inertial_delay_schedules,
        periodic_schedules: evaluation_causes.periodic_schedules,
    }
}

#[allow(clippy::too_many_arguments)]
fn build_ready_provenance<D>(
    compiled: &crate::CompiledNetwork<D>,
    revision: NetworkRevision,
    at: Time<D>,
    explicit_levels: &BTreeMap<ExternalInputKey<Level>, LogicLevel>,
    pulses: &BTreeMap<ExternalInputKey<Pulse>, PulseCount>,
    evaluation: &FullEvaluation,
    previous: &ProvenanceView<D>,
    previous_input_causes: &BTreeMap<ExternalInputKey<Level>, CauseRef>,
    previous_output_causes: &BTreeMap<ExternalOutputKey<Level>, CauseRef>,
    previous_output_baselines: &BTreeMap<ExternalOutputKey<Level>, LogicLevel>,
    previous_edge_observation_causes: &BTreeMap<NodeKey, CauseRef>,
    previous_toggle_inversion_causes: &BTreeMap<NodeKey, CauseRef>,
    previous_establishment_causes: &BTreeMap<NodeKey, CauseRef>,
    previous_transport_transition_causes: &BTreeMap<NodeKey, CauseRef>,
    previous_periodic_anchor_causes: &BTreeMap<NodeKey, CauseRef>,
    due_pulse_delays: &BTreeMap<NodeKey, Vec<CauseRef>>,
    due_transport_delays: &BTreeMap<NodeKey, Vec<CauseRef>>,
    due_inertial_delays: &BTreeMap<NodeKey, Vec<CauseRef>>,
    due_periodic_boundaries: &BTreeMap<NodeKey, Vec<CauseRef>>,
) -> ProvenanceBuild<D> {
    let scope = UNFINALIZED_PROVENANCE_SCOPE;
    let mut records = previous
        .records
        .iter()
        .map(|record| remap_record(record, scope))
        .collect::<Vec<_>>();
    let transaction_cause = push_record(
        scope,
        &mut records,
        ProvenanceRecord::ReadyTransaction { at, revision },
    );
    let mut input_causes = previous_input_causes
        .iter()
        .map(|(input, cause)| (*input, remap_cause(*cause, scope)))
        .collect::<BTreeMap<_, _>>();
    for (input, value) in explicit_levels {
        let cause = push_record(
            scope,
            &mut records,
            ProvenanceRecord::ExternalObservation {
                input: *input,
                value: *value,
            },
        );
        input_causes.insert(*input, cause);
    }
    let pulse_input_causes = pulses
        .iter()
        .map(|(input, count)| {
            let cause = push_record(
                scope,
                &mut records,
                ProvenanceRecord::ExternalPulseObservation {
                    input: *input,
                    count: *count,
                },
            );
            (*input, cause)
        })
        .collect::<BTreeMap<_, _>>();
    let remapped_output_causes = previous_output_causes
        .iter()
        .map(|(output, cause)| (*output, remap_cause(*cause, scope)))
        .collect::<BTreeMap<_, _>>();
    let evaluation_causes = append_evaluation_provenance(
        scope,
        &mut records,
        transaction_cause,
        evaluation,
        EvaluationProvenanceInputs {
            compiled,
            levels: &input_causes,
            pulses: &pulse_input_causes,
            due_pulse_delays,
            due_transport_delays,
            due_inertial_delays,
            due_periodic_boundaries,
        },
        Some(PreviousLevelOutputs {
            causes: &remapped_output_causes,
            baselines: previous_output_baselines,
        }),
        Some(PreviousStateCauses {
            edge_observations: previous_edge_observation_causes,
            toggle_inversions: previous_toggle_inversion_causes,
            establishments: previous_establishment_causes,
            transport_transitions: previous_transport_transition_causes,
            periodic_anchors: previous_periodic_anchor_causes,
        }),
    );

    ProvenanceBuild {
        provenance: ProvenanceView {
            scope,
            records: Arc::new(records),
        },
        operation_causes: evaluation_causes.operation_causes,
        input_causes,
        output_causes: evaluation_causes.level_outputs,
        pulse_output_causes: evaluation_causes.pulse_outputs,
        edge_observation_causes: evaluation_causes.edge_observations,
        toggle_inversion_causes: evaluation_causes.toggle_inversions,
        establishment_causes: evaluation_causes.establishments,
        transport_output_transitions: evaluation_causes.transport_output_transitions,
        pulse_delay_schedules: evaluation_causes.pulse_delay_schedules,
        transport_delay_schedules: evaluation_causes.transport_delay_schedules,
        inertial_delay_schedules: evaluation_causes.inertial_delay_schedules,
        periodic_schedules: evaluation_causes.periodic_schedules,
    }
}

struct EvaluationCauseMaps {
    operation_causes: Vec<CauseRef>,
    level_outputs: BTreeMap<ExternalOutputKey<Level>, CauseRef>,
    pulse_outputs: BTreeMap<ExternalOutputKey<Pulse>, CauseRef>,
    edge_observations: BTreeMap<NodeKey, CauseRef>,
    toggle_inversions: BTreeMap<NodeKey, CauseRef>,
    establishments: BTreeMap<NodeKey, CauseRef>,
    transport_output_transitions: BTreeMap<NodeKey, CauseRef>,
    pulse_delay_schedules: BTreeMap<NodeKey, CauseRef>,
    transport_delay_schedules: BTreeMap<NodeKey, CauseRef>,
    inertial_delay_schedules: BTreeMap<NodeKey, CauseRef>,
    periodic_schedules: BTreeMap<NodeKey, CauseRef>,
}

fn append_evaluation_provenance<D>(
    scope: ProvenanceScope,
    records: &mut Vec<ProvenanceRecord<D>>,
    transaction_cause: CauseRef,
    evaluation: &FullEvaluation,
    input_causes: EvaluationProvenanceInputs<'_, D>,
    previous_outputs: Option<PreviousLevelOutputs<'_>>,
    previous_state: Option<PreviousStateCauses<'_>>,
) -> EvaluationCauseMaps {
    let previous_output_causes = previous_outputs.as_ref().map(|previous| previous.causes);
    let previous_output_baselines = previous_outputs.as_ref().map(|previous| previous.baselines);
    let mut operation_causes = Vec::with_capacity(evaluation.causes.len());
    let mut output_causes = BTreeMap::new();
    let mut pulse_output_causes = BTreeMap::new();
    let mut edge_observation_causes = previous_state
        .as_ref()
        .into_iter()
        .flat_map(|previous| previous.edge_observations.iter())
        .map(|(node, cause)| (*node, remap_cause(*cause, scope)))
        .collect::<BTreeMap<_, _>>();
    let mut toggle_inversion_causes = previous_state
        .as_ref()
        .into_iter()
        .flat_map(|previous| previous.toggle_inversions.iter())
        .map(|(node, cause)| (*node, remap_cause(*cause, scope)))
        .collect::<BTreeMap<_, _>>();
    let mut establishment_causes = previous_state
        .as_ref()
        .into_iter()
        .flat_map(|previous| previous.establishments.iter())
        .map(|(node, cause)| (*node, remap_cause(*cause, scope)))
        .collect::<BTreeMap<_, _>>();
    let mut transport_output_transitions = previous_state
        .as_ref()
        .into_iter()
        .flat_map(|previous| previous.transport_transitions.iter())
        .map(|(node, cause)| (*node, remap_cause(*cause, scope)))
        .collect::<BTreeMap<_, _>>();
    let periodic_anchor_causes = previous_state
        .as_ref()
        .into_iter()
        .flat_map(|previous| previous.periodic_anchors.iter())
        .map(|(node, cause)| (*node, remap_cause(*cause, scope)))
        .collect::<BTreeMap<_, _>>();
    let mut transport_delay_schedules = BTreeMap::new();

    for cause in &evaluation.causes {
        let resolved = match cause {
            EvaluationCause::ExternalInput(input) => {
                let Some(cause) = input_causes.levels.get(input).copied() else {
                    panic!("evaluated external input must retain one authoritative cause");
                };
                cause
            }
            EvaluationCause::PulseExternalInput(input) => input_causes
                .pulses
                .get(input)
                .copied()
                .unwrap_or(transaction_cause),
            EvaluationCause::Constant(node) => push_record(
                scope,
                records,
                ProvenanceRecord::Derived {
                    subject: provenance_subject(input_causes.compiled, *node),
                    supporters: vec![transaction_cause],
                },
            ),
            EvaluationCause::Node { node, predecessors } => {
                let mut supporters = vec![transaction_cause];
                for predecessor in predecessors {
                    supporters.push(operation_cause(&operation_causes, *predecessor));
                }
                supporters.sort();
                supporters.dedup();
                push_record(
                    scope,
                    records,
                    ProvenanceRecord::Derived {
                        subject: provenance_subject(input_causes.compiled, *node),
                        supporters,
                    },
                )
            }
            EvaluationCause::PulseCombinational {
                node,
                contributions,
                result,
                supporters: cause_supporters,
            } => {
                let mut grouped = contributions
                    .iter()
                    .map(|contribution| PulseContribution {
                        port: input_causes.compiled.pulse_port_subject(contribution.port),
                        count: contribution.count,
                        cause: operation_cause(&operation_causes, contribution.source),
                    })
                    .collect::<Vec<_>>();
                grouped.sort_by(|left, right| left.port.cmp(&right.port));
                let mut supporters = vec![transaction_cause];
                supporters.extend(grouped.iter().map(|contribution| contribution.cause));
                supporters.extend(
                    cause_supporters
                        .iter()
                        .map(|source| operation_cause(&operation_causes, *source)),
                );
                supporters.sort();
                supporters.dedup();
                push_record(
                    scope,
                    records,
                    ProvenanceRecord::PulseDerived {
                        subject: provenance_subject(input_causes.compiled, *node),
                        contributions: grouped,
                        result: *result,
                        supporters,
                    },
                )
            }
            EvaluationCause::PulseRoute { node, control } => {
                let mut supporters = vec![
                    transaction_cause,
                    operation_cause(&operation_causes, *control),
                ];
                supporters.sort();
                supporters.dedup();
                push_record(
                    scope,
                    records,
                    ProvenanceRecord::Derived {
                        subject: provenance_subject(input_causes.compiled, *node),
                        supporters,
                    },
                )
            }
            EvaluationCause::PulseRouteOutput {
                node,
                contribution,
                result,
                source,
                selected,
            } => {
                let route_cause = operation_cause(&operation_causes, *source);
                // SPEC: docs/specs/contracts/level-controlled-pulse.yaml
                // "selected-output-provenance" — suppressed output has control-only support;
                // selected output retains the complete pulse batch, including zero.
                let grouped = if *selected {
                    vec![PulseContribution {
                        port: input_causes.compiled.pulse_port_subject(contribution.port),
                        count: contribution.count,
                        cause: operation_cause(&operation_causes, contribution.source),
                    }]
                } else {
                    Vec::new()
                };
                let mut supporters = vec![transaction_cause, route_cause];
                if let Some(contribution) = grouped.first() {
                    supporters.push(contribution.cause);
                }
                supporters.sort();
                supporters.dedup();
                push_record(
                    scope,
                    records,
                    ProvenanceRecord::PulseDerived {
                        subject: provenance_subject(input_causes.compiled, *node),
                        contributions: grouped,
                        result: *result,
                        supporters,
                    },
                )
            }
            EvaluationCause::EdgeDetector {
                node,
                input,
                previous,
                current: _,
                emitted: _,
            } => {
                let mut supporters = vec![
                    transaction_cause,
                    operation_cause(&operation_causes, *input),
                ];
                if matches!(previous, EdgeObservation::Established(_)) && previous_state.is_some() {
                    let Some(previous_cause) = edge_observation_causes.get(node).copied() else {
                        panic!(
                            "ready edge detector must retain the cause of its previous observation"
                        );
                    };
                    supporters.push(previous_cause);
                }
                supporters.sort();
                supporters.dedup();
                let reference = push_record(
                    scope,
                    records,
                    ProvenanceRecord::Derived {
                        subject: provenance_subject(input_causes.compiled, *node),
                        supporters,
                    },
                );
                edge_observation_causes.insert(*node, reference);
                reference
            }
            EvaluationCause::Toggle {
                node,
                input,
                count: _,
                previous: _,
                result: _,
                inverted,
            } => {
                let mut supporters = vec![
                    transaction_cause,
                    operation_cause(&operation_causes, *input),
                ];
                if let Some(previous_inversion) = toggle_inversion_causes.get(node).copied() {
                    supporters.push(previous_inversion);
                }
                supporters.sort();
                supporters.dedup();
                let reference = push_record(
                    scope,
                    records,
                    ProvenanceRecord::Derived {
                        subject: provenance_subject(input_causes.compiled, *node),
                        supporters,
                    },
                );
                if *inverted {
                    toggle_inversion_causes.insert(*node, reference);
                }
                reference
            }
            EvaluationCause::PulseSetResetLatch {
                node,
                set,
                reset,
                set_port,
                reset_port,
                set_count,
                reset_count,
                previous: _,
                result,
                policy: _,
            } => {
                let mut contributions = vec![
                    PulseContribution {
                        port: input_causes.compiled.pulse_port_subject(*set_port),
                        count: *set_count,
                        cause: operation_cause(&operation_causes, *set),
                    },
                    PulseContribution {
                        port: input_causes.compiled.pulse_port_subject(*reset_port),
                        count: *reset_count,
                        cause: operation_cause(&operation_causes, *reset),
                    },
                ];
                contributions.sort_by(|left, right| left.port.cmp(&right.port));
                let mut supporters = vec![transaction_cause];
                supporters.extend(contributions.iter().map(|entry| entry.cause));
                if let Some(previous_cause) = establishment_causes.get(node).copied() {
                    supporters.push(previous_cause);
                }
                supporters.sort();
                supporters.dedup();
                let reference = push_record(
                    scope,
                    records,
                    ProvenanceRecord::PulseControlledLevel {
                        subject: provenance_subject(input_causes.compiled, *node),
                        contributions,
                        result: *result,
                        supporters,
                    },
                );
                if previous_state.is_none() || evaluation.stored_level_establishments.contains(node)
                {
                    establishment_causes.insert(*node, reference);
                }
                reference
            }
            EvaluationCause::LevelSetResetLatch {
                node, set, reset, ..
            } => {
                let mut supporters = vec![
                    transaction_cause,
                    operation_cause(&operation_causes, *set),
                    operation_cause(&operation_causes, *reset),
                ];
                if let Some(previous) = establishment_causes.get(node).copied() {
                    supporters.push(previous);
                }
                supporters.sort();
                supporters.dedup();
                let reference = push_record(
                    scope,
                    records,
                    ProvenanceRecord::Derived {
                        subject: provenance_subject(input_causes.compiled, *node),
                        supporters,
                    },
                );
                if previous_state.is_none() || evaluation.stored_level_establishments.contains(node)
                {
                    establishment_causes.insert(*node, reference);
                }
                reference
            }
            EvaluationCause::SampleHold {
                node,
                value,
                sample,
                sample_port,
                sample_count,
                result,
            } => {
                let mut supporters = vec![
                    transaction_cause,
                    operation_cause(&operation_causes, *value),
                ];
                let contributions = vec![PulseContribution {
                    port: input_causes.compiled.pulse_port_subject(*sample_port),
                    count: *sample_count,
                    cause: operation_cause(&operation_causes, *sample),
                }];
                supporters.push(contributions[0].cause);
                if sample_count.is_zero()
                    && let Some(previous) = establishment_causes.get(node).copied()
                {
                    supporters.push(previous);
                }
                supporters.sort();
                supporters.dedup();
                let reference = push_record(
                    scope,
                    records,
                    ProvenanceRecord::PulseControlledLevel {
                        subject: provenance_subject(input_causes.compiled, *node),
                        contributions,
                        result: *result,
                        supporters,
                    },
                );
                // SPEC: docs/specs/contracts/sample-hold.yaml "held-state-inspection-and-causality"
                // Retention keeps its authoritative history; captures remain explainable.
                // We treat every positive sample, including equal values, as an establishment.
                if previous_state.is_none() || evaluation.stored_level_establishments.contains(node)
                {
                    establishment_causes.insert(*node, reference);
                }
                reference
            }
            EvaluationCause::PulseDelay { node } => {
                let mut supporters = vec![transaction_cause];
                supporters.extend(
                    input_causes
                        .due_pulse_delays
                        .get(node)
                        .into_iter()
                        .flatten()
                        .map(|cause| remap_cause(*cause, scope)),
                );
                supporters.sort();
                supporters.dedup();
                push_record(
                    scope,
                    records,
                    ProvenanceRecord::Derived {
                        subject: provenance_subject(input_causes.compiled, *node),
                        supporters,
                    },
                )
            }
            EvaluationCause::TransportDelay { node } => {
                let mut supporters = vec![transaction_cause];
                if let Some(previous) = transport_output_transitions.get(node).copied() {
                    supporters.push(previous);
                }
                supporters.extend(
                    input_causes
                        .due_transport_delays
                        .get(node)
                        .into_iter()
                        .flatten()
                        .map(|cause| remap_cause(*cause, scope)),
                );
                supporters.sort();
                supporters.dedup();
                let reference = push_record(
                    scope,
                    records,
                    ProvenanceRecord::Derived {
                        subject: provenance_subject(input_causes.compiled, *node),
                        supporters,
                    },
                );
                // SPEC: docs/specs/contracts/transport-delay.yaml "focused-temporal-state-and-causality"
                // The latest output transition advances only when the output changes.
                if previous_state.is_none() || evaluation.stored_level_establishments.contains(node)
                {
                    transport_output_transitions.insert(*node, reference);
                }
                reference
            }
            EvaluationCause::InertialDelay { node } => {
                let mut supporters = vec![transaction_cause];
                if let Some(previous) = transport_output_transitions.get(node).copied() {
                    supporters.push(previous);
                }
                supporters.extend(
                    input_causes
                        .due_inertial_delays
                        .get(node)
                        .into_iter()
                        .flatten()
                        .map(|cause| remap_cause(*cause, scope)),
                );
                supporters.sort();
                supporters.dedup();
                let reference = push_record(
                    scope,
                    records,
                    ProvenanceRecord::Derived {
                        subject: provenance_subject(input_causes.compiled, *node),
                        supporters,
                    },
                );
                if previous_state.is_none() || evaluation.stored_level_establishments.contains(node)
                {
                    transport_output_transitions.insert(*node, reference);
                }
                reference
            }
            EvaluationCause::Periodic {
                node,
                enable,
                due: _,
                emitted: _,
            } => {
                let mut supporters = vec![
                    transaction_cause,
                    operation_cause(&operation_causes, *enable),
                ];
                supporters.extend(
                    input_causes
                        .due_periodic_boundaries
                        .get(node)
                        .into_iter()
                        .flatten()
                        .map(|cause| remap_cause(*cause, scope)),
                );
                if let Some(anchor) = periodic_anchor_causes.get(node).copied() {
                    supporters.push(anchor);
                }
                supporters.sort();
                supporters.dedup();
                push_record(
                    scope,
                    records,
                    ProvenanceRecord::Derived {
                        subject: provenance_subject(input_causes.compiled, *node),
                        supporters,
                    },
                )
            }
            EvaluationCause::Alias(source) => operation_cause(&operation_causes, *source),
            EvaluationCause::ExternalOutput { output, source } => {
                let unchanged_cause = previous_output_baselines
                    .and_then(|baselines| baselines.get(output))
                    .zip(evaluation.external_outputs.get(output))
                    .filter(|(before, after)| before == after)
                    .and(previous_output_causes)
                    .and_then(|causes| causes.get(output))
                    .copied();
                if let Some(cause) = unchanged_cause {
                    output_causes.insert(*output, cause);
                    operation_causes.push(cause);
                    continue;
                }
                let mut supporters = vec![
                    transaction_cause,
                    operation_cause(&operation_causes, *source),
                ];
                supporters.sort();
                supporters.dedup();
                let reference = push_record(
                    scope,
                    records,
                    ProvenanceRecord::Derived {
                        subject: ProvenanceSubject::ExternalOutput(*output),
                        supporters,
                    },
                );
                output_causes.insert(*output, reference);
                reference
            }
            EvaluationCause::PulseExternalOutput { output, source } => {
                let mut supporters = vec![
                    transaction_cause,
                    operation_cause(&operation_causes, *source),
                ];
                supporters.sort();
                supporters.dedup();
                let reference = push_record(
                    scope,
                    records,
                    ProvenanceRecord::Derived {
                        subject: ProvenanceSubject::PulseExternalOutput(*output),
                        supporters,
                    },
                );
                pulse_output_causes.insert(*output, reference);
                reference
            }
        };
        operation_causes.push(resolved);
    }
    let mut pulse_delay_schedules = BTreeMap::new();
    for proposal in &evaluation.pulse_delay_proposals {
        let mut supporters = vec![
            transaction_cause,
            operation_cause(&operation_causes, proposal.input_source),
        ];
        supporters.sort();
        supporters.dedup();
        let reference = push_record(
            scope,
            records,
            ProvenanceRecord::Derived {
                subject: provenance_subject(input_causes.compiled, proposal.node),
                supporters,
            },
        );
        pulse_delay_schedules.insert(proposal.node, reference);
    }
    for proposal in &evaluation.transport_delay_proposals {
        let mut supporters = vec![
            transaction_cause,
            operation_cause(&operation_causes, proposal.input_source),
        ];
        supporters.sort();
        supporters.dedup();
        let reference = push_record(
            scope,
            records,
            ProvenanceRecord::Derived {
                subject: provenance_subject(input_causes.compiled, proposal.node),
                supporters,
            },
        );
        transport_delay_schedules.insert(proposal.node, reference);
    }
    let mut inertial_delay_schedules = BTreeMap::new();
    for proposal in &evaluation.inertial_delay_proposals {
        let mut supporters = vec![
            transaction_cause,
            operation_cause(&operation_causes, proposal.input_source),
        ];
        supporters.sort();
        supporters.dedup();
        let reference = push_record(
            scope,
            records,
            ProvenanceRecord::Derived {
                subject: provenance_subject(input_causes.compiled, proposal.node),
                supporters,
            },
        );
        inertial_delay_schedules.insert(proposal.node, reference);
    }
    let mut periodic_schedules = BTreeMap::new();
    for proposal in &evaluation.periodic_proposals {
        let mut supporters = vec![
            transaction_cause,
            operation_cause(&operation_causes, proposal.input_source),
        ];
        supporters.extend(
            input_causes
                .due_periodic_boundaries
                .get(&proposal.node)
                .into_iter()
                .flatten()
                .map(|cause| remap_cause(*cause, scope)),
        );
        if let Some(anchor) = periodic_anchor_causes.get(&proposal.node).copied() {
            supporters.push(anchor);
        }
        supporters.sort();
        supporters.dedup();
        let reference = push_record(
            scope,
            records,
            ProvenanceRecord::Derived {
                subject: provenance_subject(input_causes.compiled, proposal.node),
                supporters,
            },
        );
        periodic_schedules.insert(proposal.node, reference);
    }
    EvaluationCauseMaps {
        operation_causes,
        level_outputs: output_causes,
        pulse_outputs: pulse_output_causes,
        edge_observations: edge_observation_causes,
        toggle_inversions: toggle_inversion_causes,
        establishments: establishment_causes,
        transport_output_transitions,
        pulse_delay_schedules,
        transport_delay_schedules,
        inertial_delay_schedules,
        periodic_schedules,
    }
}

fn provenance_subject<D>(compiled: &crate::CompiledNetwork<D>, node: NodeKey) -> ProvenanceSubject {
    match compiled.node_subject(node) {
        NodeSubject::Node(node) => ProvenanceSubject::Node(node),
        NodeSubject::Qualified(node) => ProvenanceSubject::QualifiedNode(node),
    }
}

fn remap_cause(cause: CauseRef, scope: ProvenanceScope) -> CauseRef {
    CauseRef {
        scope,
        ordinal: cause.ordinal,
    }
}

fn finalize_provenance_build<D>(
    build: &mut ProvenanceBuild<D>,
    network_key: NetworkKey,
    fingerprint: NetworkFingerprint,
) {
    let scope = provenance_view_scope(network_key, fingerprint, build.provenance.records.as_ref());
    let Some(records) = Arc::get_mut(&mut build.provenance.records) else {
        panic!("unpublished transaction provenance must be uniquely owned during finalization");
    };
    for record in records {
        *record = remap_record(record, scope);
    }
    build.provenance.scope = scope;
    for cause in &mut build.operation_causes {
        *cause = remap_cause(*cause, scope);
    }
    for cause in build.input_causes.values_mut() {
        *cause = remap_cause(*cause, scope);
    }
    for cause in build.output_causes.values_mut() {
        *cause = remap_cause(*cause, scope);
    }
    for cause in build.pulse_output_causes.values_mut() {
        *cause = remap_cause(*cause, scope);
    }
    for cause in build.edge_observation_causes.values_mut() {
        *cause = remap_cause(*cause, scope);
    }
    for cause in build.toggle_inversion_causes.values_mut() {
        *cause = remap_cause(*cause, scope);
    }
    for cause in build.establishment_causes.values_mut() {
        *cause = remap_cause(*cause, scope);
    }
    for cause in build.transport_output_transitions.values_mut() {
        *cause = remap_cause(*cause, scope);
    }
    for cause in build.pulse_delay_schedules.values_mut() {
        *cause = remap_cause(*cause, scope);
    }
    for cause in build.transport_delay_schedules.values_mut() {
        *cause = remap_cause(*cause, scope);
    }
    for cause in build.inertial_delay_schedules.values_mut() {
        *cause = remap_cause(*cause, scope);
    }
    for cause in build.periodic_schedules.values_mut() {
        *cause = remap_cause(*cause, scope);
    }
}

fn remap_record<D>(record: &ProvenanceRecord<D>, scope: ProvenanceScope) -> ProvenanceRecord<D> {
    match record {
        ProvenanceRecord::InitializationTransaction { at, revision } => {
            ProvenanceRecord::InitializationTransaction {
                at: *at,
                revision: *revision,
            }
        }
        ProvenanceRecord::ReadyTransaction { at, revision } => ProvenanceRecord::ReadyTransaction {
            at: *at,
            revision: *revision,
        },
        ProvenanceRecord::ExternalObservation { input, value } => {
            ProvenanceRecord::ExternalObservation {
                input: *input,
                value: *value,
            }
        }
        ProvenanceRecord::ExternalPulseObservation { input, count } => {
            ProvenanceRecord::ExternalPulseObservation {
                input: *input,
                count: *count,
            }
        }
        ProvenanceRecord::PendingPulseDelay {
            event,
            owner,
            origin,
            deadline,
            count,
            revision,
            supporters,
        } => ProvenanceRecord::PendingPulseDelay {
            event: *event,
            owner: owner.clone(),
            origin: *origin,
            deadline: *deadline,
            count: *count,
            revision: *revision,
            supporters: supporters
                .iter()
                .map(|cause| remap_cause(*cause, scope))
                .collect(),
        },
        ProvenanceRecord::PendingPeriodicBoundary {
            event,
            owner,
            origin,
            deadline,
            anchor,
            ordinal,
            first_emission,
            reenable_phase,
            revision,
            supporters,
        } => ProvenanceRecord::PendingPeriodicBoundary {
            event: *event,
            owner: owner.clone(),
            origin: *origin,
            deadline: *deadline,
            anchor: *anchor,
            ordinal: *ordinal,
            first_emission: *first_emission,
            reenable_phase: *reenable_phase,
            revision: *revision,
            supporters: supporters
                .iter()
                .map(|cause| remap_cause(*cause, scope))
                .collect(),
        },
        ProvenanceRecord::PendingInertialDelay {
            event,
            owner,
            origin,
            deadline,
            target,
            revision,
            supporters,
        } => ProvenanceRecord::PendingInertialDelay {
            event: *event,
            owner: owner.clone(),
            origin: *origin,
            deadline: *deadline,
            target: *target,
            revision: *revision,
            supporters: supporters
                .iter()
                .map(|cause| remap_cause(*cause, scope))
                .collect(),
        },
        ProvenanceRecord::PendingTransportDelay {
            event,
            owner,
            origin,
            deadline,
            target,
            revision,
            supporters,
        } => ProvenanceRecord::PendingTransportDelay {
            event: *event,
            owner: owner.clone(),
            origin: *origin,
            deadline: *deadline,
            target: *target,
            revision: *revision,
            supporters: supporters
                .iter()
                .map(|cause| remap_cause(*cause, scope))
                .collect(),
        },
        ProvenanceRecord::Derived {
            subject,
            supporters,
        } => ProvenanceRecord::Derived {
            subject: subject.clone(),
            supporters: supporters
                .iter()
                .map(|cause| remap_cause(*cause, scope))
                .collect(),
        },
        ProvenanceRecord::PulseDerived {
            subject,
            contributions,
            result,
            supporters,
        } => ProvenanceRecord::PulseDerived {
            subject: subject.clone(),
            contributions: contributions
                .iter()
                .map(|contribution| PulseContribution {
                    port: contribution.port.clone(),
                    count: contribution.count,
                    cause: remap_cause(contribution.cause, scope),
                })
                .collect(),
            result: *result,
            supporters: supporters
                .iter()
                .map(|cause| remap_cause(*cause, scope))
                .collect(),
        },
        ProvenanceRecord::PulseControlledLevel {
            subject,
            contributions,
            result,
            supporters,
        } => ProvenanceRecord::PulseControlledLevel {
            subject: subject.clone(),
            contributions: contributions
                .iter()
                .map(|contribution| PulseContribution {
                    port: contribution.port.clone(),
                    count: contribution.count,
                    cause: remap_cause(contribution.cause, scope),
                })
                .collect(),
            result: *result,
            supporters: supporters
                .iter()
                .map(|cause| remap_cause(*cause, scope))
                .collect(),
        },
    }
}

fn push_record<D>(
    scope: ProvenanceScope,
    records: &mut Vec<ProvenanceRecord<D>>,
    record: ProvenanceRecord<D>,
) -> CauseRef {
    let ordinal = match u32::try_from(records.len()) {
        Ok(value) => value,
        Err(_) => panic!("transaction provenance exceeds the supported reference space"),
    };
    records.push(record);
    CauseRef { scope, ordinal }
}

fn operation_cause(causes: &[CauseRef], index: usize) -> CauseRef {
    let Some(cause) = causes.get(index).copied() else {
        panic!("evaluation provenance predecessor must precede its dependent operation");
    };
    cause
}

fn initialization_events<D>(
    at: Time<D>,
    revision: NetworkRevision,
    values: &BTreeMap<ExternalOutputKey<Level>, LogicLevel>,
    causes: &BTreeMap<ExternalOutputKey<Level>, CauseRef>,
    pulse_values: &BTreeMap<ExternalOutputKey<Pulse>, PulseCount>,
    pulse_causes: &BTreeMap<ExternalOutputKey<Pulse>, CauseRef>,
) -> Vec<OutputEvent<D>> {
    let mut events = values
        .iter()
        .map(|(output, value)| {
            let Some(cause) = causes.get(output).copied() else {
                panic!("every evaluated external output must retain one committed cause");
            };
            OutputEvent::LevelEstablished {
                output: *output,
                value: *value,
                at,
                cause,
                revision,
            }
        })
        .collect::<Vec<_>>();
    events.extend(pulse_events(at, revision, pulse_values, pulse_causes));
    sort_output_events(&mut events);
    events
}

fn changed_events<D>(
    at: Time<D>,
    revision: NetworkRevision,
    previous: &BTreeMap<ExternalOutputKey<Level>, LogicLevel>,
    settled: &BTreeMap<ExternalOutputKey<Level>, LogicLevel>,
    causes: &BTreeMap<ExternalOutputKey<Level>, CauseRef>,
    pulse_values: &BTreeMap<ExternalOutputKey<Pulse>, PulseCount>,
    pulse_causes: &BTreeMap<ExternalOutputKey<Pulse>, CauseRef>,
) -> Vec<OutputEvent<D>> {
    let mut events = settled
        .iter()
        .filter_map(|(output, to)| {
            let Some(from) = previous.get(output).copied() else {
                panic!("ready transaction must preserve every external output baseline");
            };
            if from == *to {
                return None;
            }
            let Some(cause) = causes.get(output).copied() else {
                panic!("every changed external output must retain one committed cause");
            };
            Some(OutputEvent::LevelChanged {
                output: *output,
                from,
                to: *to,
                at,
                cause,
                revision,
            })
        })
        .collect::<Vec<_>>();
    events.extend(pulse_events(at, revision, pulse_values, pulse_causes));
    sort_output_events(&mut events);
    events
}

fn pulse_events<D>(
    at: Time<D>,
    revision: NetworkRevision,
    values: &BTreeMap<ExternalOutputKey<Pulse>, PulseCount>,
    causes: &BTreeMap<ExternalOutputKey<Pulse>, CauseRef>,
) -> Vec<OutputEvent<D>> {
    values
        .iter()
        .filter(|(_, count)| count.is_positive())
        .map(|(output, count)| {
            let Some(cause) = causes.get(output).copied() else {
                panic!("every nonzero pulse output must retain one committed cause");
            };
            OutputEvent::Pulsed {
                output: *output,
                count: *count,
                at,
                cause,
                revision,
            }
        })
        .collect()
}

fn sort_output_events<D>(events: &mut [OutputEvent<D>]) {
    events.sort_by_key(|event| match event {
        OutputEvent::LevelEstablished { output, .. } | OutputEvent::LevelChanged { output, .. } => {
            (0_u8, output.as_u128())
        }
        OutputEvent::Pulsed { output, .. } => (1_u8, output.as_u128()),
    });
}

#[cfg(test)]
mod tests {
    use super::{UNFINALIZED_PROVENANCE_SCOPE, aggregate_due};
    use crate::authored::{
        ConnectionDef, ExternalInputDef, ExternalOutputDef, InputPortRole, NodeDef, NodeKind,
        NodePorts, UncheckedNetwork,
    };
    use crate::diagnostics::{DiagnosticCode, Responsibility, Severity};
    use crate::key::{
        ConnectionKey, ExternalInputKey, ExternalOutputKey, InPortKey, NetworkKey, NodeKey,
        OutPortKey, SignalSourceKey,
    };
    use crate::machine::{PendingEvent, PendingEventKey, PendingTransportDelay};
    use crate::metadata::DiagnosticMeta;
    use crate::signal::{Level, LogicLevel, Pulse, PulseCount};
    use crate::{
        CauseInspection, CauseLookupFailure, CauseRef, ConflictPolicy, MachineStatus,
        NetworkRevision, NodeSubject, OutputEvent, ProvenanceSubject, ProvenanceView,
        RuntimeFailureEvidence, RuntimePolicy, RuntimePolicyLimit, TimeDomainId, Transaction,
    };
    use std::collections::BTreeSet;

    fn compiled(network_key: u128, output_key: u128) -> crate::CompiledNetwork<()> {
        compiled_with_input(network_key, 1, output_key)
    }

    fn compiled_with_input(
        network_key: u128,
        input_key: u128,
        output_key: u128,
    ) -> crate::CompiledNetwork<()> {
        let input = ExternalInputKey::<Level>::from_u128(input_key);
        UncheckedNetwork::new(
            NetworkKey::from_u128(network_key),
            TimeDomainId::from_u128(2),
            DiagnosticMeta::default(),
            Vec::new(),
            vec![ExternalInputDef::new(
                input.into(),
                DiagnosticMeta::default(),
            )],
            vec![ExternalOutputDef::new(
                ExternalOutputKey::<Level>::from_u128(output_key).into(),
                SignalSourceKey::ExternalInput(input).into(),
                DiagnosticMeta::default(),
            )],
            Vec::new(),
        )
        .validate()
        .require_artifact()
        .unwrap_or_else(|_| panic!("fixture must validate"))
        .compile()
        .require_artifact()
        .unwrap_or_else(|_| panic!("fixture must compile"))
    }

    fn compiled_all() -> crate::CompiledNetwork<()> {
        compiled_variadic(NodeKind::all())
    }

    fn compiled_variadic(kind: NodeKind<()>) -> crate::CompiledNetwork<()> {
        let first = ExternalInputKey::<Level>::from_u128(1);
        let second = ExternalInputKey::<Level>::from_u128(2);
        let first_port = InPortKey::<Level>::from_u128(11);
        let second_port = InPortKey::<Level>::from_u128(12);
        let node_output = OutPortKey::<Level>::from_u128(13);
        UncheckedNetwork::new(
            NetworkKey::from_u128(20),
            TimeDomainId::from_u128(2),
            DiagnosticMeta::default(),
            vec![NodeDef::new(
                NodeKey::from_u128(10),
                kind,
                NodePorts::new(
                    vec![first_port.into(), second_port.into()],
                    vec![node_output.into()],
                ),
                DiagnosticMeta::default(),
            )],
            vec![
                ExternalInputDef::new(first.into(), DiagnosticMeta::default()),
                ExternalInputDef::new(second.into(), DiagnosticMeta::default()),
            ],
            vec![ExternalOutputDef::new(
                ExternalOutputKey::<Level>::from_u128(30).into(),
                SignalSourceKey::NodeOutput(node_output).into(),
                DiagnosticMeta::default(),
            )],
            vec![
                ConnectionDef::new(
                    ConnectionKey::from_u128(40),
                    first.into(),
                    first_port.into(),
                    DiagnosticMeta::default(),
                ),
                ConnectionDef::new(
                    ConnectionKey::from_u128(41),
                    second.into(),
                    second_port.into(),
                    DiagnosticMeta::default(),
                ),
            ],
        )
        .validate()
        .require_artifact()
        .unwrap_or_else(|_| panic!("variadic fixture must validate"))
        .compile()
        .require_artifact()
        .unwrap_or_else(|_| panic!("variadic fixture must compile"))
    }

    fn compiled_merge() -> crate::CompiledNetwork<()> {
        let first = ExternalInputKey::<Pulse>::from_u128(1);
        let second = ExternalInputKey::<Pulse>::from_u128(2);
        let first_port = InPortKey::<Pulse>::from_u128(11);
        let second_port = InPortKey::<Pulse>::from_u128(12);
        let node_output = OutPortKey::<Pulse>::from_u128(13);
        UncheckedNetwork::new(
            NetworkKey::from_u128(20),
            TimeDomainId::from_u128(2),
            DiagnosticMeta::default(),
            vec![NodeDef::new(
                NodeKey::from_u128(10),
                NodeKind::merge(),
                NodePorts::new(
                    vec![first_port.into(), second_port.into()],
                    vec![node_output.into()],
                ),
                DiagnosticMeta::default(),
            )],
            vec![
                ExternalInputDef::new(first.into(), DiagnosticMeta::default()),
                ExternalInputDef::new(second.into(), DiagnosticMeta::default()),
            ],
            vec![ExternalOutputDef::new(
                ExternalOutputKey::<Pulse>::from_u128(30).into(),
                SignalSourceKey::NodeOutput(node_output).into(),
                DiagnosticMeta::default(),
            )],
            vec![
                ConnectionDef::new(
                    ConnectionKey::from_u128(40),
                    first.into(),
                    first_port.into(),
                    DiagnosticMeta::default(),
                ),
                ConnectionDef::new(
                    ConnectionKey::from_u128(41),
                    second.into(),
                    second_port.into(),
                    DiagnosticMeta::default(),
                ),
            ],
        )
        .validate()
        .require_artifact()
        .unwrap_or_else(|_| panic!("Merge fixture must validate"))
        .compile()
        .require_artifact()
        .unwrap_or_else(|_| panic!("Merge fixture must compile"))
    }

    fn compiled_select() -> crate::CompiledNetwork<()> {
        let inputs = [
            ExternalInputKey::<Level>::from_u128(1),
            ExternalInputKey::<Level>::from_u128(2),
            ExternalInputKey::<Level>::from_u128(3),
        ];
        let ports = [
            InPortKey::<Level>::from_u128(11),
            InPortKey::<Level>::from_u128(12),
            InPortKey::<Level>::from_u128(13),
        ];
        let node_output = OutPortKey::<Level>::from_u128(14);
        UncheckedNetwork::new(
            NetworkKey::from_u128(20),
            TimeDomainId::from_u128(2),
            DiagnosticMeta::default(),
            vec![NodeDef::new(
                NodeKey::from_u128(10),
                NodeKind::select(),
                NodePorts::with_input_roles(
                    ports.into_iter().map(Into::into).collect(),
                    vec![
                        InputPortRole::Selector,
                        InputPortRole::WhenLow,
                        InputPortRole::WhenHigh,
                    ],
                    vec![node_output.into()],
                ),
                DiagnosticMeta::default(),
            )],
            inputs
                .into_iter()
                .map(|key| ExternalInputDef::new(key.into(), DiagnosticMeta::default()))
                .collect(),
            vec![ExternalOutputDef::new(
                ExternalOutputKey::<Level>::from_u128(30).into(),
                SignalSourceKey::NodeOutput(node_output).into(),
                DiagnosticMeta::default(),
            )],
            ports
                .into_iter()
                .zip(inputs)
                .enumerate()
                .map(|(index, (port, input))| {
                    ConnectionDef::new(
                        ConnectionKey::from_u128(40 + index as u128),
                        input.into(),
                        port.into(),
                        DiagnosticMeta::default(),
                    )
                })
                .collect(),
        )
        .validate()
        .require_artifact()
        .unwrap_or_else(|_| panic!("Select fixture must validate"))
        .compile()
        .require_artifact()
        .unwrap_or_else(|_| panic!("Select fixture must compile"))
    }

    fn compiled_toggle(initial: LogicLevel, downstream_not: bool) -> crate::CompiledNetwork<()> {
        let pulse = ExternalInputKey::<Pulse>::from_u128(1);
        let toggle_input = InPortKey::<Pulse>::from_u128(11);
        let toggle_output = OutPortKey::<Level>::from_u128(12);
        let mut nodes = vec![NodeDef::new(
            NodeKey::from_u128(10),
            NodeKind::toggle(initial),
            NodePorts::with_input_roles(
                vec![toggle_input.into()],
                vec![InputPortRole::Toggle],
                vec![toggle_output.into()],
            ),
            DiagnosticMeta::default(),
        )];
        let mut connections = vec![ConnectionDef::new(
            ConnectionKey::from_u128(40),
            pulse.into(),
            toggle_input.into(),
            DiagnosticMeta::default(),
        )];
        let source = if downstream_not {
            let input = InPortKey::<Level>::from_u128(21);
            let output = OutPortKey::<Level>::from_u128(22);
            nodes.push(NodeDef::new(
                NodeKey::from_u128(20),
                NodeKind::not(),
                NodePorts::new(vec![input.into()], vec![output.into()]),
                DiagnosticMeta::default(),
            ));
            connections.push(ConnectionDef::new(
                ConnectionKey::from_u128(41),
                toggle_output.into(),
                input.into(),
                DiagnosticMeta::default(),
            ));
            output
        } else {
            toggle_output
        };
        UncheckedNetwork::new(
            NetworkKey::from_u128(20),
            TimeDomainId::from_u128(2),
            DiagnosticMeta::default(),
            nodes,
            vec![ExternalInputDef::new(
                pulse.into(),
                DiagnosticMeta::default(),
            )],
            vec![ExternalOutputDef::new(
                ExternalOutputKey::<Level>::from_u128(30).into(),
                SignalSourceKey::NodeOutput(source).into(),
                DiagnosticMeta::default(),
            )],
            connections,
        )
        .validate()
        .require_artifact()
        .unwrap_or_else(|_| panic!("Toggle fixture must validate"))
        .compile()
        .require_artifact()
        .unwrap_or_else(|_| panic!("Toggle fixture must compile"))
    }

    fn compiled_two_toggles() -> crate::CompiledNetwork<()> {
        let inputs = [
            ExternalInputKey::<Pulse>::from_u128(1),
            ExternalInputKey::<Pulse>::from_u128(2),
        ];
        let ports = [
            InPortKey::<Pulse>::from_u128(11),
            InPortKey::<Pulse>::from_u128(21),
        ];
        let outputs = [
            OutPortKey::<Level>::from_u128(12),
            OutPortKey::<Level>::from_u128(22),
        ];
        UncheckedNetwork::new(
            NetworkKey::from_u128(20),
            TimeDomainId::from_u128(2),
            DiagnosticMeta::default(),
            [LogicLevel::Low, LogicLevel::High]
                .into_iter()
                .enumerate()
                .map(|(index, initial)| {
                    NodeDef::new(
                        NodeKey::from_u128(10 + index as u128 * 10),
                        NodeKind::toggle(initial),
                        NodePorts::with_input_roles(
                            vec![ports[index].into()],
                            vec![InputPortRole::Toggle],
                            vec![outputs[index].into()],
                        ),
                        DiagnosticMeta::default(),
                    )
                })
                .collect(),
            inputs
                .into_iter()
                .map(|key| ExternalInputDef::new(key.into(), DiagnosticMeta::default()))
                .collect(),
            outputs
                .into_iter()
                .enumerate()
                .map(|(index, source)| {
                    ExternalOutputDef::new(
                        ExternalOutputKey::<Level>::from_u128(30 + index as u128).into(),
                        SignalSourceKey::NodeOutput(source).into(),
                        DiagnosticMeta::default(),
                    )
                })
                .collect(),
            ports
                .into_iter()
                .zip(inputs)
                .enumerate()
                .map(|(index, (target, source))| {
                    ConnectionDef::new(
                        ConnectionKey::from_u128(40 + index as u128),
                        source.into(),
                        target.into(),
                        DiagnosticMeta::default(),
                    )
                })
                .collect(),
        )
        .validate()
        .require_artifact()
        .unwrap_or_else(|_| panic!("two-Toggle fixture must validate"))
        .compile()
        .require_artifact()
        .unwrap_or_else(|_| panic!("two-Toggle fixture must compile"))
    }

    fn policy() -> RuntimePolicy {
        policy_with([1, 2, 0, 1, 3])
    }

    fn policy_with(values: [u64; 5]) -> RuntimePolicy {
        RuntimePolicy::builder()
            .max_internal_reactions(values[0])
            .max_evaluated_operations(values[1])
            .max_pending_events(values[2])
            .max_events_created_per_transaction(values[3])
            .max_required_provenance_growth(values[4])
            .build()
            .unwrap_or_else(|failure| panic!("policy must build: {failure}"))
    }

    fn initialized_machine(
        compiled: &crate::CompiledNetwork<()>,
        value: LogicLevel,
    ) -> crate::Machine<()> {
        let mut machine = compiled.spawn(policy_with([10, 100, 0, 100, 1_000]));
        machine
            .apply(Transaction::initialize(
                crate::time::Time::from_ticks(10),
                machine.revision(),
                compiled
                    .input_snapshot()
                    .set(ExternalInputKey::from_u128(1), value)
                    .and_then(crate::InputSnapshotBuilder::finish)
                    .unwrap_or_else(|_| panic!("snapshot must build")),
            ))
            .unwrap_or_else(|failure| panic!("initialization must succeed: {failure}"));
        machine
    }

    #[test]
    fn every_runtime_leaf_projects_one_registry_backed_problem() {
        let first = compiled_with_input(1, 2, 3);
        let second = compiled_with_input(4, 5, 6);
        let cases = vec![
            (
                RuntimeFailureEvidence::AlreadyInitialized,
                DiagnosticCode::LifecycleAlreadyInitialized,
            ),
            (
                RuntimeFailureEvidence::DeltaBeforeInitialization,
                DiagnosticCode::LifecycleDeltaBeforeInitialization,
            ),
            (
                RuntimeFailureEvidence::TimeNotStrictlyIncreasing {
                    current_ticks: 7,
                    requested_ticks: 7,
                },
                DiagnosticCode::RuntimeTimeNotStrictlyIncreasing,
            ),
            (
                RuntimeFailureEvidence::StaleRevision {
                    expected: NetworkRevision::from_value(1),
                    actual: NetworkRevision::from_value(2),
                },
                DiagnosticCode::RuntimeStaleRevision,
            ),
            (
                RuntimeFailureEvidence::WrongNetwork {
                    expected_key: first.network_key(),
                    actual_key: second.network_key(),
                    expected_fingerprint: first.fingerprint(),
                    actual_fingerprint: second.fingerprint(),
                },
                DiagnosticCode::InputWrongNetwork,
            ),
            (
                RuntimeFailureEvidence::ForeignInputSchema {
                    expected: first.input_schema_fingerprint(),
                    actual: second.input_schema_fingerprint(),
                },
                DiagnosticCode::InputForeignSchema,
            ),
            (
                RuntimeFailureEvidence::StaleInputSchema {
                    expected: first.input_schema_fingerprint(),
                    actual: second.input_schema_fingerprint(),
                },
                DiagnosticCode::InputStaleSchema,
            ),
            (
                RuntimeFailureEvidence::BudgetExceeded {
                    budget: RuntimePolicyLimit::MaxPendingEvents,
                    limit: 2,
                    consumed: 3,
                },
                DiagnosticCode::RuntimeBudgetExceeded,
            ),
            (
                RuntimeFailureEvidence::PulseCountOverflow {
                    node: NodeSubject::Node(NodeKey::from_u128(8)),
                    left: PulseCount::new(u64::MAX),
                    right: PulseCount::ONE,
                },
                DiagnosticCode::RuntimePulseCountOverflow,
            ),
            (
                RuntimeFailureEvidence::TimeOverflow {
                    node: NodeSubject::Node(NodeKey::from_u128(9)),
                    origin_ticks: u64::MAX,
                    delay_ticks: 1,
                },
                DiagnosticCode::RuntimeTimeOverflow,
            ),
            (
                RuntimeFailureEvidence::PulseLatchConflict {
                    node: NodeSubject::Node(NodeKey::from_u128(10)),
                    policy: ConflictPolicy::RejectTransaction,
                    previous: LogicLevel::Low,
                    set_count: PulseCount::ONE,
                    reset_count: PulseCount::new(2),
                    at_ticks: 12,
                    revision: NetworkRevision::from_value(0),
                },
                DiagnosticCode::RuntimePulseLatchConflictRejected,
            ),
        ];
        for (evidence, expected) in cases {
            let problem = evidence.problem::<()>();
            assert_eq!(evidence.code(), expected);
            assert_eq!(problem.code(), expected);
            assert_eq!(problem.evidence().code(), expected);
            assert_eq!(evidence.severity(), expected.severity());
            assert_eq!(evidence.responsibility(), expected.responsibility());
            assert!(
                expected.allows_delivery(crate::diagnostics::ProblemDelivery::OperationFailure)
            );
        }
    }

    #[test]
    fn provenance_views_reject_causes_from_different_converged_histories() {
        let compiled = compiled(100, 200);
        let mut first = initialized_machine(&compiled, LogicLevel::Low);
        let mut second = initialized_machine(&compiled, LogicLevel::Low);

        for (machine, at) in [(&mut first, 11_u64), (&mut second, 12_u64)] {
            let delta = compiled
                .input_delta()
                .set(ExternalInputKey::from_u128(1), LogicLevel::High)
                .and_then(crate::InputDeltaBuilder::finish)
                .unwrap_or_else(|_| panic!("history delta must build"));
            machine
                .apply(Transaction::advance(
                    crate::time::Time::from_ticks(at),
                    machine.revision(),
                    delta,
                ))
                .unwrap_or_else(|failure| panic!("history advance must succeed: {failure}"));
        }
        assert_ne!(first.status(), second.status());

        let finish = |machine: &mut crate::Machine<()>| {
            let delta = compiled
                .input_delta()
                .set(ExternalInputKey::from_u128(1), LogicLevel::Low)
                .and_then(crate::InputDeltaBuilder::finish)
                .unwrap_or_else(|_| panic!("converging delta must build"));
            machine
                .apply(Transaction::advance(
                    crate::time::Time::from_ticks(20),
                    machine.revision(),
                    delta,
                ))
                .unwrap_or_else(|failure| panic!("converging advance must succeed: {failure}"))
        };
        let first_result = finish(&mut first);
        let second_result = finish(&mut second);
        assert_eq!(first.status(), second.status());
        assert_eq!(first.revision(), second.revision());
        assert_eq!(
            first_result.requested_time(),
            second_result.requested_time()
        );
        assert_eq!(first.store.external_levels, second.store.external_levels);
        let [
            OutputEvent::LevelChanged {
                cause: first_cause, ..
            },
        ] = first_result.output_events()
        else {
            panic!("first converged history must publish one changed output");
        };
        let [
            OutputEvent::LevelChanged {
                cause: second_cause,
                ..
            },
        ] = second_result.output_events()
        else {
            panic!("second converged history must publish one changed output");
        };
        let first_cause = *first_cause;
        let second_cause = *second_cause;

        assert_eq!(first_cause.ordinal, second_cause.ordinal);
        assert_eq!(
            first_result.provenance().len(),
            second_result.provenance().len()
        );
        assert!(first_result.provenance().inspect(first_cause).is_ok());
        assert!(second_result.provenance().inspect(second_cause).is_ok());
        let foreign = first_result
            .provenance()
            .inspect(second_cause)
            .err()
            .unwrap_or_else(|| panic!("foreign cause must fail"));
        assert!(matches!(foreign, CauseLookupFailure::ForeignCause { .. }));
        assert_eq!(foreign.code(), DiagnosticCode::ExplanationForeignCause);
        assert_eq!(foreign.responsibility(), Responsibility::Compatibility);
        assert_eq!(foreign.problem::<()>().evidence().code(), foreign.code());
        assert!(matches!(
            second_result.provenance().inspect(first_cause),
            Err(CauseLookupFailure::ForeignCause { .. })
        ));

        let first_supporter = match first_result.provenance().inspect(first_cause) {
            Ok(CauseInspection::Derived { supporters, .. }) => match supporters.first() {
                Some(cause) => *cause,
                None => panic!("changed output provenance must retain a supporter"),
            },
            _ => panic!("changed output cause must resolve to a derived record"),
        };
        assert!(first_result.provenance().inspect(first_supporter).is_ok());
        assert!(matches!(
            second_result.provenance().inspect(first_supporter),
            Err(CauseLookupFailure::ForeignCause { .. })
        ));

        let out_of_range = CauseRef {
            scope: first_cause.scope,
            ordinal: u32::MAX,
        };
        let invalid = first_result
            .provenance()
            .inspect(out_of_range)
            .err()
            .unwrap_or_else(|| panic!("out-of-range cause must fail"));
        assert!(matches!(invalid, CauseLookupFailure::InvalidCause { .. }));
        assert_eq!(invalid.code(), DiagnosticCode::ExplanationInvalidCause);
        assert_eq!(invalid.responsibility(), Responsibility::CallerInput);
        assert_eq!(invalid.problem::<()>().evidence().code(), invalid.code());

        let shared_view = first_result.provenance().clone();
        assert!(shared_view.inspect(first_cause).is_ok());
    }

    #[derive(Debug, PartialEq, Eq)]
    struct EpisodeObservation {
        identity: crate::DiagnosticEpisodeId,
        condition: crate::DiagnosticConditionKey,
        problem: crate::diagnostics::Problem<()>,
        began: crate::time::Time<()>,
        changed: crate::time::Time<()>,
        cause: CauseRef,
        provenance: ([u8; 32], usize, usize),
    }

    #[derive(Debug, PartialEq, Eq)]
    struct MachineObservation {
        network_key: NetworkKey,
        network_fingerprint: crate::NetworkFingerprint,
        input_schema_fingerprint: crate::InputSchemaFingerprint,
        policy_id: crate::RuntimePolicyId,
        status: MachineStatus<()>,
        revision: NetworkRevision,
        external_levels: std::collections::BTreeMap<ExternalInputKey<Level>, LogicLevel>,
        settled_levels: Vec<LogicLevel>,
        output_baselines: std::collections::BTreeMap<ExternalOutputKey<Level>, LogicLevel>,
        input_causes: std::collections::BTreeMap<ExternalInputKey<Level>, CauseRef>,
        output_causes: std::collections::BTreeMap<ExternalOutputKey<Level>, CauseRef>,
        provenance: Option<([u8; 32], usize, usize)>,
        operation_levels: Vec<Option<LogicLevel>>,
        operation_causes: Vec<CauseRef>,
        edge_observations: Vec<crate::EdgeObservation>,
        edge_observation_causes: std::collections::BTreeMap<NodeKey, CauseRef>,
        episodes: Vec<EpisodeObservation>,
        stored_levels: Vec<LogicLevel>,
        toggle_inversion_causes: std::collections::BTreeMap<NodeKey, CauseRef>,
        establishment_causes: std::collections::BTreeMap<NodeKey, CauseRef>,
        pending_events: std::collections::BTreeMap<
            crate::time::Time<()>,
            Vec<crate::machine::PendingEvent<()>>,
        >,
        next_pending_event_serial: u64,
        transport_transition_causes: std::collections::BTreeMap<NodeKey, CauseRef>,
        inertial_cancellation_causes: std::collections::BTreeMap<NodeKey, CauseRef>,
        periodic_anchors: std::collections::BTreeMap<NodeKey, crate::time::Time<()>>,
        periodic_anchor_causes: std::collections::BTreeMap<NodeKey, CauseRef>,
        periodic_cancellation_causes: std::collections::BTreeMap<NodeKey, CauseRef>,
    }

    fn observe(machine: &crate::Machine<()>) -> MachineObservation {
        MachineObservation {
            network_key: machine.compiled.network_key(),
            network_fingerprint: machine.compiled.fingerprint(),
            input_schema_fingerprint: machine.compiled.input_schema_fingerprint(),
            policy_id: machine.policy.id(),
            status: machine.store.status,
            revision: machine.store.revision,
            external_levels: machine.store.external_levels.clone(),
            settled_levels: machine.store.settled_levels.clone(),
            output_baselines: machine.store.output_baselines.clone(),
            input_causes: machine.store.input_causes.clone(),
            output_causes: machine.store.output_causes.clone(),
            provenance: machine.store.provenance.as_ref().map(|view| {
                (
                    view.scope,
                    view.len(),
                    std::sync::Arc::as_ptr(&view.records) as usize,
                )
            }),
            operation_levels: machine.store.operation_levels.clone(),
            operation_causes: machine.store.operation_causes.clone(),
            edge_observations: machine.store.edge_observations.clone(),
            edge_observation_causes: machine.store.edge_observation_causes.clone(),
            episodes: machine
                .store
                .active_episodes
                .values()
                .map(|e| EpisodeObservation {
                    identity: e.identity(),
                    condition: e.condition().clone(),
                    problem: e.current().clone(),
                    began: e.began_at(),
                    changed: e.last_material_change(),
                    cause: e.cause(),
                    provenance: (
                        e.provenance().scope,
                        e.provenance().len(),
                        std::sync::Arc::as_ptr(&e.provenance().records) as usize,
                    ),
                })
                .collect(),
            stored_levels: machine.store.stored_levels.clone(),
            toggle_inversion_causes: machine.store.toggle_inversion_causes.clone(),
            establishment_causes: machine.store.establishment_causes.clone(),
            pending_events: machine.store.pending_events.clone(),
            next_pending_event_serial: machine.store.next_pending_event_serial,
            transport_transition_causes: machine.store.transport_transition_causes.clone(),
            inertial_cancellation_causes: machine.store.inertial_cancellation_causes.clone(),
            periodic_anchors: machine.store.periodic_anchors.clone(),
            periodic_anchor_causes: machine.store.periodic_anchor_causes.clone(),
            periodic_cancellation_causes: machine.store.periodic_cancellation_causes.clone(),
        }
    }

    #[test]
    fn periodic_late_budget_failures_preserve_complete_machine() {
        use crate::time::{NonZeroSpan, Time};
        use crate::{FirstEmissionPolicy, NetworkBuilder, PeriodicConfig, ReenablePhasePolicy};
        let mut builder = NetworkBuilder::<()>::new(TimeDomainId::from_u128(2));
        let input = ExternalInputKey::from_u128(10);
        let second_input = ExternalInputKey::from_u128(11);
        let enable = builder
            .add_level_input(input, DiagnosticMeta::default())
            .unwrap();
        let second_enable = builder
            .add_level_input(second_input, DiagnosticMeta::default())
            .unwrap();
        for (node, enable) in [(20, enable), (21, second_enable)] {
            let signal = builder
                .add_periodic(
                    NodeKey::from_u128(node),
                    enable,
                    PeriodicConfig::new(
                        NonZeroSpan::from_ticks(5).unwrap(),
                        FirstEmissionPolicy::AfterFirstPeriod,
                        ReenablePhasePolicy::PreservePhase,
                    ),
                    DiagnosticMeta::default(),
                )
                .unwrap()
                .into_outputs();
            builder
                .add_pulse_output(
                    ExternalOutputKey::from_u128(node),
                    signal,
                    DiagnosticMeta::default(),
                )
                .unwrap();
        }
        let compiled = builder
            .finish()
            .require_artifact()
            .unwrap()
            .compile()
            .require_artifact()
            .unwrap();
        let initialize = |policy| {
            let mut machine = compiled.spawn(policy);
            let snapshot = compiled
                .input_snapshot()
                .set(input, LogicLevel::High)
                .unwrap()
                .set(second_input, LogicLevel::Low)
                .unwrap()
                .finish()
                .unwrap();
            machine
                .apply(Transaction::initialize(
                    Time::from_ticks(0),
                    machine.revision(),
                    snapshot,
                ))
                .unwrap();
            machine
        };
        let transaction = |revision| {
            Transaction::advance(
                Time::from_ticks(16),
                revision,
                compiled
                    .input_delta()
                    .set(second_input, LogicLevel::High)
                    .unwrap()
                    .finish()
                    .unwrap(),
            )
        };
        let mut probe = initialize(policy_with([100, 10_000, 100, 1_000, 10_000]));
        let initial_records = probe.store.provenance.as_ref().unwrap().len();
        let success = probe.apply(transaction(probe.revision())).unwrap();
        let growth = (success.provenance().len() - initial_records) as u64;
        for (index, limit, budget) in [
            (0, 3, RuntimePolicyLimit::MaxInternalReactions),
            (
                1,
                compiled.operation_count() as u64 * 3,
                RuntimePolicyLimit::MaxEvaluatedOperations,
            ),
            (2, 1, RuntimePolicyLimit::MaxPendingEvents),
            (3, 5, RuntimePolicyLimit::MaxEventsCreatedPerTransaction),
            (
                4,
                growth - 1,
                RuntimePolicyLimit::MaxRequiredProvenanceGrowth,
            ),
        ] {
            let mut limits = [100, 10_000, 100, 1_000, 10_000];
            limits[index] = limit;
            let mut machine = initialize(policy_with(limits));
            let before = observe(&machine);
            let failure = machine.apply(transaction(machine.revision())).unwrap_err();
            assert!(
                matches!(failure.evidence(), RuntimeFailureEvidence::BudgetExceeded { budget: actual, .. } if *actual == budget)
            );
            assert_eq!(observe(&machine), before, "budget {budget:?}");
            let inspected = machine.inspect_periodic(NodeKey::from_u128(20)).unwrap();
            recursively_assert_acyclic(inspected.provenance(), inspected.current_support());
            recursively_assert_acyclic(inspected.provenance(), inspected.anchor_cause().unwrap());
            recursively_assert_acyclic(
                inspected.provenance(),
                inspected.pending().unwrap().cause(),
            );
        }
        for event in success.output_events() {
            if let OutputEvent::Pulsed { cause, .. } = event {
                recursively_assert_acyclic(success.provenance(), *cause);
            }
        }
    }

    #[test]
    fn toggle_exhausts_initial_state_and_pulse_parity() {
        for initial in [LogicLevel::Low, LogicLevel::High] {
            for count in [0_u64, 1, 2, 3, 10, 11] {
                let compiled = compiled_toggle(initial, false);
                let mut machine = compiled.spawn(policy_with([10, 100, 0, 100, 1_000]));
                assert_eq!(
                    machine
                        .inspect_toggle_definition(NodeKey::from_u128(10))
                        .unwrap()
                        .initial(),
                    initial
                );
                assert_eq!(
                    machine.inspect_toggle(NodeKey::from_u128(10)),
                    Err(crate::ToggleInspectionFailure::NotInitialized)
                );
                assert_eq!(
                    machine.inspect_toggle_definition(NodeKey::from_u128(999)),
                    Err(crate::ToggleInspectionFailure::UnknownNode(
                        NodeKey::from_u128(999)
                    ))
                );
                let mut snapshot = compiled.input_snapshot();
                if count != 0 {
                    snapshot = snapshot
                        .pulse(ExternalInputKey::from_u128(1), PulseCount::new(count))
                        .unwrap();
                }
                let result = machine
                    .apply(Transaction::initialize(
                        crate::time::Time::from_ticks(10),
                        machine.revision(),
                        snapshot.finish().unwrap(),
                    ))
                    .unwrap();
                let expected = if count % 2 == 1 {
                    initial.invert()
                } else {
                    initial
                };
                let inspected = machine.inspect_toggle(NodeKey::from_u128(10)).unwrap();
                assert_eq!(inspected.node(), NodeKey::from_u128(10));
                assert_eq!(inspected.committed(), expected);
                assert_eq!(inspected.initial(), initial);
                assert_eq!(inspected.revision(), machine.revision());
                assert_eq!(inspected.at(), crate::time::Time::from_ticks(10));
                assert_eq!(
                    machine.output_level(ExternalOutputKey::from_u128(30)),
                    Some(expected)
                );
                assert_eq!(inspected.latest_inversion().is_some(), count % 2 == 1);
                if let Some(cause) = inspected.latest_inversion() {
                    assert!(result.provenance().inspect(cause).is_ok());
                }
            }
        }
    }

    #[test]
    fn toggle_ready_reactions_commit_once_and_retain_latest_inversion_cause() {
        let compiled = compiled_toggle(LogicLevel::Low, false);
        let mut machine = compiled.spawn(policy_with([10, 100, 0, 100, 1_000]));
        let snapshot = compiled
            .input_snapshot()
            .pulse(ExternalInputKey::from_u128(1), PulseCount::ONE)
            .and_then(crate::InputSnapshotBuilder::finish)
            .unwrap();
        machine
            .apply(Transaction::initialize(
                crate::time::Time::from_ticks(5),
                machine.revision(),
                snapshot,
            ))
            .unwrap();
        assert_eq!(
            machine
                .inspect_toggle(NodeKey::from_u128(10))
                .unwrap()
                .committed(),
            LogicLevel::High
        );

        let even = compiled
            .input_delta()
            .pulse(ExternalInputKey::from_u128(1), PulseCount::new(2))
            .and_then(crate::InputDeltaBuilder::finish)
            .unwrap();
        let even_result = machine
            .apply(Transaction::advance(
                crate::time::Time::from_ticks(6),
                machine.revision(),
                even,
            ))
            .unwrap();
        assert!(even_result.output_events().is_empty());
        let retained = machine
            .inspect_toggle(NodeKey::from_u128(10))
            .unwrap()
            .latest_inversion()
            .unwrap();
        assert!(even_result.provenance().inspect(retained).is_ok());

        let odd = compiled
            .input_delta()
            .pulse(ExternalInputKey::from_u128(1), PulseCount::new(3))
            .and_then(crate::InputDeltaBuilder::finish)
            .unwrap();
        let odd_result = machine
            .apply(Transaction::advance(
                crate::time::Time::from_ticks(7),
                machine.revision(),
                odd,
            ))
            .unwrap();
        assert!(matches!(
            odd_result.output_events(),
            [OutputEvent::LevelChanged {
                from: LogicLevel::High,
                to: LogicLevel::Low,
                ..
            }]
        ));
        let inspected = machine.inspect_toggle(NodeKey::from_u128(10)).unwrap();
        assert_eq!(inspected.committed(), LogicLevel::Low);
        let latest = odd_result
            .provenance()
            .inspect(inspected.latest_inversion().unwrap())
            .unwrap();
        let CauseInspection::Derived { supporters, .. } = latest else {
            panic!("latest Toggle inversion must resolve to a derived cause");
        };
        assert!(supporters.iter().any(|supporter| matches!(
            odd_result.provenance().inspect(*supporter),
            Ok(CauseInspection::Derived {
                subject: ProvenanceSubject::Node(node),
                ..
            }) if node == NodeKey::from_u128(10)
        )));
    }

    #[test]
    fn toggle_settles_downstream_in_same_reaction_and_machines_are_independent() {
        let compiled = compiled_toggle(LogicLevel::Low, true);
        let mut first = compiled.spawn(policy_with([10, 100, 0, 100, 1_000]));
        let mut second = compiled.spawn(policy_with([10, 100, 0, 100, 1_000]));
        assert_eq!(
            first.inspect_toggle_definition(NodeKey::from_u128(20)),
            Err(crate::ToggleInspectionFailure::NotToggle(
                NodeKey::from_u128(20)
            ))
        );
        for (machine, count) in [(&mut first, 1_u64), (&mut second, 2_u64)] {
            let snapshot = compiled
                .input_snapshot()
                .pulse(ExternalInputKey::from_u128(1), PulseCount::new(count))
                .and_then(crate::InputSnapshotBuilder::finish)
                .unwrap();
            machine
                .apply(Transaction::initialize(
                    crate::time::Time::from_ticks(10),
                    machine.revision(),
                    snapshot,
                ))
                .unwrap();
        }
        assert_eq!(
            first
                .inspect_toggle(NodeKey::from_u128(10))
                .unwrap()
                .committed(),
            LogicLevel::High
        );
        assert_eq!(
            second
                .inspect_toggle(NodeKey::from_u128(10))
                .unwrap()
                .committed(),
            LogicLevel::Low
        );
        assert_eq!(
            first.output_level(ExternalOutputKey::from_u128(30)),
            Some(LogicLevel::Low)
        );
        assert_eq!(
            second.output_level(ExternalOutputKey::from_u128(30)),
            Some(LogicLevel::High)
        );
    }

    #[test]
    fn toggle_state_and_cause_are_failure_atomic() {
        let compiled = compiled_toggle(LogicLevel::Low, false);
        let mut rejected_initialization = compiled.spawn(policy_with([10, 100, 0, 0, 1_000]));
        let before_initialization = observe(&rejected_initialization);
        let proposed_odd = compiled
            .input_snapshot()
            .pulse(ExternalInputKey::from_u128(1), PulseCount::ONE)
            .and_then(crate::InputSnapshotBuilder::finish)
            .unwrap();
        let failure = rejected_initialization
            .apply(Transaction::initialize(
                crate::time::Time::from_ticks(1),
                rejected_initialization.revision(),
                proposed_odd,
            ))
            .unwrap_err();
        assert!(matches!(
            failure.evidence(),
            RuntimeFailureEvidence::BudgetExceeded {
                budget: RuntimePolicyLimit::MaxEventsCreatedPerTransaction,
                ..
            }
        ));
        assert_eq!(observe(&rejected_initialization), before_initialization);

        let mut machine = compiled.spawn(policy_with([10, 100, 0, 100, 1_000]));
        let snapshot = compiled
            .input_snapshot()
            .pulse(ExternalInputKey::from_u128(1), PulseCount::ONE)
            .and_then(crate::InputSnapshotBuilder::finish)
            .unwrap();
        machine
            .apply(Transaction::initialize(
                crate::time::Time::from_ticks(10),
                machine.revision(),
                snapshot,
            ))
            .unwrap();
        let before = observe(&machine);
        let odd = compiled
            .input_delta()
            .pulse(ExternalInputKey::from_u128(1), PulseCount::ONE)
            .and_then(crate::InputDeltaBuilder::finish)
            .unwrap();
        let failure = machine
            .apply(Transaction::advance(
                crate::time::Time::from_ticks(10),
                machine.revision(),
                odd,
            ))
            .unwrap_err();
        assert!(matches!(
            failure.evidence(),
            RuntimeFailureEvidence::TimeNotStrictlyIncreasing { .. }
        ));
        assert_eq!(observe(&machine), before);
    }

    #[test]
    fn multiple_toggle_cells_commit_atomically_and_ignore_input_claim_order() {
        let compiled = compiled_two_toggles();
        let initialize = |reverse: bool| {
            let mut machine = compiled.spawn(policy_with([10, 100, 0, 100, 1_000]));
            let mut snapshot = compiled.input_snapshot();
            let claims = if reverse {
                [(2_u128, 3_u64), (1, 1)]
            } else {
                [(1_u128, 1_u64), (2, 3)]
            };
            for (key, count) in claims {
                snapshot = snapshot
                    .pulse(ExternalInputKey::from_u128(key), PulseCount::new(count))
                    .unwrap();
            }
            machine
                .apply(Transaction::initialize(
                    crate::time::Time::from_ticks(10),
                    machine.revision(),
                    snapshot.finish().unwrap(),
                ))
                .unwrap();
            machine
        };

        let forward = initialize(false);
        let reverse = initialize(true);
        for machine in [&forward, &reverse] {
            assert_eq!(
                machine
                    .inspect_toggle(NodeKey::from_u128(10))
                    .unwrap()
                    .committed(),
                LogicLevel::High
            );
            assert_eq!(
                machine
                    .inspect_toggle(NodeKey::from_u128(20))
                    .unwrap()
                    .committed(),
                LogicLevel::Low
            );
            assert_eq!(
                machine.store.stored_levels,
                [LogicLevel::High, LogicLevel::Low]
            );
            assert_eq!(machine.store.toggle_inversion_causes.len(), 2);
        }
        assert_eq!(forward.store.stored_levels, reverse.store.stored_levels);
    }

    fn complex_compiled(reverse_claims: bool) -> crate::CompiledNetwork<()> {
        complex_compiled_with_name(reverse_claims, None)
    }

    fn complex_compiled_with_name(
        reverse_claims: bool,
        network_name: Option<&str>,
    ) -> crate::CompiledNetwork<()> {
        let first_input = ExternalInputKey::<Level>::from_u128(1);
        let second_input = ExternalInputKey::<Level>::from_u128(2);
        let first_not_input = InPortKey::<Level>::from_u128(11);
        let first_not_output = OutPortKey::<Level>::from_u128(12);
        let second_not_input = InPortKey::<Level>::from_u128(21);
        let second_not_output = OutPortKey::<Level>::from_u128(22);
        let mut nodes = vec![
            NodeDef::new(
                NodeKey::from_u128(10),
                NodeKind::not(),
                NodePorts::new(vec![first_not_input.into()], vec![first_not_output.into()]),
                DiagnosticMeta::default(),
            ),
            NodeDef::new(
                NodeKey::from_u128(20),
                NodeKind::not(),
                NodePorts::new(
                    vec![second_not_input.into()],
                    vec![second_not_output.into()],
                ),
                DiagnosticMeta::default(),
            ),
        ];
        let mut inputs = vec![
            ExternalInputDef::new(first_input.into(), DiagnosticMeta::default()),
            ExternalInputDef::new(second_input.into(), DiagnosticMeta::default()),
        ];
        let mut outputs = vec![
            ExternalOutputDef::new(
                ExternalOutputKey::<Level>::from_u128(50).into(),
                SignalSourceKey::NodeOutput(first_not_output).into(),
                DiagnosticMeta::default(),
            ),
            ExternalOutputDef::new(
                ExternalOutputKey::<Level>::from_u128(10).into(),
                SignalSourceKey::NodeOutput(second_not_output).into(),
                DiagnosticMeta::default(),
            ),
            ExternalOutputDef::new(
                ExternalOutputKey::<Level>::from_u128(30).into(),
                SignalSourceKey::ExternalInput(second_input).into(),
                DiagnosticMeta::default(),
            ),
        ];
        let mut connections = vec![
            ConnectionDef::new(
                ConnectionKey::from_u128(100),
                first_input.into(),
                first_not_input.into(),
                DiagnosticMeta::default(),
            ),
            ConnectionDef::new(
                ConnectionKey::from_u128(101),
                first_not_output.into(),
                second_not_input.into(),
                DiagnosticMeta::default(),
            ),
        ];
        if reverse_claims {
            nodes.reverse();
            inputs.reverse();
            outputs.reverse();
            connections.reverse();
        }
        UncheckedNetwork::new(
            NetworkKey::from_u128(500),
            TimeDomainId::from_u128(2),
            DiagnosticMeta {
                name: network_name.map(String::from),
                ..DiagnosticMeta::default()
            },
            nodes,
            inputs,
            outputs,
            connections,
        )
        .validate()
        .require_artifact()
        .unwrap_or_else(|_| panic!("fixture must validate"))
        .compile()
        .require_artifact()
        .unwrap_or_else(|_| panic!("fixture must compile"))
    }

    fn recursively_assert_acyclic(view: &ProvenanceView<()>, root: CauseRef) {
        fn visit(
            view: &ProvenanceView<()>,
            cause: CauseRef,
            visiting: &mut BTreeSet<CauseRef>,
            visited: &mut BTreeSet<CauseRef>,
        ) {
            if visited.contains(&cause) {
                return;
            }
            assert!(visiting.insert(cause), "provenance must remain acyclic");
            match view
                .inspect(cause)
                .unwrap_or_else(|failure| panic!("cause must resolve: {failure}"))
            {
                CauseInspection::Derived { supporters, .. }
                | CauseInspection::PendingPulseDelay { supporters, .. }
                | CauseInspection::PendingTransportDelay { supporters, .. }
                | CauseInspection::PendingInertialDelay { supporters, .. }
                | CauseInspection::PendingPeriodicBoundary { supporters, .. } => {
                    assert!(
                        !supporters.is_empty(),
                        "derived provenance must terminate in authoritative roots"
                    );
                    for supporter in supporters {
                        visit(view, *supporter, visiting, visited);
                    }
                }
                CauseInspection::PulseDerived { supporters, .. }
                | CauseInspection::PulseControlledLevel { supporters, .. } => {
                    assert!(!supporters.is_empty());
                    for supporter in supporters {
                        visit(view, *supporter, visiting, visited);
                    }
                }
                CauseInspection::InitializationTransaction { .. }
                | CauseInspection::ReadyTransaction { .. }
                | CauseInspection::ExternalObservation { .. }
                | CauseInspection::ExternalPulseObservation { .. } => {}
            }
            assert!(visiting.remove(&cause));
            visited.insert(cause);
        }

        visit(view, root, &mut BTreeSet::new(), &mut BTreeSet::new());
    }

    #[test]
    fn initialization_commits_ready_state_and_resolvable_establishment() {
        let compiled = compiled(10, 30);
        let input = ExternalInputKey::<Level>::from_u128(1);
        let output = ExternalOutputKey::<Level>::from_u128(30);
        let snapshot = compiled
            .input_snapshot()
            .set(input, LogicLevel::High)
            .and_then(crate::InputSnapshotBuilder::finish)
            .unwrap_or_else(|_| panic!("snapshot must build"));
        let mut machine = compiled.spawn(policy());
        let revision = machine.revision();

        let result = machine
            .apply(Transaction::initialize(
                crate::time::Time::from_ticks(37),
                revision,
                snapshot,
            ))
            .unwrap_or_else(|failure| panic!("initialization must succeed: {failure}"));

        assert_eq!(
            machine.status(),
            MachineStatus::Ready {
                now: crate::time::Time::from_ticks(37)
            }
        );
        assert_eq!(machine.revision(), revision);
        assert_eq!(machine.runtime_policy_id(), policy().id());
        assert_eq!(machine.external_level(input), Some(LogicLevel::High));
        assert_eq!(machine.output_level(output), Some(LogicLevel::High));
        let [
            OutputEvent::LevelEstablished {
                output: actual_output,
                value,
                at,
                cause,
                revision: actual_revision,
            },
        ] = result.output_events()
        else {
            panic!("initialization must return one establishment");
        };
        assert_eq!(*actual_output, output);
        assert_eq!(*value, LogicLevel::High);
        assert_eq!(*at, crate::time::Time::from_ticks(37));
        assert_eq!(*actual_revision, revision);
        assert!(result.provenance().inspect(*cause).is_ok());
        assert_eq!(machine.output_cause(output), Some(*cause));
        recursively_assert_acyclic(result.provenance(), *cause);
        assert_eq!(result.requested_time(), crate::time::Time::from_ticks(37));
        assert_eq!(result.before_revision(), revision);
        assert_eq!(result.after_revision(), revision);
    }

    #[test]
    fn all_publishes_only_settled_observable_levels() {
        let compiled = compiled_all();
        let first = ExternalInputKey::<Level>::from_u128(1);
        let second = ExternalInputKey::<Level>::from_u128(2);
        let output = ExternalOutputKey::<Level>::from_u128(30);
        let snapshot = compiled
            .input_snapshot()
            .set(first, LogicLevel::Low)
            .and_then(|builder| builder.set(second, LogicLevel::High))
            .and_then(crate::InputSnapshotBuilder::finish)
            .unwrap_or_else(|_| panic!("All snapshot must build"));
        let mut machine = compiled.spawn(policy_with([10, 100, 0, 100, 1_000]));
        let initialized = machine
            .apply(Transaction::initialize(
                crate::time::Time::from_ticks(10),
                machine.revision(),
                snapshot,
            ))
            .unwrap_or_else(|failure| panic!("All initialization must succeed: {failure}"));
        assert!(matches!(
            initialized.output_events(),
            [OutputEvent::LevelEstablished {
                output: actual,
                value: LogicLevel::Low,
                ..
            }] if *actual == output
        ));

        let changed = machine
            .apply(Transaction::advance(
                crate::time::Time::from_ticks(20),
                machine.revision(),
                compiled
                    .input_delta()
                    .set(first, LogicLevel::High)
                    .and_then(|builder| builder.set(second, LogicLevel::Low))
                    .and_then(crate::InputDeltaBuilder::finish)
                    .unwrap_or_else(|_| panic!("All delta must build")),
            ))
            .unwrap_or_else(|failure| panic!("All advance must succeed: {failure}"));
        assert!(changed.output_events().is_empty());
        assert_eq!(machine.output_level(output), Some(LogicLevel::Low));

        let changed = machine
            .apply(Transaction::advance(
                crate::time::Time::from_ticks(30),
                machine.revision(),
                compiled
                    .input_delta()
                    .set(second, LogicLevel::High)
                    .and_then(crate::InputDeltaBuilder::finish)
                    .unwrap_or_else(|_| panic!("All delta must build")),
            ))
            .unwrap_or_else(|failure| panic!("All advance must succeed: {failure}"));
        assert!(matches!(
            changed.output_events(),
            [OutputEvent::LevelChanged {
                output: actual,
                from: LogicLevel::Low,
                to: LogicLevel::High,
                ..
            }] if *actual == output
        ));
        assert_eq!(machine.output_level(output), Some(LogicLevel::High));
    }

    #[test]
    fn remaining_variadics_settle_simultaneous_changes_before_publishing() {
        let first = ExternalInputKey::<Level>::from_u128(1);
        let second = ExternalInputKey::<Level>::from_u128(2);
        let output = ExternalOutputKey::<Level>::from_u128(30);

        for kind in [
            NodeKind::<()>::any(),
            NodeKind::parity(),
            NodeKind::at_least(1),
        ] {
            let compiled = compiled_variadic(kind);
            let snapshot = compiled
                .input_snapshot()
                .set(first, LogicLevel::High)
                .and_then(|builder| builder.set(second, LogicLevel::Low))
                .and_then(crate::InputSnapshotBuilder::finish)
                .unwrap_or_else(|_| panic!("variadic snapshot must build"));
            let mut machine = compiled.spawn(policy_with([10, 100, 0, 100, 1_000]));
            machine
                .apply(Transaction::initialize(
                    crate::time::Time::from_ticks(10),
                    machine.revision(),
                    snapshot,
                ))
                .unwrap_or_else(|failure| {
                    panic!("variadic initialization must succeed: {failure}")
                });
            assert_eq!(machine.output_level(output), Some(LogicLevel::High));

            let unchanged = machine
                .apply(Transaction::advance(
                    crate::time::Time::from_ticks(20),
                    machine.revision(),
                    compiled
                        .input_delta()
                        .set(first, LogicLevel::Low)
                        .and_then(|builder| builder.set(second, LogicLevel::High))
                        .and_then(crate::InputDeltaBuilder::finish)
                        .unwrap_or_else(|_| panic!("simultaneous variadic delta must build")),
                ))
                .unwrap_or_else(|failure| panic!("variadic advance must succeed: {failure}"));
            assert!(unchanged.output_events().is_empty());
            assert_eq!(machine.output_level(output), Some(LogicLevel::High));

            let changed = machine
                .apply(Transaction::advance(
                    crate::time::Time::from_ticks(30),
                    machine.revision(),
                    compiled
                        .input_delta()
                        .set(second, LogicLevel::Low)
                        .and_then(crate::InputDeltaBuilder::finish)
                        .unwrap_or_else(|_| panic!("variadic delta must build")),
                ))
                .unwrap_or_else(|failure| panic!("variadic advance must succeed: {failure}"));
            assert!(matches!(
                changed.output_events(),
                [OutputEvent::LevelChanged {
                    output: actual,
                    from: LogicLevel::High,
                    to: LogicLevel::Low,
                    ..
                }] if *actual == output
            ));
        }
    }

    #[test]
    fn select_publishes_only_the_settled_selected_branch() {
        let compiled = compiled_select();
        let selector = ExternalInputKey::<Level>::from_u128(1);
        let when_low = ExternalInputKey::<Level>::from_u128(2);
        let when_high = ExternalInputKey::<Level>::from_u128(3);
        let output = ExternalOutputKey::<Level>::from_u128(30);
        let snapshot = compiled
            .input_snapshot()
            .set(selector, LogicLevel::Low)
            .and_then(|builder| builder.set(when_low, LogicLevel::Low))
            .and_then(|builder| builder.set(when_high, LogicLevel::High))
            .and_then(crate::InputSnapshotBuilder::finish)
            .unwrap_or_else(|_| panic!("Select snapshot must build"));
        let mut machine = compiled.spawn(policy_with([10, 100, 0, 100, 1_000]));
        machine
            .apply(Transaction::initialize(
                crate::time::Time::from_ticks(10),
                machine.revision(),
                snapshot,
            ))
            .unwrap_or_else(|failure| panic!("Select initialization must succeed: {failure}"));
        assert_eq!(machine.output_level(output), Some(LogicLevel::Low));

        let unchanged = machine
            .apply(Transaction::advance(
                crate::time::Time::from_ticks(20),
                machine.revision(),
                compiled
                    .input_delta()
                    .set(selector, LogicLevel::High)
                    .and_then(|builder| builder.set(when_low, LogicLevel::High))
                    .and_then(|builder| builder.set(when_high, LogicLevel::Low))
                    .and_then(crate::InputDeltaBuilder::finish)
                    .unwrap_or_else(|_| panic!("simultaneous Select delta must build")),
            ))
            .unwrap_or_else(|failure| panic!("Select advance must succeed: {failure}"));
        assert!(unchanged.output_events().is_empty());
        assert_eq!(machine.output_level(output), Some(LogicLevel::Low));

        let changed = machine
            .apply(Transaction::advance(
                crate::time::Time::from_ticks(30),
                machine.revision(),
                compiled
                    .input_delta()
                    .set(when_high, LogicLevel::High)
                    .and_then(crate::InputDeltaBuilder::finish)
                    .unwrap_or_else(|_| panic!("selected-branch delta must build")),
            ))
            .unwrap_or_else(|failure| panic!("Select branch advance must succeed: {failure}"));
        assert!(matches!(
            changed.output_events(),
            [OutputEvent::LevelChanged {
                output: actual,
                from: LogicLevel::Low,
                to: LogicLevel::High,
                ..
            }] if *actual == output
        ));
    }

    #[test]
    fn wrong_network_rejection_preserves_awaiting_machine() {
        let local = compiled(10, 30);
        let foreign = compiled(11, 30);
        let input = ExternalInputKey::<Level>::from_u128(1);
        let snapshot = foreign
            .input_snapshot()
            .set(input, LogicLevel::Low)
            .and_then(crate::InputSnapshotBuilder::finish)
            .unwrap_or_else(|_| panic!("snapshot must build"));
        let mut machine = local.spawn(policy());
        let revision = machine.revision();
        let before = observe(&machine);

        let failure = machine
            .apply(Transaction::initialize(
                crate::time::Time::from_ticks(9),
                revision,
                snapshot,
            ))
            .unwrap_err();

        assert!(matches!(
            failure.evidence(),
            RuntimeFailureEvidence::WrongNetwork { .. }
        ));
        assert_eq!(observe(&machine), before);
        assert_eq!(machine.revision(), revision);
        assert_eq!(machine.external_level(input), None);
        assert_eq!(machine.output_level(ExternalOutputKey::from_u128(30)), None);
    }

    #[test]
    fn ordinary_evaluator_handles_chains_fanout_and_canonical_event_order() {
        for reverse_claims in [false, true] {
            let compiled = complex_compiled(reverse_claims);
            let snapshot = compiled
                .input_snapshot()
                .set(ExternalInputKey::from_u128(1), LogicLevel::Low)
                .and_then(|builder| builder.set(ExternalInputKey::from_u128(2), LogicLevel::High))
                .and_then(crate::InputSnapshotBuilder::finish)
                .unwrap_or_else(|_| panic!("snapshot must build"));
            let mut machine = compiled.spawn(policy_with([1, 9, 0, 3, 8]));
            let result = machine
                .apply(Transaction::initialize(
                    crate::time::Time::from_ticks(0),
                    machine.revision(),
                    snapshot,
                ))
                .unwrap_or_else(|failure| panic!("initialization must succeed: {failure}"));

            let events = result
                .output_events()
                .iter()
                .map(|event| match event {
                    OutputEvent::LevelEstablished {
                        output,
                        value,
                        cause,
                        ..
                    } => (*output, *value, *cause),
                    OutputEvent::LevelChanged { .. } => {
                        panic!("initialization must not emit level changes")
                    }
                    OutputEvent::Pulsed { .. } => {
                        panic!("level-only fixture must not emit pulse events")
                    }
                })
                .collect::<Vec<_>>();
            assert_eq!(
                events.iter().map(|event| event.0).collect::<Vec<_>>(),
                vec![
                    ExternalOutputKey::from_u128(10),
                    ExternalOutputKey::from_u128(30),
                    ExternalOutputKey::from_u128(50),
                ]
            );
            assert_eq!(
                events.iter().map(|event| event.1).collect::<Vec<_>>(),
                vec![LogicLevel::Low, LogicLevel::High, LogicLevel::High]
            );
            assert_eq!(machine.store.settled_levels.len(), 9);
            for (_, _, cause) in events {
                recursively_assert_acyclic(result.provenance(), cause);
            }
        }
    }

    #[test]
    fn equivalent_initializations_differ_only_by_requested_time() {
        let compiled = complex_compiled(false);
        let input = || {
            compiled
                .input_snapshot()
                .set(ExternalInputKey::from_u128(1), LogicLevel::Low)
                .and_then(|builder| builder.set(ExternalInputKey::from_u128(2), LogicLevel::High))
                .and_then(crate::InputSnapshotBuilder::finish)
                .unwrap_or_else(|_| panic!("snapshot must build"))
        };
        let expected_policy = policy_with([1, 9, 0, 3, 8]);
        let expected_policy_id = expected_policy.id();
        let mut at_zero = compiled.spawn(expected_policy.clone());
        let mut later = compiled.spawn(expected_policy);

        let zero_result = at_zero
            .apply(Transaction::initialize(
                crate::time::Time::from_ticks(0),
                at_zero.revision(),
                input(),
            ))
            .unwrap_or_else(|failure| panic!("zero-time initialization must succeed: {failure}"));
        let later_result = later
            .apply(Transaction::initialize(
                crate::time::Time::from_ticks(91),
                later.revision(),
                input(),
            ))
            .unwrap_or_else(|failure| {
                panic!("nonzero-time initialization must succeed: {failure}")
            });

        assert_eq!(at_zero.now(), Some(crate::time::Time::from_ticks(0)));
        assert_eq!(later.now(), Some(crate::time::Time::from_ticks(91)));
        assert_eq!(at_zero.revision(), later.revision());
        assert_eq!(at_zero.runtime_policy_id(), expected_policy_id);
        assert_eq!(later.runtime_policy_id(), expected_policy_id);
        assert_eq!(at_zero.store.external_levels, later.store.external_levels);
        assert_eq!(at_zero.store.settled_levels, later.store.settled_levels);
        assert_eq!(at_zero.store.output_baselines, later.store.output_baselines);
        assert_eq!(
            zero_result.output_events().len(),
            later_result.output_events().len()
        );
        for (zero, nonzero) in zero_result
            .output_events()
            .iter()
            .zip(later_result.output_events())
        {
            match (zero, nonzero) {
                (
                    OutputEvent::LevelEstablished {
                        output: zero_output,
                        value: zero_value,
                        revision: zero_revision,
                        ..
                    },
                    OutputEvent::LevelEstablished {
                        output: later_output,
                        value: later_value,
                        revision: later_revision,
                        ..
                    },
                ) => {
                    assert_eq!(zero_output, later_output);
                    assert_eq!(zero_value, later_value);
                    assert_eq!(zero_revision, later_revision);
                }
                _ => panic!("initialization comparison must contain establishments only"),
            }
        }
    }

    #[test]
    fn constant_and_empty_output_networks_initialize_without_special_cases() {
        let constant_output = OutPortKey::<Level>::from_u128(2);
        let output = ExternalOutputKey::<Level>::from_u128(3);
        let constant = UncheckedNetwork::<()>::new(
            NetworkKey::from_u128(1),
            TimeDomainId::from_u128(2),
            DiagnosticMeta::default(),
            vec![NodeDef::new(
                NodeKey::from_u128(4),
                NodeKind::<()>::constant(LogicLevel::High),
                NodePorts::new(Vec::new(), vec![constant_output.into()]),
                DiagnosticMeta::default(),
            )],
            Vec::new(),
            vec![ExternalOutputDef::new(
                output.into(),
                SignalSourceKey::NodeOutput(constant_output).into(),
                DiagnosticMeta::default(),
            )],
            Vec::new(),
        )
        .validate()
        .require_artifact()
        .unwrap_or_else(|_| panic!("fixture must validate"))
        .compile()
        .require_artifact()
        .unwrap_or_else(|_| panic!("fixture must compile"));
        let mut constant_machine = constant.spawn(policy_with([1, 3, 0, 1, 3]));
        let result = constant_machine
            .apply(Transaction::initialize(
                crate::time::Time::from_ticks(5),
                constant_machine.revision(),
                constant
                    .input_snapshot()
                    .finish()
                    .unwrap_or_else(|_| panic!("empty snapshot must build")),
            ))
            .unwrap_or_else(|failure| panic!("constant initialization must succeed: {failure}"));
        assert_eq!(
            constant_machine.output_level(output),
            Some(LogicLevel::High)
        );
        assert_eq!(result.output_events().len(), 1);

        let empty = UncheckedNetwork::<()>::new(
            NetworkKey::from_u128(9),
            TimeDomainId::from_u128(2),
            DiagnosticMeta::default(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
        )
        .validate()
        .require_artifact()
        .unwrap_or_else(|_| panic!("fixture must validate"))
        .compile()
        .require_artifact()
        .unwrap_or_else(|_| panic!("fixture must compile"));
        let mut empty_machine = empty.spawn(policy_with([1, 0, 0, 0, 1]));
        let result = empty_machine
            .apply(Transaction::initialize(
                crate::time::Time::from_ticks(u64::MAX),
                empty_machine.revision(),
                empty
                    .input_snapshot()
                    .finish()
                    .unwrap_or_else(|_| panic!("empty snapshot must build")),
            ))
            .unwrap_or_else(|failure| panic!("empty initialization must succeed: {failure}"));
        assert!(result.output_events().is_empty());
        assert_eq!(
            empty_machine.now(),
            Some(crate::time::Time::from_ticks(u64::MAX))
        );
    }

    #[test]
    fn every_identity_and_lifecycle_rejection_is_structured_and_atomic() {
        let local = compiled(10, 30);
        let input = ExternalInputKey::<Level>::from_u128(1);

        let mut stale = local.spawn(policy());
        let expected = stale.revision();
        stale.store.revision = NetworkRevision::from_value(9);
        let before = observe(&stale);
        let failure = stale
            .apply(Transaction::initialize(
                crate::time::Time::from_ticks(1),
                expected,
                local
                    .input_snapshot()
                    .set(input, LogicLevel::Low)
                    .and_then(crate::InputSnapshotBuilder::finish)
                    .unwrap_or_else(|_| panic!("snapshot must build")),
            ))
            .unwrap_err();
        assert!(matches!(
            failure.evidence(),
            RuntimeFailureEvidence::StaleRevision { .. }
        ));
        assert_eq!(observe(&stale), before);
        assert_eq!(stale.revision(), NetworkRevision::from_value(9));

        let same_schema_other_topology = compiled(10, 31);
        let mut fingerprint_machine = local.spawn(policy());
        let before = observe(&fingerprint_machine);
        let failure = fingerprint_machine
            .apply(Transaction::initialize(
                crate::time::Time::from_ticks(1),
                fingerprint_machine.revision(),
                same_schema_other_topology
                    .input_snapshot()
                    .set(input, LogicLevel::Low)
                    .and_then(crate::InputSnapshotBuilder::finish)
                    .unwrap_or_else(|_| panic!("snapshot must build")),
            ))
            .unwrap_err();
        assert!(matches!(
            failure.evidence(),
            RuntimeFailureEvidence::WrongNetwork { .. }
        ));
        assert_eq!(observe(&fingerprint_machine), before);

        let other_schema = compiled_with_input(10, 2, 30);
        let mut schema_machine = local.spawn(policy());
        let before = observe(&schema_machine);
        let failure = schema_machine
            .apply(Transaction::initialize(
                crate::time::Time::from_ticks(1),
                schema_machine.revision(),
                other_schema
                    .input_snapshot()
                    .set(ExternalInputKey::from_u128(2), LogicLevel::Low)
                    .and_then(crate::InputSnapshotBuilder::finish)
                    .unwrap_or_else(|_| panic!("snapshot must build")),
            ))
            .unwrap_err();
        assert!(matches!(
            failure.evidence(),
            RuntimeFailureEvidence::ForeignInputSchema { .. }
        ));
        assert_eq!(observe(&schema_machine), before);

        let mut ready = local.spawn(policy());
        let first = local
            .input_snapshot()
            .set(input, LogicLevel::High)
            .and_then(crate::InputSnapshotBuilder::finish)
            .unwrap_or_else(|_| panic!("snapshot must build"));
        ready
            .apply(Transaction::initialize(
                crate::time::Time::from_ticks(4),
                ready.revision(),
                first,
            ))
            .unwrap_or_else(|failure| panic!("first initialization must succeed: {failure}"));
        let before_cause = ready.output_cause(ExternalOutputKey::from_u128(30));
        let before_provenance_len = ready
            .store
            .provenance
            .as_ref()
            .map_or(0, ProvenanceView::len);
        let before = observe(&ready);
        let failure = ready
            .apply(Transaction::initialize(
                crate::time::Time::from_ticks(5),
                ready.revision(),
                local
                    .input_snapshot()
                    .set(input, LogicLevel::Low)
                    .and_then(crate::InputSnapshotBuilder::finish)
                    .unwrap_or_else(|_| panic!("snapshot must build")),
            ))
            .unwrap_err();
        assert!(matches!(
            failure.evidence(),
            RuntimeFailureEvidence::AlreadyInitialized
        ));
        assert_eq!(observe(&ready), before);
        assert_eq!(ready.now(), Some(crate::time::Time::from_ticks(4)));
        assert_eq!(ready.external_level(input), Some(LogicLevel::High));
        assert_eq!(
            ready.output_cause(ExternalOutputKey::from_u128(30)),
            before_cause
        );
        assert_eq!(
            ready
                .store
                .provenance
                .as_ref()
                .map_or(0, ProvenanceView::len),
            before_provenance_len
        );
    }

    #[test]
    fn implemented_budget_boundaries_reject_one_over_and_preserve_machine() {
        let cases = [
            (
                RuntimePolicyLimit::MaxInternalReactions,
                [0, 2, 0, 1, 3],
                0,
                1,
            ),
            (
                RuntimePolicyLimit::MaxEvaluatedOperations,
                [1, 1, 0, 1, 3],
                1,
                2,
            ),
            (
                RuntimePolicyLimit::MaxEventsCreatedPerTransaction,
                [1, 2, 0, 0, 3],
                0,
                1,
            ),
            (
                RuntimePolicyLimit::MaxRequiredProvenanceGrowth,
                [1, 2, 0, 1, 2],
                2,
                3,
            ),
        ];
        for (expected_budget, limits, expected_limit, expected_consumed) in cases {
            let compiled = compiled(10, 30);
            let input = ExternalInputKey::<Level>::from_u128(1);
            let mut machine = compiled.spawn(policy_with(limits));
            let before = observe(&machine);
            let failure = machine
                .apply(Transaction::initialize(
                    crate::time::Time::from_ticks(7),
                    machine.revision(),
                    compiled
                        .input_snapshot()
                        .set(input, LogicLevel::High)
                        .and_then(crate::InputSnapshotBuilder::finish)
                        .unwrap_or_else(|_| panic!("snapshot must build")),
                ))
                .unwrap_err();
            assert_eq!(failure.code(), DiagnosticCode::RuntimeBudgetExceeded);
            assert_eq!(failure.severity(), Severity::Error);
            assert_eq!(failure.responsibility(), Responsibility::ResourceLimit);
            assert_eq!(
                failure.evidence(),
                &RuntimeFailureEvidence::BudgetExceeded {
                    budget: expected_budget,
                    limit: expected_limit,
                    consumed: expected_consumed,
                }
            );
            assert_eq!(observe(&machine), before);
        }

        for limits in [[1, 2, 0, 1, 3], [2, 3, 0, 2, 4]] {
            let compiled = compiled(10, 30);
            let mut machine = compiled.spawn(policy_with(limits));
            machine
                .apply(Transaction::initialize(
                    crate::time::Time::from_ticks(8),
                    machine.revision(),
                    compiled
                        .input_snapshot()
                        .set(ExternalInputKey::from_u128(1), LogicLevel::Low)
                        .and_then(crate::InputSnapshotBuilder::finish)
                        .unwrap_or_else(|_| panic!("snapshot must build")),
                ))
                .unwrap_or_else(|failure| panic!("within-budget work must succeed: {failure}"));
            assert!(machine.is_initialized());
        }
    }

    #[test]
    fn transaction_inspection_is_lifecycle_aware_and_delta_requires_ready() {
        let compiled = compiled(10, 30);
        let input = ExternalInputKey::<Level>::from_u128(1);
        let snapshot = compiled
            .input_snapshot()
            .set(input, LogicLevel::Low)
            .and_then(crate::InputSnapshotBuilder::finish)
            .unwrap_or_else(|_| panic!("snapshot must build"));
        let initialize = Transaction::initialize(
            crate::time::Time::from_ticks(10),
            NetworkRevision::from_value(0),
            snapshot,
        );
        assert!(initialize.initialization_input().is_some());
        assert!(initialize.advance_input().is_none());

        let delta = compiled
            .input_delta()
            .set(input, LogicLevel::High)
            .and_then(crate::InputDeltaBuilder::finish)
            .unwrap_or_else(|_| panic!("delta must build"));
        let advance = Transaction::advance(
            crate::time::Time::from_ticks(11),
            NetworkRevision::from_value(0),
            delta,
        );
        assert!(advance.initialization_input().is_none());
        assert!(advance.advance_input().is_some());

        let mut machine = compiled.spawn(policy_with([10, 100, 0, 100, 1_000]));
        let before = observe(&machine);
        let failure = machine.apply(advance).unwrap_err();
        assert_eq!(
            failure.code(),
            DiagnosticCode::LifecycleDeltaBeforeInitialization
        );
        assert!(matches!(
            failure.evidence(),
            RuntimeFailureEvidence::DeltaBeforeInitialization
        ));
        assert_eq!(failure.responsibility(), Responsibility::CallerInput);
        assert_eq!(observe(&machine), before);
    }

    #[test]
    fn ready_advancement_overlays_delta_and_emits_only_genuine_changes() {
        let compiled = compiled(10, 30);
        let input = ExternalInputKey::<Level>::from_u128(1);
        let output = ExternalOutputKey::<Level>::from_u128(30);
        let mut machine = compiled.spawn(policy_with([10, 100, 0, 100, 1_000]));
        let initialization = machine
            .apply(Transaction::initialize(
                crate::time::Time::from_ticks(10),
                machine.revision(),
                compiled
                    .input_snapshot()
                    .set(input, LogicLevel::Low)
                    .and_then(crate::InputSnapshotBuilder::finish)
                    .unwrap_or_else(|_| panic!("snapshot must build")),
            ))
            .unwrap_or_else(|failure| panic!("initialization must succeed: {failure}"));
        let initialization_cause = match initialization.output_events() {
            [OutputEvent::LevelEstablished { cause, .. }] => *cause,
            _ => panic!("initialization must establish one output"),
        };
        let revision = machine.revision();
        let policy_id = machine.runtime_policy_id();

        let changed = machine
            .apply(Transaction::advance(
                crate::time::Time::from_ticks(20),
                revision,
                compiled
                    .input_delta()
                    .set(input, LogicLevel::High)
                    .and_then(crate::InputDeltaBuilder::finish)
                    .unwrap_or_else(|_| panic!("delta must build")),
            ))
            .unwrap_or_else(|failure| panic!("advance must succeed: {failure}"));
        let changed_cause = match changed.output_events() {
            [
                OutputEvent::LevelChanged {
                    output: actual_output,
                    from,
                    to,
                    at,
                    cause,
                    revision: actual_revision,
                },
            ] => {
                assert_eq!(*actual_output, output);
                assert_eq!(*from, LogicLevel::Low);
                assert_eq!(*to, LogicLevel::High);
                assert_eq!(*at, crate::time::Time::from_ticks(20));
                assert_eq!(*actual_revision, revision);
                *cause
            }
            _ => panic!("one changed output must be published"),
        };
        recursively_assert_acyclic(changed.provenance(), changed_cause);
        assert_eq!(machine.external_level(input), Some(LogicLevel::High));
        assert_eq!(machine.output_level(output), Some(LogicLevel::High));
        assert_eq!(machine.output_cause(output), Some(changed_cause));
        assert_eq!(machine.now(), Some(crate::time::Time::from_ticks(20)));
        assert_eq!(machine.revision(), revision);
        assert_eq!(machine.runtime_policy_id(), policy_id);
        assert_eq!(changed.before_revision(), changed.after_revision());

        let reasserted = machine
            .apply(Transaction::advance(
                crate::time::Time::from_ticks(21),
                revision,
                compiled
                    .input_delta()
                    .set(input, LogicLevel::High)
                    .and_then(crate::InputDeltaBuilder::finish)
                    .unwrap_or_else(|_| panic!("delta must build")),
            ))
            .unwrap_or_else(|failure| panic!("reassertion must succeed: {failure}"));
        assert!(reasserted.output_events().is_empty());
        let reasserted_cause = machine
            .output_cause(output)
            .unwrap_or_else(|| panic!("unchanged output must retain a cause"));
        assert!(reasserted.provenance().inspect(reasserted_cause).is_ok());

        let empty = machine
            .apply(Transaction::advance(
                crate::time::Time::from_ticks(22),
                revision,
                compiled
                    .input_delta()
                    .finish()
                    .unwrap_or_else(|_| panic!("empty delta must build")),
            ))
            .unwrap_or_else(|failure| panic!("empty advance must succeed: {failure}"));
        assert!(empty.output_events().is_empty());
        assert_eq!(machine.external_level(input), Some(LogicLevel::High));
        let empty_cause = machine
            .output_cause(output)
            .unwrap_or_else(|| panic!("unchanged output must retain a cause"));
        assert!(empty.provenance().inspect(empty_cause).is_ok());
        assert!(
            initialization
                .provenance()
                .inspect(initialization_cause)
                .is_ok()
        );
        assert!(changed.provenance().inspect(changed_cause).is_ok());
    }

    #[test]
    fn partial_and_complete_deltas_fully_resettle_reconvergent_graphs() {
        let compiled = complex_compiled(false);
        let first = ExternalInputKey::<Level>::from_u128(1);
        let second = ExternalInputKey::<Level>::from_u128(2);
        let mut machine = compiled.spawn(policy_with([10, 100, 0, 100, 1_000]));
        machine
            .apply(Transaction::initialize(
                crate::time::Time::from_ticks(4),
                machine.revision(),
                compiled
                    .input_snapshot()
                    .set(first, LogicLevel::Low)
                    .and_then(|builder| builder.set(second, LogicLevel::High))
                    .and_then(crate::InputSnapshotBuilder::finish)
                    .unwrap_or_else(|_| panic!("snapshot must build")),
            ))
            .unwrap_or_else(|failure| panic!("initialization must succeed: {failure}"));

        let partial = machine
            .apply(Transaction::advance(
                crate::time::Time::from_ticks(9),
                machine.revision(),
                compiled
                    .input_delta()
                    .set(first, LogicLevel::High)
                    .and_then(crate::InputDeltaBuilder::finish)
                    .unwrap_or_else(|_| panic!("partial delta must build")),
            ))
            .unwrap_or_else(|failure| panic!("partial advance must succeed: {failure}"));
        assert_eq!(machine.external_level(first), Some(LogicLevel::High));
        assert_eq!(machine.external_level(second), Some(LogicLevel::High));
        assert_eq!(machine.store.settled_levels.len(), 9);
        assert_eq!(
            partial
                .output_events()
                .iter()
                .filter_map(|event| match event {
                    OutputEvent::LevelChanged { output, .. } => Some(*output),
                    OutputEvent::LevelEstablished { .. } | OutputEvent::Pulsed { .. } => None,
                })
                .collect::<Vec<_>>(),
            vec![
                ExternalOutputKey::from_u128(10),
                ExternalOutputKey::from_u128(50),
            ]
        );

        let complete = machine
            .apply(Transaction::advance(
                crate::time::Time::from_ticks(15),
                machine.revision(),
                compiled
                    .input_delta()
                    .set(second, LogicLevel::Low)
                    .and_then(|builder| builder.set(first, LogicLevel::Low))
                    .and_then(crate::InputDeltaBuilder::finish)
                    .unwrap_or_else(|_| panic!("complete delta must build")),
            ))
            .unwrap_or_else(|failure| panic!("complete advance must succeed: {failure}"));
        assert_eq!(machine.external_level(first), Some(LogicLevel::Low));
        assert_eq!(machine.external_level(second), Some(LogicLevel::Low));
        assert_eq!(machine.store.settled_levels.len(), 9);
        for event in complete.output_events() {
            if let OutputEvent::LevelChanged { cause, .. } = event {
                recursively_assert_acyclic(complete.provenance(), *cause);
            }
        }
    }

    #[test]
    fn ready_rejections_are_structured_and_preserve_complete_machine() {
        let local = compiled(10, 30);
        let foreign_network = compiled(11, 30);
        let other_schema = compiled_with_input(10, 2, 30);
        let input = ExternalInputKey::<Level>::from_u128(1);

        for requested in [9, 10] {
            let mut machine = initialized_machine(&local, LogicLevel::Low);
            let before = observe(&machine);
            let failure = machine
                .apply(Transaction::advance(
                    crate::time::Time::from_ticks(requested),
                    machine.revision(),
                    local
                        .input_delta()
                        .finish()
                        .unwrap_or_else(|_| panic!("delta must build")),
                ))
                .unwrap_err();
            assert_eq!(
                failure.code(),
                DiagnosticCode::RuntimeTimeNotStrictlyIncreasing
            );
            assert!(matches!(
                failure.evidence(),
                RuntimeFailureEvidence::TimeNotStrictlyIncreasing { .. }
            ));
            assert_eq!(observe(&machine), before);
        }

        let mut stale_revision = initialized_machine(&local, LogicLevel::Low);
        let expected = stale_revision.revision();
        stale_revision.store.revision = NetworkRevision::from_value(7);
        let before = observe(&stale_revision);
        let failure = stale_revision
            .apply(Transaction::advance(
                crate::time::Time::from_ticks(11),
                expected,
                local
                    .input_delta()
                    .finish()
                    .unwrap_or_else(|_| panic!("delta must build")),
            ))
            .unwrap_err();
        assert!(matches!(
            failure.evidence(),
            RuntimeFailureEvidence::StaleRevision { .. }
        ));
        assert_eq!(observe(&stale_revision), before);

        let mut wrong_network = initialized_machine(&local, LogicLevel::Low);
        let before = observe(&wrong_network);
        let failure = wrong_network
            .apply(Transaction::advance(
                crate::time::Time::from_ticks(11),
                wrong_network.revision(),
                foreign_network
                    .input_delta()
                    .set(input, LogicLevel::High)
                    .and_then(crate::InputDeltaBuilder::finish)
                    .unwrap_or_else(|_| panic!("delta must build")),
            ))
            .unwrap_err();
        assert!(matches!(
            failure.evidence(),
            RuntimeFailureEvidence::WrongNetwork { .. }
        ));
        assert_eq!(observe(&wrong_network), before);

        let mut stale_schema = initialized_machine(&local, LogicLevel::Low);
        let before = observe(&stale_schema);
        let failure = stale_schema
            .apply(Transaction::advance(
                crate::time::Time::from_ticks(11),
                stale_schema.revision(),
                other_schema
                    .input_delta()
                    .set(ExternalInputKey::from_u128(2), LogicLevel::High)
                    .and_then(crate::InputDeltaBuilder::finish)
                    .unwrap_or_else(|_| panic!("delta must build")),
            ))
            .unwrap_err();
        assert_eq!(failure.code(), DiagnosticCode::InputStaleSchema);
        assert!(matches!(
            failure.evidence(),
            RuntimeFailureEvidence::StaleInputSchema { .. }
        ));
        assert_eq!(observe(&stale_schema), before);

        let mixed = local
            .input_delta()
            .finish()
            .unwrap_or_else(|_| panic!("delta must build"))
            .with_test_bindings(local.fingerprint(), other_schema.input_schema_fingerprint());
        let mut foreign_schema = initialized_machine(&local, LogicLevel::Low);
        let before = observe(&foreign_schema);
        let failure = foreign_schema
            .apply(Transaction::advance(
                crate::time::Time::from_ticks(11),
                foreign_schema.revision(),
                mixed,
            ))
            .unwrap_err();
        assert_eq!(failure.code(), DiagnosticCode::InputForeignSchema);
        assert!(matches!(
            failure.evidence(),
            RuntimeFailureEvidence::ForeignInputSchema { .. }
        ));
        assert_eq!(observe(&foreign_schema), before);
    }

    #[test]
    fn merge_accepts_the_maximum_count_and_overflow_preserves_complete_machine() {
        let compiled = compiled_merge();
        let first = ExternalInputKey::<Pulse>::from_u128(1);
        let second = ExternalInputKey::<Pulse>::from_u128(2);
        let output = ExternalOutputKey::<Pulse>::from_u128(30);
        let mut machine = compiled.spawn(policy_with([10, 100, 0, 100, 1_000]));
        let initialized = machine
            .apply(Transaction::initialize(
                crate::time::Time::from_ticks(10),
                machine.revision(),
                compiled
                    .input_snapshot()
                    .pulse(first, PulseCount::new(u64::MAX))
                    .and_then(|builder| builder.pulse(second, PulseCount::ZERO))
                    .and_then(crate::InputSnapshotBuilder::finish)
                    .unwrap_or_else(|_| panic!("boundary snapshot must build")),
            ))
            .unwrap_or_else(|failure| {
                panic!("maximum representable count must succeed: {failure}")
            });
        assert!(matches!(
            initialized.output_events(),
            [OutputEvent::Pulsed {
                output: actual_output,
                count,
                ..
            }] if *actual_output == output && count.get() == u64::MAX
        ));

        let before = observe(&machine);
        let failure = machine
            .apply(Transaction::advance(
                crate::time::Time::from_ticks(11),
                machine.revision(),
                compiled
                    .input_delta()
                    .pulse(first, PulseCount::new(u64::MAX))
                    .and_then(|builder| builder.pulse(second, PulseCount::ONE))
                    .and_then(crate::InputDeltaBuilder::finish)
                    .unwrap_or_else(|_| panic!("overflowing delta must build")),
            ))
            .unwrap_err();
        assert_eq!(
            failure.evidence(),
            &RuntimeFailureEvidence::PulseCountOverflow {
                node: NodeSubject::Node(NodeKey::from_u128(10)),
                left: PulseCount::new(u64::MAX),
                right: PulseCount::ONE,
            }
        );
        assert_eq!(observe(&machine), before);
    }

    #[test]
    fn equivalent_ready_batches_ignore_insertion_authored_and_metadata_order() {
        let mut outcomes = Vec::new();
        for (reverse_claims, network_name, reverse_delta) in
            [(false, None, false), (true, Some("renamed only"), true)]
        {
            let compiled = complex_compiled_with_name(reverse_claims, network_name);
            let first = ExternalInputKey::<Level>::from_u128(1);
            let second = ExternalInputKey::<Level>::from_u128(2);
            let mut machine = compiled.spawn(policy_with([10, 100, 0, 100, 1_000]));
            machine
                .apply(Transaction::initialize(
                    crate::time::Time::from_ticks(3),
                    machine.revision(),
                    compiled
                        .input_snapshot()
                        .set(first, LogicLevel::Low)
                        .and_then(|builder| builder.set(second, LogicLevel::High))
                        .and_then(crate::InputSnapshotBuilder::finish)
                        .unwrap_or_else(|_| panic!("snapshot must build")),
                ))
                .unwrap_or_else(|failure| panic!("initialization must succeed: {failure}"));
            let delta = if reverse_delta {
                compiled
                    .input_delta()
                    .set(second, LogicLevel::Low)
                    .and_then(|builder| builder.set(first, LogicLevel::High))
            } else {
                compiled
                    .input_delta()
                    .set(first, LogicLevel::High)
                    .and_then(|builder| builder.set(second, LogicLevel::Low))
            }
            .and_then(crate::InputDeltaBuilder::finish)
            .unwrap_or_else(|_| panic!("delta must build"));
            let result = machine
                .apply(Transaction::advance(
                    crate::time::Time::from_ticks(8),
                    machine.revision(),
                    delta,
                ))
                .unwrap_or_else(|failure| panic!("advance must succeed: {failure}"));
            let events = result
                .output_events()
                .iter()
                .filter_map(|event| match event {
                    OutputEvent::LevelChanged {
                        output,
                        from,
                        to,
                        cause,
                        ..
                    } => Some((*output, *from, *to, *cause)),
                    OutputEvent::LevelEstablished { .. } | OutputEvent::Pulsed { .. } => None,
                })
                .collect::<Vec<_>>();
            outcomes.push((
                compiled.fingerprint(),
                compiled.input_schema_fingerprint(),
                machine.store.external_levels.clone(),
                machine.store.settled_levels.clone(),
                machine.store.output_baselines.clone(),
                events,
            ));
        }
        assert_eq!(outcomes[0], outcomes[1]);
    }

    #[test]
    fn ready_budget_boundaries_cover_below_exact_and_above() {
        let cases = [
            (RuntimePolicyLimit::MaxInternalReactions, 1_u64),
            (RuntimePolicyLimit::MaxEvaluatedOperations, 2_u64),
            (RuntimePolicyLimit::MaxEventsCreatedPerTransaction, 1_u64),
            (RuntimePolicyLimit::MaxRequiredProvenanceGrowth, 3_u64),
        ];
        for (budget, consumed) in cases {
            for limit in [consumed - 1, consumed, consumed + 1] {
                let compiled = compiled(10, 30);
                let mut machine = initialized_machine(&compiled, LogicLevel::Low);
                let mut limits = [10, 100, 0, 100, 1_000];
                match budget {
                    RuntimePolicyLimit::MaxInternalReactions => limits[0] = limit,
                    RuntimePolicyLimit::MaxEvaluatedOperations => limits[1] = limit,
                    RuntimePolicyLimit::MaxEventsCreatedPerTransaction => limits[3] = limit,
                    RuntimePolicyLimit::MaxRequiredProvenanceGrowth => limits[4] = limit,
                    RuntimePolicyLimit::MaxPendingEvents => {
                        panic!("pending-event accounting is outside this slice")
                    }
                }
                machine.policy = policy_with(limits);
                let before = observe(&machine);
                let result = machine.apply(Transaction::advance(
                    crate::time::Time::from_ticks(11),
                    machine.revision(),
                    compiled
                        .input_delta()
                        .set(ExternalInputKey::from_u128(1), LogicLevel::High)
                        .and_then(crate::InputDeltaBuilder::finish)
                        .unwrap_or_else(|_| panic!("delta must build")),
                ));
                if limit < consumed {
                    let failure = result.unwrap_err();
                    assert_eq!(
                        failure.evidence(),
                        &RuntimeFailureEvidence::BudgetExceeded {
                            budget,
                            limit,
                            consumed,
                        }
                    );
                    assert_eq!(observe(&machine), before);
                } else {
                    result.unwrap_or_else(|failure| {
                        panic!("within-boundary advance must succeed: {failure}")
                    });
                    assert_eq!(machine.now(), Some(crate::time::Time::from_ticks(11)));
                }
            }
        }
    }
    #[test]
    fn deadline_episode_creation_and_resolution_roll_back_with_the_complete_machine() {
        use crate::time::{NonZeroSpan, Time};
        use crate::{
            ConflictPolicy, LevelSetResetConfig, NetworkBuilder, PulseDelayConfig, ToggleConfig,
        };
        let mut b = NetworkBuilder::<()>::with_key(
            NetworkKey::from_u128(90),
            crate::TimeDomainId::from_u128(2),
        );
        let (input, p) = b.pulse_input("schedule");
        let (reject, control) = b.level_input("reject");
        let a = b
            .pulse_delay(
                p,
                PulseDelayConfig::new(NonZeroSpan::from_ticks(2).unwrap()),
            )
            .unwrap();
        let z = b
            .pulse_delay(
                p,
                PulseDelayConfig::new(NonZeroSpan::from_ticks(4).unwrap()),
            )
            .unwrap();
        let pulses = b.merge([a, z]).unwrap();
        let set = b
            .toggle(pulses, ToggleConfig::new(LogicLevel::Low))
            .unwrap();
        let high = b.constant(LogicLevel::High);
        let state = b
            .level_set_reset_latch(
                set,
                high,
                LevelSetResetConfig::new(LogicLevel::Low, ConflictPolicy::RetainAndDiagnose),
            )
            .unwrap();
        b.level_output("state", state).unwrap();
        b.level_set_reset_latch(
            control,
            control,
            LevelSetResetConfig::new(LogicLevel::Low, ConflictPolicy::RejectTransaction),
        )
        .unwrap();
        let c = b
            .finish()
            .require_artifact()
            .unwrap()
            .compile()
            .require_artifact()
            .unwrap();
        for (start, target) in [(0, 3), (2, 5)] {
            let mut m = c.spawn(policy_with([100, 10000, 100, 1000, 100000]));
            let initialized = m
                .apply(Transaction::initialize(
                    Time::from_ticks(0),
                    m.revision(),
                    c.input_snapshot()
                        .set(reject, LogicLevel::Low)
                        .unwrap()
                        .pulse(input, PulseCount::ONE)
                        .unwrap()
                        .finish()
                        .unwrap(),
                ))
                .unwrap();
            if start > 0 {
                m.apply(Transaction::advance(
                    Time::from_ticks(start),
                    m.revision(),
                    c.input_delta().finish().unwrap(),
                ))
                .unwrap();
            }
            let before = observe(&m);
            let failure = m
                .apply(Transaction::advance(
                    Time::from_ticks(target),
                    m.revision(),
                    c.input_delta()
                        .set(reject, LogicLevel::High)
                        .unwrap()
                        .finish()
                        .unwrap(),
                ))
                .unwrap_err();
            assert_eq!(
                failure.code(),
                crate::diagnostics::DiagnosticCode::RuntimeLevelLatchConflictRejected
            );
            assert_eq!(observe(&m), before);
            for event in initialized.output_events() {
                let cause = match event {
                    OutputEvent::LevelEstablished { cause, .. }
                    | OutputEvent::LevelChanged { cause, .. }
                    | OutputEvent::Pulsed { cause, .. } => *cause,
                };
                assert!(initialized.provenance().inspect(cause).is_ok());
            }
        }
    }
    #[test]
    fn sample_hold_rolls_back_capture_and_earlier_deadline_with_complete_state() {
        use crate::{
            ConflictPolicy, LevelSetResetConfig, NetworkBuilder, PulseDelayConfig, SampleHoldConfig,
        };
        let mut b = NetworkBuilder::<()>::new(crate::TimeDomainId::from_u128(2));
        let (value, v) = b.level_input("value");
        let (sample, p) = b.pulse_input("sample");
        let (reject, r) = b.level_input("reject");
        let delayed = b
            .pulse_delay(
                p,
                PulseDelayConfig::new(crate::time::NonZeroSpan::from_ticks(2).unwrap()),
            )
            .unwrap();
        let samples = b.merge([p, delayed]).unwrap();
        let node = NodeKey::from_u128(100);
        let held = b
            .add_sample_hold(
                node,
                v,
                samples,
                SampleHoldConfig::new(LogicLevel::Low),
                DiagnosticMeta::default(),
            )
            .unwrap()
            .into_outputs();
        b.level_output("held", held).unwrap();
        let bad = b.all([held, r]).unwrap();
        b.level_set_reset_latch(
            bad,
            bad,
            LevelSetResetConfig::new(LogicLevel::Low, ConflictPolicy::RejectTransaction),
        )
        .unwrap();
        let high = b.constant(LogicLevel::High);
        b.level_set_reset_latch(
            high,
            high,
            LevelSetResetConfig::new(LogicLevel::Low, ConflictPolicy::RetainAndDiagnose),
        )
        .unwrap();
        let c = b
            .finish()
            .require_artifact()
            .unwrap()
            .compile()
            .require_artifact()
            .unwrap();
        for target in [1, 3] {
            let mut m = c.spawn(policy_with([100, 10000, 100, 1000, 100000]));
            let initial = m
                .apply(Transaction::initialize(
                    crate::time::Time::from_ticks(0),
                    m.revision(),
                    c.input_snapshot()
                        .set(value, LogicLevel::Low)
                        .unwrap()
                        .set(reject, LogicLevel::Low)
                        .unwrap()
                        .pulse(sample, PulseCount::ONE)
                        .unwrap()
                        .finish()
                        .unwrap(),
                ))
                .unwrap();
            let retained = m.inspect_sample_hold(node).unwrap();
            if target == 3 {
                // Value changes before the due sample: no immediate capture at t=1.
                m.apply(Transaction::advance(
                    crate::time::Time::from_ticks(1),
                    m.revision(),
                    c.input_delta()
                        .set(value, LogicLevel::High)
                        .unwrap()
                        .finish()
                        .unwrap(),
                ))
                .unwrap();
            }
            let before = observe(&m);
            let mut delta = c.input_delta().set(reject, LogicLevel::High).unwrap();
            if target == 1 {
                delta = delta
                    .set(value, LogicLevel::High)
                    .unwrap()
                    .pulse(sample, PulseCount::ONE)
                    .unwrap();
            }
            let failure = m
                .apply(Transaction::advance(
                    crate::time::Time::from_ticks(target),
                    m.revision(),
                    delta.finish().unwrap(),
                ))
                .unwrap_err();
            assert_eq!(
                failure.code(),
                DiagnosticCode::RuntimeLevelLatchConflictRejected
            );
            assert_eq!(observe(&m), before);
            assert_eq!(
                m.inspect_sample_hold(node).unwrap().committed(),
                LogicLevel::Low
            );
            assert!(
                retained
                    .provenance()
                    .inspect(retained.latest_establishment())
                    .is_ok()
            );
            for event in initial.output_events() {
                let cause = match event {
                    OutputEvent::LevelEstablished { cause, .. }
                    | OutputEvent::LevelChanged { cause, .. }
                    | OutputEvent::Pulsed { cause, .. } => *cause,
                };
                assert!(initial.provenance().inspect(cause).is_ok());
            }
        }
    }

    #[test]
    fn sample_hold_event_and_provenance_budgets_are_atomic_at_both_lifecycle_boundaries() {
        let mut b = crate::NetworkBuilder::<()>::new(crate::TimeDomainId::from_u128(2));
        let (value, v) = b.level_input("value");
        let (sample, p) = b.pulse_input("sample");
        let node = NodeKey::from_u128(100);
        let state = b
            .add_sample_hold(
                node,
                v,
                p,
                crate::SampleHoldConfig::new(LogicLevel::Low),
                DiagnosticMeta::default(),
            )
            .unwrap()
            .into_outputs();
        b.level_output("held", state).unwrap();
        let c = b
            .finish()
            .require_artifact()
            .unwrap()
            .compile()
            .require_artifact()
            .unwrap();
        let make_init = |capture| {
            c.input_snapshot()
                .set(
                    value,
                    if capture {
                        LogicLevel::High
                    } else {
                        LogicLevel::Low
                    },
                )
                .unwrap()
                .pulse(
                    sample,
                    if capture {
                        PulseCount::ONE
                    } else {
                        PulseCount::ZERO
                    },
                )
                .unwrap()
                .finish()
                .unwrap()
        };
        let make_delta = || {
            c.input_delta()
                .set(value, LogicLevel::High)
                .unwrap()
                .pulse(sample, PulseCount::ONE)
                .unwrap()
                .finish()
                .unwrap()
        };
        let roomy = policy_with([100, 10000, 100, 1000, 100000]);
        for ready in [false, true] {
            let mut seed = c.spawn(roomy.clone());
            if ready {
                seed.apply(Transaction::initialize(
                    crate::time::Time::from_ticks(0),
                    seed.revision(),
                    make_init(false),
                ))
                .unwrap();
            }
            let apply = |m: &mut crate::Machine<()>| {
                if ready {
                    m.apply(Transaction::advance(
                        crate::time::Time::from_ticks(1),
                        m.revision(),
                        make_delta(),
                    ))
                } else {
                    m.apply(Transaction::initialize(
                        crate::time::Time::from_ticks(0),
                        m.revision(),
                        make_init(true),
                    ))
                }
            };
            let mut reference = c.spawn(roomy.clone());
            reference.store = seed.store.clone();
            let previous = reference
                .store
                .provenance
                .as_ref()
                .map_or(0, ProvenanceView::len);
            let result = apply(&mut reference).unwrap();
            let growth = (result.provenance().len() - previous) as u64;
            assert!(growth > 0);
            let events = result.output_events().len() as u64;
            assert_eq!(events, 1);
            for (index, consumed, budget) in [
                (
                    3,
                    events,
                    RuntimePolicyLimit::MaxEventsCreatedPerTransaction,
                ),
                (4, growth, RuntimePolicyLimit::MaxRequiredProvenanceGrowth),
            ] {
                for limit in [consumed - 1, consumed, consumed + 1] {
                    let mut limits = [100, 10000, 100, 1000, 100000];
                    limits[index] = limit;
                    let mut m = c.spawn(policy_with(limits));
                    // Install the same Ready reference state to isolate this transaction's budget boundary.
                    m.store = seed.store.clone();
                    let before = observe(&m);
                    match apply(&mut m) {
                        Err(failure) => {
                            assert!(limit < consumed);
                            assert_eq!(
                                failure.evidence(),
                                &RuntimeFailureEvidence::BudgetExceeded {
                                    budget,
                                    limit,
                                    consumed
                                }
                            );
                            assert_eq!(observe(&m), before);
                        }
                        Ok(result) => {
                            assert!(limit >= consumed);
                            assert_eq!(
                                m.inspect_sample_hold(node).unwrap().committed(),
                                LogicLevel::High
                            );
                            assert_eq!(result.output_events().len(), 1);
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn transport_due_batch_is_independent_of_event_storage_order() {
        let compiled = compiled_with_input(1, 2, 3);
        let node = NodeKey::from_u128(2);
        let first = PendingEvent::TransportDelay(PendingTransportDelay {
            key: PendingEventKey::from_serial(1),
            node,
            origin: crate::time::Time::from_ticks(3),
            deadline: crate::time::Time::from_ticks(8),
            target: LogicLevel::Low,
            revision: NetworkRevision::initial(),
            cause: CauseRef {
                scope: UNFINALIZED_PROVENANCE_SCOPE,
                ordinal: 1,
            },
        });
        let second = PendingEvent::TransportDelay(PendingTransportDelay {
            key: PendingEventKey::from_serial(2),
            node,
            origin: crate::time::Time::from_ticks(5),
            deadline: crate::time::Time::from_ticks(8),
            target: LogicLevel::High,
            revision: NetworkRevision::initial(),
            cause: CauseRef {
                scope: UNFINALIZED_PROVENANCE_SCOPE,
                ordinal: 2,
            },
        });
        let forward = aggregate_due(&compiled, vec![first, second]).unwrap();
        let reverse = aggregate_due(
            &compiled,
            vec![
                second,
                PendingEvent::TransportDelay(PendingTransportDelay {
                    key: PendingEventKey::from_serial(1),
                    node,
                    origin: crate::time::Time::from_ticks(3),
                    deadline: crate::time::Time::from_ticks(8),
                    target: LogicLevel::Low,
                    revision: NetworkRevision::initial(),
                    cause: CauseRef {
                        scope: UNFINALIZED_PROVENANCE_SCOPE,
                        ordinal: 1,
                    },
                }),
            ],
        )
        .unwrap();
        assert_eq!(forward.transport_targets, reverse.transport_targets);
        assert_eq!(
            forward.transport_targets.get(&node),
            Some(&LogicLevel::High)
        );
        assert_eq!(forward.transport_causes, reverse.transport_causes);
    }
}
