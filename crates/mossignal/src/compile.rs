//! Immutable dense execution topology for validated restricted networks.

use crate::authored::{
    ConnectionDef, ConnectionEndpoint, ExternalInputDef, ExternalOutputDef, InputPortRole,
    ModuleInstanceDef, ModuleInterfaceMapping, NodeDef, NodeKind, NodePorts, UncheckedNetwork,
};
use crate::diagnostics::{DiagnosticSet, Report};
use crate::identity::{InputSchemaFingerprint, NetworkFingerprint, TimeDomainId};
use crate::input::{InputDeltaBuilder, InputSnapshotBuilder};
use crate::key::{
    AnyExternalInputKey, AnyExternalOutputKey, AnyInPortKey, AnyOutPortKey, ConnectionKey,
    ExternalInputKey, ExternalOutputKey, InPortKey, ModuleInstanceKey, NetworkKey, NodeKey,
    OutPortKey,
};
use crate::machine::Machine;
use crate::module::{
    ModuleDef, NodeSubject, PulsePortSubject, QualifiedConnectionRef, QualifiedInPortRef,
    QualifiedModuleRef, QualifiedNodeRef,
};
use crate::policy::RuntimePolicy;
use crate::signal::{Level, LogicLevel, Pulse, PulseCount, SignalKind};
use crate::time::NonZeroSpan;
use crate::validation::{
    NetworkDefinitionGraphView, ReactionDependencyGraph, ReactionVertex, ValidatedNetwork,
    compilation_reaction_graph,
};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

/// An immutable executable topology compiled from a validated network.
///
/// The public value deliberately exposes only stable identity. Dense execution
/// positions and adjacency remain private to this compiled revision.
#[derive(Debug)]
pub struct CompiledNetwork<D> {
    inner: Arc<CompiledInner<D>>,
}

impl<D> Clone for CompiledNetwork<D> {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}

#[derive(Debug)]
struct CompiledInner<D> {
    definition: UncheckedNetwork<D>,
    fingerprint: NetworkFingerprint,
    input_schema_fingerprint: InputSchemaFingerprint,
    nodes: Vec<NodeDescriptor>,
    ports: Vec<PortDescriptor>,
    external_inputs: Vec<ExternalInputDescriptor>,
    external_outputs: Vec<ExternalOutputDescriptor>,
    connections: Vec<ConnectionDescriptor>,
    operations: Vec<OperationDescriptor>,
    predecessors: Vec<Vec<OperationIndex>>,
    successors: Vec<Vec<OperationIndex>>,
    node_lookup: BTreeMap<NodeKey, NodeIndex>,
    input_port_lookup: BTreeMap<AnyInPortKey, PortIndex>,
    output_port_lookup: BTreeMap<AnyOutPortKey, PortIndex>,
    external_input_lookup: BTreeMap<AnyExternalInputKey, ExternalInputIndex>,
    external_output_lookup: BTreeMap<AnyExternalOutputKey, ExternalOutputIndex>,
    operation_lookup: BTreeMap<ReactionVertex, OperationIndex>,
    toggle_initial_states: Vec<LogicLevel>,
    qualified_node_lookup: BTreeMap<QualifiedNodeRef, NodeKey>,
    qualified_node_reverse: BTreeMap<NodeKey, QualifiedNodeRef>,
    qualified_input_reverse: BTreeMap<AnyInPortKey, QualifiedInPortRef>,
    qualified_connection_lookup: BTreeMap<QualifiedConnectionRef, ConnectionKey>,
    modules: BTreeMap<QualifiedModuleRef, ModuleDef<D>>,
    module_inputs: BTreeMap<(QualifiedModuleRef, crate::key::AnyModuleInputKey), OperationIndex>,
    module_outputs: BTreeMap<(QualifiedModuleRef, crate::key::AnyModuleOutputKey), OperationIndex>,
}

#[derive(Debug)]
struct LoweredMetadata<D> {
    qualified_node_lookup: BTreeMap<QualifiedNodeRef, NodeKey>,
    qualified_node_reverse: BTreeMap<NodeKey, QualifiedNodeRef>,
    qualified_input_reverse: BTreeMap<AnyInPortKey, QualifiedInPortRef>,
    qualified_connection_lookup: BTreeMap<QualifiedConnectionRef, ConnectionKey>,
    modules: BTreeMap<QualifiedModuleRef, ModuleDef<D>>,
    module_input_sources:
        BTreeMap<(QualifiedModuleRef, crate::key::AnyModuleInputKey), ConnectionEndpoint>,
    module_output_sources:
        BTreeMap<(QualifiedModuleRef, crate::key::AnyModuleOutputKey), ConnectionEndpoint>,
}

impl<D> Default for LoweredMetadata<D> {
    fn default() -> Self {
        Self {
            qualified_node_lookup: BTreeMap::new(),
            qualified_node_reverse: BTreeMap::new(),
            qualified_input_reverse: BTreeMap::new(),
            qualified_connection_lookup: BTreeMap::new(),
            modules: BTreeMap::new(),
            module_input_sources: BTreeMap::new(),
            module_output_sources: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct NodeIndex(usize);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct PortIndex(usize);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct ExternalInputIndex(usize);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct ExternalOutputIndex(usize);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct OperationIndex(usize);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct ToggleStateIndex(usize);

impl ToggleStateIndex {
    pub(crate) const fn value(self) -> usize {
        self.0
    }
}

#[derive(Debug)]
struct NodeDescriptor {
    key: NodeKey,
    kind: CompiledNodeKind,
    inputs: Vec<PortIndex>,
    outputs: Vec<PortIndex>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CompiledNodeKind {
    Constant(LogicLevel),
    Not,
    All,
    Any,
    Parity,
    AtLeast(u64),
    Select {
        selector: PortIndex,
        when_low: PortIndex,
        when_high: PortIndex,
    },
    Merge,
    Coalesce,
    Zip,
    Toggle {
        input: PortIndex,
        state: ToggleStateIndex,
    },
    PulseDelay {
        input: PortIndex,
        delay_ticks: u64,
    },
}

#[derive(Debug)]
struct PortDescriptor {
    owner: NodeIndex,
    direction: PortDirection,
    kind: SignalKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PortDirection {
    Input,
    Output,
}

#[derive(Debug)]
struct ExternalInputDescriptor {
    key: AnyExternalInputKey,
}

#[derive(Debug)]
struct ExternalOutputDescriptor {
    key: AnyExternalOutputKey,
    source: OperationIndex,
}

#[derive(Debug)]
struct ConnectionDescriptor {
    key: ConnectionKey,
    source: OperationIndex,
    target: PortIndex,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OperationDescriptor {
    ExternalInput(ExternalInputIndex),
    Node(NodeIndex),
    NodeOutput(PortIndex),
    ExternalOutput(ExternalOutputIndex),
}

/// Complete outcomes of one restricted full reference evaluation.
///
/// This remains crate-private staging behind the public transaction result.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct FullEvaluation {
    pub(crate) values: Vec<LogicLevel>,
    pub(crate) operation_levels: Vec<Option<LogicLevel>>,
    pub(crate) operation_pulses: Vec<Option<PulseCount>>,
    pub(crate) causes: Vec<EvaluationCause>,
    pub(crate) external_outputs: BTreeMap<ExternalOutputKey<Level>, LogicLevel>,
    pub(crate) pulse_outputs: BTreeMap<ExternalOutputKey<Pulse>, PulseCount>,
    pub(crate) proposed_toggle_states: Vec<LogicLevel>,
    pub(crate) toggle_inversions: BTreeMap<NodeKey, usize>,
    pub(crate) pulse_delay_proposals: Vec<PulseDelayProposal>,
    #[cfg(test)]
    execution_counts: Vec<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PulseDelayProposal {
    pub(crate) node: NodeKey,
    pub(crate) delay_ticks: u64,
    pub(crate) count: PulseCount,
    pub(crate) input_source: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum EvaluationCause {
    ExternalInput(ExternalInputKey<Level>),
    PulseExternalInput(ExternalInputKey<Pulse>),
    Constant(NodeKey),
    Node {
        node: NodeKey,
        predecessors: Vec<usize>,
    },
    PulseCombinational {
        node: NodeKey,
        contributions: Vec<PulseEvaluationContribution>,
        result: PulseCount,
    },
    Toggle {
        node: NodeKey,
        input: usize,
        count: PulseCount,
        previous: LogicLevel,
        result: LogicLevel,
        inverted: bool,
    },
    PulseDelay {
        node: NodeKey,
    },
    Alias(usize),
    ExternalOutput {
        output: ExternalOutputKey<Level>,
        source: usize,
    },
    PulseExternalOutput {
        output: ExternalOutputKey<Pulse>,
        source: usize,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PulseEvaluationContribution {
    pub(crate) port: InPortKey<Pulse>,
    pub(crate) count: PulseCount,
    pub(crate) source: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EvaluationFailure {
    Incomplete,
    PulseCountOverflow { node: NodeKey },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EvaluationValue {
    Level(LogicLevel),
    Pulse(PulseCount),
}

impl<D> CompiledNetwork<D> {
    pub(crate) fn from_validated(validated: &ValidatedNetwork<D>) -> Report<Self, D> {
        let inner = if validated.definition().module_instances().is_empty() {
            let graph = validated.reaction_dependencies();
            CompiledInner::build(
                validated,
                clone_definition(validated.definition()),
                &graph,
                validated.topological_order(),
                LoweredMetadata::default(),
            )
        } else {
            let lowered = lower_module_instances(validated.definition());
            let (graph, order) = match compilation_reaction_graph(&lowered.definition) {
                Some(result) => result,
                None => panic!(
                    "module lowering must preserve the validated acyclic current-reaction invariant"
                ),
            };
            CompiledInner::build(
                validated,
                lowered.definition,
                &graph,
                &order,
                lowered.metadata,
            )
        };
        #[cfg(any(test, debug_assertions))]
        debug_assert!(
            inner.check_invariants().is_ok(),
            "compiled topology construction must establish its static invariants"
        );
        Report::new(
            Some(Self {
                inner: Arc::new(inner),
            }),
            DiagnosticSet::new(),
        )
    }

    /// Returns the stable authored network identity retained by compilation.
    #[must_use]
    pub fn network_key(&self) -> NetworkKey {
        self.inner.definition.key()
    }

    /// Returns the caller-owned logical tick identity retained unchanged.
    #[must_use]
    pub fn time_domain_id(&self) -> TimeDomainId {
        self.inner.definition.time_domain_id()
    }

    /// Returns the semantic network fingerprint retained unchanged.
    #[must_use]
    pub fn fingerprint(&self) -> NetworkFingerprint {
        self.inner.fingerprint
    }

    /// Returns the complete external input-schema fingerprint retained unchanged.
    #[must_use]
    pub fn input_schema_fingerprint(&self) -> InputSchemaFingerprint {
        self.inner.input_schema_fingerprint
    }

    /// Borrows the retained authored graph and complete module hierarchy.
    #[must_use]
    pub fn graph(&self) -> NetworkDefinitionGraphView<'_, D> {
        NetworkDefinitionGraphView::new(&self.inner.definition)
    }

    /// Spawns an independent uninitialized machine with an explicit policy.
    #[must_use]
    pub fn spawn(&self, policy: RuntimePolicy) -> Machine<D> {
        Machine::new(self.clone(), policy)
    }

    /// Starts a complete input snapshot bound to this exact topology.
    #[must_use]
    pub fn input_snapshot(&self) -> InputSnapshotBuilder<D> {
        InputSnapshotBuilder::new(
            self.network_key(),
            self.fingerprint(),
            self.input_schema_fingerprint(),
            self.inner
                .external_inputs
                .iter()
                .filter_map(|input| match input.key {
                    AnyExternalInputKey::Level(key) => Some(key),
                    AnyExternalInputKey::Pulse(_) => None,
                }),
            self.inner
                .external_inputs
                .iter()
                .filter_map(|input| match input.key {
                    AnyExternalInputKey::Pulse(key) => Some(key),
                    AnyExternalInputKey::Level(_) => None,
                }),
        )
    }

    /// Starts an input delta bound to this exact current topology.
    #[must_use]
    pub fn input_delta(&self) -> InputDeltaBuilder<D> {
        InputDeltaBuilder::new(
            self.network_key(),
            self.fingerprint(),
            self.input_schema_fingerprint(),
            self.inner
                .external_inputs
                .iter()
                .filter_map(|input| match input.key {
                    AnyExternalInputKey::Level(key) => Some(key),
                    AnyExternalInputKey::Pulse(_) => None,
                }),
            self.inner
                .external_inputs
                .iter()
                .filter_map(|input| match input.key {
                    AnyExternalInputKey::Pulse(key) => Some(key),
                    AnyExternalInputKey::Level(_) => None,
                }),
        )
    }

    #[cfg(test)]
    pub(crate) fn evaluate_full(
        &self,
        external_inputs: &BTreeMap<ExternalInputKey<Level>, LogicLevel>,
    ) -> Option<FullEvaluation> {
        self.inner
            .evaluate_reaction(
                external_inputs,
                &BTreeMap::new(),
                &self.inner.toggle_initial_states,
            )
            .ok()
    }

    #[cfg(test)]
    pub(crate) fn evaluate_reaction(
        &self,
        external_levels: &BTreeMap<ExternalInputKey<Level>, LogicLevel>,
        external_pulses: &BTreeMap<ExternalInputKey<Pulse>, PulseCount>,
    ) -> Result<FullEvaluation, EvaluationFailure> {
        self.inner.evaluate_reaction(
            external_levels,
            external_pulses,
            &self.inner.toggle_initial_states,
        )
    }

    pub(crate) fn evaluate_reaction_with_state(
        &self,
        external_levels: &BTreeMap<ExternalInputKey<Level>, LogicLevel>,
        external_pulses: &BTreeMap<ExternalInputKey<Pulse>, PulseCount>,
        previous_toggle_states: &[LogicLevel],
    ) -> Result<FullEvaluation, EvaluationFailure> {
        self.inner
            .evaluate_reaction(external_levels, external_pulses, previous_toggle_states)
    }

    pub(crate) fn evaluate_temporal_reaction(
        &self,
        external_levels: &BTreeMap<ExternalInputKey<Level>, LogicLevel>,
        external_pulses: &BTreeMap<ExternalInputKey<Pulse>, PulseCount>,
        previous_toggle_states: &[LogicLevel],
        due_pulses: &BTreeMap<NodeKey, PulseCount>,
    ) -> Result<FullEvaluation, EvaluationFailure> {
        self.inner.evaluate_reaction_with_due(
            external_levels,
            external_pulses,
            previous_toggle_states,
            due_pulses,
        )
    }

    pub(crate) fn operation_count(&self) -> usize {
        self.inner.operations.len()
    }

    pub(crate) fn initial_toggle_states(&self) -> Vec<LogicLevel> {
        self.inner.toggle_initial_states.clone()
    }

    pub(crate) fn toggle_state_slot(
        &self,
        node: NodeKey,
    ) -> Option<(ToggleStateIndex, LogicLevel)> {
        let descriptor = self.inner.nodes.get(self.inner.node_lookup.get(&node)?.0)?;
        match descriptor.kind {
            CompiledNodeKind::Toggle { state, .. } => {
                Some((state, self.inner.toggle_initial_states[state.0]))
            }
            _ => None,
        }
    }

    pub(crate) fn pulse_delay(&self, node: NodeKey) -> Option<NonZeroSpan<D>> {
        let descriptor = self.inner.nodes.get(self.inner.node_lookup.get(&node)?.0)?;
        let CompiledNodeKind::PulseDelay { delay_ticks, .. } = descriptor.kind else {
            return None;
        };
        NonZeroSpan::from_ticks(delay_ticks).ok()
    }

    pub(crate) fn contains_node(&self, node: NodeKey) -> bool {
        self.inner.node_lookup.contains_key(&node)
    }

    pub(crate) fn qualified_node(&self, node: NodeKey) -> Option<&QualifiedNodeRef> {
        self.inner.qualified_node_reverse.get(&node)
    }

    pub(crate) fn node_subject(&self, node: NodeKey) -> NodeSubject {
        match self.qualified_node(node) {
            Some(qualified) => NodeSubject::Qualified(qualified.clone()),
            None => NodeSubject::Node(node),
        }
    }

    pub(crate) fn pulse_port_subject(&self, port: InPortKey<Pulse>) -> PulsePortSubject {
        match self.inner.qualified_input_reverse.get(&port.into()) {
            Some(qualified) => PulsePortSubject::Qualified(qualified.clone()),
            None => PulsePortSubject::Port(port),
        }
    }

    pub(crate) fn module(&self, module: &QualifiedModuleRef) -> Option<&ModuleDef<D>> {
        self.inner.modules.get(module)
    }

    pub(crate) fn qualified_nodes_under(
        &self,
        module: &QualifiedModuleRef,
    ) -> impl Iterator<Item = (&QualifiedNodeRef, NodeKey)> {
        self.inner
            .qualified_node_lookup
            .iter()
            .filter(move |(node, _)| node.instances().starts_with(module.instances()))
            .map(|(node, flat)| (node, *flat))
    }

    pub(crate) fn qualified_connections_under(
        &self,
        module: &QualifiedModuleRef,
    ) -> impl Iterator<Item = &QualifiedConnectionRef> {
        self.inner
            .qualified_connection_lookup
            .keys()
            .filter(move |connection| connection.instances().starts_with(module.instances()))
    }

    pub(crate) fn qualified_modules_under(
        &self,
        module: &QualifiedModuleRef,
    ) -> impl Iterator<Item = &QualifiedModuleRef> {
        self.inner
            .modules
            .keys()
            .filter(move |candidate| candidate.instances().starts_with(module.instances()))
    }

    pub(crate) fn module_input_operation(
        &self,
        module: &QualifiedModuleRef,
        input: crate::key::AnyModuleInputKey,
    ) -> Option<usize> {
        self.inner
            .module_inputs
            .get(&(module.clone(), input))
            .map(|index| index.0)
    }

    pub(crate) fn module_output_operation(
        &self,
        module: &QualifiedModuleRef,
        output: crate::key::AnyModuleOutputKey,
    ) -> Option<usize> {
        self.inner
            .module_outputs
            .get(&(module.clone(), output))
            .map(|index| index.0)
    }

    pub(crate) fn node_operation(&self, node: NodeKey) -> Option<usize> {
        self.inner
            .operation_lookup
            .get(&ReactionVertex::NodeOperation(node))
            .map(|index| index.0)
    }
}

impl<D> CompiledInner<D> {
    fn evaluate_reaction(
        &self,
        external_levels: &BTreeMap<ExternalInputKey<Level>, LogicLevel>,
        external_pulses: &BTreeMap<ExternalInputKey<Pulse>, PulseCount>,
        previous_toggle_states: &[LogicLevel],
    ) -> Result<FullEvaluation, EvaluationFailure> {
        self.evaluate_reaction_with_due(
            external_levels,
            external_pulses,
            previous_toggle_states,
            &BTreeMap::new(),
        )
    }

    fn evaluate_reaction_with_due(
        &self,
        external_levels: &BTreeMap<ExternalInputKey<Level>, LogicLevel>,
        external_pulses: &BTreeMap<ExternalInputKey<Pulse>, PulseCount>,
        previous_toggle_states: &[LogicLevel],
        due_pulses: &BTreeMap<NodeKey, PulseCount>,
    ) -> Result<FullEvaluation, EvaluationFailure> {
        if previous_toggle_states.len() != self.toggle_initial_states.len() {
            return Err(EvaluationFailure::Incomplete);
        }
        let mut values = vec![None; self.operations.len()];
        let mut causes = Vec::with_capacity(self.operations.len());
        let mut external_outputs = BTreeMap::new();
        let mut pulse_outputs = BTreeMap::new();
        let mut proposed_toggle_states = previous_toggle_states.to_vec();
        let mut toggle_inversions = BTreeMap::new();
        let mut pulse_delay_proposals = Vec::new();
        #[cfg(test)]
        let mut execution_counts = vec![0; self.operations.len()];

        for (index, operation) in self.operations.iter().enumerate() {
            if self.predecessors[index]
                .iter()
                .any(|predecessor| values[predecessor.0].is_none())
            {
                return Err(EvaluationFailure::Incomplete);
            }

            #[cfg(test)]
            {
                execution_counts[index] += 1;
            }
            let (value, cause) = match operation {
                OperationDescriptor::ExternalInput(input) => {
                    match self
                        .external_inputs
                        .get(input.0)
                        .ok_or(EvaluationFailure::Incomplete)?
                        .key
                    {
                        AnyExternalInputKey::Level(key) => (
                            EvaluationValue::Level(
                                external_levels
                                    .get(&key)
                                    .copied()
                                    .ok_or(EvaluationFailure::Incomplete)?,
                            ),
                            EvaluationCause::ExternalInput(key),
                        ),
                        AnyExternalInputKey::Pulse(key) => (
                            EvaluationValue::Pulse(
                                external_pulses
                                    .get(&key)
                                    .copied()
                                    .unwrap_or(PulseCount::ZERO),
                            ),
                            EvaluationCause::PulseExternalInput(key),
                        ),
                    }
                }
                OperationDescriptor::Node(node) => match self
                    .nodes
                    .get(node.0)
                    .ok_or(EvaluationFailure::Incomplete)?
                {
                    NodeDescriptor {
                        key,
                        kind: CompiledNodeKind::Constant(value),
                        ..
                    } => (
                        EvaluationValue::Level(*value),
                        EvaluationCause::Constant(*key),
                    ),
                    NodeDescriptor {
                        key,
                        kind: CompiledNodeKind::Not,
                        inputs,
                        ..
                    } => (
                        EvaluationValue::Level(
                            self.level_input_value(
                                inputs
                                    .first()
                                    .copied()
                                    .ok_or(EvaluationFailure::Incomplete)?,
                                &values,
                            )?
                            .invert(),
                        ),
                        EvaluationCause::Node {
                            node: *key,
                            predecessors: self.predecessors[index]
                                .iter()
                                .map(|predecessor| predecessor.0)
                                .collect(),
                        },
                    ),
                    NodeDescriptor {
                        key,
                        kind: CompiledNodeKind::All,
                        inputs,
                        ..
                    } => {
                        // SPEC: docs/specs/built_in_node_semantics.md §24 "All"
                        // High is the conjunction identity, including for zero inputs.
                        let mut value = LogicLevel::High;
                        for input in inputs {
                            if self.level_input_value(*input, &values)?.is_low() {
                                value = LogicLevel::Low;
                                break;
                            }
                        }
                        (
                            EvaluationValue::Level(value),
                            EvaluationCause::Node {
                                node: *key,
                                predecessors: self.predecessors[index]
                                    .iter()
                                    .map(|predecessor| predecessor.0)
                                    .collect(),
                            },
                        )
                    }
                    NodeDescriptor {
                        key,
                        kind: CompiledNodeKind::Any,
                        inputs,
                        ..
                    } => {
                        // SPEC: docs/specs/contracts/level-combinational-expansion.yaml "any-total-law"
                        // Low is the disjunction identity, including for zero inputs.
                        let mut value = LogicLevel::Low;
                        for input in inputs {
                            if self.level_input_value(*input, &values)?.is_high() {
                                value = LogicLevel::High;
                                break;
                            }
                        }
                        (
                            EvaluationValue::Level(value),
                            EvaluationCause::Node {
                                node: *key,
                                predecessors: self.predecessors[index]
                                    .iter()
                                    .map(|predecessor| predecessor.0)
                                    .collect(),
                            },
                        )
                    }
                    NodeDescriptor {
                        key,
                        kind: CompiledNodeKind::Parity,
                        inputs,
                        ..
                    } => {
                        let mut high = false;
                        for input in inputs {
                            high ^= self.level_input_value(*input, &values)?.is_high();
                        }
                        (
                            EvaluationValue::Level(if high {
                                LogicLevel::High
                            } else {
                                LogicLevel::Low
                            }),
                            EvaluationCause::Node {
                                node: *key,
                                predecessors: self.predecessors[index]
                                    .iter()
                                    .map(|predecessor| predecessor.0)
                                    .collect(),
                            },
                        )
                    }
                    NodeDescriptor {
                        key,
                        kind: CompiledNodeKind::AtLeast(threshold),
                        inputs,
                        ..
                    } => {
                        let mut high_count = 0_u64;
                        for input in inputs {
                            if self.level_input_value(*input, &values)?.is_high() {
                                high_count = high_count.saturating_add(1);
                            }
                        }
                        (
                            EvaluationValue::Level(if high_count >= *threshold {
                                LogicLevel::High
                            } else {
                                LogicLevel::Low
                            }),
                            EvaluationCause::Node {
                                node: *key,
                                predecessors: self.predecessors[index]
                                    .iter()
                                    .map(|predecessor| predecessor.0)
                                    .collect(),
                            },
                        )
                    }
                    NodeDescriptor {
                        key,
                        kind:
                            CompiledNodeKind::Select {
                                selector,
                                when_low,
                                when_high,
                            },
                        ..
                    } => {
                        let selector = self.level_input_value(*selector, &values)?;
                        let branch = if selector.is_low() {
                            *when_low
                        } else {
                            *when_high
                        };
                        (
                            EvaluationValue::Level(self.level_input_value(branch, &values)?),
                            EvaluationCause::Node {
                                node: *key,
                                predecessors: self.predecessors[index]
                                    .iter()
                                    .map(|predecessor| predecessor.0)
                                    .collect(),
                            },
                        )
                    }
                    NodeDescriptor {
                        key,
                        kind: CompiledNodeKind::Merge,
                        inputs,
                        ..
                    } => {
                        // SPEC: docs/specs/contracts/pulse-merge.yaml "total-checked-sum-law"
                        // Every stable port contributes its complete simultaneous count.
                        let mut total = PulseCount::ZERO;
                        let mut contributions = Vec::with_capacity(inputs.len());
                        for input in inputs {
                            let count = self.pulse_input_value(*input, &values)?;
                            total = total.checked_add(count).map_err(|_| {
                                EvaluationFailure::PulseCountOverflow { node: *key }
                            })?;
                            contributions.push(PulseEvaluationContribution {
                                port: self.pulse_port_key(*input)?,
                                count,
                                source: self.input_source(*input)?.0,
                            });
                        }
                        (
                            EvaluationValue::Pulse(total),
                            EvaluationCause::PulseCombinational {
                                node: *key,
                                contributions,
                                result: total,
                            },
                        )
                    }
                    NodeDescriptor {
                        key,
                        kind: CompiledNodeKind::Coalesce,
                        inputs,
                        ..
                    } => {
                        let input = inputs.first().ok_or(EvaluationFailure::Incomplete)?;
                        let count = self.pulse_input_value(*input, &values)?;
                        let result = if count.is_positive() {
                            PulseCount::ONE
                        } else {
                            PulseCount::ZERO
                        };
                        (
                            EvaluationValue::Pulse(result),
                            EvaluationCause::PulseCombinational {
                                node: *key,
                                contributions: vec![PulseEvaluationContribution {
                                    port: self.pulse_port_key(*input)?,
                                    count,
                                    source: self.input_source(*input)?.0,
                                }],
                                result,
                            },
                        )
                    }
                    NodeDescriptor {
                        key,
                        kind: CompiledNodeKind::Zip,
                        inputs,
                        ..
                    } => {
                        // SPEC: docs/specs/contracts/pulse-combinational-expansion.yaml
                        // "zip-group-law" — one result group requires one pulse from every port.
                        let mut minimum = None;
                        let mut contributions = Vec::with_capacity(inputs.len());
                        for input in inputs {
                            let count = self.pulse_input_value(*input, &values)?;
                            minimum = Some(
                                minimum.map_or(count, |current: PulseCount| current.min(count)),
                            );
                            contributions.push(PulseEvaluationContribution {
                                port: self.pulse_port_key(*input)?,
                                count,
                                source: self.input_source(*input)?.0,
                            });
                        }
                        let result = minimum.ok_or(EvaluationFailure::Incomplete)?;
                        (
                            EvaluationValue::Pulse(result),
                            EvaluationCause::PulseCombinational {
                                node: *key,
                                contributions,
                                result,
                            },
                        )
                    }
                    NodeDescriptor {
                        key,
                        kind: CompiledNodeKind::Toggle { input, state },
                        ..
                    } => {
                        // SPEC: docs/specs/contracts/toggle.yaml "parity-output-and-successor-law"
                        // Every operation reads the shared previous vector and only stages a successor.
                        let count = self.pulse_input_value(*input, &values)?;
                        let previous = previous_toggle_states
                            .get(state.0)
                            .copied()
                            .ok_or(EvaluationFailure::Incomplete)?;
                        let inverted = count.get() % 2 == 1;
                        let result = if inverted {
                            previous.invert()
                        } else {
                            previous
                        };
                        proposed_toggle_states[state.0] = result;
                        if inverted {
                            toggle_inversions.insert(*key, index);
                        }
                        (
                            EvaluationValue::Level(result),
                            EvaluationCause::Toggle {
                                node: *key,
                                input: self.input_source(*input)?.0,
                                count,
                                previous,
                                result,
                                inverted,
                            },
                        )
                    }
                    NodeDescriptor {
                        key,
                        kind: CompiledNodeKind::PulseDelay { .. },
                        ..
                    } => {
                        // SPEC: docs/specs/contracts/pulse-delay.yaml "temporal-causality-barrier"
                        // Current output depends only on due work. Current input is read after
                        // complete settlement to create a strictly-future terminal proposal.
                        (
                            EvaluationValue::Pulse(
                                due_pulses.get(key).copied().unwrap_or(PulseCount::ZERO),
                            ),
                            EvaluationCause::PulseDelay { node: *key },
                        )
                    }
                },
                OperationDescriptor::NodeOutput(port) => {
                    let predecessor = self.predecessors[index]
                        .first()
                        .ok_or(EvaluationFailure::Incomplete)?;
                    let port = self
                        .ports
                        .get(port.0)
                        .ok_or(EvaluationFailure::Incomplete)?;
                    if port.direction != PortDirection::Output {
                        return Err(EvaluationFailure::Incomplete);
                    }
                    (
                        values[predecessor.0].ok_or(EvaluationFailure::Incomplete)?,
                        EvaluationCause::Alias(predecessor.0),
                    )
                }
                OperationDescriptor::ExternalOutput(output) => {
                    let descriptor = self
                        .external_outputs
                        .get(output.0)
                        .ok_or(EvaluationFailure::Incomplete)?;
                    let value = values[descriptor.source.0].ok_or(EvaluationFailure::Incomplete)?;
                    match (descriptor.key, value) {
                        (AnyExternalOutputKey::Level(key), EvaluationValue::Level(value)) => {
                            external_outputs.insert(key, value);
                            (
                                EvaluationValue::Level(value),
                                EvaluationCause::ExternalOutput {
                                    output: key,
                                    source: descriptor.source.0,
                                },
                            )
                        }
                        (AnyExternalOutputKey::Pulse(key), EvaluationValue::Pulse(value)) => {
                            pulse_outputs.insert(key, value);
                            (
                                EvaluationValue::Pulse(value),
                                EvaluationCause::PulseExternalOutput {
                                    output: key,
                                    source: descriptor.source.0,
                                },
                            )
                        }
                        _ => return Err(EvaluationFailure::Incomplete),
                    }
                }
            };
            values[index] = Some(value);
            causes.push(cause);
        }

        // PulseDelay's scheduling path is terminal for this reaction. Reading it
        // after all current values settle preserves upstream causality without
        // adding an input-to-current-output edge or a same-time microstep.
        for node in &self.nodes {
            let CompiledNodeKind::PulseDelay { input, delay_ticks } = node.kind else {
                continue;
            };
            let count = self.pulse_input_value(input, &values)?;
            if count.is_positive() {
                pulse_delay_proposals.push(PulseDelayProposal {
                    node: node.key,
                    delay_ticks,
                    count,
                    input_source: self.input_source(input)?.0,
                });
            }
        }

        let complete_values = values
            .into_iter()
            .collect::<Option<Vec<_>>>()
            .ok_or(EvaluationFailure::Incomplete)?;
        Ok(FullEvaluation {
            values: complete_values
                .iter()
                .filter_map(|value| match value {
                    EvaluationValue::Level(value) => Some(*value),
                    EvaluationValue::Pulse(_) => None,
                })
                .collect(),
            operation_levels: complete_values
                .iter()
                .map(|value| match value {
                    EvaluationValue::Level(value) => Some(*value),
                    EvaluationValue::Pulse(_) => None,
                })
                .collect(),
            operation_pulses: complete_values
                .iter()
                .map(|value| match value {
                    EvaluationValue::Pulse(value) => Some(*value),
                    EvaluationValue::Level(_) => None,
                })
                .collect(),
            causes,
            external_outputs,
            pulse_outputs,
            proposed_toggle_states,
            toggle_inversions,
            pulse_delay_proposals,
            #[cfg(test)]
            execution_counts,
        })
    }

    fn input_source(&self, port: PortIndex) -> Result<OperationIndex, EvaluationFailure> {
        self.connections
            .iter()
            .find(|connection| connection.target == port)
            .map(|connection| connection.source)
            .ok_or(EvaluationFailure::Incomplete)
    }

    fn level_input_value(
        &self,
        port: PortIndex,
        values: &[Option<EvaluationValue>],
    ) -> Result<LogicLevel, EvaluationFailure> {
        match values.get(self.input_source(port)?.0).copied().flatten() {
            Some(EvaluationValue::Level(value)) => Ok(value),
            _ => Err(EvaluationFailure::Incomplete),
        }
    }

    fn pulse_input_value(
        &self,
        port: PortIndex,
        values: &[Option<EvaluationValue>],
    ) -> Result<PulseCount, EvaluationFailure> {
        match values.get(self.input_source(port)?.0).copied().flatten() {
            Some(EvaluationValue::Pulse(value)) => Ok(value),
            _ => Err(EvaluationFailure::Incomplete),
        }
    }

    fn pulse_port_key(&self, port: PortIndex) -> Result<InPortKey<Pulse>, EvaluationFailure> {
        self.input_port_lookup
            .iter()
            .find_map(|(key, index)| (*index == port).then_some(*key))
            .and_then(|key| match key {
                AnyInPortKey::Pulse(key) => Some(key),
                AnyInPortKey::Level(_) => None,
            })
            .ok_or(EvaluationFailure::Incomplete)
    }

    fn build(
        validated: &ValidatedNetwork<D>,
        execution_definition: UncheckedNetwork<D>,
        graph: &ReactionDependencyGraph,
        topological_order: &[ReactionVertex],
        metadata: LoweredMetadata<D>,
    ) -> Self {
        let definition = execution_definition;
        let mut nodes = Vec::new();
        let mut ports = Vec::new();
        let mut node_lookup = BTreeMap::new();
        let mut input_port_lookup = BTreeMap::new();
        let mut output_port_lookup = BTreeMap::new();
        let mut toggle_initial_states = Vec::new();

        let mut authored_nodes: Vec<_> = definition.nodes().iter().collect();
        authored_nodes.sort_by_key(|node| node.key());
        for node in authored_nodes {
            let node_index = NodeIndex(nodes.len());
            node_lookup.insert(node.key(), node_index);
            let mut inputs = Vec::new();
            for key in node.ports().inputs() {
                let index = PortIndex(ports.len());
                input_port_lookup.insert(*key, index);
                ports.push(PortDescriptor {
                    owner: node_index,
                    direction: PortDirection::Input,
                    kind: key.kind(),
                });
                inputs.push(index);
            }
            let mut outputs = Vec::new();
            for key in node.ports().outputs() {
                let index = PortIndex(ports.len());
                output_port_lookup.insert(*key, index);
                ports.push(PortDescriptor {
                    owner: node_index,
                    direction: PortDirection::Output,
                    kind: key.kind(),
                });
                outputs.push(index);
            }
            let kind = match node.kind() {
                NodeKind::Constant(config) => CompiledNodeKind::Constant(config.value()),
                NodeKind::Not => CompiledNodeKind::Not,
                NodeKind::All => CompiledNodeKind::All,
                NodeKind::Any => CompiledNodeKind::Any,
                NodeKind::Parity => CompiledNodeKind::Parity,
                NodeKind::AtLeast(config) => CompiledNodeKind::AtLeast(config.threshold),
                NodeKind::Select => {
                    let role_port = |role| {
                        node.ports()
                            .input_roles()
                            .iter()
                            .zip(inputs.iter().copied())
                            .find_map(|(candidate, port)| (*candidate == role).then_some(port))
                            .unwrap_or_else(|| {
                                panic!(
                                    "validated Select descriptor must retain every fixed input role"
                                )
                            })
                    };
                    CompiledNodeKind::Select {
                        selector: role_port(InputPortRole::Selector),
                        when_low: role_port(InputPortRole::WhenLow),
                        when_high: role_port(InputPortRole::WhenHigh),
                    }
                }
                NodeKind::Merge => CompiledNodeKind::Merge,
                NodeKind::Coalesce => CompiledNodeKind::Coalesce,
                NodeKind::Zip => CompiledNodeKind::Zip,
                NodeKind::Toggle(config) => {
                    let input = inputs.first().copied().unwrap_or_else(|| {
                        panic!("validated Toggle descriptor must retain its pulse input")
                    });
                    let state = ToggleStateIndex(toggle_initial_states.len());
                    toggle_initial_states.push(config.initial);
                    CompiledNodeKind::Toggle { input, state }
                }
                NodeKind::PulseDelay(config) => {
                    let input = inputs.first().copied().unwrap_or_else(|| {
                        panic!("validated PulseDelay descriptor must retain its pulse input")
                    });
                    CompiledNodeKind::PulseDelay {
                        input,
                        delay_ticks: config.delay.ticks(),
                    }
                }
            };
            nodes.push(NodeDescriptor {
                key: node.key(),
                kind,
                inputs,
                outputs,
            });
        }

        let mut external_inputs: Vec<_> = definition.external_inputs().iter().collect();
        external_inputs.sort_by_key(|input| input.key());
        let external_input_lookup = external_inputs
            .iter()
            .enumerate()
            .map(|(index, input)| (input.key(), ExternalInputIndex(index)))
            .collect();
        let external_inputs = external_inputs
            .into_iter()
            .map(|input| ExternalInputDescriptor { key: input.key() })
            .collect();

        let mut external_outputs: Vec<_> = definition.external_outputs().iter().collect();
        external_outputs.sort_by_key(|output| output.key());
        let external_output_lookup = external_outputs
            .iter()
            .enumerate()
            .map(|(index, output)| (output.key(), ExternalOutputIndex(index)))
            .collect();

        let operation_lookup = topological_order
            .iter()
            .copied()
            .enumerate()
            .map(|(index, vertex)| (vertex, OperationIndex(index)))
            .collect::<BTreeMap<_, _>>();
        let operations = topological_order
            .iter()
            .copied()
            .map(|vertex| {
                operation_descriptor(
                    vertex,
                    &node_lookup,
                    &output_port_lookup,
                    &external_input_lookup,
                    &external_output_lookup,
                )
            })
            .collect::<Vec<_>>();

        let mut predecessors = vec![Vec::new(); operations.len()];
        let mut successors = vec![Vec::new(); operations.len()];
        for dependency in graph.dependencies() {
            let from = operation_lookup[&dependency.from];
            let to = operation_lookup[&dependency.to];
            successors[from.0].push(to);
            predecessors[to.0].push(from);
        }
        for adjacency in predecessors.iter_mut().chain(successors.iter_mut()) {
            adjacency.sort();
            adjacency.dedup();
        }

        let external_outputs = external_outputs
            .into_iter()
            .map(|output| ExternalOutputDescriptor {
                key: output.key(),
                source: operation_lookup[&reaction_source(output.source())],
            })
            .collect();

        let mut connections: Vec<_> = definition.connections().iter().collect();
        connections.sort_by_key(|connection| connection.key());
        let connections = connections
            .into_iter()
            .map(|connection| {
                let ConnectionEndpoint::NodeInput(target) = connection.to() else {
                    panic!("validated connection target must be a node input");
                };
                ConnectionDescriptor {
                    key: connection.key(),
                    source: operation_lookup[&connection_source(connection.from())],
                    target: input_port_lookup[&target],
                }
            })
            .collect();

        let module_inputs = metadata
            .module_input_sources
            .into_iter()
            .map(|(key, source)| (key, operation_lookup[&connection_source(source)]))
            .collect();
        let module_outputs = metadata
            .module_output_sources
            .into_iter()
            .map(|(key, source)| (key, operation_lookup[&connection_source(source)]))
            .collect();

        Self {
            definition: clone_complete_definition(validated.definition()),
            fingerprint: validated.fingerprint(),
            input_schema_fingerprint: validated.input_schema_fingerprint(),
            nodes,
            ports,
            external_inputs,
            external_outputs,
            connections,
            operations,
            predecessors,
            successors,
            node_lookup,
            input_port_lookup,
            output_port_lookup,
            external_input_lookup,
            external_output_lookup,
            operation_lookup,
            toggle_initial_states,
            qualified_node_lookup: metadata.qualified_node_lookup,
            qualified_node_reverse: metadata.qualified_node_reverse,
            qualified_input_reverse: metadata.qualified_input_reverse,
            qualified_connection_lookup: metadata.qualified_connection_lookup,
            modules: metadata.modules,
            module_inputs,
            module_outputs,
        }
    }

    #[cfg(any(test, debug_assertions))]
    fn check_invariants(&self) -> Result<(), &'static str> {
        if self.nodes.len() != self.node_lookup.len()
            || self.external_inputs.len() != self.external_input_lookup.len()
            || self.external_outputs.len() != self.external_output_lookup.len()
            || self.operations.len() != self.operation_lookup.len()
            || self.operations.len() != self.predecessors.len()
            || self.operations.len() != self.successors.len()
        {
            return Err("compiled lookup or adjacency table is incomplete");
        }
        for (key, index) in &self.node_lookup {
            if self.nodes.get(index.0).map(|node| node.key) != Some(*key) {
                return Err("compiled node lookup is ambiguous or out of bounds");
            }
        }
        for node in &self.nodes {
            let shape_valid = match node.kind {
                CompiledNodeKind::Constant(_) => node.inputs.is_empty() && node.outputs.len() == 1,
                CompiledNodeKind::Not => node.inputs.len() == 1 && node.outputs.len() == 1,
                CompiledNodeKind::All
                | CompiledNodeKind::Any
                | CompiledNodeKind::Parity
                | CompiledNodeKind::AtLeast(_)
                | CompiledNodeKind::Merge => node.outputs.len() == 1,
                CompiledNodeKind::Coalesce => node.inputs.len() == 1 && node.outputs.len() == 1,
                CompiledNodeKind::Zip => !node.inputs.is_empty() && node.outputs.len() == 1,
                CompiledNodeKind::Toggle { input, state } => {
                    node.inputs.len() == 1
                        && node.outputs.len() == 1
                        && node.inputs[0] == input
                        && state.0 < self.toggle_initial_states.len()
                }
                CompiledNodeKind::PulseDelay { input, delay_ticks } => {
                    node.inputs.len() == 1
                        && node.outputs.len() == 1
                        && node.inputs[0] == input
                        && delay_ticks > 0
                }
                CompiledNodeKind::Select {
                    selector,
                    when_low,
                    when_high,
                } => {
                    node.inputs.len() == 3
                        && node.outputs.len() == 1
                        && selector != when_low
                        && selector != when_high
                        && when_low != when_high
                        && node.inputs.contains(&selector)
                        && node.inputs.contains(&when_low)
                        && node.inputs.contains(&when_high)
                }
            };
            if !shape_valid {
                return Err("compiled descriptor disagrees with node kind");
            }
            let (input_kind, output_kind) = match node.kind {
                CompiledNodeKind::Merge | CompiledNodeKind::Coalesce | CompiledNodeKind::Zip => {
                    (SignalKind::Pulse, SignalKind::Pulse)
                }
                CompiledNodeKind::Toggle { .. } => (SignalKind::Pulse, SignalKind::Level),
                CompiledNodeKind::PulseDelay { .. } => (SignalKind::Pulse, SignalKind::Pulse),
                _ => (SignalKind::Level, SignalKind::Level),
            };
            for index in &node.inputs {
                let Some(port) = self.ports.get(index.0) else {
                    return Err("compiled port reference is out of bounds");
                };
                if port.owner != self.node_lookup[&node.key] || port.kind != input_kind {
                    return Err("compiled port disagrees with validated node");
                }
            }
            for index in &node.outputs {
                let Some(port) = self.ports.get(index.0) else {
                    return Err("compiled port reference is out of bounds");
                };
                if port.owner != self.node_lookup[&node.key] || port.kind != output_kind {
                    return Err("compiled port disagrees with validated node");
                }
            }
        }
        for (key, index) in &self.input_port_lookup {
            if self.ports.get(index.0).map(|port| port.direction) != Some(PortDirection::Input)
                || self.ports.get(index.0).map(|port| port.kind) != Some(key.kind())
            {
                return Err("compiled input-port lookup is invalid");
            }
        }
        for (key, index) in &self.external_input_lookup {
            if self.external_inputs.get(index.0).map(|input| input.key) != Some(*key) {
                return Err("compiled external-input lookup is invalid");
            }
        }
        for (key, index) in &self.output_port_lookup {
            if self.ports.get(index.0).map(|port| port.direction) != Some(PortDirection::Output)
                || self.ports.get(index.0).map(|port| port.kind) != Some(key.kind())
            {
                return Err("compiled output-port lookup is invalid");
            }
        }
        for connection in &self.connections {
            let Some(port) = self.ports.get(connection.target.0) else {
                return Err("compiled connection target is out of bounds");
            };
            if port.direction != PortDirection::Input
                || connection.source.0 >= self.operations.len()
            {
                return Err("compiled connection violates driver incidence");
            }
        }
        if self
            .connections
            .windows(2)
            .any(|pair| pair[0].key >= pair[1].key)
        {
            return Err("compiled connection lookup is ambiguous");
        }
        if self
            .predecessors
            .iter()
            .chain(&self.successors)
            .any(|adjacency| adjacency.windows(2).any(|pair| pair[0] >= pair[1]))
        {
            return Err("compiled reaction adjacency order is invalid");
        }
        for (index, successors) in self.successors.iter().enumerate() {
            for successor in successors {
                if successor.0 >= self.operations.len() || index >= successor.0 {
                    return Err("compiled reaction dependency is not forward");
                }
                if !self.predecessors[successor.0].contains(&OperationIndex(index)) {
                    return Err("compiled reaction adjacency is inconsistent");
                }
            }
        }
        for (index, predecessors) in self.predecessors.iter().enumerate() {
            for predecessor in predecessors {
                if predecessor.0 >= self.operations.len()
                    || predecessor.0 >= index
                    || !self.successors[predecessor.0].contains(&OperationIndex(index))
                {
                    return Err("compiled reaction adjacency is inconsistent");
                }
            }
        }
        for output in &self.external_outputs {
            if output.source.0 >= self.operations.len() {
                return Err("compiled external endpoint table is incomplete");
            }
        }
        Ok(())
    }
}

fn operation_descriptor(
    vertex: ReactionVertex,
    nodes: &BTreeMap<NodeKey, NodeIndex>,
    outputs: &BTreeMap<AnyOutPortKey, PortIndex>,
    external_inputs: &BTreeMap<AnyExternalInputKey, ExternalInputIndex>,
    external_outputs: &BTreeMap<AnyExternalOutputKey, ExternalOutputIndex>,
) -> OperationDescriptor {
    match vertex {
        ReactionVertex::ExternalInput(key) => {
            OperationDescriptor::ExternalInput(external_inputs[&key])
        }
        ReactionVertex::NodeOperation(key) => OperationDescriptor::Node(nodes[&key]),
        ReactionVertex::NodeOutput(key) => OperationDescriptor::NodeOutput(outputs[&key]),
        ReactionVertex::ExternalOutput(key) => {
            OperationDescriptor::ExternalOutput(external_outputs[&key])
        }
        ReactionVertex::ModuleInput(_)
        | ReactionVertex::ModuleOutput(_)
        | ReactionVertex::InstanceInput(_, _)
        | ReactionVertex::InstanceOutput(_, _) => {
            panic!("network compilation must not receive module-boundary reaction vertices")
        }
    }
}

fn connection_source(endpoint: ConnectionEndpoint) -> ReactionVertex {
    match endpoint {
        ConnectionEndpoint::ExternalInput(key) => ReactionVertex::ExternalInput(key),
        ConnectionEndpoint::NodeOutput(key) => ReactionVertex::NodeOutput(key),
        ConnectionEndpoint::NodeInput(_)
        | ConnectionEndpoint::ModuleInput(_)
        | ConnectionEndpoint::ModuleOutput { .. }
        | ConnectionEndpoint::ExternalOutput(_) => {
            panic!("validated connection source must be a reaction source")
        }
    }
}

fn reaction_source(source: crate::key::AnySignalSourceKey) -> ReactionVertex {
    match source {
        crate::key::AnySignalSourceKey::Level(crate::key::SignalSourceKey::ExternalInput(key)) => {
            ReactionVertex::ExternalInput(key.into())
        }
        crate::key::AnySignalSourceKey::Level(crate::key::SignalSourceKey::NodeOutput(key)) => {
            ReactionVertex::NodeOutput(key.into())
        }
        crate::key::AnySignalSourceKey::Pulse(crate::key::SignalSourceKey::ExternalInput(key)) => {
            ReactionVertex::ExternalInput(key.into())
        }
        crate::key::AnySignalSourceKey::Pulse(crate::key::SignalSourceKey::NodeOutput(key)) => {
            ReactionVertex::NodeOutput(key.into())
        }
        crate::key::AnySignalSourceKey::Level(crate::key::SignalSourceKey::ModuleOutput {
            ..
        })
        | crate::key::AnySignalSourceKey::Pulse(crate::key::SignalSourceKey::ModuleOutput {
            ..
        }) => {
            panic!("executable module outputs must resolve before descriptor construction")
        }
    }
}

struct LoweredNetwork<D> {
    definition: UncheckedNetwork<D>,
    metadata: LoweredMetadata<D>,
}

enum ScopeDefinition<'a, D> {
    Network(&'a UncheckedNetwork<D>),
    Module(ModuleDef<D>),
}

impl<D> ScopeDefinition<'_, D> {
    fn nodes(&self) -> &[NodeDef<D>] {
        match self {
            Self::Network(definition) => definition.nodes(),
            Self::Module(definition) => definition.graph().nodes(),
        }
    }

    fn connections(&self) -> &[ConnectionDef] {
        match self {
            Self::Network(definition) => definition.connections(),
            Self::Module(definition) => definition.graph().connections(),
        }
    }

    fn mappings(&self) -> &[ModuleInterfaceMapping] {
        match self {
            Self::Network(_) => &[],
            Self::Module(definition) => definition.graph().mappings(),
        }
    }

    fn instances(&self) -> &[ModuleInstanceDef<D>] {
        match self {
            Self::Network(definition) => definition.module_instances(),
            Self::Module(definition) => definition.graph().module_instances(),
        }
    }

    fn inputs(&self) -> Vec<crate::key::AnyModuleInputKey> {
        match self {
            Self::Network(_) => Vec::new(),
            Self::Module(definition) => definition.inputs().map(|input| input.key()).collect(),
        }
    }

    fn outputs(&self) -> Vec<crate::key::AnyModuleOutputKey> {
        match self {
            Self::Network(_) => Vec::new(),
            Self::Module(definition) => definition.outputs().map(|output| output.key()).collect(),
        }
    }
}

struct ScopeData<'a, D> {
    definition: ScopeDefinition<'a, D>,
    module: Option<QualifiedModuleRef>,
    input_bindings: BTreeMap<crate::key::AnyModuleInputKey, (usize, ConnectionEndpoint)>,
    output_mappings: BTreeMap<crate::key::AnyModuleOutputKey, ConnectionEndpoint>,
    node_inputs: BTreeMap<AnyInPortKey, AnyInPortKey>,
    node_outputs: BTreeMap<AnyOutPortKey, AnyOutPortKey>,
    instances: BTreeMap<ModuleInstanceKey, usize>,
}

type QualifiedInstancePath<D> = (ModuleInstanceDef<D>, Vec<ModuleInstanceKey>);

struct PrivateKeyAllocator {
    nodes: BTreeSet<NodeKey>,
    inputs: BTreeSet<AnyInPortKey>,
    outputs: BTreeSet<AnyOutPortKey>,
    connections: BTreeSet<ConnectionKey>,
    next_node: u128,
    next_input: u128,
    next_output: u128,
    next_connection: u128,
}

impl PrivateKeyAllocator {
    fn from_network<D>(definition: &UncheckedNetwork<D>) -> Self {
        Self {
            nodes: definition.nodes().iter().map(NodeDef::key).collect(),
            inputs: definition
                .nodes()
                .iter()
                .flat_map(|node| node.ports().inputs().iter().copied())
                .collect(),
            outputs: definition
                .nodes()
                .iter()
                .flat_map(|node| node.ports().outputs().iter().copied())
                .collect(),
            connections: definition
                .connections()
                .iter()
                .map(ConnectionDef::key)
                .collect(),
            next_node: 0,
            next_input: 0,
            next_output: 0,
            next_connection: 0,
        }
    }

    fn node(&mut self) -> NodeKey {
        loop {
            let key = NodeKey::from_u128(self.next_node);
            self.next_node = next_private_payload(self.next_node, "node");
            if self.nodes.insert(key) {
                return key;
            }
        }
    }

    fn input(&mut self, kind: SignalKind) -> AnyInPortKey {
        loop {
            let key = match kind {
                SignalKind::Level => InPortKey::<Level>::from_u128(self.next_input).into(),
                SignalKind::Pulse => InPortKey::<Pulse>::from_u128(self.next_input).into(),
            };
            self.next_input = next_private_payload(self.next_input, "input port");
            if self.inputs.insert(key) {
                return key;
            }
        }
    }

    fn output(&mut self, kind: SignalKind) -> AnyOutPortKey {
        loop {
            let key = match kind {
                SignalKind::Level => OutPortKey::<Level>::from_u128(self.next_output).into(),
                SignalKind::Pulse => OutPortKey::<Pulse>::from_u128(self.next_output).into(),
            };
            self.next_output = next_private_payload(self.next_output, "output port");
            if self.outputs.insert(key) {
                return key;
            }
        }
    }

    fn connection(&mut self) -> ConnectionKey {
        loop {
            let key = ConnectionKey::from_u128(self.next_connection);
            self.next_connection = next_private_payload(self.next_connection, "connection");
            if self.connections.insert(key) {
                return key;
            }
        }
    }
}

fn next_private_payload(value: u128, category: &'static str) -> u128 {
    match value.checked_add(1) {
        Some(next) => next,
        None => panic!("validated module lowering exhausted the private {category} key space"),
    }
}

struct ModuleLowerer<'a, D> {
    network: &'a UncheckedNetwork<D>,
    scopes: Vec<ScopeData<'a, D>>,
    allocator: PrivateKeyAllocator,
    nodes: Vec<NodeDef<D>>,
    connections: Vec<ConnectionDef>,
    metadata: LoweredMetadata<D>,
}

impl<'a, D> ModuleLowerer<'a, D> {
    fn new(network: &'a UncheckedNetwork<D>) -> Self {
        Self {
            network,
            scopes: Vec::new(),
            allocator: PrivateKeyAllocator::from_network(network),
            nodes: Vec::new(),
            connections: Vec::new(),
            metadata: LoweredMetadata::default(),
        }
    }

    fn lower(mut self) -> Result<LoweredNetwork<D>, &'static str> {
        let root = self.register_scope(
            ScopeDefinition::Network(self.network),
            None,
            BTreeMap::new(),
            true,
        )?;
        if root != 0 {
            return Err("network lowering root scope is not canonical");
        }
        self.lower_connections()?;
        self.capture_module_boundaries()?;

        let external_inputs = self
            .network
            .external_inputs()
            .iter()
            .map(|input| ExternalInputDef::new(input.key(), input.meta().clone()))
            .collect();
        let mut external_outputs = Vec::new();
        for output in self.network.external_outputs() {
            let source = self.resolve_endpoint(0, endpoint_from_signal(output.source()))?;
            external_outputs.push(ExternalOutputDef::new(
                output.key(),
                signal_from_endpoint(source)?,
                output.meta().clone(),
            ));
        }
        let definition = UncheckedNetwork::new(
            self.network.key(),
            self.network.time_domain_id(),
            self.network.meta().clone(),
            self.nodes,
            external_inputs,
            external_outputs,
            self.connections,
        );
        Ok(LoweredNetwork {
            definition,
            metadata: self.metadata,
        })
    }

    fn register_scope(
        &mut self,
        definition: ScopeDefinition<'a, D>,
        module: Option<QualifiedModuleRef>,
        input_bindings: BTreeMap<crate::key::AnyModuleInputKey, (usize, ConnectionEndpoint)>,
        top_level: bool,
    ) -> Result<usize, &'static str> {
        let output_mappings = definition
            .mappings()
            .iter()
            .filter_map(|mapping| match *mapping {
                ModuleInterfaceMapping::Output { output, source } => Some((output, source)),
                ModuleInterfaceMapping::Input { .. } => None,
            })
            .collect();
        let scope = self.scopes.len();
        self.scopes.push(ScopeData {
            definition,
            module: module.clone(),
            input_bindings,
            output_mappings,
            node_inputs: BTreeMap::new(),
            node_outputs: BTreeMap::new(),
            instances: BTreeMap::new(),
        });

        let mut nodes = self.scopes[scope].definition.nodes().to_vec();
        nodes.sort_by_key(NodeDef::key);
        for node in nodes {
            let flat_node = if top_level {
                node.key()
            } else {
                self.allocator.node()
            };
            let mut inputs = Vec::new();
            for input in node.ports().inputs() {
                let flat = if top_level {
                    *input
                } else {
                    self.allocator.input(input.kind())
                };
                self.scopes[scope].node_inputs.insert(*input, flat);
                inputs.push(flat);
            }
            let mut outputs = Vec::new();
            for output in node.ports().outputs() {
                let flat = if top_level {
                    *output
                } else {
                    self.allocator.output(output.kind())
                };
                self.scopes[scope].node_outputs.insert(*output, flat);
                outputs.push(flat);
            }
            self.nodes.push(NodeDef::new(
                flat_node,
                node.kind().clone(),
                NodePorts::with_input_roles(
                    inputs.clone(),
                    node.ports().input_roles().to_vec(),
                    outputs,
                ),
                node.meta().clone(),
            ));
            if let Some(module) = &module {
                let qualified = QualifiedNodeRef::new(module.instances().to_vec(), node.key());
                self.metadata
                    .qualified_node_lookup
                    .insert(qualified.clone(), flat_node);
                self.metadata
                    .qualified_node_reverse
                    .insert(flat_node, qualified);
                for (local, flat) in node.ports().inputs().iter().zip(&inputs) {
                    self.metadata.qualified_input_reverse.insert(
                        *flat,
                        QualifiedInPortRef::new(module.instances().to_vec(), *local),
                    );
                }
            }
        }

        let instances = canonical_instance_paths(
            self.scopes[scope].definition.instances(),
            module
                .as_ref()
                .map_or(&[][..], QualifiedModuleRef::instances),
        )?;
        for (instance, path) in instances {
            let qualified = QualifiedModuleRef::new(path);
            self.metadata
                .modules
                .insert(qualified.clone(), instance.module().clone());
            let bindings = instance
                .bindings()
                .bindings()
                .iter()
                .map(|binding| (binding.input(), (scope, binding.source())))
                .collect();
            let child = self.register_scope(
                ScopeDefinition::Module(instance.module().clone()),
                Some(qualified),
                bindings,
                false,
            )?;
            self.scopes[scope].instances.insert(instance.key(), child);
        }
        Ok(scope)
    }

    fn lower_connections(&mut self) -> Result<(), &'static str> {
        for scope in 0..self.scopes.len() {
            let module = self.scopes[scope].module.clone();
            let mut connections = self.scopes[scope].definition.connections().to_vec();
            connections.sort_by_key(ConnectionDef::key);
            for connection in connections {
                let source = self.resolve_endpoint(scope, connection.from())?;
                let target = self.resolve_target(scope, connection.to())?;
                let key = if scope == 0 {
                    connection.key()
                } else {
                    self.allocator.connection()
                };
                self.connections.push(ConnectionDef::new(
                    key,
                    source,
                    target,
                    connection.meta().clone(),
                ));
                if let Some(module) = &module {
                    self.metadata.qualified_connection_lookup.insert(
                        QualifiedConnectionRef::new(module.instances().to_vec(), connection.key()),
                        key,
                    );
                }
            }

            let mappings = self.scopes[scope].definition.mappings().to_vec();
            for mapping in mappings {
                let ModuleInterfaceMapping::Input { input, target } = mapping else {
                    continue;
                };
                let source =
                    self.resolve_endpoint(scope, ConnectionEndpoint::ModuleInput(input))?;
                let target = self.resolve_target(scope, target)?;
                self.connections.push(ConnectionDef::new(
                    self.allocator.connection(),
                    source,
                    target,
                    crate::metadata::DiagnosticMeta::default(),
                ));
            }
        }
        Ok(())
    }

    fn capture_module_boundaries(&mut self) -> Result<(), &'static str> {
        for scope in 1..self.scopes.len() {
            let Some(module) = self.scopes[scope].module.clone() else {
                return Err("non-root lowering scope lacks qualified module identity");
            };
            for input in self.scopes[scope].definition.inputs() {
                let source =
                    self.resolve_endpoint(scope, ConnectionEndpoint::ModuleInput(input))?;
                self.metadata
                    .module_input_sources
                    .insert((module.clone(), input), source);
            }
            for output in self.scopes[scope].definition.outputs() {
                let source = self.resolve_module_output(scope, output)?;
                self.metadata
                    .module_output_sources
                    .insert((module.clone(), output), source);
            }
        }
        Ok(())
    }

    fn resolve_target(
        &self,
        scope: usize,
        endpoint: ConnectionEndpoint,
    ) -> Result<ConnectionEndpoint, &'static str> {
        let ConnectionEndpoint::NodeInput(input) = endpoint else {
            return Err("validated lowered connection target is not a node input");
        };
        self.scopes[scope]
            .node_inputs
            .get(&input)
            .copied()
            .map(ConnectionEndpoint::NodeInput)
            .ok_or("validated lowered node input is missing")
    }

    fn resolve_endpoint(
        &self,
        scope: usize,
        endpoint: ConnectionEndpoint,
    ) -> Result<ConnectionEndpoint, &'static str> {
        self.resolve_endpoint_inner(scope, endpoint, &mut Vec::new())
    }

    fn resolve_endpoint_inner(
        &self,
        scope: usize,
        endpoint: ConnectionEndpoint,
        visited: &mut Vec<(usize, ConnectionEndpoint)>,
    ) -> Result<ConnectionEndpoint, &'static str> {
        if visited.contains(&(scope, endpoint)) {
            return Err("validated module source projection contains a resolution cycle");
        }
        visited.push((scope, endpoint));
        let resolved = match endpoint {
            ConnectionEndpoint::ExternalInput(_) if scope == 0 => endpoint,
            ConnectionEndpoint::NodeOutput(output) => self.scopes[scope]
                .node_outputs
                .get(&output)
                .copied()
                .map(ConnectionEndpoint::NodeOutput)
                .ok_or("validated module source references a missing node output")?,
            ConnectionEndpoint::ModuleInput(input) => {
                let (parent, source) = self.scopes[scope]
                    .input_bindings
                    .get(&input)
                    .copied()
                    .ok_or("validated module input lacks its exact binding")?;
                self.resolve_endpoint_inner(parent, source, visited)?
            }
            ConnectionEndpoint::ModuleOutput { instance, output } => {
                let child = *self.scopes[scope]
                    .instances
                    .get(&instance)
                    .ok_or("validated module output references a missing instance")?;
                let source = *self.scopes[child]
                    .output_mappings
                    .get(&output)
                    .ok_or("validated module output lacks its exact mapping")?;
                self.resolve_endpoint_inner(child, source, visited)?
            }
            ConnectionEndpoint::ExternalInput(_)
            | ConnectionEndpoint::NodeInput(_)
            | ConnectionEndpoint::ExternalOutput(_) => {
                return Err("validated module source has an invalid direction or scope");
            }
        };
        visited.pop();
        Ok(resolved)
    }

    fn resolve_module_output(
        &self,
        scope: usize,
        output: crate::key::AnyModuleOutputKey,
    ) -> Result<ConnectionEndpoint, &'static str> {
        let source = *self.scopes[scope]
            .output_mappings
            .get(&output)
            .ok_or("validated module output lacks its exact mapping")?;
        self.resolve_endpoint(scope, source)
    }
}

fn canonical_instance_paths<D>(
    instances: &[ModuleInstanceDef<D>],
    prefix: &[ModuleInstanceKey],
) -> Result<Vec<QualifiedInstancePath<D>>, &'static str> {
    let by_key: BTreeMap<_, _> = instances
        .iter()
        .map(|instance| (instance.key(), instance))
        .collect();
    let mut ordered: Vec<_> = instances.iter().collect();
    ordered.sort_by_key(|instance| instance.key());
    let mut result = Vec::with_capacity(ordered.len());
    for instance in ordered {
        let mut reversed = Vec::new();
        let mut visited = BTreeSet::new();
        let mut current = Some(instance.key());
        while let Some(key) = current {
            if !visited.insert(key) {
                return Err("validated module hierarchy contains a cycle during lowering");
            }
            reversed.push(key);
            current = match by_key.get(&key) {
                Some(candidate) => candidate.parent(),
                None => return Err("validated module hierarchy references a missing parent"),
            };
        }
        reversed.reverse();
        let mut path = prefix.to_vec();
        path.extend(reversed);
        result.push((instance.clone(), path));
    }
    Ok(result)
}

fn endpoint_from_signal(source: crate::key::AnySignalSourceKey) -> ConnectionEndpoint {
    match source {
        crate::key::AnySignalSourceKey::Level(crate::key::SignalSourceKey::ExternalInput(key)) => {
            ConnectionEndpoint::ExternalInput(key.into())
        }
        crate::key::AnySignalSourceKey::Level(crate::key::SignalSourceKey::NodeOutput(key)) => {
            ConnectionEndpoint::NodeOutput(key.into())
        }
        crate::key::AnySignalSourceKey::Level(crate::key::SignalSourceKey::ModuleOutput {
            instance,
            output,
        }) => ConnectionEndpoint::ModuleOutput {
            instance,
            output: output.into(),
        },
        crate::key::AnySignalSourceKey::Pulse(crate::key::SignalSourceKey::ExternalInput(key)) => {
            ConnectionEndpoint::ExternalInput(key.into())
        }
        crate::key::AnySignalSourceKey::Pulse(crate::key::SignalSourceKey::NodeOutput(key)) => {
            ConnectionEndpoint::NodeOutput(key.into())
        }
        crate::key::AnySignalSourceKey::Pulse(crate::key::SignalSourceKey::ModuleOutput {
            instance,
            output,
        }) => ConnectionEndpoint::ModuleOutput {
            instance,
            output: output.into(),
        },
    }
}

fn signal_from_endpoint(
    endpoint: ConnectionEndpoint,
) -> Result<crate::key::AnySignalSourceKey, &'static str> {
    match endpoint {
        ConnectionEndpoint::ExternalInput(AnyExternalInputKey::Level(key)) => Ok(
            crate::key::AnySignalSourceKey::Level(crate::key::SignalSourceKey::ExternalInput(key)),
        ),
        ConnectionEndpoint::ExternalInput(AnyExternalInputKey::Pulse(key)) => Ok(
            crate::key::AnySignalSourceKey::Pulse(crate::key::SignalSourceKey::ExternalInput(key)),
        ),
        ConnectionEndpoint::NodeOutput(AnyOutPortKey::Level(key)) => Ok(
            crate::key::AnySignalSourceKey::Level(crate::key::SignalSourceKey::NodeOutput(key)),
        ),
        ConnectionEndpoint::NodeOutput(AnyOutPortKey::Pulse(key)) => Ok(
            crate::key::AnySignalSourceKey::Pulse(crate::key::SignalSourceKey::NodeOutput(key)),
        ),
        ConnectionEndpoint::NodeInput(_)
        | ConnectionEndpoint::ModuleInput(_)
        | ConnectionEndpoint::ModuleOutput { .. }
        | ConnectionEndpoint::ExternalOutput(_) => {
            Err("lowered external output did not resolve to an ordinary signal source")
        }
    }
}

fn lower_module_instances<D>(definition: &UncheckedNetwork<D>) -> LoweredNetwork<D> {
    match ModuleLowerer::new(definition).lower() {
        Ok(lowered) => lowered,
        Err(reason) => panic!(
            "validated module lowering must resolve every qualified source and target: {reason}"
        ),
    }
}

fn clone_complete_definition<D>(definition: &UncheckedNetwork<D>) -> UncheckedNetwork<D> {
    UncheckedNetwork::new_with_instances(
        definition.key(),
        definition.time_domain_id(),
        definition.meta().clone(),
        definition.nodes().to_vec(),
        definition
            .external_inputs()
            .iter()
            .map(|input| ExternalInputDef::new(input.key(), input.meta().clone()))
            .collect(),
        definition
            .external_outputs()
            .iter()
            .map(|output| {
                ExternalOutputDef::new(output.key(), output.source(), output.meta().clone())
            })
            .collect(),
        definition.connections().to_vec(),
        definition.module_instances().to_vec(),
    )
}

fn clone_definition<D>(definition: &UncheckedNetwork<D>) -> UncheckedNetwork<D> {
    let nodes = definition
        .nodes()
        .iter()
        .map(|node| {
            let kind = match node.kind() {
                NodeKind::Constant(config) => NodeKind::constant(config.value()),
                NodeKind::Not => NodeKind::not(),
                NodeKind::All => NodeKind::all(),
                NodeKind::Any => NodeKind::any(),
                NodeKind::Parity => NodeKind::parity(),
                NodeKind::AtLeast(config) => NodeKind::at_least(config.threshold),
                NodeKind::Select => NodeKind::select(),
                NodeKind::Merge => NodeKind::merge(),
                NodeKind::Coalesce => NodeKind::coalesce(),
                NodeKind::Zip => NodeKind::zip(),
                NodeKind::Toggle(config) => NodeKind::toggle(config.initial),
                NodeKind::PulseDelay(config) => NodeKind::pulse_delay(config.delay),
            };
            crate::authored::NodeDef::new(
                node.key(),
                kind,
                crate::authored::NodePorts::with_input_roles(
                    node.ports().inputs().to_vec(),
                    node.ports().input_roles().to_vec(),
                    node.ports().outputs().to_vec(),
                ),
                node.meta().clone(),
            )
        })
        .collect();
    let external_inputs = definition
        .external_inputs()
        .iter()
        .map(|input| crate::authored::ExternalInputDef::new(input.key(), input.meta().clone()))
        .collect();
    let external_outputs = definition
        .external_outputs()
        .iter()
        .map(|output| {
            crate::authored::ExternalOutputDef::new(
                output.key(),
                output.source(),
                output.meta().clone(),
            )
        })
        .collect();
    let connections = definition
        .connections()
        .iter()
        .map(|connection| {
            crate::authored::ConnectionDef::new(
                connection.key(),
                connection.from(),
                connection.to(),
                connection.meta().clone(),
            )
        })
        .collect();
    UncheckedNetwork::new(
        definition.key(),
        definition.time_domain_id(),
        definition.meta().clone(),
        nodes,
        external_inputs,
        external_outputs,
        connections,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::authored::{ConnectionDef, ExternalInputDef, ExternalOutputDef, NodeDef, NodePorts};
    use crate::key::{
        ExternalInputKey, ExternalOutputKey, InPortKey, NetworkKey, OutPortKey, SignalSourceKey,
    };
    use crate::metadata::DiagnosticMeta;
    use crate::signal::Level;
    use std::collections::BTreeMap;

    fn constant_network(value: LogicLevel) -> UncheckedNetwork<()> {
        let output = OutPortKey::<Level>::from_u128(1);
        let external_output = ExternalOutputKey::<Level>::from_u128(2);
        UncheckedNetwork::new(
            NetworkKey::from_u128(3),
            TimeDomainId::from_u128(4),
            DiagnosticMeta::default(),
            vec![NodeDef::new(
                NodeKey::from_u128(5),
                NodeKind::constant(value),
                NodePorts::new(Vec::new(), vec![output.into()]),
                DiagnosticMeta::default(),
            )],
            Vec::new(),
            vec![ExternalOutputDef::new(
                external_output.into(),
                SignalSourceKey::NodeOutput(output).into(),
                DiagnosticMeta::default(),
            )],
            Vec::new(),
        )
    }

    fn external_not_network() -> UncheckedNetwork<()> {
        let input = ExternalInputKey::<Level>::from_u128(1);
        let not_input = InPortKey::<Level>::from_u128(2);
        let not_output = OutPortKey::<Level>::from_u128(3);
        let output = ExternalOutputKey::<Level>::from_u128(4);
        UncheckedNetwork::new(
            NetworkKey::from_u128(5),
            TimeDomainId::from_u128(6),
            DiagnosticMeta::default(),
            vec![NodeDef::new(
                NodeKey::from_u128(7),
                NodeKind::not(),
                NodePorts::new(vec![not_input.into()], vec![not_output.into()]),
                DiagnosticMeta::default(),
            )],
            vec![ExternalInputDef::new(
                input.into(),
                DiagnosticMeta::default(),
            )],
            vec![ExternalOutputDef::new(
                output.into(),
                SignalSourceKey::NodeOutput(not_output).into(),
                DiagnosticMeta::default(),
            )],
            vec![ConnectionDef::new(
                ConnectionKey::from_u128(8),
                input.into(),
                not_input.into(),
                DiagnosticMeta::default(),
            )],
        )
    }

    fn all_network(
        arity: usize,
        duplicate_source: bool,
        reverse_port_order: bool,
    ) -> UncheckedNetwork<()> {
        let inputs = (0..arity)
            .map(|index| ExternalInputKey::<Level>::from_u128(100 + index as u128))
            .collect::<Vec<_>>();
        let ports = (0..arity)
            .map(|index| InPortKey::<Level>::from_u128(200 + index as u128))
            .collect::<Vec<_>>();
        let mut authored_ports = ports.clone();
        if reverse_port_order {
            authored_ports.reverse();
        }
        let all_output = OutPortKey::<Level>::from_u128(300);
        let external_output = ExternalOutputKey::<Level>::from_u128(400);
        let connections = ports
            .iter()
            .enumerate()
            .map(|(index, port)| {
                let source = if duplicate_source && index > 0 {
                    inputs[0]
                } else {
                    inputs[index]
                };
                ConnectionDef::new(
                    ConnectionKey::from_u128(500 + index as u128),
                    source.into(),
                    (*port).into(),
                    DiagnosticMeta::default(),
                )
            })
            .collect();
        UncheckedNetwork::new(
            NetworkKey::from_u128(1),
            TimeDomainId::from_u128(2),
            DiagnosticMeta::default(),
            vec![NodeDef::new(
                NodeKey::from_u128(3),
                NodeKind::all(),
                NodePorts::new(
                    authored_ports.into_iter().map(Into::into).collect(),
                    vec![all_output.into()],
                ),
                DiagnosticMeta::default(),
            )],
            inputs
                .into_iter()
                .map(|key| ExternalInputDef::new(key.into(), DiagnosticMeta::default()))
                .collect(),
            vec![ExternalOutputDef::new(
                external_output.into(),
                SignalSourceKey::NodeOutput(all_output).into(),
                DiagnosticMeta::default(),
            )],
            connections,
        )
    }

    fn variadic_network(
        kind: NodeKind<()>,
        arity: usize,
        duplicate_source: bool,
        reverse_port_order: bool,
    ) -> UncheckedNetwork<()> {
        let inputs = (0..arity)
            .map(|index| ExternalInputKey::<Level>::from_u128(100 + index as u128))
            .collect::<Vec<_>>();
        let ports = (0..arity)
            .map(|index| InPortKey::<Level>::from_u128(200 + index as u128))
            .collect::<Vec<_>>();
        let mut authored_ports = ports.clone();
        if reverse_port_order {
            authored_ports.reverse();
        }
        let output = OutPortKey::<Level>::from_u128(300);
        let connections = ports
            .iter()
            .enumerate()
            .map(|(index, port)| {
                ConnectionDef::new(
                    crate::key::ConnectionKey::from_u128(500 + index as u128),
                    if duplicate_source && index > 0 {
                        inputs[0].into()
                    } else {
                        inputs[index].into()
                    },
                    (*port).into(),
                    DiagnosticMeta::default(),
                )
            })
            .collect();
        UncheckedNetwork::new(
            NetworkKey::from_u128(1),
            TimeDomainId::from_u128(2),
            DiagnosticMeta::default(),
            vec![NodeDef::new(
                crate::key::NodeKey::from_u128(3),
                kind,
                NodePorts::new(
                    authored_ports.into_iter().map(Into::into).collect(),
                    vec![output.into()],
                ),
                DiagnosticMeta::default(),
            )],
            inputs
                .into_iter()
                .map(|key| ExternalInputDef::new(key.into(), DiagnosticMeta::default()))
                .collect(),
            vec![ExternalOutputDef::new(
                ExternalOutputKey::<Level>::from_u128(400).into(),
                crate::key::SignalSourceKey::NodeOutput(output).into(),
                DiagnosticMeta::default(),
            )],
            connections,
        )
    }

    fn select_network(role_order: [InputPortRole; 3]) -> UncheckedNetwork<()> {
        let inputs = [
            ExternalInputKey::<Level>::from_u128(100),
            ExternalInputKey::<Level>::from_u128(101),
            ExternalInputKey::<Level>::from_u128(102),
        ];
        let ports = [
            InPortKey::<Level>::from_u128(200),
            InPortKey::<Level>::from_u128(201),
            InPortKey::<Level>::from_u128(202),
        ];
        let output = OutPortKey::<Level>::from_u128(300);
        UncheckedNetwork::new(
            NetworkKey::from_u128(1),
            TimeDomainId::from_u128(2),
            DiagnosticMeta::default(),
            vec![NodeDef::new(
                crate::key::NodeKey::from_u128(3),
                NodeKind::select(),
                NodePorts::with_input_roles(
                    ports.into_iter().map(Into::into).collect(),
                    role_order.to_vec(),
                    vec![output.into()],
                ),
                DiagnosticMeta::default(),
            )],
            inputs
                .into_iter()
                .map(|key| ExternalInputDef::new(key.into(), DiagnosticMeta::default()))
                .collect(),
            vec![ExternalOutputDef::new(
                ExternalOutputKey::<Level>::from_u128(400).into(),
                crate::key::SignalSourceKey::NodeOutput(output).into(),
                DiagnosticMeta::default(),
            )],
            ports
                .into_iter()
                .zip(inputs)
                .enumerate()
                .map(|(index, (port, input))| {
                    ConnectionDef::new(
                        crate::key::ConnectionKey::from_u128(500 + index as u128),
                        input.into(),
                        port.into(),
                        DiagnosticMeta::default(),
                    )
                })
                .collect(),
        )
    }

    fn compile_network(network: UncheckedNetwork<()>) -> CompiledNetwork<()> {
        network
            .validate()
            .require_artifact()
            .unwrap_or_else(|_| panic!("fixture must validate"))
            .compile()
            .require_artifact()
            .unwrap_or_else(|_| panic!("fixture must compile"))
    }

    fn operation_index(compiled: &CompiledNetwork<()>, vertex: ReactionVertex) -> OperationIndex {
        compiled.inner.operation_lookup[&vertex]
    }

    fn network(reverse_claim_order: bool) -> UncheckedNetwork<()> {
        let input = ExternalInputKey::<Level>::from_u128(1);
        let not_input = InPortKey::<Level>::from_u128(2);
        let not_output = OutPortKey::<Level>::from_u128(3);
        let constant_output = OutPortKey::<Level>::from_u128(4);
        let output = ExternalOutputKey::<Level>::from_u128(5);
        let constant = NodeDef::new(
            NodeKey::from_u128(10),
            NodeKind::constant(LogicLevel::High),
            NodePorts::new(Vec::new(), vec![constant_output.into()]),
            DiagnosticMeta::default(),
        );
        let inverter = NodeDef::new(
            NodeKey::from_u128(20),
            NodeKind::not(),
            NodePorts::new(vec![not_input.into()], vec![not_output.into()]),
            DiagnosticMeta::default(),
        );
        let external_input = ExternalInputDef::new(input.into(), DiagnosticMeta::default());
        let external_output = ExternalOutputDef::new(
            output.into(),
            SignalSourceKey::NodeOutput(not_output).into(),
            DiagnosticMeta::default(),
        );
        let constant_connection = ConnectionDef::new(
            ConnectionKey::from_u128(30),
            constant_output.into(),
            not_input.into(),
            DiagnosticMeta::default(),
        );
        let mut nodes = vec![constant, inverter];
        let mut connections = vec![constant_connection];
        if reverse_claim_order {
            nodes.reverse();
            connections.reverse();
        }
        UncheckedNetwork::new(
            NetworkKey::from_u128(100),
            TimeDomainId::from_u128(200),
            DiagnosticMeta::default(),
            nodes,
            vec![external_input],
            vec![external_output],
            connections,
        )
    }

    fn compiled(reverse_claim_order: bool) -> CompiledNetwork<()> {
        compile_network(network(reverse_claim_order))
    }

    #[test]
    fn full_evaluator_applies_constant_laws() {
        for value in [LogicLevel::Low, LogicLevel::High] {
            let compiled = compile_network(constant_network(value));
            let evaluation = compiled
                .evaluate_full(&BTreeMap::new())
                .unwrap_or_else(|| panic!("constant topology must evaluate"));
            let operation = operation_index(
                &compiled,
                ReactionVertex::NodeOperation(NodeKey::from_u128(5)),
            );
            assert_eq!(evaluation.values[operation.0], value);
        }
    }

    #[test]
    fn full_evaluator_applies_all_total_law_for_small_arities() {
        for arity in 0..=3 {
            let compiled = compile_network(all_network(arity, false, false));
            for valuation in 0..(1_u8 << arity) {
                let facts = (0..arity)
                    .map(|index| {
                        (
                            ExternalInputKey::from_u128(100 + index as u128),
                            if valuation & (1 << index) == 0 {
                                LogicLevel::Low
                            } else {
                                LogicLevel::High
                            },
                        )
                    })
                    .collect();
                let evaluated = compiled
                    .evaluate_full(&facts)
                    .unwrap_or_else(|| panic!("complete All valuation must evaluate"));
                let expected = if valuation == (1_u8 << arity) - 1 {
                    LogicLevel::High
                } else {
                    LogicLevel::Low
                };
                assert_eq!(
                    evaluated.external_outputs[&ExternalOutputKey::from_u128(400)],
                    expected
                );
            }
        }
    }

    #[test]
    fn full_evaluator_exhausts_remaining_variadic_level_laws() {
        for arity in 0..=4 {
            let valuations = 1_u8 << arity;
            for valuation in 0..valuations {
                let facts = (0..arity)
                    .map(|index| {
                        (
                            ExternalInputKey::from_u128(100 + index as u128),
                            if valuation & (1 << index) == 0 {
                                LogicLevel::Low
                            } else {
                                LogicLevel::High
                            },
                        )
                    })
                    .collect::<BTreeMap<_, _>>();
                let high_count = valuation.count_ones() as u64;
                for (kind, expected) in [
                    (NodeKind::any(), high_count > 0),
                    (NodeKind::parity(), high_count % 2 == 1),
                ] {
                    let evaluated = compile_network(variadic_network(kind, arity, false, false))
                        .evaluate_full(&facts)
                        .unwrap_or_else(|| panic!("complete variadic valuation must evaluate"));
                    assert_eq!(
                        evaluated.external_outputs[&ExternalOutputKey::from_u128(400)],
                        if expected {
                            LogicLevel::High
                        } else {
                            LogicLevel::Low
                        }
                    );
                }
                for threshold in 0..=(arity as u64 + 1) {
                    let evaluated = compile_network(variadic_network(
                        NodeKind::at_least(threshold),
                        arity,
                        false,
                        false,
                    ))
                    .evaluate_full(&facts)
                    .unwrap_or_else(|| panic!("complete threshold valuation must evaluate"));
                    assert_eq!(
                        evaluated.external_outputs[&ExternalOutputKey::from_u128(400)],
                        if high_count >= threshold {
                            LogicLevel::High
                        } else {
                            LogicLevel::Low
                        }
                    );
                }
            }
        }
    }

    #[test]
    fn select_exhausts_all_values_and_uses_semantic_roles() {
        for roles in [
            [
                InputPortRole::Selector,
                InputPortRole::WhenLow,
                InputPortRole::WhenHigh,
            ],
            [
                InputPortRole::WhenHigh,
                InputPortRole::Selector,
                InputPortRole::WhenLow,
            ],
        ] {
            let compiled = compile_network(select_network(roles));
            for valuation in 0..8_u8 {
                let facts = (0..3)
                    .map(|index| {
                        (
                            ExternalInputKey::from_u128(100 + index as u128),
                            if valuation & (1 << index) != 0 {
                                LogicLevel::High
                            } else {
                                LogicLevel::Low
                            },
                        )
                    })
                    .collect();
                let role_value = |role| {
                    let index = roles
                        .iter()
                        .position(|candidate| *candidate == role)
                        .unwrap_or_else(|| panic!("fixture contains every Select role"));
                    valuation & (1 << index) != 0
                };
                let expected = if role_value(InputPortRole::Selector) {
                    role_value(InputPortRole::WhenHigh)
                } else {
                    role_value(InputPortRole::WhenLow)
                };
                let evaluated = compiled
                    .evaluate_full(&facts)
                    .unwrap_or_else(|| panic!("complete Select valuation must evaluate"));
                assert_eq!(
                    evaluated.external_outputs[&ExternalOutputKey::from_u128(400)],
                    if expected {
                        LogicLevel::High
                    } else {
                        LogicLevel::Low
                    }
                );
            }
        }
    }

    #[test]
    fn remaining_variadics_preserve_duplicate_port_multiplicity_and_permutation() {
        let facts = BTreeMap::from([
            (ExternalInputKey::from_u128(100), LogicLevel::High),
            (ExternalInputKey::from_u128(101), LogicLevel::Low),
        ]);
        for (kind, expected) in [
            (NodeKind::any(), LogicLevel::High),
            (NodeKind::parity(), LogicLevel::Low),
            (NodeKind::at_least(2), LogicLevel::High),
        ] {
            let forward = compile_network(variadic_network(kind, 2, true, false));
            let kind = match &forward.inner.nodes[0].kind {
                CompiledNodeKind::Any => NodeKind::any(),
                CompiledNodeKind::Parity => NodeKind::parity(),
                CompiledNodeKind::AtLeast(threshold) => NodeKind::at_least(*threshold),
                _ => panic!("fixture must compile the requested variadic kind"),
            };
            let reverse = compile_network(variadic_network(kind, 2, true, true));
            assert_eq!(
                forward
                    .evaluate_full(&facts)
                    .unwrap_or_else(|| panic!("duplicate-source valuation must evaluate"))
                    .external_outputs,
                reverse
                    .evaluate_full(&facts)
                    .unwrap_or_else(|| panic!("permuted valuation must evaluate"))
                    .external_outputs
            );
            assert_eq!(
                forward
                    .evaluate_full(&facts)
                    .unwrap_or_else(|| panic!("duplicate-source valuation must evaluate"))
                    .external_outputs[&ExternalOutputKey::from_u128(400)],
                expected
            );
        }
    }

    #[test]
    fn compilation_retains_duplicate_source_port_multiplicity_for_all() {
        let compiled = compile_network(all_network(2, true, false));
        assert_eq!(compiled.inner.connections.len(), 2);
        let low = BTreeMap::from([
            (ExternalInputKey::from_u128(100), LogicLevel::Low),
            (ExternalInputKey::from_u128(101), LogicLevel::High),
        ]);
        let high = BTreeMap::from([
            (ExternalInputKey::from_u128(100), LogicLevel::High),
            (ExternalInputKey::from_u128(101), LogicLevel::Low),
        ]);
        assert_eq!(
            compiled.evaluate_full(&low).unwrap().external_outputs
                [&ExternalOutputKey::from_u128(400)],
            LogicLevel::Low
        );
        assert_eq!(
            compiled.evaluate_full(&high).unwrap().external_outputs
                [&ExternalOutputKey::from_u128(400)],
            LogicLevel::High
        );
    }

    #[test]
    fn all_execution_is_invariant_under_authored_port_order_permutation() {
        let forward = compile_network(all_network(3, false, false));
        let reversed = compile_network(all_network(3, false, true));
        assert_eq!(forward.fingerprint(), reversed.fingerprint());

        for valuation in 0..8_u8 {
            let facts = (0..3)
                .map(|index| {
                    (
                        ExternalInputKey::from_u128(100 + index as u128),
                        if valuation & (1 << index) == 0 {
                            LogicLevel::Low
                        } else {
                            LogicLevel::High
                        },
                    )
                })
                .collect();
            assert_eq!(
                forward.evaluate_full(&facts).unwrap().external_outputs,
                reversed.evaluate_full(&facts).unwrap().external_outputs
            );
        }
    }

    #[test]
    fn full_evaluator_propagates_external_facts_through_not_and_output() {
        let compiled = compile_network(external_not_network());
        let input = ExternalInputKey::<Level>::from_u128(1);
        let input_operation =
            operation_index(&compiled, ReactionVertex::ExternalInput(input.into()));
        let not_operation = operation_index(
            &compiled,
            ReactionVertex::NodeOperation(NodeKey::from_u128(7)),
        );
        let output_operation = operation_index(
            &compiled,
            ReactionVertex::ExternalOutput(ExternalOutputKey::<Level>::from_u128(4).into()),
        );
        for (provided, expected) in [
            (LogicLevel::Low, LogicLevel::High),
            (LogicLevel::High, LogicLevel::Low),
        ] {
            let mut facts = BTreeMap::new();
            facts.insert(input, provided);
            let evaluation = compiled
                .evaluate_full(&facts)
                .unwrap_or_else(|| panic!("complete external facts must evaluate"));

            assert_eq!(evaluation.values[input_operation.0], provided);
            assert_eq!(evaluation.values[not_operation.0], expected);
            assert_eq!(evaluation.values[output_operation.0], expected);
        }
    }

    #[test]
    fn full_evaluator_settles_a_multi_node_chain_once_after_predecessors() {
        let compiled = compiled(false);
        let mut facts = BTreeMap::new();
        facts.insert(ExternalInputKey::<Level>::from_u128(1), LogicLevel::Low);
        let evaluation = compiled
            .evaluate_full(&facts)
            .unwrap_or_else(|| panic!("complete external facts must evaluate"));

        let constant = operation_index(
            &compiled,
            ReactionVertex::NodeOperation(NodeKey::from_u128(10)),
        );
        let inverter = operation_index(
            &compiled,
            ReactionVertex::NodeOperation(NodeKey::from_u128(20)),
        );
        let observed = operation_index(
            &compiled,
            ReactionVertex::ExternalOutput(ExternalOutputKey::<Level>::from_u128(5).into()),
        );
        assert_eq!(evaluation.values[constant.0], LogicLevel::High);
        assert_eq!(evaluation.values[inverter.0], LogicLevel::Low);
        assert_eq!(evaluation.values[observed.0], LogicLevel::Low);
        assert!(evaluation.execution_counts.iter().all(|count| *count == 1));
        for (index, predecessors) in compiled.inner.predecessors.iter().enumerate() {
            for predecessor in predecessors {
                assert!(predecessor.0 < index);
                assert_eq!(evaluation.execution_counts[predecessor.0], 1);
            }
        }
    }

    #[test]
    fn full_evaluator_does_not_invent_missing_external_facts() {
        let compiled = compile_network(external_not_network());
        assert_eq!(compiled.evaluate_full(&BTreeMap::new()), None);
    }

    #[test]
    fn compilation_retains_identity_and_supports_both_validated_entry_points() {
        let validated = network(false)
            .validate()
            .require_artifact()
            .unwrap_or_else(|_| panic!("fixture must validate"));
        let borrowed = validated
            .compile_ref()
            .require_artifact()
            .unwrap_or_else(|_| panic!("borrowing compilation must succeed"));
        assert_eq!(borrowed.network_key(), validated.network_key());
        assert_eq!(borrowed.time_domain_id(), validated.time_domain_id());
        assert_eq!(borrowed.fingerprint(), validated.fingerprint());
        assert_eq!(
            borrowed.input_schema_fingerprint(),
            validated.input_schema_fingerprint()
        );
        let owned = validated
            .compile()
            .require_artifact()
            .unwrap_or_else(|_| panic!("consuming compilation must succeed"));
        assert_eq!(borrowed.fingerprint(), owned.fingerprint());
        let clone = owned.clone();
        assert!(Arc::ptr_eq(&owned.inner, &clone.inner));
    }

    #[test]
    fn compilation_resolves_complete_restricted_dense_topology() {
        let compiled = compiled(false);
        let inner = &compiled.inner;
        assert_eq!(inner.nodes.len(), 2);
        assert_eq!(inner.ports.len(), 3);
        assert_eq!(inner.external_inputs.len(), 1);
        assert_eq!(inner.external_outputs.len(), 1);
        assert_eq!(inner.connections.len(), 1);
        assert_eq!(inner.operations.len(), 6);
        assert!(inner.check_invariants().is_ok());
        assert!(
            inner
                .successors
                .iter()
                .enumerate()
                .all(|(from, targets)| { targets.iter().all(|target| from < target.0) })
        );
        assert_eq!(
            inner.nodes[0].kind,
            CompiledNodeKind::Constant(LogicLevel::High)
        );
        assert_eq!(inner.nodes[1].kind, CompiledNodeKind::Not);
    }

    #[test]
    fn compilation_is_stable_under_authored_claim_permutation() {
        let first = compiled(false);
        let second = compiled(true);
        assert_eq!(first.network_key(), second.network_key());
        assert_eq!(first.fingerprint(), second.fingerprint());
        assert_eq!(
            first.input_schema_fingerprint(),
            second.input_schema_fingerprint()
        );
        assert_eq!(
            relation(&first.inner),
            relation(&second.inner),
            "reaction adjacency must retain the same stable relation"
        );
    }

    #[test]
    fn compiled_invariant_checker_detects_seeded_forward_edge_violation() {
        let validated = network(false)
            .validate()
            .require_artifact()
            .unwrap_or_else(|_| panic!("fixture must validate"));
        let graph = validated.reaction_dependencies();
        let order = graph.topological_order();
        let mut inner = CompiledInner::build(
            &validated,
            clone_definition(validated.definition()),
            &graph,
            &order,
            LoweredMetadata::default(),
        );
        inner.successors[1].push(OperationIndex(0));
        inner.successors[1].sort();
        assert_eq!(
            inner.check_invariants(),
            Err("compiled reaction dependency is not forward")
        );
    }

    #[test]
    fn compiled_invariant_checker_detects_seeded_predecessor_asymmetry() {
        let validated = network(false)
            .validate()
            .require_artifact()
            .unwrap_or_else(|_| panic!("fixture must validate"));
        let graph = validated.reaction_dependencies();
        let order = graph.topological_order();
        let mut inner = CompiledInner::build(
            &validated,
            clone_definition(validated.definition()),
            &graph,
            &order,
            LoweredMetadata::default(),
        );
        inner.predecessors[0].push(OperationIndex(1));
        assert_eq!(
            inner.check_invariants(),
            Err("compiled reaction adjacency is inconsistent")
        );
    }

    #[test]
    fn compiled_invariant_checker_detects_seeded_duplicate_adjacency() {
        let validated = network(false)
            .validate()
            .require_artifact()
            .unwrap_or_else(|_| panic!("fixture must validate"));
        let graph = validated.reaction_dependencies();
        let order = graph.topological_order();
        let mut inner = CompiledInner::build(
            &validated,
            clone_definition(validated.definition()),
            &graph,
            &order,
            LoweredMetadata::default(),
        );
        let (index, target) = inner
            .successors
            .iter()
            .enumerate()
            .find_map(|(index, successors)| successors.first().map(|target| (index, *target)))
            .unwrap_or_else(|| panic!("fixture must contain a dependency"));
        inner.successors[index].push(target);
        assert_eq!(
            inner.check_invariants(),
            Err("compiled reaction adjacency order is invalid")
        );
    }

    fn relation(inner: &CompiledInner<()>) -> Vec<(ReactionVertex, ReactionVertex)> {
        let mut vertices =
            vec![ReactionVertex::NodeOperation(NodeKey::from_u128(0)); inner.operations.len()];
        for (vertex, index) in &inner.operation_lookup {
            vertices[index.0] = *vertex;
        }
        let mut relation = Vec::new();
        for (from, targets) in inner.successors.iter().enumerate() {
            for target in targets {
                relation.push((vertices[from], vertices[target.0]));
            }
        }
        relation
    }
}
