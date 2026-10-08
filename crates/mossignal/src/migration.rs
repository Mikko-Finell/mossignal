//! Finalization of one prepared topology replacement at its effective time.
//!
//! The prepared plan is an input. This module does not invent correspondence,
//! and it does not publish. The caller installs the returned candidate only
//! after every later fallible step has succeeded.

use crate::authored::EdgeObservation;
use crate::compile::{CompiledNetwork, DigestStateFamily, DigestStateSlot};
use crate::diagnostics::{
    DiagnosticEpisodeEvidence, PendingEventEvidence, SemanticLossEvidence, SubjectRef,
};
use crate::episode::{ActiveDiagnosticEpisode, ActiveEpisodes};
use crate::key::{
    AnyExternalInputKey, AnyExternalOutputKey, ExternalInputKey, ExternalOutputKey, NodeKey,
};
use crate::machine::{NetworkRevision, PendingEvent, PendingEventKey, PendingPeriodicBoundary};
use crate::module::{NodeSubject, QualifiedNodeRef};
use crate::patch::{
    ArtifactInvalidation, ConditionalArm, Continuity, EpisodePlan, EpisodeRule, EventRule,
    ExternalInputPlan, ExternalOutputPlan, InertialDelayMigration, InputValuationPlan, LossClass,
    ModuleContinuity, ModuleMigrationDirective, NodeMigrationDirective, OutputBaselinePlan,
    OverdueMigrationPolicy, PendingArm, PendingWorkRule, PeriodicMigration, PotentialSemanticLoss,
    PreparedPatch, ProvenanceRule, PulseDelayMigration, RegionChange, StateCompatibility,
    StaticMigrationPlan, SubjectPlan, TransportDelayMigration,
};
use crate::policy::{RuntimePolicy, RuntimePolicyLimit};
use crate::signal::{Level, LogicLevel};
use crate::time::{Span, Time};
use crate::transaction::CauseRef;
use core::cmp::Ordering;
use core::marker::PhantomData;
use std::collections::{BTreeMap, BTreeSet};

/// Caller policy for realized semantic loss during one replacement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ReconfigurationPolicy {
    /// Reject the transaction when finalization realizes any semantic loss.
    RejectStateLoss,
    /// Commit when every realized loss was classified and is reported.
    AllowReportedStateLoss,
}

/// Decided fate of one state owner.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum StateOutcome {
    /// Source state was copied unchanged.
    Preserved,
    /// Source state was carried by a converting rule.
    Migrated,
    /// Target declared state replaced the source.
    Reset,
    /// The target owns state the source did not.
    Initialized,
    /// The source state left with its owner.
    Removed,
}

/// Decided fate of one pending obligation or empty temporal rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum EventOutcome {
    /// The deadline and payload were kept.
    Preserved,
    /// The deadline was rebuilt and the originating cause was kept.
    Recomputed,
    /// The payload rule was recorded and the obligation was kept.
    Transformed,
    /// The obligation was dropped.
    Canceled,
    /// A target-only rule created no source obligation.
    Added,
}

/// Decided fate of one external input valuation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum InputOutcome {
    /// The pre-patch valuation remains until a same-time target write replaces it.
    Preserved,
    /// The source valuation was moved onto the successor input.
    Inherited,
    /// The target input is established only by explicit target input.
    Established,
    /// The input left the schema.
    Removed,
}

/// Decided fate of one external output baseline.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum OutputOutcome {
    /// The pre-patch baseline remains the comparison point.
    Preserved,
    /// The source baseline is evidence for a reassociated output.
    Carried,
    /// The output has no baseline before the target reaction.
    Established,
    /// The output left the topology.
    Removed,
}

/// Decided fate of one diagnostic episode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum EpisodeOutcome {
    /// The condition and active interval were retained under the prepared correspondence.
    Preserved,
    /// The plan rewrote the episode without moving its dense owner.
    Transformed,
    /// The episode was closed as resolved.
    Resolved,
    /// The episode ended because its owner or state disappeared.
    Terminated,
    /// The plan rejected the episode.
    Rejected,
}

/// Decided fate of one provenance rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ProvenanceOutcome {
    /// Preserved state is recorded as a migration checkpoint.
    Checkpoint,
    /// Reset replaces the provenance root in the report.
    Reset,
    /// Retiming kept the obligation and recorded the patch rule.
    Retimed,
    /// Required ancestry ended.
    Lost,
}

/// One structural subject outcome.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubjectMigrationRecord {
    continuity: Continuity,
    source: Option<SubjectRef>,
    target: Option<SubjectRef>,
}

impl SubjectMigrationRecord {
    /// Returns the structural continuity.
    #[must_use]
    pub const fn continuity(&self) -> Continuity {
        self.continuity
    }

    /// Returns the base subject.
    #[must_use]
    pub const fn source(&self) -> Option<&SubjectRef> {
        self.source.as_ref()
    }

    /// Returns the target subject.
    #[must_use]
    pub const fn target(&self) -> Option<&SubjectRef> {
        self.target.as_ref()
    }
}

/// One state-owner outcome.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateMigrationRecord {
    subject: SubjectRef,
    outcome: StateOutcome,
    fact: String,
}

impl StateMigrationRecord {
    /// Returns the state owner.
    #[must_use]
    pub const fn subject(&self) -> &SubjectRef {
        &self.subject
    }

    /// Returns the decided state outcome.
    #[must_use]
    pub const fn outcome(&self) -> StateOutcome {
        self.outcome
    }

    /// Returns the state fact the plan named.
    #[must_use]
    pub fn fact(&self) -> &str {
        &self.fact
    }
}

/// One pending-work outcome.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventMigrationRecord {
    subject: SubjectRef,
    predecessor: Option<SubjectRef>,
    outcome: EventOutcome,
    origin: Option<u64>,
    deadline: Option<u64>,
    event: Option<PendingEventKey>,
    rule: String,
}

impl EventMigrationRecord {
    /// Returns the migration directive and the conditional arm actually selected.
    #[must_use]
    pub fn rule(&self) -> &str {
        &self.rule
    }
    /// Returns the obligation identity named by this outcome.
    #[must_use]
    pub const fn event(&self) -> Option<PendingEventKey> {
        self.event
    }
    /// Returns the temporal subject.
    #[must_use]
    pub const fn subject(&self) -> &SubjectRef {
        &self.subject
    }

    /// Returns the base subject when the rule migrated an existing owner.
    #[must_use]
    pub const fn predecessor(&self) -> Option<&SubjectRef> {
        self.predecessor.as_ref()
    }

    /// Returns the decided event outcome.
    #[must_use]
    pub const fn outcome(&self) -> EventOutcome {
        self.outcome
    }

    /// Returns the retained or recomputed origin when this record names one obligation.
    #[must_use]
    pub const fn origin(&self) -> Option<u64> {
        self.origin
    }

    /// Returns the deadline after migration when this record names one obligation.
    #[must_use]
    pub const fn deadline(&self) -> Option<u64> {
        self.deadline
    }
}

/// One external-input outcome.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InputMigrationRecord {
    source: Option<AnyExternalInputKey>,
    target: Option<AnyExternalInputKey>,
    outcome: InputOutcome,
}

impl InputMigrationRecord {
    /// Returns the base input.
    #[must_use]
    pub const fn source(&self) -> Option<AnyExternalInputKey> {
        self.source
    }

    /// Returns the target input.
    #[must_use]
    pub const fn target(&self) -> Option<AnyExternalInputKey> {
        self.target
    }

    /// Returns the valuation outcome.
    #[must_use]
    pub const fn outcome(&self) -> InputOutcome {
        self.outcome
    }
}

/// One external-output outcome.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutputMigrationRecord {
    source: Option<AnyExternalOutputKey>,
    target: Option<AnyExternalOutputKey>,
    outcome: OutputOutcome,
}

impl OutputMigrationRecord {
    /// Returns the base output.
    #[must_use]
    pub const fn source(&self) -> Option<AnyExternalOutputKey> {
        self.source
    }

    /// Returns the target output.
    #[must_use]
    pub const fn target(&self) -> Option<AnyExternalOutputKey> {
        self.target
    }

    /// Returns the baseline outcome.
    #[must_use]
    pub const fn outcome(&self) -> OutputOutcome {
        self.outcome
    }
}

/// One diagnostic-episode outcome.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EpisodeMigrationRecord {
    subject: SubjectRef,
    outcome: EpisodeOutcome,
}

impl EpisodeMigrationRecord {
    /// Returns the episode owner named by the plan.
    #[must_use]
    pub const fn subject(&self) -> &SubjectRef {
        &self.subject
    }

    /// Returns the decided episode outcome.
    #[must_use]
    pub const fn outcome(&self) -> EpisodeOutcome {
        self.outcome
    }
}

/// One provenance-rule outcome.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProvenanceMigrationRecord {
    subject: SubjectRef,
    outcome: ProvenanceOutcome,
}

impl ProvenanceMigrationRecord {
    /// Returns the provenance subject.
    #[must_use]
    pub const fn subject(&self) -> &SubjectRef {
        &self.subject
    }

    /// Returns the decided provenance outcome.
    #[must_use]
    pub const fn outcome(&self) -> ProvenanceOutcome {
        self.outcome
    }
}

/// One realized semantic loss.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SemanticLossRecord {
    subject: SubjectRef,
    fact: String,
    rule: String,
    conditional: bool,
    event: Option<PendingEventKey>,
}

impl SemanticLossRecord {
    /// Returns the individual temporal obligation when this loss names one.
    #[must_use]
    pub const fn event(&self) -> Option<PendingEventKey> {
        self.event
    }
    /// Returns the subject that lost the fact.
    #[must_use]
    pub const fn subject(&self) -> &SubjectRef {
        &self.subject
    }

    /// Returns the lost fact.
    #[must_use]
    pub fn fact(&self) -> &str {
        &self.fact
    }

    /// Returns the rule or removal that caused the loss.
    #[must_use]
    pub fn rule(&self) -> &str {
        &self.rule
    }

    /// Returns whether the loss depended on a pre-patch fact.
    #[must_use]
    pub const fn conditional(&self) -> bool {
        self.conditional
    }
}

/// One internal subject inside a module migration record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InternalMigrationRecord {
    role: String,
    continuity: Continuity,
    source: Option<SubjectRef>,
    target: Option<SubjectRef>,
    state: Option<StateOutcome>,
}

impl InternalMigrationRecord {
    /// Returns the stable internal role.
    #[must_use]
    pub fn role(&self) -> &str {
        &self.role
    }

    /// Returns the internal continuity.
    #[must_use]
    pub const fn continuity(&self) -> Continuity {
        self.continuity
    }

    /// Returns the base internal subject.
    #[must_use]
    pub const fn source(&self) -> Option<&SubjectRef> {
        self.source.as_ref()
    }

    /// Returns the target internal subject.
    #[must_use]
    pub const fn target(&self) -> Option<&SubjectRef> {
        self.target.as_ref()
    }

    /// Returns the decided state outcome when the internal subject owns state.
    #[must_use]
    pub const fn state(&self) -> Option<StateOutcome> {
        self.state
    }
}

/// Module continuity together with its internal outcomes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleMigrationRecord {
    continuity: Continuity,
    source: Option<crate::key::ModuleInstanceKey>,
    target: Option<crate::key::ModuleInstanceKey>,
    internals: Vec<InternalMigrationRecord>,
}

impl ModuleMigrationRecord {
    /// Returns the instance continuity.
    #[must_use]
    pub const fn continuity(&self) -> Continuity {
        self.continuity
    }

    /// Returns the base instance.
    #[must_use]
    pub const fn source(&self) -> Option<crate::key::ModuleInstanceKey> {
        self.source
    }

    /// Returns the target instance.
    #[must_use]
    pub const fn target(&self) -> Option<crate::key::ModuleInstanceKey> {
        self.target
    }

    /// Returns internal outcomes in role order.
    #[must_use]
    pub fn internals(&self) -> &[InternalMigrationRecord] {
        &self.internals
    }
}

/// Complete classification of one committed replacement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MigrationReport<D> {
    base_revision: NetworkRevision,
    target_revision: NetworkRevision,
    base_fingerprint: crate::identity::NetworkFingerprint,
    target_fingerprint: crate::identity::NetworkFingerprint,
    subjects: Vec<SubjectMigrationRecord>,
    states: Vec<StateMigrationRecord>,
    events: Vec<EventMigrationRecord>,
    inputs: Vec<InputMigrationRecord>,
    outputs: Vec<OutputMigrationRecord>,
    episodes: Vec<EpisodeMigrationRecord>,
    provenance: Vec<ProvenanceMigrationRecord>,
    losses: Vec<SemanticLossRecord>,
    invalidated: Vec<ArtifactInvalidation>,
    regions: Vec<RegionChange>,
    modules: Vec<ModuleMigrationRecord>,
    domain: PhantomData<fn() -> D>,
}

impl<D> MigrationReport<D> {
    /// Returns the revision the patch was prepared against.
    #[must_use]
    pub const fn base_revision(&self) -> NetworkRevision {
        self.base_revision
    }

    /// Returns the revision installed with this replacement.
    #[must_use]
    pub const fn target_revision(&self) -> NetworkRevision {
        self.target_revision
    }

    /// Returns the base fingerprint.
    #[must_use]
    pub const fn base_fingerprint(&self) -> crate::identity::NetworkFingerprint {
        self.base_fingerprint
    }

    /// Returns the target fingerprint.
    #[must_use]
    pub const fn target_fingerprint(&self) -> crate::identity::NetworkFingerprint {
        self.target_fingerprint
    }

    /// Returns structural subject outcomes.
    #[must_use]
    pub fn subjects(&self) -> &[SubjectMigrationRecord] {
        &self.subjects
    }

    /// Returns state outcomes, including module internals.
    #[must_use]
    pub fn states(&self) -> &[StateMigrationRecord] {
        &self.states
    }

    /// Returns pending-event outcomes.
    #[must_use]
    pub fn events(&self) -> &[EventMigrationRecord] {
        &self.events
    }

    /// Returns external-input outcomes.
    #[must_use]
    pub fn inputs(&self) -> &[InputMigrationRecord] {
        &self.inputs
    }

    /// Returns external-output outcomes.
    #[must_use]
    pub fn outputs(&self) -> &[OutputMigrationRecord] {
        &self.outputs
    }

    /// Returns diagnostic-episode outcomes.
    #[must_use]
    pub fn episodes(&self) -> &[EpisodeMigrationRecord] {
        &self.episodes
    }

    /// Returns provenance outcomes.
    #[must_use]
    pub fn provenance(&self) -> &[ProvenanceMigrationRecord] {
        &self.provenance
    }

    /// Returns realized semantic losses.
    #[must_use]
    pub fn losses(&self) -> &[SemanticLossRecord] {
        &self.losses
    }

    /// Returns stale artifact categories.
    #[must_use]
    pub fn invalidated(&self) -> &[ArtifactInvalidation] {
        &self.invalidated
    }

    /// Returns region merges and splits.
    #[must_use]
    pub fn region_changes(&self) -> &[RegionChange] {
        &self.regions
    }

    /// Returns module grouping and internal outcomes.
    #[must_use]
    pub fn modules(&self) -> &[ModuleMigrationRecord] {
        &self.modules
    }
}

/// Unpublished state produced by finalizing one prepared patch.
pub(crate) struct FinalizedPatch<D> {
    pub compiled: CompiledNetwork<D>,
    pub revision: NetworkRevision,
    pub edge_observations: Vec<EdgeObservation>,
    pub stored_levels: Vec<LogicLevel>,
    pub external_levels: BTreeMap<ExternalInputKey<Level>, LogicLevel>,
    pub output_baselines: BTreeMap<ExternalOutputKey<Level>, LogicLevel>,
    pub pending_events: BTreeMap<Time<D>, Vec<PendingEvent<D>>>,
    pub next_serial: u64,
    pub periodic_anchors: BTreeMap<NodeKey, Time<D>>,
    pub episodes: ActiveEpisodes<D>,
    pub input_causes: BTreeMap<ExternalInputKey<Level>, CauseRef>,
    pub output_causes: BTreeMap<ExternalOutputKey<Level>, CauseRef>,
    pub edge_observation_causes: BTreeMap<NodeKey, CauseRef>,
    pub toggle_inversion_causes: BTreeMap<NodeKey, CauseRef>,
    pub establishment_causes: BTreeMap<NodeKey, CauseRef>,
    pub transport_transition_causes: BTreeMap<NodeKey, CauseRef>,
    pub inertial_cancellation_causes: BTreeMap<NodeKey, CauseRef>,
    pub periodic_anchor_causes: BTreeMap<NodeKey, CauseRef>,
    pub periodic_cancellation_causes: BTreeMap<NodeKey, CauseRef>,
    pub output_plans: BTreeMap<ExternalOutputKey<Level>, OutputBaselinePlan>,
    pub declared_edges: BTreeSet<NodeKey>,
    pub report: MigrationReport<D>,
}

/// A finalization rejection before the candidate is published.
pub(crate) enum MigrationFault {
    State {
        subject: Box<SubjectRef>,
        fact: String,
        rule: String,
    },
    Pending {
        subject: Box<SubjectRef>,
        fact: String,
        rule: String,
    },
    RequirePreserve {
        subject: Box<SubjectRef>,
        fact: String,
        rule: String,
    },
    Episode {
        subject: Box<SubjectRef>,
        evidence: Box<DiagnosticEpisodeEvidence>,
    },
    Provenance {
        subject: Box<SubjectRef>,
    },
    Ambiguous {
        subject: Box<SubjectRef>,
    },
    Conflict {
        subject: Box<SubjectRef>,
        evidence: Box<PendingEventEvidence>,
    },
    Loss {
        evidence: Box<SemanticLossEvidence>,
    },
    Time {
        node: NodeSubject,
        origin_ticks: u64,
        delay_ticks: u64,
    },
    Budget {
        budget: RuntimePolicyLimit,
        limit: u64,
        consumed: u64,
    },
}

/// Runtime signal-semantics version carried by every compiled network in this process.
#[must_use]
pub(crate) fn signal_semantics_version<D>(_compiled: &CompiledNetwork<D>) -> u64 {
    // SPEC: docs/specs/contracts/atomic-topology-replacement.yaml "patch-bearing-transaction"
    // Fingerprints already embed this constant. The comparison still names it.
    1
}

/// Pre-patch state finalization reads. The caller retains ownership.
pub(crate) struct MigrationSource<'a, D> {
    pub compiled: &'a CompiledNetwork<D>,
    pub edge_observations: &'a [EdgeObservation],
    pub stored_levels: &'a [LogicLevel],
    pub operation_levels: &'a [Option<LogicLevel>],
    pub external_levels: &'a BTreeMap<ExternalInputKey<Level>, LogicLevel>,
    pub output_baselines: &'a BTreeMap<ExternalOutputKey<Level>, LogicLevel>,
    pub pending_events: &'a BTreeMap<Time<D>, Vec<PendingEvent<D>>>,
    pub next_serial: u64,
    pub periodic_anchors: &'a BTreeMap<NodeKey, Time<D>>,
    pub episodes: &'a ActiveEpisodes<D>,
    pub input_causes: &'a BTreeMap<ExternalInputKey<Level>, CauseRef>,
    pub output_causes: &'a BTreeMap<ExternalOutputKey<Level>, CauseRef>,
    pub edge_observation_causes: &'a BTreeMap<NodeKey, CauseRef>,
    pub toggle_inversion_causes: &'a BTreeMap<NodeKey, CauseRef>,
    pub establishment_causes: &'a BTreeMap<NodeKey, CauseRef>,
    pub transport_transition_causes: &'a BTreeMap<NodeKey, CauseRef>,
    pub inertial_cancellation_causes: &'a BTreeMap<NodeKey, CauseRef>,
    pub periodic_anchor_causes: &'a BTreeMap<NodeKey, CauseRef>,
    pub periodic_cancellation_causes: &'a BTreeMap<NodeKey, CauseRef>,
}

/// Applies the prepared plan to the state reached at `at`.
pub(crate) fn finalize<D>(
    prepared: &PreparedPatch<D>,
    policy: ReconfigurationPolicy,
    at: Time<D>,
    source: &MigrationSource<'_, D>,
    limits: &RuntimePolicy,
) -> Result<FinalizedPatch<D>, MigrationFault> {
    // SPEC: docs/specs/contracts/atomic-topology-replacement.yaml "finalize-prepared-plan"
    // The static plan is applied to the reached state and is not recomputed.
    let plan = prepared.static_plan();
    let target = prepared.resulting_compiled();
    let directives = directive_index(plan);
    let source_slots = slot_index(source.compiled);
    let target_slots = slot_index(target);
    let mut edges = target.initial_edge_observations();
    let mut stored = target.initial_stored_levels();
    let mut states = Vec::new();
    let mut modules = Vec::new();
    migrate_subject_states(
        plan,
        &directives,
        &source_slots,
        &target_slots,
        source,
        &mut edges,
        &mut stored,
        &mut states,
    )?;
    migrate_modules(
        plan,
        &directives,
        &source_slots,
        &target_slots,
        source,
        &mut edges,
        &mut stored,
        &mut states,
        &mut modules,
    )?;
    let MigratedEvents {
        pending,
        next_serial,
        anchors,
        records: events,
    } = migrate_events(plan, &directives, source, target, at, source.next_serial)?;
    enforce_pending_budget::<D>(limits, &pending)?;
    reject_conflicting_transitions(target, &pending)?;
    let losses = realized_losses(plan, source, at);
    if matches!(policy, ReconfigurationPolicy::RejectStateLoss) {
        if let Some(loss) = losses.first() {
            return Err(MigrationFault::Loss {
                evidence: Box::new(SemanticLossEvidence {
                    subject: loss.subject.clone(),
                    fact: loss.fact.clone(),
                    rule: loss.rule.clone(),
                    conditional: loss.conditional,
                }),
            });
        }
    }
    let (episodes, episode_records) = migrate_episodes(plan, target, source.episodes)?;
    let provenance = migrate_provenance(plan, source.compiled, target)?;
    let (external_levels, inputs) = migrate_inputs(plan, source.external_levels);
    let MigratedOutputs {
        baselines: output_baselines,
        records: outputs,
        plans: output_plans,
    } = migrate_outputs(plan, source.output_baselines);
    let mut links = node_links(plan, source.compiled, target);
    // Reset and initialization must not carry historical state causes into the new state.
    let reset_nodes = states
        .iter()
        .filter(|state| {
            matches!(
                state.outcome,
                StateOutcome::Reset | StateOutcome::Initialized
            )
        })
        .filter_map(|state| resolve_flat(target, &state.subject))
        .collect::<BTreeSet<_>>();
    links.retain(|_, target| !reset_nodes.contains(target));
    let declared_edges = declared_edge_nodes(&states, target, &target_slots);
    let mut edge_observation_causes =
        remap_node_causes(source.edge_observation_causes, &links, target);
    for node in &declared_edges {
        edge_observation_causes.remove(node);
    }
    let report = assemble_report(
        prepared,
        plan,
        states,
        events,
        inputs,
        outputs,
        episode_records,
        provenance,
        losses,
        modules,
    );
    Ok(FinalizedPatch {
        compiled: target.clone(),
        revision: prepared.proposed_revision(),
        edge_observations: edges,
        stored_levels: stored,
        external_levels,
        output_baselines,
        pending_events: pending,
        next_serial,
        periodic_anchors: anchors,
        episodes,
        input_causes: remap_input_causes(plan, source.input_causes),
        output_causes: remap_output_causes(plan, source.output_causes),
        edge_observation_causes,
        toggle_inversion_causes: remap_node_causes(source.toggle_inversion_causes, &links, target),
        establishment_causes: remap_node_causes(source.establishment_causes, &links, target),
        transport_transition_causes: remap_node_causes(
            source.transport_transition_causes,
            &links,
            target,
        ),
        inertial_cancellation_causes: remap_node_causes(
            source.inertial_cancellation_causes,
            &links,
            target,
        ),
        periodic_anchor_causes: remap_node_causes(source.periodic_anchor_causes, &links, target),
        periodic_cancellation_causes: remap_node_causes(
            source.periodic_cancellation_causes,
            &links,
            target,
        ),
        output_plans,
        declared_edges,
        report,
    })
}

fn declared_edge_nodes<D>(
    states: &[StateMigrationRecord],
    target: &CompiledNetwork<D>,
    slots: &BTreeMap<SubjectRef, SlotLoc>,
) -> BTreeSet<NodeKey> {
    // SPEC: docs/specs/contracts/atomic-topology-replacement.yaml
    // "revision-provenance-and-episodes"
    // Initialized or reset edge memory is declared state, so it has no retained cause.
    let mut nodes = BTreeSet::new();
    for state in states {
        if !matches!(
            state.outcome,
            StateOutcome::Initialized | StateOutcome::Reset
        ) {
            continue;
        }
        let Some(slot) = slots.get(&state.subject) else {
            continue;
        };
        if !matches!(slot.family, FamilyLoc::Edge(_)) {
            continue;
        }
        if let Some(node) = resolve_flat(target, &state.subject) {
            nodes.insert(node);
        }
    }
    nodes
}

struct SlotLoc {
    family: FamilyLoc,
    reset_to: Option<LogicLevel>,
}

enum FamilyLoc {
    Edge(usize),
    Stored(usize),
    Transport { remembered: usize, output: usize },
    Inertial { remembered: usize, output: usize },
    Periodic(usize),
}

fn slot_index<D>(compiled: &CompiledNetwork<D>) -> BTreeMap<SubjectRef, SlotLoc> {
    let mut index = BTreeMap::new();
    for slot in compiled.state_slots() {
        let mut location = locate(&slot);
        if !slot.owner.instances.is_empty() {
            let module = crate::QualifiedModuleRef::new(slot.owner.instances.clone());
            location.reset_to = compiled
                .module(&module)
                .and_then(|module| module.standard_declaration())
                .and_then(|declaration| {
                    declaration
                        .parameters()
                        .find_map(|parameter| match parameter.value() {
                            crate::StandardParameterValue::LogicLevel(level)
                                if parameter.key().as_str() == "reset_to" =>
                            {
                                Some(*level)
                            }
                            _ => None,
                        })
                });
        }
        index.insert(subject_of_slot(&slot), location);
    }
    index
}

fn subject_of_slot(slot: &DigestStateSlot) -> SubjectRef {
    if slot.owner.instances.is_empty() {
        SubjectRef::Node(slot.flat)
    } else {
        SubjectRef::QualifiedNode(QualifiedNodeRef::new(
            slot.owner.instances.clone(),
            slot.owner.node,
        ))
    }
}

fn locate(slot: &DigestStateSlot) -> SlotLoc {
    let family = match slot.family {
        DigestStateFamily::Edge { index } => FamilyLoc::Edge(index),
        DigestStateFamily::StoredLevel { index } => FamilyLoc::Stored(index),
        DigestStateFamily::Transport { remembered, output } => {
            FamilyLoc::Transport { remembered, output }
        }
        DigestStateFamily::Inertial { remembered, output } => {
            FamilyLoc::Inertial { remembered, output }
        }
        DigestStateFamily::Periodic { previous_enable } => FamilyLoc::Periodic(previous_enable),
    };
    SlotLoc {
        family,
        reset_to: None,
    }
}

fn directive_index<D>(
    plan: &StaticMigrationPlan<D>,
) -> BTreeMap<SubjectRef, NodeMigrationDirective<D>> {
    let mut index = BTreeMap::new();
    for subject in plan.subjects() {
        if let Some(directive) = subject.directive() {
            if let Some(key) = subject
                .target()
                .cloned()
                .or_else(|| subject.source().cloned())
            {
                index.insert(key, directive);
            }
        }
    }
    for module in plan.modules() {
        index_module_directives(module, &mut index);
    }
    index
}

fn index_module_directives<D>(
    module: &ModuleContinuity<D>,
    index: &mut BTreeMap<SubjectRef, NodeMigrationDirective<D>>,
) {
    let ModuleMigrationDirective::Explicit {
        node_overrides,
        internal_reassociations,
    } = module.directive()
    else {
        return;
    };
    for directive in node_overrides {
        for internal in module.internals() {
            if local_node(internal.source()) == Some(directive.node())
                || local_node(internal.target()) == Some(directive.node())
            {
                if let Some(subject) = internal
                    .target()
                    .cloned()
                    .or_else(|| internal.source().cloned())
                {
                    index.insert(subject, directive.migration());
                }
            }
        }
    }
    for link in internal_reassociations {
        for internal in module.internals() {
            if local_node(internal.source()) == Some(link.from())
                && local_node(internal.target()) == Some(link.to())
            {
                if let Some(subject) = internal
                    .target()
                    .cloned()
                    .or_else(|| internal.source().cloned())
                {
                    index.insert(subject, link.migration());
                }
            }
        }
    }
}

fn local_node(subject: Option<&SubjectRef>) -> Option<NodeKey> {
    match subject? {
        SubjectRef::Node(key) => Some(*key),
        SubjectRef::QualifiedNode(node) => Some(node.node()),
        _ => None,
    }
}

#[allow(clippy::too_many_arguments)]
fn migrate_subject_states<D>(
    plan: &StaticMigrationPlan<D>,
    directives: &BTreeMap<SubjectRef, NodeMigrationDirective<D>>,
    source_slots: &BTreeMap<SubjectRef, SlotLoc>,
    target_slots: &BTreeMap<SubjectRef, SlotLoc>,
    source: &MigrationSource<'_, D>,
    edges: &mut [EdgeObservation],
    stored: &mut [LogicLevel],
    states: &mut Vec<StateMigrationRecord>,
) -> Result<(), MigrationFault> {
    for subject in plan.subjects() {
        let Some(compatibility) = subject.state() else {
            if let Some(key) = subject.source() {
                if let Some(slot) = source_slots.get(key) {
                    if subject
                        .target()
                        .and_then(|key| target_slots.get(key))
                        .is_none()
                    {
                        states.push(StateMigrationRecord {
                            subject: key.clone(),
                            outcome: StateOutcome::Removed,
                            fact: removed_state_fact(slot).to_owned(),
                        });
                    }
                }
            }
            continue;
        };
        let key = subject
            .target()
            .cloned()
            .or_else(|| subject.source().cloned())
            .unwrap_or(SubjectRef::Network(source.compiled.network_key()));
        let outcome = apply_state(
            &key,
            subject.source(),
            compatibility,
            directives.get(&key).copied(),
            source_slots,
            target_slots,
            source,
            edges,
            stored,
        )?;
        states.push(StateMigrationRecord {
            subject: key,
            outcome,
            fact: state_fact(compatibility),
        });
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn migrate_modules<D>(
    plan: &StaticMigrationPlan<D>,
    directives: &BTreeMap<SubjectRef, NodeMigrationDirective<D>>,
    source_slots: &BTreeMap<SubjectRef, SlotLoc>,
    target_slots: &BTreeMap<SubjectRef, SlotLoc>,
    source: &MigrationSource<'_, D>,
    edges: &mut [EdgeObservation],
    stored: &mut [LogicLevel],
    states: &mut Vec<StateMigrationRecord>,
    modules: &mut Vec<ModuleMigrationRecord>,
) -> Result<(), MigrationFault> {
    for module in plan.modules() {
        let mut internals = Vec::new();
        for internal in module.internals() {
            let state = match internal.state() {
                Some(compatibility) => {
                    let key = internal
                        .target()
                        .cloned()
                        .or_else(|| internal.source().cloned())
                        .unwrap_or(SubjectRef::Network(source.compiled.network_key()));
                    let outcome = apply_state(
                        &key,
                        internal.source(),
                        compatibility,
                        directives.get(&key).copied(),
                        source_slots,
                        target_slots,
                        source,
                        edges,
                        stored,
                    )?;
                    states.push(StateMigrationRecord {
                        subject: key,
                        outcome,
                        fact: state_fact(compatibility),
                    });
                    Some(outcome)
                }
                None => {
                    match internal
                        .source()
                        .and_then(|key| source_slots.get(key).map(|slot| (key, slot)))
                    {
                        Some((key, slot))
                            if internal
                                .target()
                                .and_then(|key| target_slots.get(key))
                                .is_none() =>
                        {
                            states.push(StateMigrationRecord {
                                subject: key.clone(),
                                outcome: StateOutcome::Removed,
                                fact: removed_state_fact(slot).to_owned(),
                            });
                            Some(StateOutcome::Removed)
                        }
                        _ => None,
                    }
                }
            };
            internals.push(InternalMigrationRecord {
                role: internal.role().to_owned(),
                continuity: internal.continuity(),
                source: internal.source().cloned(),
                target: internal.target().cloned(),
                state,
            });
        }
        modules.push(ModuleMigrationRecord {
            continuity: module.continuity(),
            source: module.source(),
            target: module.target(),
            internals,
        });
    }
    Ok(())
}

fn state_fact(compatibility: &StateCompatibility) -> String {
    match compatibility {
        StateCompatibility::Preserve => "preserved",
        StateCompatibility::Migrate => "migrated",
        StateCompatibility::Reset => "reset",
        StateCompatibility::Reject => "reject",
        StateCompatibility::Initialize => "initialized",
        StateCompatibility::Conditional { fact, .. } => fact,
    }
    .to_owned()
}

fn removed_state_fact(slot: &SlotLoc) -> &'static str {
    match slot.family {
        FamilyLoc::Edge(_) => "edge_observation",
        FamilyLoc::Stored(_) => "stored_level",
        FamilyLoc::Transport { .. } => "transport_level",
        FamilyLoc::Inertial { .. } => "inertial_level",
        FamilyLoc::Periodic(_) => "periodic_enable",
    }
}

#[allow(clippy::too_many_arguments)]
fn apply_state<D>(
    subject: &SubjectRef,
    source_subject: Option<&SubjectRef>,
    compatibility: &StateCompatibility,
    directive: Option<NodeMigrationDirective<D>>,
    source_slots: &BTreeMap<SubjectRef, SlotLoc>,
    target_slots: &BTreeMap<SubjectRef, SlotLoc>,
    source: &MigrationSource<'_, D>,
    edges: &mut [EdgeObservation],
    stored: &mut [LogicLevel],
) -> Result<StateOutcome, MigrationFault> {
    let require_preserve = matches!(directive, Some(NodeMigrationDirective::RequirePreserve));
    let source_slot = source_subject.and_then(|subject| source_slots.get(subject));
    let target_slot = target_slots.get(subject);
    let arm = decide_state(
        source_subject.unwrap_or(subject),
        compatibility,
        source,
        source_slot.is_some(),
    )?;
    if require_preserve && !lossless(arm, source_slot.is_some()) {
        return Err(MigrationFault::RequirePreserve {
            subject: Box::new(subject.clone()),
            fact: state_fact(compatibility),
            rule: "require_preserve".to_owned(),
        });
    }
    match arm {
        ChosenState::Preserve | ChosenState::Migrate => {
            copy_state(subject, source_slot, target_slot, source, edges, stored)?;
            if matches!(
                compatibility,
                StateCompatibility::Conditional {
                    fact: "reset_to",
                    ..
                }
            ) && matches!(arm, ChosenState::Migrate)
            {
                match target_slot {
                    Some(SlotLoc {
                        family: FamilyLoc::Stored(index),
                        reset_to: Some(level),
                    }) => stored[*index] = *level,
                    _ => panic!(
                        "reset_to migration must name a standard held-state cell and target parameter"
                    ),
                }
            }
            Ok(if matches!(arm, ChosenState::Preserve) {
                StateOutcome::Preserved
            } else {
                StateOutcome::Migrated
            })
        }
        ChosenState::Reset => Ok(StateOutcome::Reset),
        ChosenState::Initialize => Ok(StateOutcome::Initialized),
        ChosenState::Reject => Err(MigrationFault::State {
            subject: Box::new(subject.clone()),
            fact: state_fact(compatibility),
            rule: "reject".to_owned(),
        }),
    }
}

fn lossless(arm: ChosenState, source_exists: bool) -> bool {
    matches!(arm, ChosenState::Preserve)
        || (matches!(arm, ChosenState::Initialize) && !source_exists)
}

#[derive(Clone, Copy)]
enum ChosenState {
    Preserve,
    Migrate,
    Reset,
    Initialize,
    Reject,
}

fn decide_state<D>(
    subject: &SubjectRef,
    compatibility: &StateCompatibility,
    source: &MigrationSource<'_, D>,
    source_exists: bool,
) -> Result<ChosenState, MigrationFault> {
    let chosen = match compatibility {
        StateCompatibility::Preserve => ChosenState::Preserve,
        StateCompatibility::Migrate => ChosenState::Migrate,
        StateCompatibility::Reset => ChosenState::Reset,
        StateCompatibility::Reject => ChosenState::Reject,
        StateCompatibility::Initialize => ChosenState::Initialize,
        StateCompatibility::Conditional {
            fact,
            when_clear,
            when_set,
        } => {
            let set = fact_is_set_at(source, subject, fact, Time::from_ticks(0));
            arm_to_state(if set { *when_set } else { *when_clear })
        }
    };
    if matches!(chosen, ChosenState::Initialize) && source_exists {
        return Ok(ChosenState::Preserve);
    }
    Ok(chosen)
}

fn arm_to_state(arm: ConditionalArm) -> ChosenState {
    match arm {
        ConditionalArm::Preserve => ChosenState::Preserve,
        ConditionalArm::Migrate => ChosenState::Migrate,
        ConditionalArm::Reset => ChosenState::Reset,
        ConditionalArm::Reject => ChosenState::Reject,
    }
}

fn copy_state<D>(
    subject: &SubjectRef,
    source_slot: Option<&SlotLoc>,
    target_slot: Option<&SlotLoc>,
    source: &MigrationSource<'_, D>,
    edges: &mut [EdgeObservation],
    stored: &mut [LogicLevel],
) -> Result<(), MigrationFault> {
    let (Some(from), Some(to)) = (source_slot, target_slot) else {
        panic!("migration plan preserves state for a subject with no compiled slot: {subject:?}");
    };
    match (&from.family, &to.family) {
        (FamilyLoc::Edge(from_index), FamilyLoc::Edge(to_index)) => {
            edges[*to_index] = observation_at(source.edge_observations, *from_index);
        }
        (FamilyLoc::Stored(from_index), FamilyLoc::Stored(to_index))
        | (FamilyLoc::Periodic(from_index), FamilyLoc::Periodic(to_index)) => {
            stored[*to_index] = level_at(source.stored_levels, *from_index);
        }
        (
            FamilyLoc::Transport {
                remembered: from_remembered,
                output: from_output,
            },
            FamilyLoc::Transport {
                remembered: to_remembered,
                output: to_output,
            },
        )
        | (
            FamilyLoc::Inertial {
                remembered: from_remembered,
                output: from_output,
            },
            FamilyLoc::Inertial {
                remembered: to_remembered,
                output: to_output,
            },
        ) => {
            stored[*to_remembered] = level_at(source.stored_levels, *from_remembered);
            stored[*to_output] = level_at(source.stored_levels, *from_output);
        }
        _ => panic!("migration plan preserves state across incompatible families: {subject:?}"),
    }
    Ok(())
}

fn observation_at(values: &[EdgeObservation], index: usize) -> EdgeObservation {
    values
        .get(index)
        .copied()
        .unwrap_or_else(|| panic!("compiled edge slot must exist in the reached state"))
}

fn level_at(values: &[LogicLevel], index: usize) -> LogicLevel {
    values
        .get(index)
        .copied()
        .unwrap_or_else(|| panic!("compiled stored-level slot must exist in the reached state"))
}

struct EventClaim {
    source: Option<NodeKey>,
    target: Option<NodeKey>,
    subject: SubjectRef,
    predecessor: Option<SubjectRef>,
    rule: PendingWorkRule,
}

struct MigratedEvents<D> {
    pending: BTreeMap<Time<D>, Vec<PendingEvent<D>>>,
    next_serial: u64,
    anchors: BTreeMap<NodeKey, Time<D>>,
    records: Vec<EventMigrationRecord>,
}

fn migrate_events<D>(
    plan: &StaticMigrationPlan<D>,
    directives: &BTreeMap<SubjectRef, NodeMigrationDirective<D>>,
    source: &MigrationSource<'_, D>,
    target: &CompiledNetwork<D>,
    at: Time<D>,
    mut next_serial: u64,
) -> Result<MigratedEvents<D>, MigrationFault> {
    let claims = claim_events(plan.event_rules(), source.compiled, target)?;
    let mut pending: BTreeMap<Time<D>, Vec<PendingEvent<D>>> = BTreeMap::new();
    let mut anchors: BTreeMap<NodeKey, Time<D>> = BTreeMap::new();
    let mut records = Vec::new();
    for claim in claims {
        let directive = directives.get(&claim.subject).copied();
        let owned = claim
            .source
            .map(|node| events_of_node(source.pending_events, node))
            .unwrap_or_default();
        let arm = decide_pending(&claim, &owned, source, at)?;
        let named_rule = directive
            .map(|directive| format!("{directive:?}/{arm:?}"))
            .unwrap_or_else(|| format!("Standard/{arm:?}"));
        let migrated =
            apply_pending_arm(&claim, &owned, arm, directive, target, at, &mut next_serial)?;
        if migrated.is_empty() {
            if owned.is_empty() {
                records.push(event_record(
                    &claim,
                    outcome_of_arm(arm),
                    None,
                    None,
                    None,
                    &named_rule,
                ));
            } else {
                for event in &owned {
                    let (key, _, origin, deadline, _, _) = event.identity();
                    records.push(event_record(
                        &claim,
                        outcome_of_arm(arm),
                        Some(origin.ticks()),
                        Some(deadline.ticks()),
                        Some(key),
                        &named_rule,
                    ));
                }
            }
        }
        for event in migrated {
            let (key, _, origin, deadline, _, _) = event.identity();
            records.push(event_record(
                &claim,
                outcome_of_arm(arm),
                Some(origin.ticks()),
                Some(deadline.ticks()),
                Some(key),
                &named_rule,
            ));
            pending.entry(deadline).or_default().push(event);
        }
        update_anchor(&claim, directive, arm, source, target, at, &mut anchors);
    }
    reject_unclaimed_events(plan.event_rules(), source)?;
    Ok(MigratedEvents {
        pending,
        next_serial,
        anchors,
        records,
    })
}

fn claim_events<D>(
    rules: &[EventRule],
    source: &CompiledNetwork<D>,
    target: &CompiledNetwork<D>,
) -> Result<Vec<EventClaim>, MigrationFault> {
    let mut seen = BTreeMap::new();
    let mut claims = Vec::new();
    for rule in rules {
        let source_subject = rule
            .predecessor()
            .cloned()
            .unwrap_or_else(|| rule.subject().clone());
        let Some(flat) = resolve_flat(source, &source_subject) else {
            claims.push(EventClaim {
                source: None,
                target: resolve_flat(target, rule.subject()),
                subject: rule.subject().clone(),
                predecessor: rule.predecessor().cloned(),
                rule: rule.rule().clone(),
            });
            continue;
        };
        if seen.insert(flat, ()).is_some() {
            return Err(MigrationFault::Ambiguous {
                subject: Box::new(source_subject),
            });
        }
        claims.push(EventClaim {
            source: Some(flat),
            target: resolve_flat(target, rule.subject()),
            subject: rule.subject().clone(),
            predecessor: rule.predecessor().cloned(),
            rule: rule.rule().clone(),
        });
    }
    Ok(claims)
}

fn reject_unclaimed_events<D>(
    rules: &[EventRule],
    source: &MigrationSource<'_, D>,
) -> Result<(), MigrationFault> {
    let mut claimed = BTreeMap::new();
    for rule in rules {
        let subject = rule.predecessor().unwrap_or(rule.subject());
        if let Some(flat) = resolve_flat(source.compiled, subject) {
            claimed.insert(flat, ());
        }
    }
    for event in source.pending_events.values().flatten() {
        let node = event.identity().1;
        if !claimed.contains_key(&node) {
            return Err(MigrationFault::Ambiguous {
                subject: Box::new(subject_of_node(source.compiled, node)),
            });
        }
    }
    Ok(())
}

fn events_of_node<D>(
    pending: &BTreeMap<Time<D>, Vec<PendingEvent<D>>>,
    node: NodeKey,
) -> Vec<PendingEvent<D>> {
    pending
        .values()
        .flatten()
        .copied()
        .filter(|event| event.identity().1 == node)
        .collect()
}

fn decide_pending<D>(
    claim: &EventClaim,
    events: &[PendingEvent<D>],
    source: &MigrationSource<'_, D>,
    at: Time<D>,
) -> Result<PendingArm, MigrationFault> {
    match &claim.rule {
        PendingWorkRule::PreserveDeadline => Ok(PendingArm::PreserveDeadline),
        PendingWorkRule::RecomputeDeadline => Ok(PendingArm::RecomputeDeadline),
        PendingWorkRule::TransformPayload => Ok(PendingArm::TransformPayload),
        PendingWorkRule::Cancel => Ok(PendingArm::Cancel),
        PendingWorkRule::Reject => {
            if events.is_empty() {
                Ok(PendingArm::Cancel)
            } else {
                Err(MigrationFault::Pending {
                    subject: Box::new(claim.subject.clone()),
                    fact: "pending".to_owned(),
                    rule: "reject".to_owned(),
                })
            }
        }
        PendingWorkRule::Conditional {
            fact,
            when_clear,
            when_set,
        } => {
            let set = fact_is_set_at(
                source,
                claim.predecessor.as_ref().unwrap_or(&claim.subject),
                fact,
                at,
            );
            let arm = if set { *when_set } else { *when_clear };
            if matches!(arm, PendingArm::Reject) && !events.is_empty() {
                return Err(MigrationFault::Pending {
                    subject: Box::new(claim.subject.clone()),
                    fact: (*fact).to_owned(),
                    rule: "reject".to_owned(),
                });
            }
            Ok(arm)
        }
    }
}

fn outcome_of_arm(arm: PendingArm) -> EventOutcome {
    match arm {
        PendingArm::PreserveDeadline => EventOutcome::Preserved,
        PendingArm::RecomputeDeadline => EventOutcome::Recomputed,
        PendingArm::TransformPayload => EventOutcome::Transformed,
        PendingArm::Cancel => EventOutcome::Canceled,
        PendingArm::Reject => EventOutcome::Canceled,
    }
}

fn event_record(
    claim: &EventClaim,
    outcome: EventOutcome,
    origin: Option<u64>,
    deadline: Option<u64>,
    event: Option<PendingEventKey>,
    rule: &str,
) -> EventMigrationRecord {
    let outcome = if claim.predecessor.is_none() && matches!(outcome, EventOutcome::Preserved) {
        EventOutcome::Added
    } else {
        outcome
    };
    EventMigrationRecord {
        subject: claim.subject.clone(),
        predecessor: claim.predecessor.clone(),
        outcome,
        origin,
        deadline,
        event,
        rule: rule.to_owned(),
    }
}

fn apply_pending_arm<D>(
    claim: &EventClaim,
    events: &[PendingEvent<D>],
    arm: PendingArm,
    directive: Option<NodeMigrationDirective<D>>,
    target: &CompiledNetwork<D>,
    at: Time<D>,
    next_serial: &mut u64,
) -> Result<Vec<PendingEvent<D>>, MigrationFault> {
    let destination = claim.target.filter(|node| target.contains_node(*node));
    match arm {
        PendingArm::Cancel | PendingArm::Reject => Ok(Vec::new()),
        PendingArm::PreserveDeadline | PendingArm::TransformPayload => {
            let Some(destination) = destination else {
                return Ok(Vec::new());
            };
            Ok(events
                .iter()
                .copied()
                .map(|event| {
                    let mut event = retarget(event, destination);
                    if matches!(
                        directive,
                        Some(NodeMigrationDirective::Periodic(
                            PeriodicMigration::PreserveNextDeadline
                        ))
                    ) {
                        if let PendingEvent::Periodic(boundary) = &mut event {
                            // SPEC: docs/specs/reconfiguration_and_topology_patch_spec.md "51.2 PreserveNextDeadline"
                            // The kept boundary is ordinal zero of the target cadence.
                            boundary.anchor = boundary.deadline;
                            boundary.ordinal = 0;
                            if let Some((_, first, phase, _)) = target.periodic(destination) {
                                boundary.first_emission = first;
                                boundary.reenable_phase = phase;
                            }
                        }
                    }
                    event
                })
                .collect())
        }
        PendingArm::RecomputeDeadline => recompute_events(
            claim,
            events,
            directive,
            destination,
            target,
            at,
            next_serial,
        ),
    }
}

fn recompute_events<D>(
    claim: &EventClaim,
    events: &[PendingEvent<D>],
    directive: Option<NodeMigrationDirective<D>>,
    destination: Option<NodeKey>,
    target: &CompiledNetwork<D>,
    at: Time<D>,
    next_serial: &mut u64,
) -> Result<Vec<PendingEvent<D>>, MigrationFault> {
    let Some(destination) = destination else {
        return Ok(Vec::new());
    };
    let mode = recompute_mode(directive, target.periodic(destination).is_some());
    match mode {
        RecomputeMode::Reanchor | RecomputeMode::FromAnchor => {
            recompute_periodic(events, destination, target, at, next_serial, mode)
        }
        RecomputeMode::Restart => events
            .iter()
            .copied()
            .map(|event| retime(event, destination, at, at, target, next_serial))
            .collect(),
        RecomputeMode::FromOrigin { overdue } => events
            .iter()
            .copied()
            .map(|event| {
                let origin = event.identity().2;
                let retimed = retime(event, destination, origin, at, target, next_serial)?;
                let deadline = retimed.identity().3;
                if deadline < at {
                    match overdue {
                        OverdueMigrationPolicy::Reject => Err(MigrationFault::Pending {
                            subject: Box::new(claim.subject.clone()),
                            fact: "overdue".to_owned(),
                            rule: "recompute".to_owned(),
                        }),
                        OverdueMigrationPolicy::MatureAtPatchTime => Ok(with_deadline(retimed, at)),
                    }
                } else {
                    Ok(retimed)
                }
            })
            .collect(),
    }
}

enum RecomputeMode {
    FromOrigin { overdue: OverdueMigrationPolicy },
    Restart,
    FromAnchor,
    Reanchor,
}

fn recompute_mode<D>(
    directive: Option<NodeMigrationDirective<D>>,
    periodic: bool,
) -> RecomputeMode {
    match directive {
        Some(
            NodeMigrationDirective::PulseDelay(PulseDelayMigration::RestartFromPatchTime)
            | NodeMigrationDirective::TransportDelay(TransportDelayMigration::RestartFromPatchTime)
            | NodeMigrationDirective::InertialDelay(InertialDelayMigration::RestartFromPatchTime),
        ) => RecomputeMode::Restart,
        Some(NodeMigrationDirective::PulseDelay(PulseDelayMigration::RecomputeFromOrigin {
            overdue,
        }))
        | Some(NodeMigrationDirective::TransportDelay(
            TransportDelayMigration::RecomputeFromOrigin { overdue },
        ))
        | Some(NodeMigrationDirective::InertialDelay(
            InertialDelayMigration::RecomputeFromOrigin { overdue },
        )) => RecomputeMode::FromOrigin { overdue },
        Some(NodeMigrationDirective::Periodic(PeriodicMigration::ReanchorAtPatchTime)) => {
            RecomputeMode::Reanchor
        }
        Some(NodeMigrationDirective::Periodic(_)) => RecomputeMode::FromAnchor,
        None if periodic => RecomputeMode::FromAnchor,
        _ => RecomputeMode::FromOrigin {
            overdue: OverdueMigrationPolicy::Reject,
        },
    }
}

fn recompute_periodic<D>(
    events: &[PendingEvent<D>],
    destination: NodeKey,
    target: &CompiledNetwork<D>,
    at: Time<D>,
    next_serial: &mut u64,
    mode: RecomputeMode,
) -> Result<Vec<PendingEvent<D>>, MigrationFault> {
    let Some((period, first, phase, _)) = target.periodic(destination) else {
        return Ok(Vec::new());
    };
    if events.is_empty() {
        return Ok(Vec::new());
    }
    let anchor = if matches!(mode, RecomputeMode::Reanchor) {
        at
    } else {
        events
            .iter()
            .find_map(|event| match event {
                PendingEvent::Periodic(event) => Some(event.anchor),
                _ => None,
            })
            .unwrap_or(at)
    };
    let deadline = if matches!(mode, RecomputeMode::Reanchor)
        && first == crate::FirstEmissionPolicy::Immediate
    {
        at
    } else {
        next_boundary(anchor, at, period.ticks(), destination, target)?
    };
    let ordinal = (deadline.ticks() - anchor.ticks()) / period.ticks();
    let cause = events[0].identity().5;
    let key = allocate_serial(next_serial)?;
    let sample = events[0];
    Ok(vec![PendingEvent::Periodic(PendingPeriodicBoundary {
        key,
        node: destination,
        origin: at,
        deadline,
        anchor,
        ordinal,
        first_emission: first,
        reenable_phase: phase,
        revision: sample.identity().4,
        cause,
    })])
}

fn next_boundary<D>(
    anchor: Time<D>,
    at: Time<D>,
    period: u64,
    node: NodeKey,
    target: &CompiledNetwork<D>,
) -> Result<Time<D>, MigrationFault> {
    let steps = if at.ticks() <= anchor.ticks() {
        1
    } else {
        let elapsed = at.ticks() - anchor.ticks();
        let quot = elapsed / period;
        if elapsed % period == 0 {
            quot.max(1)
        } else {
            quot.saturating_add(1)
        }
    };
    let offset = period
        .checked_mul(steps)
        .ok_or_else(|| time_fault(target, node, anchor.ticks(), period))?;
    anchor
        .checked_add(Span::from_ticks(offset))
        .map_err(|_| time_fault(target, node, anchor.ticks(), period))
}

fn retime<D>(
    event: PendingEvent<D>,
    destination: NodeKey,
    origin: Time<D>,
    at: Time<D>,
    target: &CompiledNetwork<D>,
    next_serial: &mut u64,
) -> Result<PendingEvent<D>, MigrationFault> {
    let delay = delay_of(target, destination)
        .ok_or_else(|| time_fault(target, destination, origin.ticks(), 0))?;
    let deadline = origin
        .checked_add(Span::from_ticks(delay))
        .map_err(|_| time_fault(target, destination, origin.ticks(), delay))?;
    let _ = (at, next_serial);
    Ok(with_schedule(
        retarget(event, destination),
        origin,
        deadline,
    ))
}

fn delay_of<D>(compiled: &CompiledNetwork<D>, node: NodeKey) -> Option<u64> {
    compiled
        .pulse_delay(node)
        .map(|span| span.ticks())
        .or_else(|| {
            compiled
                .transport_delay(node)
                .map(|(span, ..)| span.ticks())
        })
        .or_else(|| compiled.inertial_delay(node).map(|(span, ..)| span.ticks()))
        .or_else(|| compiled.periodic(node).map(|(span, ..)| span.ticks()))
}

fn allocate_serial(next_serial: &mut u64) -> Result<PendingEventKey, MigrationFault> {
    let serial = *next_serial;
    *next_serial = next_serial.checked_add(1).ok_or(MigrationFault::Budget {
        budget: RuntimePolicyLimit::MaxPendingEvents,
        limit: u64::MAX,
        consumed: u64::MAX,
    })?;
    Ok(PendingEventKey::from_serial(serial))
}

fn retarget<D>(event: PendingEvent<D>, node: NodeKey) -> PendingEvent<D> {
    match event {
        PendingEvent::PulseDelay(mut event) => {
            event.node = node;
            PendingEvent::PulseDelay(event)
        }
        PendingEvent::TransportDelay(mut event) => {
            event.node = node;
            PendingEvent::TransportDelay(event)
        }
        PendingEvent::Inertial(mut event) => {
            event.node = node;
            PendingEvent::Inertial(event)
        }
        PendingEvent::Periodic(mut event) => {
            event.node = node;
            PendingEvent::Periodic(event)
        }
    }
}

fn with_schedule<D>(event: PendingEvent<D>, origin: Time<D>, deadline: Time<D>) -> PendingEvent<D> {
    match event {
        PendingEvent::PulseDelay(mut event) => {
            event.origin = origin;
            event.deadline = deadline;
            PendingEvent::PulseDelay(event)
        }
        PendingEvent::TransportDelay(mut event) => {
            event.origin = origin;
            event.deadline = deadline;
            PendingEvent::TransportDelay(event)
        }
        PendingEvent::Inertial(mut event) => {
            event.origin = origin;
            event.deadline = deadline;
            PendingEvent::Inertial(event)
        }
        PendingEvent::Periodic(mut event) => {
            event.origin = origin;
            event.deadline = deadline;
            PendingEvent::Periodic(event)
        }
    }
}

fn with_deadline<D>(event: PendingEvent<D>, deadline: Time<D>) -> PendingEvent<D> {
    let origin = event.identity().2;
    with_schedule(event, origin, deadline)
}

fn update_anchor<D>(
    claim: &EventClaim,
    directive: Option<NodeMigrationDirective<D>>,
    arm: PendingArm,
    source: &MigrationSource<'_, D>,
    target: &CompiledNetwork<D>,
    at: Time<D>,
    anchors: &mut BTreeMap<NodeKey, Time<D>>,
) {
    let Some(destination) = claim.target.filter(|node| target.periodic(*node).is_some()) else {
        return;
    };
    if matches!(arm, PendingArm::Cancel | PendingArm::Reject)
        || matches!(directive, Some(NodeMigrationDirective::Reset))
    {
        anchors.remove(&destination);
        return;
    }
    if matches!(
        directive,
        Some(NodeMigrationDirective::Periodic(
            PeriodicMigration::ReanchorAtPatchTime
        ))
    ) {
        anchors.insert(destination, at);
        return;
    }
    if matches!(
        directive,
        Some(NodeMigrationDirective::Periodic(
            PeriodicMigration::PreserveNextDeadline
        ))
    ) {
        if let Some(deadline) = claim.source.and_then(|node| {
            events_of_node(source.pending_events, node)
                .first()
                .map(|event| event.identity().3)
        }) {
            anchors.insert(destination, deadline);
            return;
        }
    }
    if let Some(anchor) = claim
        .source
        .and_then(|node| source.periodic_anchors.get(&node).copied())
    {
        anchors.insert(destination, anchor);
    }
}

fn time_fault<D>(
    compiled: &CompiledNetwork<D>,
    node: NodeKey,
    origin: u64,
    delay: u64,
) -> MigrationFault {
    MigrationFault::Time {
        node: compiled.node_subject(node),
        origin_ticks: origin,
        delay_ticks: delay,
    }
}

fn enforce_pending_budget<D>(
    limits: &RuntimePolicy,
    pending: &BTreeMap<Time<D>, Vec<PendingEvent<D>>>,
) -> Result<(), MigrationFault> {
    let consumed = pending
        .values()
        .map(|batch| batch.len())
        .try_fold(0_u64, |total, count| {
            total.checked_add(u64::try_from(count).unwrap_or(u64::MAX))
        })
        .unwrap_or(u64::MAX);
    let limit = limits.max_pending_events();
    if consumed > limit {
        return Err(MigrationFault::Budget {
            budget: RuntimePolicyLimit::MaxPendingEvents,
            limit,
            consumed,
        });
    }
    Ok(())
}

fn reject_conflicting_transitions<D>(
    target: &CompiledNetwork<D>,
    pending: &BTreeMap<Time<D>, Vec<PendingEvent<D>>>,
) -> Result<(), MigrationFault> {
    for batch in pending.values() {
        let mut transport = BTreeMap::new();
        let mut inertial = BTreeMap::new();
        for event in batch {
            let node = event.identity().1;
            let duplicate = match event {
                PendingEvent::TransportDelay(_) => transport.insert(node, ()).is_some(),
                PendingEvent::Inertial(_) => inertial.insert(node, ()).is_some(),
                _ => false,
            };
            if duplicate {
                return Err(MigrationFault::Conflict {
                    subject: Box::new(subject_of_node(target, node)),
                    evidence: Box::new(PendingEventEvidence {
                        event: Some(event.identity().0.value()),
                        owner: format!("{:032x}", node.as_u128()),
                        origin: Some(event.identity().2.ticks()),
                        deadline: Some(event.identity().3.ticks()),
                        detail: "migrated transitions share one owner and deadline".to_owned(),
                    }),
                });
            }
        }
    }
    Ok(())
}

fn subject_of_node<D>(compiled: &CompiledNetwork<D>, node: NodeKey) -> SubjectRef {
    match compiled.node_subject(node) {
        NodeSubject::Node(node) => SubjectRef::Node(node),
        NodeSubject::Qualified(node) => SubjectRef::QualifiedNode(node),
    }
}

fn resolve_flat<D>(compiled: &CompiledNetwork<D>, subject: &SubjectRef) -> Option<NodeKey> {
    match subject {
        SubjectRef::Node(key) if compiled.contains_node(*key) => Some(*key),
        SubjectRef::QualifiedNode(node) => {
            compiled.resolve_stable_node(node.instances(), node.node())
        }
        _ => None,
    }
}

fn realized_losses<D>(
    plan: &StaticMigrationPlan<D>,
    source: &MigrationSource<'_, D>,
    at: Time<D>,
) -> Vec<SemanticLossRecord> {
    // SPEC: docs/specs/contracts/atomic-topology-replacement.yaml "explicit-loss-policy"
    // Actual loss is the prepared loss whose pre-patch fact is realized.
    let mut losses = Vec::new();
    for loss in plan.potential_losses() {
        if loss_is_realized(loss, source, at) {
            let events = if matches!(
                loss.fact(),
                "pending_group"
                    | "pending_transition"
                    | "inertial_candidate"
                    | "periodic_schedule"
                    | "elapsed_wait"
                    | "elapsed_qualification"
            ) {
                resolve_flat(source.compiled, loss.subject())
                    .map(|node| events_of_node(source.pending_events, node))
                    .unwrap_or_default()
            } else {
                Vec::new()
            };
            let keys = if events.is_empty() {
                vec![None]
            } else {
                events
                    .iter()
                    .map(|event| Some(event.identity().0))
                    .collect()
            };
            for event in keys {
                losses.push(SemanticLossRecord {
                    subject: loss.subject().clone(),
                    fact: loss.fact().to_owned(),
                    rule: loss.rule().to_owned(),
                    conditional: matches!(loss.class(), LossClass::Conditional),
                    event,
                });
            }
        }
    }
    for rule in plan.episodes() {
        if matches!(rule.rule(), EpisodeRule::Terminate)
            && !matching_episodes(source.episodes, rule.subject()).is_empty()
        {
            losses.push(SemanticLossRecord {
                subject: rule.subject().clone(),
                fact: "diagnostic_episode".to_owned(),
                rule: "terminate".to_owned(),
                conditional: true,
                event: None,
            });
        }
    }
    losses.sort_by(loss_order);
    losses
}

fn loss_is_realized<D>(
    loss: &PotentialSemanticLoss,
    source: &MigrationSource<'_, D>,
    at: Time<D>,
) -> bool {
    match loss.class() {
        LossClass::Unavoidable => true,
        LossClass::Conditional => fact_is_set_at(source, loss.subject(), loss.fact(), at),
    }
}

fn fact_is_set_at<D>(
    source: &MigrationSource<'_, D>,
    subject: &SubjectRef,
    fact: &str,
    at: Time<D>,
) -> bool {
    let flat = resolve_flat(source.compiled, subject);
    let events = flat
        .map(|node| events_of_node(source.pending_events, node))
        .unwrap_or_default();
    match fact {
        "level_valuation" => match subject {
            SubjectRef::ExternalInput(AnyExternalInputKey::Level(key)) => {
                source.external_levels.contains_key(key)
            }
            _ => false,
        },
        "reset_to" => match subject {
            SubjectRef::QualifiedNode(node) => {
                let module = crate::QualifiedModuleRef::new(node.instances().to_vec());
                source
                    .compiled
                    .module_input_operation(
                        &module,
                        crate::standard::stateful::level_resettable_sample_hold_reset_key().into(),
                    )
                    .and_then(|index| source.operation_levels.get(index).copied().flatten())
                    .is_some_and(LogicLevel::is_high)
            }
            _ => false,
        },
        "level_baseline" => match subject {
            SubjectRef::ExternalOutput(AnyExternalOutputKey::Level(key)) => {
                source.output_baselines.contains_key(key)
            }
            _ => false,
        },
        "pending_group" => events
            .iter()
            .any(|event| matches!(event, PendingEvent::PulseDelay(_))),
        "pending_transition" => events
            .iter()
            .any(|event| matches!(event, PendingEvent::TransportDelay(_))),
        "inertial_candidate" => events
            .iter()
            .any(|event| matches!(event, PendingEvent::Inertial(_))),
        "periodic_schedule" => events
            .iter()
            .any(|event| matches!(event, PendingEvent::Periodic(_))),
        "periodic_anchor" => flat.is_some_and(|node| source.periodic_anchors.contains_key(&node)),
        "elapsed_wait" | "elapsed_qualification" => {
            events.iter().any(|event| event.identity().2 < at)
        }
        other => panic!("migration plan names an unclassified conditional fact: {other}"),
    }
}

fn loss_order(left: &SemanticLossRecord, right: &SemanticLossRecord) -> Ordering {
    left.subject
        .cmp_canonical(&right.subject)
        .then_with(|| left.event.cmp(&right.event))
        .then_with(|| left.fact.cmp(&right.fact))
        .then_with(|| left.rule.cmp(&right.rule))
}

fn migrate_episodes<D>(
    plan: &StaticMigrationPlan<D>,
    target: &CompiledNetwork<D>,
    episodes: &ActiveEpisodes<D>,
) -> Result<(ActiveEpisodes<D>, Vec<EpisodeMigrationRecord>), MigrationFault> {
    let mut kept = BTreeMap::new();
    let mut records = Vec::new();
    for rule in plan.episodes() {
        let matched = matching_episodes(episodes, rule.subject());
        let outcome = apply_episode_rule(rule, &matched)?;
        if matches!(
            outcome,
            EpisodeOutcome::Preserved | EpisodeOutcome::Transformed
        ) {
            for (_, episode) in matched {
                let successor = successor_subject(plan, rule.subject()).unwrap_or(rule.subject());
                let owner = match successor {
                    SubjectRef::Node(node) => NodeSubject::Node(*node),
                    SubjectRef::QualifiedNode(node) => NodeSubject::Qualified(node.clone()),
                    _ => panic!("episode plan must identify one primitive owner"),
                };
                let migrated = episode.migrate_owner(target.network_key(), owner);
                kept.insert(migrated.condition().clone(), migrated);
            }
        }
        records.push(EpisodeMigrationRecord {
            subject: rule.subject().clone(),
            outcome,
        });
    }
    Ok((kept, records))
}

fn successor_subject<'a, D>(
    plan: &'a StaticMigrationPlan<D>,
    source: &SubjectRef,
) -> Option<&'a SubjectRef> {
    plan.subjects()
        .iter()
        .find(|subject| subject.source() == Some(source))
        .and_then(SubjectPlan::target)
        .or_else(|| {
            plan.modules()
                .iter()
                .flat_map(|module| module.internals())
                .find(|internal| internal.source() == Some(source))
                .and_then(|internal| internal.target())
        })
}

fn predecessor_subject<'a, D>(
    plan: &'a StaticMigrationPlan<D>,
    target: &SubjectRef,
) -> Option<&'a SubjectRef> {
    plan.subjects()
        .iter()
        .find(|subject| subject.target() == Some(target))
        .and_then(SubjectPlan::source)
        .or_else(|| {
            plan.modules()
                .iter()
                .flat_map(|module| module.internals())
                .find(|internal| internal.target() == Some(target))
                .and_then(|internal| internal.source())
        })
}

fn state_source<'a, D>(
    plan: &'a StaticMigrationPlan<D>,
    record: &'a StateMigrationRecord,
) -> Option<&'a SubjectRef> {
    match record.outcome {
        StateOutcome::Initialized => None,
        StateOutcome::Removed => Some(&record.subject),
        _ => predecessor_subject(plan, &record.subject).or(Some(&record.subject)),
    }
}

fn matching_episodes<'a, D>(
    episodes: &'a ActiveEpisodes<D>,
    subject: &SubjectRef,
) -> Vec<(
    &'a crate::episode::DiagnosticConditionKey,
    &'a ActiveDiagnosticEpisode<D>,
)> {
    episodes
        .iter()
        .filter(|(key, _)| &episode_subject(key.owner()) == subject)
        .collect()
}

fn episode_subject(owner: &NodeSubject) -> SubjectRef {
    match owner {
        NodeSubject::Node(node) => SubjectRef::Node(*node),
        NodeSubject::Qualified(node) => SubjectRef::QualifiedNode(node.clone()),
    }
}

fn apply_episode_rule<D>(
    rule: &EpisodePlan,
    matched: &[(
        &crate::episode::DiagnosticConditionKey,
        &ActiveDiagnosticEpisode<D>,
    )],
) -> Result<EpisodeOutcome, MigrationFault> {
    if matches!(rule.rule(), EpisodeRule::Reject) && !matched.is_empty() {
        let (_, episode) = matched[0];
        return Err(MigrationFault::Episode {
            subject: Box::new(rule.subject().clone()),
            evidence: Box::new(DiagnosticEpisodeEvidence {
                identity: hex_bytes(&episode.identity().as_bytes()),
                code: episode.condition().code().as_str().to_owned(),
                owner: owner_text(episode.condition().owner()),
                discriminator: Some(u64::from(episode.condition().discriminator())),
                began_at: Some(episode.began_at().ticks()),
                last_material_change: Some(episode.last_material_change().ticks()),
            }),
        });
    }
    Ok(match rule.rule() {
        EpisodeRule::Preserve => EpisodeOutcome::Preserved,
        EpisodeRule::Transform => EpisodeOutcome::Transformed,
        EpisodeRule::Resolve => EpisodeOutcome::Resolved,
        EpisodeRule::Terminate => EpisodeOutcome::Terminated,
        EpisodeRule::Reject => EpisodeOutcome::Rejected,
    })
}

fn migrate_provenance<D>(
    plan: &StaticMigrationPlan<D>,
    source: &CompiledNetwork<D>,
    target: &CompiledNetwork<D>,
) -> Result<Vec<ProvenanceMigrationRecord>, MigrationFault> {
    let mut records = Vec::new();
    for rule in plan.provenance() {
        let resolved = resolve_flat(source, rule.subject()).is_some()
            || resolve_flat(target, rule.subject()).is_some();
        if !resolved && !matches!(rule.rule(), ProvenanceRule::Loss) {
            if matches!(
                rule.subject(),
                SubjectRef::Node(_) | SubjectRef::QualifiedNode(_)
            ) {
                panic!(
                    "migration plan names a provenance root neither compiled network contains: {:?}",
                    rule.subject()
                );
            }
            return Err(MigrationFault::Provenance {
                subject: Box::new(rule.subject().clone()),
            });
        }
        records.push(ProvenanceMigrationRecord {
            subject: rule.subject().clone(),
            outcome: match rule.rule() {
                ProvenanceRule::Checkpoint => ProvenanceOutcome::Checkpoint,
                ProvenanceRule::Reset => ProvenanceOutcome::Reset,
                ProvenanceRule::Retime => ProvenanceOutcome::Retimed,
                ProvenanceRule::Loss => ProvenanceOutcome::Lost,
            },
        });
    }
    Ok(records)
}

fn migrate_inputs<D>(
    plan: &StaticMigrationPlan<D>,
    levels: &BTreeMap<ExternalInputKey<Level>, LogicLevel>,
) -> (
    BTreeMap<ExternalInputKey<Level>, LogicLevel>,
    Vec<InputMigrationRecord>,
) {
    let mut migrated = BTreeMap::new();
    let mut records = Vec::new();
    for input in plan.external_inputs() {
        let outcome = match input.valuation() {
            InputValuationPlan::Preserve => InputOutcome::Preserved,
            InputValuationPlan::Inherit => InputOutcome::Inherited,
            InputValuationPlan::Establish => InputOutcome::Established,
            InputValuationPlan::Remove => InputOutcome::Removed,
        };
        copy_input_level(input, levels, &mut migrated);
        records.push(InputMigrationRecord {
            source: input.source(),
            target: input.target(),
            outcome,
        });
    }
    (migrated, records)
}

fn copy_input_level(
    input: &ExternalInputPlan,
    levels: &BTreeMap<ExternalInputKey<Level>, LogicLevel>,
    migrated: &mut BTreeMap<ExternalInputKey<Level>, LogicLevel>,
) {
    let Some(AnyExternalInputKey::Level(source)) = input.source() else {
        return;
    };
    let Some(value) = levels.get(&source).copied() else {
        return;
    };
    match input.valuation() {
        InputValuationPlan::Preserve => {
            migrated.insert(source, value);
        }
        InputValuationPlan::Inherit => {
            if let Some(AnyExternalInputKey::Level(target)) = input.target() {
                migrated.insert(target, value);
            }
        }
        InputValuationPlan::Establish | InputValuationPlan::Remove => {}
    }
}

struct MigratedOutputs {
    baselines: BTreeMap<ExternalOutputKey<Level>, LogicLevel>,
    records: Vec<OutputMigrationRecord>,
    plans: BTreeMap<ExternalOutputKey<Level>, OutputBaselinePlan>,
}

fn migrate_outputs<D>(
    plan: &StaticMigrationPlan<D>,
    baselines: &BTreeMap<ExternalOutputKey<Level>, LogicLevel>,
) -> MigratedOutputs {
    let mut migrated = BTreeMap::new();
    let mut plans = BTreeMap::new();
    let mut records = Vec::new();
    for output in plan.external_outputs() {
        let outcome = match output.baseline() {
            OutputBaselinePlan::Preserve => OutputOutcome::Preserved,
            OutputBaselinePlan::CarryAsEvidence => OutputOutcome::Carried,
            OutputBaselinePlan::Establish => OutputOutcome::Established,
            OutputBaselinePlan::Remove => OutputOutcome::Removed,
        };
        if let Some(AnyExternalOutputKey::Level(target)) = output.target() {
            plans.insert(target, output.baseline());
        }
        copy_output_baseline(output, baselines, &mut migrated);
        records.push(OutputMigrationRecord {
            source: output.source(),
            target: output.target(),
            outcome,
        });
    }
    MigratedOutputs {
        baselines: migrated,
        records,
        plans,
    }
}

fn copy_output_baseline(
    output: &ExternalOutputPlan,
    baselines: &BTreeMap<ExternalOutputKey<Level>, LogicLevel>,
    migrated: &mut BTreeMap<ExternalOutputKey<Level>, LogicLevel>,
) {
    let Some(AnyExternalOutputKey::Level(source)) = output.source() else {
        return;
    };
    let Some(value) = baselines.get(&source).copied() else {
        return;
    };
    match output.baseline() {
        OutputBaselinePlan::Preserve => {
            migrated.insert(source, value);
        }
        OutputBaselinePlan::CarryAsEvidence => {
            if let Some(AnyExternalOutputKey::Level(target)) = output.target() {
                migrated.insert(target, value);
            }
        }
        OutputBaselinePlan::Establish | OutputBaselinePlan::Remove => {}
    }
}

fn node_links<D>(
    plan: &StaticMigrationPlan<D>,
    source: &CompiledNetwork<D>,
    target: &CompiledNetwork<D>,
) -> BTreeMap<NodeKey, NodeKey> {
    let mut links = BTreeMap::new();
    for subject in plan.subjects() {
        let (Some(from_subject), Some(to_subject)) = (subject.source(), subject.target()) else {
            continue;
        };
        let (Some(from), Some(to)) = (
            resolve_flat(source, from_subject),
            resolve_flat(target, to_subject),
        ) else {
            continue;
        };
        links.insert(from, to);
    }
    for module in plan.modules() {
        for internal in module.internals() {
            let (Some(from_subject), Some(to_subject)) = (internal.source(), internal.target())
            else {
                continue;
            };
            let (Some(from), Some(to)) = (
                resolve_flat(source, from_subject),
                resolve_flat(target, to_subject),
            ) else {
                continue;
            };
            links.insert(from, to);
        }
    }
    links
}

fn remap_node_causes(
    causes: &BTreeMap<NodeKey, CauseRef>,
    links: &BTreeMap<NodeKey, NodeKey>,
    target: &CompiledNetwork<impl Sized>,
) -> BTreeMap<NodeKey, CauseRef> {
    let mut remapped = BTreeMap::new();
    for (node, cause) in causes {
        let Some(destination) = links.get(node).copied() else {
            continue;
        };
        if target.contains_node(destination) {
            remapped.insert(destination, *cause);
        }
    }
    remapped
}

fn remap_input_causes(
    plan: &StaticMigrationPlan<impl Sized>,
    causes: &BTreeMap<ExternalInputKey<Level>, CauseRef>,
) -> BTreeMap<ExternalInputKey<Level>, CauseRef> {
    let mut remapped = BTreeMap::new();
    for input in plan.external_inputs() {
        let Some(AnyExternalInputKey::Level(source)) = input.source() else {
            continue;
        };
        let Some(cause) = causes.get(&source).copied() else {
            continue;
        };
        match input.valuation() {
            InputValuationPlan::Preserve => {
                remapped.insert(source, cause);
            }
            InputValuationPlan::Inherit => {
                if let Some(AnyExternalInputKey::Level(target)) = input.target() {
                    remapped.insert(target, cause);
                }
            }
            InputValuationPlan::Establish | InputValuationPlan::Remove => {}
        }
    }
    remapped
}

fn remap_output_causes(
    plan: &StaticMigrationPlan<impl Sized>,
    causes: &BTreeMap<ExternalOutputKey<Level>, CauseRef>,
) -> BTreeMap<ExternalOutputKey<Level>, CauseRef> {
    let mut remapped = BTreeMap::new();
    for output in plan.external_outputs() {
        let Some(AnyExternalOutputKey::Level(source)) = output.source() else {
            continue;
        };
        let Some(cause) = causes.get(&source).copied() else {
            continue;
        };
        match output.baseline() {
            OutputBaselinePlan::Preserve => {
                remapped.insert(source, cause);
            }
            OutputBaselinePlan::CarryAsEvidence => {
                if let Some(AnyExternalOutputKey::Level(target)) = output.target() {
                    remapped.insert(target, cause);
                }
            }
            OutputBaselinePlan::Establish | OutputBaselinePlan::Remove => {}
        }
    }
    remapped
}

#[allow(clippy::too_many_arguments)]
fn assemble_report<D>(
    prepared: &PreparedPatch<D>,
    plan: &StaticMigrationPlan<D>,
    mut states: Vec<StateMigrationRecord>,
    mut events: Vec<EventMigrationRecord>,
    mut inputs: Vec<InputMigrationRecord>,
    mut outputs: Vec<OutputMigrationRecord>,
    mut episodes: Vec<EpisodeMigrationRecord>,
    mut provenance: Vec<ProvenanceMigrationRecord>,
    losses: Vec<SemanticLossRecord>,
    modules: Vec<ModuleMigrationRecord>,
) -> MigrationReport<D> {
    let mut subjects = plan
        .subjects()
        .iter()
        .map(subject_record)
        .collect::<Vec<_>>();
    sort_optional(&mut subjects, |record| {
        (record.source.as_ref(), record.target.as_ref())
    });
    // SPEC: docs/specs/contracts/atomic-topology-replacement.yaml "migration-report"
    // Source identity orders migrated facts even when correspondence changes target key order.
    states.sort_by(|left, right| {
        cmp_optional(state_source(plan, left), state_source(plan, right))
            .then_with(|| left.subject.cmp_canonical(&right.subject))
            .then_with(|| left.fact.cmp(&right.fact))
    });
    events.sort_by(|left, right| {
        let source = |record: &EventMigrationRecord| {
            record.predecessor.clone().or_else(|| {
                (!matches!(record.outcome, EventOutcome::Added)).then(|| record.subject.clone())
            })
        };
        cmp_optional(source(left).as_ref(), source(right).as_ref())
            .then_with(|| left.subject.cmp_canonical(&right.subject))
            .then_with(|| left.event.cmp(&right.event))
            .then_with(|| left.origin.cmp(&right.origin))
            .then_with(|| left.deadline.cmp(&right.deadline))
            .then_with(|| left.rule.cmp(&right.rule))
    });
    inputs.sort_by(|left, right| {
        left.source
            .cmp(&right.source)
            .then(left.target.cmp(&right.target))
    });
    outputs.sort_by(|left, right| {
        left.source
            .cmp(&right.source)
            .then(left.target.cmp(&right.target))
    });
    episodes.sort_by(|left, right| left.subject.cmp_canonical(&right.subject));
    provenance.sort_by(|left, right| left.subject.cmp_canonical(&right.subject));
    MigrationReport {
        base_revision: prepared.base_revision(),
        target_revision: prepared.proposed_revision(),
        base_fingerprint: prepared.base_fingerprint(),
        target_fingerprint: prepared.resulting_fingerprint(),
        subjects,
        states,
        events,
        inputs,
        outputs,
        episodes,
        provenance,
        losses,
        invalidated: plan.invalidated().to_vec(),
        regions: plan.region_changes().to_vec(),
        modules,
        domain: PhantomData,
    }
}

fn subject_record<D>(plan: &SubjectPlan<D>) -> SubjectMigrationRecord {
    SubjectMigrationRecord {
        continuity: plan.continuity(),
        source: plan.source().cloned(),
        target: plan.target().cloned(),
    }
}

fn sort_optional<T>(
    records: &mut [T],
    key: impl Fn(&T) -> (Option<&SubjectRef>, Option<&SubjectRef>),
) {
    records.sort_by(|left, right| {
        let (left_source, left_target) = key(left);
        let (right_source, right_target) = key(right);
        cmp_optional(left_source, right_source).then(cmp_optional(left_target, right_target))
    });
}

fn cmp_optional(left: Option<&SubjectRef>, right: Option<&SubjectRef>) -> Ordering {
    match (left, right) {
        (None, None) => Ordering::Equal,
        (None, Some(_)) => Ordering::Less,
        (Some(_), None) => Ordering::Greater,
        (Some(left), Some(right)) => left.cmp_canonical(right),
    }
}

fn hex_bytes(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn owner_text(owner: &NodeSubject) -> String {
    match owner {
        NodeSubject::Node(node) => format!("{:032x}", node.as_u128()),
        NodeSubject::Qualified(node) => format!("{:032x}", node.node().as_u128()),
    }
}
