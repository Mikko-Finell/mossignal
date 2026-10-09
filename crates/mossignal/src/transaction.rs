//! Restricted initialization and ready-machine Level and Pulse transactions.

use crate::authored::{ConflictPolicy, EdgeObservation};
use crate::compile::{
    EvaluationCause, EvaluationFailure, FullEvaluation, LevelLatchConflict, PulseLatchConflict,
};
use crate::diagnostics::{
    BudgetEvidence, ConflictControls, ConflictEvidence, DiagnosticCode, DiagnosticEpisodeEvidence,
    DiagnosticOccurrence, DigestMismatchEvidence, InputSchemaEvidence, LifecycleEvidence,
    MigrationEvidence, NodeEvidence, OperationSubjectRef, ParameterEvidence, PendingEventEvidence,
    Problem, ProblemEvidence, ProvenanceEvidence, ReplayEvidence, Responsibility,
    RevisionMismatchEvidence, SemanticLossEvidence, Severity, StaleArtifactEvidence, SubjectRef,
    TimeEvidence, TimeOperation,
};
use crate::identity::{
    ExecutionStateDigest, InputSchemaFingerprint, NetworkFingerprint, ObservableStateDigest,
};
use crate::input::{InputDelta, InputSnapshot};
use crate::key::{AnyExternalInputKey, ExternalInputKey, ExternalOutputKey, NetworkKey, NodeKey};
use crate::machine::{
    ForecastState, Machine, MachineStatus, NetworkRevision, PendingEvent, PendingEventKey,
    PendingInertialDelay, PendingPeriodicBoundary, PendingPulseDelay, PendingTransportDelay,
    Schedule,
};
use crate::migration::{
    FinalizedPatch, MigrationFault, MigrationReport, MigrationSource, ReconfigurationPolicy,
    finalize, signal_semantics_version,
};
use crate::module::{NodeSubject, PulsePortSubject, QualifiedNodeRef};
use crate::patch::{InputValuationPlan, OutputBaselinePlan, PreparedPatch};
use crate::policy::{RuntimePolicy, RuntimePolicyId, RuntimePolicyLimit};
use crate::signal::{Level, LogicLevel, Pulse, PulseCount};
use crate::time::{ReactionStamp, Span, Time};
use core::fmt;
use core::marker::PhantomData;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

type ProvenanceScope = [u8; 32];
#[cfg(test)]
const UNFINALIZED_PROVENANCE_SCOPE: ProvenanceScope = [0; 32];
pub(crate) const MIGRATED_CANCELLATION_RULE: &str = "retained_cancellation";

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
    patch: Option<(PreparedPatch<D>, ReconfigurationPolicy)>,
    expected_execution: Option<ExecutionStateDigest>,
}

impl<D> Clone for Transaction<D> {
    fn clone(&self) -> Self {
        Self {
            at: self.at,
            expected_revision: self.expected_revision,
            kind: self.kind.clone(),
            patch: self.patch.clone(),
            expected_execution: self.expected_execution,
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
            patch: None,
            expected_execution: None,
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
            patch: None,
            expected_execution: None,
        }
    }

    /// Attaches one prepared replacement and the caller's loss policy.
    ///
    /// The input must already be bound to the prepared target schema. A base
    /// revision that differs from this transaction's expected revision is
    /// rejected before the transaction can be applied.
    pub fn with_patch(
        mut self,
        prepared: PreparedPatch<D>,
        policy: ReconfigurationPolicy,
    ) -> Result<Self, TransactionBuildFailure<D>> {
        if prepared.base_revision() != self.expected_revision {
            return Err(TransactionBuildFailure::BaseRevisionMismatch {
                network: prepared.network_key(),
                expected: self.expected_revision,
                actual: prepared.base_revision(),
                marker: PhantomData,
            });
        }
        if let Some(evidence) = target_binding_problem(&self.kind, &prepared) {
            return Err(TransactionBuildFailure::TargetSchemaMismatch {
                network: prepared.network_key(),
                evidence: Box::new(evidence),
                marker: PhantomData,
            });
        }
        self.patch = Some((prepared, policy));
        Ok(self)
    }

    /// Requires the machine's execution digest to match before any candidate work.
    #[must_use]
    pub fn expect_execution_state(mut self, digest: ExecutionStateDigest) -> Self {
        self.expected_execution = Some(digest);
        self
    }

    /// Returns whether this transaction carries a prepared topology replacement.
    #[must_use]
    pub const fn carries_patch(&self) -> bool {
        self.patch.is_some()
    }

    pub(crate) fn prepared_patch(&self) -> Option<&PreparedPatch<D>> {
        self.patch.as_ref().map(|(prepared, _)| prepared)
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
    TimeRegression {
        current_ticks: u64,
        requested_ticks: u64,
    },
    ReactionOrderOverflow {
        time_ticks: u64,
        previous_order: u64,
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
        reaction_order: u64,
        revision: NetworkRevision,
    },

    LevelLatchConflict {
        node: NodeSubject,
        policy: ConflictPolicy,
        previous: LogicLevel,
        set_level: LogicLevel,
        reset_level: LogicLevel,
        at_ticks: u64,
        reaction_order: u64,
        revision: NetworkRevision,
    },
    StaleExecutionState {
        expected: String,
        actual: String,
    },
    PatchBearingRecording,
    StalePreparedPatch {
        network: NetworkKey,
        evidence: StaleArtifactEvidence,
    },
    TargetInputSchemaMismatch {
        network: NetworkKey,
        evidence: InputSchemaEvidence,
    },
    StateMigrationRejected {
        evidence: MigrationEvidence,
    },
    PendingEventMigrationRejected {
        evidence: MigrationEvidence,
    },
    RequirePreserveFailed {
        evidence: MigrationEvidence,
    },
    EpisodeMigrationRejected {
        subject: SubjectRef,
        evidence: DiagnosticEpisodeEvidence,
    },
    ProvenanceMigrationRejected {
        subject: SubjectRef,
        evidence: ProvenanceEvidence,
    },
    AmbiguousEventMigration {
        subject: SubjectRef,
    },
    ConflictingMigratedTransitions {
        subject: SubjectRef,
        evidence: PendingEventEvidence,
    },
    StateLossRejected {
        evidence: SemanticLossEvidence,
    },
    ReconfigurationTimeOverflow {
        node: NodeSubject,
        origin_ticks: u64,
        delay_ticks: u64,
    },
    ReconfigurationBudgetExceeded {
        budget: RuntimePolicyLimit,
        limit: u64,
        consumed: u64,
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
            Self::TimeRegression {
                current_ticks,
                requested_ticks,
            } => ProblemEvidence::RuntimeTimeRegression {
                evidence: TimeEvidence {
                    owner: None,
                    operation: TimeOperation::TransactionAdvance,
                    left_ticks: *current_ticks,
                    right_ticks: *requested_ticks,
                },
                marker: PhantomData,
            },
            Self::ReactionOrderOverflow {
                time_ticks,
                previous_order,
            } => ProblemEvidence::RuntimeReactionOrderOverflow {
                evidence: TimeEvidence {
                    owner: None,
                    operation: TimeOperation::ReactionOrderIncrement,
                    left_ticks: *time_ticks,
                    right_ticks: *previous_order,
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
                reaction_order,
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
                            reaction_order: *reaction_order,
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
                reaction_order,
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
                            reaction_order: *reaction_order,
                            at_ticks: *at_ticks,
                            revision: *revision,
                        },
                        marker: PhantomData,
                    },
                );
            }
            Self::StaleExecutionState { expected, actual } => {
                ProblemEvidence::RuntimeStaleExecutionState {
                    evidence: DigestMismatchEvidence {
                        kind: "execution_state",
                        expected: expected.clone(),
                        actual: actual.clone(),
                        context: "machine".to_owned(),
                    },
                    marker: PhantomData,
                }
            }
            Self::PatchBearingRecording => {
                let mut evidence = ReplayEvidence::new();
                evidence.underlying_code = "patch".to_owned();
                ProblemEvidence::ReplayPatchPreparationDiverged {
                    evidence,
                    marker: PhantomData,
                }
            }
            Self::StalePreparedPatch { network, evidence } => {
                return Problem::new(
                    SubjectRef::Network(*network),
                    Vec::new(),
                    ProblemEvidence::ReconfigurationStalePreparedPatch {
                        evidence: evidence.clone(),
                        marker: PhantomData,
                    },
                );
            }
            Self::TargetInputSchemaMismatch { network, evidence } => {
                return Problem::new(
                    SubjectRef::Network(*network),
                    Vec::new(),
                    ProblemEvidence::ReconfigurationTargetInputSchemaMismatch {
                        evidence: *evidence,
                        marker: PhantomData,
                    },
                );
            }
            Self::StateMigrationRejected { evidence } => {
                return migration_problem(
                    evidence,
                    ProblemEvidence::ReconfigurationStateMigrationRejected {
                        evidence: evidence.clone(),
                        marker: PhantomData,
                    },
                );
            }
            Self::PendingEventMigrationRejected { evidence } => {
                return migration_problem(
                    evidence,
                    ProblemEvidence::ReconfigurationPendingEventMigrationRejected {
                        evidence: evidence.clone(),
                        marker: PhantomData,
                    },
                );
            }
            Self::RequirePreserveFailed { evidence } => {
                return migration_problem(
                    evidence,
                    ProblemEvidence::ReconfigurationRequirePreserveFailed {
                        evidence: evidence.clone(),
                        marker: PhantomData,
                    },
                );
            }
            Self::EpisodeMigrationRejected { subject, evidence } => {
                return Problem::new(
                    subject.clone(),
                    Vec::new(),
                    ProblemEvidence::ReconfigurationEpisodeMigrationRejected {
                        evidence: evidence.clone(),
                        marker: PhantomData,
                    },
                );
            }
            Self::ProvenanceMigrationRejected { subject, evidence } => {
                return Problem::new(
                    subject.clone(),
                    Vec::new(),
                    ProblemEvidence::ReconfigurationProvenanceMigrationRejected {
                        evidence: *evidence,
                        marker: PhantomData,
                    },
                );
            }
            Self::AmbiguousEventMigration { subject } => {
                return Problem::new(
                    subject.clone(),
                    Vec::new(),
                    ProblemEvidence::ReconfigurationAmbiguousEventMigration {
                        marker: PhantomData,
                    },
                );
            }
            Self::ConflictingMigratedTransitions { subject, evidence } => {
                return Problem::new(
                    subject.clone(),
                    Vec::new(),
                    ProblemEvidence::ReconfigurationConflictingMigratedTransitions {
                        evidence: evidence.clone(),
                        marker: PhantomData,
                    },
                );
            }
            Self::StateLossRejected { evidence } => {
                return Problem::new(
                    evidence.subject.clone(),
                    Vec::new(),
                    ProblemEvidence::ReconfigurationStateLossRejected {
                        evidence: evidence.clone(),
                        marker: PhantomData,
                    },
                );
            }
            Self::ReconfigurationTimeOverflow {
                node,
                origin_ticks,
                delay_ticks,
            } => ProblemEvidence::RuntimeTimeOverflow {
                evidence: TimeEvidence {
                    owner: Some(node_evidence(node)),
                    operation: TimeOperation::Reconfiguration,
                    left_ticks: *origin_ticks,
                    right_ticks: *delay_ticks,
                },
                marker: PhantomData,
            },
            Self::ReconfigurationBudgetExceeded {
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
        };
        Problem::new(primary, Vec::new(), evidence)
    }
}

fn migration_problem<D>(evidence: &MigrationEvidence, problem: ProblemEvidence<D>) -> Problem<D> {
    Problem::new(evidence.subject.clone(), Vec::new(), problem)
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

    /// Projects a finalization rejection into the closed reconfiguration family.
    #[must_use]
    pub fn reconfiguration(&self) -> Option<ReconfigurationFailure<D>> {
        Some(match self.evidence.as_ref() {
            RuntimeFailureEvidence::StalePreparedPatch { .. } => {
                ReconfigurationFailure::StalePreparedPatch(self.evidence.problem())
            }
            RuntimeFailureEvidence::TargetInputSchemaMismatch { .. } => {
                ReconfigurationFailure::TargetInputSchemaMismatch(self.evidence.problem())
            }
            RuntimeFailureEvidence::StateMigrationRejected { .. } => {
                ReconfigurationFailure::StateMigrationRejected(self.evidence.problem())
            }
            RuntimeFailureEvidence::PendingEventMigrationRejected { .. } => {
                ReconfigurationFailure::PendingEventMigrationRejected(self.evidence.problem())
            }
            RuntimeFailureEvidence::RequirePreserveFailed { .. } => {
                ReconfigurationFailure::RequirePreserveFailed(self.evidence.problem())
            }
            RuntimeFailureEvidence::EpisodeMigrationRejected { .. } => {
                ReconfigurationFailure::EpisodeMigrationRejected(self.evidence.problem())
            }
            RuntimeFailureEvidence::ProvenanceMigrationRejected { .. } => {
                ReconfigurationFailure::ProvenanceMigrationRejected(self.evidence.problem())
            }
            RuntimeFailureEvidence::AmbiguousEventMigration { .. } => {
                ReconfigurationFailure::AmbiguousEventMigration(self.evidence.problem())
            }
            RuntimeFailureEvidence::ConflictingMigratedTransitions { .. } => {
                ReconfigurationFailure::ConflictingMigratedTransitions(self.evidence.problem())
            }
            RuntimeFailureEvidence::StateLossRejected { .. } => {
                ReconfigurationFailure::StateLossRejected(self.evidence.problem())
            }
            RuntimeFailureEvidence::ReconfigurationTimeOverflow { .. } => {
                ReconfigurationFailure::TimeArithmeticFailure(self.evidence.problem())
            }
            RuntimeFailureEvidence::ReconfigurationBudgetExceeded { .. } => {
                ReconfigurationFailure::MigrationBudgetExceeded(self.evidence.problem())
            }
            _ => return None,
        })
    }

    pub(crate) fn rejected_patch_recording() -> Self {
        Self::new(RuntimeFailureEvidence::PatchBearingRecording)
    }
}

/// A closed finalization rejection owned by one runtime failure.
#[derive(Debug)]
#[non_exhaustive]
pub enum ReconfigurationFailure<D> {
    /// The prepared patch does not match the live machine.
    StalePreparedPatch(Problem<D>),
    /// Target-bound input does not match the prepared schema at application.
    TargetInputSchemaMismatch(Problem<D>),
    /// A state rule rejected its subject.
    StateMigrationRejected(Problem<D>),
    /// A pending-event rule rejected its subject.
    PendingEventMigrationRejected(Problem<D>),
    /// A require-preserve rule observed a lossy outcome.
    RequirePreserveFailed(Problem<D>),
    /// An episode rule rejected an active episode.
    EpisodeMigrationRejected(Problem<D>),
    /// A provenance rule could not be applied.
    ProvenanceMigrationRejected(Problem<D>),
    /// More than one event rule claimed one source owner.
    AmbiguousEventMigration(Problem<D>),
    /// Migrated transitions share one owner and deadline.
    ConflictingMigratedTransitions(Problem<D>),
    /// The caller policy rejected a realized semantic loss.
    StateLossRejected(Problem<D>),
    /// Checked time arithmetic failed during finalization.
    TimeArithmeticFailure(Problem<D>),
    /// A runtime budget failed during finalization.
    MigrationBudgetExceeded(Problem<D>),
}

impl<D> ReconfigurationFailure<D> {
    /// Returns the catalogue problem retained by this rejection.
    #[must_use]
    pub const fn problem(&self) -> &Problem<D> {
        match self {
            Self::StalePreparedPatch(problem)
            | Self::TargetInputSchemaMismatch(problem)
            | Self::StateMigrationRejected(problem)
            | Self::PendingEventMigrationRejected(problem)
            | Self::RequirePreserveFailed(problem)
            | Self::EpisodeMigrationRejected(problem)
            | Self::ProvenanceMigrationRejected(problem)
            | Self::AmbiguousEventMigration(problem)
            | Self::ConflictingMigratedTransitions(problem)
            | Self::StateLossRejected(problem)
            | Self::TimeArithmeticFailure(problem)
            | Self::MigrationBudgetExceeded(problem) => problem,
        }
    }

    /// Returns the catalogue code represented by this rejection.
    #[must_use]
    pub const fn code(&self) -> DiagnosticCode {
        self.problem().code()
    }
}

/// A rejection raised while attaching a patch, before application.
#[non_exhaustive]
pub enum TransactionBuildFailure<D> {
    /// The prepared base revision differs from the transaction's expected revision.
    BaseRevisionMismatch {
        /// Network the patch names.
        network: NetworkKey,
        /// Revision this transaction expects.
        expected: NetworkRevision,
        /// Revision the patch was prepared against.
        actual: NetworkRevision,
        marker: PhantomData<fn() -> D>,
    },
    /// The transaction input is not bound to the prepared target schema.
    TargetSchemaMismatch {
        /// Network the patch names.
        network: NetworkKey,
        /// Compared schema identities.
        evidence: Box<InputSchemaEvidence>,
        marker: PhantomData<fn() -> D>,
    },
}

impl<D> TransactionBuildFailure<D> {
    /// Returns the catalogue problem for this build rejection.
    #[must_use]
    pub fn problem(&self) -> Problem<D> {
        match self {
            Self::BaseRevisionMismatch {
                network,
                expected,
                actual,
                marker: _,
            } => Problem::new(
                SubjectRef::Network(*network),
                Vec::new(),
                ProblemEvidence::ReconfigurationBaseRevisionMismatch {
                    expected: *expected,
                    actual: *actual,
                    marker: PhantomData,
                },
            ),
            Self::TargetSchemaMismatch {
                network,
                evidence,
                marker: _,
            } => Problem::new(
                SubjectRef::Network(*network),
                Vec::new(),
                ProblemEvidence::ReconfigurationTargetInputSchemaMismatch {
                    evidence: **evidence,
                    marker: PhantomData,
                },
            ),
        }
    }

    /// Returns the catalogue code represented by this rejection.
    #[must_use]
    pub fn code(&self) -> DiagnosticCode {
        self.problem().code()
    }
}

impl<D> fmt::Debug for TransactionBuildFailure<D> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BaseRevisionMismatch {
                network,
                expected,
                actual,
                marker: _,
            } => formatter
                .debug_struct("BaseRevisionMismatch")
                .field("network", network)
                .field("expected", expected)
                .field("actual", actual)
                .finish(),
            Self::TargetSchemaMismatch {
                network,
                evidence,
                marker: _,
            } => formatter
                .debug_struct("TargetSchemaMismatch")
                .field("network", network)
                .field("evidence", evidence)
                .finish(),
        }
    }
}

impl<D> fmt::Display for TransactionBuildFailure<D> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "transaction build rejected with {}",
            self.code().as_str()
        )
    }
}

impl<D> std::error::Error for TransactionBuildFailure<D> {}

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
    pub(crate) scope: ProvenanceScope,
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
    TopologyChange {
        at: ReactionStamp<D>,
        revision: NetworkRevision,
        base: NetworkFingerprint,
        target: NetworkFingerprint,
        supporters: Vec<CauseRef>,
    },
    Migration {
        subject: ProvenanceSubject,
        rule: String,
        supporters: Vec<CauseRef>,
    },
    Checkpoint {
        fact: Vec<u8>,
        supporters: Vec<CauseRef>,
    },
    InitializationTransaction {
        at: ReactionStamp<D>,
        revision: NetworkRevision,
    },
    ReadyTransaction {
        at: ReactionStamp<D>,
        revision: NetworkRevision,
    },
    ExternalObservation {
        stamp: ReactionStamp<D>,
        input: ExternalInputKey<Level>,
        value: LogicLevel,
    },
    ExternalPulseObservation {
        stamp: ReactionStamp<D>,
        input: ExternalInputKey<Pulse>,
        count: PulseCount,
    },
    PendingPulseDelay {
        event: PendingEventKey,
        owner: NodeSubject,
        stimulus: ReactionStamp<D>,
        origin: Time<D>,
        deadline: Time<D>,
        count: PulseCount,
        revision: NetworkRevision,
        supporters: Vec<CauseRef>,
    },
    PendingTransportDelay {
        event: PendingEventKey,
        owner: NodeSubject,
        stimulus: ReactionStamp<D>,
        origin: Time<D>,
        deadline: Time<D>,
        target: LogicLevel,
        revision: NetworkRevision,
        supporters: Vec<CauseRef>,
    },
    PendingInertialDelay {
        event: PendingEventKey,
        owner: NodeSubject,
        stimulus: ReactionStamp<D>,
        origin: Time<D>,
        deadline: Time<D>,
        target: LogicLevel,
        revision: NetworkRevision,
        supporters: Vec<CauseRef>,
    },
    PendingPeriodicBoundary {
        event: PendingEventKey,
        owner: NodeSubject,
        stimulus: ReactionStamp<D>,
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
    fn translate_causes(&mut self, translate: impl Fn(CauseRef) -> CauseRef) {
        match self {
            Self::TopologyChange { supporters, .. }
            | Self::Migration { supporters, .. }
            | Self::Checkpoint { supporters, .. }
            | Self::PendingPulseDelay { supporters, .. }
            | Self::PendingTransportDelay { supporters, .. }
            | Self::PendingInertialDelay { supporters, .. }
            | Self::PendingPeriodicBoundary { supporters, .. }
            | Self::Derived { supporters, .. } => {
                for cause in supporters {
                    *cause = translate(*cause);
                }
            }
            Self::PulseDerived {
                supporters,
                contributions,
                ..
            }
            | Self::PulseControlledLevel {
                supporters,
                contributions,
                ..
            } => {
                for cause in supporters {
                    *cause = translate(*cause);
                }
                for contribution in contributions {
                    contribution.cause = translate(contribution.cause);
                }
            }
            Self::InitializationTransaction { .. }
            | Self::ReadyTransaction { .. }
            | Self::ExternalObservation { .. }
            | Self::ExternalPulseObservation { .. } => {}
        }
    }
    pub(crate) fn supporters(&self) -> &[CauseRef] {
        match self {
            Self::PendingPulseDelay { supporters, .. }
            | Self::PendingTransportDelay { supporters, .. }
            | Self::PendingInertialDelay { supporters, .. }
            | Self::PendingPeriodicBoundary { supporters, .. }
            | Self::Derived { supporters, .. }
            | Self::PulseDerived { supporters, .. }
            | Self::PulseControlledLevel { supporters, .. } => supporters,
            Self::Migration { supporters, .. } | Self::Checkpoint { supporters, .. } => supporters,
            Self::TopologyChange { supporters, .. } => supporters,
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
            Self::Migration { supporters, .. } | Self::Checkpoint { supporters, .. } => {
                supporters.reverse()
            }
            Self::TopologyChange { supporters, .. } => supporters.reverse(),
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
    /// The topology fact effective at this reaction; it is not a signal.
    TopologyChange {
        at: ReactionStamp<D>,
        revision: NetworkRevision,
        base: NetworkFingerprint,
        target: NetworkFingerprint,
        supporters: &'a [CauseRef],
    },
    /// A target fact established by a named migration rule and its source ancestry.
    Migration {
        subject: ProvenanceSubject,
        rule: &'a str,
        supporters: &'a [CauseRef],
    },
    /// An authoritative historical fact, independent of the installed topology.
    ///
    /// `fact` is canonical CBOR containing the original provenance record's
    /// stable subject, kind, time, revision, and content-addressed predecessor
    /// relations. Temporal facts also retain their complete event identity.
    /// `supporters` resolve the retained ancestry in this view.
    /// Initialization patches instead retain the base's declared state cells.
    Checkpoint {
        fact: &'a [u8],
        supporters: &'a [CauseRef],
    },
    InitializationTransaction {
        at: ReactionStamp<D>,
        revision: NetworkRevision,
    },
    ReadyTransaction {
        at: ReactionStamp<D>,
        revision: NetworkRevision,
    },
    ExternalObservation {
        stamp: ReactionStamp<D>,
        input: ExternalInputKey<Level>,
        value: LogicLevel,
    },
    ExternalPulseObservation {
        stamp: ReactionStamp<D>,
        input: ExternalInputKey<Pulse>,
        count: PulseCount,
    },
    PendingPulseDelay {
        event: PendingEventKey,
        owner: &'a NodeSubject,
        stimulus: ReactionStamp<D>,
        origin: Time<D>,
        deadline: Time<D>,
        count: PulseCount,
        revision: NetworkRevision,
        supporters: &'a [CauseRef],
    },
    PendingTransportDelay {
        event: PendingEventKey,
        owner: &'a NodeSubject,
        stimulus: ReactionStamp<D>,
        origin: Time<D>,
        deadline: Time<D>,
        target: LogicLevel,
        revision: NetworkRevision,
        supporters: &'a [CauseRef],
    },
    PendingInertialDelay {
        event: PendingEventKey,
        owner: &'a NodeSubject,
        stimulus: ReactionStamp<D>,
        origin: Time<D>,
        deadline: Time<D>,
        target: LogicLevel,
        revision: NetworkRevision,
        supporters: &'a [CauseRef],
    },
    PendingPeriodicBoundary {
        event: PendingEventKey,
        owner: &'a NodeSubject,
        stimulus: ReactionStamp<D>,
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
    records: Arc<crate::causal_store::Records<D>>,
}

impl<D> fmt::Debug for ProvenanceView<D> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProvenanceView")
            .field("scope", &self.scope)
            .field("records", &self.records.len())
            .finish()
    }
}
impl<D> PartialEq for ProvenanceView<D> {
    fn eq(&self, other: &Self) -> bool {
        self.scope == other.scope
            && (Arc::ptr_eq(&self.records, &other.records)
                || (self.len() == other.len()
                    && (0..self.len()).all(|position| {
                        self.records.cause(position) == other.records.cause(position)
                    })))
    }
}
impl<D> Eq for ProvenanceView<D> {}

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
    declared_edges: &'a BTreeSet<NodeKey>,
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
        let Some(position) = self.records.position(cause) else {
            return Err(if cause.scope == self.scope {
                CauseLookupFailure::InvalidCause {
                    scope: self.scope,
                    ordinal: cause.ordinal,
                }
            } else {
                CauseLookupFailure::ForeignCause {
                    expected_scope: self.scope,
                    actual_scope: cause.scope,
                    ordinal: cause.ordinal,
                }
            });
        };
        let record = &self.records[position];
        Ok(match record {
            ProvenanceRecord::TopologyChange {
                at,
                revision,
                base,
                target,
                supporters,
            } => CauseInspection::TopologyChange {
                at: *at,
                revision: *revision,
                base: *base,
                target: *target,
                supporters,
            },
            ProvenanceRecord::Migration {
                subject,
                rule,
                supporters,
            } => CauseInspection::Migration {
                subject: subject.clone(),
                rule,
                supporters,
            },
            ProvenanceRecord::Checkpoint { fact, supporters } => {
                CauseInspection::Checkpoint { fact, supporters }
            }
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
            ProvenanceRecord::ExternalObservation {
                input,
                value,
                stamp,
            } => CauseInspection::ExternalObservation {
                stamp: *stamp,
                input: *input,
                value: *value,
            },
            ProvenanceRecord::ExternalPulseObservation {
                input,
                count,
                stamp,
            } => CauseInspection::ExternalPulseObservation {
                stamp: *stamp,
                input: *input,
                count: *count,
            },
            ProvenanceRecord::PendingPulseDelay {
                event,
                owner,
                stimulus,
                origin,
                deadline,
                count,
                revision,
                supporters,
            } => CauseInspection::PendingPulseDelay {
                event: *event,
                owner,
                stimulus: *stimulus,
                origin: *origin,
                deadline: *deadline,
                count: *count,
                revision: *revision,
                supporters,
            },
            ProvenanceRecord::PendingTransportDelay {
                event,
                owner,
                stimulus,
                origin,
                deadline,
                target,
                revision,
                supporters,
            } => CauseInspection::PendingTransportDelay {
                event: *event,
                owner,
                stimulus: *stimulus,
                origin: *origin,
                deadline: *deadline,
                target: *target,
                revision: *revision,
                supporters,
            },
            ProvenanceRecord::PendingInertialDelay {
                event,
                owner,
                stimulus,
                origin,
                deadline,
                target,
                revision,
                supporters,
            } => CauseInspection::PendingInertialDelay {
                event: *event,
                owner,
                stimulus: *stimulus,
                origin: *origin,
                deadline: *deadline,
                target: *target,
                revision: *revision,
                supporters,
            },
            ProvenanceRecord::PendingPeriodicBoundary {
                event,
                owner,
                stimulus,
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
                stimulus: *stimulus,
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

    pub(crate) fn records(&self) -> &crate::causal_store::Records<D> {
        &self.records
    }

    pub(crate) const fn scope(&self) -> ProvenanceScope {
        self.scope
    }

    pub(crate) fn restored(
        compiled: &crate::CompiledNetwork<D>,
        input: Vec<ProvenanceRecord<D>>,
    ) -> Self {
        let scope = crate::causal_store::fresh_scope(None);
        let mut records = crate::causal_store::Records::new(scope);
        for record in &input {
            records.push(remap_record(record, scope));
        }
        let view = Self {
            scope,
            records: Arc::new(records),
        };
        crate::state_digest::freeze_provenance(compiled, &view);
        view
    }

    pub(crate) fn resolve_ordinal(&self, cause: CauseRef) -> usize {
        match self.records.position(cause) {
            Some(position) => position,
            None => panic!("committed cause must resolve in its owning provenance view"),
        }
    }

    pub(crate) fn owned_roots(&self, roots: &[CauseRef]) -> Self {
        Self {
            scope: self.scope,
            records: Arc::new(self.records.owned_closure(roots)),
        }
    }

    pub(crate) fn include(&mut self, other: &Self) {
        Arc::make_mut(&mut self.records).include(&other.records);
    }

    #[cfg(test)]
    pub(crate) fn duplicate_allocation_for_test(
        &mut self,
        compiled: &crate::CompiledNetwork<D>,
        cause: CauseRef,
    ) -> CauseRef {
        let scope = crate::causal_store::fresh_scope(Some(self.scope));
        let mut records = self.records.fork(scope);
        let duplicate = records.push(remap_record(
            &self.records[self.resolve_ordinal(cause)],
            scope,
        ));
        self.scope = scope;
        self.records = Arc::new(records);
        crate::state_digest::freeze_provenance(compiled, self);
        duplicate
    }

    #[cfg(test)]
    pub(crate) fn reverse_unordered_supporters(&mut self)
    where
        D: Clone,
    {
        Arc::make_mut(&mut self.records).reverse_supporters();
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
                stimulus: pending.stimulus,
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
                stimulus: pending.stimulus,
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
                stimulus: pending.stimulus,
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
                stimulus: pending.stimulus,
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
        stamp: ReactionStamp<D>,
        cause: CauseRef,
        revision: NetworkRevision,
    },
    LevelChanged {
        output: ExternalOutputKey<Level>,
        from: LogicLevel,
        to: LogicLevel,
        stamp: ReactionStamp<D>,
        cause: CauseRef,
        revision: NetworkRevision,
    },
    Pulsed {
        output: ExternalOutputKey<Pulse>,
        count: PulseCount,
        stamp: ReactionStamp<D>,
        cause: CauseRef,
        revision: NetworkRevision,
    },
}

impl<D> OutputEvent<D> {
    /// Returns the producing machine-local occurrence.
    #[must_use]
    pub const fn stamp(&self) -> ReactionStamp<D> {
        match self {
            Self::LevelEstablished { stamp: at, .. }
            | Self::LevelChanged { stamp: at, .. }
            | Self::Pulsed { stamp: at, .. } => *at,
        }
    }
    /// Returns exact physical time derived from the producing stamp.
    #[must_use]
    pub const fn at(&self) -> Time<D> {
        self.stamp().time()
    }
}

impl<D> fmt::Debug for OutputEvent<D> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LevelEstablished {
                output,
                value,
                stamp: at,
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
                stamp: at,
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
                stamp: at,
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
    processed_reactions: Vec<ReactionStamp<D>>,
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
    migration: Option<MigrationReport<D>>,
}

impl<D> TransactionResult<D> {
    /// Returns the logical time requested by the applied transaction.
    #[must_use]
    pub const fn requested_time(&self) -> Time<D> {
        self.requested_time
    }

    /// Returns every candidate reaction in chronological occurrence order.
    #[must_use]
    pub fn processed_reactions(&self) -> &[ReactionStamp<D>] {
        &self.processed_reactions
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

    /// Returns the migration report when this transaction committed a patch.
    #[must_use]
    pub const fn migration(&self) -> Option<&MigrationReport<D>> {
        self.migration.as_ref()
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
            .field(
                "migration",
                &self
                    .migration
                    .as_ref()
                    .map(|report| report.target_revision()),
            )
            .finish()
    }
}

/// Identities of the machine a forecast was evaluated against.
///
/// `execution_digest` is that machine's execution-state digest before the
/// forecast. It is the forecast freshness identity, not the successor digest.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct ForecastBasis<D> {
    /// Topology revision of the borrowed machine.
    pub revision: NetworkRevision,
    /// Execution-state digest of the borrowed machine before the forecast.
    pub execution_digest: ExecutionStateDigest,
    /// Requested logical time of the consumed transaction.
    pub requested_time: Time<D>,
    /// Runtime policy identity of the borrowed machine.
    pub runtime_policy_id: RuntimePolicyId,
}

impl<D> fmt::Debug for ForecastBasis<D> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ForecastBasis")
            .field("revision", &self.revision)
            .field("execution_digest", &self.execution_digest)
            .field("requested_time", &self.requested_time)
            .field("runtime_policy_id", &self.runtime_policy_id)
            .finish()
    }
}

/// Owned hypothetical result of one forecast.
pub struct ForecastResult<D> {
    result: TransactionResult<D>,
    state: ForecastState<D>,
    basis: ForecastBasis<D>,
}

impl<D> ForecastResult<D> {
    /// Returns the transaction result the shared transition produced.
    #[must_use]
    pub const fn result(&self) -> &TransactionResult<D> {
        &self.result
    }

    /// Returns the unpublished candidate.
    #[must_use]
    pub const fn state(&self) -> &ForecastState<D> {
        &self.state
    }

    /// Returns the pre-forecast basis.
    #[must_use]
    pub const fn basis(&self) -> &ForecastBasis<D> {
        &self.basis
    }

    /// Splits the forecast into its result, candidate, and basis.
    #[must_use]
    pub fn into_parts(self) -> (TransactionResult<D>, ForecastState<D>, ForecastBasis<D>) {
        (self.result, self.state, self.basis)
    }
}

impl<D> fmt::Debug for ForecastResult<D> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ForecastResult")
            .field("result", &self.result)
            .field("state", &self.state)
            .field("basis", &self.basis)
            .finish()
    }
}

/// One evaluated successor, owned privately until every adapter check succeeds.
pub(crate) struct StagedTransaction<D> {
    successor: Machine<D>,
    result: TransactionResult<D>,
}

impl<D> StagedTransaction<D> {
    pub(crate) fn result(&self) -> &TransactionResult<D> {
        &self.result
    }

    pub(crate) fn publish(self, machine: &mut Machine<D>) -> TransactionResult<D> {
        *machine = self.successor;
        self.result
    }

    pub(crate) fn into_parts(self) -> (Machine<D>, TransactionResult<D>) {
        (self.successor, self.result)
    }
}

impl<D> Machine<D> {
    /// Applies an owned transaction atomically.
    pub fn apply(
        &mut self,
        transaction: Transaction<D>,
    ) -> Result<TransactionResult<D>, RuntimeFailure<D>> {
        Ok(self.stage(transaction)?.publish(self))
    }

    pub(crate) fn stage(
        &self,
        transaction: Transaction<D>,
    ) -> Result<StagedTransaction<D>, RuntimeFailure<D>> {
        // SPEC: docs/specs/contracts/live-bindings.yaml "prepublication-projection"
        // Core apply, forecast and bound apply evaluate once on the same private successor.
        let mut successor = self.duplicate_for_staging();
        let result = successor.evaluate_transaction(transaction)?;
        #[cfg(test)]
        causal_preparation_fault(&self.policy, crate::causal_work::Fault::Result)?;
        #[cfg(test)]
        causal_preparation_fault(&self.policy, crate::causal_work::Fault::Projection)?;
        #[cfg(test)]
        crate::state_digest_reference::assert_machine(&successor);
        Ok(StagedTransaction { successor, result })
    }

    fn evaluate_transaction(
        &mut self,
        transaction: Transaction<D>,
    ) -> Result<TransactionResult<D>, RuntimeFailure<D>> {
        let Transaction {
            at,
            expected_revision,
            kind,
            patch,
            expected_execution,
        } = transaction;
        match kind {
            TransactionKind::Initialize(input) => {
                self.apply_initialization(at, expected_revision, input, patch, expected_execution)
            }
            TransactionKind::Advance(input) => {
                self.apply_advance(at, expected_revision, input, patch, expected_execution)
            }
        }
    }

    /// Forecasts one owned transaction without publishing it.
    ///
    /// The original machine stays unchanged on success and failure. A later
    /// explicit `apply` of the same semantic transaction is what can publish
    /// that result, and only while this machine's preconditions still hold.
    pub fn forecast(
        &self,
        transaction: Transaction<D>,
    ) -> Result<ForecastResult<D>, RuntimeFailure<D>> {
        // SPEC: docs/specs/contracts/transaction-forecast.yaml "owned-forecast-result"
        // The basis digest is taken before apply, so it cannot become the successor digest.
        let basis = ForecastBasis {
            revision: self.revision(),
            execution_digest: self.execution_state_digest(),
            requested_time: transaction.requested_time(),
            runtime_policy_id: self.runtime_policy_id(),
        };
        let staged = self.stage(transaction)?;
        Ok(ForecastResult {
            result: staged.result,
            state: ForecastState::from_candidate(staged.successor),
            basis,
        })
    }

    fn apply_initialization(
        &mut self,
        at: Time<D>,
        expected_revision: NetworkRevision,
        input: InputSnapshot<D>,
        patch: Option<(PreparedPatch<D>, ReconfigurationPolicy)>,
        expected_execution: Option<ExecutionStateDigest>,
    ) -> Result<TransactionResult<D>, RuntimeFailure<D>> {
        admit_initialization(
            self,
            expected_revision,
            &input,
            patch.as_ref(),
            expected_execution,
        )?;
        let stamp = ReactionStamp::from_parts(at, 0);
        let mut installed_network = None;
        let mut migration_report = None;
        let mut revision = self.store.revision;
        let mut migrated_edges = None;
        let mut migrated_stored = None;
        if let Some((prepared, policy)) = patch {
            let limits = self.policy.clone();
            let source = store_source(self);
            let finalized =
                finalize(&prepared, policy, stamp, &source, &limits).map_err(migration_failure)?;
            migrated_edges = Some(finalized.edge_observations);
            migrated_stored = Some(finalized.stored_levels);
            revision = finalized.revision;
            migration_report = Some(finalized.report);
            installed_network = Some(finalized.compiled);
        }
        let network = match installed_network.as_ref() {
            Some(network) => network,
            None => &self.compiled,
        };
        let edge_observations = match migrated_edges.as_deref() {
            Some(edges) => edges,
            None => self.store.edge_observations.as_slice(),
        };
        let stored_levels = match migrated_stored.as_deref() {
            Some(stored) => stored,
            None => self.store.stored_levels.as_slice(),
        };

        let patched = installed_network.is_some();
        enforce_outer_reaction_budgets::<D>(&self.policy, network, 1)
            .map_err(|failure| reconfiguration_phase(failure, patched))?;
        let (levels, pulses) = input.into_parts();
        let evaluation = evaluate_reaction::<D>(
            network,
            &levels,
            &pulses,
            edge_observations,
            stored_levels,
            stamp,
            revision,
            &BTreeMap::new(),
        )?;
        let occurrences = pulse_latch_occurrences(network, stamp, revision, &evaluation);
        let mut active_episodes = self.store.active_episodes.clone();
        let mut diagnostic_episode_changes = Vec::new();
        let mut built = build_initialization_provenance(
            network,
            revision,
            stamp,
            &levels,
            &pulses,
            &evaluation,
            migration_report
                .as_ref()
                .map(|report| (report, crate::state_digest::declared_state_checkpoint(self))),
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
                compiled: network,
                pending: &mut pending_events,
                next_serial: &mut next_pending_event_serial,
                created_events: &mut created_pending_events,
            },
            stamp,
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
        )
        .map_err(|failure| reconfiguration_phase(failure, patched))?;
        finalize_provenance_build(&mut built, network);
        #[cfg(test)]
        causal_preparation_fault(&self.policy, crate::causal_work::Fault::Provenance)
            .map_err(|failure| reconfiguration_phase(failure, patched))?;
        let standard_history = crate::standard::stateful::observe_reaction(
            network,
            &evaluation,
            &built.operation_causes,
            &BTreeMap::new(),
            |cause| remap_cause(cause, built.provenance.scope),
        );
        let standard_causes = standard_history
            .iter()
            .map(|(module, history)| (module.clone(), history.causal_roles()))
            .collect();
        remap_cause_map(&mut inertial_cancellation_causes, built.provenance.scope);
        remap_cause_map(&mut periodic_anchor_causes, built.provenance.scope);
        remap_cause_map(&mut periodic_cancellation_causes, built.provenance.scope);
        remap_pending_causes(&mut pending_events, built.provenance.scope);
        reconcile_level_episodes(
            network,
            stamp,
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
        )
        .map_err(|failure| reconfiguration_phase(failure, patched))?;

        let output_events = initialization_events(
            stamp,
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
        )
        .map_err(|failure| reconfiguration_phase(failure, patched))?;
        let schedule = schedule_from_pending(&pending_events);
        let before_execution_digest = self.execution_state_digest();
        let before_revision = self.store.revision;
        let provenance = built.provenance.clone();
        Ok(publish_success(
            self,
            PublicationReport {
                processed_reactions: vec![stamp],
                before_execution_digest,
                requested_time: at,
                before_revision,
                after_revision: revision,
                migration: migration_report,
                output_events,
                occurrences,
                diagnostic_episode_changes,
                schedule,
                provenance,
            },
            PublishedCandidate {
                stamp,
                standard_history,
                standard_causes,
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
                installed_network,
                revision,
            },
        ))
    }

    fn apply_advance(
        &mut self,
        at: Time<D>,
        expected_revision: NetworkRevision,
        input: InputDelta<D>,
        patch: Option<(PreparedPatch<D>, ReconfigurationPolicy)>,
        expected_execution: Option<ExecutionStateDigest>,
    ) -> Result<TransactionResult<D>, RuntimeFailure<D>> {
        admit_advance(
            self,
            at,
            expected_revision,
            &input,
            patch.as_ref(),
            expected_execution,
        )?;

        let mut last_reaction = self.last_reaction();
        let mut processed_reactions = Vec::new();
        let mut revision = self.store.revision;
        let (explicit_levels, pulses) = input.into_parts();
        let mut standard_history = self.store.standard_history.clone();
        let mut standard_causes = self.store.standard_causes.clone();
        let mut levels = self.store.external_levels.clone();
        let mut pending_events = self.store.pending_events.clone();
        let mut next_pending_event_serial = self.store.next_pending_event_serial;
        let mut edge_observations = self.store.edge_observations.clone();
        let mut stored_levels = self.store.stored_levels.clone();
        let mut operation_levels = self.store.operation_levels.clone();
        let mut current_operation_causes = self.store.operation_causes.clone();
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
        let mut provenance_growth = 0_usize;
        let mut output_events = Vec::new();
        let mut occurrences = Vec::new();
        let mut active_episodes = self.store.active_episodes.clone();
        let mut diagnostic_episode_changes = Vec::new();
        let mut created_pending_events = 0_u64;
        let mut reaction_count = 0_u64;
        let empty_levels = BTreeMap::new();
        let empty_pulses = BTreeMap::new();

        // SPEC: docs/specs/contracts/atomic-topology-replacement.yaml "effective-time-order"
        // Deadlines strictly earlier than the requested time run on the installed topology.
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
            let stamp = allocate_reaction(&mut last_reaction, deadline)?;
            processed_reactions.push(stamp);
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
                .map_err(|failure| evaluation_failure(&self.compiled, failure, stamp, revision))?;
            occurrences.extend(pulse_latch_occurrences(
                &self.compiled,
                stamp,
                revision,
                &internal,
            ));
            reaction_count = reaction_count.saturating_add(1);
            enforce_outer_reaction_budgets::<D>(&self.policy, &self.compiled, reaction_count)?;

            let before_reaction_records = provenance.len();
            let mut built = build_ready_provenance(
                &self.compiled,
                revision,
                stamp,
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
                &BTreeSet::new(),
                None,
            );
            remap_pending_causes(&mut pending_events, built.provenance.scope);
            remap_output_event_causes(&mut output_events, built.provenance.scope);
            let mut reaction_events = changed_events(
                stamp,
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
            operation_levels = internal.operation_levels.clone();
            output_baselines = internal.external_outputs.clone();
            schedule_pulse_delays(
                PulseDelayScheduling {
                    compiled: &self.compiled,
                    pending: &mut pending_events,
                    next_serial: &mut next_pending_event_serial,
                    created_events: &mut created_pending_events,
                },
                stamp,
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
            finalize_provenance_build(&mut built, &self.compiled);
            #[cfg(test)]
            causal_preparation_fault(&self.policy, crate::causal_work::Fault::Provenance)?;
            standard_history = crate::standard::stateful::observe_reaction(
                &self.compiled,
                &internal,
                &built.operation_causes,
                &standard_causes,
                |cause| remap_cause(cause, built.provenance.scope),
            );
            let updated_causes = standard_history
                .iter()
                .map(|(module, history)| (module.clone(), history.causal_roles()))
                .collect();
            standard_causes = updated_causes;
            remap_cause_map(&mut inertial_cancellation_causes, built.provenance.scope);
            remap_cause_map(&mut periodic_anchor_causes, built.provenance.scope);
            remap_cause_map(&mut periodic_cancellation_causes, built.provenance.scope);
            remap_pending_causes(&mut pending_events, built.provenance.scope);
            remap_output_event_causes(&mut output_events, built.provenance.scope);
            reconcile_level_episodes(
                &self.compiled,
                stamp,
                revision,
                &internal,
                &built,
                &mut active_episodes,
                &mut diagnostic_episode_changes,
            );
            remap_episode_changes(&mut diagnostic_episode_changes, built.provenance.scope);
            current_operation_causes = built.operation_causes.clone();
            input_causes = built.input_causes;
            output_causes = built.output_causes;
            edge_observation_causes = built.edge_observation_causes;
            toggle_inversion_causes = built.toggle_inversion_causes;
            establishment_causes = built.establishment_causes;
            transport_transition_causes = built.transport_output_transitions;
            provenance_growth = provenance_growth.saturating_add(
                built
                    .provenance
                    .len()
                    .saturating_sub(before_reaction_records),
            );
            provenance = built.provenance;
            enforce_created_event_budget::<D>(
                &self.policy,
                created_pending_events,
                output_events
                    .len()
                    .saturating_add(occurrences.len())
                    .saturating_add(diagnostic_episode_changes.len()),
            )?;
            enforce_provenance_growth::<D>(&self.policy, provenance_growth)?;
        }

        let stamp = allocate_reaction(&mut last_reaction, at)?;
        processed_reactions.push(stamp);
        let mut installed_network = None;
        let mut migration_report = None;
        let mut output_plans = BTreeMap::new();
        let mut declared_edges = BTreeSet::new();
        let mut patch_cause = None;
        if let Some((prepared, policy)) = patch {
            let source = MigrationSource {
                compiled: &self.compiled,
                edge_observations: &edge_observations,
                stored_levels: &stored_levels,
                operation_levels: &operation_levels,
                external_levels: &levels,
                output_baselines: &output_baselines,
                pending_events: &pending_events,
                next_serial: next_pending_event_serial,
                periodic_anchors: &periodic_anchors,
                episodes: &active_episodes,
                input_causes: &input_causes,
                output_causes: &output_causes,
                edge_observation_causes: &edge_observation_causes,
                toggle_inversion_causes: &toggle_inversion_causes,
                establishment_causes: &establishment_causes,
                transport_transition_causes: &transport_transition_causes,
                inertial_cancellation_causes: &inertial_cancellation_causes,
                periodic_anchor_causes: &periodic_anchor_causes,
                periodic_cancellation_causes: &periodic_cancellation_causes,
            };
            let mut finalized = finalize(&prepared, policy, stamp, &source, &self.policy)
                .map_err(migration_failure)?;
            let mut source_roots = current_operation_causes.clone();
            source_roots.extend(input_causes.values().copied());
            source_roots.extend(output_causes.values().copied());
            for causes in [
                &edge_observation_causes,
                &toggle_inversion_causes,
                &establishment_causes,
                &transport_transition_causes,
                &inertial_cancellation_causes,
                &periodic_anchor_causes,
                &periodic_cancellation_causes,
            ] {
                source_roots.extend(causes.values().copied());
            }
            source_roots.extend(
                pending_events
                    .values()
                    .flatten()
                    .map(|event| event.identity().5),
            );
            source_roots.extend(
                standard_causes
                    .values()
                    .flat_map(|facts| facts.retained_causes()),
            );
            for episode in active_episodes.values() {
                provenance.include(episode.provenance());
                source_roots.push(episode.cause());
            }
            source_roots.sort();
            source_roots.dedup();
            #[cfg(test)]
            assert_eq!(
                source_roots,
                crate::state_digest_reference::migration_source_roots(
                    &source,
                    &current_operation_causes,
                    &standard_causes,
                ),
                "typed SOURCE roots after earlier deadlines, before target settlement"
            );
            let result_roots: Vec<_> = output_events
                .iter()
                .map(output_event_cause)
                .chain(
                    diagnostic_episode_changes
                        .iter()
                        .map(crate::DiagnosticEpisodeChange::cause),
                )
                .collect();
            let migrated = checkpoint_migration(
                &self.compiled,
                &provenance,
                stamp,
                &mut finalized,
                &source_roots,
                &result_roots,
            );
            provenance_growth = provenance_growth.saturating_add(migrated.growth);
            for event in &mut output_events {
                translate_output_event(event, &migrated.translated);
            }
            for change in &mut diagnostic_episode_changes {
                change.remap_cause(translated_cause(&migrated.translated, change.cause()));
            }
            for facts in standard_causes.values_mut() {
                facts.translate_causes(|cause| translated_cause(&migrated.translated, cause));
            }
            for episode in finalized.episodes.values_mut() {
                episode.translate_cause(&migrated.view, |cause| {
                    translated_cause(&migrated.translated, cause)
                });
            }
            patch_cause = Some(migrated.patch);
            for (condition, episode) in &active_episodes {
                let owner = node_subject_ref(condition.owner());
                let preserved = finalized.report.episodes().iter().any(|record| {
                    record.subject() == &owner
                        && matches!(
                            record.outcome(),
                            crate::EpisodeOutcome::Preserved | crate::EpisodeOutcome::Transformed
                        )
                });
                if !preserved && !finalized.episodes.contains_key(condition) {
                    diagnostic_episode_changes.push(
                        crate::episode::DiagnosticEpisodeChange::migration_end(
                            episode,
                            crate::DiagnosticEpisodeChangeKind::Terminated,
                            stamp,
                            migrated.patch,
                        ),
                    );
                }
            }
            provenance = migrated.view;
            edge_observations = finalized.edge_observations;
            stored_levels = finalized.stored_levels;
            levels = finalized.external_levels;
            output_baselines = finalized.output_baselines;
            pending_events = finalized.pending_events;
            created_pending_events = created_pending_events.saturating_add(
                finalized
                    .next_serial
                    .saturating_sub(next_pending_event_serial),
            );
            next_pending_event_serial = finalized.next_serial;
            periodic_anchors = finalized.periodic_anchors;
            active_episodes = finalized.episodes;
            input_causes = finalized.input_causes;
            output_causes = finalized.output_causes;
            edge_observation_causes = finalized.edge_observation_causes;
            toggle_inversion_causes = finalized.toggle_inversion_causes;
            establishment_causes = finalized.establishment_causes;
            transport_transition_causes = finalized.transport_transition_causes;
            inertial_cancellation_causes = finalized.inertial_cancellation_causes;
            periodic_anchor_causes = finalized.periodic_anchor_causes;
            periodic_cancellation_causes = finalized.periodic_cancellation_causes;
            output_plans = finalized.output_plans;
            declared_edges = finalized.declared_edges;
            migration_report = Some(finalized.report);
            revision = finalized.revision;
            installed_network = Some(finalized.compiled);
            standard_history.retain(|module, _| {
                installed_network
                    .as_ref()
                    .is_some_and(|network| network.module(module).is_some())
            });
        }
        let network = match installed_network.as_ref() {
            Some(network) => network,
            None => &self.compiled,
        };
        let patched = installed_network.is_some();

        // Target-time external levels become authoritative only after every
        // strictly earlier internal deadline has completed on candidate state.
        levels.extend(explicit_levels.iter().map(|(key, value)| (*key, *value)));
        let due = pending_events
            .remove(&at)
            .map(|batch| aggregate_due::<D>(network, batch))
            .transpose()
            .map_err(|failure| reconfiguration_phase(failure, patched))?
            .unwrap_or_default();
        let evaluation = network
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
            .map_err(|failure| {
                reconfiguration_phase(
                    evaluation_failure(network, failure, stamp, revision),
                    patched,
                )
            })?;
        occurrences.extend(pulse_latch_occurrences(
            network,
            stamp,
            revision,
            &evaluation,
        ));
        reaction_count = reaction_count.saturating_add(1);
        // Earlier reactions used the old graph; charge their actual operation count.
        enforce_budget::<D>(
            &self.policy,
            RuntimePolicyLimit::MaxInternalReactions,
            reaction_count,
        )
        .and_then(|()| {
            enforce_budget::<D>(
                &self.policy,
                RuntimePolicyLimit::MaxEvaluatedOperations,
                reaction_count
                    .saturating_sub(1)
                    .saturating_mul(count_as_u64(self.compiled.operation_count()))
                    .saturating_add(count_as_u64(network.operation_count())),
            )
        })
        .map_err(|failure| reconfiguration_phase(failure, patched))?;

        let before_final_records = provenance.len();
        let mut built = build_ready_provenance(
            network,
            revision,
            stamp,
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
            &declared_edges,
            patch_cause,
        );
        remap_pending_causes(&mut pending_events, built.provenance.scope);
        remap_output_event_causes(&mut output_events, built.provenance.scope);
        let mut final_events = if patched {
            planned_level_events(
                stamp,
                revision,
                &output_plans,
                LevelOutputValues {
                    previous: &output_baselines,
                    settled: &evaluation.external_outputs,
                },
                &built.output_causes,
                &evaluation.pulse_outputs,
                &built.pulse_output_causes,
            )
        } else {
            changed_events(
                stamp,
                revision,
                &output_baselines,
                &evaluation.external_outputs,
                &built.output_causes,
                &evaluation.pulse_outputs,
                &built.pulse_output_causes,
            )
        };
        output_events.append(&mut final_events);
        schedule_pulse_delays(
            PulseDelayScheduling {
                compiled: network,
                pending: &mut pending_events,
                next_serial: &mut next_pending_event_serial,
                created_events: &mut created_pending_events,
            },
            stamp,
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
        )
        .map_err(|failure| reconfiguration_phase(failure, patched))?;
        finalize_provenance_build(&mut built, network);
        #[cfg(test)]
        causal_preparation_fault(&self.policy, crate::causal_work::Fault::Provenance)
            .map_err(|failure| reconfiguration_phase(failure, patched))?;
        standard_history = crate::standard::stateful::observe_reaction(
            network,
            &evaluation,
            &built.operation_causes,
            &standard_causes,
            |cause| remap_cause(cause, built.provenance.scope),
        );
        let updated_causes = standard_history
            .iter()
            .map(|(module, history)| (module.clone(), history.causal_roles()))
            .collect();
        standard_causes = updated_causes;
        remap_cause_map(&mut inertial_cancellation_causes, built.provenance.scope);
        remap_cause_map(&mut periodic_anchor_causes, built.provenance.scope);
        remap_cause_map(&mut periodic_cancellation_causes, built.provenance.scope);
        remap_pending_causes(&mut pending_events, built.provenance.scope);
        remap_output_event_causes(&mut output_events, built.provenance.scope);
        reconcile_level_episodes(
            network,
            stamp,
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
        )
        .map_err(|failure| reconfiguration_phase(failure, patched))?;
        enforce_provenance_growth::<D>(
            &self.policy,
            provenance_growth
                .saturating_add(built.provenance.len().saturating_sub(before_final_records)),
        )
        .map_err(|failure| reconfiguration_phase(failure, patched))?;
        let schedule = schedule_from_pending(&pending_events);
        let before_execution_digest = self.execution_state_digest();
        let before_revision = self.store.revision;
        let provenance = built.provenance.clone();
        Ok(publish_success(
            self,
            PublicationReport {
                processed_reactions,
                before_execution_digest,
                requested_time: at,
                before_revision,
                after_revision: revision,
                migration: migration_report,
                output_events,
                occurrences,
                diagnostic_episode_changes,
                schedule,
                provenance,
            },
            PublishedCandidate {
                stamp,
                standard_history,
                standard_causes,
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
                installed_network,
                revision,
            },
        ))
    }
}

// SPEC: docs/specs/contracts/ordered-reactions.yaml "atomic-order-and-resource-failure"
// Allocation lives only in the outer candidate; rejection consumes no live occurrence.
fn allocate_reaction<D>(
    last: &mut Option<ReactionStamp<D>>,
    at: Time<D>,
) -> Result<ReactionStamp<D>, RuntimeFailure<D>> {
    let order = match *last {
        Some(previous) if previous.time() == at => {
            previous.order().checked_add(1).ok_or_else(|| {
                RuntimeFailure::new(RuntimeFailureEvidence::ReactionOrderOverflow {
                    time_ticks: at.ticks(),
                    previous_order: previous.order(),
                })
            })?
        }
        Some(previous) if previous.time() > at => {
            return Err(RuntimeFailure::new(
                RuntimeFailureEvidence::TimeRegression {
                    current_ticks: previous.time().ticks(),
                    requested_ticks: at.ticks(),
                },
            ));
        }
        _ => 0,
    };
    let stamp = ReactionStamp::from_parts(at, order);
    *last = Some(stamp);
    Ok(stamp)
}

fn admit_initialization<D>(
    machine: &Machine<D>,
    expected_revision: NetworkRevision,
    input: &InputSnapshot<D>,
    patch: Option<&(PreparedPatch<D>, ReconfigurationPolicy)>,
    expected_execution: Option<ExecutionStateDigest>,
) -> Result<(), RuntimeFailure<D>> {
    if machine.is_initialized() {
        return Err(RuntimeFailure::new(
            RuntimeFailureEvidence::AlreadyInitialized,
        ));
    }
    check_expected_execution(machine, expected_execution)?;
    check_expected_revision(machine, expected_revision)?;
    if let Some((prepared, _)) = patch {
        check_patch_freshness(machine, prepared)?;
        runtime_schema_mismatch(prepared, snapshot_binding_problem(input, prepared))?;
    } else {
        validate_snapshot_binding(machine.compiled(), input)?;
    }
    Ok(())
}

fn admit_advance<D>(
    machine: &Machine<D>,
    at: Time<D>,
    expected_revision: NetworkRevision,
    input: &InputDelta<D>,
    patch: Option<&(PreparedPatch<D>, ReconfigurationPolicy)>,
    expected_execution: Option<ExecutionStateDigest>,
) -> Result<(), RuntimeFailure<D>> {
    let MachineStatus::Ready { now } = machine.status() else {
        return Err(RuntimeFailure::new(
            RuntimeFailureEvidence::DeltaBeforeInitialization,
        ));
    };
    check_expected_execution(machine, expected_execution)?;
    check_expected_revision(machine, expected_revision)?;
    if let Some((prepared, _)) = patch {
        check_patch_freshness(machine, prepared)?;
        runtime_schema_mismatch(prepared, delta_binding_problem(input, prepared))?;
    } else {
        validate_delta_binding(machine.compiled(), input)?;
    }
    if at < now {
        return Err(RuntimeFailure::new(
            RuntimeFailureEvidence::TimeRegression {
                current_ticks: now.ticks(),
                requested_ticks: at.ticks(),
            },
        ));
    }
    Ok(())
}

fn check_expected_execution<D>(
    machine: &Machine<D>,
    expected: Option<ExecutionStateDigest>,
) -> Result<(), RuntimeFailure<D>> {
    let Some(expected) = expected else {
        return Ok(());
    };
    let actual = machine.execution_state_digest();
    if expected == actual {
        return Ok(());
    }
    Err(RuntimeFailure::new(
        RuntimeFailureEvidence::StaleExecutionState {
            expected: expected.to_string(),
            actual: actual.to_string(),
        },
    ))
}

fn check_expected_revision<D>(
    machine: &Machine<D>,
    expected: NetworkRevision,
) -> Result<(), RuntimeFailure<D>> {
    let actual = machine.revision();
    if expected == actual {
        return Ok(());
    }
    Err(RuntimeFailure::new(RuntimeFailureEvidence::StaleRevision {
        expected,
        actual,
    }))
}

fn check_patch_freshness<D>(
    machine: &Machine<D>,
    prepared: &PreparedPatch<D>,
) -> Result<(), RuntimeFailure<D>> {
    let compiled = machine.compiled();
    let target = prepared.resulting_compiled();
    let fresh = compiled.network_key() == prepared.network_key()
        && machine.revision() == prepared.base_revision()
        && compiled.fingerprint() == prepared.base_fingerprint()
        && signal_semantics_version(compiled) == signal_semantics_version(target)
        && compiled.time_domain_id() == target.time_domain_id();
    if fresh {
        return Ok(());
    }
    Err(RuntimeFailure::new(
        RuntimeFailureEvidence::StalePreparedPatch {
            network: prepared.network_key(),
            evidence: StaleArtifactEvidence {
                expected_network: format!("{:032x}", prepared.network_key().as_u128()),
                actual_network: format!("{:032x}", compiled.network_key().as_u128()),
                expected_revision: prepared.base_revision().value(),
                actual_revision: machine.revision().value(),
                expected_fingerprint: prepared.base_fingerprint().to_string(),
                actual_fingerprint: compiled.fingerprint().to_string(),
                expected_time_domain: target.time_domain_id().to_string(),
                actual_time_domain: compiled.time_domain_id().to_string(),
            },
        },
    ))
}

fn target_binding_problem<D>(
    kind: &TransactionKind<D>,
    prepared: &PreparedPatch<D>,
) -> Option<InputSchemaEvidence> {
    match kind {
        TransactionKind::Initialize(input) => snapshot_binding_problem(input, prepared),
        TransactionKind::Advance(input) => delta_binding_problem(input, prepared),
    }
}

fn snapshot_binding_problem<D>(
    input: &InputSnapshot<D>,
    prepared: &PreparedPatch<D>,
) -> Option<InputSchemaEvidence> {
    let target = prepared.resulting_compiled();
    let missing = target
        .external_level_inputs()
        .iter()
        .any(|key| !input.levels.contains_key(key));
    if missing
        || target_identity_differs(
            target,
            prepared.resulting_fingerprint(),
            input.network_key(),
            input.network_fingerprint(),
            input.input_schema_fingerprint(),
        )
    {
        Some(target_schema_evidence(
            target,
            prepared.resulting_fingerprint(),
            input.network_key(),
            input.network_fingerprint(),
            input.input_schema_fingerprint(),
        ))
    } else {
        None
    }
}

fn delta_binding_problem<D>(
    input: &InputDelta<D>,
    prepared: &PreparedPatch<D>,
) -> Option<InputSchemaEvidence> {
    let target = prepared.resulting_compiled();
    let missing = prepared.static_plan().external_inputs().iter().any(|plan| {
        plan.valuation() == InputValuationPlan::Establish
            && matches!(
                plan.target(),
                Some(AnyExternalInputKey::Level(key)) if !input.levels.contains_key(&key)
            )
    });
    if missing
        || target_identity_differs(
            target,
            prepared.resulting_fingerprint(),
            input.network_key(),
            input.network_fingerprint(),
            input.input_schema_fingerprint(),
        )
    {
        Some(target_schema_evidence(
            target,
            prepared.resulting_fingerprint(),
            input.network_key(),
            input.network_fingerprint(),
            input.input_schema_fingerprint(),
        ))
    } else {
        None
    }
}

fn target_identity_differs<D>(
    target: &crate::CompiledNetwork<D>,
    resulting_fingerprint: NetworkFingerprint,
    actual_network: NetworkKey,
    actual_fingerprint: NetworkFingerprint,
    actual_schema: InputSchemaFingerprint,
) -> bool {
    actual_network != target.network_key()
        || actual_fingerprint != resulting_fingerprint
        || actual_schema != target.input_schema_fingerprint()
}

fn target_schema_evidence<D>(
    target: &crate::CompiledNetwork<D>,
    resulting_fingerprint: NetworkFingerprint,
    actual_network: NetworkKey,
    actual_fingerprint: NetworkFingerprint,
    actual_schema: InputSchemaFingerprint,
) -> InputSchemaEvidence {
    InputSchemaEvidence {
        expected_network: Some(target.network_key()),
        actual_network: Some(actual_network),
        expected_fingerprint: Some(resulting_fingerprint),
        actual_fingerprint: Some(actual_fingerprint),
        expected_schema: Some(target.input_schema_fingerprint()),
        actual_schema: Some(actual_schema),
    }
}

fn runtime_schema_mismatch<D>(
    prepared: &PreparedPatch<D>,
    evidence: Option<InputSchemaEvidence>,
) -> Result<(), RuntimeFailure<D>> {
    match evidence {
        Some(evidence) => Err(RuntimeFailure::new(
            RuntimeFailureEvidence::TargetInputSchemaMismatch {
                network: prepared.network_key(),
                evidence,
            },
        )),
        None => Ok(()),
    }
}

fn store_source<D>(machine: &Machine<D>) -> MigrationSource<'_, D> {
    let store = &machine.store;
    MigrationSource {
        compiled: machine.compiled(),
        edge_observations: &store.edge_observations,
        stored_levels: &store.stored_levels,
        operation_levels: &store.operation_levels,
        external_levels: &store.external_levels,
        output_baselines: &store.output_baselines,
        pending_events: &store.pending_events,
        next_serial: store.next_pending_event_serial,
        periodic_anchors: &store.periodic_anchors,
        episodes: &store.active_episodes,
        input_causes: &store.input_causes,
        output_causes: &store.output_causes,
        edge_observation_causes: &store.edge_observation_causes,
        toggle_inversion_causes: &store.toggle_inversion_causes,
        establishment_causes: &store.establishment_causes,
        transport_transition_causes: &store.transport_transition_causes,
        inertial_cancellation_causes: &store.inertial_cancellation_causes,
        periodic_anchor_causes: &store.periodic_anchor_causes,
        periodic_cancellation_causes: &store.periodic_cancellation_causes,
    }
}

fn migration_failure<D>(fault: MigrationFault) -> RuntimeFailure<D> {
    let evidence = match fault {
        MigrationFault::State {
            subject,
            fact,
            rule,
        } => RuntimeFailureEvidence::StateMigrationRejected {
            evidence: MigrationEvidence {
                subject: *subject,
                fact,
                rule,
            },
        },
        MigrationFault::Pending {
            subject,
            fact,
            rule,
        } => RuntimeFailureEvidence::PendingEventMigrationRejected {
            evidence: MigrationEvidence {
                subject: *subject,
                fact,
                rule,
            },
        },
        MigrationFault::RequirePreserve {
            subject,
            fact,
            rule,
        } => RuntimeFailureEvidence::RequirePreserveFailed {
            evidence: MigrationEvidence {
                subject: *subject,
                fact,
                rule,
            },
        },
        MigrationFault::Episode { subject, evidence } => {
            RuntimeFailureEvidence::EpisodeMigrationRejected {
                subject: *subject,
                evidence: *evidence,
            }
        }
        MigrationFault::Provenance { subject } => {
            RuntimeFailureEvidence::ProvenanceMigrationRejected {
                subject: *subject,
                evidence: ProvenanceEvidence {
                    expected_scope: [0; 32],
                    actual_scope: [0; 32],
                    ordinal: 0,
                },
            }
        }
        MigrationFault::Ambiguous { subject } => {
            RuntimeFailureEvidence::AmbiguousEventMigration { subject: *subject }
        }
        MigrationFault::Conflict { subject, evidence } => {
            RuntimeFailureEvidence::ConflictingMigratedTransitions {
                subject: *subject,
                evidence: *evidence,
            }
        }
        MigrationFault::Loss { evidence } => RuntimeFailureEvidence::StateLossRejected {
            evidence: *evidence,
        },
        MigrationFault::Time {
            node,
            origin_ticks,
            delay_ticks,
        } => RuntimeFailureEvidence::ReconfigurationTimeOverflow {
            node,
            origin_ticks,
            delay_ticks,
        },
        MigrationFault::Budget {
            budget,
            limit,
            consumed,
        } => RuntimeFailureEvidence::ReconfigurationBudgetExceeded {
            budget,
            limit,
            consumed,
        },
    };
    RuntimeFailure::new(evidence)
}

fn reconfiguration_phase<D>(failure: RuntimeFailure<D>, patched: bool) -> RuntimeFailure<D> {
    if !patched {
        return failure;
    }
    let evidence = match failure.evidence() {
        RuntimeFailureEvidence::TimeOverflow {
            node,
            origin_ticks,
            delay_ticks,
        }
        | RuntimeFailureEvidence::TransportTimeOverflow {
            node,
            origin_ticks,
            delay_ticks,
        }
        | RuntimeFailureEvidence::InertialTimeOverflow {
            node,
            origin_ticks,
            delay_ticks,
        } => RuntimeFailureEvidence::ReconfigurationTimeOverflow {
            node: node.clone(),
            origin_ticks: *origin_ticks,
            delay_ticks: *delay_ticks,
        },
        RuntimeFailureEvidence::PeriodicTimeOverflow {
            node,
            origin_ticks,
            period_ticks,
        } => RuntimeFailureEvidence::ReconfigurationTimeOverflow {
            node: node.clone(),
            origin_ticks: *origin_ticks,
            delay_ticks: *period_ticks,
        },
        RuntimeFailureEvidence::BudgetExceeded {
            budget,
            limit,
            consumed,
        } => RuntimeFailureEvidence::ReconfigurationBudgetExceeded {
            budget: *budget,
            limit: *limit,
            consumed: *consumed,
        },
        _ => return failure,
    };
    RuntimeFailure::new(evidence)
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
    at: ReactionStamp<D>,
    revision: NetworkRevision,
    periodic_anchors: &BTreeMap<NodeKey, crate::machine::PeriodicPhase<D>>,
) -> Result<FullEvaluation, RuntimeFailure<D>> {
    match compiled.evaluate_reaction_with_state(
        levels,
        pulses,
        previous_edge_observations,
        previous_stored_levels,
        at.time(),
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
    at: ReactionStamp<D>,
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
    at: ReactionStamp<D>,
    revision: NetworkRevision,
) -> RuntimeFailureEvidence {
    RuntimeFailureEvidence::PulseLatchConflict {
        node: compiled.node_subject(conflict.node),
        policy: conflict.policy,
        previous: conflict.previous,
        set_count: conflict.set_count,
        reset_count: conflict.reset_count,
        reaction_order: at.order(),
        at_ticks: at.time().ticks(),
        revision,
    }
}

fn level_latch_failure<D>(
    compiled: &crate::CompiledNetwork<D>,
    conflict: LevelLatchConflict,
    at: ReactionStamp<D>,
    revision: NetworkRevision,
) -> RuntimeFailureEvidence {
    RuntimeFailureEvidence::LevelLatchConflict {
        node: compiled.node_subject(conflict.node),
        policy: conflict.policy,
        previous: conflict.previous,
        set_level: conflict.set_level,
        reset_level: conflict.reset_level,
        reaction_order: at.order(),
        at_ticks: at.time().ticks(),
        revision,
    }
}

fn pulse_latch_occurrences<D>(
    compiled: &crate::CompiledNetwork<D>,
    at: ReactionStamp<D>,
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
                        reaction_order: at.order(),
                        at_ticks: at.time().ticks(),
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
    at: ReactionStamp<D>,
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
                        reaction_order: at.order(),
                        at_ticks: at.time().ticks(),
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
    transport_origins: BTreeMap<NodeKey, (u64, u64)>,
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
                let origin = (event.stimulus.time().ticks(), event.stimulus.order());
                if due
                    .transport_origins
                    .get(&event.node)
                    .is_none_or(|previous| origin > *previous)
                {
                    due.transport_targets.insert(event.node, event.target);
                    due.transport_origins.insert(event.node, origin);
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
    stamp: ReactionStamp<D>,
    revision: NetworkRevision,
    evaluation: &FullEvaluation,
    proposal_causes: &BTreeMap<NodeKey, CauseRef>,
    transport_proposal_causes: &BTreeMap<NodeKey, CauseRef>,
    inertial_proposal_causes: &BTreeMap<NodeKey, CauseRef>,
    periodic_proposal_causes: &BTreeMap<NodeKey, CauseRef>,
    cancellation_causes: &mut BTreeMap<NodeKey, CauseRef>,
    periodic_anchors: &mut BTreeMap<NodeKey, crate::machine::PeriodicPhase<D>>,
    periodic_anchor_causes: &mut BTreeMap<NodeKey, CauseRef>,
    periodic_cancellation_causes: &mut BTreeMap<NodeKey, CauseRef>,
    provenance: &mut ProvenanceView<D>,
    policy: &RuntimePolicy,
) -> Result<(), RuntimeFailure<D>> {
    let origin = stamp.time();
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
                    stimulus: stamp,
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
                    stimulus: stamp,
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
                    stimulus: stamp,
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
                // SPEC: docs/specs/contracts/ordered-reactions.yaml "singular-inertial-and-once-only-phase"
                // An explicit disabled settlement consumes this boundary for the retained phase.
                if let Some(phase) = periodic_anchors.get_mut(&proposal.node) {
                    if origin >= phase.anchor
                        && (origin.ticks() - phase.anchor.ticks()) % proposal.period_ticks == 0
                    {
                        phase.settled = Some(origin);
                    }
                }
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
                    periodic_anchors.insert(
                        proposal.node,
                        crate::machine::PeriodicPhase {
                            anchor: origin,
                            origin: stamp,
                            settled: Some(origin),
                        },
                    );
                    periodic_anchor_causes.insert(proposal.node, scheduling_cause);
                }
                let anchor = match periodic_anchors.get(&proposal.node).copied() {
                    Some(phase) => phase.anchor,
                    None => panic!("enabled Periodic proposal must retain a phase anchor"),
                };
                let ordinal = if let Some(due) = proposal.due_ordinal {
                    due.checked_add(1)
                } else if origin < anchor {
                    // A preserved boundary may be the future reference of the target cadence.
                    Some(0)
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
                    stimulus: stamp,
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
    logical_records_appended: usize,
) -> Result<(), RuntimeFailure<D>> {
    enforce_budget::<D>(
        policy,
        RuntimePolicyLimit::MaxRequiredProvenanceGrowth,
        count_as_u64(logical_records_appended),
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

fn output_event_cause<D>(event: &OutputEvent<D>) -> CauseRef {
    match event {
        OutputEvent::LevelEstablished { cause, .. }
        | OutputEvent::LevelChanged { cause, .. }
        | OutputEvent::Pulsed { cause, .. } => *cause,
    }
}
fn translate_output_event<D>(event: &mut OutputEvent<D>, mapping: &BTreeMap<CauseRef, CauseRef>) {
    let cause = match event {
        OutputEvent::LevelEstablished { cause, .. }
        | OutputEvent::LevelChanged { cause, .. }
        | OutputEvent::Pulsed { cause, .. } => cause,
    };
    *cause = translated_cause(mapping, *cause);
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
    stamp: ReactionStamp<D>,
    standard_history:
        BTreeMap<crate::QualifiedModuleRef, crate::standard::stateful::StandardHistory>,
    standard_causes: BTreeMap<crate::QualifiedModuleRef, crate::standard::stateful::StandardCauses>,
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
    periodic_anchors: BTreeMap<NodeKey, crate::machine::PeriodicPhase<D>>,
    periodic_anchor_causes: BTreeMap<NodeKey, CauseRef>,
    periodic_cancellation_causes: BTreeMap<NodeKey, CauseRef>,
    active_episodes: crate::episode::ActiveEpisodes<D>,
    pending_events: BTreeMap<Time<D>, Vec<PendingEvent<D>>>,
    next_pending_event_serial: u64,
    installed_network: Option<crate::CompiledNetwork<D>>,
    revision: NetworkRevision,
}

struct PublicationReport<D> {
    before_execution_digest: ExecutionStateDigest,
    requested_time: Time<D>,
    processed_reactions: Vec<ReactionStamp<D>>,
    before_revision: NetworkRevision,
    after_revision: NetworkRevision,
    migration: Option<MigrationReport<D>>,
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
    let mut roots: Vec<_> = report
        .output_events
        .iter()
        .map(|event| match event {
            OutputEvent::LevelEstablished { cause, .. }
            | OutputEvent::LevelChanged { cause, .. }
            | OutputEvent::Pulsed { cause, .. } => *cause,
        })
        .chain(
            report
                .diagnostic_episode_changes
                .iter()
                .map(crate::DiagnosticEpisodeChange::cause),
        )
        .collect();
    roots.extend(candidate.operation_causes.iter().copied());
    roots.extend(candidate.input_causes.values().copied());
    roots.extend(candidate.output_causes.values().copied());
    roots.extend(
        candidate
            .standard_history
            .values()
            .flat_map(|history| history.retained_causes()),
    );
    for causes in [
        &candidate.edge_observation_causes,
        &candidate.toggle_inversion_causes,
        &candidate.establishment_causes,
        &candidate.transport_transition_causes,
        &candidate.inertial_cancellation_causes,
        &candidate.periodic_anchor_causes,
        &candidate.periodic_cancellation_causes,
    ] {
        roots.extend(causes.values().copied());
    }
    roots.extend(
        candidate
            .pending_events
            .values()
            .flatten()
            .map(|event| event.identity().5),
    );
    let provenance = report.provenance.owned_roots(&roots);
    publish_candidate(machine, candidate);
    TransactionResult {
        requested_time: report.requested_time,
        processed_reactions: report.processed_reactions,
        before_revision: report.before_revision,
        after_revision: report.after_revision,
        before_execution_digest: report.before_execution_digest,
        after_execution_digest: machine.execution_state_digest(),
        after_observable_digest: machine.observable_state_digest(),
        output_events: report.output_events,
        occurrences: report.occurrences,
        diagnostic_episode_changes: report.diagnostic_episode_changes,
        schedule: report.schedule,
        provenance,
        migration: report.migration,
    }
}

fn publish_candidate<D>(machine: &mut Machine<D>, published: PublishedCandidate<D>) {
    let PublishedCandidate {
        stamp,
        standard_history,
        standard_causes,
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
        installed_network,
        revision,
    } = published;
    // SPEC: docs/specs/processor_and_runtime_architecture.md §50 "Reference execution strategy"
    // Fallible effects are complete; finish only the privately owned successor here.
    if let Some(compiled) = installed_network {
        machine.compiled = compiled;
    }
    let candidate = &mut machine.store;
    candidate.revision = revision;
    candidate.standard_history = standard_history;
    candidate.standard_causes = standard_causes;
    candidate.status = MachineStatus::Ready { now: at };
    candidate.last_reaction = Some(stamp);
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
    // SPEC: docs/specs/contracts/provenance-retention.yaml "current-machine-root-manifest"
    // Artifact owners are independent; collection never truncates required ancestry.
    let roots = crate::causal_roots::machine(machine);
    let Some(mut view) = machine.store.provenance.take() else {
        panic!("published ready state must retain causal ownership");
    };
    for episode in machine.store.active_episodes.values() {
        view.include(episode.provenance());
    }
    machine.store.provenance = Some(view.owned_roots(&roots));
}

fn count_as_u64(count: usize) -> u64 {
    u64::try_from(count).unwrap_or(u64::MAX)
}

fn enforce_budget<D>(
    policy: &RuntimePolicy,
    budget: RuntimePolicyLimit,
    consumed: u64,
) -> Result<(), RuntimeFailure<D>> {
    #[cfg(test)]
    if budget == RuntimePolicyLimit::MaxRequiredProvenanceGrowth {
        crate::causal_work::update(|work| work.logical_growth = consumed);
    }
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

fn build_initialization_provenance<D>(
    compiled: &crate::CompiledNetwork<D>,
    revision: NetworkRevision,
    at: ReactionStamp<D>,
    levels: &BTreeMap<ExternalInputKey<Level>, LogicLevel>,
    pulses: &BTreeMap<ExternalInputKey<Pulse>, PulseCount>,
    evaluation: &FullEvaluation,
    migration: Option<(&MigrationReport<D>, Vec<u8>)>,
) -> ProvenanceBuild<D> {
    let scope = crate::causal_store::fresh_scope(None);
    let mut records = crate::causal_store::Records::new(scope);
    let ordinary_transaction = push_record(
        scope,
        &mut records,
        ProvenanceRecord::InitializationTransaction { at, revision },
    );
    let transaction_cause = if let Some((report, fact)) = migration {
        let patch = push_record(
            scope,
            &mut records,
            ProvenanceRecord::TopologyChange {
                at,
                revision,
                base: report.base_fingerprint(),
                target: report.target_fingerprint(),
                supporters: Vec::new(),
            },
        );
        push_record(
            scope,
            &mut records,
            ProvenanceRecord::Checkpoint {
                fact,
                supporters: vec![ordinary_transaction, patch],
            },
        )
    } else {
        ordinary_transaction
    };
    let input_causes = levels
        .iter()
        .map(|(input, value)| {
            let cause = push_record(
                scope,
                &mut records,
                ProvenanceRecord::ExternalObservation {
                    stamp: at,
                    input: *input,
                    value: *value,
                },
            );
            (*input, cause)
        })
        .collect::<BTreeMap<_, _>>();
    // SPEC: docs/specs/contracts/replay-artifacts.yaml "embedded-transaction"
    // Zero occurrences have no separate causal fact, matching persisted input omission.
    let pulse_input_causes = pulses
        .iter()
        .filter(|(_, count)| count.is_positive())
        .map(|(input, count)| {
            let cause = push_record(
                scope,
                &mut records,
                ProvenanceRecord::ExternalPulseObservation {
                    stamp: at,
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

struct MigrationProvenance<D> {
    view: ProvenanceView<D>,
    translated: BTreeMap<CauseRef, CauseRef>,
    patch: CauseRef,
    growth: usize,
}
fn translated_cause(mapping: &BTreeMap<CauseRef, CauseRef>, cause: CauseRef) -> CauseRef {
    match mapping.get(&cause).copied() {
        Some(cause) => cause,
        None => panic!(
            "every translated root and predecessor must belong to the selected source closure"
        ),
    }
}

fn checkpoint_migration<D>(
    source: &crate::CompiledNetwork<D>,
    previous: &ProvenanceView<D>,
    at: ReactionStamp<D>,
    finalized: &mut FinalizedPatch<D>,
    source_roots: &[CauseRef],
    result_roots: &[CauseRef],
) -> MigrationProvenance<D> {
    // SPEC: docs/specs/contracts/atomic-topology-replacement.yaml "revision-provenance-and-episodes"
    // Historical facts use their source canonical encoding, so removed subjects and
    // module slots cannot be interpreted against the target topology on restoration.
    let scope = crate::causal_store::fresh_scope(None);
    let mut roots = source_roots.to_vec();
    roots.extend_from_slice(result_roots);
    let selected = previous.owned_roots(&roots);
    let facts = crate::state_digest::checkpoint_facts(source, &selected);
    let mut records = crate::causal_store::Records::new(scope);
    let mut translated = BTreeMap::new();
    for (position, (record, fact)) in selected.records.iter().zip(facts).enumerate() {
        let supporters = record
            .predecessor_causes()
            .into_iter()
            .map(|cause| translated_cause(&translated, cause))
            .collect();
        let record = match record {
            ProvenanceRecord::Checkpoint { .. } => {
                translate_record(record, |cause| translated_cause(&translated, cause))
            }
            _ => ProvenanceRecord::Checkpoint { fact, supporters },
        };
        let cause = push_record(scope, &mut records, record);
        translated.insert(selected.records.cause(position), cause);
    }
    let replacement_count = records.len();
    // SPEC: docs/specs/contracts/atomic-topology-replacement.yaml "current-source-topology-supporters"
    // The supporter set uses canonical content, never private allocation multiplicity.
    let mut supporter_content = BTreeMap::new();
    for cause in source_roots {
        let position = previous.resolve_ordinal(*cause);
        let Some(content) = previous.records.canonical(position).get() else {
            panic!("source facts must be frozen before migration");
        };
        supporter_content
            .entry(content.digest)
            .or_insert_with(|| translated_cause(&translated, *cause));
    }
    let patch = push_record(
        scope,
        &mut records,
        ProvenanceRecord::TopologyChange {
            at,
            revision: finalized.revision,
            base: source.fingerprint(),
            target: finalized.compiled.fingerprint(),
            supporters: supporter_content.into_values().collect(),
        },
    );
    #[cfg(test)]
    crate::state_digest_reference::assert_migration(
        source,
        previous,
        source_roots,
        result_roots,
        &records,
        &translated,
        patch,
    );
    for (input, value) in &finalized.external_levels {
        let cause = push_record(
            scope,
            &mut records,
            ProvenanceRecord::ExternalObservation {
                stamp: at,
                input: *input,
                value: *value,
            },
        );
        finalized.input_causes.insert(*input, cause);
    }
    for (output, cause) in &mut finalized.output_causes {
        *cause = push_record(
            scope,
            &mut records,
            ProvenanceRecord::Migration {
                subject: ProvenanceSubject::ExternalOutput(*output),
                rule: "output_baseline".to_owned(),
                supporters: vec![translated_cause(&translated, *cause), patch],
            },
        );
    }
    for (causes, rule_override) in [
        (&mut finalized.edge_observation_causes, None),
        (&mut finalized.toggle_inversion_causes, None),
        (&mut finalized.establishment_causes, None),
        (&mut finalized.transport_transition_causes, None),
        (
            &mut finalized.inertial_cancellation_causes,
            Some(MIGRATED_CANCELLATION_RULE),
        ),
        (&mut finalized.periodic_anchor_causes, None),
        (
            &mut finalized.periodic_cancellation_causes,
            Some(MIGRATED_CANCELLATION_RULE),
        ),
    ] {
        for (node, cause) in causes {
            let subject = finalized.compiled.node_subject(*node);
            // SPEC: docs/specs/contracts/atomic-topology-replacement.yaml "committed-snapshot-and-later-replay"
            // Checkpointing pending ancestry must retain a cancellation root's role.
            let rule = rule_override.unwrap_or_else(|| {
                finalized
                    .report
                    .states()
                    .iter()
                    .find(|state| match state.subject() {
                        SubjectRef::Node(key) => subject == NodeSubject::Node(*key),
                        SubjectRef::QualifiedNode(key) => {
                            subject == NodeSubject::Qualified(key.clone())
                        }
                        _ => false,
                    })
                    .map(|state| state.fact())
                    .unwrap_or("preserve")
            });
            *cause = push_record(
                scope,
                &mut records,
                ProvenanceRecord::Migration {
                    subject: provenance_subject(&finalized.compiled, *node),
                    rule: rule.to_owned(),
                    supporters: vec![translated_cause(&translated, *cause), patch],
                },
            );
        }
    }
    // SPEC: docs/specs/contracts/periodic.yaml "focused-inspection-and-provenance"
    // Reanchoring a fresh disabled timer creates a phase without a prior anchor cause.
    for node in finalized.periodic_anchors.keys() {
        if let std::collections::btree_map::Entry::Vacant(entry) =
            finalized.periodic_anchor_causes.entry(*node)
        {
            entry.insert(push_record(
                scope,
                &mut records,
                ProvenanceRecord::Migration {
                    subject: provenance_subject(&finalized.compiled, *node),
                    rule: "reanchor_at_patch_time".to_owned(),
                    supporters: vec![patch],
                },
            ));
        }
    }
    for event in finalized.pending_events.values_mut().flatten() {
        let (key, node, origin, deadline, revision, previous_cause) = event.identity();
        let owner = finalized.compiled.node_subject(node);
        let rule = match finalized
            .report
            .events()
            .iter()
            .find(|record| record.event() == Some(key))
        {
            Some(record) => record.rule(),
            None => panic!("every migrated pending obligation must have its report record"),
        };
        // SPEC: docs/specs/contracts/atomic-topology-replacement.yaml "revision-provenance-and-episodes"
        // The scheduling fact retains its source ancestry and names the rule changing timing.
        let migration = push_record(
            scope,
            &mut records,
            ProvenanceRecord::Migration {
                subject: provenance_subject(&finalized.compiled, node),
                rule: rule.to_owned(),
                supporters: vec![translated_cause(&translated, previous_cause), patch],
            },
        );
        let supporters = vec![migration];
        let record = match event {
            PendingEvent::PulseDelay(event) => ProvenanceRecord::PendingPulseDelay {
                stimulus: event.stimulus,
                event: key,
                owner,
                origin,
                deadline,
                count: event.count,
                revision,
                supporters,
            },
            PendingEvent::TransportDelay(event) => ProvenanceRecord::PendingTransportDelay {
                stimulus: event.stimulus,
                event: key,
                owner,
                origin,
                deadline,
                target: event.target,
                revision,
                supporters,
            },
            PendingEvent::Inertial(event) => ProvenanceRecord::PendingInertialDelay {
                stimulus: event.stimulus,
                event: key,
                owner,
                origin,
                deadline,
                target: event.target,
                revision,
                supporters,
            },
            PendingEvent::Periodic(event) => ProvenanceRecord::PendingPeriodicBoundary {
                stimulus: event.stimulus,
                event: key,
                owner,
                origin,
                deadline,
                anchor: event.anchor,
                ordinal: event.ordinal,
                first_emission: event.first_emission,
                reenable_phase: event.reenable_phase,
                revision,
                supporters,
            },
        };
        let cause = push_record(scope, &mut records, record);
        match event {
            PendingEvent::PulseDelay(event) => event.cause = cause,
            PendingEvent::TransportDelay(event) => event.cause = cause,
            PendingEvent::Inertial(event) => event.cause = cause,
            PendingEvent::Periodic(event) => event.cause = cause,
        }
    }
    let growth = records.len().saturating_sub(replacement_count);
    MigrationProvenance {
        view: ProvenanceView {
            scope,
            records: Arc::new(records),
        },
        translated,
        patch,
        growth,
    }
}

#[allow(clippy::too_many_arguments)]
fn build_ready_provenance<D>(
    compiled: &crate::CompiledNetwork<D>,
    revision: NetworkRevision,
    at: ReactionStamp<D>,
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
    declared_edges: &BTreeSet<NodeKey>,
    patch_cause: Option<CauseRef>,
) -> ProvenanceBuild<D> {
    let scope = crate::causal_store::fresh_scope(Some(previous.scope));
    let mut records = previous.records.fork(scope);
    let ordinary_transaction_cause = push_record(
        scope,
        &mut records,
        ProvenanceRecord::ReadyTransaction { at, revision },
    );
    let transaction_cause = patch_cause.unwrap_or(ordinary_transaction_cause);
    let mut input_causes = previous_input_causes
        .iter()
        .map(|(input, cause)| (*input, remap_cause(*cause, scope)))
        .collect::<BTreeMap<_, _>>();
    for (input, value) in explicit_levels {
        let cause = push_record(
            scope,
            &mut records,
            ProvenanceRecord::ExternalObservation {
                stamp: at,
                input: *input,
                value: *value,
            },
        );
        input_causes.insert(*input, cause);
    }
    // SPEC: docs/specs/contracts/replay-artifacts.yaml "embedded-transaction"
    // Zero occurrences have no separate causal fact, matching persisted input omission.
    let pulse_input_causes = pulses
        .iter()
        .filter(|(_, count)| count.is_positive())
        .map(|(input, count)| {
            let cause = push_record(
                scope,
                &mut records,
                ProvenanceRecord::ExternalPulseObservation {
                    stamp: at,
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
            declared_edges,
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

fn retained_edge_cause(
    previous_state: Option<&PreviousStateCauses<'_>>,
    causes: &BTreeMap<NodeKey, CauseRef>,
    node: &NodeKey,
    previous: &EdgeObservation,
) -> Option<CauseRef> {
    if !matches!(previous, EdgeObservation::Established(_)) {
        return None;
    }
    let state = previous_state?;
    if let Some(cause) = causes.get(node).copied() {
        return Some(cause);
    }
    if state.declared_edges.contains(node) {
        // SPEC: docs/specs/contracts/atomic-topology-replacement.yaml
        // "revision-provenance-and-episodes"
        // Declared initial or reset observation starts a new cause at this reaction.
        return None;
    }
    panic!("ready edge detector must retain the cause of its previous observation");
}

fn append_evaluation_provenance<D>(
    scope: ProvenanceScope,
    records: &mut crate::causal_store::Records<D>,
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
                if let Some(previous_cause) = retained_edge_cause(
                    previous_state.as_ref(),
                    &edge_observation_causes,
                    node,
                    previous,
                ) {
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
    if crate::causal_store::same_lineage(cause.scope, scope) {
        cause
    } else {
        CauseRef {
            scope,
            ordinal: cause.ordinal,
        }
    }
}

fn finalize_provenance_build<D>(
    build: &mut ProvenanceBuild<D>,
    compiled: &crate::CompiledNetwork<D>,
) {
    crate::state_digest::freeze_provenance(compiled, &build.provenance);
}

pub(crate) fn remap_record<D>(
    record: &ProvenanceRecord<D>,
    scope: ProvenanceScope,
) -> ProvenanceRecord<D> {
    translate_record(record, |cause| remap_cause(cause, scope))
}

fn translate_record<D>(
    record: &ProvenanceRecord<D>,
    translate: impl Fn(CauseRef) -> CauseRef,
) -> ProvenanceRecord<D> {
    #[cfg(test)]
    crate::causal_work::update(|work| work.old_record_rewrites += 1);
    match record {
        ProvenanceRecord::TopologyChange {
            at,
            revision,
            base,
            target,
            supporters,
        } => ProvenanceRecord::TopologyChange {
            at: *at,
            revision: *revision,
            base: *base,
            target: *target,
            supporters: supporters.iter().map(|cause| translate(*cause)).collect(),
        },
        ProvenanceRecord::Migration {
            subject,
            rule,
            supporters,
        } => ProvenanceRecord::Migration {
            subject: subject.clone(),
            rule: rule.clone(),
            supporters: supporters.iter().map(|cause| translate(*cause)).collect(),
        },
        ProvenanceRecord::Checkpoint { fact, supporters } => ProvenanceRecord::Checkpoint {
            fact: fact.clone(),
            supporters: supporters.iter().map(|cause| translate(*cause)).collect(),
        },
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
        ProvenanceRecord::ExternalObservation {
            input,
            value,
            stamp,
        } => ProvenanceRecord::ExternalObservation {
            stamp: *stamp,
            input: *input,
            value: *value,
        },
        ProvenanceRecord::ExternalPulseObservation {
            input,
            count,
            stamp,
        } => ProvenanceRecord::ExternalPulseObservation {
            stamp: *stamp,
            input: *input,
            count: *count,
        },
        ProvenanceRecord::PendingPulseDelay {
            event,
            owner,
            stimulus,
            origin,
            deadline,
            count,
            revision,
            supporters,
        } => ProvenanceRecord::PendingPulseDelay {
            event: *event,
            owner: owner.clone(),
            stimulus: *stimulus,
            origin: *origin,
            deadline: *deadline,
            count: *count,
            revision: *revision,
            supporters: supporters.iter().map(|cause| translate(*cause)).collect(),
        },
        ProvenanceRecord::PendingPeriodicBoundary {
            event,
            owner,
            stimulus,
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
            stimulus: *stimulus,
            origin: *origin,
            deadline: *deadline,
            anchor: *anchor,
            ordinal: *ordinal,
            first_emission: *first_emission,
            reenable_phase: *reenable_phase,
            revision: *revision,
            supporters: supporters.iter().map(|cause| translate(*cause)).collect(),
        },
        ProvenanceRecord::PendingInertialDelay {
            event,
            owner,
            stimulus,
            origin,
            deadline,
            target,
            revision,
            supporters,
        } => ProvenanceRecord::PendingInertialDelay {
            event: *event,
            owner: owner.clone(),
            stimulus: *stimulus,
            origin: *origin,
            deadline: *deadline,
            target: *target,
            revision: *revision,
            supporters: supporters.iter().map(|cause| translate(*cause)).collect(),
        },
        ProvenanceRecord::PendingTransportDelay {
            event,
            owner,
            stimulus,
            origin,
            deadline,
            target,
            revision,
            supporters,
        } => ProvenanceRecord::PendingTransportDelay {
            event: *event,
            owner: owner.clone(),
            stimulus: *stimulus,
            origin: *origin,
            deadline: *deadline,
            target: *target,
            revision: *revision,
            supporters: supporters.iter().map(|cause| translate(*cause)).collect(),
        },
        ProvenanceRecord::Derived {
            subject,
            supporters,
        } => ProvenanceRecord::Derived {
            subject: subject.clone(),
            supporters: supporters.iter().map(|cause| translate(*cause)).collect(),
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
                    cause: translate(contribution.cause),
                })
                .collect(),
            result: *result,
            supporters: supporters.iter().map(|cause| translate(*cause)).collect(),
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
                    cause: translate(contribution.cause),
                })
                .collect(),
            result: *result,
            supporters: supporters.iter().map(|cause| translate(*cause)).collect(),
        },
    }
}

fn push_record<D>(
    scope: ProvenanceScope,
    records: &mut crate::causal_store::Records<D>,
    mut record: ProvenanceRecord<D>,
) -> CauseRef {
    record.translate_causes(|cause| remap_cause(cause, scope));
    records.push(record)
}

fn operation_cause(causes: &[CauseRef], index: usize) -> CauseRef {
    let Some(cause) = causes.get(index).copied() else {
        panic!("evaluation provenance predecessor must precede its dependent operation");
    };
    cause
}

fn initialization_events<D>(
    at: ReactionStamp<D>,
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
                stamp: at,
                cause,
                revision,
            }
        })
        .collect::<Vec<_>>();
    events.extend(pulse_events(at, revision, pulse_values, pulse_causes));
    sort_output_events(&mut events);
    events
}

struct LevelOutputValues<'a> {
    previous: &'a BTreeMap<ExternalOutputKey<Level>, LogicLevel>,
    settled: &'a BTreeMap<ExternalOutputKey<Level>, LogicLevel>,
}

fn planned_level_events<D>(
    at: ReactionStamp<D>,
    revision: NetworkRevision,
    plans: &BTreeMap<ExternalOutputKey<Level>, OutputBaselinePlan>,
    values: LevelOutputValues<'_>,
    causes: &BTreeMap<ExternalOutputKey<Level>, CauseRef>,
    pulse_values: &BTreeMap<ExternalOutputKey<Pulse>, PulseCount>,
    pulse_causes: &BTreeMap<ExternalOutputKey<Pulse>, CauseRef>,
) -> Vec<OutputEvent<D>> {
    let LevelOutputValues { previous, settled } = values;
    let mut events = Vec::new();
    for (output, to) in settled {
        let plan = match plans.get(output).copied() {
            Some(plan) => plan,
            None => panic!("settled output must have a baseline plan"),
        };
        let cause = match causes.get(output).copied() {
            Some(cause) => cause,
            None => panic!("every evaluated external output must retain one committed cause"),
        };
        match plan {
            OutputBaselinePlan::Establish | OutputBaselinePlan::CarryAsEvidence => {
                events.push(OutputEvent::LevelEstablished {
                    output: *output,
                    value: *to,
                    stamp: at,
                    cause,
                    revision,
                });
            }
            OutputBaselinePlan::Preserve => {
                let from = match previous.get(output).copied() {
                    Some(from) => from,
                    None => panic!("preserved output must retain its baseline"),
                };
                if from != *to {
                    events.push(OutputEvent::LevelChanged {
                        output: *output,
                        from,
                        to: *to,
                        stamp: at,
                        cause,
                        revision,
                    });
                }
            }
            OutputBaselinePlan::Remove => {
                panic!("removed output must not remain in the settled set");
            }
        }
    }
    events.extend(pulse_events(at, revision, pulse_values, pulse_causes));
    sort_output_events(&mut events);
    events
}

fn changed_events<D>(
    at: ReactionStamp<D>,
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
                stamp: at,
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
    at: ReactionStamp<D>,
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
                stamp: at,
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
fn causal_preparation_fault<D>(
    policy: &RuntimePolicy,
    fault: crate::causal_work::Fault,
) -> Result<(), RuntimeFailure<D>> {
    if crate::causal_work::take_fault(fault) {
        Err(RuntimeFailure::new(
            RuntimeFailureEvidence::BudgetExceeded {
                budget: RuntimePolicyLimit::MaxRequiredProvenanceGrowth,
                limit: policy.max_required_provenance_growth(),
                consumed: u64::MAX,
            },
        ))
    } else {
        Ok(())
    }
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

    #[test]
    fn bound_runtime_rechecks_new_level_values_before_processing_any_deadline() {
        use crate::time::{NonZeroSpan, Time};
        use crate::{
            BindingSet, BoundMachine, InputObservation, NetworkBuilder, PulseDelayConfig,
            ReconfigurationPolicy,
        };
        let mut builder =
            NetworkBuilder::<()>::with_key(NetworkKey::from_u128(91), TimeDomainId::from_u128(2));
        let (input, pulse) = builder.pulse_input("trip");
        let delayed = builder
            .pulse_delay(
                pulse,
                PulseDelayConfig::new(NonZeroSpan::from_ticks(5).unwrap()),
            )
            .unwrap();
        let output = builder.pulse_output("delayed", delayed).unwrap();
        let compiled = builder
            .finish()
            .require_artifact()
            .unwrap()
            .compile()
            .require_artifact()
            .unwrap();
        let bindings = BindingSet::builder(&compiled)
            .bind_input(input, "trip")
            .unwrap()
            .bind_output(output, "delayed")
            .unwrap()
            .finish()
            .unwrap();
        let mut bound = BoundMachine::spawn(
            &compiled,
            policy_with([100, 1000, 100, 100, 1000]),
            bindings,
        )
        .unwrap();
        bound
            .initialize(
                Time::from_ticks(0),
                [InputObservation::Pulse {
                    input: "trip",
                    count: PulseCount::new(u64::MAX),
                }],
            )
            .unwrap();
        bound
            .advance(
                Time::from_ticks(0),
                [InputObservation::Pulse {
                    input: "trip",
                    count: PulseCount::ONE,
                }],
            )
            .unwrap();
        let extra = ExternalInputKey::<Level>::from_u128(100);
        let prepared = bound
            .prepare_patch(
                bound
                    .machine()
                    .patch()
                    .add_external_input(ExternalInputDef::new(
                        extra.into(),
                        DiagnosticMeta::default(),
                    ))
                    .unwrap()
                    .finish(),
            )
            .require_artifact()
            .unwrap();
        let target = BindingSet::builder(prepared.resulting_compiled())
            .bind_input(input, "trip")
            .unwrap()
            .bind_input(extra, "extra")
            .unwrap()
            .bind_output(output, "delayed")
            .unwrap()
            .finish()
            .unwrap();
        let delta = prepared
            .input_delta()
            .set(extra, LogicLevel::High)
            .unwrap()
            .finish()
            .unwrap();
        let mut tx = Transaction::advance(Time::from_ticks(6), bound.machine().revision(), delta)
            .with_patch(prepared, ReconfigurationPolicy::RejectStateLoss)
            .unwrap();
        // Model a malformed artifact after the public construction check; runtime
        // must independently enforce target establishment before the due tick 5.
        let super::TransactionKind::Advance(input) = &mut tx.kind else {
            unreachable!()
        };
        input.levels.remove(&extra);
        let before = observe(bound.machine());
        let failure = bound.apply_reconfigured(tx, target).err().unwrap();
        assert_eq!(
            failure.code(),
            DiagnosticCode::ReconfigurationTargetInputSchemaMismatch
        );
        assert_eq!(observe(bound.machine()), before);
        assert_eq!(bound.bindings().input_identifier(extra), None);
        assert_eq!(bound.bindings().output_identifier(output), Some(&"delayed"));
        // The due batch would itself reject. Target admission must take precedence.
        assert_eq!(
            bound.advance(Time::from_ticks(6), []).err().unwrap().code(),
            DiagnosticCode::RuntimePulseCountOverflow
        );
        assert_eq!(observe(bound.machine()), before);
    }

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
    fn dropped_quiet_reactions_reclaim_irrelevant_catalogue_and_membership() {
        let compiled = compiled(100, 200);
        let mut machine = initialized_machine(&compiled, LogicLevel::Low);
        let bound = machine.store.provenance.as_ref().unwrap().len() + 2;
        for step in 1..=40 {
            drop(
                machine
                    .apply(Transaction::advance(
                        crate::time::Time::from_ticks(10 + step),
                        machine.revision(),
                        compiled.input_delta().finish().unwrap(),
                    ))
                    .unwrap(),
            );
            let view = machine.store.provenance.as_ref().unwrap();
            assert!(
                view.len() <= bound,
                "quiet history {step} retained {} records (bound {bound})",
                view.len()
            );
            assert!(crate::causal_work::live_nodes() <= bound);
        }
    }

    #[test]
    fn quiet_growth_counts_appends_even_when_current_membership_does_not_grow() {
        let compiled = compiled(100, 200);
        let seed = initialized_machine(&compiled, LogicLevel::Low);
        let retained = seed
            .inspect_output(ExternalOutputKey::from_u128(200))
            .unwrap();
        let prior = seed.store.provenance.as_ref().unwrap().len();
        for limit in [0, 1, 2] {
            let mut machine = compiled.spawn(policy_with([100, 1000, 100, 100, limit]));
            machine.store = seed.store.clone();
            let before = machine.snapshot();
            crate::causal_work::reset();
            let result = machine.apply(Transaction::advance(
                crate::time::Time::from_ticks(11),
                machine.revision(),
                compiled.input_delta().finish().unwrap(),
            ));
            // A level wire appends only a Ready fact. Its input origin and
            // unchanged output baseline survive; the unused new fact is reclaimed.
            let expected = 1;
            assert_eq!(crate::causal_work::read().nodes_created, expected);
            if limit < expected as u64 {
                assert!(matches!(
                    result.unwrap_err().evidence(),
                    RuntimeFailureEvidence::BudgetExceeded {
                        budget: RuntimePolicyLimit::MaxRequiredProvenanceGrowth,
                        consumed: 1,
                        ..
                    }
                ));
                assert_eq!(machine.snapshot(), before);
            } else {
                result.unwrap();
                assert!(machine.store.provenance.as_ref().unwrap().len() <= prior + 1);
            }
        }
        retained
            .provenance()
            .explain_cause(retained.current_support.unwrap())
            .unwrap();
    }

    #[test]
    fn zero_pulse_normalization_has_equal_forecast_and_apply_growth_boundaries() {
        let compiled = compiled_merge();
        let transaction = |machine: &crate::Machine<()>, initialize, counts: Option<(u64, u64)>| {
            if initialize {
                let mut input = compiled.input_snapshot();
                if let Some((first, second)) = counts {
                    for (key, count) in [(1, first), (2, second)] {
                        input = input
                            .pulse(ExternalInputKey::from_u128(key), PulseCount::new(count))
                            .unwrap();
                    }
                }
                Transaction::initialize(
                    crate::time::Time::from_ticks(0),
                    machine.revision(),
                    input.finish().unwrap(),
                )
            } else {
                let mut input = compiled.input_delta();
                if let Some((first, second)) = counts {
                    for (key, count) in [(1, first), (2, second)] {
                        input = input
                            .pulse(ExternalInputKey::from_u128(key), PulseCount::new(count))
                            .unwrap();
                    }
                }
                Transaction::advance(
                    crate::time::Time::from_ticks(1),
                    machine.revision(),
                    input.finish().unwrap(),
                )
            }
        };
        let mut seed = compiled.spawn(policy_with([100, 1000, 100, 100, 100]));
        seed.apply(transaction(&seed, true, None)).unwrap();
        // No occurrences append transaction + Merge + output derivations.
        // Positive batches additionally append both input observations.
        for initialize in [true, false] {
            for limit in [2, 3, 4] {
                let mut snapshots = Vec::new();
                for counts in [None, Some((0, 0))] {
                    let mut machine = compiled.spawn(policy_with([100, 1000, 100, 100, limit]));
                    if !initialize {
                        machine.store = seed.store.clone();
                    }
                    let transaction = transaction(&machine, initialize, counts);
                    let before = observe(&machine);
                    crate::causal_work::reset();
                    let forecast = machine.forecast(transaction.clone());
                    assert_eq!(crate::causal_work::read().nodes_created, 3);
                    assert_eq!(observe(&machine), before);
                    crate::causal_work::reset();
                    let applied = machine.apply(transaction);
                    assert_eq!(crate::causal_work::read().nodes_created, 3);
                    if limit < 3 {
                        for failure in [forecast.unwrap_err(), applied.unwrap_err()] {
                            assert!(matches!(
                                failure.evidence(),
                                RuntimeFailureEvidence::BudgetExceeded {
                                    budget: RuntimePolicyLimit::MaxRequiredProvenanceGrowth,
                                    consumed: 3,
                                    ..
                                }
                            ));
                        }
                        assert_eq!(observe(&machine), before);
                    } else {
                        let forecast = forecast.unwrap();
                        applied.unwrap();
                        assert_eq!(crate::causal_work::read().logical_growth, 3);
                        assert_eq!(forecast.state().snapshot(), machine.snapshot());
                    }
                    snapshots.push(machine.snapshot());
                }
                assert_eq!(snapshots[0], snapshots[1]);
            }
            let mut positive = compiled.spawn(policy_with([100, 1000, 100, 100, 100]));
            if !initialize {
                positive.store = seed.store.clone();
            }
            crate::causal_work::reset();
            let result = positive
                .apply(transaction(&positive, initialize, Some((1, 2))))
                .unwrap();
            assert_eq!(crate::causal_work::read().nodes_created, 5);
            assert_eq!(crate::causal_work::read().logical_growth, 5);
            assert!(
                matches!(result.output_events(), [OutputEvent::Pulsed { count, .. }] if *count == PulseCount::new(3))
            );
        }
    }

    #[test]
    fn ordinary_advance_shares_prior_records_and_hashes_only_new_nodes() {
        for history in [4_u64, 40] {
            let compiled = compiled(100, 200);
            let mut machine = initialized_machine(&compiled, LogicLevel::Low);
            for step in 1..=history {
                let delta = compiled.input_delta().finish().unwrap();
                machine
                    .apply(Transaction::advance(
                        crate::time::Time::from_ticks(10 + step),
                        machine.revision(),
                        delta,
                    ))
                    .unwrap();
            }
            let prior_len = machine.store.provenance.as_ref().unwrap().len();
            crate::causal_work::reset();
            let delta = compiled.input_delta().finish().unwrap();
            machine
                .apply(Transaction::advance(
                    crate::time::Time::from_ticks(11 + history),
                    machine.revision(),
                    delta,
                ))
                .unwrap();
            let work = crate::causal_work::read();
            let new_nodes = work.nodes_created;
            assert_eq!(work.old_record_rewrites, 0, "history {history}: {work:?}");
            assert_eq!(work.scope_records_hashed, 0, "history {history}: {work:?}");
            assert_eq!(
                work.canonical_encodes, new_nodes,
                "history {history}: {work:?}"
            );
            assert_eq!(
                work.canonical_hashes, new_nodes,
                "history {history}: {work:?}"
            );
            assert_eq!(new_nodes, 1);
            assert_eq!(work.namespace_allocations, 1);
            assert_eq!(work.catalogue_handles_copied, prior_len);
            assert_eq!(work.membership_entries_copied, prior_len);
            crate::causal_work::reset();
            let _ = machine.execution_state_digest();
            let _ = machine.observable_state_digest();
            let _ = machine.snapshot();
            let work = crate::causal_work::read();
            assert_eq!(work.old_record_rewrites, 0);
            assert_eq!(work.scope_records_hashed, 0);
            assert_eq!(work.canonical_encodes, 0);
            assert_eq!(work.canonical_hashes, 0);
            assert_eq!(work.namespace_allocations, 0);
            assert_eq!(work.catalogue_handles_copied, 0);
            assert_eq!(work.membership_entries_copied, 0);
        }
    }

    #[test]
    fn growing_stateful_ancestry_reuses_content_but_still_emits_required_closure() {
        let mut smaller_closure = None;
        for history in [4_u64, 40] {
            let compiled = compiled_toggle(LogicLevel::Low, false);
            let mut machine = compiled.spawn(policy_with([10, 100, 0, 100, 1_000]));
            machine
                .apply(Transaction::initialize(
                    crate::time::Time::from_ticks(0),
                    machine.revision(),
                    compiled.input_snapshot().finish().unwrap(),
                ))
                .unwrap();
            let advance = |machine: &mut crate::Machine<()>, at| {
                let delta = compiled
                    .input_delta()
                    .pulse(ExternalInputKey::from_u128(1), PulseCount::ONE)
                    .unwrap()
                    .finish()
                    .unwrap();
                machine
                    .apply(Transaction::advance(
                        crate::time::Time::from_ticks(at),
                        machine.revision(),
                        delta,
                    ))
                    .unwrap()
            };
            for at in 1..=history {
                advance(&mut machine, at);
            }
            let old = machine.inspect_toggle(NodeKey::from_u128(10)).unwrap();
            let ancestor = old.latest_inversion().unwrap();
            let prior_len = machine.store.provenance.as_ref().unwrap().len();
            crate::causal_work::reset();
            let result = advance(&mut machine, history + 1);
            let work = crate::causal_work::read();
            let current = machine.store.provenance.as_ref().unwrap();
            let added = work.nodes_created;
            assert!(added > 1);
            assert_eq!(work.old_record_rewrites, 0);
            assert_eq!(work.scope_records_hashed, 0);
            assert_eq!(work.canonical_encodes, added);
            assert_eq!(work.canonical_hashes, added);
            assert_eq!(work.namespace_allocations, 1);
            assert_eq!(work.catalogue_handles_copied, prior_len);
            assert_eq!(work.membership_entries_copied, prior_len);
            assert!(
                old.provenance()
                    .records()
                    .shared_node(current.records(), ancestor)
            );
            result.provenance().explain_cause(ancestor).unwrap();

            let reference = crate::state_digest_reference::observable_digest_input(&machine, 3, 3);
            crate::causal_work::reset();
            let actual = crate::state_digest::observable_digest_input(&machine, 3, 3);
            let work = crate::causal_work::read();
            assert_eq!(actual, reference);
            assert_eq!(work.canonical_encodes, 0);
            assert_eq!(work.canonical_hashes, 0);
            assert!(work.closure_records_visited > 0);
            assert!(work.closure_records_emitted > 0);
            assert!(work.closure_bytes_emitted > 0);
            let closure = (
                work.closure_records_visited,
                work.closure_records_emitted,
                work.closure_bytes_emitted,
            );
            if let Some((visited, emitted, bytes)) = smaller_closure {
                assert!(closure.0 > visited);
                assert!(closure.1 > emitted);
                assert!(closure.2 > bytes);
            }
            smaller_closure = Some(closure);
        }
    }

    #[test]
    fn repeated_owned_inspection_preserves_value_equality() {
        let compiled = compiled_toggle(LogicLevel::Low, false);
        let mut machine = compiled.spawn(policy_with([10, 100, 0, 100, 1_000]));
        let snapshot = compiled.input_snapshot().finish().unwrap();
        machine
            .apply(Transaction::initialize(
                crate::time::Time::from_ticks(0),
                machine.revision(),
                snapshot,
            ))
            .unwrap();
        assert_eq!(
            machine.inspect_toggle(NodeKey::from_u128(10)).unwrap(),
            machine.inspect_toggle(NodeKey::from_u128(10)).unwrap()
        );
        let delta = compiled
            .input_delta()
            .pulse(ExternalInputKey::from_u128(1), PulseCount::ONE)
            .unwrap()
            .finish()
            .unwrap();
        machine
            .apply(Transaction::advance(
                crate::time::Time::from_ticks(1),
                machine.revision(),
                delta,
            ))
            .unwrap();
        assert_eq!(
            machine.inspect_toggle(NodeKey::from_u128(10)).unwrap(),
            machine.inspect_toggle(NodeKey::from_u128(10)).unwrap()
        );
    }

    #[test]
    fn shared_nodes_preserve_ancestors_and_isolate_forecast_branches() {
        let compiled = compiled(100, 200);
        let output = ExternalOutputKey::from_u128(200);
        let mut machine = initialized_machine(&compiled, LogicLevel::Low);
        let old = machine.inspect_output(output).unwrap();
        let old_cause = old.latest_transition.unwrap();
        let live_before = crate::causal_work::live_nodes();
        let revision = machine.revision();
        let transaction = || {
            Transaction::advance(
                crate::time::Time::from_ticks(11),
                revision,
                compiled
                    .input_delta()
                    .set(ExternalInputKey::from_u128(1), LogicLevel::High)
                    .unwrap()
                    .finish()
                    .unwrap(),
            )
        };
        let forecast = machine.forecast(transaction()).unwrap();
        let branch = match forecast.result().output_events()[0] {
            OutputEvent::LevelChanged { cause, .. } => cause,
            _ => panic!("changed output"),
        };
        assert!(matches!(
            old.provenance().inspect(branch),
            Err(CauseLookupFailure::ForeignCause { .. })
        ));
        let result = machine.apply(transaction()).unwrap();
        let committed = machine.output_cause(output).unwrap();
        assert_ne!(branch, committed);
        assert!(matches!(
            result.provenance().inspect(branch),
            Err(CauseLookupFailure::ForeignCause { .. })
        ));
        assert!(matches!(
            forecast.result().provenance().inspect(committed),
            Err(CauseLookupFailure::ForeignCause { .. })
        ));
        let current = machine.store.provenance.as_ref().unwrap();
        assert_eq!(
            crate::state_digest::cause_content(forecast.result().provenance(), branch),
            crate::state_digest::cause_content(result.provenance(), committed)
        );
        assert!(
            current.records().position(old_cause).is_none(),
            "replaced stateless fact is owned only by old artifacts"
        );
        old.provenance().inspect(old_cause).unwrap();
        drop(forecast);
        drop(result);
        drop(machine);
        recursively_assert_acyclic(old.provenance(), old_cause);
        assert!(crate::causal_work::live_nodes() < live_before + 5);
    }

    #[test]
    fn rejected_preparation_and_discarded_forecasts_release_new_nodes() {
        let compiled = compiled(100, 200);
        let mut machine = initialized_machine(&compiled, LogicLevel::Low);
        let owned = machine
            .inspect_output(ExternalOutputKey::from_u128(200))
            .unwrap();
        let before = observe(&machine);
        let snapshot = machine.snapshot();
        let live_before = crate::causal_work::live_nodes();
        let transaction = || {
            Transaction::advance(
                crate::time::Time::from_ticks(11),
                crate::NetworkRevision::from_value(0),
                compiled
                    .input_delta()
                    .set(ExternalInputKey::from_u128(1), LogicLevel::High)
                    .unwrap()
                    .finish()
                    .unwrap(),
            )
        };
        for stage in [
            crate::causal_work::Fault::Provenance,
            crate::causal_work::Fault::Result,
            crate::causal_work::Fault::Projection,
        ] {
            for forecast in [false, true] {
                crate::causal_work::inject(stage);
                let failure = if forecast {
                    machine.forecast(transaction()).unwrap_err()
                } else {
                    machine.apply(transaction()).unwrap_err()
                };
                assert!(matches!(
                    failure.evidence(),
                    RuntimeFailureEvidence::BudgetExceeded { .. }
                ));
                assert_eq!(observe(&machine), before, "stage {stage:?}");
                assert_eq!(machine.snapshot(), snapshot);
                assert_eq!(
                    crate::causal_work::live_nodes(),
                    live_before,
                    "stage {stage:?}"
                );
                recursively_assert_acyclic(owned.provenance(), owned.latest_transition.unwrap());
            }
        }
        for _ in 0..32 {
            drop(machine.forecast(transaction()).unwrap());
            assert_eq!(crate::causal_work::live_nodes(), live_before);
            assert_eq!(machine.snapshot(), snapshot);
        }
    }

    #[test]
    fn generated_counted_histories_match_uncached_projection_and_private_clone_apply() {
        for seed in 0_u64..12 {
            let compiled = compiled_merge();
            let mut machine = compiled.spawn(policy_with([100, 10_000, 100, 1_000, 100_000]));
            let mut reference = machine.duplicate_for_staging();
            let init = Transaction::initialize(
                crate::time::Time::from_ticks(0),
                machine.revision(),
                compiled
                    .input_snapshot()
                    .pulse(
                        ExternalInputKey::from_u128(1),
                        PulseCount::new(seed % 3 + 1),
                    )
                    .unwrap()
                    .pulse(ExternalInputKey::from_u128(2), PulseCount::new(2))
                    .unwrap()
                    .finish()
                    .unwrap(),
            );
            let mut retained = vec![machine.apply(init.clone()).unwrap()];
            reference.apply(init).unwrap();
            for step in 0..10 {
                let first = PulseCount::new((seed + step * 3) % 5);
                let second = PulseCount::new((seed * 7 + step) % 4 + 1);
                let delta = if seed % 2 == 0 {
                    compiled
                        .input_delta()
                        .pulse(ExternalInputKey::from_u128(1), first)
                        .unwrap()
                        .pulse(ExternalInputKey::from_u128(2), second)
                        .unwrap()
                } else {
                    compiled
                        .input_delta()
                        .pulse(ExternalInputKey::from_u128(2), second)
                        .unwrap()
                        .pulse(ExternalInputKey::from_u128(1), first)
                        .unwrap()
                }
                .finish()
                .unwrap();
                let tx = Transaction::advance(
                    crate::time::Time::from_ticks(step / 2),
                    machine.revision(),
                    delta,
                );
                let forecast = machine.forecast(tx.clone()).unwrap();
                let applied = machine.apply(tx.clone()).unwrap();
                let ordinary = reference.apply(tx).unwrap();
                assert_eq!(machine.snapshot(), reference.snapshot());
                assert_eq!(forecast.state().snapshot(), machine.snapshot());
                assert_eq!(
                    applied.after_observable_digest(),
                    ordinary.after_observable_digest()
                );
                let causes = |result: &crate::TransactionResult<()>| {
                    result
                        .output_events()
                        .iter()
                        .map(|event| match event {
                            OutputEvent::Pulsed { cause, .. } => {
                                crate::state_digest::cause_content(result.provenance(), *cause)
                            }
                            _ => panic!("counted fixture publishes only pulse events"),
                        })
                        .collect::<Vec<_>>()
                };
                assert_eq!(causes(&applied), causes(forecast.result()));
                assert_eq!(causes(&applied), causes(&ordinary));
                retained.push(applied);
            }
            drop(reference);
            drop(machine);
            for result in retained {
                for event in result.output_events() {
                    if let OutputEvent::Pulsed { cause, .. } = event {
                        recursively_assert_acyclic(result.provenance(), *cause);
                    }
                }
            }
        }
    }

    #[test]
    fn external_artifact_lifetimes_and_candidate_allocations_do_not_change_patches() {
        use crate::time::Time;
        let compiled = compiled(100, 200);
        let mut held = initialized_machine(&compiled, LogicLevel::Low);
        let mut dropped = initialized_machine(&compiled, LogicLevel::Low);
        let mut results = Vec::new();
        let mut inspections = Vec::new();
        let mut forecasts = Vec::new();
        for step in 1..=8 {
            let tx = Transaction::advance(
                Time::from_ticks(10 + step),
                held.revision(),
                compiled.input_delta().finish().unwrap(),
            );
            forecasts.push(held.forecast(tx.clone()).unwrap());
            results.push(held.apply(tx.clone()).unwrap());
            inspections.push(
                held.inspect_output(ExternalOutputKey::from_u128(200))
                    .unwrap(),
            );
            drop(dropped.forecast(tx.clone()).unwrap());
            drop(dropped.apply(tx).unwrap());
        }
        assert_eq!(held.snapshot(), dropped.snapshot());
        let prepared = held
            .prepare_patch(
                held.patch()
                    .add_external_output(ExternalOutputDef::new(
                        ExternalOutputKey::<Level>::from_u128(300).into(),
                        SignalSourceKey::ExternalInput(ExternalInputKey::<Level>::from_u128(1))
                            .into(),
                        DiagnosticMeta::default(),
                    ))
                    .unwrap()
                    .finish(),
            )
            .require_artifact()
            .unwrap();
        let tx = Transaction::advance(
            Time::from_ticks(19),
            held.revision(),
            prepared
                .resulting_compiled()
                .input_delta()
                .finish()
                .unwrap(),
        )
        .with_patch(prepared, crate::ReconfigurationPolicy::RejectStateLoss)
        .unwrap();
        let first = held.apply(tx.clone()).unwrap();
        let second = dropped.apply(tx).unwrap();
        assert_eq!(held.snapshot(), dropped.snapshot());
        assert_eq!(
            first.after_execution_digest(),
            second.after_execution_digest()
        );
        assert_eq!(
            first.after_observable_digest(),
            second.after_observable_digest()
        );
        let delta = held
            .compiled()
            .input_delta()
            .set(ExternalInputKey::from_u128(1), LogicLevel::High)
            .unwrap()
            .finish()
            .unwrap();
        let tx = Transaction::advance(Time::from_ticks(20), held.revision(), delta);
        held.apply(tx.clone()).unwrap();
        dropped.apply(tx).unwrap();
        assert_eq!(held.snapshot(), dropped.snapshot());
        drop(held);
        drop(dropped);
        for inspection in inspections {
            recursively_assert_acyclic(
                inspection.provenance(),
                inspection.latest_transition.unwrap(),
            );
        }
        for forecast in forecasts {
            assert!(
                forecast
                    .state()
                    .inspect_output(ExternalOutputKey::from_u128(200))
                    .is_ok()
            );
        }
        for result in results {
            for position in 0..result.provenance().len() {
                result
                    .provenance()
                    .inspect(result.provenance().records().cause(position))
                    .unwrap();
            }
        }
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
                RuntimeFailureEvidence::TimeRegression {
                    current_ticks: 7,
                    requested_ticks: 7,
                },
                DiagnosticCode::RuntimeTimeRegression,
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
                    reaction_order: 0,
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
        last_reaction: Option<crate::ReactionStamp<()>>,
        revision: NetworkRevision,
        external_levels: std::collections::BTreeMap<ExternalInputKey<Level>, LogicLevel>,
        settled_levels: Vec<LogicLevel>,
        output_baselines: std::collections::BTreeMap<ExternalOutputKey<Level>, LogicLevel>,
        input_causes: std::collections::BTreeMap<ExternalInputKey<Level>, CauseRef>,
        output_causes: std::collections::BTreeMap<ExternalOutputKey<Level>, CauseRef>,
        provenance: Option<([u8; 32], usize, usize)>,
        operation_levels: Vec<Option<LogicLevel>>,
        operation_causes: Vec<CauseRef>,
        standard_causes: std::collections::BTreeMap<
            crate::QualifiedModuleRef,
            crate::standard::stateful::StandardCauses,
        >,
        execution: crate::ExecutionStateDigest,
        observable: crate::ObservableStateDigest,
        snapshot: Vec<u8>,
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
        periodic_anchors: std::collections::BTreeMap<NodeKey, crate::machine::PeriodicPhase<()>>,
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
            last_reaction: machine.last_reaction(),
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
            standard_causes: machine.store.standard_causes.clone(),
            execution: machine.execution_state_digest(),
            observable: machine.observable_state_digest(),
            snapshot: machine.snapshot().artifact_bytes().to_vec(),
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
        crate::causal_work::reset();
        let success = probe.apply(transaction(probe.revision())).unwrap();
        // Each reaction appends one transaction, two node/output facts each,
        // and two scheduling explanations. Each due reaction appends one pending
        // boundary; the final reaction adds one input observation and boundary.
        let growth = 3 * (1 + 2 + 2 + 2 + 1) + (1 + 2 + 2 + 2 + 1 + 1);
        assert_eq!(crate::causal_work::read().nodes_created as u64, growth);
        assert_eq!(crate::causal_work::read().logical_growth, growth);
        for limit in [growth, growth + 1] {
            let mut machine = initialize(policy_with([100, 10_000, 100, 1_000, limit]));
            crate::causal_work::reset();
            let admitted = machine.apply(transaction(machine.revision())).unwrap();
            assert_eq!(crate::causal_work::read().logical_growth, growth);
            assert_eq!(
                admitted.after_execution_digest(),
                success.after_execution_digest()
            );
            assert_eq!(
                admitted.after_observable_digest(),
                success.after_observable_digest()
            );
        }
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
    fn ordered_same_time_reactions_preserve_committed_predecessors() {
        let compiled = compiled_toggle(LogicLevel::Low, false);
        let mut machine = compiled.spawn(policy_with([10, 100, 0, 100, 1_000]));
        let at = crate::time::Time::from_ticks(10);
        machine
            .apply(Transaction::initialize(
                at,
                machine.revision(),
                compiled.input_snapshot().finish().unwrap(),
            ))
            .unwrap();
        for expected in [LogicLevel::High, LogicLevel::Low] {
            let delta = compiled
                .input_delta()
                .pulse(ExternalInputKey::from_u128(1), PulseCount::new(1))
                .unwrap()
                .finish()
                .unwrap();
            machine
                .apply(Transaction::advance(at, machine.revision(), delta))
                .unwrap();
            assert_eq!(
                machine.output_level(ExternalOutputKey::from_u128(30)),
                Some(expected)
            );
        }
        let before = machine.execution_state_digest();
        machine
            .apply(Transaction::advance(
                at,
                machine.revision(),
                compiled.input_delta().finish().unwrap(),
            ))
            .unwrap();
        assert_ne!(before, machine.execution_state_digest());
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
                crate::time::Time::from_ticks(9),
                machine.revision(),
                odd,
            ))
            .unwrap_err();
        assert!(matches!(
            failure.evidence(),
            RuntimeFailureEvidence::TimeRegression { .. }
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
                CauseInspection::TopologyChange { supporters, .. }
                | CauseInspection::Migration { supporters, .. }
                | CauseInspection::Checkpoint { supporters, .. } => {
                    for supporter in supporters {
                        visit(view, *supporter, visiting, visited);
                    }
                }
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
                stamp: at,
                cause,
                revision: actual_revision,
            },
        ] = result.output_events()
        else {
            panic!("initialization must return one establishment");
        };
        assert_eq!(*actual_output, output);
        assert_eq!(*value, LogicLevel::High);
        assert_eq!(at.time(), crate::time::Time::from_ticks(37));
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
                    stamp: at,
                    cause,
                    revision: actual_revision,
                },
            ] => {
                assert_eq!(*actual_output, output);
                assert_eq!(*from, LogicLevel::Low);
                assert_eq!(*to, LogicLevel::High);
                assert_eq!(at.time(), crate::time::Time::from_ticks(20));
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

        for requested in [0, 9] {
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
            assert_eq!(failure.code(), DiagnosticCode::RuntimeTimeRegression);
            assert!(matches!(
                failure.evidence(),
                RuntimeFailureEvidence::TimeRegression { .. }
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
                    } => Some((
                        *output,
                        *from,
                        *to,
                        crate::state_digest::cause_content(result.provenance(), *cause),
                    )),
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
            crate::causal_work::reset();
            let result = apply(&mut reference).unwrap();
            let growth = crate::causal_work::read().nodes_created as u64;
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
            stimulus: crate::ReactionStamp::from_parts(crate::time::Time::from_ticks(3), 0),
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
            stimulus: crate::ReactionStamp::from_parts(crate::time::Time::from_ticks(5), 0),
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
                    stimulus: crate::ReactionStamp::from_parts(crate::time::Time::from_ticks(3), 0),
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
    #[test]
    fn reaction_order_overflow_restores_and_rejects_atomically() {
        let c = compiled_toggle(LogicLevel::Low, false);
        let mut m = c.spawn(policy_with([10, 100, 0, 100, 1_000]));
        m.apply(Transaction::initialize(
            crate::time::Time::from_ticks(10),
            m.revision(),
            c.input_snapshot().finish().unwrap(),
        ))
        .unwrap();
        m.store.last_reaction = Some(crate::ReactionStamp::from_parts(
            crate::time::Time::from_ticks(10),
            u64::MAX - 1,
        ));
        // Construct the final coherent occurrence through ordinary apply;
        // only the synthetic precondition skips irrelevant quiet occurrences.
        m.apply(Transaction::advance(
            crate::time::Time::from_ticks(10),
            m.revision(),
            c.input_delta().finish().unwrap(),
        ))
        .unwrap();
        assert_eq!(m.last_reaction().unwrap().order(), u64::MAX);
        let mut restored = c
            .restore(m.snapshot(), policy_with([10, 100, 0, 100, 1_000]))
            .unwrap();
        let before = restored.snapshot();
        for forecast in [true, false] {
            let tx = Transaction::advance(
                crate::time::Time::from_ticks(10),
                restored.revision(),
                c.input_delta().finish().unwrap(),
            );
            let failure = if forecast {
                restored.forecast(tx).unwrap_err()
            } else {
                restored.apply(tx).unwrap_err()
            };
            assert_eq!(failure.code().as_str(), "runtime.reaction_order_overflow");
            assert_eq!(restored.snapshot(), before);
        }
        let result = restored
            .apply(Transaction::advance(
                crate::time::Time::from_ticks(11),
                restored.revision(),
                c.input_delta().finish().unwrap(),
            ))
            .unwrap();
        assert_eq!(result.processed_reactions()[0].order(), 0);
    }
}
