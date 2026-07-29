//! Typed authoring for the restricted Level and Pulse network foundation.

use crate::authored::{
    ConnectionDef, ConnectionEndpoint, ExternalInputDef, ExternalOutputDef, ModuleInputDef,
    ModuleInterfaceMapping, ModuleOutputDef, NodeDef, NodeKind, NodePorts, PulseDelayConfig,
    ToggleConfig, UncheckedModule, UncheckedNetwork,
};
use crate::diagnostics::Report;
use crate::identity::TimeDomainId;
use crate::key::{
    AnyExternalInputKey, ExternalInputKey, ExternalOutputKey, InPortKey, KeyAllocator,
    ModuleInputKey, ModuleOutputKey, NetworkKey, NodeKey, OutPortKey, SignalSourceKey,
};
use crate::metadata::DiagnosticMeta;
use crate::signal::{Level, LogicLevel, Pulse, SignalType};
use crate::{ModuleDef, ValidatedNetwork};
use core::sync::atomic::{AtomicU64, Ordering};
use std::collections::BTreeSet;

static NEXT_BUILDER_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Copy)]
enum VariadicNodeKind {
    All,
    Any,
    Parity,
    AtLeast(u64),
}

impl VariadicNodeKind {
    fn authored<D>(self) -> NodeKind<D> {
        match self {
            Self::All => NodeKind::all(),
            Self::Any => NodeKind::any(),
            Self::Parity => NodeKind::parity(),
            Self::AtLeast(threshold) => NodeKind::at_least(threshold),
        }
    }
}

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
    /// An explicit pulse input key is already present in this builder.
    DuplicatePulseExternalInputKey(ExternalInputKey<Pulse>),
    /// An explicit pulse output key is already present in this builder.
    DuplicatePulseExternalOutputKey(ExternalOutputKey<Pulse>),
    /// An explicit level module-input key is already present in this builder.
    DuplicateModuleInputKey(ModuleInputKey<Level>),
    /// An explicit level module-output key is already present in this builder.
    DuplicateModuleOutputKey(ModuleOutputKey<Level>),
    /// An explicit pulse module-input key is already present in this builder.
    DuplicatePulseModuleInputKey(ModuleInputKey<Pulse>),
    /// An explicit pulse module-output key is already present in this builder.
    DuplicatePulseModuleOutputKey(ModuleOutputKey<Pulse>),
    /// An explicit node key is already present in this builder.
    DuplicateNodeKey(NodeKey),
    /// An explicit input-port key is already present in this builder.
    DuplicateInPortKey(InPortKey<Level>),
    /// An explicit output-port key is already present in this builder.
    DuplicateOutPortKey(OutPortKey<Level>),
    /// An explicit pulse input-port key is already present in this builder.
    DuplicatePulseInPortKey(InPortKey<Pulse>),
    /// An explicit pulse output-port key is already present in this builder.
    DuplicatePulseOutPortKey(OutPortKey<Pulse>),
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
    pulse_input_keys: BTreeSet<ExternalInputKey<Pulse>>,
    pulse_output_keys: BTreeSet<ExternalOutputKey<Pulse>>,
    pulse_in_port_keys: BTreeSet<InPortKey<Pulse>>,
    pulse_out_port_keys: BTreeSet<OutPortKey<Pulse>>,
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
            pulse_input_keys: BTreeSet::new(),
            pulse_output_keys: BTreeSet::new(),
            pulse_in_port_keys: BTreeSet::new(),
            pulse_out_port_keys: BTreeSet::new(),
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

    /// Adds a convenience pulse input with locally allocated stable identity.
    pub fn pulse_input(
        &mut self,
        name: impl Into<String>,
    ) -> (ExternalInputKey<Pulse>, Signal<Pulse>) {
        let key = self.next_pulse_external_input_key();
        let signal = match self.add_pulse_input(key, named_meta(name)) {
            Ok(signal) => signal,
            Err(_) => panic!("fresh pulse input key must not conflict with builder state"),
        };
        (key, signal)
    }

    /// Adds an explicit pulse input and retains its metadata unchanged.
    pub fn add_pulse_input(
        &mut self,
        key: ExternalInputKey<Pulse>,
        meta: DiagnosticMeta,
    ) -> Result<Signal<Pulse>, AuthoringFailure> {
        if !self.pulse_input_keys.insert(key) {
            return Err(AuthoringFailure::DuplicatePulseExternalInputKey(key));
        }
        self.external_inputs
            .push(ExternalInputDef::new(key.into(), meta));
        Ok(self.signal(SignalSourceKey::ExternalInput(key)))
    }

    /// Adds a convenience pulse output with locally allocated stable identity.
    pub fn pulse_output(
        &mut self,
        name: impl Into<String>,
        source: Signal<Pulse>,
    ) -> Result<ExternalOutputKey<Pulse>, AuthoringFailure> {
        let key = self.next_pulse_external_output_key();
        self.add_pulse_output(key, source, named_meta(name))?;
        Ok(key)
    }

    /// Adds an explicit pulse output and retains its metadata unchanged.
    pub fn add_pulse_output(
        &mut self,
        key: ExternalOutputKey<Pulse>,
        source: Signal<Pulse>,
        meta: DiagnosticMeta,
    ) -> Result<(), AuthoringFailure> {
        self.require_local(source)?;
        if !self.pulse_output_keys.insert(key) {
            return Err(AuthoringFailure::DuplicatePulseExternalOutputKey(key));
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
        self.variadic(inputs, VariadicNodeKind::All)
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
        self.add_variadic(key, inputs, meta, VariadicNodeKind::All)
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
        self.add_variadic_with_ports(key, output, inputs, meta, VariadicNodeKind::All)
    }

    /// Adds a variadic pulse Merge with locally allocated stable identities.
    pub fn merge<I>(&mut self, inputs: I) -> Result<Signal<Pulse>, AuthoringFailure>
    where
        I: IntoIterator<Item = Signal<Pulse>>,
    {
        self.pulse_variadic(inputs)
    }

    /// Adds an explicitly keyed Merge with locally allocated port identities.
    pub fn add_merge<I>(
        &mut self,
        key: NodeKey,
        inputs: I,
        meta: DiagnosticMeta,
    ) -> Result<AddedNode<Signal<Pulse>>, AuthoringFailure>
    where
        I: IntoIterator<Item = Signal<Pulse>>,
    {
        self.add_pulse_variadic(key, inputs, meta)
    }

    /// Adds an explicitly keyed Merge with exact stable pulse-port identities.
    pub fn add_merge_with_ports<I>(
        &mut self,
        key: NodeKey,
        output: OutPortKey<Pulse>,
        inputs: I,
        meta: DiagnosticMeta,
    ) -> Result<AddedNode<Signal<Pulse>>, AuthoringFailure>
    where
        I: IntoIterator<Item = (InPortKey<Pulse>, Signal<Pulse>)>,
    {
        self.add_pulse_variadic_with_ports(key, output, inputs, meta)
    }

    /// Adds a pulse-controlled Toggle with locally allocated stable identities.
    pub fn toggle(
        &mut self,
        input: Signal<Pulse>,
        config: ToggleConfig,
    ) -> Result<Signal<Level>, AuthoringFailure> {
        let key = self.next_node_key();
        let input_port = self.next_pulse_in_port_key();
        let output_port = self.next_out_port_key();
        Ok(self
            .add_toggle_with_ports(
                key,
                input_port,
                output_port,
                input,
                config,
                DiagnosticMeta::default(),
            )?
            .into_outputs())
    }

    /// Adds an explicitly keyed Toggle with locally allocated port identities.
    pub fn add_toggle(
        &mut self,
        key: NodeKey,
        input: Signal<Pulse>,
        config: ToggleConfig,
        meta: DiagnosticMeta,
    ) -> Result<AddedNode<Signal<Level>>, AuthoringFailure> {
        let input_port = self.next_pulse_in_port_key();
        let output_port = self.next_out_port_key();
        self.add_toggle_with_ports(key, input_port, output_port, input, config, meta)
    }

    /// Adds an explicitly keyed Toggle with exact fixed port identities.
    pub fn add_toggle_with_ports(
        &mut self,
        key: NodeKey,
        input_port: InPortKey<Pulse>,
        output_port: OutPortKey<Level>,
        input: Signal<Pulse>,
        config: ToggleConfig,
        meta: DiagnosticMeta,
    ) -> Result<AddedNode<Signal<Level>>, AuthoringFailure> {
        self.require_local(input)?;
        if self.node_keys.contains(&key) {
            return Err(AuthoringFailure::DuplicateNodeKey(key));
        }
        if self.pulse_in_port_keys.contains(&input_port) {
            return Err(AuthoringFailure::DuplicatePulseInPortKey(input_port));
        }
        if self.out_port_keys.contains(&output_port) {
            return Err(AuthoringFailure::DuplicateOutPortKey(output_port));
        }
        self.node_keys.insert(key);
        self.pulse_in_port_keys.insert(input_port);
        self.out_port_keys.insert(output_port);
        self.nodes.push(NodeDef::new(
            key,
            NodeKind::Toggle(config),
            NodePorts::with_input_roles(
                vec![input_port.into()],
                vec![crate::authored::InputPortRole::Toggle],
                vec![output_port.into()],
            ),
            meta,
        ));
        self.connections.push(ConnectionDef::new(
            self.allocator.connection(),
            source_endpoint_pulse(input.source),
            ConnectionEndpoint::node_input(input_port.into()),
            DiagnosticMeta::default(),
        ));
        Ok(AddedNode {
            key,
            outputs: self.signal(SignalSourceKey::NodeOutput(output_port)),
        })
    }

    /// Adds a PulseDelay with locally allocated stable identities.
    pub fn pulse_delay(
        &mut self,
        input: Signal<Pulse>,
        config: PulseDelayConfig<D>,
    ) -> Result<Signal<Pulse>, AuthoringFailure> {
        let key = self.next_node_key();
        let input_port = self.next_pulse_in_port_key();
        let output_port = self.next_pulse_out_port_key();
        Ok(self
            .add_pulse_delay_with_ports(
                key,
                input_port,
                output_port,
                input,
                config,
                DiagnosticMeta::default(),
            )?
            .into_outputs())
    }

    /// Adds an explicitly keyed PulseDelay with locally allocated port identities.
    pub fn add_pulse_delay(
        &mut self,
        key: NodeKey,
        input: Signal<Pulse>,
        config: PulseDelayConfig<D>,
        meta: DiagnosticMeta,
    ) -> Result<AddedNode<Signal<Pulse>>, AuthoringFailure> {
        let input_port = self.next_pulse_in_port_key();
        let output_port = self.next_pulse_out_port_key();
        self.add_pulse_delay_with_ports(key, input_port, output_port, input, config, meta)
    }

    /// Adds an explicitly keyed PulseDelay with exact fixed port identities.
    pub fn add_pulse_delay_with_ports(
        &mut self,
        key: NodeKey,
        input_port: InPortKey<Pulse>,
        output_port: OutPortKey<Pulse>,
        input: Signal<Pulse>,
        config: PulseDelayConfig<D>,
        meta: DiagnosticMeta,
    ) -> Result<AddedNode<Signal<Pulse>>, AuthoringFailure> {
        self.require_local(input)?;
        if self.node_keys.contains(&key) {
            return Err(AuthoringFailure::DuplicateNodeKey(key));
        }
        if self.pulse_in_port_keys.contains(&input_port) {
            return Err(AuthoringFailure::DuplicatePulseInPortKey(input_port));
        }
        if self.pulse_out_port_keys.contains(&output_port) {
            return Err(AuthoringFailure::DuplicatePulseOutPortKey(output_port));
        }
        self.node_keys.insert(key);
        self.pulse_in_port_keys.insert(input_port);
        self.pulse_out_port_keys.insert(output_port);
        self.nodes.push(NodeDef::new(
            key,
            NodeKind::PulseDelay(config),
            NodePorts::with_input_roles(
                vec![input_port.into()],
                vec![crate::authored::InputPortRole::PulseDelay],
                vec![output_port.into()],
            ),
            meta,
        ));
        self.connections.push(ConnectionDef::new(
            self.allocator.connection(),
            source_endpoint_pulse(input.source),
            ConnectionEndpoint::node_input(input_port.into()),
            DiagnosticMeta::default(),
        ));
        Ok(AddedNode {
            key,
            outputs: self.signal(SignalSourceKey::NodeOutput(output_port)),
        })
    }

    /// Adds a variadic level disjunction with locally allocated stable identities.
    pub fn any<I>(&mut self, inputs: I) -> Result<Signal<Level>, AuthoringFailure>
    where
        I: IntoIterator<Item = Signal<Level>>,
    {
        self.variadic(inputs, VariadicNodeKind::Any)
    }

    /// Adds an explicitly keyed disjunction with locally allocated port identities.
    pub fn add_any<I>(
        &mut self,
        key: NodeKey,
        inputs: I,
        meta: DiagnosticMeta,
    ) -> Result<AddedNode<Signal<Level>>, AuthoringFailure>
    where
        I: IntoIterator<Item = Signal<Level>>,
    {
        self.add_variadic(key, inputs, meta, VariadicNodeKind::Any)
    }

    /// Adds an explicitly keyed disjunction with exact stable port identities.
    pub fn add_any_with_ports<I>(
        &mut self,
        key: NodeKey,
        output: OutPortKey<Level>,
        inputs: I,
        meta: DiagnosticMeta,
    ) -> Result<AddedNode<Signal<Level>>, AuthoringFailure>
    where
        I: IntoIterator<Item = (InPortKey<Level>, Signal<Level>)>,
    {
        self.add_variadic_with_ports(key, output, inputs, meta, VariadicNodeKind::Any)
    }

    /// Adds a variadic odd-parity node with locally allocated stable identities.
    pub fn parity<I>(&mut self, inputs: I) -> Result<Signal<Level>, AuthoringFailure>
    where
        I: IntoIterator<Item = Signal<Level>>,
    {
        self.variadic(inputs, VariadicNodeKind::Parity)
    }

    /// Adds an explicitly keyed parity node with locally allocated port identities.
    pub fn add_parity<I>(
        &mut self,
        key: NodeKey,
        inputs: I,
        meta: DiagnosticMeta,
    ) -> Result<AddedNode<Signal<Level>>, AuthoringFailure>
    where
        I: IntoIterator<Item = Signal<Level>>,
    {
        self.add_variadic(key, inputs, meta, VariadicNodeKind::Parity)
    }

    /// Adds an explicitly keyed parity node with exact stable port identities.
    pub fn add_parity_with_ports<I>(
        &mut self,
        key: NodeKey,
        output: OutPortKey<Level>,
        inputs: I,
        meta: DiagnosticMeta,
    ) -> Result<AddedNode<Signal<Level>>, AuthoringFailure>
    where
        I: IntoIterator<Item = (InPortKey<Level>, Signal<Level>)>,
    {
        self.add_variadic_with_ports(key, output, inputs, meta, VariadicNodeKind::Parity)
    }

    /// Adds a variadic threshold node with locally allocated stable identities.
    pub fn at_least<I>(
        &mut self,
        threshold: u64,
        inputs: I,
    ) -> Result<Signal<Level>, AuthoringFailure>
    where
        I: IntoIterator<Item = Signal<Level>>,
    {
        self.variadic(inputs, VariadicNodeKind::AtLeast(threshold))
    }

    /// Adds an explicitly keyed threshold node with locally allocated port identities.
    pub fn add_at_least<I>(
        &mut self,
        key: NodeKey,
        threshold: u64,
        inputs: I,
        meta: DiagnosticMeta,
    ) -> Result<AddedNode<Signal<Level>>, AuthoringFailure>
    where
        I: IntoIterator<Item = Signal<Level>>,
    {
        self.add_variadic(key, inputs, meta, VariadicNodeKind::AtLeast(threshold))
    }

    /// Adds an explicitly keyed threshold node with exact stable port identities.
    pub fn add_at_least_with_ports<I>(
        &mut self,
        key: NodeKey,
        output: OutPortKey<Level>,
        threshold: u64,
        inputs: I,
        meta: DiagnosticMeta,
    ) -> Result<AddedNode<Signal<Level>>, AuthoringFailure>
    where
        I: IntoIterator<Item = (InPortKey<Level>, Signal<Level>)>,
    {
        self.add_variadic_with_ports(
            key,
            output,
            inputs,
            meta,
            VariadicNodeKind::AtLeast(threshold),
        )
    }

    /// Adds a fixed level branch selector with locally allocated identities.
    pub fn select(
        &mut self,
        selector: Signal<Level>,
        when_low: Signal<Level>,
        when_high: Signal<Level>,
    ) -> Result<Signal<Level>, AuthoringFailure> {
        self.require_all_local(&[selector, when_low, when_high])?;
        let key = self.next_node_key();
        let selector_port = self.next_in_port_key();
        let when_low_port = self.next_in_port_key();
        let when_high_port = self.next_in_port_key();
        let output = self.next_out_port_key();
        Ok(self
            .add_select_with_ports(
                key,
                selector_port,
                when_low_port,
                when_high_port,
                output,
                selector,
                when_low,
                when_high,
                DiagnosticMeta::default(),
            )?
            .into_outputs())
    }

    /// Adds an explicitly keyed selector with locally allocated port identities.
    pub fn add_select(
        &mut self,
        key: NodeKey,
        selector: Signal<Level>,
        when_low: Signal<Level>,
        when_high: Signal<Level>,
        meta: DiagnosticMeta,
    ) -> Result<AddedNode<Signal<Level>>, AuthoringFailure> {
        self.require_all_local(&[selector, when_low, when_high])?;
        let selector_port = self.next_in_port_key();
        let when_low_port = self.next_in_port_key();
        let when_high_port = self.next_in_port_key();
        let output = self.next_out_port_key();
        self.add_select_with_ports(
            key,
            selector_port,
            when_low_port,
            when_high_port,
            output,
            selector,
            when_low,
            when_high,
            meta,
        )
    }

    /// Adds an explicitly keyed selector with exact stable port identities.
    #[allow(clippy::too_many_arguments)]
    pub fn add_select_with_ports(
        &mut self,
        key: NodeKey,
        selector_port: InPortKey<Level>,
        when_low_port: InPortKey<Level>,
        when_high_port: InPortKey<Level>,
        output: OutPortKey<Level>,
        selector: Signal<Level>,
        when_low: Signal<Level>,
        when_high: Signal<Level>,
        meta: DiagnosticMeta,
    ) -> Result<AddedNode<Signal<Level>>, AuthoringFailure> {
        self.require_all_local(&[selector, when_low, when_high])?;
        self.insert_variadic_node_keys(
            key,
            [selector_port, when_low_port, when_high_port],
            output,
        )?;
        self.nodes.push(NodeDef::new(
            key,
            NodeKind::select(),
            NodePorts::with_input_roles(
                vec![
                    selector_port.into(),
                    when_low_port.into(),
                    when_high_port.into(),
                ],
                vec![
                    crate::authored::InputPortRole::Selector,
                    crate::authored::InputPortRole::WhenLow,
                    crate::authored::InputPortRole::WhenHigh,
                ],
                vec![output.into()],
            ),
            meta,
        ));
        for (port, input) in [
            (selector_port, selector),
            (when_low_port, when_low),
            (when_high_port, when_high),
        ] {
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

    fn variadic<I>(
        &mut self,
        inputs: I,
        kind: VariadicNodeKind,
    ) -> Result<Signal<Level>, AuthoringFailure>
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
        match self.add_variadic_with_ports(
            key,
            output,
            keyed_inputs,
            DiagnosticMeta::default(),
            kind,
        ) {
            Ok(added) => Ok(added.into_outputs()),
            Err(_) => panic!("fresh allocator keys must not conflict with builder state"),
        }
    }

    fn add_variadic<I>(
        &mut self,
        key: NodeKey,
        inputs: I,
        meta: DiagnosticMeta,
        kind: VariadicNodeKind,
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
        self.add_variadic_with_ports(key, output, keyed_inputs, meta, kind)
    }

    fn add_variadic_with_ports<I>(
        &mut self,
        key: NodeKey,
        output: OutPortKey<Level>,
        inputs: I,
        meta: DiagnosticMeta,
        kind: VariadicNodeKind,
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
            kind.authored(),
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

    fn pulse_variadic<I>(&mut self, inputs: I) -> Result<Signal<Pulse>, AuthoringFailure>
    where
        I: IntoIterator<Item = Signal<Pulse>>,
    {
        let inputs = inputs.into_iter().collect::<Vec<_>>();
        self.require_all_local(&inputs)?;
        let key = self.next_node_key();
        let output = self.next_pulse_out_port_key();
        let mut keyed_inputs = Vec::with_capacity(inputs.len());
        for input in inputs {
            keyed_inputs.push((self.next_pulse_in_port_key(), input));
        }
        match self.add_pulse_variadic_with_ports(
            key,
            output,
            keyed_inputs,
            DiagnosticMeta::default(),
        ) {
            Ok(added) => Ok(added.into_outputs()),
            Err(_) => panic!("fresh pulse node keys must not conflict with builder state"),
        }
    }

    fn add_pulse_variadic<I>(
        &mut self,
        key: NodeKey,
        inputs: I,
        meta: DiagnosticMeta,
    ) -> Result<AddedNode<Signal<Pulse>>, AuthoringFailure>
    where
        I: IntoIterator<Item = Signal<Pulse>>,
    {
        let inputs = inputs.into_iter().collect::<Vec<_>>();
        self.require_all_local(&inputs)?;
        let output = self.next_pulse_out_port_key();
        let mut keyed_inputs = Vec::with_capacity(inputs.len());
        for input in inputs {
            keyed_inputs.push((self.next_pulse_in_port_key(), input));
        }
        self.add_pulse_variadic_with_ports(key, output, keyed_inputs, meta)
    }

    fn add_pulse_variadic_with_ports<I>(
        &mut self,
        key: NodeKey,
        output: OutPortKey<Pulse>,
        inputs: I,
        meta: DiagnosticMeta,
    ) -> Result<AddedNode<Signal<Pulse>>, AuthoringFailure>
    where
        I: IntoIterator<Item = (InPortKey<Pulse>, Signal<Pulse>)>,
    {
        let inputs = inputs.into_iter().collect::<Vec<_>>();
        for (_, input) in &inputs {
            self.require_local(*input)?;
        }
        self.insert_pulse_variadic_node_keys(key, inputs.iter().map(|(port, _)| *port), output)?;
        self.nodes.push(NodeDef::new(
            key,
            NodeKind::merge(),
            NodePorts::new(
                inputs.iter().map(|(port, _)| (*port).into()).collect(),
                vec![output.into()],
            ),
            meta,
        ));
        for (port, input) in inputs {
            self.connections.push(ConnectionDef::new(
                self.allocator.connection(),
                source_endpoint_pulse(input.source),
                ConnectionEndpoint::node_input(port.into()),
                DiagnosticMeta::default(),
            ));
        }
        Ok(AddedNode {
            key,
            outputs: self.signal(SignalSourceKey::NodeOutput(output)),
        })
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

    fn insert_pulse_variadic_node_keys<I>(
        &mut self,
        key: NodeKey,
        inputs: I,
        output: OutPortKey<Pulse>,
    ) -> Result<(), AuthoringFailure>
    where
        I: IntoIterator<Item = InPortKey<Pulse>>,
    {
        if self.node_keys.contains(&key) {
            return Err(AuthoringFailure::DuplicateNodeKey(key));
        }
        let inputs = inputs.into_iter().collect::<Vec<_>>();
        let mut proposed = BTreeSet::new();
        for input in &inputs {
            if self.pulse_in_port_keys.contains(input) || !proposed.insert(*input) {
                return Err(AuthoringFailure::DuplicatePulseInPortKey(*input));
            }
        }
        if self.pulse_out_port_keys.contains(&output) {
            return Err(AuthoringFailure::DuplicatePulseOutPortKey(output));
        }
        self.node_keys.insert(key);
        self.pulse_in_port_keys.extend(inputs);
        self.pulse_out_port_keys.insert(output);
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

    fn next_pulse_in_port_key(&mut self) -> InPortKey<Pulse> {
        loop {
            let key = self.allocator.in_port();
            if !self.pulse_in_port_keys.contains(&key) {
                return key;
            }
        }
    }

    fn next_pulse_out_port_key(&mut self) -> OutPortKey<Pulse> {
        loop {
            let key = self.allocator.out_port();
            if !self.pulse_out_port_keys.contains(&key) {
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

    fn next_pulse_external_input_key(&mut self) -> ExternalInputKey<Pulse> {
        loop {
            let key = self.allocator.external_input();
            if !self.pulse_input_keys.contains(&key) {
                return key;
            }
        }
    }

    fn next_pulse_external_output_key(&mut self) -> ExternalOutputKey<Pulse> {
        loop {
            let key = self.allocator.external_output();
            if !self.pulse_output_keys.contains(&key) {
                return key;
            }
        }
    }
}

/// An owning, typed authoring surface for one reusable caller-authored module.
///
/// Signals are scoped to this builder. The builder exposes a typed module
/// interface while reusing the ordinary primitive construction and authored
/// graph representation used by [`NetworkBuilder`].
///
/// Signal kinds remain part of the typed interface:
///
/// ```compile_fail
/// use mossignal::ModuleBuilder;
///
/// let mut module = ModuleBuilder::<()>::new();
/// let (_, pulse) = module.pulse_input("pulse");
/// let _ = module.level_output("wrong kind", pulse);
/// ```
pub struct ModuleBuilder<D> {
    meta: DiagnosticMeta,
    interface_allocator: KeyAllocator,
    graph: NetworkBuilder<D>,
    inputs: Vec<ModuleInputDef>,
    outputs: Vec<ModuleOutputDef>,
    mappings: Vec<ModuleInterfaceMapping>,
    level_input_keys: BTreeSet<ModuleInputKey<Level>>,
    pulse_input_keys: BTreeSet<ModuleInputKey<Pulse>>,
    level_output_keys: BTreeSet<ModuleOutputKey<Level>>,
    pulse_output_keys: BTreeSet<ModuleOutputKey<Pulse>>,
}

impl<D> ModuleBuilder<D> {
    /// Creates an empty caller-authored module with deterministic local allocation.
    #[must_use]
    pub fn new() -> Self {
        Self::with_meta(DiagnosticMeta::default())
    }

    /// Creates an empty caller-authored module with retained diagnostic metadata.
    #[must_use]
    pub fn with_meta(meta: DiagnosticMeta) -> Self {
        Self {
            meta,
            interface_allocator: KeyAllocator::new(0),
            graph: NetworkBuilder::new(TimeDomainId::from_u128(0)),
            inputs: Vec::new(),
            outputs: Vec::new(),
            mappings: Vec::new(),
            level_input_keys: BTreeSet::new(),
            pulse_input_keys: BTreeSet::new(),
            level_output_keys: BTreeSet::new(),
            pulse_output_keys: BTreeSet::new(),
        }
    }

    /// Adds a convenience level input with locally allocated stable identity.
    pub fn level_input(
        &mut self,
        name: impl Into<String>,
    ) -> (ModuleInputKey<Level>, Signal<Level>) {
        let key = self.next_level_input_key();
        let signal = match self.add_level_input(key, named_meta(name)) {
            Ok(signal) => signal,
            Err(_) => panic!("fresh module input key must not conflict with builder state"),
        };
        (key, signal)
    }

    /// Adds an explicit level module input and retains its metadata unchanged.
    pub fn add_level_input(
        &mut self,
        key: ModuleInputKey<Level>,
        meta: DiagnosticMeta,
    ) -> Result<Signal<Level>, AuthoringFailure> {
        if self.level_input_keys.contains(&key) {
            return Err(AuthoringFailure::DuplicateModuleInputKey(key));
        }
        let surrogate = ExternalInputKey::from_u128(key.as_u128());
        let signal = match self.graph.add_level_input(surrogate, meta.clone()) {
            Ok(signal) => signal,
            Err(_) => panic!("unique module input must have a unique private source surrogate"),
        };
        self.level_input_keys.insert(key);
        self.inputs.push(ModuleInputDef::new(key.into(), meta));
        Ok(signal)
    }

    /// Adds a convenience pulse input with locally allocated stable identity.
    pub fn pulse_input(
        &mut self,
        name: impl Into<String>,
    ) -> (ModuleInputKey<Pulse>, Signal<Pulse>) {
        let key = self.next_pulse_input_key();
        let signal = match self.add_pulse_input(key, named_meta(name)) {
            Ok(signal) => signal,
            Err(_) => panic!("fresh pulse module input key must not conflict with builder state"),
        };
        (key, signal)
    }

    /// Adds an explicit pulse module input and retains its metadata unchanged.
    pub fn add_pulse_input(
        &mut self,
        key: ModuleInputKey<Pulse>,
        meta: DiagnosticMeta,
    ) -> Result<Signal<Pulse>, AuthoringFailure> {
        if self.pulse_input_keys.contains(&key) {
            return Err(AuthoringFailure::DuplicatePulseModuleInputKey(key));
        }
        let surrogate = ExternalInputKey::from_u128(key.as_u128());
        let signal = match self.graph.add_pulse_input(surrogate, meta.clone()) {
            Ok(signal) => signal,
            Err(_) => {
                panic!("unique pulse module input must have a unique private source surrogate")
            }
        };
        self.pulse_input_keys.insert(key);
        self.inputs.push(ModuleInputDef::new(key.into(), meta));
        Ok(signal)
    }

    /// Adds a convenience level output with locally allocated stable identity.
    pub fn level_output(
        &mut self,
        name: impl Into<String>,
        source: Signal<Level>,
    ) -> Result<ModuleOutputKey<Level>, AuthoringFailure> {
        self.graph.require_local(source)?;
        let key = self.next_level_output_key();
        self.add_level_output(key, source, named_meta(name))?;
        Ok(key)
    }

    /// Adds an explicit level module output and retains its metadata unchanged.
    pub fn add_level_output(
        &mut self,
        key: ModuleOutputKey<Level>,
        source: Signal<Level>,
        meta: DiagnosticMeta,
    ) -> Result<(), AuthoringFailure> {
        self.graph.require_local(source)?;
        if !self.level_output_keys.insert(key) {
            return Err(AuthoringFailure::DuplicateModuleOutputKey(key));
        }
        self.outputs.push(ModuleOutputDef::new(key.into(), meta));
        self.mappings.push(ModuleInterfaceMapping::output(
            key.into(),
            source_endpoint(source.source),
        ));
        Ok(())
    }

    /// Adds a convenience pulse output with locally allocated stable identity.
    pub fn pulse_output(
        &mut self,
        name: impl Into<String>,
        source: Signal<Pulse>,
    ) -> Result<ModuleOutputKey<Pulse>, AuthoringFailure> {
        self.graph.require_local(source)?;
        let key = self.next_pulse_output_key();
        self.add_pulse_output(key, source, named_meta(name))?;
        Ok(key)
    }

    /// Adds an explicit pulse module output and retains its metadata unchanged.
    pub fn add_pulse_output(
        &mut self,
        key: ModuleOutputKey<Pulse>,
        source: Signal<Pulse>,
        meta: DiagnosticMeta,
    ) -> Result<(), AuthoringFailure> {
        self.graph.require_local(source)?;
        if !self.pulse_output_keys.insert(key) {
            return Err(AuthoringFailure::DuplicatePulseModuleOutputKey(key));
        }
        self.outputs.push(ModuleOutputDef::new(key.into(), meta));
        self.mappings.push(ModuleInterfaceMapping::output(
            key.into(),
            source_endpoint_pulse(source.source),
        ));
        Ok(())
    }

    /// Adds an infallible level constant with locally allocated identity.
    pub fn constant(&mut self, value: LogicLevel) -> Signal<Level> {
        self.graph.constant(value)
    }

    /// Adds an explicit constant node with a locally allocated output port.
    pub fn add_constant(
        &mut self,
        key: NodeKey,
        value: LogicLevel,
        meta: DiagnosticMeta,
    ) -> Result<AddedNode<Signal<Level>>, AuthoringFailure> {
        self.graph.add_constant(key, value, meta)
    }

    /// Adds an explicit constant node with an exact output-port identity.
    pub fn add_constant_with_output(
        &mut self,
        key: NodeKey,
        output: OutPortKey<Level>,
        value: LogicLevel,
        meta: DiagnosticMeta,
    ) -> Result<AddedNode<Signal<Level>>, AuthoringFailure> {
        self.graph
            .add_constant_with_output(key, output, value, meta)
    }

    /// Adds a level inverter with locally allocated identities.
    pub fn not(&mut self, input: Signal<Level>) -> Result<Signal<Level>, AuthoringFailure> {
        self.graph.require_local(input)?;
        self.graph.not(input)
    }

    /// Adds an explicit inverter with locally allocated port identities.
    pub fn add_not(
        &mut self,
        key: NodeKey,
        input: Signal<Level>,
        meta: DiagnosticMeta,
    ) -> Result<AddedNode<Signal<Level>>, AuthoringFailure> {
        self.graph.require_local(input)?;
        self.graph.add_not(key, input, meta)
    }

    /// Adds an explicit inverter with exact port identities.
    pub fn add_not_with_ports(
        &mut self,
        key: NodeKey,
        input_port: InPortKey<Level>,
        output_port: OutPortKey<Level>,
        input: Signal<Level>,
        meta: DiagnosticMeta,
    ) -> Result<AddedNode<Signal<Level>>, AuthoringFailure> {
        self.graph.require_local(input)?;
        self.graph
            .add_not_with_ports(key, input_port, output_port, input, meta)
    }

    /// Adds a variadic level conjunction.
    pub fn all<I>(&mut self, inputs: I) -> Result<Signal<Level>, AuthoringFailure>
    where
        I: IntoIterator<Item = Signal<Level>>,
    {
        let inputs = inputs.into_iter().collect::<Vec<_>>();
        self.graph.require_all_local(&inputs)?;
        self.graph.all(inputs)
    }

    /// Adds an explicitly keyed conjunction with locally allocated ports.
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
        self.graph.require_all_local(&inputs)?;
        self.graph.add_all(key, inputs, meta)
    }

    /// Adds an explicitly keyed conjunction with exact ports.
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
            self.graph.require_local(*input)?;
        }
        self.graph.add_all_with_ports(key, output, inputs, meta)
    }

    /// Adds a variadic level disjunction.
    pub fn any<I>(&mut self, inputs: I) -> Result<Signal<Level>, AuthoringFailure>
    where
        I: IntoIterator<Item = Signal<Level>>,
    {
        let inputs = inputs.into_iter().collect::<Vec<_>>();
        self.graph.require_all_local(&inputs)?;
        self.graph.any(inputs)
    }

    /// Adds an explicitly keyed disjunction with locally allocated ports.
    pub fn add_any<I>(
        &mut self,
        key: NodeKey,
        inputs: I,
        meta: DiagnosticMeta,
    ) -> Result<AddedNode<Signal<Level>>, AuthoringFailure>
    where
        I: IntoIterator<Item = Signal<Level>>,
    {
        let inputs = inputs.into_iter().collect::<Vec<_>>();
        self.graph.require_all_local(&inputs)?;
        self.graph.add_any(key, inputs, meta)
    }

    /// Adds an explicitly keyed disjunction with exact ports.
    pub fn add_any_with_ports<I>(
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
            self.graph.require_local(*input)?;
        }
        self.graph.add_any_with_ports(key, output, inputs, meta)
    }

    /// Adds a variadic odd-parity node.
    pub fn parity<I>(&mut self, inputs: I) -> Result<Signal<Level>, AuthoringFailure>
    where
        I: IntoIterator<Item = Signal<Level>>,
    {
        let inputs = inputs.into_iter().collect::<Vec<_>>();
        self.graph.require_all_local(&inputs)?;
        self.graph.parity(inputs)
    }

    /// Adds an explicitly keyed parity node with locally allocated ports.
    pub fn add_parity<I>(
        &mut self,
        key: NodeKey,
        inputs: I,
        meta: DiagnosticMeta,
    ) -> Result<AddedNode<Signal<Level>>, AuthoringFailure>
    where
        I: IntoIterator<Item = Signal<Level>>,
    {
        let inputs = inputs.into_iter().collect::<Vec<_>>();
        self.graph.require_all_local(&inputs)?;
        self.graph.add_parity(key, inputs, meta)
    }

    /// Adds an explicitly keyed parity node with exact ports.
    pub fn add_parity_with_ports<I>(
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
            self.graph.require_local(*input)?;
        }
        self.graph.add_parity_with_ports(key, output, inputs, meta)
    }

    /// Adds a variadic threshold node.
    pub fn at_least<I>(
        &mut self,
        threshold: u64,
        inputs: I,
    ) -> Result<Signal<Level>, AuthoringFailure>
    where
        I: IntoIterator<Item = Signal<Level>>,
    {
        let inputs = inputs.into_iter().collect::<Vec<_>>();
        self.graph.require_all_local(&inputs)?;
        self.graph.at_least(threshold, inputs)
    }

    /// Adds an explicitly keyed threshold node with locally allocated ports.
    pub fn add_at_least<I>(
        &mut self,
        key: NodeKey,
        threshold: u64,
        inputs: I,
        meta: DiagnosticMeta,
    ) -> Result<AddedNode<Signal<Level>>, AuthoringFailure>
    where
        I: IntoIterator<Item = Signal<Level>>,
    {
        let inputs = inputs.into_iter().collect::<Vec<_>>();
        self.graph.require_all_local(&inputs)?;
        self.graph.add_at_least(key, threshold, inputs, meta)
    }

    /// Adds an explicitly keyed threshold node with exact ports.
    pub fn add_at_least_with_ports<I>(
        &mut self,
        key: NodeKey,
        output: OutPortKey<Level>,
        threshold: u64,
        inputs: I,
        meta: DiagnosticMeta,
    ) -> Result<AddedNode<Signal<Level>>, AuthoringFailure>
    where
        I: IntoIterator<Item = (InPortKey<Level>, Signal<Level>)>,
    {
        let inputs = inputs.into_iter().collect::<Vec<_>>();
        for (_, input) in &inputs {
            self.graph.require_local(*input)?;
        }
        self.graph
            .add_at_least_with_ports(key, output, threshold, inputs, meta)
    }

    /// Adds a fixed level branch selector.
    pub fn select(
        &mut self,
        selector: Signal<Level>,
        when_low: Signal<Level>,
        when_high: Signal<Level>,
    ) -> Result<Signal<Level>, AuthoringFailure> {
        self.graph
            .require_all_local(&[selector, when_low, when_high])?;
        self.graph.select(selector, when_low, when_high)
    }

    /// Adds an explicitly keyed selector with locally allocated ports.
    pub fn add_select(
        &mut self,
        key: NodeKey,
        selector: Signal<Level>,
        when_low: Signal<Level>,
        when_high: Signal<Level>,
        meta: DiagnosticMeta,
    ) -> Result<AddedNode<Signal<Level>>, AuthoringFailure> {
        self.graph
            .require_all_local(&[selector, when_low, when_high])?;
        self.graph
            .add_select(key, selector, when_low, when_high, meta)
    }

    /// Adds an explicitly keyed selector with exact ports.
    #[allow(clippy::too_many_arguments)]
    pub fn add_select_with_ports(
        &mut self,
        key: NodeKey,
        selector_port: InPortKey<Level>,
        when_low_port: InPortKey<Level>,
        when_high_port: InPortKey<Level>,
        output: OutPortKey<Level>,
        selector: Signal<Level>,
        when_low: Signal<Level>,
        when_high: Signal<Level>,
        meta: DiagnosticMeta,
    ) -> Result<AddedNode<Signal<Level>>, AuthoringFailure> {
        self.graph
            .require_all_local(&[selector, when_low, when_high])?;
        self.graph.add_select_with_ports(
            key,
            selector_port,
            when_low_port,
            when_high_port,
            output,
            selector,
            when_low,
            when_high,
            meta,
        )
    }

    /// Adds a variadic pulse Merge.
    pub fn merge<I>(&mut self, inputs: I) -> Result<Signal<Pulse>, AuthoringFailure>
    where
        I: IntoIterator<Item = Signal<Pulse>>,
    {
        let inputs = inputs.into_iter().collect::<Vec<_>>();
        self.graph.require_all_local(&inputs)?;
        self.graph.merge(inputs)
    }

    /// Adds an explicitly keyed Merge with locally allocated ports.
    pub fn add_merge<I>(
        &mut self,
        key: NodeKey,
        inputs: I,
        meta: DiagnosticMeta,
    ) -> Result<AddedNode<Signal<Pulse>>, AuthoringFailure>
    where
        I: IntoIterator<Item = Signal<Pulse>>,
    {
        let inputs = inputs.into_iter().collect::<Vec<_>>();
        self.graph.require_all_local(&inputs)?;
        self.graph.add_merge(key, inputs, meta)
    }

    /// Adds an explicitly keyed Merge with exact ports.
    pub fn add_merge_with_ports<I>(
        &mut self,
        key: NodeKey,
        output: OutPortKey<Pulse>,
        inputs: I,
        meta: DiagnosticMeta,
    ) -> Result<AddedNode<Signal<Pulse>>, AuthoringFailure>
    where
        I: IntoIterator<Item = (InPortKey<Pulse>, Signal<Pulse>)>,
    {
        let inputs = inputs.into_iter().collect::<Vec<_>>();
        for (_, input) in &inputs {
            self.graph.require_local(*input)?;
        }
        self.graph.add_merge_with_ports(key, output, inputs, meta)
    }

    /// Adds a pulse-controlled Toggle.
    pub fn toggle(
        &mut self,
        input: Signal<Pulse>,
        config: ToggleConfig,
    ) -> Result<Signal<Level>, AuthoringFailure> {
        self.graph.require_local(input)?;
        self.graph.toggle(input, config)
    }

    /// Adds an explicitly keyed Toggle with locally allocated ports.
    pub fn add_toggle(
        &mut self,
        key: NodeKey,
        input: Signal<Pulse>,
        config: ToggleConfig,
        meta: DiagnosticMeta,
    ) -> Result<AddedNode<Signal<Level>>, AuthoringFailure> {
        self.graph.require_local(input)?;
        self.graph.add_toggle(key, input, config, meta)
    }

    /// Adds an explicitly keyed Toggle with exact ports.
    pub fn add_toggle_with_ports(
        &mut self,
        key: NodeKey,
        input_port: InPortKey<Pulse>,
        output_port: OutPortKey<Level>,
        input: Signal<Pulse>,
        config: ToggleConfig,
        meta: DiagnosticMeta,
    ) -> Result<AddedNode<Signal<Level>>, AuthoringFailure> {
        self.graph.require_local(input)?;
        self.graph
            .add_toggle_with_ports(key, input_port, output_port, input, config, meta)
    }

    /// Adds a PulseDelay.
    pub fn pulse_delay(
        &mut self,
        input: Signal<Pulse>,
        config: PulseDelayConfig<D>,
    ) -> Result<Signal<Pulse>, AuthoringFailure> {
        self.graph.require_local(input)?;
        self.graph.pulse_delay(input, config)
    }

    /// Adds an explicitly keyed PulseDelay with locally allocated ports.
    pub fn add_pulse_delay(
        &mut self,
        key: NodeKey,
        input: Signal<Pulse>,
        config: PulseDelayConfig<D>,
        meta: DiagnosticMeta,
    ) -> Result<AddedNode<Signal<Pulse>>, AuthoringFailure> {
        self.graph.require_local(input)?;
        self.graph.add_pulse_delay(key, input, config, meta)
    }

    /// Adds an explicitly keyed PulseDelay with exact ports.
    pub fn add_pulse_delay_with_ports(
        &mut self,
        key: NodeKey,
        input_port: InPortKey<Pulse>,
        output_port: OutPortKey<Pulse>,
        input: Signal<Pulse>,
        config: PulseDelayConfig<D>,
        meta: DiagnosticMeta,
    ) -> Result<AddedNode<Signal<Pulse>>, AuthoringFailure> {
        self.graph.require_local(input)?;
        self.graph
            .add_pulse_delay_with_ports(key, input_port, output_port, input, config, meta)
    }

    /// Consumes the builder into the canonical unchecked user-module definition.
    #[must_use]
    pub fn into_unchecked(self) -> UncheckedModule<D> {
        let Self {
            meta,
            graph,
            inputs,
            outputs,
            mappings: output_mappings,
            ..
        } = self;
        let NetworkBuilder {
            nodes, connections, ..
        } = graph;
        let mut mappings = Vec::new();
        let mut internal_connections = Vec::with_capacity(connections.len());
        for connection in connections {
            match connection.from() {
                ConnectionEndpoint::ExternalInput(key) => {
                    mappings.push(ModuleInterfaceMapping::input(
                        module_input_from_surrogate(key),
                        connection.to(),
                    ))
                }
                _ => internal_connections.push(connection),
            }
        }
        mappings.extend(output_mappings);
        UncheckedModule::new_user(meta, inputs, outputs, mappings, nodes, internal_connections)
    }

    /// Consumes, lowers, and validates the authored user module.
    #[must_use]
    pub fn finish(self) -> Report<ModuleDef<D>, D>
    where
        D: PartialEq,
    {
        self.into_unchecked().validate()
    }

    fn next_level_input_key(&mut self) -> ModuleInputKey<Level> {
        loop {
            let key = self.interface_allocator.module_input();
            if !self.level_input_keys.contains(&key) {
                return key;
            }
        }
    }

    fn next_pulse_input_key(&mut self) -> ModuleInputKey<Pulse> {
        loop {
            let key = self.interface_allocator.module_input();
            if !self.pulse_input_keys.contains(&key) {
                return key;
            }
        }
    }

    fn next_level_output_key(&mut self) -> ModuleOutputKey<Level> {
        loop {
            let key = self.interface_allocator.module_output();
            if !self.level_output_keys.contains(&key) {
                return key;
            }
        }
    }

    fn next_pulse_output_key(&mut self) -> ModuleOutputKey<Pulse> {
        loop {
            let key = self.interface_allocator.module_output();
            if !self.pulse_output_keys.contains(&key) {
                return key;
            }
        }
    }
}

impl<D> Default for ModuleBuilder<D> {
    fn default() -> Self {
        Self::new()
    }
}

fn module_input_from_surrogate(key: AnyExternalInputKey) -> crate::key::AnyModuleInputKey {
    match key {
        AnyExternalInputKey::Level(key) => ModuleInputKey::<Level>::from_u128(key.as_u128()).into(),
        AnyExternalInputKey::Pulse(key) => ModuleInputKey::<Pulse>::from_u128(key.as_u128()).into(),
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

fn source_endpoint_pulse(source: SignalSourceKey<Pulse>) -> ConnectionEndpoint {
    match source {
        SignalSourceKey::ExternalInput(key) => ConnectionEndpoint::external_input(key.into()),
        SignalSourceKey::NodeOutput(key) => ConnectionEndpoint::node_output(key.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::authored::{
        ConnectionDef, ExternalInputDef, ExternalOutputDef, InputPortRole, NodeDef, NodeKind,
        NodePorts, UncheckedNetwork,
    };
    use crate::key::AnySignalSourceKey;
    use crate::signal::PulseCount;
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
    fn typed_and_dynamic_merge_definitions_have_identical_identity_and_behavior() {
        let domain = TimeDomainId::from_u128(10);
        let network = NetworkKey::from_u128(1);
        let first = ExternalInputKey::<Pulse>::from_u128(2);
        let second = ExternalInputKey::<Pulse>::from_u128(3);
        let node = NodeKey::from_u128(4);
        let first_port = InPortKey::<Pulse>::from_u128(5);
        let second_port = InPortKey::<Pulse>::from_u128(6);
        let node_output = OutPortKey::<Pulse>::from_u128(7);
        let output = ExternalOutputKey::<Pulse>::from_u128(8);

        let mut typed = NetworkBuilder::<()>::with_key(network, domain);
        let first_signal = typed
            .add_pulse_input(first, DiagnosticMeta::default())
            .unwrap();
        let second_signal = typed
            .add_pulse_input(second, DiagnosticMeta::default())
            .unwrap();
        let merge = typed
            .add_merge_with_ports(
                node,
                node_output,
                [(first_port, first_signal), (second_port, second_signal)],
                DiagnosticMeta::default(),
            )
            .unwrap()
            .into_outputs();
        typed
            .add_pulse_output(output, merge, DiagnosticMeta::default())
            .unwrap();
        let typed = typed.finish().require_artifact().unwrap();

        let dynamic = UncheckedNetwork::new(
            network,
            domain,
            DiagnosticMeta::default(),
            vec![NodeDef::new(
                node,
                NodeKind::<()>::merge(),
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
            typed.input_schema_fingerprint(),
            dynamic.input_schema_fingerprint()
        );
        assert_eq!(
            typed.reaction_dependencies(),
            dynamic.reaction_dependencies()
        );
        assert_eq!(typed.topological_order(), dynamic.topological_order());
        let typed = typed.compile().require_artifact().unwrap();
        let dynamic = dynamic.compile().require_artifact().unwrap();
        let pulses = BTreeMap::from([(first, PulseCount::new(2)), (second, PulseCount::new(5))]);
        assert_eq!(
            typed
                .evaluate_reaction(&BTreeMap::new(), &pulses)
                .unwrap()
                .pulse_outputs,
            dynamic
                .evaluate_reaction(&BTreeMap::new(), &pulses)
                .unwrap()
                .pulse_outputs
        );
    }

    #[test]
    fn typed_and_dynamic_remaining_primitives_have_identical_identity_and_behavior() {
        let domain = TimeDomainId::from_u128(10);
        let network = NetworkKey::from_u128(1);
        let first = ExternalInputKey::from_u128(2);
        let second = ExternalInputKey::from_u128(3);
        let node = NodeKey::from_u128(4);
        let first_port = InPortKey::from_u128(5);
        let second_port = InPortKey::from_u128(6);
        let node_output = OutPortKey::from_u128(7);
        let output = ExternalOutputKey::from_u128(8);

        for dynamic_kind in [
            NodeKind::<()>::any(),
            NodeKind::parity(),
            NodeKind::at_least(2),
        ] {
            let mut typed = NetworkBuilder::<()>::with_key(network, domain);
            let first_signal = typed
                .add_level_input(first, DiagnosticMeta::default())
                .unwrap();
            let second_signal = typed
                .add_level_input(second, DiagnosticMeta::default())
                .unwrap();
            let added = match &dynamic_kind {
                NodeKind::Any => typed.add_any_with_ports(
                    node,
                    node_output,
                    [(first_port, first_signal), (second_port, second_signal)],
                    DiagnosticMeta::default(),
                ),
                NodeKind::Parity => typed.add_parity_with_ports(
                    node,
                    node_output,
                    [(first_port, first_signal), (second_port, second_signal)],
                    DiagnosticMeta::default(),
                ),
                NodeKind::AtLeast(config) => typed.add_at_least_with_ports(
                    node,
                    node_output,
                    config.threshold,
                    [(first_port, first_signal), (second_port, second_signal)],
                    DiagnosticMeta::default(),
                ),
                _ => panic!("fixture must contain a remaining variadic primitive"),
            }
            .unwrap()
            .into_outputs();
            typed
                .add_level_output(output, added, DiagnosticMeta::default())
                .unwrap();
            let typed = typed.finish().require_artifact().unwrap();

            let dynamic = UncheckedNetwork::new(
                network,
                domain,
                DiagnosticMeta::default(),
                vec![NodeDef::new(
                    node,
                    dynamic_kind,
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
            let typed = typed.compile().require_artifact().unwrap();
            let dynamic = dynamic.compile().require_artifact().unwrap();
            for valuation in 0..4 {
                let inputs = BTreeMap::from([
                    (
                        first,
                        if valuation & 1 == 0 {
                            LogicLevel::Low
                        } else {
                            LogicLevel::High
                        },
                    ),
                    (
                        second,
                        if valuation & 2 == 0 {
                            LogicLevel::Low
                        } else {
                            LogicLevel::High
                        },
                    ),
                ]);
                assert_eq!(
                    typed.evaluate_full(&inputs).unwrap().external_outputs,
                    dynamic.evaluate_full(&inputs).unwrap().external_outputs
                );
            }
        }

        let mut typed = NetworkBuilder::<()>::with_key(network, domain);
        let selector = typed
            .add_level_input(first, DiagnosticMeta::default())
            .unwrap();
        let when_low = typed
            .add_level_input(second, DiagnosticMeta::default())
            .unwrap();
        let third = ExternalInputKey::from_u128(10);
        let when_high = typed
            .add_level_input(third, DiagnosticMeta::default())
            .unwrap();
        let when_high_port = InPortKey::from_u128(9);
        let select = typed
            .add_select_with_ports(
                node,
                first_port,
                second_port,
                when_high_port,
                node_output,
                selector,
                when_low,
                when_high,
                DiagnosticMeta::default(),
            )
            .unwrap();
        assert_eq!(
            select.outputs().source_key(),
            SignalSourceKey::NodeOutput(node_output)
        );
        typed
            .add_level_output(output, select.into_outputs(), DiagnosticMeta::default())
            .unwrap();
        let typed = typed.finish().require_artifact().unwrap();

        let roles = vec![
            InputPortRole::Selector,
            InputPortRole::WhenLow,
            InputPortRole::WhenHigh,
        ];
        let dynamic = UncheckedNetwork::new(
            network,
            domain,
            DiagnosticMeta::default(),
            vec![NodeDef::new(
                node,
                NodeKind::<()>::select(),
                NodePorts::with_input_roles(
                    vec![first_port.into(), second_port.into(), when_high_port.into()],
                    roles,
                    vec![node_output.into()],
                ),
                DiagnosticMeta::default(),
            )],
            vec![
                ExternalInputDef::new(first.into(), DiagnosticMeta::default()),
                ExternalInputDef::new(second.into(), DiagnosticMeta::default()),
                ExternalInputDef::new(third.into(), DiagnosticMeta::default()),
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
                ConnectionDef::new(
                    crate::key::ConnectionKey::from_u128(2),
                    third.into(),
                    when_high_port.into(),
                    DiagnosticMeta::default(),
                ),
            ],
        )
        .validate()
        .require_artifact()
        .unwrap();
        assert_eq!(typed.fingerprint(), dynamic.fingerprint());

        let typed = typed.compile().require_artifact().unwrap();
        let dynamic = dynamic.compile().require_artifact().unwrap();
        for valuation in 0..8 {
            let inputs = BTreeMap::from([
                (
                    first,
                    if valuation & 1 == 0 {
                        LogicLevel::Low
                    } else {
                        LogicLevel::High
                    },
                ),
                (
                    second,
                    if valuation & 2 == 0 {
                        LogicLevel::Low
                    } else {
                        LogicLevel::High
                    },
                ),
                (
                    third,
                    if valuation & 4 == 0 {
                        LogicLevel::Low
                    } else {
                        LogicLevel::High
                    },
                ),
            ]);
            assert_eq!(
                typed.evaluate_full(&inputs).unwrap().external_outputs,
                dynamic.evaluate_full(&inputs).unwrap().external_outputs
            );
        }
    }

    #[test]
    fn convenience_allocation_is_deterministic() {
        let build = || {
            let mut builder = NetworkBuilder::<()>::new(TimeDomainId::from_u128(1));
            let first = builder.level_input("first");
            let second = builder.level_input("second");
            let inverted = builder.not(first.1).unwrap();
            let any = builder.any([inverted, second.1]).unwrap();
            let parity = builder.parity([first.1, second.1]).unwrap();
            let threshold = builder.at_least(2, [any, parity]).unwrap();
            let output = builder.select(first.1, parity, threshold).unwrap();
            let endpoint = builder.level_output("output", output).unwrap();
            (first.0, second.0, endpoint, builder.into_unchecked())
        };
        let left = build();
        let right = build();
        assert_eq!(left.0, right.0);
        assert_eq!(left.1, right.1);
        assert_eq!(left.2, right.2);
        assert_eq!(left.3, right.3);
    }

    #[test]
    fn typed_and_direct_dynamic_toggle_paths_are_equivalent() {
        let network = NetworkKey::from_u128(1);
        let domain = TimeDomainId::from_u128(2);
        let pulse = ExternalInputKey::<Pulse>::from_u128(3);
        let node = NodeKey::from_u128(4);
        let input = InPortKey::<Pulse>::from_u128(5);
        let output = OutPortKey::<Level>::from_u128(6);
        let external_output = ExternalOutputKey::<Level>::from_u128(7);
        let mut builder = NetworkBuilder::<()>::with_key(network, domain);
        let signal = builder
            .add_pulse_input(pulse, DiagnosticMeta::default())
            .unwrap();
        let toggled = builder
            .add_toggle_with_ports(
                node,
                input,
                output,
                signal,
                ToggleConfig::new(LogicLevel::High),
                DiagnosticMeta::default(),
            )
            .unwrap()
            .into_outputs();
        builder
            .add_level_output(external_output, toggled, DiagnosticMeta::default())
            .unwrap();
        let typed = builder.into_unchecked();
        let connection = typed.connections()[0].key();
        let dynamic = UncheckedNetwork::new(
            network,
            domain,
            DiagnosticMeta::default(),
            vec![NodeDef::new(
                node,
                NodeKind::<()>::toggle(LogicLevel::High),
                NodePorts::with_input_roles(
                    vec![input.into()],
                    vec![crate::authored::InputPortRole::Toggle],
                    vec![output.into()],
                ),
                DiagnosticMeta::default(),
            )],
            vec![ExternalInputDef::new(
                pulse.into(),
                DiagnosticMeta::default(),
            )],
            vec![ExternalOutputDef::new(
                external_output.into(),
                SignalSourceKey::NodeOutput(output).into(),
                DiagnosticMeta::default(),
            )],
            vec![ConnectionDef::new(
                connection,
                pulse.into(),
                input.into(),
                DiagnosticMeta::default(),
            )],
        );
        let typed = typed.validate().require_artifact().unwrap();
        let dynamic = dynamic.validate().require_artifact().unwrap();
        assert_eq!(typed.fingerprint(), dynamic.fingerprint());
        let typed = typed.compile().require_artifact().unwrap();
        let dynamic = dynamic.compile().require_artifact().unwrap();
        for count in [0_u64, 1, 2, 3, 10, 11] {
            let pulses = BTreeMap::from([(pulse, PulseCount::new(count))]);
            assert_eq!(
                typed.evaluate_reaction(&BTreeMap::new(), &pulses).unwrap(),
                dynamic
                    .evaluate_reaction(&BTreeMap::new(), &pulses)
                    .unwrap()
            );
        }

        let policy = || {
            crate::RuntimePolicy::builder()
                .max_internal_reactions(10)
                .max_evaluated_operations(100)
                .max_pending_events(0)
                .max_events_created_per_transaction(100)
                .max_required_provenance_growth(1_000)
                .build()
                .unwrap()
        };
        let mut typed_machine = typed.spawn(policy());
        let mut dynamic_machine = dynamic.spawn(policy());
        let snapshot = typed
            .input_snapshot()
            .pulse(pulse, PulseCount::ONE)
            .and_then(crate::InputSnapshotBuilder::finish)
            .unwrap();
        for machine in [&mut typed_machine, &mut dynamic_machine] {
            machine
                .apply(crate::Transaction::initialize(
                    crate::time::Time::from_ticks(1),
                    machine.revision(),
                    snapshot.clone(),
                ))
                .unwrap();
        }
        let delta = typed
            .input_delta()
            .pulse(pulse, PulseCount::new(3))
            .and_then(crate::InputDeltaBuilder::finish)
            .unwrap();
        for machine in [&mut typed_machine, &mut dynamic_machine] {
            machine
                .apply(crate::Transaction::advance(
                    crate::time::Time::from_ticks(2),
                    machine.revision(),
                    delta.clone(),
                ))
                .unwrap();
        }
        assert_eq!(
            typed_machine.output_level(external_output),
            dynamic_machine.output_level(external_output)
        );
        assert_eq!(
            typed_machine.inspect_toggle(node).unwrap(),
            dynamic_machine.inspect_toggle(node).unwrap()
        );
    }

    #[test]
    fn typed_and_direct_dynamic_pulse_delay_paths_are_equivalent() {
        let network = NetworkKey::from_u128(101);
        let domain = TimeDomainId::from_u128(102);
        let pulse = ExternalInputKey::<Pulse>::from_u128(103);
        let node = NodeKey::from_u128(104);
        let input = InPortKey::<Pulse>::from_u128(105);
        let output = OutPortKey::<Pulse>::from_u128(106);
        let external_output = ExternalOutputKey::<Pulse>::from_u128(107);
        let delay = crate::time::NonZeroSpan::from_ticks(5).unwrap();
        let config = PulseDelayConfig::new(delay);

        let mut builder = NetworkBuilder::<()>::with_key(network, domain);
        let signal = builder
            .add_pulse_input(pulse, DiagnosticMeta::default())
            .unwrap();
        let delayed = builder
            .add_pulse_delay_with_ports(
                node,
                input,
                output,
                signal,
                config,
                DiagnosticMeta::default(),
            )
            .unwrap()
            .into_outputs();
        builder
            .add_pulse_output(external_output, delayed, DiagnosticMeta::default())
            .unwrap();
        let typed = builder.into_unchecked();
        let connection = typed.connections()[0].key();

        let dynamic = UncheckedNetwork::new(
            network,
            domain,
            DiagnosticMeta::default(),
            vec![NodeDef::new(
                node,
                NodeKind::PulseDelay(config),
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
            vec![ExternalOutputDef::new(
                external_output.into(),
                SignalSourceKey::NodeOutput(output).into(),
                DiagnosticMeta::default(),
            )],
            vec![ConnectionDef::new(
                connection,
                pulse.into(),
                input.into(),
                DiagnosticMeta::default(),
            )],
        );

        let typed = typed.validate().require_artifact().unwrap();
        let dynamic = dynamic.validate().require_artifact().unwrap();
        assert_eq!(typed.fingerprint(), dynamic.fingerprint());
        let typed = typed.compile().require_artifact().unwrap();
        let dynamic = dynamic.compile().require_artifact().unwrap();
        let pulses = BTreeMap::from([(pulse, PulseCount::new(4))]);
        assert_eq!(
            typed.evaluate_reaction(&BTreeMap::new(), &pulses).unwrap(),
            dynamic
                .evaluate_reaction(&BTreeMap::new(), &pulses)
                .unwrap()
        );
    }

    #[test]
    fn typed_module_lowering_matches_the_exact_dynamic_definition() {
        let input = ModuleInputKey::<Level>::from_u128(10);
        let node = NodeKey::from_u128(11);
        let node_input = InPortKey::<Level>::from_u128(12);
        let node_output = OutPortKey::<Level>::from_u128(13);
        let output = ModuleOutputKey::<Level>::from_u128(14);

        let mut typed = ModuleBuilder::<()>::with_meta(meta("module"));
        let source = typed.add_level_input(input, meta("input")).unwrap();
        let inverted = typed
            .add_not_with_ports(node, node_input, node_output, source, meta("not"))
            .unwrap()
            .into_outputs();
        typed
            .add_level_output(output, inverted, meta("output"))
            .unwrap();

        let dynamic = UncheckedModule::new_user(
            meta("module"),
            vec![ModuleInputDef::new(input.into(), meta("input"))],
            vec![ModuleOutputDef::new(output.into(), meta("output"))],
            vec![
                ModuleInterfaceMapping::input(input.into(), node_input.into()),
                ModuleInterfaceMapping::output(output.into(), node_output.into()),
            ],
            vec![NodeDef::new(
                node,
                NodeKind::not(),
                NodePorts::new(vec![node_input.into()], vec![node_output.into()]),
                meta("not"),
            )],
            Vec::new(),
        );

        assert_eq!(typed.into_unchecked(), dynamic);
    }

    fn representative_typed_module() -> ModuleBuilder<()> {
        let mut module = ModuleBuilder::with_meta(meta("representative"));
        let (_, first) = module.level_input("first");
        let (_, second) = module.level_input("second");
        let (_, pulses) = module.pulse_input("pulses");

        let constant = module.constant(LogicLevel::High);
        let inverted = module.not(first).unwrap();
        let all = module.all([first, second]).unwrap();
        let any = module.any([first, second]).unwrap();
        let parity = module.parity([first, second]).unwrap();
        let threshold = module.at_least(1, [first, second]).unwrap();
        let selected = module.select(constant, inverted, all).unwrap();
        let merged = module.merge([pulses, pulses]).unwrap();
        let toggled = module
            .toggle(merged, ToggleConfig::new(LogicLevel::Low))
            .unwrap();
        let delay = crate::time::NonZeroSpan::from_ticks(3).unwrap();
        let delayed = module
            .pulse_delay(pulses, PulseDelayConfig::new(delay))
            .unwrap();

        module.level_output("selected", selected).unwrap();
        module.level_output("any", any).unwrap();
        module.level_output("parity", parity).unwrap();
        module.level_output("threshold", threshold).unwrap();
        module.level_output("toggled", toggled).unwrap();
        module.pulse_output("delayed", delayed).unwrap();
        module
    }

    #[test]
    fn typed_module_supports_the_complete_implemented_primitive_family() {
        let unchecked = representative_typed_module().into_unchecked();
        assert_eq!(unchecked.inputs().len(), 3);
        assert_eq!(unchecked.outputs().len(), 6);
        assert_eq!(unchecked.nodes().len(), 10);
        assert!(
            unchecked
                .connections()
                .iter()
                .all(|connection| matches!(connection.from(), ConnectionEndpoint::NodeOutput(_)))
        );

        let kinds = unchecked
            .nodes()
            .iter()
            .map(NodeDef::kind)
            .collect::<Vec<_>>();
        assert!(
            kinds
                .iter()
                .any(|kind| matches!(kind, NodeKind::Constant(_)))
        );
        assert!(kinds.iter().any(|kind| matches!(kind, NodeKind::Not)));
        assert!(kinds.iter().any(|kind| matches!(kind, NodeKind::All)));
        assert!(kinds.iter().any(|kind| matches!(kind, NodeKind::Any)));
        assert!(kinds.iter().any(|kind| matches!(kind, NodeKind::Parity)));
        assert!(
            kinds
                .iter()
                .any(|kind| matches!(kind, NodeKind::AtLeast(_)))
        );
        assert!(kinds.iter().any(|kind| matches!(kind, NodeKind::Select)));
        assert!(kinds.iter().any(|kind| matches!(kind, NodeKind::Merge)));
        assert!(kinds.iter().any(|kind| matches!(kind, NodeKind::Toggle(_))));
        assert!(
            kinds
                .iter()
                .any(|kind| matches!(kind, NodeKind::PulseDelay(_)))
        );
    }

    #[test]
    fn typed_module_finish_matches_direct_unchecked_validation() {
        let direct = representative_typed_module().into_unchecked();
        let direct_report = direct.validate_ref();
        let typed_report = representative_typed_module().finish();
        assert_eq!(typed_report.diagnostics(), direct_report.diagnostics());
        let typed = typed_report.require_artifact().unwrap();
        let direct = direct_report.require_artifact().unwrap();
        assert_eq!(typed.fingerprint(), direct.fingerprint());
        assert_eq!(typed.inputs().len(), direct.inputs().len());
        assert_eq!(typed.outputs().len(), direct.outputs().len());
        assert_eq!(typed.graph().nodes(), direct.graph().nodes());
        assert_eq!(typed.graph().connections(), direct.graph().connections());
        assert!(matches!(typed.origin(), crate::ModuleOrigin::User));
    }

    #[test]
    fn module_interface_duplicates_are_structured_and_non_mutating() {
        let input = ModuleInputKey::<Level>::from_u128(1);
        let output = ModuleOutputKey::<Level>::from_u128(2);
        let mut module = ModuleBuilder::<()>::new();
        let source = module
            .add_level_input(input, DiagnosticMeta::default())
            .unwrap();
        assert!(matches!(
            module.add_level_input(input, meta("duplicate")),
            Err(AuthoringFailure::DuplicateModuleInputKey(actual)) if actual == input
        ));
        let constant = module.constant(LogicLevel::Low);
        module
            .add_level_output(output, constant, DiagnosticMeta::default())
            .unwrap();
        assert!(matches!(
            module.add_level_output(output, source, meta("duplicate")),
            Err(AuthoringFailure::DuplicateModuleOutputKey(actual)) if actual == output
        ));
        let lowered = module.into_unchecked();
        assert_eq!(lowered.inputs().len(), 1);
        assert_eq!(lowered.outputs().len(), 1);
        assert_eq!(lowered.mappings().len(), 1);
    }

    fn exercise_foreign_level_calls(receiver: &mut ModuleBuilder<()>, foreign: Signal<Level>) {
        assert!(matches!(
            receiver.not(foreign),
            Err(AuthoringFailure::ForeignSignal)
        ));
        assert!(matches!(
            receiver.all([foreign]),
            Err(AuthoringFailure::ForeignSignal)
        ));
        assert!(matches!(
            receiver.any([foreign]),
            Err(AuthoringFailure::ForeignSignal)
        ));
        assert!(matches!(
            receiver.parity([foreign]),
            Err(AuthoringFailure::ForeignSignal)
        ));
        assert!(matches!(
            receiver.at_least(1, [foreign]),
            Err(AuthoringFailure::ForeignSignal)
        ));
        assert!(matches!(
            receiver.select(foreign, foreign, foreign),
            Err(AuthoringFailure::ForeignSignal)
        ));
        assert_eq!(
            receiver.level_output("foreign", foreign),
            Err(AuthoringFailure::ForeignSignal)
        );
    }

    fn exercise_foreign_pulse_calls(receiver: &mut ModuleBuilder<()>, foreign: Signal<Pulse>) {
        assert!(matches!(
            receiver.merge([foreign]),
            Err(AuthoringFailure::ForeignSignal)
        ));
        assert!(matches!(
            receiver.toggle(foreign, ToggleConfig::new(LogicLevel::Low)),
            Err(AuthoringFailure::ForeignSignal)
        ));
        let delay = crate::time::NonZeroSpan::from_ticks(1).unwrap();
        assert!(matches!(
            receiver.pulse_delay(foreign, PulseDelayConfig::new(delay)),
            Err(AuthoringFailure::ForeignSignal)
        ));
        assert_eq!(
            receiver.pulse_output("foreign", foreign),
            Err(AuthoringFailure::ForeignSignal)
        );
    }

    #[test]
    fn foreign_module_and_network_signals_fail_before_authored_mutation() {
        let mut foreign_module = ModuleBuilder::<()>::new();
        let (_, module_level) = foreign_module.level_input("level");
        let (_, module_pulse) = foreign_module.pulse_input("pulse");
        let mut receiver = ModuleBuilder::<()>::new();
        exercise_foreign_level_calls(&mut receiver, module_level);
        exercise_foreign_pulse_calls(&mut receiver, module_pulse);

        let mut foreign_network = NetworkBuilder::<()>::new(TimeDomainId::from_u128(1));
        let (_, network_level) = foreign_network.level_input("level");
        let (_, network_pulse) = foreign_network.pulse_input("pulse");
        exercise_foreign_level_calls(&mut receiver, network_level);
        exercise_foreign_pulse_calls(&mut receiver, network_pulse);

        let lowered = receiver.into_unchecked();
        assert!(lowered.inputs().is_empty());
        assert!(lowered.outputs().is_empty());
        assert!(lowered.mappings().is_empty());
        assert!(lowered.nodes().is_empty());
        assert!(lowered.connections().is_empty());
    }
}
