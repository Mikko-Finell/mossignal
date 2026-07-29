//! Private structural validation for the opening authored graph.

#![allow(dead_code)] // Consumed by the following private validation phase.

use crate::authored::{
    ConnectionEndpoint, InputPortRole, ModuleInstanceDef, ModuleInterfaceMapping, NodeDef,
    NodeKind, UncheckedModule, UncheckedNetwork,
};
use crate::compile::CompiledNetwork;
use crate::diagnostics::{
    CurrentReactionCycleStep, Diagnostic, DiagnosticSet, DuplicateClaim, DuplicateNodeKind,
    FixedArityRole, Problem, ProblemEvidence, ReactionMemberRef, ReactionRole, Report, SubjectRef,
};
use crate::identity::{InputSchemaFingerprint, NetworkFingerprint, TimeDomainId, fingerprints};
use crate::key::{
    AnyExternalInputKey, AnyExternalOutputKey, AnyInPortKey, AnyModuleInputKey, AnyModuleOutputKey,
    AnyOutPortKey, AnySignalSourceKey, ConnectionKey, ModuleInstanceKey, NodeKey, SignalSourceKey,
};
use crate::signal::{LogicLevel, SignalKind};
use std::collections::{BTreeMap, BTreeSet};

/// Structurally usable input for the later dependency-validation phase.
///
/// This is deliberately crate-private: structural validation alone does not
/// establish the public `ValidatedNetwork` lifecycle state.
pub(crate) struct StructuralCandidate<'a, D> {
    network: &'a UncheckedNetwork<D>,
}

impl<'a, D> StructuralCandidate<'a, D> {
    pub(crate) const fn network(&self) -> &'a UncheckedNetwork<D> {
        self.network
    }

    /// Derives the private current-reaction dependency relation.
    pub(crate) fn reaction_dependencies(&self) -> ReactionDependencyGraph {
        ReactionDependencyGraph::from_candidate(self)
    }
}

/// One current-reaction fact or deterministic operation in the restricted graph.
///
/// This remains separate from authored connectivity: input ports are connection
/// attachment points, while node operations consume their settled sources.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum ReactionVertex {
    ExternalInput(AnyExternalInputKey),
    ModuleInput(AnyModuleInputKey),
    NodeOperation(NodeKey),
    NodeOutput(AnyOutPortKey),
    ModuleOutput(AnyModuleOutputKey),
    InstanceInput(ModuleInstanceKey, AnyModuleInputKey),
    InstanceOutput(ModuleInstanceKey, AnyModuleOutputKey),
    ExternalOutput(AnyExternalOutputKey),
}

/// The stable authored relation contributing a reaction dependency.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum ReactionDependencySubject {
    Connection(ConnectionKey),
    ModuleInput(AnyModuleInputKey),
    Node(NodeKey),
    ModuleOutput(AnyModuleOutputKey),
    InstanceBinding(ModuleInstanceKey, AnyModuleInputKey),
    InstanceProjection(ModuleInstanceKey),
    ExternalOutput(AnyExternalOutputKey),
}

/// One directed dependency in the restricted current-reaction graph.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct ReactionDependency {
    pub(crate) from: ReactionVertex,
    pub(crate) to: ReactionVertex,
    pub(crate) subject: ReactionDependencySubject,
}

/// Deterministic private inspection of the restricted dependency relation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ReactionDependencyGraph {
    vertices: BTreeSet<ReactionVertex>,
    dependencies: BTreeSet<ReactionDependency>,
}

/// An opaque network definition that has passed restricted structural and
/// current-reaction causality validation.
#[derive(Debug)]
pub struct ValidatedNetwork<D> {
    definition: UncheckedNetwork<D>,
    topological_order: Vec<ReactionVertex>,
    fingerprint: NetworkFingerprint,
    input_schema_fingerprint: InputSchemaFingerprint,
}

impl<D> ValidatedNetwork<D> {
    /// Returns the stable identity of the validated authored network.
    #[must_use]
    pub const fn network_key(&self) -> crate::key::NetworkKey {
        self.definition.key()
    }

    /// Returns the caller-owned logical tick identity bound before validation.
    #[must_use]
    pub const fn time_domain_id(&self) -> TimeDomainId {
        self.definition.time_domain_id()
    }

    /// Returns the immutable semantic fingerprint computed by successful validation.
    #[must_use]
    pub const fn fingerprint(&self) -> NetworkFingerprint {
        self.fingerprint
    }

    /// Returns the immutable complete external input-schema fingerprint.
    #[must_use]
    pub const fn input_schema_fingerprint(&self) -> InputSchemaFingerprint {
        self.input_schema_fingerprint
    }

    /// Borrows the canonical validated authored graph, including retained hierarchy.
    #[must_use]
    pub const fn graph(&self) -> NetworkDefinitionGraphView<'_, D> {
        NetworkDefinitionGraphView {
            definition: &self.definition,
        }
    }

    /// Compiles this validated definition into immutable execution topology.
    #[must_use]
    pub fn compile(self) -> Report<CompiledNetwork<D>, D> {
        if !self.definition.module_instances().is_empty() {
            return unsupported_module_compilation(&self.definition);
        }
        CompiledNetwork::from_validated(&self)
    }

    /// Compiles a shared validated definition without consuming it.
    #[must_use]
    pub fn compile_ref(&self) -> Report<CompiledNetwork<D>, D> {
        if !self.definition.module_instances().is_empty() {
            return unsupported_module_compilation(&self.definition);
        }
        CompiledNetwork::from_validated(self)
    }

    pub(crate) const fn definition(&self) -> &UncheckedNetwork<D> {
        &self.definition
    }

    pub(crate) fn topological_order(&self) -> &[ReactionVertex] {
        &self.topological_order
    }

    pub(crate) fn reaction_dependencies(&self) -> ReactionDependencyGraph {
        StructuralCandidate {
            network: &self.definition,
        }
        .reaction_dependencies()
    }
}

/// Immutable read-only view of a validated network definition and retained hierarchy.
#[derive(Clone, Copy)]
pub struct NetworkDefinitionGraphView<'a, D> {
    definition: &'a UncheckedNetwork<D>,
}

impl<'a, D> NetworkDefinitionGraphView<'a, D> {
    #[must_use]
    pub fn nodes(self) -> &'a [NodeDef<D>] {
        self.definition.nodes()
    }
    #[must_use]
    pub fn connections(self) -> &'a [crate::authored::ConnectionDef] {
        self.definition.connections()
    }
    #[must_use]
    pub fn module_instances(self) -> &'a [ModuleInstanceDef<D>] {
        self.definition.module_instances()
    }
    #[must_use]
    pub fn qualified_nodes(self) -> Vec<crate::module::QualifiedNodeRef> {
        crate::module::qualified_nodes(self.definition.module_instances())
    }
    #[must_use]
    pub fn qualified_connections(self) -> Vec<crate::module::QualifiedConnectionRef> {
        crate::module::qualified_connections(self.definition.module_instances())
    }
}

fn unsupported_module_compilation<D>(
    definition: &UncheckedNetwork<D>,
) -> Report<CompiledNetwork<D>, D> {
    let mut instances = Vec::new();
    collect_instance_keys(definition.module_instances(), &mut instances);
    let problem = Problem::new(
        SubjectRef::Network(definition.key()),
        Vec::new(),
        ProblemEvidence::unsupported_module_instances(instances),
    );
    let diagnostics = match Diagnostic::new(problem) {
        Ok(diagnostic) => DiagnosticSet::from_diagnostic(diagnostic),
        Err(_) => panic!(
            "compilation.unsupported_module_instances must remain a report-deliverable catalogue condition"
        ),
    };
    Report::new(None, diagnostics)
}

fn collect_instance_keys<D>(
    definitions: &[ModuleInstanceDef<D>],
    instances: &mut Vec<ModuleInstanceKey>,
) {
    // SPEC: docs/specs/contracts/module-instantiation-hierarchy.yaml
    // "temporary-module-compilation-boundary" — nested instances are affected too.
    for instance in definitions {
        instances.push(instance.key());
        collect_instance_keys(instance.module().graph().module_instances(), instances);
    }
}

impl ReactionDependencyGraph {
    fn from_candidate<D>(candidate: &StructuralCandidate<'_, D>) -> Self {
        let network = candidate.network();
        let mut vertices = BTreeSet::new();
        let mut dependencies = BTreeSet::new();
        let mut input_owners = BTreeMap::new();

        for input in network.external_inputs() {
            vertices.insert(ReactionVertex::ExternalInput(input.key()));
        }

        for node in network.nodes() {
            let operation = ReactionVertex::NodeOperation(node.key());
            vertices.insert(operation);
            for input in node.ports().inputs() {
                input_owners.insert(*input, node.key());
            }
            for output in node.ports().outputs() {
                let output = ReactionVertex::NodeOutput(*output);
                vertices.insert(output);
                dependencies.insert(ReactionDependency {
                    from: operation,
                    to: output,
                    subject: ReactionDependencySubject::Node(node.key()),
                });
            }
        }

        for connection in network.connections() {
            let ConnectionEndpoint::NodeInput(input) = connection.to() else {
                continue;
            };
            let Some(&owner) = input_owners.get(&input) else {
                continue;
            };
            // SPEC: docs/specs/contracts/pulse-delay.yaml "temporal-causality-barrier"
            // Current PulseDelay input schedules later work and never feeds its current output.
            if network
                .nodes()
                .iter()
                .any(|node| node.key() == owner && matches!(node.kind(), NodeKind::PulseDelay(_)))
            {
                continue;
            }
            let Some(source) = reaction_source(connection.from()) else {
                continue;
            };
            vertices.insert(source);
            dependencies.insert(ReactionDependency {
                from: source,
                to: ReactionVertex::NodeOperation(owner),
                subject: ReactionDependencySubject::Connection(connection.key()),
            });
        }

        add_instance_dependencies(network.module_instances(), &mut vertices, &mut dependencies);

        for output in network.external_outputs() {
            let source = reaction_source_from_signal(output.source());
            vertices.insert(source);
            let observed = ReactionVertex::ExternalOutput(output.key());
            vertices.insert(observed);
            dependencies.insert(ReactionDependency {
                from: source,
                to: observed,
                subject: ReactionDependencySubject::ExternalOutput(output.key()),
            });
        }

        Self {
            vertices,
            dependencies,
        }
    }

    pub(crate) fn from_module<D>(module: &UncheckedModule<D>) -> Self {
        let mut vertices = BTreeSet::new();
        let mut dependencies = BTreeSet::new();
        let mut input_owners = BTreeMap::new();

        for input in module.inputs() {
            vertices.insert(ReactionVertex::ModuleInput(input.key()));
        }
        for node in module.nodes() {
            let operation = ReactionVertex::NodeOperation(node.key());
            vertices.insert(operation);
            for input in node.ports().inputs() {
                input_owners.insert(*input, node.key());
            }
            for output in node.ports().outputs() {
                let output = ReactionVertex::NodeOutput(*output);
                vertices.insert(output);
                dependencies.insert(ReactionDependency {
                    from: operation,
                    to: output,
                    subject: ReactionDependencySubject::Node(node.key()),
                });
            }
        }
        for connection in module.connections() {
            let ConnectionEndpoint::NodeInput(input) = connection.to() else {
                continue;
            };
            let Some(&owner) = input_owners.get(&input) else {
                continue;
            };
            if module
                .nodes()
                .iter()
                .any(|node| node.key() == owner && matches!(node.kind(), NodeKind::PulseDelay(_)))
            {
                continue;
            }
            let Some(source) = reaction_source(connection.from()) else {
                continue;
            };
            dependencies.insert(ReactionDependency {
                from: source,
                to: ReactionVertex::NodeOperation(owner),
                subject: ReactionDependencySubject::Connection(connection.key()),
            });
        }
        for mapping in module.mappings() {
            match *mapping {
                ModuleInterfaceMapping::Input { input, target } => {
                    let ConnectionEndpoint::NodeInput(target) = target else {
                        continue;
                    };
                    let Some(&owner) = input_owners.get(&target) else {
                        continue;
                    };
                    if module.nodes().iter().any(|node| {
                        node.key() == owner && matches!(node.kind(), NodeKind::PulseDelay(_))
                    }) {
                        continue;
                    }
                    dependencies.insert(ReactionDependency {
                        from: ReactionVertex::ModuleInput(input),
                        to: ReactionVertex::NodeOperation(owner),
                        subject: ReactionDependencySubject::ModuleInput(input),
                    });
                }
                ModuleInterfaceMapping::Output { output, source } => {
                    let Some(source) = reaction_source(source) else {
                        continue;
                    };
                    let observed = ReactionVertex::ModuleOutput(output);
                    vertices.insert(observed);
                    dependencies.insert(ReactionDependency {
                        from: source,
                        to: observed,
                        subject: ReactionDependencySubject::ModuleOutput(output),
                    });
                }
            }
        }
        add_instance_dependencies(module.module_instances(), &mut vertices, &mut dependencies);
        for edge in &dependencies {
            vertices.insert(edge.from);
            vertices.insert(edge.to);
        }
        Self {
            vertices,
            dependencies,
        }
    }

    pub(crate) fn vertices(&self) -> impl Iterator<Item = ReactionVertex> + '_ {
        self.vertices.iter().copied()
    }

    pub(crate) fn dependencies(&self) -> impl Iterator<Item = ReactionDependency> + '_ {
        self.dependencies.iter().copied()
    }

    pub(crate) fn cyclic_components(&self) -> Vec<BTreeSet<ReactionVertex>> {
        let mut forward: BTreeMap<ReactionVertex, Vec<ReactionVertex>> = BTreeMap::new();
        let mut reverse: BTreeMap<ReactionVertex, Vec<ReactionVertex>> = BTreeMap::new();
        for vertex in &self.vertices {
            forward.entry(*vertex).or_default();
            reverse.entry(*vertex).or_default();
        }
        for dependency in &self.dependencies {
            forward
                .entry(dependency.from)
                .or_default()
                .push(dependency.to);
            reverse
                .entry(dependency.to)
                .or_default()
                .push(dependency.from);
        }
        for edges in forward.values_mut().chain(reverse.values_mut()) {
            edges.sort();
            edges.dedup();
        }
        let mut visited = BTreeSet::new();
        let mut finish = Vec::new();
        for vertex in &self.vertices {
            dfs_finish(*vertex, &forward, &mut visited, &mut finish);
        }
        visited.clear();
        let mut components = Vec::new();
        for vertex in finish.into_iter().rev() {
            if visited.contains(&vertex) {
                continue;
            }
            let mut component = BTreeSet::new();
            dfs_collect(vertex, &reverse, &mut visited, &mut component);
            let cyclic = component.len() > 1
                || component.iter().any(|member| {
                    self.dependencies
                        .iter()
                        .any(|edge| edge.from == *member && edge.to == *member)
                });
            if cyclic {
                components.push(component);
            }
        }
        components.sort_by(|left, right| left.iter().cmp(right.iter()));
        components
    }

    pub(crate) fn cycle_witness(
        &self,
        component: &BTreeSet<ReactionVertex>,
    ) -> Vec<ReactionDependency> {
        let mut edges: Vec<_> = self
            .dependencies
            .iter()
            .copied()
            .filter(|edge| component.contains(&edge.from) && component.contains(&edge.to))
            .collect();
        edges.sort_by(compare_dependency_steps);
        if component.len() == 1 {
            return edges
                .into_iter()
                .filter(|edge| edge.from == edge.to)
                .take(1)
                .collect();
        }
        let first = edges
            .iter()
            .copied()
            .find(|edge| edge.from != edge.to)
            .unwrap_or_else(|| panic!("cyclic multi-member SCC must contain a non-self edge"));
        let mut witness = vec![first];
        witness.extend(shortest_path(first.to, first.from, &edges));
        witness
    }

    pub(crate) fn topological_order(&self) -> Vec<ReactionVertex> {
        let mut indegree: BTreeMap<_, usize> =
            self.vertices.iter().copied().map(|v| (v, 0)).collect();
        let mut outgoing: BTreeMap<_, BTreeSet<_>> = BTreeMap::new();
        for edge in &self.dependencies {
            // SPEC: docs/specs/built_in_node_semantics.md §16 "Duplicate sources"
            // Stable incidences remain distinct, but topological indegree counts
            // the dependency between a given pair of operations only once.
            if outgoing.entry(edge.from).or_default().insert(edge.to) {
                *indegree.entry(edge.to).or_default() += 1;
            }
        }
        let mut ready: BTreeSet<_> = indegree
            .iter()
            .filter_map(|(vertex, degree)| (*degree == 0).then_some(*vertex))
            .collect();
        let mut order = Vec::new();
        while let Some(vertex) = ready.pop_first() {
            order.push(vertex);
            if let Some(targets) = outgoing.get(&vertex) {
                for target in targets {
                    let degree = indegree.get_mut(target).unwrap_or_else(|| {
                        panic!("reaction graph targets must have an indegree entry")
                    });
                    *degree -= 1;
                    if *degree == 0 {
                        ready.insert(*target);
                    }
                }
            }
        }
        order
    }
}

fn add_instance_dependencies<D>(
    instances: &[ModuleInstanceDef<D>],
    vertices: &mut BTreeSet<ReactionVertex>,
    dependencies: &mut BTreeSet<ReactionDependency>,
) {
    for instance in instances {
        for input in instance.module().inputs() {
            vertices.insert(ReactionVertex::InstanceInput(instance.key(), input.key()));
        }
        for output in instance.module().outputs() {
            vertices.insert(ReactionVertex::InstanceOutput(instance.key(), output.key()));
        }
        for binding in instance.bindings().bindings() {
            let Some(source) = reaction_source(binding.source()) else {
                continue;
            };
            dependencies.insert(ReactionDependency {
                from: source,
                to: ReactionVertex::InstanceInput(instance.key(), binding.input()),
                subject: ReactionDependencySubject::InstanceBinding(
                    instance.key(),
                    binding.input(),
                ),
            });
        }
        for (input, output) in public_module_dependencies(instance.module()) {
            dependencies.insert(ReactionDependency {
                from: ReactionVertex::InstanceInput(instance.key(), input),
                to: ReactionVertex::InstanceOutput(instance.key(), output),
                subject: ReactionDependencySubject::InstanceProjection(instance.key()),
            });
        }
    }
    for edge in dependencies.iter() {
        vertices.insert(edge.from);
        vertices.insert(edge.to);
    }
}

fn public_module_dependencies<D>(
    module: &crate::module::ModuleDef<D>,
) -> BTreeSet<(AnyModuleInputKey, AnyModuleOutputKey)> {
    let graph = ReactionDependencyGraph::from_module(module.definition());
    let mut outgoing: BTreeMap<ReactionVertex, Vec<ReactionVertex>> = BTreeMap::new();
    for edge in graph.dependencies() {
        outgoing.entry(edge.from).or_default().push(edge.to);
    }
    let mut result = BTreeSet::new();
    for input in module.inputs() {
        let start = ReactionVertex::ModuleInput(input.key());
        let mut pending = vec![start];
        let mut visited = BTreeSet::new();
        while let Some(vertex) = pending.pop() {
            if !visited.insert(vertex) {
                continue;
            }
            if let ReactionVertex::ModuleOutput(output) = vertex {
                result.insert((input.key(), output));
            }
            if let Some(next) = outgoing.get(&vertex) {
                pending.extend(next.iter().copied());
            }
        }
    }
    result
}

fn dfs_finish(
    vertex: ReactionVertex,
    graph: &BTreeMap<ReactionVertex, Vec<ReactionVertex>>,
    visited: &mut BTreeSet<ReactionVertex>,
    finish: &mut Vec<ReactionVertex>,
) {
    if !visited.insert(vertex) {
        return;
    }
    if let Some(next) = graph.get(&vertex) {
        for target in next {
            dfs_finish(*target, graph, visited, finish);
        }
    }
    finish.push(vertex);
}

fn dfs_collect(
    vertex: ReactionVertex,
    graph: &BTreeMap<ReactionVertex, Vec<ReactionVertex>>,
    visited: &mut BTreeSet<ReactionVertex>,
    component: &mut BTreeSet<ReactionVertex>,
) {
    if !visited.insert(vertex) {
        return;
    }
    component.insert(vertex);
    if let Some(next) = graph.get(&vertex) {
        for target in next {
            dfs_collect(*target, graph, visited, component);
        }
    }
}

fn shortest_path(
    start: ReactionVertex,
    target: ReactionVertex,
    edges: &[ReactionDependency],
) -> Vec<ReactionDependency> {
    let mut frontier = vec![(start, Vec::new())];
    while !frontier.is_empty() {
        frontier.sort_by(|left, right| compare_dependency_paths(&left.1, &right.1));
        let mut next = Vec::new();
        for (vertex, path) in frontier {
            for edge in edges.iter().copied().filter(|edge| edge.from == vertex) {
                let mut extended = path.clone();
                extended.push(edge);
                if edge.to == target {
                    return extended;
                }
                if !path
                    .iter()
                    .any(|prior| prior.from == edge.to || prior.to == edge.to)
                {
                    next.push((edge.to, extended));
                }
            }
        }
        frontier = next;
    }
    panic!("a cyclic SCC witness start must have a return path")
}

fn compare_dependency_steps(
    left: &ReactionDependency,
    right: &ReactionDependency,
) -> std::cmp::Ordering {
    cycle_step(*left).cmp(&cycle_step(*right))
}

fn compare_dependency_paths(
    left: &[ReactionDependency],
    right: &[ReactionDependency],
) -> std::cmp::Ordering {
    left.iter()
        .map(|edge| cycle_step(*edge))
        .cmp(right.iter().map(|edge| cycle_step(*edge)))
}

pub(crate) fn reaction_member(vertex: ReactionVertex) -> ReactionMemberRef {
    match vertex {
        ReactionVertex::ExternalInput(key) => ReactionMemberRef {
            subject: SubjectRef::ExternalInput(key),
            role: ReactionRole::ExternalInput,
        },
        ReactionVertex::ModuleInput(key) => ReactionMemberRef {
            subject: SubjectRef::ModuleInput(key),
            role: ReactionRole::ModuleInput,
        },
        ReactionVertex::NodeOperation(key) => ReactionMemberRef {
            subject: SubjectRef::Node(key),
            role: ReactionRole::NodeOperation,
        },
        ReactionVertex::NodeOutput(key) => ReactionMemberRef {
            subject: SubjectRef::OutPort(key),
            role: ReactionRole::NodeOutput,
        },
        ReactionVertex::ModuleOutput(key) => ReactionMemberRef {
            subject: SubjectRef::ModuleOutput(key),
            role: ReactionRole::ModuleOutput,
        },
        ReactionVertex::InstanceInput(instance, key) => ReactionMemberRef {
            subject: SubjectRef::ModuleInstanceInput(instance, key),
            role: ReactionRole::ModuleInput,
        },
        ReactionVertex::InstanceOutput(instance, key) => ReactionMemberRef {
            subject: SubjectRef::ModuleInstanceOutput(instance, key),
            role: ReactionRole::ModuleOutput,
        },
        ReactionVertex::ExternalOutput(key) => ReactionMemberRef {
            subject: SubjectRef::ExternalOutput(key),
            role: ReactionRole::ExternalOutput,
        },
    }
}

pub(crate) fn cycle_step(edge: ReactionDependency) -> CurrentReactionCycleStep {
    CurrentReactionCycleStep {
        source: reaction_member(edge.from),
        dependency: match edge.subject {
            ReactionDependencySubject::Connection(key) => SubjectRef::Connection(key),
            ReactionDependencySubject::ModuleInput(key) => SubjectRef::ModuleInput(key),
            ReactionDependencySubject::Node(key) => SubjectRef::Node(key),
            ReactionDependencySubject::ModuleOutput(key) => SubjectRef::ModuleOutput(key),
            ReactionDependencySubject::InstanceBinding(instance, key) => {
                SubjectRef::ModuleInstanceInput(instance, key)
            }
            ReactionDependencySubject::InstanceProjection(instance) => {
                SubjectRef::ModuleInstance(instance)
            }
            ReactionDependencySubject::ExternalOutput(key) => SubjectRef::ExternalOutput(key),
        },
        target: reaction_member(edge.to),
    }
}

impl<D> UncheckedNetwork<D> {
    /// Validates the restricted authored graph and returns an opaque artifact
    /// only when its current-reaction graph is acyclic.
    pub fn validate(self) -> Report<ValidatedNetwork<D>, D>
    where
        D: PartialEq,
    {
        let structural = self.validate_structural();
        let (candidate, mut diagnostics) = structural.into_parts();
        let graph = match candidate {
            Some(candidate) => candidate.reaction_dependencies(),
            None => return Report::new(None, diagnostics),
        };
        let cyclic_components = graph.cyclic_components();
        for component in &cyclic_components {
            let members = component.iter().copied().map(reaction_member).collect();
            let witness = graph
                .cycle_witness(component)
                .into_iter()
                .map(cycle_step)
                .collect();
            let problem = Problem::new(
                SubjectRef::Network(self.key()),
                Vec::new(),
                ProblemEvidence::current_reaction_cycle(members, witness),
            );
            let diagnostic = Diagnostic::new(problem)
                .unwrap_or_else(|_| panic!("current-reaction cycle must be a report finding"));
            diagnostics.insert(diagnostic);
        }
        if !cyclic_components.is_empty() {
            return Report::new(None, diagnostics);
        }
        let order = graph.topological_order();
        let definition = normalized_network(&self);
        let (fingerprint, input_schema_fingerprint) = fingerprints(&definition);
        Report::new(
            Some(ValidatedNetwork {
                definition,
                topological_order: order,
                fingerprint,
                input_schema_fingerprint,
            }),
            diagnostics,
        )
    }

    /// Performs only the opening, pre-semantic structural checks.
    pub(crate) fn validate_structural(&self) -> Report<StructuralCandidate<'_, D>, D>
    where
        D: PartialEq,
    {
        StructuralValidator::new(self).run()
    }

    pub(crate) fn validate_structural_for_module(
        &self,
        boundary_drivers: &BTreeSet<AnyInPortKey>,
        module_inputs: &BTreeSet<AnyModuleInputKey>,
    ) -> Report<StructuralCandidate<'_, D>, D>
    where
        D: PartialEq,
    {
        StructuralValidator::for_module(self, boundary_drivers, module_inputs).run()
    }
}

fn normalized_network<D>(network: &UncheckedNetwork<D>) -> UncheckedNetwork<D> {
    let mut nodes = network.nodes().to_vec();
    let mut external_inputs = network.external_inputs().to_vec();
    let mut external_outputs = network.external_outputs().to_vec();
    let mut connections = network.connections().to_vec();
    let mut module_instances = network.module_instances().to_vec();
    nodes.sort_by_key(NodeDef::key);
    external_inputs.sort_by_key(|input| input.key());
    external_outputs.sort_by_key(|output| output.key());
    connections.sort_by_key(|connection| connection.key());
    module_instances.sort_by_key(ModuleInstanceDef::key);
    module_instances = module_instances
        .into_iter()
        .map(|instance| {
            let mut bindings = instance.bindings().bindings().to_vec();
            bindings.sort_by_key(|binding| (binding.input(), endpoint_subject(binding.source())));
            ModuleInstanceDef::new(
                instance.key(),
                instance.module().clone(),
                crate::authored::ModuleBindingSet::new(bindings),
                instance.parent(),
                instance.meta().clone(),
            )
        })
        .collect();
    UncheckedNetwork::new_with_instances(
        network.key(),
        network.time_domain_id(),
        network.meta().clone(),
        nodes,
        external_inputs,
        external_outputs,
        connections,
        module_instances,
    )
}

struct StructuralValidator<'a, D> {
    network: &'a UncheckedNetwork<D>,
    diagnostics: DiagnosticSet<D>,
    nodes: BTreeSet<crate::key::NodeKey>,
    inputs: BTreeSet<AnyInPortKey>,
    outputs: BTreeSet<AnyOutPortKey>,
    external_inputs: BTreeSet<AnyExternalInputKey>,
    external_outputs: BTreeSet<AnyExternalOutputKey>,
    module_inputs: BTreeSet<AnyModuleInputKey>,
    module_instances: BTreeMap<ModuleInstanceKey, &'a ModuleInstanceDef<D>>,
    boundary_drivers: BTreeSet<AnyInPortKey>,
    use_network_duplicate_subject: bool,
}

impl<'a, D: PartialEq> StructuralValidator<'a, D> {
    fn new(network: &'a UncheckedNetwork<D>) -> Self {
        Self {
            network,
            diagnostics: DiagnosticSet::new(),
            nodes: BTreeSet::new(),
            inputs: BTreeSet::new(),
            outputs: BTreeSet::new(),
            external_inputs: BTreeSet::new(),
            external_outputs: BTreeSet::new(),
            module_inputs: BTreeSet::new(),
            module_instances: BTreeMap::new(),
            boundary_drivers: BTreeSet::new(),
            use_network_duplicate_subject: true,
        }
    }

    fn for_module(
        network: &'a UncheckedNetwork<D>,
        boundary_drivers: &BTreeSet<AnyInPortKey>,
        module_inputs: &BTreeSet<AnyModuleInputKey>,
    ) -> Self {
        let mut validator = Self::new(network);
        validator.boundary_drivers = boundary_drivers.clone();
        validator.module_inputs = module_inputs.clone();
        validator.use_network_duplicate_subject = false;
        validator
    }

    fn run(mut self) -> Report<StructuralCandidate<'a, D>, D> {
        self.collect_keys_and_duplicates();
        self.validate_node_shapes();
        self.validate_module_instances();
        self.validate_connections();
        self.validate_external_outputs();
        Report::new(
            Some(StructuralCandidate {
                network: self.network,
            }),
            self.diagnostics,
        )
    }

    fn add(&mut self, primary: SubjectRef, evidence: ProblemEvidence<D>) {
        if let Ok(diagnostic) = Diagnostic::new(Problem::new(primary, Vec::new(), evidence)) {
            self.diagnostics.insert(diagnostic);
        }
    }

    fn collect_keys_and_duplicates(&mut self) {
        let mut nodes = BTreeMap::new();
        let mut inputs = BTreeMap::new();
        let mut outputs = BTreeMap::new();
        let mut connections = BTreeMap::new();
        let mut external_inputs = BTreeMap::new();
        let mut external_outputs = BTreeMap::new();
        let mut module_instances = BTreeMap::new();
        for node in self.network.nodes() {
            nodes
                .entry(node.key())
                .or_insert_with(Vec::new)
                .push(DuplicateClaim::Node {
                    key: node.key(),
                    kind: match node.kind() {
                        NodeKind::Constant(config) => DuplicateNodeKind::Constant(config.value()),
                        NodeKind::Not => DuplicateNodeKind::Not,
                        NodeKind::All => DuplicateNodeKind::All,
                        NodeKind::Any => DuplicateNodeKind::Any,
                        NodeKind::Parity => DuplicateNodeKind::Parity,
                        NodeKind::AtLeast(config) => DuplicateNodeKind::AtLeast(config.threshold),
                        NodeKind::Select => DuplicateNodeKind::Select,
                        NodeKind::Merge => DuplicateNodeKind::Merge,
                        NodeKind::Toggle(config) => DuplicateNodeKind::Toggle(config.initial),
                        NodeKind::PulseDelay(config) => {
                            DuplicateNodeKind::PulseDelay(config.delay.ticks())
                        }
                    },
                    inputs: node.ports().inputs().to_vec(),
                    input_roles: node.ports().input_roles().to_vec(),
                    outputs: node.ports().outputs().to_vec(),
                    origin: node.meta().origin.clone(),
                });
            self.nodes.insert(node.key());
            for key in node.ports().inputs() {
                inputs
                    .entry(*key)
                    .or_insert_with(Vec::new)
                    .push(DuplicateClaim::InPort {
                        key: *key,
                        owner: node.key(),
                        origin: node.meta().origin.clone(),
                    });
                self.inputs.insert(*key);
            }
            for key in node.ports().outputs() {
                outputs
                    .entry(*key)
                    .or_insert_with(Vec::new)
                    .push(DuplicateClaim::OutPort {
                        key: *key,
                        owner: node.key(),
                        origin: node.meta().origin.clone(),
                    });
                self.outputs.insert(*key);
            }
        }
        for connection in self.network.connections() {
            connections
                .entry(connection.key())
                .or_insert_with(Vec::new)
                .push(DuplicateClaim::Connection {
                    key: connection.key(),
                    source: endpoint_subject(connection.from()),
                    target: endpoint_subject(connection.to()),
                    origin: connection.meta().origin.clone(),
                });
        }
        for endpoint in self.network.external_inputs() {
            external_inputs
                .entry(endpoint.key())
                .or_insert_with(Vec::new)
                .push(DuplicateClaim::ExternalInput {
                    key: endpoint.key(),
                    origin: endpoint.meta().origin.clone(),
                });
            self.external_inputs.insert(endpoint.key());
        }
        for endpoint in self.network.external_outputs() {
            external_outputs
                .entry(endpoint.key())
                .or_insert_with(Vec::new)
                .push(DuplicateClaim::ExternalOutput {
                    key: endpoint.key(),
                    source: signal_source_subject(endpoint.source()),
                    origin: endpoint.meta().origin.clone(),
                });
            self.external_outputs.insert(endpoint.key());
        }
        for instance in self.network.module_instances() {
            module_instances
                .entry(instance.key())
                .or_insert_with(Vec::new)
                .push(DuplicateClaim::ModuleInstance {
                    key: instance.key(),
                    module: instance.module().fingerprint(),
                    parent: instance.parent(),
                    bindings: instance
                        .bindings()
                        .bindings()
                        .iter()
                        .map(|binding| (binding.input(), endpoint_subject(binding.source())))
                        .collect(),
                    origin: instance.meta().origin.clone(),
                });
            self.module_instances
                .entry(instance.key())
                .or_insert(instance);
        }
        for claims in nodes.values().filter(|claims| claims.len() > 1) {
            self.duplicate(claims);
        }
        for claims in inputs.values().filter(|claims| claims.len() > 1) {
            self.duplicate(claims);
        }
        for claims in outputs.values().filter(|claims| claims.len() > 1) {
            self.duplicate(claims);
        }
        for claims in connections.values().filter(|claims| claims.len() > 1) {
            self.duplicate(claims);
        }
        for claims in external_inputs.values().filter(|claims| claims.len() > 1) {
            self.duplicate(claims);
        }
        for claims in external_outputs.values().filter(|claims| claims.len() > 1) {
            self.duplicate(claims);
        }
        for claims in module_instances.values().filter(|claims| claims.len() > 1) {
            self.duplicate(claims);
        }
    }

    fn duplicate(&mut self, claims: &[DuplicateClaim]) {
        // The kernel preserves multiplicity and canonicalizes claim subjects.
        self.add(
            if self.use_network_duplicate_subject {
                SubjectRef::Network(self.network.key())
            } else {
                duplicate_claim_subject(&claims[0])
            },
            ProblemEvidence::duplicate_key(duplicate_claim_subject(&claims[0]), claims.to_vec()),
        );
    }

    fn validate_node_shapes(&mut self) {
        for node in self.network.nodes() {
            let expected_inputs = match node.kind() {
                NodeKind::Constant(_) => Some(0),
                NodeKind::Not => Some(1),
                NodeKind::All | NodeKind::Any | NodeKind::Parity | NodeKind::AtLeast(_) => None,
                NodeKind::Select => Some(3),
                NodeKind::Merge => None,
                NodeKind::Toggle(_) | NodeKind::PulseDelay(_) => Some(1),
            };
            let (expected_input_kind, expected_output_kind) = match node.kind() {
                NodeKind::Merge => (SignalKind::Pulse, SignalKind::Pulse),
                NodeKind::Toggle(_) => (SignalKind::Pulse, SignalKind::Level),
                NodeKind::PulseDelay(_) => (SignalKind::Pulse, SignalKind::Pulse),
                _ => (SignalKind::Level, SignalKind::Level),
            };
            let expected_outputs = 1;
            let inputs = node.ports().inputs();
            let outputs = node.ports().outputs();
            if let Some(expected_inputs) = expected_inputs
                && inputs.len() != expected_inputs
            {
                self.add(
                    SubjectRef::Node(node.key()),
                    ProblemEvidence::invalid_fixed_arity(
                        FixedArityRole::Input,
                        inputs.iter().copied().map(SubjectRef::InPort).collect(),
                        expected_inputs,
                        inputs.len(),
                    ),
                );
            }
            if outputs.len() != expected_outputs {
                self.add(
                    SubjectRef::Node(node.key()),
                    ProblemEvidence::invalid_fixed_arity(
                        FixedArityRole::Output,
                        outputs.iter().copied().map(SubjectRef::OutPort).collect(),
                        expected_outputs,
                        outputs.len(),
                    ),
                );
            }
            if matches!(
                node.kind(),
                NodeKind::Not | NodeKind::Select | NodeKind::Toggle(_) | NodeKind::PulseDelay(_)
            ) && expected_inputs.is_some_and(|expected| inputs.len() < expected)
            {
                self.add(
                    SubjectRef::Node(node.key()),
                    ProblemEvidence::missing_required_input(
                        SubjectRef::Node(node.key()),
                        expected_input_kind,
                    ),
                );
            }
            if inputs.iter().any(|key| key.kind() != expected_input_kind) {
                self.add(
                    SubjectRef::Node(node.key()),
                    ProblemEvidence::invalid_fixed_arity(
                        FixedArityRole::Input,
                        inputs.iter().copied().map(SubjectRef::InPort).collect(),
                        expected_inputs.unwrap_or(inputs.len()),
                        inputs.len(),
                    ),
                );
            }
            if outputs.iter().any(|key| key.kind() != expected_output_kind) {
                self.add(
                    SubjectRef::Node(node.key()),
                    ProblemEvidence::invalid_fixed_arity(
                        FixedArityRole::Output,
                        outputs.iter().copied().map(SubjectRef::OutPort).collect(),
                        expected_outputs,
                        outputs.len(),
                    ),
                );
            }
            let role_count = node.ports().input_roles().len();
            let valid_role_count = match node.kind() {
                NodeKind::Select => {
                    let required = [
                        InputPortRole::Selector,
                        InputPortRole::WhenLow,
                        InputPortRole::WhenHigh,
                    ];
                    required
                        .iter()
                        .filter(|role| {
                            node.ports()
                                .input_roles()
                                .iter()
                                .filter(|candidate| *candidate == *role)
                                .count()
                                == 1
                        })
                        .count()
                }
                NodeKind::Toggle(_) => node
                    .ports()
                    .input_roles()
                    .iter()
                    .filter(|role| **role == InputPortRole::Toggle)
                    .count(),
                NodeKind::PulseDelay(_) => node
                    .ports()
                    .input_roles()
                    .iter()
                    .filter(|role| **role == InputPortRole::PulseDelay)
                    .count(),
                _ => node
                    .ports()
                    .input_roles()
                    .iter()
                    .filter(|role| **role == InputPortRole::Input)
                    .count(),
            };
            let expected_role_count = inputs.len();
            if role_count != expected_role_count || valid_role_count != expected_role_count {
                self.add(
                    SubjectRef::Node(node.key()),
                    ProblemEvidence::invalid_fixed_arity(
                        FixedArityRole::Input,
                        inputs.iter().copied().map(SubjectRef::InPort).collect(),
                        expected_role_count,
                        valid_role_count,
                    ),
                );
            }
            if matches!(
                node.kind(),
                NodeKind::All
                    | NodeKind::Any
                    | NodeKind::Parity
                    | NodeKind::AtLeast(_)
                    | NodeKind::Merge
            ) && outputs.len() == 1
                && outputs.iter().all(|key| key.kind() == expected_output_kind)
                && inputs.iter().all(|key| key.kind() == expected_input_kind)
                && role_count == inputs.len()
                && valid_role_count == inputs.len()
            {
                let ports = inputs.iter().copied().map(SubjectRef::InPort).collect();
                if inputs.is_empty() {
                    self.add(
                        SubjectRef::Node(node.key()),
                        ProblemEvidence::empty_variadic_node(ports),
                    );
                } else if inputs.len() == 1 {
                    self.add(
                        SubjectRef::Node(node.key()),
                        ProblemEvidence::unary_degenerate_node(ports),
                    );
                }
                // SPEC: docs/specs/contracts/level-combinational-expansion.yaml
                // "nonblocking-quality-findings" — arity warnings suppress the
                // generic constant-result warning for the same static cause.
                if let NodeKind::AtLeast(config) = node.kind()
                    && inputs.len() > 1
                    && (config.threshold == 0
                        || usize::try_from(config.threshold)
                            .map_or(true, |threshold| threshold > inputs.len()))
                {
                    self.add(
                        SubjectRef::Node(node.key()),
                        ProblemEvidence::constant_result_node(
                            inputs.iter().copied().map(SubjectRef::InPort).collect(),
                            if config.threshold == 0 {
                                LogicLevel::High
                            } else {
                                LogicLevel::Low
                            },
                        ),
                    );
                }
            }
        }
    }

    fn validate_module_instances(&mut self) {
        for instance in self.network.module_instances() {
            let required: BTreeSet<_> = instance
                .module()
                .inputs()
                .map(|input| input.key())
                .collect();
            let mut claims: BTreeMap<AnyModuleInputKey, Vec<SubjectRef>> = BTreeMap::new();
            for binding in instance.bindings().bindings() {
                let primary = SubjectRef::ModuleInstanceInput(instance.key(), binding.input());
                let source_subject = endpoint_subject(binding.source());
                claims
                    .entry(binding.input())
                    .or_default()
                    .push(source_subject);
                let valid_source = self.instance_source_exists(binding.source());
                if !required.contains(&binding.input())
                    || !is_source(binding.source())
                    || binding.input().kind() != binding.source().kind()
                    || !valid_source
                {
                    self.add(
                        primary,
                        ProblemEvidence::invalid_module_binding(
                            instance.key(),
                            binding.input(),
                            vec![source_subject],
                        ),
                    );
                }
            }
            for input in required {
                let sources = claims.get(&input).cloned().unwrap_or_default();
                if sources.len() != 1 {
                    self.add(
                        SubjectRef::ModuleInstanceInput(instance.key(), input),
                        ProblemEvidence::invalid_module_binding(instance.key(), input, sources),
                    );
                }
            }
            if let Some(parent) = instance.parent()
                && !self.module_instances.contains_key(&parent)
            {
                self.add(
                    SubjectRef::ModuleInstance(instance.key()),
                    ProblemEvidence::malformed_hierarchy(instance.key(), parent),
                );
            }
        }

        let parents: BTreeMap<_, _> = self
            .network
            .module_instances()
            .iter()
            .filter_map(|instance| instance.parent().map(|parent| (instance.key(), parent)))
            .collect();
        let mut emitted = BTreeSet::new();
        for start in parents.keys().copied() {
            let mut path = Vec::new();
            let mut positions = BTreeMap::new();
            let mut current = start;
            while let Some(parent) = parents.get(&current).copied() {
                if let Some(position) = positions.get(&current).copied() {
                    let mut cycle = path[position..].to_vec();
                    cycle.sort();
                    cycle.dedup();
                    if emitted.insert(cycle.clone()) {
                        self.add(
                            SubjectRef::ModuleInstance(cycle[0]),
                            ProblemEvidence::hierarchy_cycle(cycle),
                        );
                    }
                    break;
                }
                positions.insert(current, path.len());
                path.push(current);
                current = parent;
            }
        }
    }

    fn instance_source_exists(&self, source: ConnectionEndpoint) -> bool {
        match source {
            ConnectionEndpoint::ExternalInput(key) => self.external_inputs.contains(&key),
            ConnectionEndpoint::NodeOutput(key) => self.outputs.contains(&key),
            ConnectionEndpoint::ModuleInput(key) => self.module_inputs.contains(&key),
            ConnectionEndpoint::ModuleOutput { instance, output } => self
                .module_instances
                .get(&instance)
                .is_some_and(|definition| {
                    definition
                        .module()
                        .outputs()
                        .any(|candidate| candidate.key() == output)
                }),
            ConnectionEndpoint::NodeInput(_) | ConnectionEndpoint::ExternalOutput(_) => false,
        }
    }

    fn validate_connections(&mut self) {
        let mut drivers: BTreeMap<AnyInPortKey, Vec<SubjectRef>> = BTreeMap::new();
        let mut variadic_inputs = BTreeMap::new();
        for node in self.network.nodes().iter().filter(|node| {
            matches!(
                node.kind(),
                NodeKind::All
                    | NodeKind::Any
                    | NodeKind::Parity
                    | NodeKind::AtLeast(_)
                    | NodeKind::Merge
            )
        }) {
            for input in node.ports().inputs() {
                variadic_inputs.insert(*input, node.key());
            }
        }
        let mut variadic_sources: BTreeMap<
            crate::key::NodeKey,
            BTreeMap<AnySignalSourceKey, BTreeSet<AnyInPortKey>>,
        > = BTreeMap::new();
        for connection in self.network.connections() {
            let source = connection.from();
            let target = connection.to();
            let source_subject = endpoint_subject(source);
            let target_subject = endpoint_subject(target);
            let source_valid = self.validate_endpoint(connection.key(), source);
            let target_valid = self.validate_endpoint(connection.key(), target);
            if !is_source(source) || !is_target(target) {
                self.add(
                    SubjectRef::Connection(connection.key()),
                    ProblemEvidence::invalid_direction(source_subject, target_subject),
                );
            }
            if source.kind() != target.kind() {
                self.add(
                    SubjectRef::Connection(connection.key()),
                    ProblemEvidence::signal_kind_mismatch(
                        source_subject,
                        target_subject,
                        source.kind(),
                        target.kind(),
                    ),
                );
            }
            if source_valid
                && target_valid
                && is_source(source)
                && is_target(target)
                && source.kind() == target.kind()
            {
                if let ConnectionEndpoint::NodeInput(input) = target {
                    drivers
                        .entry(input)
                        .or_default()
                        .push(SubjectRef::Connection(connection.key()));
                    if let Some(owner) = variadic_inputs.get(&input)
                        && let Some(source) = endpoint_signal_source(source)
                    {
                        variadic_sources
                            .entry(*owner)
                            .or_default()
                            .entry(source)
                            .or_default()
                            .insert(input);
                    }
                }
            }
        }
        for (input, found) in drivers.iter().filter(|(_, found)| found.len() > 1) {
            self.add(
                SubjectRef::InPort(*input),
                ProblemEvidence::unsupported_multiple_drivers(found.clone()),
            );
        }
        for node in self.network.nodes() {
            for input in node.ports().inputs() {
                if !drivers.contains_key(input) && !self.boundary_drivers.contains(input) {
                    self.add(
                        SubjectRef::Node(node.key()),
                        ProblemEvidence::missing_required_input(
                            SubjectRef::InPort(*input),
                            input.kind(),
                        ),
                    );
                }
            }
        }
        for (node, sources) in variadic_sources {
            for (source, ports) in sources {
                if ports.len() > 1 {
                    self.add(
                        SubjectRef::Node(node),
                        ProblemEvidence::duplicate_source(
                            signal_source_subject(source),
                            ports.into_iter().map(SubjectRef::InPort).collect(),
                        ),
                    );
                }
            }
        }
    }

    fn validate_endpoint(
        &mut self,
        connection: crate::key::ConnectionKey,
        endpoint: ConnectionEndpoint,
    ) -> bool {
        match endpoint {
            ConnectionEndpoint::ExternalInput(key) if !self.external_inputs.contains(&key) => {
                self.add(
                    SubjectRef::Connection(connection),
                    ProblemEvidence::missing_endpoint(SubjectRef::ExternalInput(key), key.kind()),
                );
                false
            }
            ConnectionEndpoint::NodeOutput(key) if !self.outputs.contains(&key) => {
                self.add(
                    SubjectRef::Connection(connection),
                    ProblemEvidence::missing_port(SubjectRef::OutPort(key), key.kind()),
                );
                false
            }
            ConnectionEndpoint::NodeInput(key) if !self.inputs.contains(&key) => {
                self.add(
                    SubjectRef::Connection(connection),
                    ProblemEvidence::missing_port(SubjectRef::InPort(key), key.kind()),
                );
                false
            }
            ConnectionEndpoint::ExternalOutput(key) if !self.external_outputs.contains(&key) => {
                self.add(
                    SubjectRef::Connection(connection),
                    ProblemEvidence::missing_endpoint(SubjectRef::ExternalOutput(key), key.kind()),
                );
                false
            }
            ConnectionEndpoint::ModuleInput(key) if !self.module_inputs.contains(&key) => {
                self.add(
                    SubjectRef::ModuleInput(key),
                    ProblemEvidence::missing_endpoint(SubjectRef::ModuleInput(key), key.kind()),
                );
                false
            }
            ConnectionEndpoint::ModuleOutput { instance, output }
                if !self.instance_source_exists(endpoint) =>
            {
                self.add(
                    SubjectRef::ModuleInstance(instance),
                    ProblemEvidence::missing_endpoint(
                        SubjectRef::ModuleInstanceOutput(instance, output),
                        output.kind(),
                    ),
                );
                false
            }
            _ => true,
        }
    }

    fn validate_external_outputs(&mut self) {
        for output in self.network.external_outputs() {
            if output.key().kind() != output.source().kind() {
                self.add(
                    SubjectRef::ExternalOutput(output.key()),
                    ProblemEvidence::signal_kind_mismatch(
                        signal_source_subject(output.source()),
                        SubjectRef::ExternalOutput(output.key()),
                        output.source().kind(),
                        output.key().kind(),
                    ),
                );
            }
            match output.source() {
                AnySignalSourceKey::Level(SignalSourceKey::ExternalInput(key))
                    if !self.external_inputs.contains(&key.into()) =>
                {
                    self.add(
                        SubjectRef::ExternalOutput(output.key()),
                        ProblemEvidence::missing_endpoint(
                            SubjectRef::ExternalInput(key.into()),
                            SignalKind::Level,
                        ),
                    )
                }
                AnySignalSourceKey::Pulse(SignalSourceKey::ExternalInput(key))
                    if !self.external_inputs.contains(&key.into()) =>
                {
                    self.add(
                        SubjectRef::ExternalOutput(output.key()),
                        ProblemEvidence::missing_endpoint(
                            SubjectRef::ExternalInput(key.into()),
                            SignalKind::Pulse,
                        ),
                    )
                }
                AnySignalSourceKey::Level(SignalSourceKey::NodeOutput(key))
                    if !self.outputs.contains(&key.into()) =>
                {
                    self.add(
                        SubjectRef::ExternalOutput(output.key()),
                        ProblemEvidence::missing_port(
                            SubjectRef::OutPort(key.into()),
                            SignalKind::Level,
                        ),
                    )
                }
                AnySignalSourceKey::Pulse(SignalSourceKey::NodeOutput(key))
                    if !self.outputs.contains(&key.into()) =>
                {
                    self.add(
                        SubjectRef::ExternalOutput(output.key()),
                        ProblemEvidence::missing_port(
                            SubjectRef::OutPort(key.into()),
                            SignalKind::Pulse,
                        ),
                    )
                }
                AnySignalSourceKey::Level(SignalSourceKey::ModuleOutput {
                    instance,
                    output: module_output,
                }) if !self.instance_source_exists(ConnectionEndpoint::module_output(
                    instance,
                    module_output.into(),
                )) =>
                {
                    self.add(
                        SubjectRef::ExternalOutput(output.key()),
                        ProblemEvidence::missing_endpoint(
                            SubjectRef::ModuleInstanceOutput(instance, module_output.into()),
                            SignalKind::Level,
                        ),
                    )
                }
                AnySignalSourceKey::Pulse(SignalSourceKey::ModuleOutput {
                    instance,
                    output: module_output,
                }) if !self.instance_source_exists(ConnectionEndpoint::module_output(
                    instance,
                    module_output.into(),
                )) =>
                {
                    self.add(
                        SubjectRef::ExternalOutput(output.key()),
                        ProblemEvidence::missing_endpoint(
                            SubjectRef::ModuleInstanceOutput(instance, module_output.into()),
                            SignalKind::Pulse,
                        ),
                    )
                }
                _ => {}
            }
        }
    }
}

fn is_source(endpoint: ConnectionEndpoint) -> bool {
    matches!(
        endpoint,
        ConnectionEndpoint::ExternalInput(_)
            | ConnectionEndpoint::NodeOutput(_)
            | ConnectionEndpoint::ModuleInput(_)
            | ConnectionEndpoint::ModuleOutput { .. }
    )
}

fn is_target(endpoint: ConnectionEndpoint) -> bool {
    matches!(endpoint, ConnectionEndpoint::NodeInput(_))
}

fn reaction_source(endpoint: ConnectionEndpoint) -> Option<ReactionVertex> {
    match endpoint {
        ConnectionEndpoint::ExternalInput(key) => Some(ReactionVertex::ExternalInput(key)),
        ConnectionEndpoint::NodeOutput(key) => Some(ReactionVertex::NodeOutput(key)),
        ConnectionEndpoint::ModuleInput(key) => Some(ReactionVertex::ModuleInput(key)),
        ConnectionEndpoint::ModuleOutput { instance, output } => {
            Some(ReactionVertex::InstanceOutput(instance, output))
        }
        ConnectionEndpoint::NodeInput(_) | ConnectionEndpoint::ExternalOutput(_) => None,
    }
}

fn reaction_source_from_signal(source: AnySignalSourceKey) -> ReactionVertex {
    match source {
        AnySignalSourceKey::Level(SignalSourceKey::ExternalInput(key)) => {
            ReactionVertex::ExternalInput(key.into())
        }
        AnySignalSourceKey::Pulse(SignalSourceKey::ExternalInput(key)) => {
            ReactionVertex::ExternalInput(key.into())
        }
        AnySignalSourceKey::Level(SignalSourceKey::NodeOutput(key)) => {
            ReactionVertex::NodeOutput(key.into())
        }
        AnySignalSourceKey::Pulse(SignalSourceKey::NodeOutput(key)) => {
            ReactionVertex::NodeOutput(key.into())
        }
        AnySignalSourceKey::Level(SignalSourceKey::ModuleOutput { instance, output }) => {
            ReactionVertex::InstanceOutput(instance, output.into())
        }
        AnySignalSourceKey::Pulse(SignalSourceKey::ModuleOutput { instance, output }) => {
            ReactionVertex::InstanceOutput(instance, output.into())
        }
    }
}

fn endpoint_subject(endpoint: ConnectionEndpoint) -> SubjectRef {
    match endpoint {
        ConnectionEndpoint::ExternalInput(key) => SubjectRef::ExternalInput(key),
        ConnectionEndpoint::NodeInput(key) => SubjectRef::InPort(key),
        ConnectionEndpoint::NodeOutput(key) => SubjectRef::OutPort(key),
        ConnectionEndpoint::ModuleInput(key) => SubjectRef::ModuleInput(key),
        ConnectionEndpoint::ModuleOutput { instance, output } => {
            SubjectRef::ModuleInstanceOutput(instance, output)
        }
        ConnectionEndpoint::ExternalOutput(key) => SubjectRef::ExternalOutput(key),
    }
}

fn signal_source_subject(source: AnySignalSourceKey) -> SubjectRef {
    match source {
        AnySignalSourceKey::Level(SignalSourceKey::ExternalInput(key)) => {
            SubjectRef::ExternalInput(key.into())
        }
        AnySignalSourceKey::Pulse(SignalSourceKey::ExternalInput(key)) => {
            SubjectRef::ExternalInput(key.into())
        }
        AnySignalSourceKey::Level(SignalSourceKey::NodeOutput(key)) => {
            SubjectRef::OutPort(key.into())
        }
        AnySignalSourceKey::Pulse(SignalSourceKey::NodeOutput(key)) => {
            SubjectRef::OutPort(key.into())
        }
        AnySignalSourceKey::Level(SignalSourceKey::ModuleOutput { instance, output }) => {
            SubjectRef::ModuleInstanceOutput(instance, output.into())
        }
        AnySignalSourceKey::Pulse(SignalSourceKey::ModuleOutput { instance, output }) => {
            SubjectRef::ModuleInstanceOutput(instance, output.into())
        }
    }
}

fn endpoint_signal_source(endpoint: ConnectionEndpoint) -> Option<AnySignalSourceKey> {
    match endpoint {
        ConnectionEndpoint::ExternalInput(AnyExternalInputKey::Level(key)) => {
            Some(SignalSourceKey::ExternalInput(key).into())
        }
        ConnectionEndpoint::ExternalInput(AnyExternalInputKey::Pulse(key)) => {
            Some(SignalSourceKey::ExternalInput(key).into())
        }
        ConnectionEndpoint::NodeOutput(AnyOutPortKey::Level(key)) => {
            Some(SignalSourceKey::NodeOutput(key).into())
        }
        ConnectionEndpoint::NodeOutput(AnyOutPortKey::Pulse(key)) => {
            Some(SignalSourceKey::NodeOutput(key).into())
        }
        ConnectionEndpoint::ModuleOutput {
            instance,
            output: AnyModuleOutputKey::Level(output),
        } => Some(SignalSourceKey::ModuleOutput { instance, output }.into()),
        ConnectionEndpoint::ModuleOutput {
            instance,
            output: AnyModuleOutputKey::Pulse(output),
        } => Some(SignalSourceKey::ModuleOutput { instance, output }.into()),
        ConnectionEndpoint::NodeInput(_)
        | ConnectionEndpoint::ModuleInput(_)
        | ConnectionEndpoint::ExternalOutput(_) => None,
    }
}

fn duplicate_claim_subject(claim: &DuplicateClaim) -> SubjectRef {
    match claim {
        DuplicateClaim::Node { key, .. } => SubjectRef::Node(*key),
        DuplicateClaim::InPort { key, .. } => SubjectRef::InPort(*key),
        DuplicateClaim::OutPort { key, .. } => SubjectRef::OutPort(*key),
        DuplicateClaim::Connection { key, .. } => SubjectRef::Connection(*key),
        DuplicateClaim::ExternalInput { key, .. } => SubjectRef::ExternalInput(*key),
        DuplicateClaim::ExternalOutput { key, .. } => SubjectRef::ExternalOutput(*key),
        DuplicateClaim::ModuleInput { key, .. } => SubjectRef::ModuleInput(*key),
        DuplicateClaim::ModuleOutput { key, .. } => SubjectRef::ModuleOutput(*key),
        DuplicateClaim::ModuleInstance { key, .. } => SubjectRef::ModuleInstance(*key),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::authored::{
        ConnectionDef, ExternalInputDef, ExternalOutputDef, InputPortRole, NodeDef, NodePorts,
    };
    use crate::diagnostics::DiagnosticCode;
    use crate::key::{
        ConnectionKey, ExternalInputKey, ExternalOutputKey, InPortKey, NetworkKey, NodeKey,
        OutPortKey, SignalSourceKey,
    };
    use crate::metadata::DiagnosticMeta;
    use crate::signal::{Level, LogicLevel, Pulse};
    use crate::time::NonZeroSpan;

    fn all_definition(
        inputs: Vec<InPortKey<Level>>,
        connections: Vec<ConnectionDef>,
    ) -> UncheckedNetwork<()> {
        UncheckedNetwork::new(
            NetworkKey::from_u128(1),
            TimeDomainId::from_u128(2),
            DiagnosticMeta::default(),
            vec![NodeDef::new(
                NodeKey::from_u128(3),
                NodeKind::all(),
                NodePorts::new(
                    inputs.into_iter().map(Into::into).collect(),
                    vec![OutPortKey::<Level>::from_u128(4).into()],
                ),
                DiagnosticMeta::default(),
            )],
            vec![ExternalInputDef::new(
                ExternalInputKey::<Level>::from_u128(5).into(),
                DiagnosticMeta::default(),
            )],
            vec![],
            connections,
        )
    }

    fn variadic_definition(
        kind: NodeKind<()>,
        inputs: Vec<InPortKey<Level>>,
        connections: Vec<ConnectionDef>,
    ) -> UncheckedNetwork<()> {
        UncheckedNetwork::new(
            NetworkKey::from_u128(1),
            TimeDomainId::from_u128(2),
            DiagnosticMeta::default(),
            vec![NodeDef::new(
                NodeKey::from_u128(3),
                kind,
                NodePorts::new(
                    inputs.into_iter().map(Into::into).collect(),
                    vec![OutPortKey::<Level>::from_u128(4).into()],
                ),
                DiagnosticMeta::default(),
            )],
            vec![ExternalInputDef::new(
                ExternalInputKey::<Level>::from_u128(5).into(),
                DiagnosticMeta::default(),
            )],
            vec![],
            connections,
        )
    }

    #[test]
    fn zero_and_unary_all_are_valid_with_canonical_warnings() {
        let empty = all_definition(vec![], vec![]).validate();
        assert!(empty.artifact().is_some());
        assert_eq!(empty.diagnostics().len(), 1);
        let empty_problem = empty.diagnostics().iter().next().unwrap().problem();
        assert_eq!(
            empty_problem.code(),
            DiagnosticCode::ValidationEmptyVariadicNode
        );
        assert_eq!(
            empty_problem.severity(),
            crate::diagnostics::Severity::Warning
        );
        assert_eq!(
            empty_problem.responsibility(),
            crate::diagnostics::Responsibility::Advisory
        );
        assert_eq!(
            empty_problem.primary(),
            &SubjectRef::Node(NodeKey::from_u128(3))
        );
        assert!(matches!(
            empty_problem.evidence(),
            ProblemEvidence::ValidationEmptyVariadicNode { ports, .. } if ports.is_empty()
        ));

        let input = InPortKey::<Level>::from_u128(6);
        let unary = all_definition(
            vec![input],
            vec![ConnectionDef::new(
                ConnectionKey::from_u128(7),
                ExternalInputKey::<Level>::from_u128(5).into(),
                input.into(),
                DiagnosticMeta::default(),
            )],
        )
        .validate();
        assert!(unary.artifact().is_some());
        assert_eq!(unary.diagnostics().len(), 1);
        let unary_problem = unary.diagnostics().iter().next().unwrap().problem();
        assert_eq!(
            unary_problem.code(),
            DiagnosticCode::ValidationUnaryDegenerateNode
        );
        assert_eq!(
            unary_problem.severity(),
            crate::diagnostics::Severity::Warning
        );
        assert_eq!(
            unary_problem.responsibility(),
            crate::diagnostics::Responsibility::Advisory
        );
        assert_eq!(
            unary_problem.primary(),
            &SubjectRef::Node(NodeKey::from_u128(3))
        );
        assert!(matches!(
            unary_problem.evidence(),
            ProblemEvidence::ValidationUnaryDegenerateNode { ports, .. }
                if ports == &vec![SubjectRef::InPort(input.into())]
        ));
    }

    #[test]
    fn all_reports_duplicate_sources_without_collapsing_ports() {
        let first = InPortKey::<Level>::from_u128(6);
        let second = InPortKey::<Level>::from_u128(7);
        let source = ExternalInputKey::<Level>::from_u128(5);
        let definition = all_definition(
            vec![second, first],
            vec![
                ConnectionDef::new(
                    ConnectionKey::from_u128(9),
                    source.into(),
                    second.into(),
                    DiagnosticMeta::default(),
                ),
                ConnectionDef::new(
                    ConnectionKey::from_u128(8),
                    source.into(),
                    first.into(),
                    DiagnosticMeta::default(),
                ),
            ],
        );
        let structural = definition.validate_structural();
        let connection_dependencies = structural
            .artifact()
            .unwrap()
            .reaction_dependencies()
            .dependencies()
            .filter(|dependency| {
                matches!(dependency.subject, ReactionDependencySubject::Connection(_))
            })
            .count();
        assert_eq!(connection_dependencies, 2);
        let report = definition.validate();
        assert!(report.artifact().is_some());
        assert_eq!(report.diagnostics().len(), 1);
        let problem = report.diagnostics().iter().next().unwrap().problem();
        assert_eq!(problem.severity(), crate::diagnostics::Severity::Warning);
        assert_eq!(
            problem.responsibility(),
            crate::diagnostics::Responsibility::CallerInput
        );
        assert_eq!(problem.primary(), &SubjectRef::Node(NodeKey::from_u128(3)));
        match problem.evidence() {
            ProblemEvidence::ValidationDuplicateSource {
                source: actual,
                ports,
                ..
            } => {
                assert_eq!(*actual, SubjectRef::ExternalInput(source.into()));
                assert_eq!(
                    ports,
                    &vec![
                        SubjectRef::InPort(first.into()),
                        SubjectRef::InPort(second.into())
                    ]
                );
            }
            _ => panic!("expected duplicate-source evidence"),
        }
    }

    #[test]
    fn remaining_variadics_report_duplicate_sources_without_collapsing_ports() {
        let first = InPortKey::<Level>::from_u128(6);
        let second = InPortKey::<Level>::from_u128(7);
        let source = ExternalInputKey::<Level>::from_u128(5);

        for kind in [
            NodeKind::<()>::any(),
            NodeKind::parity(),
            NodeKind::at_least(2),
        ] {
            let report = variadic_definition(
                kind,
                vec![second, first],
                vec![
                    ConnectionDef::new(
                        ConnectionKey::from_u128(9),
                        source.into(),
                        second.into(),
                        DiagnosticMeta::default(),
                    ),
                    ConnectionDef::new(
                        ConnectionKey::from_u128(8),
                        source.into(),
                        first.into(),
                        DiagnosticMeta::default(),
                    ),
                ],
            )
            .validate();
            assert!(report.artifact().is_some());
            let duplicate = report
                .diagnostics()
                .iter()
                .find(|diagnostic| {
                    diagnostic.problem().code() == DiagnosticCode::ValidationDuplicateSource
                })
                .unwrap_or_else(|| panic!("repeated variadic source must be diagnosed"));
            assert!(matches!(
                duplicate.problem().evidence(),
                ProblemEvidence::ValidationDuplicateSource {
                    source: actual,
                    ports,
                    ..
                } if *actual == SubjectRef::ExternalInput(source.into())
                    && ports == &vec![
                        SubjectRef::InPort(first.into()),
                        SubjectRef::InPort(second.into()),
                    ]
            ));
        }
    }

    #[test]
    fn remaining_variadic_advisories_are_nonblocking_and_preserve_artifacts() {
        for kind in [NodeKind::any(), NodeKind::parity(), NodeKind::at_least(1)] {
            let report = variadic_definition(kind, Vec::new(), Vec::new()).validate();
            assert!(report.artifact().is_some());
            assert_eq!(report.diagnostics().len(), 1);
            assert_eq!(
                report.diagnostics().iter().next().unwrap().problem().code(),
                DiagnosticCode::ValidationEmptyVariadicNode
            );
        }

        let input = InPortKey::<Level>::from_u128(6);
        let connection = || {
            ConnectionDef::new(
                ConnectionKey::from_u128(7),
                ExternalInputKey::<Level>::from_u128(5).into(),
                input.into(),
                DiagnosticMeta::default(),
            )
        };
        for kind in [NodeKind::any(), NodeKind::parity(), NodeKind::at_least(1)] {
            let report = variadic_definition(kind, vec![input], vec![connection()]).validate();
            assert!(report.artifact().is_some());
            assert_eq!(
                report.diagnostics().iter().next().unwrap().problem().code(),
                DiagnosticCode::ValidationUnaryDegenerateNode
            );
        }
    }

    #[test]
    fn at_least_reports_constant_results_without_rejecting_valid_structure() {
        let first = InPortKey::<Level>::from_u128(6);
        let second = InPortKey::<Level>::from_u128(7);
        let connections = |source| {
            vec![
                ConnectionDef::new(
                    ConnectionKey::from_u128(8),
                    source,
                    first.into(),
                    DiagnosticMeta::default(),
                ),
                ConnectionDef::new(
                    ConnectionKey::from_u128(9),
                    source,
                    second.into(),
                    DiagnosticMeta::default(),
                ),
            ]
        };
        for (threshold, expected) in [(0, LogicLevel::High), (3, LogicLevel::Low)] {
            let report = variadic_definition(
                NodeKind::at_least(threshold),
                vec![first, second],
                connections(ExternalInputKey::<Level>::from_u128(5).into()),
            )
            .validate();
            assert!(report.artifact().is_some());
            let constant = report
                .diagnostics()
                .iter()
                .find(|diagnostic| {
                    diagnostic.problem().code() == DiagnosticCode::ValidationConstantResultNode
                })
                .unwrap_or_else(|| panic!("constant threshold must report its static result"));
            assert!(matches!(
                constant.problem().evidence(),
                ProblemEvidence::ValidationConstantResultNode { result, .. }
                    if *result == expected
            ));
        }
    }

    #[test]
    fn unary_at_least_suppresses_the_less_specific_constant_result_warning() {
        let input = InPortKey::<Level>::from_u128(6);
        let connection = || {
            ConnectionDef::new(
                ConnectionKey::from_u128(7),
                ExternalInputKey::<Level>::from_u128(5).into(),
                input.into(),
                DiagnosticMeta::default(),
            )
        };

        for threshold in [0, 2] {
            let report = variadic_definition(
                NodeKind::at_least(threshold),
                vec![input],
                vec![connection()],
            )
            .validate();
            assert!(report.artifact().is_some());
            assert_eq!(report.diagnostics().len(), 1);
            assert_eq!(
                report
                    .diagnostics()
                    .iter()
                    .next()
                    .map(|diagnostic| diagnostic.problem().code()),
                Some(DiagnosticCode::ValidationUnaryDegenerateNode)
            );
        }
    }

    #[test]
    fn select_requires_distinct_fixed_roles_and_retains_all_static_dependencies() {
        let ports = [
            InPortKey::<Level>::from_u128(10),
            InPortKey::<Level>::from_u128(11),
            InPortKey::<Level>::from_u128(12),
        ];
        let inputs = [
            ExternalInputKey::<Level>::from_u128(20),
            ExternalInputKey::<Level>::from_u128(21),
            ExternalInputKey::<Level>::from_u128(22),
        ];
        let output = OutPortKey::<Level>::from_u128(30);
        let definition = |roles: Vec<InputPortRole>| {
            UncheckedNetwork::new(
                NetworkKey::from_u128(1),
                TimeDomainId::from_u128(2),
                DiagnosticMeta::default(),
                vec![NodeDef::new(
                    NodeKey::from_u128(3),
                    NodeKind::<()>::select(),
                    NodePorts::with_input_roles(
                        ports.into_iter().map(Into::into).collect(),
                        roles,
                        vec![output.into()],
                    ),
                    DiagnosticMeta::default(),
                )],
                inputs
                    .into_iter()
                    .map(|key| ExternalInputDef::new(key.into(), DiagnosticMeta::default()))
                    .collect(),
                Vec::new(),
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
        };

        let valid = definition(vec![
            InputPortRole::Selector,
            InputPortRole::WhenLow,
            InputPortRole::WhenHigh,
        ])
        .validate();
        let artifact = valid
            .artifact()
            .unwrap_or_else(|| panic!("complete Select roles must validate"));
        let dependencies = artifact.reaction_dependencies();
        for input in inputs {
            assert!(dependencies.dependencies().any(|edge| {
                edge.from == ReactionVertex::ExternalInput(input.into())
                    && edge.to == ReactionVertex::NodeOperation(NodeKey::from_u128(3))
            }));
        }

        let invalid = definition(vec![
            InputPortRole::Selector,
            InputPortRole::WhenLow,
            InputPortRole::WhenLow,
        ])
        .validate();
        assert!(invalid.artifact().is_none());
        assert!(invalid.diagnostics().iter().any(|diagnostic| {
            diagnostic.problem().code() == DiagnosticCode::ValidationInvalidFixedArity
        }));
    }

    #[test]
    fn every_remaining_kind_participates_in_current_reaction_cycle_rejection() {
        for kind in [
            NodeKind::<()>::any(),
            NodeKind::parity(),
            NodeKind::at_least(1),
        ] {
            let input = InPortKey::<Level>::from_u128(10);
            let output = OutPortKey::<Level>::from_u128(11);
            let report = UncheckedNetwork::new(
                NetworkKey::from_u128(1),
                TimeDomainId::from_u128(2),
                DiagnosticMeta::default(),
                vec![NodeDef::new(
                    NodeKey::from_u128(3),
                    kind,
                    NodePorts::new(vec![input.into()], vec![output.into()]),
                    DiagnosticMeta::default(),
                )],
                Vec::new(),
                Vec::new(),
                vec![ConnectionDef::new(
                    ConnectionKey::from_u128(4),
                    output.into(),
                    input.into(),
                    DiagnosticMeta::default(),
                )],
            )
            .validate();
            assert!(report.artifact().is_none());
            assert!(report.diagnostics().iter().any(|diagnostic| {
                diagnostic.problem().code() == DiagnosticCode::ValidationCurrentReactionCycle
            }));
        }

        let pulse_input = InPortKey::<Pulse>::from_u128(20);
        let pulse_output = OutPortKey::<Pulse>::from_u128(21);
        let merge = UncheckedNetwork::new(
            NetworkKey::from_u128(1),
            TimeDomainId::from_u128(2),
            DiagnosticMeta::default(),
            vec![NodeDef::new(
                NodeKey::from_u128(3),
                NodeKind::<()>::merge(),
                NodePorts::new(vec![pulse_input.into()], vec![pulse_output.into()]),
                DiagnosticMeta::default(),
            )],
            Vec::new(),
            Vec::new(),
            vec![ConnectionDef::new(
                ConnectionKey::from_u128(4),
                pulse_output.into(),
                pulse_input.into(),
                DiagnosticMeta::default(),
            )],
        )
        .validate();
        assert!(merge.artifact().is_none());
        assert!(merge.diagnostics().iter().any(|diagnostic| {
            diagnostic.problem().code() == DiagnosticCode::ValidationCurrentReactionCycle
        }));

        let ports = [
            InPortKey::<Level>::from_u128(10),
            InPortKey::<Level>::from_u128(11),
            InPortKey::<Level>::from_u128(12),
        ];
        let output = OutPortKey::<Level>::from_u128(13);
        let external = ExternalInputKey::<Level>::from_u128(14);
        let select = UncheckedNetwork::new(
            NetworkKey::from_u128(1),
            TimeDomainId::from_u128(2),
            DiagnosticMeta::default(),
            vec![NodeDef::new(
                NodeKey::from_u128(3),
                NodeKind::<()>::select(),
                NodePorts::with_input_roles(
                    ports.into_iter().map(Into::into).collect(),
                    vec![
                        InputPortRole::Selector,
                        InputPortRole::WhenLow,
                        InputPortRole::WhenHigh,
                    ],
                    vec![output.into()],
                ),
                DiagnosticMeta::default(),
            )],
            vec![ExternalInputDef::new(
                external.into(),
                DiagnosticMeta::default(),
            )],
            Vec::new(),
            vec![
                ConnectionDef::new(
                    ConnectionKey::from_u128(4),
                    output.into(),
                    ports[0].into(),
                    DiagnosticMeta::default(),
                ),
                ConnectionDef::new(
                    ConnectionKey::from_u128(5),
                    external.into(),
                    ports[1].into(),
                    DiagnosticMeta::default(),
                ),
                ConnectionDef::new(
                    ConnectionKey::from_u128(6),
                    external.into(),
                    ports[2].into(),
                    DiagnosticMeta::default(),
                ),
            ],
        )
        .validate();
        assert!(select.artifact().is_none());
        assert!(select.diagnostics().iter().any(|diagnostic| {
            diagnostic.problem().code() == DiagnosticCode::ValidationCurrentReactionCycle
        }));
    }

    #[test]
    fn all_current_reaction_signature_detects_a_self_cycle() {
        let input = InPortKey::<Level>::from_u128(6);
        let output = OutPortKey::<Level>::from_u128(4);
        let report = all_definition(
            vec![input],
            vec![ConnectionDef::new(
                ConnectionKey::from_u128(7),
                output.into(),
                input.into(),
                DiagnosticMeta::default(),
            )],
        )
        .validate();
        assert!(report.artifact().is_none());
        assert!(report.diagnostics().iter().any(|diagnostic| {
            diagnostic.problem().code() == DiagnosticCode::ValidationCurrentReactionCycle
        }));
    }

    #[test]
    fn derives_current_reaction_dependencies_separately_from_authored_connections() {
        let constant_output = OutPortKey::<Level>::from_u128(1);
        let not_input = InPortKey::<Level>::from_u128(2);
        let not_output = OutPortKey::<Level>::from_u128(3);
        let network = UncheckedNetwork::new(
            NetworkKey::from_u128(1),
            crate::identity::TimeDomainId::from_u128(1),
            DiagnosticMeta::default(),
            vec![
                NodeDef::new(
                    NodeKey::from_u128(10),
                    NodeKind::<()>::constant(LogicLevel::High),
                    NodePorts::new(vec![], vec![constant_output.into()]),
                    DiagnosticMeta::default(),
                ),
                NodeDef::new(
                    NodeKey::from_u128(20),
                    NodeKind::<()>::not(),
                    NodePorts::new(vec![not_input.into()], vec![not_output.into()]),
                    DiagnosticMeta::default(),
                ),
            ],
            vec![],
            vec![ExternalOutputDef::new(
                ExternalOutputKey::<Level>::from_u128(4).into(),
                SignalSourceKey::NodeOutput(not_output).into(),
                DiagnosticMeta::default(),
            )],
            vec![ConnectionDef::new(
                ConnectionKey::from_u128(5),
                constant_output.into(),
                not_input.into(),
                DiagnosticMeta::default(),
            )],
        );

        let report = network.validate_structural();
        let graph = report
            .artifact()
            .expect("the restricted graph is structurally valid")
            .reaction_dependencies();

        assert_eq!(
            graph.dependencies().collect::<Vec<_>>(),
            vec![
                ReactionDependency {
                    from: ReactionVertex::NodeOperation(NodeKey::from_u128(10)),
                    to: ReactionVertex::NodeOutput(constant_output.into()),
                    subject: ReactionDependencySubject::Node(NodeKey::from_u128(10)),
                },
                ReactionDependency {
                    from: ReactionVertex::NodeOperation(NodeKey::from_u128(20)),
                    to: ReactionVertex::NodeOutput(not_output.into()),
                    subject: ReactionDependencySubject::Node(NodeKey::from_u128(20)),
                },
                ReactionDependency {
                    from: ReactionVertex::NodeOutput(constant_output.into()),
                    to: ReactionVertex::NodeOperation(NodeKey::from_u128(20)),
                    subject: ReactionDependencySubject::Connection(ConnectionKey::from_u128(5)),
                },
                ReactionDependency {
                    from: ReactionVertex::NodeOutput(not_output.into()),
                    to: ReactionVertex::ExternalOutput(
                        ExternalOutputKey::<Level>::from_u128(4).into(),
                    ),
                    subject: ReactionDependencySubject::ExternalOutput(
                        ExternalOutputKey::<Level>::from_u128(4).into(),
                    ),
                },
            ]
        );
        assert_eq!(graph.vertices().count(), 5);
    }

    #[test]
    fn derives_external_input_roots_and_not_signature() {
        let external_input = ExternalInputKey::<Level>::from_u128(1);
        let not_input = InPortKey::<Level>::from_u128(2);
        let not_output = OutPortKey::<Level>::from_u128(3);
        let external_output = ExternalOutputKey::<Level>::from_u128(4);
        let network = UncheckedNetwork::new(
            NetworkKey::from_u128(1),
            crate::identity::TimeDomainId::from_u128(1),
            DiagnosticMeta::default(),
            vec![NodeDef::new(
                NodeKey::from_u128(10),
                NodeKind::<()>::not(),
                NodePorts::new(vec![not_input.into()], vec![not_output.into()]),
                DiagnosticMeta::default(),
            )],
            vec![ExternalInputDef::new(
                external_input.into(),
                DiagnosticMeta::default(),
            )],
            vec![ExternalOutputDef::new(
                external_output.into(),
                SignalSourceKey::NodeOutput(not_output).into(),
                DiagnosticMeta::default(),
            )],
            vec![ConnectionDef::new(
                ConnectionKey::from_u128(5),
                external_input.into(),
                not_input.into(),
                DiagnosticMeta::default(),
            )],
        );

        let graph = network
            .validate_structural()
            .artifact()
            .expect("the restricted graph is structurally valid")
            .reaction_dependencies();

        assert_eq!(
            graph.dependencies().collect::<Vec<_>>(),
            vec![
                ReactionDependency {
                    from: ReactionVertex::ExternalInput(external_input.into()),
                    to: ReactionVertex::NodeOperation(NodeKey::from_u128(10)),
                    subject: ReactionDependencySubject::Connection(ConnectionKey::from_u128(5)),
                },
                ReactionDependency {
                    from: ReactionVertex::NodeOperation(NodeKey::from_u128(10)),
                    to: ReactionVertex::NodeOutput(not_output.into()),
                    subject: ReactionDependencySubject::Node(NodeKey::from_u128(10)),
                },
                ReactionDependency {
                    from: ReactionVertex::NodeOutput(not_output.into()),
                    to: ReactionVertex::ExternalOutput(external_output.into()),
                    subject: ReactionDependencySubject::ExternalOutput(external_output.into()),
                },
            ]
        );
    }

    #[test]
    fn dependency_inspection_is_invariant_under_authored_claim_permutation() {
        let constant_output = OutPortKey::<Level>::from_u128(1);
        let not_input = InPortKey::<Level>::from_u128(2);
        let not_output = OutPortKey::<Level>::from_u128(3);
        let constant = NodeDef::new(
            NodeKey::from_u128(10),
            NodeKind::<()>::constant(LogicLevel::High),
            NodePorts::new(vec![], vec![constant_output.into()]),
            DiagnosticMeta::default(),
        );
        let inverter = NodeDef::new(
            NodeKey::from_u128(20),
            NodeKind::<()>::not(),
            NodePorts::new(vec![not_input.into()], vec![not_output.into()]),
            DiagnosticMeta::default(),
        );
        let connection = ConnectionDef::new(
            ConnectionKey::from_u128(5),
            constant_output.into(),
            not_input.into(),
            DiagnosticMeta::default(),
        );
        let output = ExternalOutputDef::new(
            ExternalOutputKey::<Level>::from_u128(4).into(),
            SignalSourceKey::NodeOutput(not_output).into(),
            DiagnosticMeta::default(),
        );
        let second_output = ExternalOutputDef::new(
            ExternalOutputKey::<Level>::from_u128(5).into(),
            SignalSourceKey::NodeOutput(constant_output).into(),
            DiagnosticMeta::default(),
        );

        let first = UncheckedNetwork::new(
            NetworkKey::from_u128(1),
            crate::identity::TimeDomainId::from_u128(1),
            DiagnosticMeta::default(),
            vec![constant.clone(), inverter.clone()],
            vec![],
            vec![output.clone(), second_output.clone()],
            vec![connection.clone()],
        );
        let second = UncheckedNetwork::new(
            NetworkKey::from_u128(1),
            crate::identity::TimeDomainId::from_u128(1),
            DiagnosticMeta::default(),
            vec![inverter, constant],
            vec![],
            vec![second_output, output],
            vec![connection],
        );

        let first_graph = first
            .validate_structural()
            .artifact()
            .expect("the first graph is structurally valid")
            .reaction_dependencies();
        let second_graph = second
            .validate_structural()
            .artifact()
            .expect("the second graph is structurally valid")
            .reaction_dependencies();

        assert_eq!(first_graph, second_graph);
    }

    #[test]
    fn accepts_a_structurally_valid_constant_and_not_graph() {
        let constant = OutPortKey::<Level>::from_u128(1);
        let input = InPortKey::<Level>::from_u128(2);
        let output = OutPortKey::<Level>::from_u128(3);
        let network = UncheckedNetwork::new(
            NetworkKey::from_u128(1),
            crate::identity::TimeDomainId::from_u128(1),
            DiagnosticMeta::default(),
            vec![
                NodeDef::new(
                    NodeKey::from_u128(1),
                    NodeKind::<()>::constant(LogicLevel::High),
                    NodePorts::new(vec![], vec![constant.into()]),
                    DiagnosticMeta::default(),
                ),
                NodeDef::new(
                    NodeKey::from_u128(2),
                    NodeKind::<()>::not(),
                    NodePorts::new(vec![input.into()], vec![output.into()]),
                    DiagnosticMeta::default(),
                ),
            ],
            vec![],
            vec![ExternalOutputDef::new(
                ExternalOutputKey::<Level>::from_u128(1).into(),
                SignalSourceKey::NodeOutput(output).into(),
                DiagnosticMeta::default(),
            )],
            vec![ConnectionDef::new(
                ConnectionKey::from_u128(1),
                constant.into(),
                input.into(),
                DiagnosticMeta::default(),
            )],
        );
        let report = network.validate_structural();
        assert!(report.artifact().is_some());
        assert!(report.diagnostics().is_empty());
    }

    #[test]
    fn external_outputs_reject_sources_of_the_other_signal_kind() {
        for output in [
            ExternalOutputDef::new(
                ExternalOutputKey::<Pulse>::from_u128(2).into(),
                SignalSourceKey::ExternalInput(ExternalInputKey::<Level>::from_u128(1)).into(),
                DiagnosticMeta::default(),
            ),
            ExternalOutputDef::new(
                ExternalOutputKey::<Level>::from_u128(2).into(),
                SignalSourceKey::ExternalInput(ExternalInputKey::<Pulse>::from_u128(1)).into(),
                DiagnosticMeta::default(),
            ),
        ] {
            let input = match output.source() {
                crate::key::AnySignalSourceKey::Level(SignalSourceKey::ExternalInput(key)) => {
                    ExternalInputDef::new(key.into(), DiagnosticMeta::default())
                }
                crate::key::AnySignalSourceKey::Pulse(SignalSourceKey::ExternalInput(key)) => {
                    ExternalInputDef::new(key.into(), DiagnosticMeta::default())
                }
                _ => panic!("fixture source must be an external input"),
            };
            let report = UncheckedNetwork::<()>::new(
                NetworkKey::from_u128(1),
                crate::identity::TimeDomainId::from_u128(1),
                DiagnosticMeta::default(),
                vec![],
                vec![input],
                vec![output],
                vec![],
            )
            .validate();

            assert!(report.artifact().is_none());
            assert!(report.diagnostics().iter().any(|diagnostic| {
                diagnostic.problem().code() == DiagnosticCode::ValidationSignalKindMismatch
            }));
        }
    }

    #[test]
    fn accumulates_independent_structural_errors_and_omits_candidate() {
        let output = OutPortKey::<Level>::from_u128(1);
        let input = InPortKey::<Level>::from_u128(2);
        let network = UncheckedNetwork::new(
            NetworkKey::from_u128(1),
            crate::identity::TimeDomainId::from_u128(1),
            DiagnosticMeta::default(),
            vec![
                NodeDef::new(
                    NodeKey::from_u128(1),
                    NodeKind::<()>::constant(LogicLevel::Low),
                    NodePorts::new(vec![input.into()], vec![output.into()]),
                    DiagnosticMeta::default(),
                ),
                NodeDef::new(
                    NodeKey::from_u128(2),
                    NodeKind::<()>::not(),
                    NodePorts::new(vec![], vec![]),
                    DiagnosticMeta::default(),
                ),
            ],
            vec![],
            vec![ExternalOutputDef::new(
                ExternalOutputKey::<Level>::from_u128(1).into(),
                SignalSourceKey::ExternalInput(crate::key::ExternalInputKey::<Level>::from_u128(
                    99,
                ))
                .into(),
                DiagnosticMeta::default(),
            )],
            vec![
                ConnectionDef::new(
                    ConnectionKey::from_u128(1),
                    OutPortKey::<Level>::from_u128(88).into(),
                    InPortKey::<Level>::from_u128(77).into(),
                    DiagnosticMeta::default(),
                ),
                ConnectionDef::new(
                    ConnectionKey::from_u128(2),
                    InPortKey::<Level>::from_u128(2).into(),
                    OutPortKey::<crate::signal::Pulse>::from_u128(3).into(),
                    DiagnosticMeta::default(),
                ),
            ],
        );
        let report = network.validate_structural();
        assert!(report.artifact().is_none());
        let codes: Vec<_> = report
            .diagnostics()
            .iter()
            .map(|diagnostic| diagnostic.problem().code())
            .collect();
        assert!(codes.contains(&DiagnosticCode::ValidationInvalidFixedArity));
        assert!(codes.contains(&DiagnosticCode::ValidationMissingRequiredInput));
        assert!(codes.contains(&DiagnosticCode::ValidationMissingPort));
        assert!(codes.contains(&DiagnosticCode::ValidationMissingEndpoint));
        assert!(codes.contains(&DiagnosticCode::ValidationInvalidDirection));
        assert!(codes.contains(&DiagnosticCode::ValidationSignalKindMismatch));
    }

    #[test]
    fn reports_duplicate_scopes_and_single_driver_without_losing_other_findings() {
        let source = OutPortKey::<Level>::from_u128(1);
        let target = InPortKey::<Level>::from_u128(2);
        let node = NodeDef::new(
            NodeKey::from_u128(1),
            NodeKind::<()>::constant(LogicLevel::High),
            NodePorts::new(vec![], vec![source.into()]),
            DiagnosticMeta::default(),
        );
        let target_node = NodeDef::new(
            NodeKey::from_u128(2),
            NodeKind::<()>::not(),
            NodePorts::new(
                vec![target.into()],
                vec![OutPortKey::<Level>::from_u128(3).into()],
            ),
            DiagnosticMeta::default(),
        );
        let network = UncheckedNetwork::new(
            NetworkKey::from_u128(1),
            crate::identity::TimeDomainId::from_u128(1),
            DiagnosticMeta::default(),
            vec![node.clone(), node, target_node],
            vec![],
            vec![],
            vec![
                ConnectionDef::new(
                    ConnectionKey::from_u128(1),
                    source.into(),
                    target.into(),
                    DiagnosticMeta::default(),
                ),
                ConnectionDef::new(
                    ConnectionKey::from_u128(2),
                    source.into(),
                    target.into(),
                    DiagnosticMeta::default(),
                ),
            ],
        );
        let report = network.validate_structural();
        let codes: Vec<_> = report
            .diagnostics()
            .iter()
            .map(|diagnostic| diagnostic.problem().code())
            .collect();
        assert!(codes.contains(&DiagnosticCode::ValidationDuplicateKey));
        assert!(codes.contains(&DiagnosticCode::ValidationUnsupportedMultipleDrivers));
    }

    #[test]
    fn diagnostics_are_invariant_under_connection_permutation() {
        let source = OutPortKey::<Level>::from_u128(1);
        let target = InPortKey::<Level>::from_u128(2);
        let nodes = vec![
            NodeDef::new(
                NodeKey::from_u128(1),
                NodeKind::<()>::constant(LogicLevel::High),
                NodePorts::new(vec![], vec![source.into()]),
                DiagnosticMeta::default(),
            ),
            NodeDef::new(
                NodeKey::from_u128(2),
                NodeKind::<()>::not(),
                NodePorts::new(
                    vec![target.into()],
                    vec![OutPortKey::<Level>::from_u128(3).into()],
                ),
                DiagnosticMeta::default(),
            ),
        ];
        let first = ConnectionDef::new(
            ConnectionKey::from_u128(1),
            source.into(),
            target.into(),
            DiagnosticMeta::default(),
        );
        let second = ConnectionDef::new(
            ConnectionKey::from_u128(2),
            source.into(),
            target.into(),
            DiagnosticMeta::default(),
        );
        let forward = UncheckedNetwork::new(
            NetworkKey::from_u128(1),
            crate::identity::TimeDomainId::from_u128(1),
            DiagnosticMeta::default(),
            nodes.clone(),
            vec![],
            vec![],
            vec![first.clone(), second.clone()],
        );
        let reverse = UncheckedNetwork::new(
            NetworkKey::from_u128(1),
            crate::identity::TimeDomainId::from_u128(1),
            DiagnosticMeta::default(),
            nodes,
            vec![],
            vec![],
            vec![second, first],
        );
        assert_eq!(
            forward.validate_structural().diagnostics(),
            reverse.validate_structural().diagnostics()
        );
    }

    #[test]
    fn reports_missing_endpoints_even_when_connection_direction_is_invalid() {
        let network = UncheckedNetwork::<()>::new(
            NetworkKey::from_u128(1),
            crate::identity::TimeDomainId::from_u128(1),
            DiagnosticMeta::default(),
            vec![],
            vec![],
            vec![],
            vec![ConnectionDef::new(
                ConnectionKey::from_u128(1),
                InPortKey::<Level>::from_u128(10).into(),
                OutPortKey::<Level>::from_u128(20).into(),
                DiagnosticMeta::default(),
            )],
        );

        let codes: Vec<_> = network
            .validate_structural()
            .diagnostics()
            .iter()
            .map(|diagnostic| diagnostic.problem().code())
            .collect();

        assert!(codes.contains(&DiagnosticCode::ValidationMissingPort));
        assert!(codes.contains(&DiagnosticCode::ValidationInvalidDirection));
    }

    #[test]
    fn validation_constructs_an_artifact_for_a_dag_and_rejects_a_two_node_cycle() {
        let a_input = InPortKey::<Level>::from_u128(1);
        let a_output = OutPortKey::<Level>::from_u128(2);
        let b_input = InPortKey::<Level>::from_u128(3);
        let b_output = OutPortKey::<Level>::from_u128(4);
        let cyclic_nodes = vec![
            NodeDef::new(
                NodeKey::from_u128(10),
                NodeKind::<()>::not(),
                NodePorts::new(vec![a_input.into()], vec![a_output.into()]),
                DiagnosticMeta::default(),
            ),
            NodeDef::new(
                NodeKey::from_u128(20),
                NodeKind::<()>::not(),
                NodePorts::new(vec![b_input.into()], vec![b_output.into()]),
                DiagnosticMeta::default(),
            ),
        ];
        let dag = UncheckedNetwork::new(
            NetworkKey::from_u128(1),
            crate::identity::TimeDomainId::from_u128(1),
            DiagnosticMeta::default(),
            vec![
                NodeDef::new(
                    NodeKey::from_u128(10),
                    NodeKind::<()>::constant(LogicLevel::High),
                    NodePorts::new(vec![], vec![a_output.into()]),
                    DiagnosticMeta::default(),
                ),
                NodeDef::new(
                    NodeKey::from_u128(20),
                    NodeKind::<()>::not(),
                    NodePorts::new(vec![b_input.into()], vec![b_output.into()]),
                    DiagnosticMeta::default(),
                ),
            ],
            vec![],
            vec![],
            vec![ConnectionDef::new(
                ConnectionKey::from_u128(1),
                a_output.into(),
                b_input.into(),
                DiagnosticMeta::default(),
            )],
        );
        assert!(dag.validate().artifact().is_some());

        let cyclic = UncheckedNetwork::new(
            NetworkKey::from_u128(1),
            crate::identity::TimeDomainId::from_u128(1),
            DiagnosticMeta::default(),
            cyclic_nodes,
            vec![],
            vec![],
            vec![
                ConnectionDef::new(
                    ConnectionKey::from_u128(1),
                    a_output.into(),
                    b_input.into(),
                    DiagnosticMeta::default(),
                ),
                ConnectionDef::new(
                    ConnectionKey::from_u128(2),
                    b_output.into(),
                    a_input.into(),
                    DiagnosticMeta::default(),
                ),
            ],
        );
        let report = cyclic.validate();
        assert!(report.artifact().is_none());
        assert_eq!(report.diagnostics().len(), 1);
        let evidence = report
            .diagnostics()
            .iter()
            .next()
            .map(|diagnostic| diagnostic.problem().evidence());
        match evidence {
            Some(ProblemEvidence::ValidationCurrentReactionCycle {
                members, witness, ..
            }) => {
                assert!(members.len() >= 4);
                assert!(!witness.is_empty());
                assert_eq!(
                    witness.first().map(|step| step.source),
                    witness.last().map(|step| step.target)
                );
            }
            _ => panic!("expected current-reaction cycle evidence"),
        }
    }

    #[test]
    fn self_loops_produce_one_diagnostic_per_cyclic_scc() {
        let a_input = InPortKey::<Level>::from_u128(1);
        let a_output = OutPortKey::<Level>::from_u128(2);
        let b_input = InPortKey::<Level>::from_u128(3);
        let b_output = OutPortKey::<Level>::from_u128(4);
        let network = UncheckedNetwork::new(
            NetworkKey::from_u128(1),
            crate::identity::TimeDomainId::from_u128(1),
            DiagnosticMeta::default(),
            vec![
                NodeDef::new(
                    NodeKey::from_u128(10),
                    NodeKind::<()>::not(),
                    NodePorts::new(vec![a_input.into()], vec![a_output.into()]),
                    DiagnosticMeta::default(),
                ),
                NodeDef::new(
                    NodeKey::from_u128(20),
                    NodeKind::<()>::not(),
                    NodePorts::new(vec![b_input.into()], vec![b_output.into()]),
                    DiagnosticMeta::default(),
                ),
            ],
            vec![],
            vec![],
            vec![
                ConnectionDef::new(
                    ConnectionKey::from_u128(1),
                    a_output.into(),
                    a_input.into(),
                    DiagnosticMeta::default(),
                ),
                ConnectionDef::new(
                    ConnectionKey::from_u128(2),
                    b_output.into(),
                    b_input.into(),
                    DiagnosticMeta::default(),
                ),
            ],
        );
        let report = network.validate();
        assert!(report.artifact().is_none());
        assert_eq!(report.diagnostics().len(), 2);
        assert!(report.diagnostics().iter().all(|diagnostic| {
            diagnostic.problem().code() == DiagnosticCode::ValidationCurrentReactionCycle
        }));
    }

    #[test]
    fn witness_ties_use_dependency_before_target_order() {
        let operation = |key| ReactionVertex::NodeOperation(NodeKey::from_u128(key));
        let output = |key| ReactionVertex::NodeOutput(OutPortKey::<Level>::from_u128(key).into());
        let connection = |from, to, key| ReactionDependency {
            from,
            to,
            subject: ReactionDependencySubject::Connection(ConnectionKey::from_u128(key)),
        };
        let node = |from, to, key| ReactionDependency {
            from,
            to,
            subject: ReactionDependencySubject::Node(NodeKey::from_u128(key)),
        };
        let start = node(operation(10), output(100), 10);
        let preferred = connection(output(100), operation(30), 1);
        let graph = ReactionDependencyGraph {
            vertices: [
                operation(10),
                operation(20),
                operation(30),
                output(100),
                output(200),
                output(300),
            ]
            .into_iter()
            .collect(),
            dependencies: [
                start,
                connection(output(100), operation(20), 9),
                node(operation(20), output(200), 20),
                connection(output(200), operation(10), 50),
                preferred,
                node(operation(30), output(300), 30),
                connection(output(300), operation(10), 60),
            ]
            .into_iter()
            .collect(),
        };
        let component = graph
            .cyclic_components()
            .into_iter()
            .next()
            .unwrap_or_else(|| panic!("the graph is one cyclic SCC"));

        let witness = graph.cycle_witness(&component);

        assert_eq!(witness[0], start);
        assert_eq!(witness[1], preferred);
    }

    #[test]
    fn bounded_cycle_detection_matches_a_reachability_oracle() {
        let vertices: Vec<_> = (0..3)
            .map(|key| ReactionVertex::NodeOperation(NodeKey::from_u128(key)))
            .collect();
        for mask in 0_u16..(1 << 9) {
            let dependencies: BTreeSet<_> = (0..3)
                .flat_map(|from| (0..3).map(move |to| (from, to)))
                .enumerate()
                .filter(|(bit, _)| mask & (1 << bit) != 0)
                .map(|(bit, (from, to))| ReactionDependency {
                    from: vertices[from],
                    to: vertices[to],
                    subject: ReactionDependencySubject::Connection(ConnectionKey::from_u128(
                        bit as u128,
                    )),
                })
                .collect();
            let graph = ReactionDependencyGraph {
                vertices: vertices.iter().copied().collect(),
                dependencies,
            };

            assert_eq!(graph.cyclic_components(), oracle_cyclic_components(&graph));
        }
    }

    #[test]
    fn witnesses_are_closed_real_and_confined_for_each_cyclic_scc() {
        let vertex = |key| ReactionVertex::NodeOperation(NodeKey::from_u128(key));
        let edge = |from, to, key| ReactionDependency {
            from: vertex(from),
            to: vertex(to),
            subject: ReactionDependencySubject::Connection(ConnectionKey::from_u128(key)),
        };
        let graph = ReactionDependencyGraph {
            vertices: (1..=4).map(vertex).collect(),
            dependencies: [
                edge(1, 1, 1),
                edge(1, 2, 2),
                edge(2, 1, 3),
                edge(2, 3, 4),
                edge(3, 2, 5),
                edge(4, 4, 6),
            ]
            .into_iter()
            .collect(),
        };

        let components = graph.cyclic_components();
        assert_eq!(components.len(), 2);
        assert_eq!(components[0].len(), 3);
        for component in components {
            let witness = graph.cycle_witness(&component);
            assert!(!witness.is_empty());
            assert_eq!(
                witness.first().map(|step| step.from),
                witness.last().map(|step| step.to)
            );
            assert!(witness.windows(2).all(|steps| steps[0].to == steps[1].from));
            assert!(witness.iter().all(|step| {
                graph.dependencies.contains(step)
                    && component.contains(&step.from)
                    && component.contains(&step.to)
            }));
        }
    }

    #[test]
    fn deterministic_topological_order_respects_every_dependency() {
        let vertex = |key| ReactionVertex::NodeOperation(NodeKey::from_u128(key));
        let edge = |from, to, key| ReactionDependency {
            from: vertex(from),
            to: vertex(to),
            subject: ReactionDependencySubject::Connection(ConnectionKey::from_u128(key)),
        };
        let dependencies = [edge(1, 3, 1), edge(2, 3, 2), edge(2, 4, 3)];
        let make_graph = |edges: &[ReactionDependency]| ReactionDependencyGraph {
            vertices: (1..=5).map(vertex).collect(),
            dependencies: edges.iter().copied().collect(),
        };
        let forward = make_graph(&dependencies);
        let reverse = make_graph(&dependencies.into_iter().rev().collect::<Vec<_>>());

        let order = forward.topological_order();
        assert_eq!(order, reverse.topological_order());
        assert_eq!(order.len(), forward.vertices.len());
        for dependency in &forward.dependencies {
            let source = order
                .iter()
                .position(|vertex| *vertex == dependency.from)
                .unwrap_or_else(|| panic!("source is present"));
            let target = order
                .iter()
                .position(|vertex| *vertex == dependency.to)
                .unwrap_or_else(|| panic!("target is present"));
            assert!(source < target);
        }
    }

    #[test]
    fn cycle_diagnostic_and_witness_ignore_authored_insertion_order() {
        let a_input = InPortKey::<Level>::from_u128(1);
        let a_output = OutPortKey::<Level>::from_u128(2);
        let b_input = InPortKey::<Level>::from_u128(3);
        let b_output = OutPortKey::<Level>::from_u128(4);
        let nodes = vec![
            NodeDef::new(
                NodeKey::from_u128(10),
                NodeKind::<()>::not(),
                NodePorts::new(vec![a_input.into()], vec![a_output.into()]),
                DiagnosticMeta::default(),
            ),
            NodeDef::new(
                NodeKey::from_u128(20),
                NodeKind::<()>::not(),
                NodePorts::new(vec![b_input.into()], vec![b_output.into()]),
                DiagnosticMeta::default(),
            ),
        ];
        let connections = vec![
            ConnectionDef::new(
                ConnectionKey::from_u128(9),
                a_output.into(),
                b_input.into(),
                DiagnosticMeta::default(),
            ),
            ConnectionDef::new(
                ConnectionKey::from_u128(1),
                b_output.into(),
                a_input.into(),
                DiagnosticMeta::default(),
            ),
        ];
        let network = |nodes, connections| {
            UncheckedNetwork::new(
                NetworkKey::from_u128(1),
                crate::identity::TimeDomainId::from_u128(1),
                DiagnosticMeta::default(),
                nodes,
                vec![],
                vec![],
                connections,
            )
        };
        let mut reversed_nodes = nodes.clone();
        reversed_nodes.reverse();
        let mut reversed_connections = connections.clone();
        reversed_connections.reverse();

        let forward = network(nodes, connections).validate();
        let reverse = network(reversed_nodes, reversed_connections).validate();

        assert_eq!(forward.diagnostics(), reverse.diagnostics());
        assert_eq!(forward.diagnostics().len(), 1);
    }

    fn oracle_cyclic_components(graph: &ReactionDependencyGraph) -> Vec<BTreeSet<ReactionVertex>> {
        let vertices: Vec<_> = graph.vertices().collect();
        let mut reachable = vec![vec![false; vertices.len()]; vertices.len()];
        for (index, row) in reachable.iter_mut().enumerate() {
            row[index] = true;
        }
        for edge in graph.dependencies() {
            let from = vertices
                .iter()
                .position(|vertex| *vertex == edge.from)
                .unwrap_or_else(|| panic!("edge source is a graph vertex"));
            let to = vertices
                .iter()
                .position(|vertex| *vertex == edge.to)
                .unwrap_or_else(|| panic!("edge target is a graph vertex"));
            reachable[from][to] = true;
        }
        for via in 0..vertices.len() {
            for from in 0..vertices.len() {
                for to in 0..vertices.len() {
                    reachable[from][to] |= reachable[from][via] && reachable[via][to];
                }
            }
        }
        let mut assigned: BTreeSet<ReactionVertex> = BTreeSet::new();
        let mut components = Vec::new();
        for (index, vertex) in vertices.iter().enumerate() {
            if assigned.contains(vertex) {
                continue;
            }
            let component: BTreeSet<_> = vertices
                .iter()
                .enumerate()
                .filter(|(other, _)| reachable[index][*other] && reachable[*other][index])
                .map(|(_, member)| *member)
                .collect();
            assigned.extend(component.iter().copied());
            let cyclic = component.len() > 1
                || graph
                    .dependencies()
                    .any(|edge| edge.from == *vertex && edge.to == *vertex);
            if cyclic {
                components.push(component);
            }
        }
        components.sort_by(|left, right| left.iter().cmp(right.iter()));
        components
    }

    #[test]
    fn duplicate_claim_evidence_retains_conflicting_node_facts() {
        let key = NodeKey::from_u128(1);
        let network = UncheckedNetwork::new(
            NetworkKey::from_u128(1),
            crate::identity::TimeDomainId::from_u128(1),
            DiagnosticMeta::default(),
            vec![
                NodeDef::new(
                    key,
                    NodeKind::<()>::constant(LogicLevel::High),
                    NodePorts::new(vec![], vec![OutPortKey::<Level>::from_u128(2).into()]),
                    DiagnosticMeta::default(),
                ),
                NodeDef::new(
                    key,
                    NodeKind::<()>::not(),
                    NodePorts::new(
                        vec![InPortKey::<Level>::from_u128(3).into()],
                        vec![OutPortKey::<Level>::from_u128(4).into()],
                    ),
                    DiagnosticMeta::default(),
                ),
            ],
            vec![],
            vec![],
            vec![],
        );

        let report = network.validate_structural();
        let duplicate = report
            .diagnostics()
            .iter()
            .find(|diagnostic| {
                diagnostic.problem().code() == DiagnosticCode::ValidationDuplicateKey
            })
            .unwrap_or_else(|| unreachable!("duplicate node key must be diagnosed"));
        match duplicate.problem().evidence() {
            ProblemEvidence::ValidationDuplicateKey { claims, .. } => {
                assert_eq!(claims.len(), 2);
                assert_ne!(claims[0], claims[1]);
            }
            _ => unreachable!("duplicate-key diagnostic has its registered evidence"),
        }
    }

    #[test]
    fn toggle_enforces_fixed_shape_and_retains_immediate_dependency() {
        let pulse = ExternalInputKey::<Pulse>::from_u128(1);
        let input = InPortKey::<Pulse>::from_u128(2);
        let output = OutPortKey::<Level>::from_u128(3);
        let node = NodeKey::from_u128(4);
        let definition = UncheckedNetwork::new(
            NetworkKey::from_u128(5),
            TimeDomainId::from_u128(6),
            DiagnosticMeta::default(),
            vec![NodeDef::new(
                node,
                NodeKind::<()>::toggle(LogicLevel::Low),
                NodePorts::with_input_roles(
                    vec![input.into()],
                    vec![InputPortRole::Toggle],
                    vec![output.into()],
                ),
                DiagnosticMeta::default(),
            )],
            vec![ExternalInputDef::new(
                pulse.into(),
                DiagnosticMeta::default(),
            )],
            Vec::new(),
            vec![ConnectionDef::new(
                ConnectionKey::from_u128(7),
                pulse.into(),
                input.into(),
                DiagnosticMeta::default(),
            )],
        );
        let structural = definition.validate_structural();
        let candidate = structural.artifact().unwrap();
        let graph = candidate.reaction_dependencies();
        assert!(graph.dependencies().any(|edge| {
            edge.from == ReactionVertex::ExternalInput(pulse.into())
                && edge.to == ReactionVertex::NodeOperation(node)
        }));

        let malformed = UncheckedNetwork::<()>::new(
            NetworkKey::from_u128(5),
            TimeDomainId::from_u128(6),
            DiagnosticMeta::default(),
            vec![NodeDef::new(
                node,
                NodeKind::toggle(LogicLevel::Low),
                NodePorts::new(
                    vec![InPortKey::<Level>::from_u128(2).into()],
                    vec![OutPortKey::<Pulse>::from_u128(3).into()],
                ),
                DiagnosticMeta::default(),
            )],
            Vec::new(),
            Vec::new(),
            Vec::new(),
        );
        assert!(malformed.validate_structural().artifact().is_none());
    }

    #[test]
    fn pulse_delay_enforces_fixed_shape_and_is_a_current_reaction_barrier() {
        let pulse = ExternalInputKey::<Pulse>::from_u128(1);
        let input = InPortKey::<Pulse>::from_u128(2);
        let output = OutPortKey::<Pulse>::from_u128(3);
        let node = NodeKey::from_u128(4);
        let delay = NonZeroSpan::from_ticks(5).unwrap();
        let definition = UncheckedNetwork::new(
            NetworkKey::from_u128(5),
            TimeDomainId::from_u128(6),
            DiagnosticMeta::default(),
            vec![NodeDef::new(
                node,
                NodeKind::pulse_delay(delay),
                NodePorts::with_input_roles(
                    vec![input.into()],
                    vec![InputPortRole::PulseDelay],
                    vec![output.into()],
                ),
                DiagnosticMeta::default(),
            )],
            vec![ExternalInputDef::new(
                pulse.into(),
                DiagnosticMeta::default(),
            )],
            Vec::new(),
            vec![ConnectionDef::new(
                ConnectionKey::from_u128(7),
                pulse.into(),
                input.into(),
                DiagnosticMeta::default(),
            )],
        );
        let structural = definition.validate_structural();
        let candidate = structural.artifact().unwrap();
        let graph = candidate.reaction_dependencies();
        assert!(!graph.dependencies().any(|edge| {
            edge.from == ReactionVertex::ExternalInput(pulse.into())
                && edge.to == ReactionVertex::NodeOperation(node)
        }));
        assert!(graph.dependencies().any(|edge| {
            edge.from == ReactionVertex::NodeOperation(node)
                && edge.to == ReactionVertex::NodeOutput(output.into())
        }));

        let malformed = UncheckedNetwork::<()>::new(
            NetworkKey::from_u128(5),
            TimeDomainId::from_u128(6),
            DiagnosticMeta::default(),
            vec![NodeDef::new(
                node,
                NodeKind::pulse_delay(delay),
                NodePorts::new(
                    vec![InPortKey::<Level>::from_u128(2).into()],
                    vec![OutPortKey::<Level>::from_u128(3).into()],
                ),
                DiagnosticMeta::default(),
            )],
            Vec::new(),
            Vec::new(),
            Vec::new(),
        );
        assert!(malformed.validate_structural().artifact().is_none());

        let delayed_feedback = UncheckedNetwork::new(
            NetworkKey::from_u128(15),
            TimeDomainId::from_u128(16),
            DiagnosticMeta::default(),
            vec![NodeDef::new(
                node,
                NodeKind::pulse_delay(delay),
                NodePorts::with_input_roles(
                    vec![input.into()],
                    vec![InputPortRole::PulseDelay],
                    vec![output.into()],
                ),
                DiagnosticMeta::default(),
            )],
            Vec::new(),
            vec![ExternalOutputDef::new(
                ExternalOutputKey::<Pulse>::from_u128(8).into(),
                SignalSourceKey::NodeOutput(output).into(),
                DiagnosticMeta::default(),
            )],
            vec![ConnectionDef::new(
                ConnectionKey::from_u128(9),
                output.into(),
                input.into(),
                DiagnosticMeta::default(),
            )],
        );
        assert!(
            delayed_feedback.validate().artifact().is_some(),
            "a strictly-future feedback edge is not a current-reaction cycle"
        );
    }
}
