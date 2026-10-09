//! Owned projections and bounded explanations of committed semantic facts.

use crate::authored::{
    EdgeObservation, FirstEmissionPolicy, NodeDef, NodeKind, ReenablePhasePolicy,
};
use crate::diagnostics::{
    DiagnosticCode, InspectionEvidence, InspectionSubjectKind, LifecycleEvidence,
    OperationSubjectRef, PendingEventEvidence, Problem, ProblemEvidence, Responsibility, Severity,
    SubjectRef,
};
use crate::graph::{GraphElement, GraphQueryFailure, GraphSubjectRef, Region};
use crate::key::{
    AnyExternalOutputKey, AnyInPortKey, AnyOutPortKey, ExternalOutputKey, ModuleInstanceKey,
    NodeKey,
};
use crate::machine::PendingEvent;
use crate::signal::{Level, LogicLevel, PulseCount, SignalKind};
use crate::time::{Span, Time};
use crate::transaction::{CauseLookupFailure, ProvenanceRecord};
use crate::{
    ActiveDiagnosticEpisode, CauseRef, CompiledNetwork, ForecastState, Machine, ModuleInspection,
    NetworkRevision, NodeSubject, PendingEventKey, ProvenanceView, QualifiedModuleRef,
    QualifiedNodeRef, TransactionResult,
};
use core::marker::PhantomData;
use std::collections::BTreeSet;

/// A malformed structural request or an unavailable current observation.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InspectionFailure {
    NotInitialized,
    Graph(GraphQueryFailure),
    UnknownPending(PendingEventKey),
    UnknownExplanationSubject(GraphSubjectRef),
    UnknownOutputEvent(usize),
    NoCurrentPulse(AnyExternalOutputKey),
    Cause(CauseLookupFailure),
}
impl InspectionFailure {
    /// Returns the catalogue code for this structured failure.
    #[must_use]
    pub fn code(&self) -> DiagnosticCode {
        self.problem::<()>().code()
    }
    /// Returns fixed catalogue severity.
    #[must_use]
    pub fn severity(&self) -> Severity {
        self.code().severity()
    }
    /// Returns fixed catalogue responsibility.
    #[must_use]
    pub fn responsibility(&self) -> Responsibility {
        self.code().responsibility()
    }
    /// Returns structured evidence identifying the request.
    #[must_use]
    pub fn problem<D>(&self) -> Problem<D> {
        match self {
            Self::Graph(failure) => failure.problem(),
            Self::Cause(failure) => failure.problem(),
            Self::NotInitialized => Problem::new(
                SubjectRef::Operation(OperationSubjectRef::MachineLifecycle),
                Vec::new(),
                ProblemEvidence::LifecycleNotInitialized {
                    evidence: LifecycleEvidence {
                        operation: OperationSubjectRef::MachineLifecycle,
                        current_time_ticks: None,
                    },
                    marker: PhantomData,
                },
            ),
            Self::UnknownPending(event) => Problem::new(
                SubjectRef::Operation(OperationSubjectRef::MachineLifecycle),
                Vec::new(),
                ProblemEvidence::InspectionPendingEventNotFound {
                    evidence: PendingEventEvidence {
                        event: Some(event.value()),
                        owner: String::new(),
                        origin: None,
                        deadline: None,
                        detail: "requested event is absent from the committed pending calendar"
                            .to_owned(),
                    },
                    marker: PhantomData,
                },
            ),
            Self::UnknownExplanationSubject(subject) => Problem::new(
                subject.diagnostic_subject(),
                Vec::new(),
                ProblemEvidence::ExplanationUnknownSubject {
                    evidence: subject.inspection_evidence(),
                    marker: PhantomData,
                },
            ),
            Self::UnknownOutputEvent(index) => unknown_explanation(
                SubjectRef::Operation(OperationSubjectRef::ProvenanceView),
                InspectionSubjectKind::OutputEvent(*index),
            ),
            Self::NoCurrentPulse(output) => {
                let requested = SubjectRef::ExternalOutput(*output);
                Problem::new(
                    requested.clone(),
                    Vec::new(),
                    ProblemEvidence::InspectionWrongSubjectKind {
                        evidence: InspectionEvidence {
                            requested,
                            qualified_path: Vec::new(),
                            expected: InspectionSubjectKind::LevelOutput,
                            actual: Some(InspectionSubjectKind::SignalKind(SignalKind::Pulse)),
                        },
                        marker: PhantomData,
                    },
                )
            }
        }
    }
}
impl From<GraphQueryFailure> for InspectionFailure {
    fn from(value: GraphQueryFailure) -> Self {
        Self::Graph(value)
    }
}
impl From<CauseLookupFailure> for InspectionFailure {
    fn from(value: CauseLookupFailure) -> Self {
        Self::Cause(value)
    }
}
fn unknown_explanation<D>(requested: SubjectRef, expected: InspectionSubjectKind) -> Problem<D> {
    Problem::new(
        requested.clone(),
        Vec::new(),
        ProblemEvidence::ExplanationUnknownSubject {
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

/// A current persistent input fact. Pulse ports have no current value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InputPortInspection {
    pub port: AnyInPortKey,
    pub level: Option<LogicLevel>,
    /// Present only for persistent levels; never an implicit pulse history.
    pub current_support: Option<CauseRef>,
}
/// A current persistent output fact. Pulse ports have no current value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutputPortInspection {
    pub port: AnyOutPortKey,
    pub level: Option<LogicLevel>,
    pub current_support: Option<CauseRef>,
}

/// Committed state, with typed temporal and capture evidence where applicable.
#[non_exhaustive]
pub enum NodeStateInspection<D> {
    Stateless,
    Edge {
        observation: EdgeObservation,
        establishment: Option<CauseRef>,
    },
    StoredLevel {
        value: LogicLevel,
        establishment: Option<CauseRef>,
    },
    SampleHold(Box<crate::SampleHoldInspection<D>>),
    TransportDelay(Box<crate::TransportDelayInspection<D>>),
    InertialDelay(Box<crate::InertialDelayInspection<D>>),
    Periodic(Box<crate::PeriodicInspection<D>>),
}

/// One node's owned definition, current facts, stored state and pending work.
pub struct NodeInspection<D> {
    pub subject: NodeSubject,
    /// Includes kind, configuration, declared initial state and metadata.
    pub definition: NodeDef<D>,
    pub revision: NetworkRevision,
    pub at: Time<D>,
    pub region: Region,
    pub inputs: Vec<InputPortInspection>,
    pub outputs: Vec<OutputPortInspection>,
    pub state: NodeStateInspection<D>,
    pub pending: Vec<PendingEventInspection<D>>,
    pub next_deadline: Option<Time<D>>,
    pub active_diagnostics: Vec<ActiveDiagnosticEpisode<D>>,
    /// Current persistent output justification, separate from state establishment.
    pub current_support: Option<CauseRef>,
    /// Explicitly historical support of the last committed reaction; Pulse counts
    /// found in this record are history and never persistent current values.
    pub last_reaction_cause: CauseRef,
    /// Historical state/output establishment when retained for this node family.
    pub latest_transition: Option<CauseRef>,
    pub retention: RetentionStatus,
    provenance: ProvenanceView<D>,
}
impl<D> NodeInspection<D> {
    /// Retains every cause in this owned observation after subsequent mutations.
    #[must_use]
    pub const fn provenance(&self) -> &ProvenanceView<D> {
        &self.provenance
    }
    fn evidence_causes(&self) -> Vec<CauseRef> {
        let mut causes = node_fact_causes(
            &self.inputs,
            &self.outputs,
            &self.state,
            &self.pending,
            &self.active_diagnostics,
        );
        causes.push(self.last_reaction_cause);
        causes.extend(self.latest_transition);
        causes
    }
}

fn node_fact_causes<D>(
    inputs: &[InputPortInspection],
    outputs: &[OutputPortInspection],
    state: &NodeStateInspection<D>,
    pending: &[PendingEventInspection<D>],
    active: &[ActiveDiagnosticEpisode<D>],
) -> Vec<CauseRef> {
    let mut causes = inputs
        .iter()
        .filter_map(|port| port.current_support)
        .collect::<Vec<_>>();
    causes.extend(outputs.iter().filter_map(|port| port.current_support));
    causes.extend(pending.iter().map(|event| event.cause));
    causes.extend(active.iter().map(ActiveDiagnosticEpisode::cause));
    match state {
        NodeStateInspection::Stateless => {}
        NodeStateInspection::Edge { establishment, .. }
        | NodeStateInspection::StoredLevel { establishment, .. } => causes.extend(*establishment),
        NodeStateInspection::SampleHold(state) => {
            causes.extend([state.current_support(), state.latest_establishment()]);
        }
        NodeStateInspection::TransportDelay(state) => {
            causes.extend([state.current_support(), state.latest_transition()]);
        }
        NodeStateInspection::InertialDelay(state) => {
            causes.extend([state.current_support(), state.latest_transition()]);
            causes.extend(state.last_cancellation());
        }
        NodeStateInspection::Periodic(state) => {
            causes.push(state.current_support());
            causes.extend(state.anchor_cause());
            causes.extend(state.last_cancellation());
        }
    }
    causes
}

/// Current external output state. A Pulse output has no persistent current value.
pub struct OutputInspection<D> {
    pub output: AnyExternalOutputKey,
    pub revision: NetworkRevision,
    pub at: Time<D>,
    pub level: Option<LogicLevel>,
    pub current_support: Option<CauseRef>,
    pub latest_transition: Option<CauseRef>,
    pub retention: RetentionStatus,
    provenance: ProvenanceView<D>,
}
impl<D> OutputInspection<D> {
    /// Returns the retained provenance resolving this observation's causes.
    #[must_use]
    pub const fn provenance(&self) -> &ProvenanceView<D> {
        &self.provenance
    }
}

/// The semantic payload of one pending temporal obligation.
#[non_exhaustive]
pub enum PendingPayload<D> {
    Pulse(PulseCount),
    Level(LogicLevel),
    Periodic {
        anchor: Time<D>,
        ordinal: u64,
        first_emission: FirstEmissionPolicy,
        reenable_phase: ReenablePhasePolicy,
    },
}
/// One owned pending obligation, including its scheduling owner and law.
pub struct PendingEventInspection<D> {
    pub event: PendingEventKey,
    pub owner: NodeSubject,
    /// Identifies the scheduling, cancellation and same-deadline law/configuration.
    pub node_kind: NodeKind<D>,
    /// Revision observed by this read; `revision` is the scheduling revision.
    pub observed_revision: NetworkRevision,
    /// Immutable originating occurrence, independent of a restarted timing basis.
    pub origin_stamp: crate::ReactionStamp<D>,
    pub origin: Time<D>,
    pub deadline: Time<D>,
    pub remaining: Span<D>,
    pub revision: NetworkRevision,
    pub at: Time<D>,
    pub payload: PendingPayload<D>,
    pub cause: CauseRef,
    pub retention: RetentionStatus,
    provenance: ProvenanceView<D>,
}
impl<D> PendingEventInspection<D> {
    /// Returns retained ancestry for this event after it matures or is canceled.
    #[must_use]
    pub const fn provenance(&self) -> &ProvenanceView<D> {
        &self.provenance
    }
}

/// Explicit ancestry coverage; checkpoint references resolve in the owned view.
/// Checkpoint facts retain their canonical subject/time/revision evidence.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RetentionStatus {
    CompleteFromInitialization,
    CompleteFromCheckpoints { checkpoints: Vec<CauseRef> },
}
/// One derivation and its joint, semantically unordered immediate supporters.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExplanationEdge {
    pub cause: CauseRef,
    pub supporters: Vec<CauseRef>,
}
/// Owned backward closure of recorded support. Record payloads remain inspectable.
pub struct CausalExplanation<D> {
    pub roots: Vec<CauseRef>,
    pub edges: Vec<ExplanationEdge>,
    pub retention: RetentionStatus,
    provenance: ProvenanceView<D>,
}
impl<D> CausalExplanation<D> {
    /// Returns the immutable graph retaining every referenced record and payload.
    #[must_use]
    pub const fn provenance(&self) -> &ProvenanceView<D> {
        &self.provenance
    }
}
impl<D> ProvenanceView<D> {
    /// Follows one recorded cause, preserving grouped pulse contributions in records.
    pub fn explain_cause(
        &self,
        cause: CauseRef,
    ) -> Result<CausalExplanation<D>, CauseLookupFailure> {
        causal(self, &[cause])
    }
}
fn causal<D>(
    provenance: &ProvenanceView<D>,
    roots: &[CauseRef],
) -> Result<CausalExplanation<D>, CauseLookupFailure> {
    let mut roots = roots.to_vec();
    roots.sort();
    roots.dedup();
    let mut seen = BTreeSet::new();
    let mut work = roots.clone();
    let mut edges = Vec::new();
    let mut checkpoints = Vec::new();
    while let Some(cause) = work.pop() {
        if !seen.insert(cause) {
            continue;
        }
        provenance.inspect(cause)?;
        let record = &provenance.records()[provenance.resolve_ordinal(cause)];
        let mut supporters = record.predecessor_causes();
        supporters.sort();
        supporters.dedup();
        if matches!(record, ProvenanceRecord::Checkpoint { .. }) {
            checkpoints.push(cause);
        }
        work.extend(supporters.iter().copied());
        edges.push(ExplanationEdge { cause, supporters });
    }
    edges.sort_by_key(|edge| edge.cause);
    checkpoints.sort();
    // SPEC: docs/specs/contracts/causal-explanations.yaml "authoritative-retention-boundary"
    // Never claim ancestry from initialization when a checkpoint is traversed.
    let retention = if checkpoints.is_empty() {
        RetentionStatus::CompleteFromInitialization
    } else {
        RetentionStatus::CompleteFromCheckpoints { checkpoints }
    };
    let owned = provenance.owned_roots(&roots);
    Ok(CausalExplanation {
        roots,
        edges,
        retention,
        provenance: owned,
    })
}

/// Bounded current explanation requests; historical output events use their result.
#[non_exhaustive]
#[derive(Debug, Clone)]
pub enum Explain {
    CurrentNode(NodeKey),
    QualifiedNode(QualifiedNodeRef),
    CurrentModule(ModuleInstanceKey),
    QualifiedModule(QualifiedModuleRef),
    CurrentOutput(AnyExternalOutputKey),
    Pending(PendingEventKey),
}
/// The observed semantic facts and applicable law of an explanation.
#[non_exhaustive]
pub enum ExplainedObservation<D> {
    Node(Box<NodeInspection<D>>),
    /// Public ports/descriptor and standard public-law facts precede primitive drill-down.
    Module(Box<ModuleInspection<D>>),
    Output(OutputInspection<D>),
    Pending(PendingEventInspection<D>),
    OutputEvent {
        output: AnyExternalOutputKey,
        at: crate::ReactionStamp<D>,
        revision: NetworkRevision,
        value: OutputEventValue,
        cause: CauseRef,
    },
}
/// A committed establishment, transition, or exact pulse batch.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputEventValue {
    Established(LogicLevel),
    Changed { from: LogicLevel, to: LogicLevel },
    Pulsed(PulseCount),
}
/// Structured owned explanation with current support distinct from transition history.
pub struct Explanation<D> {
    pub observed: ExplainedObservation<D>,
    pub current_support: Vec<CauseRef>,
    /// Causes of other observed facts, including pending work, stored state and
    /// explicitly historical or suppressed public-module input evidence.
    pub evidence_roots: Vec<CauseRef>,
    pub latest_transition: Option<CauseRef>,
    pub causal: CausalExplanation<D>,
}

/// The standard module's public behavioral law, before primitive drill-down.
#[non_exhaustive]
pub enum ModuleBehavior<'a, D> {
    User,
    Exactly(crate::ExactlyExplanation),
    AtMost(crate::AtMostExplanation),
    AllEqual(crate::AllEqualExplanation),
    /// Exact descriptor, aggregate prior/successor state and public control/suppression facts.
    Stateful(&'a crate::StatefulStandardInspection<D>),
}
impl<D> ModuleInspection<D> {
    /// Returns the public standard law and its current or explicitly historical facts.
    /// The instance, exact descriptor and public ports remain in this inspection.
    #[must_use]
    pub fn public_behavior(&self) -> ModuleBehavior<'_, D> {
        if let Some(value) = self.exactly() {
            ModuleBehavior::Exactly(value.explanation())
        } else if let Some(value) = self.at_most() {
            ModuleBehavior::AtMost(value.explanation())
        } else if let Some(value) = self.all_equal() {
            ModuleBehavior::AllEqual(value.explanation())
        } else if let Some(value) = self.stateful_standard() {
            ModuleBehavior::Stateful(value)
        } else {
            ModuleBehavior::User
        }
    }
}

impl<D> CompiledNetwork<D> {
    /// Returns an owned stable node definition without requiring machine initialization.
    pub fn inspect_node_definition(
        &self,
        subject: NodeSubject,
    ) -> Result<NodeDef<D>, GraphQueryFailure> {
        self.node_definition(&subject)
            .cloned()
            .ok_or_else(|| GraphQueryFailure::UnknownSubject(GraphSubjectRef::node(&subject)))
    }
}
impl<D> Machine<D> {
    fn observation_context(&self) -> Result<(Time<D>, ProvenanceView<D>), InspectionFailure> {
        let at = self.now().ok_or(InspectionFailure::NotInitialized)?;
        let Some(provenance) = self.store.provenance.as_ref() else {
            panic!("ready machines must retain committed provenance");
        };
        Ok((at, provenance.clone()))
    }
    /// Inspects a containing-network node with owned committed facts and causes.
    pub fn inspect_node(&self, node: NodeKey) -> Result<NodeInspection<D>, InspectionFailure> {
        self.inspect_node_subject(NodeSubject::Node(node))
    }
    /// Inspects one qualified internal primitive without exposing its private flat key.
    pub fn inspect_qualified_node(
        &self,
        node: QualifiedNodeRef,
    ) -> Result<NodeInspection<D>, InspectionFailure> {
        self.inspect_node_subject(NodeSubject::Qualified(node))
    }
    /// Inspects either a direct or qualified primitive.
    pub fn inspect_node_subject(
        &self,
        subject: NodeSubject,
    ) -> Result<NodeInspection<D>, InspectionFailure> {
        let definition = self.compiled.inspect_node_definition(subject.clone())?;
        let graph_subject = GraphSubjectRef::node(&subject);
        let (path, key) = match &subject {
            NodeSubject::Node(key) => (&[][..], *key),
            NodeSubject::Qualified(key) => (key.instances(), key.node()),
        };
        let Some(flat) = self.compiled.resolve_stable_node(path, key) else {
            panic!("retained definitions must have compiled primitive correspondence");
        };
        let (at, provenance) = self.observation_context()?;
        let support = |index| self.store.operation_causes[index];
        let inputs: Vec<InputPortInspection> = self
            .compiled
            .input_port_operations(flat)
            .into_iter()
            .map(|(port, index)| InputPortInspection {
                port,
                level: self.store.operation_levels[index],
                current_support: (port.kind() == SignalKind::Level).then(|| support(index)),
            })
            .collect();
        let outputs: Vec<OutputPortInspection> = self
            .compiled
            .output_port_operations(flat)
            .into_iter()
            .map(|(port, index)| OutputPortInspection {
                port,
                level: self.store.operation_levels[index],
                current_support: (port.kind() == SignalKind::Level).then(|| support(index)),
            })
            .collect();
        let last_reaction_cause = self
            .compiled
            .node_operation(flat)
            .map(support)
            .unwrap_or_else(|| {
                panic!("compiled nodes must retain reaction operation correspondence")
            });
        let latest_transition = self
            .store
            .establishment_causes
            .get(&flat)
            .or_else(|| self.store.toggle_inversion_causes.get(&flat))
            .or_else(|| self.store.edge_observation_causes.get(&flat))
            .or_else(|| self.store.transport_transition_causes.get(&flat))
            .copied();
        let state = if let Some(value) = self.sample_hold_observation(flat) {
            NodeStateInspection::SampleHold(Box::new(value))
        } else if let Some(value) = self.transport_delay_observation(flat) {
            NodeStateInspection::TransportDelay(Box::new(value))
        } else if let Some(value) = self.inertial_delay_observation(flat) {
            NodeStateInspection::InertialDelay(Box::new(value))
        } else if let Some(value) = self.periodic_observation(flat) {
            NodeStateInspection::Periodic(Box::new(value))
        } else if let Some((slot, _, _)) = self.compiled.edge_state_slot(flat) {
            NodeStateInspection::Edge {
                observation: self.store.edge_observations[slot.value()],
                establishment: latest_transition,
            }
        } else if let Some((slot, _)) = self
            .compiled
            .toggle_state_slot(flat)
            .or_else(|| {
                self.compiled
                    .pulse_set_reset_state_slot(flat)
                    .map(|(slot, initial, _)| (slot, initial))
            })
            .or_else(|| {
                self.compiled
                    .level_set_reset_state_slot(flat)
                    .map(|(slot, initial, _)| (slot, initial))
            })
        {
            NodeStateInspection::StoredLevel {
                value: self.store.stored_levels[slot.value()],
                establishment: latest_transition,
            }
        } else {
            NodeStateInspection::Stateless
        };
        let pending = self
            .store
            .pending_events
            .values()
            .flatten()
            .filter(|event| event.identity().1 == flat)
            .map(|event| self.pending_observation(*event, at, &provenance))
            .collect::<Vec<_>>();
        let next_deadline = pending.first().map(|event| event.deadline);
        let active_diagnostics: Vec<ActiveDiagnosticEpisode<D>> = self
            .store
            .active_episodes
            .values()
            .filter(|episode| episode.current().primary() == &graph_subject.diagnostic_subject())
            .cloned()
            .collect();
        let current_support = outputs
            .iter()
            .any(|output| output.level.is_some())
            .then_some(last_reaction_cause);
        let mut roots = vec![last_reaction_cause];
        roots.extend(latest_transition);
        // SPEC: docs/specs/contracts/causal-explanations.yaml "authoritative-retention-boundary"
        // The observation's boundary covers every exposed fact, including inactive inputs.
        roots.extend(node_fact_causes(
            &inputs,
            &outputs,
            &state,
            &pending,
            &active_diagnostics,
        ));
        let causal = causal(&provenance, &roots)?;
        let retention = causal.retention;
        let provenance = causal.provenance;
        Ok(NodeInspection {
            subject,
            definition,
            at,
            revision: self.revision(),
            region: self.compiled.region_containing_subject(&graph_subject)?,
            inputs,
            outputs,
            state,
            pending,
            next_deadline,
            active_diagnostics,
            current_support,
            last_reaction_cause,
            latest_transition,
            retention,
            provenance,
        })
    }
    /// Inspects a typed persistent output; Pulse outputs use `inspect_output_subject`.
    pub fn inspect_output(
        &self,
        output: ExternalOutputKey<Level>,
    ) -> Result<OutputInspection<D>, InspectionFailure> {
        self.inspect_output_subject(output.into())
    }
    /// Observes output existence and persistent levels; Pulse current facts are absent.
    pub fn inspect_output_subject(
        &self,
        output: AnyExternalOutputKey,
    ) -> Result<OutputInspection<D>, InspectionFailure> {
        let Some(operation) = self.compiled.external_output_support_operation(output) else {
            return Err(GraphQueryFailure::UnknownSubject(GraphSubjectRef::root(
                GraphElement::ExternalOutput(output),
            ))
            .into());
        };
        let (at, provenance) = self.observation_context()?;
        let (level, current_support, latest_transition) = match output {
            AnyExternalOutputKey::Level(output) => (
                self.output_level(output),
                Some(self.store.operation_causes[operation]),
                self.output_cause(output),
            ),
            AnyExternalOutputKey::Pulse(_) => (None, None, None),
        };
        let roots: Vec<_> = current_support
            .into_iter()
            .chain(latest_transition)
            .collect();
        let causal = causal(&provenance, &roots)?;
        let retention = causal.retention;
        let provenance = causal.provenance;
        Ok(OutputInspection {
            output,
            at,
            revision: self.revision(),
            level,
            current_support,
            latest_transition,
            retention,
            provenance,
        })
    }
    /// Observes an existing committed pending event, including its complete payload.
    pub fn inspect_pending(
        &self,
        key: PendingEventKey,
    ) -> Result<PendingEventInspection<D>, InspectionFailure> {
        let (at, provenance) = self.observation_context()?;
        let event = self
            .store
            .pending_events
            .values()
            .flatten()
            .find(|event| event.identity().0 == key)
            .copied()
            .ok_or(InspectionFailure::UnknownPending(key))?;
        Ok(self.pending_observation(event, at, &provenance))
    }
    /// Lists owned pending work in chronological calendar order.
    pub fn inspect_pending_events(
        &self,
    ) -> Result<Vec<PendingEventInspection<D>>, InspectionFailure> {
        let (at, provenance) = self.observation_context()?;
        Ok(self
            .store
            .pending_events
            .values()
            .flatten()
            .map(|event| self.pending_observation(*event, at, &provenance))
            .collect())
    }
    fn pending_observation(
        &self,
        event: PendingEvent<D>,
        at: Time<D>,
        provenance: &ProvenanceView<D>,
    ) -> PendingEventInspection<D> {
        let (key, flat, origin, deadline, revision, cause) = event.identity();
        let owner = self.compiled.node_subject(flat);
        let Some(definition) = self.compiled.node_definition(&owner) else {
            panic!("pending owners must retain primitive definitions");
        };
        let payload = match event {
            PendingEvent::PulseDelay(event) => PendingPayload::Pulse(event.count),
            PendingEvent::TransportDelay(event) => PendingPayload::Level(event.target),
            PendingEvent::Inertial(event) => PendingPayload::Level(event.target),
            PendingEvent::Periodic(event) => PendingPayload::Periodic {
                anchor: event.anchor,
                ordinal: event.ordinal,
                first_emission: event.first_emission,
                reenable_phase: event.reenable_phase,
            },
        };
        let remaining = deadline
            .checked_duration_since(at)
            .unwrap_or_else(|_| panic!("committed pending deadlines must be strictly future"));
        let explanation = causal(provenance, &[cause]).unwrap_or_else(|_| {
            panic!("committed pending causes must resolve in retained provenance")
        });
        PendingEventInspection {
            origin_stamp: event.stimulus(),
            event: key,
            owner,
            node_kind: definition.kind().clone(),
            observed_revision: self.revision(),
            origin,
            deadline,
            remaining,
            revision,
            at,
            payload,
            cause,
            retention: explanation.retention,
            provenance: explanation.provenance,
        }
    }
    /// Explains current committed facts through node laws and recorded support.
    pub fn explain(&self, request: Explain) -> Result<Explanation<D>, InspectionFailure> {
        // SPEC: docs/specs/exhaustive_diagnostic_code_catalogue.md §33 Explanation and provenance access
        // Inspection delegates retain request evidence while explanation absence uses its own code.
        self.explain_observation(request)
            .map_err(|failure| match failure {
                InspectionFailure::Graph(GraphQueryFailure::UnknownSubject(subject)) => {
                    InspectionFailure::UnknownExplanationSubject(subject)
                }
                failure => failure,
            })
    }

    fn explain_observation(&self, request: Explain) -> Result<Explanation<D>, InspectionFailure> {
        let (observed, current_support, latest_transition) = match request {
            Explain::CurrentNode(node) => {
                let node = self.inspect_node(node)?;
                let support = node.current_support.into_iter().collect();
                let transition = node.latest_transition;
                (
                    ExplainedObservation::Node(Box::new(node)),
                    support,
                    transition,
                )
            }
            Explain::QualifiedNode(node) => {
                let node = self.inspect_qualified_node(node)?;
                let support = node.current_support.into_iter().collect();
                let transition = node.latest_transition;
                (
                    ExplainedObservation::Node(Box::new(node)),
                    support,
                    transition,
                )
            }
            Explain::CurrentOutput(output) => {
                let output = self.inspect_output_subject(output)?;
                if output.level.is_none() {
                    return Err(InspectionFailure::NoCurrentPulse(output.output));
                }
                let support = output.current_support.into_iter().collect();
                let transition = output.latest_transition;
                (ExplainedObservation::Output(output), support, transition)
            }
            Explain::Pending(event) => {
                let pending = self.inspect_pending(event)?;
                let cause = pending.cause;
                (ExplainedObservation::Pending(pending), vec![cause], None)
            }
            Explain::CurrentModule(module) => {
                return self.explain(Explain::QualifiedModule(
                    QualifiedModuleRef::from_instances(vec![module])
                        .unwrap_or_else(|| panic!("one-element module paths are non-empty")),
                ));
            }
            Explain::QualifiedModule(module) => {
                self.observation_context()?;
                let observation = self.inspect_qualified_module(module.clone()).map_err(
                    |failure| match failure {
                        crate::ModuleInspectionFailure::NotInitialized => {
                            InspectionFailure::NotInitialized
                        }
                        crate::ModuleInspectionFailure::UnknownModule(_) => {
                            GraphQueryFailure::UnknownSubject(GraphSubjectRef::qualified(
                                module.instances()[..module.instances().len() - 1].to_vec(),
                                GraphElement::Module(module.instance()),
                            ))
                            .into()
                        }
                    },
                )?;
                let support = self
                    .compiled
                    .module(&module)
                    .into_iter()
                    .flat_map(|definition| definition.outputs())
                    // SPEC: docs/specs/contracts/semantic-inspection.yaml "state-versus-history"
                    // Pulse derivations belong to explicit reaction evidence, not current support.
                    .filter(|output| output.key().kind() == SignalKind::Level)
                    .filter_map(|output| {
                        self.compiled.module_output_operation(&module, output.key())
                    })
                    .map(|operation| self.store.operation_causes[operation])
                    .collect();
                (
                    ExplainedObservation::Module(Box::new(observation)),
                    support,
                    None,
                )
            }
        };
        let (_, provenance) = self.observation_context()?;
        let mut evidence_roots = Vec::new();
        match &observed {
            ExplainedObservation::Node(node) => {
                evidence_roots.extend(node.evidence_causes());
            }
            ExplainedObservation::Module(module) => {
                for node in module.nodes() {
                    let facts = self.inspect_qualified_node(node.node().clone())?;
                    evidence_roots.extend(facts.evidence_causes());
                }
                if let Some(standard) = module.stateful_standard() {
                    evidence_roots.extend(standard.public_causes.iter().map(|(_, cause)| *cause));
                    evidence_roots.extend(standard.internal_causes.iter().map(|(_, cause)| *cause));
                    evidence_roots.extend(standard.latest_reset_cause);
                    evidence_roots.extend(standard.latest_accepted_toggle_cause);
                    evidence_roots.extend(standard.latest_capture_cause);
                }
            }
            _ => {}
        }
        evidence_roots.sort();
        evidence_roots.dedup();
        let mut roots = current_support.clone();
        roots.extend(latest_transition);
        roots.extend(evidence_roots.iter().copied());
        let causal = causal(&provenance, &roots)?;
        Ok(Explanation {
            observed,
            current_support,
            evidence_roots,
            latest_transition,
            causal,
        })
    }
}

impl<D> TransactionResult<D> {
    /// Explains one event by its position in this result's committed event stream.
    /// The returned artifact retains the event's own revision, time and provenance.
    pub fn explain_output_event(&self, index: usize) -> Result<Explanation<D>, InspectionFailure> {
        let event = self
            .output_events()
            .get(index)
            .ok_or(InspectionFailure::UnknownOutputEvent(index))?;
        let (output, at, revision, value, cause) = match event {
            crate::OutputEvent::LevelEstablished {
                output,
                value,
                stamp: at,
                revision,
                cause,
            } => (
                (*output).into(),
                *at,
                *revision,
                OutputEventValue::Established(*value),
                *cause,
            ),
            crate::OutputEvent::LevelChanged {
                output,
                from,
                to,
                stamp: at,
                revision,
                cause,
            } => (
                (*output).into(),
                *at,
                *revision,
                OutputEventValue::Changed {
                    from: *from,
                    to: *to,
                },
                *cause,
            ),
            crate::OutputEvent::Pulsed {
                output,
                count,
                stamp: at,
                revision,
                cause,
            } => (
                (*output).into(),
                *at,
                *revision,
                OutputEventValue::Pulsed(*count),
                *cause,
            ),
        };
        Ok(Explanation {
            observed: ExplainedObservation::OutputEvent {
                output,
                at,
                revision,
                value,
                cause,
            },
            current_support: Vec::new(),
            evidence_roots: Vec::new(),
            latest_transition: Some(cause),
            causal: causal(self.provenance(), &[cause])?,
        })
    }
}
impl<D> ForecastState<D> {
    /// Projects the same owned direct-node inspection from the unpublished candidate.
    pub fn inspect_node(&self, node: NodeKey) -> Result<NodeInspection<D>, InspectionFailure> {
        self.machine.inspect_node(node)
    }
    /// Projects the same qualified primitive observation from the candidate.
    pub fn inspect_qualified_node(
        &self,
        node: QualifiedNodeRef,
    ) -> Result<NodeInspection<D>, InspectionFailure> {
        self.machine.inspect_qualified_node(node)
    }
    /// Projects the same owned level-output inspection from the candidate.
    pub fn inspect_output(
        &self,
        output: ExternalOutputKey<Level>,
    ) -> Result<OutputInspection<D>, InspectionFailure> {
        self.machine.inspect_output(output)
    }
    /// Projects the same owned pending-event observation from the candidate.
    pub fn inspect_pending(
        &self,
        event: PendingEventKey,
    ) -> Result<PendingEventInspection<D>, InspectionFailure> {
        self.machine.inspect_pending(event)
    }
    /// Projects the candidate's chronological pending calendar.
    pub fn inspect_pending_events(
        &self,
    ) -> Result<Vec<PendingEventInspection<D>>, InspectionFailure> {
        self.machine.inspect_pending_events()
    }
    /// Explains the unpublished candidate using the ordinary read-only path.
    pub fn explain(&self, request: Explain) -> Result<Explanation<D>, InspectionFailure> {
        self.machine.explain(request)
    }
}
