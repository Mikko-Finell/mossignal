//! Typed authoring for the restricted level-only network foundation.

use crate::ValidatedNetwork;
use crate::authored::{
    ConnectionDef, ConnectionEndpoint, ExternalInputDef, ExternalOutputDef, NodeDef, NodeKind,
    NodePorts, UncheckedNetwork,
};
use crate::diagnostics::Report;
use crate::identity::TimeDomainId;
use crate::key::{
    ExternalInputKey, ExternalOutputKey, InPortKey, KeyAllocator, NetworkKey, NodeKey, OutPortKey,
    SignalSourceKey,
};
use crate::metadata::DiagnosticMeta;
use crate::signal::{Level, LogicLevel, SignalType};
use core::sync::atomic::{AtomicU64, Ordering};
use std::collections::BTreeSet;

static NEXT_BUILDER_ID: AtomicU64 = AtomicU64::new(1);

/// A structured failure detected while authoring through [`NetworkBuilder`].
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthoringFailure {
    /// A signal belongs to another live builder scope.
    ForeignSignal,
    /// An explicit external input key is already present in this builder.
    DuplicateExternalInputKey(ExternalInputKey<Level>),
    /// An explicit external output key is already present in this builder.
    DuplicateExternalOutputKey(ExternalOutputKey<Level>),
    /// An explicit node key is already present in this builder.
    DuplicateNodeKey(NodeKey),
    /// An explicit input-port key is already present in this builder.
    DuplicateInPortKey(InPortKey<Level>),
    /// An explicit output-port key is already present in this builder.
    DuplicateOutPortKey(OutPortKey<Level>),
}

/// A typed, builder-scoped reference to one authored signal source.
///
/// This is an authoring handle only. It is neither a persistent structural key
/// nor a runtime, binding, snapshot, replay, or persistence artifact.
#[derive(Debug)]
pub struct Signal<S: SignalType> {
    builder_id: u64,
    source: SignalSourceKey<S>,
}

impl<S: SignalType> Copy for Signal<S> {}

impl<S: SignalType> Clone for Signal<S> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<S: SignalType> Signal<S> {
    /// Returns the stable source identity represented by this authoring handle.
    #[must_use]
    pub const fn source_key(self) -> SignalSourceKey<S> {
        self.source
    }
}

/// The result of explicitly keyed node construction.
#[derive(Debug, Clone, Copy)]
pub struct AddedNode<O> {
    key: NodeKey,
    outputs: O,
}

impl<O> AddedNode<O> {
    /// Returns the stable identity supplied for the node.
    #[must_use]
    pub const fn key(&self) -> NodeKey {
        self.key
    }

    /// Borrows the authored output handle or handles.
    #[must_use]
    pub const fn outputs(&self) -> &O {
        &self.outputs
    }

    /// Consumes the result and returns its output handle or handles.
    #[must_use]
    pub fn into_outputs(self) -> O {
        self.outputs
    }

    /// Consumes the result into its node identity and output handle or handles.
    #[must_use]
    pub fn into_parts(self) -> (NodeKey, O) {
        (self.key, self.outputs)
    }
}

/// An owning, typed authoring surface for a restricted network definition.
pub struct NetworkBuilder<D> {
    key: NetworkKey,
    time_domain_id: TimeDomainId,
    meta: DiagnosticMeta,
    builder_id: u64,
    allocator: KeyAllocator,
    nodes: Vec<NodeDef<D>>,
    external_inputs: Vec<ExternalInputDef>,
    external_outputs: Vec<ExternalOutputDef>,
    connections: Vec<ConnectionDef>,
    node_keys: BTreeSet<NodeKey>,
    input_keys: BTreeSet<ExternalInputKey<Level>>,
    output_keys: BTreeSet<ExternalOutputKey<Level>>,
    in_port_keys: BTreeSet<InPortKey<Level>>,
    out_port_keys: BTreeSet<OutPortKey<Level>>,
}

impl<D> NetworkBuilder<D> {
    /// Creates a builder using a deterministic local allocator namespace.
    #[must_use]
    pub fn new(time_domain_id: TimeDomainId) -> Self {
        Self::with_key_and_meta(
            NetworkKey::from_u128(0),
            time_domain_id,
            DiagnosticMeta::default(),
        )
    }

    /// Creates a builder with an explicit durable network identity.
    #[must_use]
    pub fn with_key(key: NetworkKey, time_domain_id: TimeDomainId) -> Self {
        Self::with_key_and_meta(key, time_domain_id, DiagnosticMeta::default())
    }

    /// Creates a builder with explicit durable network identity and metadata.
    #[must_use]
    pub fn with_key_and_meta(
        key: NetworkKey,
        time_domain_id: TimeDomainId,
        meta: DiagnosticMeta,
    ) -> Self {
        Self {
            key,
            time_domain_id,
            meta,
            builder_id: NEXT_BUILDER_ID.fetch_add(1, Ordering::Relaxed),
            allocator: KeyAllocator::new(0),
            nodes: Vec::new(),
            external_inputs: Vec::new(),
            external_outputs: Vec::new(),
            connections: Vec::new(),
            node_keys: BTreeSet::new(),
            input_keys: BTreeSet::new(),
            output_keys: BTreeSet::new(),
            in_port_keys: BTreeSet::new(),
            out_port_keys: BTreeSet::new(),
        }
    }

    /// Returns the stable identity bound to the authored network.
    #[must_use]
    pub const fn network_key(&self) -> NetworkKey {
        self.key
    }

    /// Returns the caller-supplied logical-time identity bound at creation.
    #[must_use]
    pub const fn time_domain_id(&self) -> TimeDomainId {
        self.time_domain_id
    }

    /// Adds a convenience level input with locally allocated stable identity.
    pub fn level_input(
        &mut self,
        name: impl Into<String>,
    ) -> (ExternalInputKey<Level>, Signal<Level>) {
        let key = self.next_external_input_key();
        let signal = match self.add_level_input(key, named_meta(name)) {
            Ok(signal) => signal,
            Err(_) => panic!("fresh allocator key must not conflict with builder state"),
        };
        (key, signal)
    }

    /// Adds an explicit level input and retains its metadata unchanged.
    pub fn add_level_input(
        &mut self,
        key: ExternalInputKey<Level>,
        meta: DiagnosticMeta,
    ) -> Result<Signal<Level>, AuthoringFailure> {
        if !self.input_keys.insert(key) {
            return Err(AuthoringFailure::DuplicateExternalInputKey(key));
        }
        self.external_inputs
            .push(ExternalInputDef::new(key.into(), meta));
        Ok(self.signal(SignalSourceKey::ExternalInput(key)))
    }

    /// Adds a convenience level output with locally allocated stable identity.
    pub fn level_output(
        &mut self,
        name: impl Into<String>,
        source: Signal<Level>,
    ) -> Result<ExternalOutputKey<Level>, AuthoringFailure> {
        let key = self.next_external_output_key();
        self.add_level_output(key, source, named_meta(name))?;
        Ok(key)
    }

    /// Adds an explicit level output and retains its metadata unchanged.
    pub fn add_level_output(
        &mut self,
        key: ExternalOutputKey<Level>,
        source: Signal<Level>,
        meta: DiagnosticMeta,
    ) -> Result<(), AuthoringFailure> {
        self.require_local(source)?;
        if !self.output_keys.insert(key) {
            return Err(AuthoringFailure::DuplicateExternalOutputKey(key));
        }
        self.external_outputs.push(ExternalOutputDef::new(
            key.into(),
            source.source.into(),
            meta,
        ));
        Ok(())
    }

    /// Adds an infallible level constant with locally allocated identity.
    pub fn constant(&mut self, value: LogicLevel) -> Signal<Level> {
        let key = self.next_node_key();
        let output = self.next_out_port_key();
        match self.add_constant_with_output(key, output, value, DiagnosticMeta::default()) {
            Ok(added) => added.into_outputs(),
            Err(_) => panic!("fresh allocator keys must not conflict with builder state"),
        }
    }

    /// Adds an explicit constant node with a locally allocated output port.
    pub fn add_constant(
        &mut self,
        key: NodeKey,
        value: LogicLevel,
        meta: DiagnosticMeta,
    ) -> Result<AddedNode<Signal<Level>>, AuthoringFailure> {
        let output = self.next_out_port_key();
        self.add_constant_with_output(key, output, value, meta)
    }

    /// Adds an explicit constant node with an exact output-port identity.
    pub fn add_constant_with_output(
        &mut self,
        key: NodeKey,
        output: OutPortKey<Level>,
        value: LogicLevel,
        meta: DiagnosticMeta,
    ) -> Result<AddedNode<Signal<Level>>, AuthoringFailure> {
        self.insert_node_keys(key, None, output)?;
        self.nodes.push(NodeDef::new(
            key,
            NodeKind::constant(value),
            NodePorts::new(Vec::new(), vec![output.into()]),
            meta,
        ));
        Ok(AddedNode {
            key,
            outputs: self.signal(SignalSourceKey::NodeOutput(output)),
        })
    }

    /// Adds a level inverter with locally allocated node and port identities.
    pub fn not(&mut self, input: Signal<Level>) -> Result<Signal<Level>, AuthoringFailure> {
        let key = self.next_node_key();
        let input_port = self.next_in_port_key();
        let output_port = self.next_out_port_key();
        Ok(self
            .add_not_with_ports(
                key,
                input_port,
                output_port,
                input,
                DiagnosticMeta::default(),
            )?
            .into_outputs())
    }

    /// Adds an explicit inverter node with locally allocated port identities.
    pub fn add_not(
        &mut self,
        key: NodeKey,
        input: Signal<Level>,
        meta: DiagnosticMeta,
    ) -> Result<AddedNode<Signal<Level>>, AuthoringFailure> {
        let input_port = self.next_in_port_key();
        let output_port = self.next_out_port_key();
        self.add_not_with_ports(key, input_port, output_port, input, meta)
    }

    /// Adds an explicit inverter node with exact port identities.
    pub fn add_not_with_ports(
        &mut self,
        key: NodeKey,
        input_port: InPortKey<Level>,
        output_port: OutPortKey<Level>,
        input: Signal<Level>,
        meta: DiagnosticMeta,
    ) -> Result<AddedNode<Signal<Level>>, AuthoringFailure> {
        self.require_local(input)?;
        self.insert_node_keys(key, Some(input_port), output_port)?;
        self.nodes.push(NodeDef::new(
            key,
            NodeKind::not(),
            NodePorts::new(vec![input_port.into()], vec![output_port.into()]),
            meta,
        ));
        self.connections.push(ConnectionDef::new(
            self.allocator.connection(),
            source_endpoint(input.source),
            ConnectionEndpoint::node_input(input_port.into()),
            DiagnosticMeta::default(),
        ));
        Ok(AddedNode {
            key,
            outputs: self.signal(SignalSourceKey::NodeOutput(output_port)),
        })
    }

    /// Adds a variadic level conjunction with locally allocated stable identities.
    pub fn all<I>(&mut self, inputs: I) -> Result<Signal<Level>, AuthoringFailure>
    where
        I: IntoIterator<Item = Signal<Level>>,
    {
        let inputs = inputs.into_iter().collect::<Vec<_>>();
        self.require_all_local(&inputs)?;
        let key = self.next_node_key();
        let output = self.next_out_port_key();
        let mut keyed_inputs = Vec::with_capacity(inputs.len());
        for input in inputs {
            keyed_inputs.push((self.next_in_port_key(), input));
        }
        match self.add_all_with_ports(key, output, keyed_inputs, DiagnosticMeta::default()) {
            Ok(added) => Ok(added.into_outputs()),
            Err(_) => panic!("fresh allocator keys must not conflict with builder state"),
        }
    }

    /// Adds an explicitly keyed conjunction with locally allocated port identities.
    pub fn add_all<I>(
        &mut self,
        key: NodeKey,
        inputs: I,
        meta: DiagnosticMeta,
    ) -> Result<AddedNode<Signal<Level>>, AuthoringFailure>
    where
        I: IntoIterator<Item = Signal<Level>>,
    {
        let inputs = inputs.into_iter().collect::<Vec<_>>();
        self.require_all_local(&inputs)?;
        let output = self.next_out_port_key();
        let mut keyed_inputs = Vec::with_capacity(inputs.len());
        for input in inputs {
            keyed_inputs.push((self.next_in_port_key(), input));
        }
        self.add_all_with_ports(key, output, keyed_inputs, meta)
    }

    /// Adds an explicitly keyed conjunction with exact stable port identities.
    pub fn add_all_with_ports<I>(
        &mut self,
        key: NodeKey,
        output: OutPortKey<Level>,
        inputs: I,
        meta: DiagnosticMeta,
    ) -> Result<AddedNode<Signal<Level>>, AuthoringFailure>
    where
        I: IntoIterator<Item = (InPortKey<Level>, Signal<Level>)>,
    {
        let inputs = inputs.into_iter().collect::<Vec<_>>();
        for (_, input) in &inputs {
            self.require_local(*input)?;
        }
        self.insert_variadic_node_keys(key, inputs.iter().map(|(port, _)| *port), output)?;
        self.nodes.push(NodeDef::new(
            key,
            NodeKind::all(),
            NodePorts::new(
                inputs.iter().map(|(port, _)| (*port).into()).collect(),
                vec![output.into()],
            ),
            meta,
        ));
        for (port, input) in inputs {
            self.connections.push(ConnectionDef::new(
                self.allocator.connection(),
                source_endpoint(input.source),
                ConnectionEndpoint::node_input(port.into()),
                DiagnosticMeta::default(),
            ));
        }
        Ok(AddedNode {
            key,
            outputs: self.signal(SignalSourceKey::NodeOutput(output)),
        })
    }

    /// Consumes the builder into the canonical dynamic authored definition.
    #[must_use]
    pub fn into_unchecked(self) -> UncheckedNetwork<D> {
        UncheckedNetwork::new(
            self.key,
            self.time_domain_id,
            self.meta,
            self.nodes,
            self.external_inputs,
            self.external_outputs,
            self.connections,
        )
    }

    /// Consumes, materializes, and validates the authored definition.
    #[must_use]
    pub fn finish(self) -> Report<ValidatedNetwork<D>, D>
    where
        D: PartialEq,
    {
        self.into_unchecked().validate()
    }

    fn signal<S: SignalType>(&self, source: SignalSourceKey<S>) -> Signal<S> {
        Signal {
            builder_id: self.builder_id,
            source,
        }
    }

    fn require_local<S: SignalType>(&self, signal: Signal<S>) -> Result<(), AuthoringFailure> {
        if signal.builder_id == self.builder_id {
            Ok(())
        } else {
            Err(AuthoringFailure::ForeignSignal)
        }
    }

    fn require_all_local<S: SignalType>(
        &self,
        signals: &[Signal<S>],
    ) -> Result<(), AuthoringFailure> {
        for signal in signals {
            self.require_local(*signal)?;
        }
        Ok(())
    }

    fn insert_node_keys(
        &mut self,
        key: NodeKey,
        input: Option<InPortKey<Level>>,
        output: OutPortKey<Level>,
    ) -> Result<(), AuthoringFailure> {
        if self.node_keys.contains(&key) {
            return Err(AuthoringFailure::DuplicateNodeKey(key));
        }
        if let Some(input) = input {
            if self.in_port_keys.contains(&input) {
                return Err(AuthoringFailure::DuplicateInPortKey(input));
            }
        }
        if self.out_port_keys.contains(&output) {
            return Err(AuthoringFailure::DuplicateOutPortKey(output));
        }
        self.node_keys.insert(key);
        if let Some(input) = input {
            self.in_port_keys.insert(input);
        }
        self.out_port_keys.insert(output);
        Ok(())
    }

    fn insert_variadic_node_keys<I>(
        &mut self,
        key: NodeKey,
        inputs: I,
        output: OutPortKey<Level>,
    ) -> Result<(), AuthoringFailure>
    where
        I: IntoIterator<Item = InPortKey<Level>>,
    {
        if self.node_keys.contains(&key) {
            return Err(AuthoringFailure::DuplicateNodeKey(key));
        }
        let inputs = inputs.into_iter().collect::<Vec<_>>();
        let mut proposed = BTreeSet::new();
        for input in &inputs {
            if self.in_port_keys.contains(input) || !proposed.insert(*input) {
                return Err(AuthoringFailure::DuplicateInPortKey(*input));
            }
        }
        if self.out_port_keys.contains(&output) {
            return Err(AuthoringFailure::DuplicateOutPortKey(output));
        }
        self.node_keys.insert(key);
        self.in_port_keys.extend(inputs);
        self.out_port_keys.insert(output);
        Ok(())
    }

    fn next_node_key(&mut self) -> NodeKey {
        loop {
            let key = self.allocator.node();
            if !self.node_keys.contains(&key) {
                return key;
            }
        }
    }

    fn next_in_port_key(&mut self) -> InPortKey<Level> {
        loop {
            let key = self.allocator.in_port();
            if !self.in_port_keys.contains(&key) {
                return key;
            }
        }
    }

    fn next_out_port_key(&mut self) -> OutPortKey<Level> {
        loop {
            let key = self.allocator.out_port();
            if !self.out_port_keys.contains(&key) {
                return key;
            }
        }
    }

    fn next_external_input_key(&mut self) -> ExternalInputKey<Level> {
        loop {
            let key = self.allocator.external_input();
            if !self.input_keys.contains(&key) {
                return key;
            }
        }
    }

    fn next_external_output_key(&mut self) -> ExternalOutputKey<Level> {
        loop {
            let key = self.allocator.external_output();
            if !self.output_keys.contains(&key) {
                return key;
            }
        }
    }
}

fn named_meta(name: impl Into<String>) -> DiagnosticMeta {
    DiagnosticMeta {
        name: Some(name.into()),
        ..DiagnosticMeta::default()
    }
}

fn source_endpoint(source: SignalSourceKey<Level>) -> ConnectionEndpoint {
    match source {
        SignalSourceKey::ExternalInput(key) => ConnectionEndpoint::external_input(key.into()),
        SignalSourceKey::NodeOutput(key) => ConnectionEndpoint::node_output(key.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::authored::{
        ConnectionDef, ExternalInputDef, ExternalOutputDef, NodeDef, NodeKind, NodePorts,
        UncheckedNetwork,
    };
    use crate::key::AnySignalSourceKey;
    use std::collections::BTreeMap;

    fn meta(name: &str) -> DiagnosticMeta {
        DiagnosticMeta {
            name: Some(name.into()),
            ..DiagnosticMeta::default()
        }
    }

    #[test]
    fn lowers_explicit_level_structure_without_changing_keys_or_metadata() {
        let domain = TimeDomainId::from_u128(11);
        let network = NetworkKey::from_u128(12);
        let input = ExternalInputKey::from_u128(13);
        let not = NodeKey::from_u128(14);
        let not_input = InPortKey::from_u128(15);
        let not_output = OutPortKey::from_u128(16);
        let output = ExternalOutputKey::from_u128(17);
        let mut builder = NetworkBuilder::<()>::with_key_and_meta(network, domain, meta("network"));
        let source = builder.add_level_input(input, meta("input")).unwrap();
        let inverted = builder
            .add_not_with_ports(not, not_input, not_output, source, meta("not"))
            .unwrap()
            .into_outputs();
        builder
            .add_level_output(output, inverted, meta("output"))
            .unwrap();

        let lowered = builder.into_unchecked();
        assert_eq!(lowered.key(), network);
        assert_eq!(lowered.time_domain_id(), domain);
        assert_eq!(lowered.meta(), &meta("network"));
        assert_eq!(
            lowered.external_inputs(),
            &[ExternalInputDef::new(input.into(), meta("input"))]
        );
        assert_eq!(lowered.nodes()[0].key(), not);
        assert_eq!(lowered.nodes()[0].ports().inputs(), &[not_input.into()]);
        assert_eq!(lowered.nodes()[0].ports().outputs(), &[not_output.into()]);
        assert_eq!(lowered.nodes()[0].meta(), &meta("not"));
        assert_eq!(
            lowered.external_outputs(),
            &[ExternalOutputDef::new(
                output.into(),
                AnySignalSourceKey::Level(SignalSourceKey::NodeOutput(not_output)),
                meta("output")
            )]
        );
    }

    #[test]
    fn foreign_signal_is_rejected_without_poisoning_builder() {
        let domain = TimeDomainId::from_u128(1);
        let mut left = NetworkBuilder::<()>::new(domain);
        let foreign = left.level_input("foreign").1;
        let mut right = NetworkBuilder::<()>::new(domain);
        assert!(matches!(
            right.not(foreign),
            Err(AuthoringFailure::ForeignSignal)
        ));
        let local = right.level_input("local").1;
        assert!(right.not(local).is_ok());
    }

    #[test]
    fn all_rejects_foreign_and_duplicate_explicit_inputs_without_poisoning_builder() {
        let domain = TimeDomainId::from_u128(1);
        let mut foreign_builder = NetworkBuilder::<()>::new(domain);
        let foreign = foreign_builder.level_input("foreign").1;
        let mut builder = NetworkBuilder::<()>::new(domain);
        let local = builder.level_input("local").1;
        assert!(matches!(
            builder.all([local, foreign]),
            Err(AuthoringFailure::ForeignSignal)
        ));

        let duplicate = InPortKey::from_u128(10);
        assert!(matches!(
            builder.add_all_with_ports(
                NodeKey::from_u128(11),
                OutPortKey::from_u128(12),
                [(duplicate, local), (duplicate, local)],
                DiagnosticMeta::default(),
            ),
            Err(AuthoringFailure::DuplicateInPortKey(actual)) if actual == duplicate
        ));

        let node = NodeKey::from_u128(13);
        let output = OutPortKey::from_u128(14);
        builder
            .add_all_with_ports(
                node,
                output,
                [(InPortKey::from_u128(15), local)],
                DiagnosticMeta::default(),
            )
            .unwrap();
        assert!(matches!(
            builder.add_all_with_ports(
                node,
                OutPortKey::from_u128(16),
                [(InPortKey::from_u128(17), local)],
                DiagnosticMeta::default(),
            ),
            Err(AuthoringFailure::DuplicateNodeKey(actual)) if actual == node
        ));
        assert!(matches!(
            builder.add_all_with_ports(
                NodeKey::from_u128(18),
                output,
                [(InPortKey::from_u128(19), local)],
                DiagnosticMeta::default(),
            ),
            Err(AuthoringFailure::DuplicateOutPortKey(actual)) if actual == output
        ));
        assert!(builder.all([local]).is_ok());
    }

    #[test]
    fn explicit_all_preserves_variadic_port_identity_order_and_multiplicity() {
        let mut builder = NetworkBuilder::<()>::new(TimeDomainId::from_u128(1));
        let first = builder.level_input("first").1;
        let second = builder.level_input("second").1;
        let node = NodeKey::from_u128(20);
        let output = OutPortKey::from_u128(21);
        let first_port = InPortKey::from_u128(22);
        let second_port = InPortKey::from_u128(23);
        let third_port = InPortKey::from_u128(24);
        let added = builder
            .add_all_with_ports(
                node,
                output,
                [
                    (second_port, second),
                    (first_port, first),
                    (third_port, first),
                ],
                meta("all"),
            )
            .unwrap();
        assert_eq!(added.key(), node);
        assert_eq!(
            added.outputs().source_key(),
            SignalSourceKey::NodeOutput(output)
        );

        let lowered = builder.into_unchecked();
        let all = lowered
            .nodes()
            .iter()
            .find(|candidate| candidate.key() == node)
            .unwrap();
        assert!(matches!(all.kind(), NodeKind::All));
        assert_eq!(
            all.ports().inputs(),
            &[second_port.into(), first_port.into(), third_port.into()]
        );
        assert_eq!(all.ports().outputs(), &[output.into()]);
        assert_eq!(lowered.connections().len(), 3);
    }

    #[test]
    fn all_convenience_allocates_input_ports_in_iterator_order() {
        let mut builder = NetworkBuilder::<()>::new(TimeDomainId::from_u128(1));
        let first = builder.level_input("first");
        let second = builder.level_input("second");
        builder.all([second.1, first.1]).unwrap();

        let lowered = builder.into_unchecked();
        let all = lowered.nodes().last().unwrap();
        let inputs = all.ports().inputs();
        assert_eq!(inputs.len(), 2);
        assert_eq!(
            lowered.connections()[0].from(),
            ConnectionEndpoint::ExternalInput(second.0.into())
        );
        assert_eq!(
            lowered.connections()[0].to(),
            ConnectionEndpoint::NodeInput(inputs[0])
        );
        assert_eq!(
            lowered.connections()[1].from(),
            ConnectionEndpoint::ExternalInput(first.0.into())
        );
        assert_eq!(
            lowered.connections()[1].to(),
            ConnectionEndpoint::NodeInput(inputs[1])
        );
    }

    #[test]
    fn duplicate_explicit_keys_are_rejected() {
        let mut builder = NetworkBuilder::<()>::new(TimeDomainId::from_u128(1));
        let input = ExternalInputKey::from_u128(2);
        assert!(
            builder
                .add_level_input(input, DiagnosticMeta::default())
                .is_ok()
        );
        assert!(matches!(
            builder.add_level_input(input, DiagnosticMeta::default()),
            Err(AuthoringFailure::DuplicateExternalInputKey(actual)) if actual == input
        ));
        let node = NodeKey::from_u128(3);
        assert!(
            builder
                .add_constant(node, LogicLevel::Low, DiagnosticMeta::default())
                .is_ok()
        );
        assert!(matches!(
            builder.add_constant(node, LogicLevel::High, DiagnosticMeta::default()),
            Err(AuthoringFailure::DuplicateNodeKey(actual)) if actual == node
        ));
    }

    #[test]
    fn equal_constants_remain_distinct_nodes() {
        let mut builder = NetworkBuilder::<()>::new(TimeDomainId::from_u128(1));
        let first = builder.constant(LogicLevel::High).source_key();
        let second = builder.constant(LogicLevel::High).source_key();
        assert_ne!(first, second);
        let lowered = builder.into_unchecked();
        assert_eq!(lowered.nodes().len(), 2);
        assert_ne!(lowered.nodes()[0].key(), lowered.nodes()[1].key());
    }

    #[test]
    fn typed_and_dynamic_definitions_compile_to_identical_behavior() {
        let domain = TimeDomainId::from_u128(10);
        let network = NetworkKey::from_u128(1);
        let input = ExternalInputKey::from_u128(2);
        let not = NodeKey::from_u128(3);
        let not_input = InPortKey::from_u128(4);
        let not_output = OutPortKey::from_u128(5);
        let output = ExternalOutputKey::from_u128(6);

        let mut typed = NetworkBuilder::<()>::with_key(network, domain);
        let typed_input = typed
            .add_level_input(ExternalInputKey::from_u128(2), DiagnosticMeta::default())
            .unwrap();
        let typed_not = typed
            .add_not_with_ports(
                not,
                not_input,
                not_output,
                typed_input,
                DiagnosticMeta::default(),
            )
            .unwrap()
            .into_outputs();
        typed
            .add_level_output(output, typed_not, DiagnosticMeta::default())
            .unwrap();
        let typed = typed.finish().require_artifact().unwrap();

        let dynamic = UncheckedNetwork::new(
            network,
            domain,
            DiagnosticMeta::default(),
            vec![NodeDef::new(
                not,
                NodeKind::<()>::not(),
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
                crate::key::ConnectionKey::from_u128(0),
                input.into(),
                not_input.into(),
                DiagnosticMeta::default(),
            )],
        )
        .validate()
        .require_artifact()
        .unwrap();

        assert_eq!(typed.fingerprint(), dynamic.fingerprint());
        assert_eq!(
            typed.input_schema_fingerprint(),
            dynamic.input_schema_fingerprint()
        );
        assert_eq!(typed.time_domain_id(), domain);
        assert_eq!(dynamic.time_domain_id(), domain);
        let typed = typed.compile().require_artifact().unwrap();
        let dynamic = dynamic.compile().require_artifact().unwrap();
        assert_eq!(typed.fingerprint(), dynamic.fingerprint());
        assert_eq!(typed.time_domain_id(), domain);
        assert_eq!(dynamic.time_domain_id(), domain);
        let inputs = BTreeMap::from([(input, LogicLevel::High)]);
        assert_eq!(
            typed.evaluate_full(&inputs).unwrap().external_outputs,
            dynamic.evaluate_full(&inputs).unwrap().external_outputs
        );
    }

    #[test]
    fn typed_and_dynamic_all_definitions_have_identical_identity_and_behavior() {
        let domain = TimeDomainId::from_u128(10);
        let network = NetworkKey::from_u128(1);
        let first = ExternalInputKey::from_u128(2);
        let second = ExternalInputKey::from_u128(3);
        let node = NodeKey::from_u128(4);
        let first_port = InPortKey::from_u128(5);
        let second_port = InPortKey::from_u128(6);
        let node_output = OutPortKey::from_u128(7);
        let output = ExternalOutputKey::from_u128(8);

        let mut typed = NetworkBuilder::<()>::with_key(network, domain);
        let first_signal = typed
            .add_level_input(first, DiagnosticMeta::default())
            .unwrap();
        let second_signal = typed
            .add_level_input(second, DiagnosticMeta::default())
            .unwrap();
        let all = typed
            .add_all_with_ports(
                node,
                node_output,
                [(first_port, first_signal), (second_port, second_signal)],
                DiagnosticMeta::default(),
            )
            .unwrap()
            .into_outputs();
        typed
            .add_level_output(output, all, DiagnosticMeta::default())
            .unwrap();
        let typed = typed.finish().require_artifact().unwrap();

        let dynamic = UncheckedNetwork::new(
            network,
            domain,
            DiagnosticMeta::default(),
            vec![NodeDef::new(
                node,
                NodeKind::<()>::all(),
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
                output.into(),
                SignalSourceKey::NodeOutput(node_output).into(),
                DiagnosticMeta::default(),
            )],
            vec![
                ConnectionDef::new(
                    crate::key::ConnectionKey::from_u128(0),
                    first.into(),
                    first_port.into(),
                    DiagnosticMeta::default(),
                ),
                ConnectionDef::new(
                    crate::key::ConnectionKey::from_u128(1),
                    second.into(),
                    second_port.into(),
                    DiagnosticMeta::default(),
                ),
            ],
        )
        .validate()
        .require_artifact()
        .unwrap();

        assert_eq!(typed.fingerprint(), dynamic.fingerprint());
        assert_eq!(
            typed.reaction_dependencies(),
            dynamic.reaction_dependencies()
        );
        assert_eq!(typed.topological_order(), dynamic.topological_order());
        let typed = typed.compile().require_artifact().unwrap();
        let dynamic = dynamic.compile().require_artifact().unwrap();
        for first_level in [LogicLevel::Low, LogicLevel::High] {
            for second_level in [LogicLevel::Low, LogicLevel::High] {
                let inputs = BTreeMap::from([(first, first_level), (second, second_level)]);
                assert_eq!(
                    typed.evaluate_full(&inputs).unwrap().external_outputs,
                    dynamic.evaluate_full(&inputs).unwrap().external_outputs
                );
            }
        }
    }

    #[test]
    fn convenience_allocation_is_deterministic() {
        let build = || {
            let mut builder = NetworkBuilder::<()>::new(TimeDomainId::from_u128(1));
            let input = builder.level_input("input");
            let output = builder.not(input.1).unwrap();
            let endpoint = builder.level_output("output", output).unwrap();
            (input.0, endpoint, builder.into_unchecked())
        };
        let left = build();
        let right = build();
        assert_eq!(left.0, right.0);
        assert_eq!(left.1, right.1);
        assert_eq!(left.2, right.2);
    }
}
