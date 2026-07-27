//! Immutable dense execution topology for validated restricted networks.

use crate::authored::{ConnectionEndpoint, NodeKind, UncheckedNetwork};
use crate::diagnostics::{DiagnosticSet, Report};
use crate::identity::{InputSchemaFingerprint, NetworkFingerprint, TimeDomainId};
use crate::key::{
    AnyExternalInputKey, AnyExternalOutputKey, AnyInPortKey, AnyOutPortKey, ConnectionKey,
    NetworkKey, NodeKey,
};
use crate::signal::{LogicLevel, SignalKind};
use crate::validation::{ReactionDependencyGraph, ReactionVertex, ValidatedNetwork};
use std::collections::BTreeMap;
use std::sync::Arc;

/// An immutable executable topology compiled from a validated network.
///
/// The public value deliberately exposes only stable identity. Dense execution
/// positions and adjacency remain private to this compiled revision.
#[derive(Clone, Debug)]
pub struct CompiledNetwork<D> {
    inner: Arc<CompiledInner<D>>,
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
}

impl<D> CompiledInner<D> {
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
            let expected = match node.kind {
                CompiledNodeKind::Constant(_) => (0, 1),
                CompiledNodeKind::Not => (1, 1),
            };
            if (node.inputs.len(), node.outputs.len()) != expected {
                return Err("compiled descriptor disagrees with node kind");
            }
            for index in node.inputs.iter().chain(node.outputs.iter()) {
                let Some(port) = self.ports.get(index.0) else {
                    return Err("compiled port reference is out of bounds");
                };
                if port.owner != self.node_lookup[&node.key] || port.kind != SignalKind::Level {
                    return Err("compiled port disagrees with validated node");
                }
            }
        }
        for (key, index) in &self.input_port_lookup {
            if self.ports.get(index.0).map(|port| port.direction) != Some(PortDirection::Input)
                || key.kind() != SignalKind::Level
            {
                return Err("compiled input-port lookup is invalid");
            }
        }
        for (key, index) in &self.external_input_lookup {
            if self.external_inputs.get(index.0).map(|input| input.key) != Some(*key)
                || key.kind() != SignalKind::Level
            {
                return Err("compiled external-input lookup is invalid");
            }
        }
        for (key, index) in &self.output_port_lookup {
            if self.ports.get(index.0).map(|port| port.direction) != Some(PortDirection::Output)
                || key.kind() != SignalKind::Level
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
            if output.key.kind() != SignalKind::Level || output.source.0 >= self.operations.len() {
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
        crate::key::AnySignalSourceKey::Pulse(_) => {
            panic!("restricted validated compilation cannot contain pulse sources")
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
            };
            crate::authored::NodeDef::new(
                node.key(),
                kind,
                crate::authored::NodePorts::new(
                    node.ports().inputs().to_vec(),
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
        network(reverse_claim_order)
            .validate()
            .require_artifact()
            .unwrap_or_else(|_| panic!("fixture must validate"))
            .compile()
            .require_artifact()
            .unwrap_or_else(|_| panic!("fixture must compile"))
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
