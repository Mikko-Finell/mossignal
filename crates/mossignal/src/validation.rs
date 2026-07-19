//! Private structural validation for the opening authored graph.

#![allow(dead_code)] // Consumed by the following private validation phase.

use crate::authored::{ConnectionEndpoint, NodeKind, UncheckedNetwork};
use crate::diagnostics::{
    Diagnostic, DiagnosticSet, FixedArityRole, Problem, ProblemEvidence, Report, SubjectRef,
};
use crate::key::{
    AnyExternalInputKey, AnyExternalOutputKey, AnyInPortKey, AnyOutPortKey, AnySignalSourceKey,
    SignalSourceKey,
};
use crate::signal::SignalKind;
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
}

impl<D> UncheckedNetwork<D> {
    /// Performs only the opening, pre-semantic structural checks.
    pub(crate) fn validate_structural(&self) -> Report<StructuralCandidate<'_, D>, D>
    where
        D: PartialEq,
    {
        StructuralValidator::new(self).run()
    }
}

struct StructuralValidator<'a, D> {
    network: &'a UncheckedNetwork<D>,
    diagnostics: DiagnosticSet<D>,
    nodes: BTreeSet<crate::key::NodeKey>,
    inputs: BTreeSet<AnyInPortKey>,
    outputs: BTreeSet<AnyOutPortKey>,
    external_inputs: BTreeSet<AnyExternalInputKey>,
    external_outputs: BTreeSet<AnyExternalOutputKey>,
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
        }
    }

    fn run(mut self) -> Report<StructuralCandidate<'a, D>, D> {
        self.collect_keys_and_duplicates();
        self.validate_node_shapes();
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
        for node in self.network.nodes() {
            nodes
                .entry(node.key())
                .or_insert_with(Vec::new)
                .push(SubjectRef::Node(node.key()));
            self.nodes.insert(node.key());
            for key in node.ports().inputs() {
                inputs
                    .entry(*key)
                    .or_insert_with(Vec::new)
                    .push(SubjectRef::InPort(*key));
                self.inputs.insert(*key);
            }
            for key in node.ports().outputs() {
                outputs
                    .entry(*key)
                    .or_insert_with(Vec::new)
                    .push(SubjectRef::OutPort(*key));
                self.outputs.insert(*key);
            }
        }
        for connection in self.network.connections() {
            connections
                .entry(connection.key())
                .or_insert_with(Vec::new)
                .push(SubjectRef::Connection(connection.key()));
        }
        for endpoint in self.network.external_inputs() {
            external_inputs
                .entry(endpoint.key())
                .or_insert_with(Vec::new)
                .push(SubjectRef::ExternalInput(endpoint.key()));
            self.external_inputs.insert(endpoint.key());
        }
        for endpoint in self.network.external_outputs() {
            external_outputs
                .entry(endpoint.key())
                .or_insert_with(Vec::new)
                .push(SubjectRef::ExternalOutput(endpoint.key()));
            self.external_outputs.insert(endpoint.key());
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
    }

    fn duplicate(&mut self, claims: &[SubjectRef]) {
        // The kernel preserves multiplicity and canonicalizes claim subjects.
        self.add(
            SubjectRef::Network(self.network.key()),
            ProblemEvidence::duplicate_key(claims[0], claims.to_vec()),
        );
    }

    fn validate_node_shapes(&mut self) {
        for node in self.network.nodes() {
            let (expected_inputs, expected_outputs) = match node.kind() {
                NodeKind::Constant(_) => (0, 1),
                NodeKind::Not => (1, 1),
            };
            let inputs = node.ports().inputs();
            let outputs = node.ports().outputs();
            if inputs.len() != expected_inputs {
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
            if matches!(node.kind(), NodeKind::Not) && inputs.is_empty() {
                self.add(
                    SubjectRef::Node(node.key()),
                    ProblemEvidence::missing_required_input(
                        SubjectRef::Node(node.key()),
                        SignalKind::Level,
                    ),
                );
            }
            if inputs.iter().any(|key| key.kind() != SignalKind::Level) {
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
            if outputs.iter().any(|key| key.kind() != SignalKind::Level) {
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
        }
    }

    fn validate_connections(&mut self) {
        let mut drivers: BTreeMap<AnyInPortKey, Vec<SubjectRef>> = BTreeMap::new();
        for connection in self.network.connections() {
            let source = connection.from();
            let target = connection.to();
            let source_subject = endpoint_subject(source);
            let target_subject = endpoint_subject(target);
            let source_valid = self.validate_source(connection.key(), source);
            let target_valid = self.validate_target(connection.key(), target);
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
            if source_valid && target_valid && source.kind() == target.kind() {
                if let ConnectionEndpoint::NodeInput(input) = target {
                    drivers
                        .entry(input)
                        .or_default()
                        .push(SubjectRef::Connection(connection.key()));
                }
            }
        }
        for (input, found) in drivers.into_iter().filter(|(_, found)| found.len() > 1) {
            self.add(
                SubjectRef::InPort(input),
                ProblemEvidence::unsupported_multiple_drivers(found),
            );
        }
    }

    fn validate_source(
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
            ConnectionEndpoint::ExternalInput(_) | ConnectionEndpoint::NodeOutput(_) => true,
            _ => true,
        }
    }

    fn validate_target(
        &mut self,
        connection: crate::key::ConnectionKey,
        endpoint: ConnectionEndpoint,
    ) -> bool {
        match endpoint {
            ConnectionEndpoint::NodeInput(key) if !self.inputs.contains(&key) => {
                self.add(
                    SubjectRef::Connection(connection),
                    ProblemEvidence::missing_port(SubjectRef::InPort(key), key.kind()),
                );
                false
            }
            ConnectionEndpoint::NodeInput(_) => true,
            _ => true,
        }
    }

    fn validate_external_outputs(&mut self) {
        for output in self.network.external_outputs() {
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
                _ => {}
            }
        }
    }
}

fn is_source(endpoint: ConnectionEndpoint) -> bool {
    matches!(
        endpoint,
        ConnectionEndpoint::ExternalInput(_) | ConnectionEndpoint::NodeOutput(_)
    )
}

fn is_target(endpoint: ConnectionEndpoint) -> bool {
    matches!(endpoint, ConnectionEndpoint::NodeInput(_))
}

fn endpoint_subject(endpoint: ConnectionEndpoint) -> SubjectRef {
    match endpoint {
        ConnectionEndpoint::ExternalInput(key) => SubjectRef::ExternalInput(key),
        ConnectionEndpoint::NodeInput(key) => SubjectRef::InPort(key),
        ConnectionEndpoint::NodeOutput(key) => SubjectRef::OutPort(key),
        ConnectionEndpoint::ExternalOutput(key) => SubjectRef::ExternalOutput(key),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::authored::{ConnectionDef, ExternalOutputDef, NodeDef, NodePorts};
    use crate::diagnostics::DiagnosticCode;
    use crate::key::{
        ConnectionKey, ExternalOutputKey, InPortKey, NetworkKey, NodeKey, OutPortKey,
        SignalSourceKey,
    };
    use crate::metadata::DiagnosticMeta;
    use crate::signal::{Level, LogicLevel};

    #[test]
    fn accepts_a_structurally_valid_constant_and_not_graph() {
        let constant = OutPortKey::<Level>::from_u128(1);
        let input = InPortKey::<Level>::from_u128(2);
        let output = OutPortKey::<Level>::from_u128(3);
        let network = UncheckedNetwork::new(
            NetworkKey::from_u128(1),
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
    fn accumulates_independent_structural_errors_and_omits_candidate() {
        let output = OutPortKey::<Level>::from_u128(1);
        let input = InPortKey::<Level>::from_u128(2);
        let network = UncheckedNetwork::new(
            NetworkKey::from_u128(1),
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
            DiagnosticMeta::default(),
            nodes.clone(),
            vec![],
            vec![],
            vec![first.clone(), second.clone()],
        );
        let reverse = UncheckedNetwork::new(
            NetworkKey::from_u128(1),
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
}
