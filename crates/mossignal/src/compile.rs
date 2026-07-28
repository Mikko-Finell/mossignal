//! Immutable dense execution topology for validated restricted networks.

use crate::authored::{ConnectionEndpoint, InputPortRole, NodeKind, UncheckedNetwork};
use crate::diagnostics::{DiagnosticSet, Report};
use crate::identity::{InputSchemaFingerprint, NetworkFingerprint, TimeDomainId};
use crate::input::{InputDeltaBuilder, InputSnapshotBuilder};
use crate::key::{
    AnyExternalInputKey, AnyExternalOutputKey, AnyInPortKey, AnyOutPortKey, ConnectionKey,
    ExternalInputKey, ExternalOutputKey, InPortKey, NetworkKey, NodeKey,
};
use crate::machine::Machine;
use crate::policy::RuntimePolicy;
use crate::signal::{Level, LogicLevel, Pulse, PulseCount, SignalKind};
use crate::validation::{ReactionDependencyGraph, ReactionVertex, ValidatedNetwork};
use std::collections::BTreeMap;
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
    pub(crate) causes: Vec<EvaluationCause>,
    pub(crate) external_outputs: BTreeMap<ExternalOutputKey<Level>, LogicLevel>,
    pub(crate) pulse_outputs: BTreeMap<ExternalOutputKey<Pulse>, PulseCount>,
    #[cfg(test)]
    execution_counts: Vec<usize>,
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
    PulseMerge {
        node: NodeKey,
        contributions: Vec<PulseEvaluationContribution>,
        result: PulseCount,
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
        let graph = validated.reaction_dependencies();
        let inner = CompiledInner::build(validated, &graph);
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
            .evaluate_reaction(external_inputs, &BTreeMap::new())
            .ok()
    }

    pub(crate) fn evaluate_reaction(
        &self,
        external_levels: &BTreeMap<ExternalInputKey<Level>, LogicLevel>,
        external_pulses: &BTreeMap<ExternalInputKey<Pulse>, PulseCount>,
    ) -> Result<FullEvaluation, EvaluationFailure> {
        self.inner
            .evaluate_reaction(external_levels, external_pulses)
    }

    pub(crate) fn operation_count(&self) -> usize {
        self.inner.operations.len()
    }
}

impl<D> CompiledInner<D> {
    fn evaluate_reaction(
        &self,
        external_levels: &BTreeMap<ExternalInputKey<Level>, LogicLevel>,
        external_pulses: &BTreeMap<ExternalInputKey<Pulse>, PulseCount>,
    ) -> Result<FullEvaluation, EvaluationFailure> {
        let mut values = vec![None; self.operations.len()];
        let mut causes = Vec::with_capacity(self.operations.len());
        let mut external_outputs = BTreeMap::new();
        let mut pulse_outputs = BTreeMap::new();
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
                            EvaluationCause::PulseMerge {
                                node: *key,
                                contributions,
                                result: total,
                            },
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
            causes,
            external_outputs,
            pulse_outputs,
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

    fn build(validated: &ValidatedNetwork<D>, graph: &ReactionDependencyGraph) -> Self {
        let definition = clone_definition(validated.definition());
        let mut nodes = Vec::new();
        let mut ports = Vec::new();
        let mut node_lookup = BTreeMap::new();
        let mut input_port_lookup = BTreeMap::new();
        let mut output_port_lookup = BTreeMap::new();

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

        let operation_lookup = validated
            .topological_order()
            .iter()
            .copied()
            .enumerate()
            .map(|(index, vertex)| (vertex, OperationIndex(index)))
            .collect::<BTreeMap<_, _>>();
        let operations = validated
            .topological_order()
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

        Self {
            definition,
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
            let expected_kind = if matches!(node.kind, CompiledNodeKind::Merge) {
                SignalKind::Pulse
            } else {
                SignalKind::Level
            };
            for index in node.inputs.iter().chain(node.outputs.iter()) {
                let Some(port) = self.ports.get(index.0) else {
                    return Err("compiled port reference is out of bounds");
                };
                if port.owner != self.node_lookup[&node.key] || port.kind != expected_kind {
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
    }
}

fn connection_source(endpoint: ConnectionEndpoint) -> ReactionVertex {
    match endpoint {
        ConnectionEndpoint::ExternalInput(key) => ReactionVertex::ExternalInput(key),
        ConnectionEndpoint::NodeOutput(key) => ReactionVertex::NodeOutput(key),
        ConnectionEndpoint::NodeInput(_) | ConnectionEndpoint::ExternalOutput(_) => {
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
    }
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
        let mut inner = CompiledInner::build(
            &network(false)
                .validate()
                .require_artifact()
                .unwrap_or_else(|_| panic!("fixture must validate")),
            &network(false)
                .validate_structural()
                .artifact()
                .unwrap_or_else(|| panic!("fixture must validate structurally"))
                .reaction_dependencies(),
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
        let mut inner = CompiledInner::build(&validated, &graph);
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
        let mut inner = CompiledInner::build(&validated, &graph);
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
