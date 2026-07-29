//! Validation and immutable inspection for reusable user modules.

use crate::authored::{
    ConnectionDef, ConnectionEndpoint, ModuleInputDef, ModuleInterfaceMapping, ModuleOutputDef,
    NodeDef, UncheckedModule, UncheckedNetwork,
};
use crate::diagnostics::{
    Diagnostic, DiagnosticSet, DuplicateClaim, Problem, ProblemEvidence, Report, SubjectRef,
};
use crate::identity::{ModuleFingerprint, TimeDomainId, module_fingerprint};
use crate::key::{AnyInPortKey, AnyModuleInputKey, AnyModuleOutputKey, AnyOutPortKey, NetworkKey};
use crate::metadata::DiagnosticMeta;
use crate::signal::SignalKind;
use crate::validation::{ReactionDependencyGraph, cycle_step, reaction_member};
use core::fmt;
use core::marker::PhantomData;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

/// The semantic origin of one validated reusable module.
#[non_exhaustive]
pub enum ModuleOrigin<D> {
    /// A caller-authored reusable module.
    User,
    #[doc(hidden)]
    __Domain(PhantomData<fn() -> D>, core::convert::Infallible),
}

impl<D> Clone for ModuleOrigin<D> {
    fn clone(&self) -> Self {
        match self {
            Self::User => Self::User,
            Self::__Domain(_, never) => match *never {},
        }
    }
}

impl<D> fmt::Debug for ModuleOrigin<D> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::User => formatter.write_str("User"),
            Self::__Domain(_, never) => match *never {},
        }
    }
}

impl<D> PartialEq for ModuleOrigin<D> {
    fn eq(&self, other: &Self) -> bool {
        matches!((self, other), (Self::User, Self::User))
    }
}

impl<D> Eq for ModuleOrigin<D> {}

struct ModuleData<D> {
    origin: ModuleOrigin<D>,
    fingerprint: ModuleFingerprint,
    definition: UncheckedModule<D>,
}

/// An immutable validated reusable user-module definition.
pub struct ModuleDef<D> {
    data: Arc<ModuleData<D>>,
}

impl<D> Clone for ModuleDef<D> {
    fn clone(&self) -> Self {
        Self {
            data: Arc::clone(&self.data),
        }
    }
}

impl<D> fmt::Debug for ModuleDef<D> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ModuleDef")
            .field("origin", &self.data.origin)
            .field("fingerprint", &self.data.fingerprint)
            .field("definition", &self.data.definition)
            .finish()
    }
}

impl<D> ModuleDef<D> {
    /// Returns the canonical semantic identity of this reusable module.
    #[must_use]
    pub fn fingerprint(&self) -> ModuleFingerprint {
        self.data.fingerprint
    }

    /// Returns the explicit semantic origin of this module.
    #[must_use]
    pub fn origin(&self) -> &ModuleOrigin<D> {
        &self.data.origin
    }

    /// Returns presentation metadata retained from the authored module.
    #[must_use]
    pub fn meta(&self) -> &DiagnosticMeta {
        self.data.definition.meta()
    }

    /// Iterates over the typed public inputs in canonical stable-key order.
    #[must_use]
    pub fn inputs(&self) -> ModuleInputIter<'_> {
        ModuleInputIter {
            inner: self.data.definition.inputs().iter(),
        }
    }

    /// Iterates over the typed public outputs in canonical stable-key order.
    #[must_use]
    pub fn outputs(&self) -> ModuleOutputIter<'_> {
        ModuleOutputIter {
            inner: self.data.definition.outputs().iter(),
        }
    }

    /// Borrows the immutable validated definition graph.
    #[must_use]
    pub fn graph(&self) -> DefinitionGraphView<'_, D> {
        DefinitionGraphView {
            definition: &self.data.definition,
        }
    }
}

/// Canonical iterator over public module-input definitions.
pub struct ModuleInputIter<'a> {
    inner: core::slice::Iter<'a, ModuleInputDef>,
}

impl<'a> Iterator for ModuleInputIter<'a> {
    type Item = &'a ModuleInputDef;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}

impl ExactSizeIterator for ModuleInputIter<'_> {}

/// Canonical iterator over public module-output definitions.
pub struct ModuleOutputIter<'a> {
    inner: core::slice::Iter<'a, ModuleOutputDef>,
}

impl<'a> Iterator for ModuleOutputIter<'a> {
    type Item = &'a ModuleOutputDef;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}

impl ExactSizeIterator for ModuleOutputIter<'_> {}

/// Immutable read-only view of one validated module definition graph.
#[derive(Clone, Copy)]
pub struct DefinitionGraphView<'a, D> {
    definition: &'a UncheckedModule<D>,
}

impl<'a, D> DefinitionGraphView<'a, D> {
    /// Returns module-local primitive nodes in canonical stable-key order.
    #[must_use]
    pub fn nodes(self) -> &'a [NodeDef<D>] {
        self.definition.nodes()
    }

    /// Returns module-local connections in canonical stable-key order.
    #[must_use]
    pub fn connections(self) -> &'a [ConnectionDef] {
        self.definition.connections()
    }

    /// Returns public-interface incidence in canonical semantic order.
    #[must_use]
    pub fn mappings(self) -> &'a [ModuleInterfaceMapping] {
        self.definition.mappings()
    }
}

impl<D> UncheckedModule<D> {
    /// Consumes and validates this reusable user-module definition.
    #[must_use]
    pub fn validate(self) -> Report<ModuleDef<D>, D>
    where
        D: PartialEq,
    {
        validate_module(&self)
    }

    /// Validates this reusable user-module definition without consuming it.
    #[must_use]
    pub fn validate_ref(&self) -> Report<ModuleDef<D>, D>
    where
        D: PartialEq,
    {
        validate_module(self)
    }
}

fn validate_module<D: PartialEq>(module: &UncheckedModule<D>) -> Report<ModuleDef<D>, D> {
    let mut diagnostics = DiagnosticSet::new();
    let mut inputs = BTreeMap::<AnyModuleInputKey, Vec<&ModuleInputDef>>::new();
    let mut outputs = BTreeMap::<AnyModuleOutputKey, Vec<&ModuleOutputDef>>::new();
    let mut in_ports = BTreeSet::new();
    let mut out_ports = BTreeSet::new();

    for input in module.inputs() {
        inputs.entry(input.key()).or_default().push(input);
    }
    for output in module.outputs() {
        outputs.entry(output.key()).or_default().push(output);
    }
    for node in module.nodes() {
        in_ports.extend(node.ports().inputs().iter().copied());
        out_ports.extend(node.ports().outputs().iter().copied());
    }
    for (key, claims) in inputs.iter().filter(|(_, claims)| claims.len() > 1) {
        add(
            &mut diagnostics,
            SubjectRef::ModuleInput(*key),
            ProblemEvidence::duplicate_key(
                SubjectRef::ModuleInput(*key),
                claims
                    .iter()
                    .map(|claim| DuplicateClaim::ModuleInput {
                        key: *key,
                        origin: claim.meta().origin.clone(),
                    })
                    .collect(),
            ),
        );
    }
    for (key, claims) in outputs.iter().filter(|(_, claims)| claims.len() > 1) {
        add(
            &mut diagnostics,
            SubjectRef::ModuleOutput(*key),
            ProblemEvidence::duplicate_key(
                SubjectRef::ModuleOutput(*key),
                claims
                    .iter()
                    .map(|claim| DuplicateClaim::ModuleOutput {
                        key: *key,
                        origin: claim.meta().origin.clone(),
                    })
                    .collect(),
            ),
        );
    }

    let mut boundary_drivers = BTreeSet::new();
    let mut drivers = BTreeMap::<AnyInPortKey, Vec<SubjectRef>>::new();
    let mut output_mappings = BTreeMap::<AnyModuleOutputKey, Vec<SubjectRef>>::new();

    for mapping in module.mappings() {
        match *mapping {
            ModuleInterfaceMapping::Input { input, target } => {
                let primary = SubjectRef::ModuleInput(input);
                if !inputs.contains_key(&input) {
                    add(
                        &mut diagnostics,
                        primary,
                        ProblemEvidence::missing_endpoint(primary, input.kind()),
                    );
                }
                let ConnectionEndpoint::NodeInput(target_key) = target else {
                    add(
                        &mut diagnostics,
                        primary,
                        ProblemEvidence::invalid_direction(primary, endpoint_subject(target)),
                    );
                    continue;
                };
                if !in_ports.contains(&target_key) {
                    add(
                        &mut diagnostics,
                        primary,
                        ProblemEvidence::missing_port(
                            SubjectRef::InPort(target_key),
                            target_key.kind(),
                        ),
                    );
                    continue;
                }
                if input.kind() != target_key.kind() {
                    add(
                        &mut diagnostics,
                        primary,
                        ProblemEvidence::signal_kind_mismatch(
                            primary,
                            SubjectRef::InPort(target_key),
                            input.kind(),
                            target_key.kind(),
                        ),
                    );
                    continue;
                }
                if inputs.contains_key(&input) {
                    boundary_drivers.insert(target_key);
                    drivers.entry(target_key).or_default().push(primary);
                }
            }
            ModuleInterfaceMapping::Output { output, source } => {
                let primary = SubjectRef::ModuleOutput(output);
                output_mappings
                    .entry(output)
                    .or_default()
                    .push(endpoint_subject(source));
                if !outputs.contains_key(&output) {
                    add(
                        &mut diagnostics,
                        primary,
                        ProblemEvidence::missing_endpoint(primary, output.kind()),
                    );
                }
                let ConnectionEndpoint::NodeOutput(source_key) = source else {
                    add(
                        &mut diagnostics,
                        primary,
                        ProblemEvidence::invalid_direction(endpoint_subject(source), primary),
                    );
                    continue;
                };
                if !out_ports.contains(&source_key) {
                    add(
                        &mut diagnostics,
                        primary,
                        ProblemEvidence::missing_port(
                            SubjectRef::OutPort(source_key),
                            source_key.kind(),
                        ),
                    );
                    continue;
                }
                if output.kind() != source_key.kind() {
                    add(
                        &mut diagnostics,
                        primary,
                        ProblemEvidence::signal_kind_mismatch(
                            SubjectRef::OutPort(source_key),
                            primary,
                            source_key.kind(),
                            output.kind(),
                        ),
                    );
                }
            }
        }
    }

    for connection in module.connections() {
        if let (ConnectionEndpoint::NodeOutput(source), ConnectionEndpoint::NodeInput(target)) =
            (connection.from(), connection.to())
            && out_ports.contains(&source)
            && in_ports.contains(&target)
            && source.kind() == target.kind()
        {
            drivers
                .entry(target)
                .or_default()
                .push(SubjectRef::Connection(connection.key()));
        }
    }
    for (input, found) in drivers.iter().filter(|(_, found)| found.len() > 1) {
        add(
            &mut diagnostics,
            SubjectRef::InPort(*input),
            ProblemEvidence::unsupported_multiple_drivers(found.clone()),
        );
    }
    for output in outputs.keys() {
        match output_mappings.get(output) {
            None => add(
                &mut diagnostics,
                SubjectRef::ModuleOutput(*output),
                ProblemEvidence::missing_required_input(
                    SubjectRef::ModuleOutput(*output),
                    output.kind(),
                ),
            ),
            Some(found) if found.len() > 1 => add(
                &mut diagnostics,
                SubjectRef::ModuleOutput(*output),
                ProblemEvidence::unsupported_multiple_drivers(found.clone()),
            ),
            Some(_) => {}
        }
    }

    let structural_definition = UncheckedNetwork::new(
        NetworkKey::from_u128(0),
        TimeDomainId::from_u128(0),
        DiagnosticMeta::default(),
        module.nodes().to_vec(),
        Vec::new(),
        Vec::new(),
        module.connections().to_vec(),
    );
    let (_, structural_diagnostics) = structural_definition
        .validate_structural_for_module(&boundary_drivers)
        .into_parts();
    for diagnostic in structural_diagnostics {
        diagnostics.insert(diagnostic);
    }
    if diagnostics.has_severity(crate::diagnostics::Severity::Error) {
        return Report::new(None, diagnostics);
    }

    let graph = ReactionDependencyGraph::from_module(module);
    for component in graph.cyclic_components() {
        let members: Vec<_> = component.iter().copied().map(reaction_member).collect();
        let Some(primary) = members.first().map(|member| member.subject) else {
            continue;
        };
        let witness = graph
            .cycle_witness(&component)
            .into_iter()
            .map(cycle_step)
            .collect();
        add(
            &mut diagnostics,
            primary,
            ProblemEvidence::current_reaction_cycle(members, witness),
        );
    }
    if diagnostics.has_severity(crate::diagnostics::Severity::Error) {
        return Report::new(None, diagnostics);
    }

    let definition = normalized(module);
    let fingerprint = module_fingerprint(&definition);
    Report::new(
        Some(ModuleDef {
            data: Arc::new(ModuleData {
                origin: ModuleOrigin::User,
                fingerprint,
                definition,
            }),
        }),
        diagnostics,
    )
}

fn add<D: PartialEq>(
    diagnostics: &mut DiagnosticSet<D>,
    primary: SubjectRef,
    evidence: ProblemEvidence<D>,
) {
    if let Ok(diagnostic) = Diagnostic::new(Problem::new(primary, Vec::new(), evidence)) {
        diagnostics.insert(diagnostic);
    }
}

fn normalized<D>(module: &UncheckedModule<D>) -> UncheckedModule<D> {
    let mut inputs = module.inputs().to_vec();
    let mut outputs = module.outputs().to_vec();
    let mut mappings = module.mappings().to_vec();
    let mut nodes = module.nodes().to_vec();
    let mut connections = module.connections().to_vec();
    inputs.sort_by_key(|input| input.key());
    outputs.sort_by_key(|output| output.key());
    mappings.sort_by_key(mapping_order);
    nodes.sort_by_key(|node| node.key());
    connections.sort_by_key(|connection| connection.key());
    UncheckedModule::new_user(
        module.meta().clone(),
        inputs,
        outputs,
        mappings,
        nodes,
        connections,
    )
}

fn mapping_order(mapping: &ModuleInterfaceMapping) -> (u8, SignalKind, u128, u8, SignalKind, u128) {
    match *mapping {
        ModuleInterfaceMapping::Input { input, target } => {
            let (target_tag, target_kind, target_key) = endpoint_order(target);
            (
                0,
                input.kind(),
                module_input_key(input),
                target_tag,
                target_kind,
                target_key,
            )
        }
        ModuleInterfaceMapping::Output { output, source } => {
            let (source_tag, source_kind, source_key) = endpoint_order(source);
            (
                1,
                output.kind(),
                module_output_key(output),
                source_tag,
                source_kind,
                source_key,
            )
        }
    }
}

fn endpoint_order(endpoint: ConnectionEndpoint) -> (u8, SignalKind, u128) {
    match endpoint {
        ConnectionEndpoint::ExternalInput(key) => (0, key.kind(), erased_key(key)),
        ConnectionEndpoint::NodeInput(key) => (1, key.kind(), in_port_key(key)),
        ConnectionEndpoint::NodeOutput(key) => (2, key.kind(), out_port_key(key)),
        ConnectionEndpoint::ExternalOutput(key) => (3, key.kind(), erased_key(key)),
    }
}

fn endpoint_subject(endpoint: ConnectionEndpoint) -> SubjectRef {
    match endpoint {
        ConnectionEndpoint::ExternalInput(key) => SubjectRef::ExternalInput(key),
        ConnectionEndpoint::NodeInput(key) => SubjectRef::InPort(key),
        ConnectionEndpoint::NodeOutput(key) => SubjectRef::OutPort(key),
        ConnectionEndpoint::ExternalOutput(key) => SubjectRef::ExternalOutput(key),
    }
}

fn module_input_key(key: AnyModuleInputKey) -> u128 {
    match key {
        AnyModuleInputKey::Level(key) => key.as_u128(),
        AnyModuleInputKey::Pulse(key) => key.as_u128(),
    }
}

fn module_output_key(key: AnyModuleOutputKey) -> u128 {
    match key {
        AnyModuleOutputKey::Level(key) => key.as_u128(),
        AnyModuleOutputKey::Pulse(key) => key.as_u128(),
    }
}

fn in_port_key(key: AnyInPortKey) -> u128 {
    match key {
        AnyInPortKey::Level(key) => key.as_u128(),
        AnyInPortKey::Pulse(key) => key.as_u128(),
    }
}

fn out_port_key(key: AnyOutPortKey) -> u128 {
    match key {
        AnyOutPortKey::Level(key) => key.as_u128(),
        AnyOutPortKey::Pulse(key) => key.as_u128(),
    }
}

fn erased_key<K>(key: K) -> u128
where
    K: IntoErasedPayload,
{
    key.payload()
}

trait IntoErasedPayload {
    fn payload(self) -> u128;
}

impl IntoErasedPayload for crate::key::AnyExternalInputKey {
    fn payload(self) -> u128 {
        match self {
            Self::Level(key) => key.as_u128(),
            Self::Pulse(key) => key.as_u128(),
        }
    }
}

impl IntoErasedPayload for crate::key::AnyExternalOutputKey {
    fn payload(self) -> u128 {
        match self {
            Self::Level(key) => key.as_u128(),
            Self::Pulse(key) => key.as_u128(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::authored::{InputPortRole, NodeKind, NodePorts};
    use crate::diagnostics::DiagnosticCode;
    use crate::identity::canonical_module_input;
    use crate::key::{InPortKey, ModuleInputKey, ModuleOutputKey, NodeKey, OutPortKey};
    use crate::metadata::OriginRef;
    use crate::signal::{Level, LogicLevel, Pulse};
    use crate::time::NonZeroSpan;

    fn meta(name: &str) -> DiagnosticMeta {
        DiagnosticMeta {
            name: Some(name.into()),
            origin: Some(OriginRef::text(format!("{name} origin"))),
            ..DiagnosticMeta::default()
        }
    }

    fn not_module(reverse: bool, with_meta: bool) -> UncheckedModule<()> {
        let input = ModuleInputKey::<Level>::from_u128(1);
        let output = ModuleOutputKey::<Level>::from_u128(2);
        let node_input = InPortKey::<Level>::from_u128(11);
        let node_output = OutPortKey::<Level>::from_u128(12);
        let mut mappings = vec![
            ModuleInterfaceMapping::input(input.into(), node_input.into()),
            ModuleInterfaceMapping::output(output.into(), node_output.into()),
        ];
        if reverse {
            mappings.reverse();
        }
        UncheckedModule::new_user(
            if with_meta {
                meta("module")
            } else {
                DiagnosticMeta::default()
            },
            vec![ModuleInputDef::new(
                input.into(),
                if with_meta {
                    meta("input")
                } else {
                    DiagnosticMeta::default()
                },
            )],
            vec![ModuleOutputDef::new(
                output.into(),
                if with_meta {
                    meta("output")
                } else {
                    DiagnosticMeta::default()
                },
            )],
            mappings,
            vec![NodeDef::new(
                NodeKey::from_u128(10),
                NodeKind::not(),
                NodePorts::new(vec![node_input.into()], vec![node_output.into()]),
                if with_meta {
                    meta("not")
                } else {
                    DiagnosticMeta::default()
                },
            )],
            Vec::new(),
        )
    }

    fn cycle_module(two_nodes: bool, delayed: bool) -> UncheckedModule<()> {
        let first_input = InPortKey::<Pulse>::from_u128(11);
        let first_output = OutPortKey::<Pulse>::from_u128(12);
        let second_input = InPortKey::<Pulse>::from_u128(21);
        let second_output = OutPortKey::<Pulse>::from_u128(22);
        let delay = NonZeroSpan::from_ticks(1)
            .unwrap_or_else(|failure| panic!("test delay must be positive: {failure}"));
        let first_kind = if delayed {
            NodeKind::pulse_delay(delay)
        } else {
            NodeKind::merge()
        };
        let mut nodes = vec![NodeDef::new(
            NodeKey::from_u128(10),
            first_kind,
            NodePorts::with_input_roles(
                vec![first_input.into()],
                vec![if delayed {
                    InputPortRole::PulseDelay
                } else {
                    InputPortRole::Input
                }],
                vec![first_output.into()],
            ),
            DiagnosticMeta::default(),
        )];
        let mut connections = vec![ConnectionDef::new(
            crate::key::ConnectionKey::from_u128(30),
            first_output.into(),
            if two_nodes {
                second_input.into()
            } else {
                first_input.into()
            },
            DiagnosticMeta::default(),
        )];
        let source = if two_nodes {
            nodes.push(NodeDef::new(
                NodeKey::from_u128(20),
                NodeKind::merge(),
                NodePorts::new(vec![second_input.into()], vec![second_output.into()]),
                DiagnosticMeta::default(),
            ));
            connections.push(ConnectionDef::new(
                crate::key::ConnectionKey::from_u128(31),
                second_output.into(),
                first_input.into(),
                DiagnosticMeta::default(),
            ));
            second_output
        } else {
            first_output
        };
        let output = ModuleOutputKey::<Pulse>::from_u128(2);
        UncheckedModule::new_user(
            DiagnosticMeta::default(),
            Vec::new(),
            vec![ModuleOutputDef::new(
                output.into(),
                DiagnosticMeta::default(),
            )],
            vec![ModuleInterfaceMapping::output(output.into(), source.into())],
            nodes,
            connections,
        )
    }

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|byte| format!("{byte:02x}")).collect()
    }

    #[test]
    fn validates_and_inspects_user_module_without_mutating_borrowed_input() {
        let unchecked = not_module(false, true);
        let retained = unchecked.clone();
        let report = unchecked.validate_ref();
        assert!(!report.has_errors());
        assert_eq!(unchecked, retained);
        let module = report
            .artifact()
            .unwrap_or_else(|| panic!("fixture must validate"));
        assert!(matches!(module.origin(), ModuleOrigin::User));
        assert_eq!(module.inputs().len(), 1);
        assert_eq!(module.outputs().len(), 1);
        assert_eq!(module.graph().nodes().len(), 1);
        assert_eq!(module.graph().mappings().len(), 2);
        assert_eq!(module.meta().name.as_deref(), Some("module"));
        assert_eq!(module.clone().fingerprint(), module.fingerprint());
    }

    #[test]
    fn public_input_fanout_and_unused_inputs_are_valid() {
        let shared = ModuleInputKey::<Level>::from_u128(1);
        let unused = ModuleInputKey::<Level>::from_u128(2);
        let mut nodes: Vec<NodeDef<()>> = Vec::new();
        let mut mappings = Vec::new();
        let mut outputs = Vec::new();
        for offset in [10_u128, 20] {
            let input = InPortKey::<Level>::from_u128(offset + 1);
            let output = OutPortKey::<Level>::from_u128(offset + 2);
            let public_output = ModuleOutputKey::<Level>::from_u128(offset);
            nodes.push(NodeDef::new(
                NodeKey::from_u128(offset),
                NodeKind::not(),
                NodePorts::new(vec![input.into()], vec![output.into()]),
                DiagnosticMeta::default(),
            ));
            mappings.push(ModuleInterfaceMapping::input(shared.into(), input.into()));
            mappings.push(ModuleInterfaceMapping::output(
                public_output.into(),
                output.into(),
            ));
            outputs.push(ModuleOutputDef::new(
                public_output.into(),
                DiagnosticMeta::default(),
            ));
        }
        let module = UncheckedModule::new_user(
            DiagnosticMeta::default(),
            vec![
                ModuleInputDef::new(shared.into(), DiagnosticMeta::default()),
                ModuleInputDef::new(unused.into(), DiagnosticMeta::default()),
            ],
            outputs,
            mappings,
            nodes,
            Vec::new(),
        );
        assert!(module.validate().artifact().is_some());
    }

    #[test]
    fn interface_defects_accumulate_and_block_the_artifact() {
        let mut module = not_module(false, false);
        let input = ModuleInputKey::<Level>::from_u128(1);
        let output = ModuleOutputKey::<Level>::from_u128(2);
        let pulse = ModuleInputKey::<Pulse>::from_u128(99);
        module = UncheckedModule::new_user(
            DiagnosticMeta::default(),
            vec![
                ModuleInputDef::new(input.into(), DiagnosticMeta::default()),
                ModuleInputDef::new(input.into(), DiagnosticMeta::default()),
            ],
            vec![
                ModuleOutputDef::new(output.into(), DiagnosticMeta::default()),
                ModuleOutputDef::new(output.into(), DiagnosticMeta::default()),
            ],
            vec![
                ModuleInterfaceMapping::input(
                    pulse.into(),
                    InPortKey::<Level>::from_u128(11).into(),
                ),
                ModuleInterfaceMapping::input(
                    input.into(),
                    OutPortKey::<Level>::from_u128(12).into(),
                ),
                ModuleInterfaceMapping::output(
                    output.into(),
                    InPortKey::<Level>::from_u128(11).into(),
                ),
                ModuleInterfaceMapping::output(
                    output.into(),
                    OutPortKey::<Level>::from_u128(999).into(),
                ),
            ],
            module.nodes().to_vec(),
            Vec::new(),
        );
        let report = module.validate();
        assert!(report.artifact().is_none());
        let codes: BTreeSet<_> = report
            .diagnostics()
            .iter()
            .map(|finding| finding.problem().code())
            .collect();
        assert!(codes.contains(&DiagnosticCode::ValidationDuplicateKey));
        assert!(codes.contains(&DiagnosticCode::ValidationMissingEndpoint));
        assert!(codes.contains(&DiagnosticCode::ValidationMissingPort));
        assert!(codes.contains(&DiagnosticCode::ValidationInvalidDirection));
        assert!(codes.contains(&DiagnosticCode::ValidationSignalKindMismatch));
        assert!(codes.contains(&DiagnosticCode::ValidationUnsupportedMultipleDrivers));
    }

    #[test]
    fn primitive_shape_validation_is_reused_for_modules() {
        let mut module = not_module(false, false);
        let output = module.nodes()[0].ports().outputs()[0];
        module = UncheckedModule::new_user(
            DiagnosticMeta::default(),
            module.inputs().to_vec(),
            module.outputs().to_vec(),
            module.mappings().to_vec(),
            vec![NodeDef::new(
                NodeKey::from_u128(10),
                NodeKind::not(),
                NodePorts::new(Vec::new(), vec![output]),
                DiagnosticMeta::default(),
            )],
            Vec::new(),
        );
        let report = module.validate();
        assert!(report.artifact().is_none());
        assert!(report.diagnostics().iter().any(|finding| {
            finding.problem().code() == DiagnosticCode::ValidationInvalidFixedArity
        }));
    }

    #[test]
    fn every_supported_primitive_validates_inside_one_user_module() {
        let level = ModuleInputKey::<Level>::from_u128(1);
        let pulse = ModuleInputKey::<Pulse>::from_u128(2);
        let public_output = ModuleOutputKey::<Level>::from_u128(3);
        let mut nodes = Vec::new();
        let mut mappings = Vec::new();

        let constant_output = OutPortKey::<Level>::from_u128(100);
        nodes.push(NodeDef::new(
            NodeKey::from_u128(100),
            NodeKind::constant(LogicLevel::High),
            NodePorts::new(Vec::new(), vec![constant_output.into()]),
            DiagnosticMeta::default(),
        ));

        for (node_key, input_key, output_key, kind) in [
            (110, 111, 112, NodeKind::<()>::not()),
            (120, 121, 122, NodeKind::all()),
            (130, 131, 132, NodeKind::any()),
            (140, 141, 142, NodeKind::parity()),
            (150, 151, 152, NodeKind::at_least(1)),
        ] {
            let input = InPortKey::<Level>::from_u128(input_key);
            let output = OutPortKey::<Level>::from_u128(output_key);
            nodes.push(NodeDef::new(
                NodeKey::from_u128(node_key),
                kind,
                NodePorts::new(vec![input.into()], vec![output.into()]),
                DiagnosticMeta::default(),
            ));
            mappings.push(ModuleInterfaceMapping::input(level.into(), input.into()));
        }

        let select_inputs = [
            InPortKey::<Level>::from_u128(161),
            InPortKey::<Level>::from_u128(162),
            InPortKey::<Level>::from_u128(163),
        ];
        nodes.push(NodeDef::new(
            NodeKey::from_u128(160),
            NodeKind::select(),
            NodePorts::with_input_roles(
                select_inputs.iter().copied().map(Into::into).collect(),
                vec![
                    InputPortRole::Selector,
                    InputPortRole::WhenLow,
                    InputPortRole::WhenHigh,
                ],
                vec![OutPortKey::<Level>::from_u128(164).into()],
            ),
            DiagnosticMeta::default(),
        ));
        mappings.extend(
            select_inputs
                .iter()
                .copied()
                .map(|input| ModuleInterfaceMapping::input(level.into(), input.into())),
        );

        let merge_input = InPortKey::<Pulse>::from_u128(171);
        nodes.push(NodeDef::new(
            NodeKey::from_u128(170),
            NodeKind::merge(),
            NodePorts::new(
                vec![merge_input.into()],
                vec![OutPortKey::<Pulse>::from_u128(172).into()],
            ),
            DiagnosticMeta::default(),
        ));
        mappings.push(ModuleInterfaceMapping::input(
            pulse.into(),
            merge_input.into(),
        ));

        let toggle_input = InPortKey::<Pulse>::from_u128(181);
        let toggle_output = OutPortKey::<Level>::from_u128(182);
        nodes.push(NodeDef::new(
            NodeKey::from_u128(180),
            NodeKind::toggle(LogicLevel::Low),
            NodePorts::with_input_roles(
                vec![toggle_input.into()],
                vec![InputPortRole::Toggle],
                vec![toggle_output.into()],
            ),
            DiagnosticMeta::default(),
        ));
        mappings.push(ModuleInterfaceMapping::input(
            pulse.into(),
            toggle_input.into(),
        ));
        mappings.push(ModuleInterfaceMapping::output(
            public_output.into(),
            toggle_output.into(),
        ));

        let delay_input = InPortKey::<Pulse>::from_u128(191);
        let delay = NonZeroSpan::from_ticks(5)
            .unwrap_or_else(|failure| panic!("test delay must be positive: {failure}"));
        nodes.push(NodeDef::new(
            NodeKey::from_u128(190),
            NodeKind::pulse_delay(delay),
            NodePorts::with_input_roles(
                vec![delay_input.into()],
                vec![InputPortRole::PulseDelay],
                vec![OutPortKey::<Pulse>::from_u128(192).into()],
            ),
            DiagnosticMeta::default(),
        ));
        mappings.push(ModuleInterfaceMapping::input(
            pulse.into(),
            delay_input.into(),
        ));

        let module = UncheckedModule::new_user(
            DiagnosticMeta::default(),
            vec![
                ModuleInputDef::new(level.into(), DiagnosticMeta::default()),
                ModuleInputDef::new(pulse.into(), DiagnosticMeta::default()),
            ],
            vec![ModuleOutputDef::new(
                public_output.into(),
                DiagnosticMeta::default(),
            )],
            mappings,
            nodes,
            Vec::new(),
        );
        let report = module.validate();
        assert!(!report.has_errors());
        assert_eq!(
            report
                .artifact()
                .unwrap_or_else(|| panic!("complete primitive fixture validates"))
                .graph()
                .nodes()
                .len(),
            10
        );
    }

    #[test]
    fn module_cycle_validation_rejects_sccs_but_accepts_later_time_barriers() {
        for module in [cycle_module(false, false), cycle_module(true, false)] {
            let report = module.validate();
            assert!(report.artifact().is_none());
            assert!(report.diagnostics().iter().any(|finding| {
                finding.problem().code() == DiagnosticCode::ValidationCurrentReactionCycle
            }));
        }
        assert!(cycle_module(false, true).validate().artifact().is_some());
    }

    #[test]
    fn module_identity_is_order_invariant_metadata_free_and_semantically_sensitive() {
        let plain = not_module(false, false).validate();
        let reordered = not_module(true, true).validate();
        let plain = plain
            .artifact()
            .unwrap_or_else(|| panic!("fixture validates"));
        let reordered = reordered
            .artifact()
            .unwrap_or_else(|| panic!("fixture validates"));
        assert_eq!(plain.fingerprint(), reordered.fingerprint());

        let input = ModuleInputKey::<Level>::from_u128(1);
        let output = ModuleOutputKey::<Level>::from_u128(2);
        let node_input = InPortKey::<Level>::from_u128(11);
        let node_output = OutPortKey::<Level>::from_u128(12);
        let changed = UncheckedModule::new_user(
            DiagnosticMeta::default(),
            vec![ModuleInputDef::new(input.into(), DiagnosticMeta::default())],
            vec![ModuleOutputDef::new(
                output.into(),
                DiagnosticMeta::default(),
            )],
            vec![
                ModuleInterfaceMapping::input(input.into(), node_input.into()),
                ModuleInterfaceMapping::output(output.into(), node_output.into()),
            ],
            vec![NodeDef::new(
                NodeKey::from_u128(10),
                NodeKind::<()>::all(),
                NodePorts::new(vec![node_input.into()], vec![node_output.into()]),
                DiagnosticMeta::default(),
            )],
            Vec::new(),
        )
        .validate();
        let changed = changed
            .artifact()
            .unwrap_or_else(|| panic!("fixture validates"));
        assert_ne!(plain.fingerprint(), changed.fingerprint());
    }

    #[test]
    fn version_one_module_vectors_cover_minimal_and_nontrivial_user_modules() {
        let minimal = UncheckedModule::<()>::new_user(
            DiagnosticMeta::default(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
        );
        let nontrivial = not_module(false, false);
        assert_eq!(
            hex(&canonical_module_input(&minimal)),
            "838266646f6d61696e781f6d6f737369676e616c2f6d6f64756c655f66696e6765727072696e742f763182677061796c6f616487826b636f6e6e656374696f6e73808266696e707574738082686d617070696e67738082656e6f6465738082666f726967696e826475736572f682676f757470757473808265706f72747380826776657273696f6e01"
        );
        assert_eq!(
            minimal
                .validate_ref()
                .artifact()
                .unwrap_or_else(|| panic!("minimal module validates"))
                .fingerprint()
                .to_string(),
            "ad744d6e6b8c404c2fa67bb75845bbc420c3f060d581236606851debb53e2ac6"
        );
        assert_eq!(
            hex(&canonical_module_input(&nontrivial)),
            "838266646f6d61696e781f6d6f737369676e616c2f6d6f64756c655f66696e6765727072696e742f763182677061796c6f616487826b636f6e6e656374696f6e73808266696e70757473818282636b65795000000000000000000000000000000001826b7369676e616c5f6b696e6482656c6576656cf682686d617070696e6773828265696e707574838265696e7075745000000000000000000000000000000001826b7369676e616c5f6b696e6482656c6576656cf68266746172676574500000000000000000000000000000000b82666f75747075748382666f75747075745000000000000000000000000000000002826b7369676e616c5f6b696e6482656c6576656cf68266736f75726365500000000000000000000000000000000c82656e6f646573818282636b6579500000000000000000000000000000000a82646b696e6482636e6f74f682666f726967696e826475736572f682676f757470757473818282636b65795000000000000000000000000000000002826b7369676e616c5f6b696e6482656c6576656cf68265706f72747382858269646972656374696f6e8265696e707574f682636b6579500000000000000000000000000000000b82656f776e6572500000000000000000000000000000000a826d73656d616e7469635f726f6c658265696e707574f6826b7369676e616c5f6b696e6482656c6576656cf6858269646972656374696f6e82666f7574707574f682636b6579500000000000000000000000000000000c82656f776e6572500000000000000000000000000000000a826d73656d616e7469635f726f6c6582666f7574707574f6826b7369676e616c5f6b696e6482656c6576656cf6826776657273696f6e01"
        );
        assert_eq!(
            nontrivial
                .validate_ref()
                .artifact()
                .unwrap_or_else(|| panic!("nontrivial module validates"))
                .fingerprint()
                .to_string(),
            "74f0d17a50136478cc673aa0c17c609f0a27279b468d03cfa737c3fee8472743"
        );
    }

    #[test]
    fn module_fingerprint_covers_stateful_and_temporal_semantics() {
        let pulse = ModuleInputKey::<Pulse>::from_u128(1);
        let output = ModuleOutputKey::<Level>::from_u128(2);
        let input = InPortKey::<Pulse>::from_u128(11);
        let node_output = OutPortKey::<Level>::from_u128(12);
        let build = |initial| {
            UncheckedModule::new_user(
                DiagnosticMeta::default(),
                vec![ModuleInputDef::new(pulse.into(), DiagnosticMeta::default())],
                vec![ModuleOutputDef::new(
                    output.into(),
                    DiagnosticMeta::default(),
                )],
                vec![
                    ModuleInterfaceMapping::input(pulse.into(), input.into()),
                    ModuleInterfaceMapping::output(output.into(), node_output.into()),
                ],
                vec![NodeDef::new(
                    NodeKey::from_u128(10),
                    NodeKind::<()>::toggle(initial),
                    NodePorts::with_input_roles(
                        vec![input.into()],
                        vec![InputPortRole::Toggle],
                        vec![node_output.into()],
                    ),
                    DiagnosticMeta::default(),
                )],
                Vec::new(),
            )
        };
        let low = build(LogicLevel::Low).validate();
        let high = build(LogicLevel::High).validate();
        assert_ne!(
            low.artifact().unwrap().fingerprint(),
            high.artifact().unwrap().fingerprint()
        );
    }
}
