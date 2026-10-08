//! Stable structural navigation over retained definitions.

use crate::authored::{ConnectionEndpoint, ModuleInstanceDef, ModuleInterfaceMapping, NodeDef};
use crate::diagnostics::{
    DiagnosticCode, InspectionEvidence, InspectionSubjectKind, Problem, ProblemEvidence,
    Responsibility, Severity, SubjectRef,
};
use crate::key::*;
use crate::{CompiledNetwork, NetworkFingerprint, NodeSubject};
use core::marker::PhantomData;
use std::collections::{BTreeMap, BTreeSet};

/// A local structural subject. A containing module path qualifies local keys.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum GraphElement {
    Node(NodeKey),
    InPort(AnyInPortKey),
    OutPort(AnyOutPortKey),
    Connection(ConnectionKey),
    ExternalInput(AnyExternalInputKey),
    ExternalOutput(AnyExternalOutputKey),
    Module(ModuleInstanceKey),
    ModuleInput(AnyModuleInputKey),
    ModuleOutput(AnyModuleOutputKey),
}

/// A stable graph identity, qualified by its outermost-to-innermost owner path.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct GraphSubjectRef {
    instances: Vec<ModuleInstanceKey>,
    element: GraphElement,
}

impl GraphSubjectRef {
    /// Refers to a subject in the containing network.
    #[must_use]
    pub fn root(element: GraphElement) -> Self {
        Self::qualified(Vec::new(), element)
    }
    /// Refers to a local subject inside the supplied complete owner path.
    /// Membership is checked by queries, so this also represents absent subjects.
    #[must_use]
    pub fn qualified(instances: Vec<ModuleInstanceKey>, element: GraphElement) -> Self {
        Self { instances, element }
    }
    /// Returns the containing module path.
    #[must_use]
    pub fn instances(&self) -> &[ModuleInstanceKey] {
        &self.instances
    }
    /// Returns the local subject.
    #[must_use]
    pub const fn element(&self) -> GraphElement {
        self.element
    }
    pub(crate) fn node(subject: &NodeSubject) -> Self {
        match subject {
            NodeSubject::Node(node) => Self::root(GraphElement::Node(*node)),
            NodeSubject::Qualified(node) => {
                Self::qualified(node.instances().to_vec(), GraphElement::Node(node.node()))
            }
        }
    }
    pub(crate) fn diagnostic_subject(&self) -> SubjectRef {
        match self.element {
            GraphElement::Node(node) if !self.instances.is_empty() => SubjectRef::QualifiedNode(
                crate::QualifiedNodeRef::new(self.instances.clone(), node),
            ),
            GraphElement::Node(node) => SubjectRef::Node(node),
            GraphElement::InPort(port) => SubjectRef::InPort(port),
            GraphElement::OutPort(port) => SubjectRef::OutPort(port),
            GraphElement::Connection(key) => SubjectRef::Connection(key),
            GraphElement::ExternalInput(key) => SubjectRef::ExternalInput(key),
            GraphElement::ExternalOutput(key) => SubjectRef::ExternalOutput(key),
            GraphElement::Module(key) => SubjectRef::ModuleInstance(key),
            GraphElement::ModuleInput(key) => SubjectRef::ModuleInput(key),
            GraphElement::ModuleOutput(key) => SubjectRef::ModuleOutput(key),
        }
    }
    pub(crate) fn inspection_evidence(&self) -> InspectionEvidence {
        InspectionEvidence {
            requested: self.diagnostic_subject(),
            qualified_path: self.instances.clone(),
            expected: InspectionSubjectKind::GraphElement(self.element),
            actual: None,
        }
    }
}

/// A membership-derived region identity, used with fingerprint and machine revision.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct RegionId([u8; 32]);
impl RegionId {
    /// Returns the opaque membership digest.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// An owned weak structural component. It owns no execution state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Region {
    id: RegionId,
    fingerprint: NetworkFingerprint,
    subjects: Vec<GraphSubjectRef>,
}
impl Region {
    /// Returns the membership-derived identity.
    #[must_use]
    pub const fn id(&self) -> RegionId {
        self.id
    }
    /// Returns the topology binding. Machine observations also identify revision.
    #[must_use]
    pub const fn fingerprint(&self) -> NetworkFingerprint {
        self.fingerprint
    }
    /// Returns every member in deterministic stable-subject order.
    #[must_use]
    pub fn subjects(&self) -> &[GraphSubjectRef] {
        &self.subjects
    }
}

/// An owned conservative directed slice, with no claim of current activation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NetworkSlice {
    fingerprint: NetworkFingerprint,
    subjects: Vec<GraphSubjectRef>,
}
impl NetworkSlice {
    /// Returns the topology binding.
    #[must_use]
    pub const fn fingerprint(&self) -> NetworkFingerprint {
        self.fingerprint
    }
    /// Returns the slice in deterministic stable-subject order.
    #[must_use]
    pub fn subjects(&self) -> &[GraphSubjectRef] {
        &self.subjects
    }
}

/// A graph request naming a subject absent from the retained topology.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GraphQueryFailure {
    UnknownSubject(GraphSubjectRef),
}
impl GraphQueryFailure {
    /// Returns the catalogue code.
    #[must_use]
    pub const fn code(&self) -> DiagnosticCode {
        DiagnosticCode::InspectionUnknownSubject
    }
    /// Returns fixed catalogue severity.
    #[must_use]
    pub const fn severity(&self) -> Severity {
        self.code().severity()
    }
    /// Returns fixed catalogue responsibility.
    #[must_use]
    pub const fn responsibility(&self) -> Responsibility {
        self.code().responsibility()
    }
    /// Returns structured stable-subject evidence, retaining qualification.
    #[must_use]
    pub fn problem<D>(&self) -> Problem<D> {
        let Self::UnknownSubject(subject) = self;
        let requested = subject.diagnostic_subject();
        Problem::new(
            requested.clone(),
            Vec::new(),
            ProblemEvidence::InspectionUnknownSubject {
                evidence: subject.inspection_evidence(),
                marker: PhantomData,
            },
        )
    }
}

#[derive(Default)]
struct Navigation {
    forward: BTreeMap<GraphSubjectRef, BTreeSet<GraphSubjectRef>>,
    backward: BTreeMap<GraphSubjectRef, BTreeSet<GraphSubjectRef>>,
    weak: BTreeMap<GraphSubjectRef, BTreeSet<GraphSubjectRef>>,
}
impl Navigation {
    fn subject(&mut self, subject: GraphSubjectRef) {
        self.forward.entry(subject.clone()).or_default();
        self.backward.entry(subject.clone()).or_default();
        self.weak.entry(subject).or_default();
    }
    fn contain(&mut self, from: GraphSubjectRef, to: GraphSubjectRef) {
        self.subject(from.clone());
        self.subject(to.clone());
        self.weak
            .entry(from.clone())
            .or_default()
            .insert(to.clone());
        self.weak.entry(to).or_default().insert(from);
    }
    fn edge(&mut self, from: GraphSubjectRef, to: GraphSubjectRef) {
        self.contain(from.clone(), to.clone());
        self.forward
            .entry(from.clone())
            .or_default()
            .insert(to.clone());
        self.backward.entry(to).or_default().insert(from);
    }
    fn nodes<D>(&mut self, path: &[ModuleInstanceKey], nodes: &[NodeDef<D>]) {
        for node in nodes {
            let owner = GraphSubjectRef::qualified(path.to_vec(), GraphElement::Node(node.key()));
            self.subject(owner.clone());
            for port in node.ports().inputs() {
                self.edge(
                    GraphSubjectRef::qualified(path.to_vec(), GraphElement::InPort(*port)),
                    owner.clone(),
                );
            }
            // SPEC: docs/specs/contracts/structural-graph-navigation.yaml "structural-possibility"
            // Include temporal paths here; same-reaction causality is a separate graph.
            for port in node.ports().outputs() {
                self.edge(
                    owner.clone(),
                    GraphSubjectRef::qualified(path.to_vec(), GraphElement::OutPort(*port)),
                );
            }
        }
    }
    fn endpoint(
        path: &[ModuleInstanceKey],
        endpoint: ConnectionEndpoint,
        instances: &[ModuleInstanceDef<impl Sized>],
    ) -> GraphSubjectRef {
        match endpoint {
            ConnectionEndpoint::ExternalInput(key) => {
                GraphSubjectRef::root(GraphElement::ExternalInput(key))
            }
            ConnectionEndpoint::ExternalOutput(key) => {
                GraphSubjectRef::root(GraphElement::ExternalOutput(key))
            }
            ConnectionEndpoint::NodeInput(key) => {
                GraphSubjectRef::qualified(path.to_vec(), GraphElement::InPort(key))
            }
            ConnectionEndpoint::NodeOutput(key) => {
                GraphSubjectRef::qualified(path.to_vec(), GraphElement::OutPort(key))
            }
            ConnectionEndpoint::ModuleInput(key) => {
                GraphSubjectRef::qualified(path.to_vec(), GraphElement::ModuleInput(key))
            }
            ConnectionEndpoint::ModuleOutput { instance, output } => GraphSubjectRef::qualified(
                instance_owner_path(path, instance, instances),
                GraphElement::ModuleOutput(output),
            ),
        }
    }
    fn connections<D>(
        &mut self,
        path: &[ModuleInstanceKey],
        connections: &[crate::authored::ConnectionDef],
        instances: &[ModuleInstanceDef<D>],
    ) {
        for connection in connections {
            let identity = GraphSubjectRef::qualified(
                path.to_vec(),
                GraphElement::Connection(connection.key()),
            );
            self.edge(
                Self::endpoint(path, connection.from(), instances),
                identity.clone(),
            );
            self.edge(identity, Self::endpoint(path, connection.to(), instances));
        }
    }
    fn modules<D>(&mut self, path: &[ModuleInstanceKey], instances: &[ModuleInstanceDef<D>]) {
        for instance in instances {
            let child = instance_owner_path(path, instance.key(), instances);
            let module = module_subject(&child);
            self.subject(module.clone());
            let definition = instance.module().definition();
            self.nodes(&child, definition.nodes());
            self.connections(
                &child,
                definition.connections(),
                definition.module_instances(),
            );
            for node in definition.nodes() {
                self.contain(
                    module.clone(),
                    GraphSubjectRef::qualified(child.clone(), GraphElement::Node(node.key())),
                );
            }
            for input in instance.module().inputs() {
                self.contain(
                    module.clone(),
                    GraphSubjectRef::qualified(
                        child.clone(),
                        GraphElement::ModuleInput(input.key()),
                    ),
                );
            }
            for output in instance.module().outputs() {
                self.contain(
                    module.clone(),
                    GraphSubjectRef::qualified(
                        child.clone(),
                        GraphElement::ModuleOutput(output.key()),
                    ),
                );
            }
            for binding in instance.bindings().bindings() {
                self.edge(
                    Self::endpoint(path, binding.source(), instances),
                    GraphSubjectRef::qualified(
                        child.clone(),
                        GraphElement::ModuleInput(binding.input()),
                    ),
                );
            }
            for mapping in definition.mappings() {
                match mapping {
                    ModuleInterfaceMapping::Input { input, target } => self.edge(
                        GraphSubjectRef::qualified(
                            child.clone(),
                            GraphElement::ModuleInput(*input),
                        ),
                        Self::endpoint(&child, *target, definition.module_instances()),
                    ),
                    ModuleInterfaceMapping::Output { output, source } => self.edge(
                        Self::endpoint(&child, *source, definition.module_instances()),
                        GraphSubjectRef::qualified(
                            child.clone(),
                            GraphElement::ModuleOutput(*output),
                        ),
                    ),
                }
            }
            self.modules(&child, definition.module_instances());
            for nested in definition.module_instances() {
                self.contain(
                    module.clone(),
                    module_subject(&instance_owner_path(
                        &child,
                        nested.key(),
                        definition.module_instances(),
                    )),
                );
            }
        }
    }
    fn new<D>(compiled: &CompiledNetwork<D>) -> Self {
        let graph = compiled.definition();
        let mut result = Self::default();
        result.nodes(&[], graph.nodes());
        result.connections(&[], graph.connections(), graph.module_instances());
        for input in graph.external_inputs() {
            result.subject(GraphSubjectRef::root(GraphElement::ExternalInput(
                input.key(),
            )));
        }
        for output in graph.external_outputs() {
            let source = match output.source() {
                AnySignalSourceKey::Level(source) => source_endpoint(source),
                AnySignalSourceKey::Pulse(source) => source_endpoint(source),
            };
            result.edge(
                Self::endpoint(&[], source, graph.module_instances()),
                GraphSubjectRef::root(GraphElement::ExternalOutput(output.key())),
            );
        }
        result.modules(&[], graph.module_instances());
        for instance in graph.module_instances() {
            if let Some(parent) = instance.parent() {
                result.contain(
                    module_subject(&instance_owner_path(&[], parent, graph.module_instances())),
                    module_subject(&instance_owner_path(
                        &[],
                        instance.key(),
                        graph.module_instances(),
                    )),
                );
            }
        }
        result
    }
    fn check(&self, subject: &GraphSubjectRef) -> Result<(), GraphQueryFailure> {
        if self.forward.contains_key(subject) {
            Ok(())
        } else {
            Err(GraphQueryFailure::UnknownSubject(subject.clone()))
        }
    }
}
fn module_subject(path: &[ModuleInstanceKey]) -> GraphSubjectRef {
    let Some((key, parent)) = path.split_last() else {
        panic!("compiled module paths must be non-empty");
    };
    GraphSubjectRef::qualified(parent.to_vec(), GraphElement::Module(*key))
}
fn instance_owner_path<D>(
    prefix: &[ModuleInstanceKey],
    instance: ModuleInstanceKey,
    instances: &[ModuleInstanceDef<D>],
) -> Vec<ModuleInstanceKey> {
    let mut path = vec![instance];
    let mut current = instances
        .iter()
        .find(|item| item.key() == instance)
        .and_then(ModuleInstanceDef::parent);
    while let Some(parent) = current {
        path.push(parent);
        current = instances
            .iter()
            .find(|item| item.key() == parent)
            .and_then(ModuleInstanceDef::parent);
    }
    path.reverse();
    [prefix, path.as_slice()].concat()
}
fn source_endpoint<S: crate::signal::SignalType>(source: SignalSourceKey<S>) -> ConnectionEndpoint
where
    AnyExternalInputKey: From<ExternalInputKey<S>>,
    AnyOutPortKey: From<OutPortKey<S>>,
    AnyModuleOutputKey: From<ModuleOutputKey<S>>,
{
    match source {
        SignalSourceKey::ExternalInput(key) => ConnectionEndpoint::ExternalInput(key.into()),
        SignalSourceKey::NodeOutput(key) => ConnectionEndpoint::NodeOutput(key.into()),
        SignalSourceKey::ModuleOutput { instance, output } => ConnectionEndpoint::ModuleOutput {
            instance,
            output: output.into(),
        },
    }
}
fn closure(
    adjacency: &BTreeMap<GraphSubjectRef, BTreeSet<GraphSubjectRef>>,
    root: &GraphSubjectRef,
) -> BTreeSet<GraphSubjectRef> {
    let mut seen = BTreeSet::new();
    let mut work = vec![root.clone()];
    while let Some(subject) = work.pop() {
        if seen.insert(subject.clone()) {
            if let Some(next) = adjacency.get(&subject) {
                work.extend(next.iter().cloned());
            }
        }
    }
    seen
}
fn qualified_members(mut subjects: BTreeSet<GraphSubjectRef>) -> Vec<GraphSubjectRef> {
    let paths: Vec<_> = subjects
        .iter()
        .map(|subject| subject.instances.clone())
        .collect();
    for path in paths {
        for end in 1..=path.len() {
            subjects.insert(module_subject(&path[..end]));
        }
    }
    subjects.into_iter().collect()
}
fn query_closure(
    adjacency: &BTreeMap<GraphSubjectRef, BTreeSet<GraphSubjectRef>>,
    subject: &GraphSubjectRef,
) -> BTreeSet<GraphSubjectRef> {
    let mut result = closure(adjacency, subject);
    if let GraphElement::Module(key) = subject.element {
        let mut path = subject.instances.clone();
        path.push(key);
        for member in adjacency
            .keys()
            .filter(|member| member.instances.starts_with(&path))
        {
            result.extend(closure(adjacency, member));
        }
    }
    result
}
impl<D> CompiledNetwork<D> {
    /// Derives all weak structural regions in stable membership order.
    #[must_use]
    pub fn regions(&self) -> Vec<Region> {
        let navigation = Navigation::new(self);
        let mut remaining: BTreeSet<_> = navigation.weak.keys().cloned().collect();
        let mut regions = Vec::new();
        while let Some(first) = remaining.first().cloned() {
            let members = closure(&navigation.weak, &first);
            remaining.retain(|subject| !members.contains(subject));
            let subjects: Vec<_> = members.into_iter().collect();
            let id = region_id(&subjects);
            regions.push(Region {
                id,
                fingerprint: self.fingerprint(),
                subjects,
            });
        }
        regions
    }
    /// Finds a weak component containing an existing stable structural subject.
    pub fn region_containing_subject(
        &self,
        subject: &GraphSubjectRef,
    ) -> Result<Region, GraphQueryFailure> {
        Navigation::new(self).check(subject)?;
        self.regions()
            .into_iter()
            .find(|region| region.subjects.binary_search(subject).is_ok())
            .ok_or_else(|| GraphQueryFailure::UnknownSubject(subject.clone()))
    }
    /// Finds the weak component of a containing-network node.
    pub fn region_containing(&self, node: NodeKey) -> Result<Region, GraphQueryFailure> {
        self.region_containing_subject(&GraphSubjectRef::root(GraphElement::Node(node)))
    }
    /// Finds the weak component of an external output, including node-free wiring.
    pub fn region_containing_output(
        &self,
        output: AnyExternalOutputKey,
    ) -> Result<Region, GraphQueryFailure> {
        self.region_containing_subject(&GraphSubjectRef::root(GraphElement::ExternalOutput(output)))
    }
    /// Returns structural subjects that could eventually affect an output.
    pub fn slice_affecting(
        &self,
        output: AnyExternalOutputKey,
    ) -> Result<NetworkSlice, GraphQueryFailure> {
        self.slice_upstream(&GraphSubjectRef::root(GraphElement::ExternalOutput(output)))
    }
    /// Returns structural subjects an external input could eventually affect.
    pub fn slice_affected_by(
        &self,
        input: AnyExternalInputKey,
    ) -> Result<NetworkSlice, GraphQueryFailure> {
        self.slice_downstream(&GraphSubjectRef::root(GraphElement::ExternalInput(input)))
    }
    /// Returns all conservative upstream subjects of an existing subject.
    pub fn slice_upstream(
        &self,
        subject: &GraphSubjectRef,
    ) -> Result<NetworkSlice, GraphQueryFailure> {
        let navigation = Navigation::new(self);
        navigation.check(subject)?;
        Ok(NetworkSlice {
            fingerprint: self.fingerprint(),
            subjects: qualified_members(query_closure(&navigation.backward, subject)),
        })
    }
    /// Returns all conservative downstream subjects of an existing subject.
    pub fn slice_downstream(
        &self,
        subject: &GraphSubjectRef,
    ) -> Result<NetworkSlice, GraphQueryFailure> {
        let navigation = Navigation::new(self);
        navigation.check(subject)?;
        Ok(NetworkSlice {
            fingerprint: self.fingerprint(),
            subjects: qualified_members(query_closure(&navigation.forward, subject)),
        })
    }
    /// Returns subjects lying on directed structural walks between two existing subjects.
    /// An unreachable destination produces an empty slice. Structural cycles terminate.
    pub fn slice_between(
        &self,
        source: &GraphSubjectRef,
        destination: &GraphSubjectRef,
    ) -> Result<NetworkSlice, GraphQueryFailure> {
        let navigation = Navigation::new(self);
        navigation.check(source)?;
        navigation.check(destination)?;
        let from = query_closure(&navigation.forward, source);
        let to = query_closure(&navigation.backward, destination);
        Ok(NetworkSlice {
            fingerprint: self.fingerprint(),
            subjects: qualified_members(from.intersection(&to).cloned().collect()),
        })
    }
}

fn region_id(subjects: &[GraphSubjectRef]) -> RegionId {
    let mut hash = blake3::Hasher::new();
    hash.update(b"mossignal.structural-region.v1\0");
    for subject in subjects {
        hash.update(&(subject.instances.len() as u64).to_be_bytes());
        for instance in &subject.instances {
            hash.update(&instance.as_u128().to_be_bytes());
        }
        let (tag, kind, key) = match subject.element {
            GraphElement::Node(key) => (0, 0, key.as_u128()),
            GraphElement::InPort(AnyInPortKey::Level(key)) => (1, 0, key.as_u128()),
            GraphElement::InPort(AnyInPortKey::Pulse(key)) => (1, 1, key.as_u128()),
            GraphElement::OutPort(AnyOutPortKey::Level(key)) => (2, 0, key.as_u128()),
            GraphElement::OutPort(AnyOutPortKey::Pulse(key)) => (2, 1, key.as_u128()),
            GraphElement::Connection(key) => (3, 0, key.as_u128()),
            GraphElement::ExternalInput(AnyExternalInputKey::Level(key)) => (4, 0, key.as_u128()),
            GraphElement::ExternalInput(AnyExternalInputKey::Pulse(key)) => (4, 1, key.as_u128()),
            GraphElement::ExternalOutput(AnyExternalOutputKey::Level(key)) => (5, 0, key.as_u128()),
            GraphElement::ExternalOutput(AnyExternalOutputKey::Pulse(key)) => (5, 1, key.as_u128()),
            GraphElement::Module(key) => (6, 0, key.as_u128()),
            GraphElement::ModuleInput(AnyModuleInputKey::Level(key)) => (7, 0, key.as_u128()),
            GraphElement::ModuleInput(AnyModuleInputKey::Pulse(key)) => (7, 1, key.as_u128()),
            GraphElement::ModuleOutput(AnyModuleOutputKey::Level(key)) => (8, 0, key.as_u128()),
            GraphElement::ModuleOutput(AnyModuleOutputKey::Pulse(key)) => (8, 1, key.as_u128()),
        };
        hash.update(&[tag, kind]);
        hash.update(&key.to_be_bytes());
    }
    RegionId(*hash.finalize().as_bytes())
}
