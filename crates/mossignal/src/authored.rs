//! Lossless authored network and reusable-module definitions awaiting structural validation.
//!
//! This module deliberately represents structure only. It does not validate,
//! compile, instantiate, or execute a definition.

use crate::identity::TimeDomainId;
use crate::key::{
    AnyExternalInputKey, AnyExternalOutputKey, AnyInPortKey, AnyModuleInputKey, AnyModuleOutputKey,
    AnyOutPortKey, AnySignalSourceKey, ConnectionKey, ExternalInputKey, ExternalOutputKey,
    InPortKey, ModuleInstanceKey, NetworkKey, NodeKey, OutPortKey,
};
use crate::metadata::DiagnosticMeta;
use crate::signal::{Level, LogicLevel, Pulse, SignalKind};
use crate::time::NonZeroSpan;
use core::fmt;
use core::hash::{Hash, Hasher};
use core::marker::PhantomData;

/// An owned, stable-keyed network definition that may be structurally invalid.
///
/// Its collections retain every supplied claim in caller order. Structural
/// validation is a later, one-way transition and is intentionally absent here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UncheckedNetwork<D> {
    key: NetworkKey,
    time_domain_id: TimeDomainId,
    meta: DiagnosticMeta,
    nodes: Vec<NodeDef<D>>,
    external_inputs: Vec<ExternalInputDef>,
    external_outputs: Vec<ExternalOutputDef>,
    connections: Vec<ConnectionDef>,
    module_instances: Vec<ModuleInstanceDef<D>>,
}

impl<D> UncheckedNetwork<D> {
    /// Creates an unchecked definition without interpreting or validating it.
    #[must_use]
    pub fn new(
        key: NetworkKey,
        time_domain_id: TimeDomainId,
        meta: DiagnosticMeta,
        nodes: Vec<NodeDef<D>>,
        external_inputs: Vec<ExternalInputDef>,
        external_outputs: Vec<ExternalOutputDef>,
        connections: Vec<ConnectionDef>,
    ) -> Self {
        Self::new_with_instances(
            key,
            time_domain_id,
            meta,
            nodes,
            external_inputs,
            external_outputs,
            connections,
            Vec::new(),
        )
    }

    /// Creates an unchecked definition including lossless module-instance claims.
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub fn new_with_instances(
        key: NetworkKey,
        time_domain_id: TimeDomainId,
        meta: DiagnosticMeta,
        nodes: Vec<NodeDef<D>>,
        external_inputs: Vec<ExternalInputDef>,
        external_outputs: Vec<ExternalOutputDef>,
        connections: Vec<ConnectionDef>,
        module_instances: Vec<ModuleInstanceDef<D>>,
    ) -> Self {
        // SPEC: docs/specs/contracts/authored-network-definition.yaml "malformed-claims-retained"
        // Claim vectors must remain lossless so duplicate and malformed input reaches validation.
        Self {
            key,
            time_domain_id,
            meta,
            nodes,
            external_inputs,
            external_outputs,
            connections,
            module_instances,
        }
    }

    /// Returns the network's stable authored identity.
    #[must_use]
    pub const fn key(&self) -> NetworkKey {
        self.key
    }

    /// Returns the caller-supplied identity of this network's logical tick.
    #[must_use]
    pub const fn time_domain_id(&self) -> TimeDomainId {
        self.time_domain_id
    }

    /// Returns presentation metadata attached to this network.
    #[must_use]
    pub const fn meta(&self) -> &DiagnosticMeta {
        &self.meta
    }

    /// Returns every node claim, including duplicate keys.
    #[must_use]
    pub fn nodes(&self) -> &[NodeDef<D>] {
        &self.nodes
    }

    /// Returns every external-input claim, including duplicate keys.
    #[must_use]
    pub fn external_inputs(&self) -> &[ExternalInputDef] {
        &self.external_inputs
    }

    /// Returns every external-output claim, including duplicate keys.
    #[must_use]
    pub fn external_outputs(&self) -> &[ExternalOutputDef] {
        &self.external_outputs
    }

    /// Returns every connection claim, including malformed and duplicate claims.
    #[must_use]
    pub fn connections(&self) -> &[ConnectionDef] {
        &self.connections
    }

    /// Returns every module-instance claim, including duplicates and malformed bindings.
    #[must_use]
    pub fn module_instances(&self) -> &[ModuleInstanceDef<D>] {
        &self.module_instances
    }
}

/// The explicit origin retained by an unchecked caller-authored module.
///
/// Standard catalogue declarations belong to a later validated-module slice.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UncheckedModuleOrigin {
    /// The module was authored by a caller rather than produced from a standard declaration.
    User,
}

/// One stable-keyed public module-input declaration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleInputDef {
    key: AnyModuleInputKey,
    meta: DiagnosticMeta,
}

impl ModuleInputDef {
    /// Creates a module-input declaration without validating its mappings.
    #[must_use]
    pub const fn new(key: AnyModuleInputKey, meta: DiagnosticMeta) -> Self {
        Self { key, meta }
    }

    /// Returns the stable kind-aware public input identity.
    #[must_use]
    pub const fn key(&self) -> AnyModuleInputKey {
        self.key
    }

    /// Returns presentation metadata attached to this public input.
    #[must_use]
    pub const fn meta(&self) -> &DiagnosticMeta {
        &self.meta
    }
}

/// One stable-keyed public module-output declaration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleOutputDef {
    key: AnyModuleOutputKey,
    meta: DiagnosticMeta,
}

impl ModuleOutputDef {
    /// Creates a module-output declaration without validating its mappings.
    #[must_use]
    pub const fn new(key: AnyModuleOutputKey, meta: DiagnosticMeta) -> Self {
        Self { key, meta }
    }

    /// Returns the stable kind-aware public output identity.
    #[must_use]
    pub const fn key(&self) -> AnyModuleOutputKey {
        self.key
    }

    /// Returns presentation metadata attached to this public output.
    #[must_use]
    pub const fn meta(&self) -> &DiagnosticMeta {
        &self.meta
    }
}

/// One raw mapping claim between a public module interface and private structure.
///
/// Endpoint direction and signal kind are retained but deliberately unchecked.
/// Repeating a claim, omitting one, or supplying several claims for one public
/// key remains observable to later module validation.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModuleInterfaceMapping {
    /// Claims that a public module input drives one private endpoint.
    Input {
        /// The stable kind-aware public input identity.
        input: AnyModuleInputKey,
        /// The claimed private destination, including a direction-invalid endpoint.
        target: ConnectionEndpoint,
    },
    /// Claims that a public module output exports one private endpoint.
    Output {
        /// The stable kind-aware public output identity.
        output: AnyModuleOutputKey,
        /// The claimed private source, including a direction-invalid endpoint.
        source: ConnectionEndpoint,
    },
}

/// One owned unchecked module-input binding claim.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModuleBinding {
    input: AnyModuleInputKey,
    source: ConnectionEndpoint,
}

impl ModuleBinding {
    /// Creates a binding claim without checking membership, direction, or kind.
    #[must_use]
    pub const fn new(input: AnyModuleInputKey, source: ConnectionEndpoint) -> Self {
        Self { input, source }
    }

    /// Returns the claimed public module-input identity.
    #[must_use]
    pub const fn input(&self) -> AnyModuleInputKey {
        self.input
    }

    /// Returns the claimed stable source endpoint.
    #[must_use]
    pub const fn source(&self) -> ConnectionEndpoint {
        self.source
    }
}

/// An owned lossless collection of module-input binding claims.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ModuleBindingSet {
    bindings: Vec<ModuleBinding>,
}

impl ModuleBindingSet {
    /// Retains the supplied binding claims in caller order.
    #[must_use]
    pub const fn new(bindings: Vec<ModuleBinding>) -> Self {
        Self { bindings }
    }

    /// Returns every binding claim, including duplicates and malformed claims.
    #[must_use]
    pub fn bindings(&self) -> &[ModuleBinding] {
        &self.bindings
    }
}

/// One owned unchecked module-instance claim.
pub struct ModuleInstanceDef<D> {
    key: ModuleInstanceKey,
    module: crate::module::ModuleDef<D>,
    bindings: ModuleBindingSet,
    parent: Option<ModuleInstanceKey>,
    meta: DiagnosticMeta,
}

impl<D> ModuleInstanceDef<D> {
    /// Creates an instance claim without validating bindings or containment.
    #[must_use]
    pub fn new(
        key: ModuleInstanceKey,
        module: crate::module::ModuleDef<D>,
        bindings: ModuleBindingSet,
        parent: Option<ModuleInstanceKey>,
        meta: DiagnosticMeta,
    ) -> Self {
        Self {
            key,
            module,
            bindings,
            parent,
            meta,
        }
    }

    #[must_use]
    pub const fn key(&self) -> ModuleInstanceKey {
        self.key
    }
    #[must_use]
    pub const fn module(&self) -> &crate::module::ModuleDef<D> {
        &self.module
    }
    #[must_use]
    pub const fn bindings(&self) -> &ModuleBindingSet {
        &self.bindings
    }
    #[must_use]
    pub const fn parent(&self) -> Option<ModuleInstanceKey> {
        self.parent
    }
    #[must_use]
    pub const fn meta(&self) -> &DiagnosticMeta {
        &self.meta
    }
}

impl<D> Clone for ModuleInstanceDef<D> {
    fn clone(&self) -> Self {
        Self::new(
            self.key,
            self.module.clone(),
            self.bindings.clone(),
            self.parent,
            self.meta.clone(),
        )
    }
}

impl<D> PartialEq for ModuleInstanceDef<D> {
    fn eq(&self, other: &Self) -> bool {
        self.key == other.key
            && self.module.fingerprint() == other.module.fingerprint()
            && self.bindings == other.bindings
            && self.parent == other.parent
            && self.meta == other.meta
    }
}

impl<D> Eq for ModuleInstanceDef<D> {}

impl<D> fmt::Debug for ModuleInstanceDef<D> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ModuleInstanceDef")
            .field("key", &self.key)
            .field("module", &self.module)
            .field("bindings", &self.bindings)
            .field("parent", &self.parent)
            .field("meta", &self.meta)
            .finish()
    }
}

impl ModuleInterfaceMapping {
    /// Creates an unchecked public-input-to-private-endpoint mapping claim.
    #[must_use]
    pub const fn input(input: AnyModuleInputKey, target: ConnectionEndpoint) -> Self {
        Self::Input { input, target }
    }

    /// Creates an unchecked private-endpoint-to-public-output mapping claim.
    #[must_use]
    pub const fn output(output: AnyModuleOutputKey, source: ConnectionEndpoint) -> Self {
        Self::Output { output, source }
    }
}

/// An owned caller-authored reusable module definition that may be malformed.
///
/// Every collection retains caller order and duplicates. Equality is raw
/// structural equality over those retained claims and metadata; it is not a
/// semantic module identity or persistence-order promise.
pub struct UncheckedModule<D> {
    origin: UncheckedModuleOrigin,
    meta: DiagnosticMeta,
    inputs: Vec<ModuleInputDef>,
    outputs: Vec<ModuleOutputDef>,
    mappings: Vec<ModuleInterfaceMapping>,
    nodes: Vec<NodeDef<D>>,
    connections: Vec<ConnectionDef>,
    module_instances: Vec<ModuleInstanceDef<D>>,
}

impl<D> UncheckedModule<D> {
    /// Creates an unchecked user module without interpreting or validating any claim.
    #[must_use]
    pub fn new_user(
        meta: DiagnosticMeta,
        inputs: Vec<ModuleInputDef>,
        outputs: Vec<ModuleOutputDef>,
        mappings: Vec<ModuleInterfaceMapping>,
        nodes: Vec<NodeDef<D>>,
        connections: Vec<ConnectionDef>,
    ) -> Self {
        Self::new_user_with_instances(
            meta,
            inputs,
            outputs,
            mappings,
            nodes,
            connections,
            Vec::new(),
        )
    }

    /// Creates an unchecked user module including nested module-instance claims.
    #[must_use]
    pub fn new_user_with_instances(
        meta: DiagnosticMeta,
        inputs: Vec<ModuleInputDef>,
        outputs: Vec<ModuleOutputDef>,
        mappings: Vec<ModuleInterfaceMapping>,
        nodes: Vec<NodeDef<D>>,
        connections: Vec<ConnectionDef>,
        module_instances: Vec<ModuleInstanceDef<D>>,
    ) -> Self {
        // SPEC: docs/specs/contracts/unchecked-module-definition.yaml
        // "malformed-claims-remain-observable"
        // Separate raw vectors preserve duplicates, omissions, wrong kinds, and wrong directions.
        Self {
            origin: UncheckedModuleOrigin::User,
            meta,
            inputs,
            outputs,
            mappings,
            nodes,
            connections,
            module_instances,
        }
    }

    /// Returns the module's explicit caller-authored origin.
    #[must_use]
    pub const fn origin(&self) -> UncheckedModuleOrigin {
        self.origin
    }

    /// Returns presentation metadata attached to this module.
    #[must_use]
    pub const fn meta(&self) -> &DiagnosticMeta {
        &self.meta
    }

    /// Returns every public input declaration in caller order, including duplicates.
    #[must_use]
    pub fn inputs(&self) -> &[ModuleInputDef] {
        &self.inputs
    }

    /// Returns every public output declaration in caller order, including duplicates.
    #[must_use]
    pub fn outputs(&self) -> &[ModuleOutputDef] {
        &self.outputs
    }

    /// Returns every raw interface mapping claim in caller order.
    #[must_use]
    pub fn mappings(&self) -> &[ModuleInterfaceMapping] {
        &self.mappings
    }

    /// Returns every private module-local node claim in caller order.
    #[must_use]
    pub fn nodes(&self) -> &[NodeDef<D>] {
        &self.nodes
    }

    /// Returns every private module-local connection claim in caller order.
    #[must_use]
    pub fn connections(&self) -> &[ConnectionDef] {
        &self.connections
    }

    /// Returns every nested module-instance claim in caller order.
    #[must_use]
    pub fn module_instances(&self) -> &[ModuleInstanceDef<D>] {
        &self.module_instances
    }
}

impl<D> Clone for UncheckedModule<D> {
    fn clone(&self) -> Self {
        Self {
            origin: self.origin,
            meta: self.meta.clone(),
            inputs: self.inputs.clone(),
            outputs: self.outputs.clone(),
            mappings: self.mappings.clone(),
            nodes: self.nodes.clone(),
            connections: self.connections.clone(),
            module_instances: self.module_instances.clone(),
        }
    }
}

impl<D> PartialEq for UncheckedModule<D> {
    fn eq(&self, other: &Self) -> bool {
        self.origin == other.origin
            && self.meta == other.meta
            && self.inputs == other.inputs
            && self.outputs == other.outputs
            && self.mappings == other.mappings
            && self.nodes == other.nodes
            && self.connections == other.connections
            && self.module_instances == other.module_instances
    }
}

impl<D> Eq for UncheckedModule<D> {}

impl<D> fmt::Debug for UncheckedModule<D> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("UncheckedModule")
            .field("origin", &self.origin)
            .field("meta", &self.meta)
            .field("inputs", &self.inputs)
            .field("outputs", &self.outputs)
            .field("mappings", &self.mappings)
            .field("nodes", &self.nodes)
            .field("connections", &self.connections)
            .field("module_instances", &self.module_instances)
            .finish()
    }
}

/// One stable-keyed authored node claim.
pub struct NodeDef<D> {
    key: NodeKey,
    kind: NodeKind<D>,
    ports: NodePorts,
    meta: DiagnosticMeta,
}

impl<D> NodeDef<D> {
    /// Creates a node claim without checking its ports against its kind.
    #[must_use]
    pub fn new(key: NodeKey, kind: NodeKind<D>, ports: NodePorts, meta: DiagnosticMeta) -> Self {
        Self {
            key,
            kind,
            ports,
            meta,
        }
    }

    /// Returns the node's stable authored identity.
    #[must_use]
    pub const fn key(&self) -> NodeKey {
        self.key
    }

    /// Returns the claimed node kind.
    #[must_use]
    pub const fn kind(&self) -> &NodeKind<D> {
        &self.kind
    }

    /// Returns the node's claimed input and output ports.
    #[must_use]
    pub const fn ports(&self) -> &NodePorts {
        &self.ports
    }

    /// Returns presentation metadata attached to this node.
    #[must_use]
    pub const fn meta(&self) -> &DiagnosticMeta {
        &self.meta
    }

    /// Consumes this node claim into its complete owned parts.
    #[must_use]
    pub fn into_parts(self) -> (NodeKey, NodeKind<D>, NodePorts, DiagnosticMeta) {
        (self.key, self.kind, self.ports, self.meta)
    }
}

impl<D> Clone for NodeDef<D> {
    fn clone(&self) -> Self {
        Self {
            key: self.key,
            kind: self.kind.clone(),
            ports: self.ports.clone(),
            meta: self.meta.clone(),
        }
    }
}

impl<D> PartialEq for NodeDef<D> {
    fn eq(&self, other: &Self) -> bool {
        self.key == other.key
            && self.kind == other.kind
            && self.ports == other.ports
            && self.meta == other.meta
    }
}

impl<D> Eq for NodeDef<D> {}

impl<D> fmt::Debug for NodeDef<D> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("NodeDef")
            .field("key", &self.key)
            .field("kind", &self.kind)
            .field("ports", &self.ports)
            .field("meta", &self.meta)
            .finish()
    }
}

/// The restricted initial closed set of authored node kinds.
#[non_exhaustive]
pub enum NodeKind<D> {
    /// A level constant with no inputs and one level output after validation.
    Constant(ConstantConfig<D>),
    /// A level inverter with one level input and one level output after validation.
    Not,
    /// A total variadic level conjunction with one level output after validation.
    All,
    /// A total variadic level disjunction with one level output after validation.
    Any,
    /// A total variadic odd-parity operation with one level output after validation.
    Parity,
    /// A total variadic level threshold with one level output after validation.
    AtLeast(AtLeastConfig),
    /// A fixed level branch selector with one level output after validation.
    Select,
    /// A total variadic pulse multiplicity merge with one pulse output after validation.
    Merge,
    /// A unary pulse presence bound with one pulse output after validation.
    Coalesce,
    /// A nonempty variadic pulse grouping operation with one pulse output after validation.
    Zip,
    /// A fixed High-enabled pulse gate with one pulse output after validation.
    PulseGate,
    /// A fixed level-controlled pulse selector with one pulse output after validation.
    PulseSelect,
    /// A fixed level-controlled pulse router with two pulse outputs after validation.
    PulseRoute,
    /// A pulse-controlled stored level with one pulse input and one level output.
    Toggle(ToggleConfig),
    /// A temporal pulse reproducer with one pulse input and one pulse output.
    PulseDelay(PulseDelayConfig<D>),
}

impl<D> Clone for NodeKind<D> {
    fn clone(&self) -> Self {
        match self {
            Self::Constant(config) => Self::constant(config.value()),
            Self::Not => Self::Not,
            Self::All => Self::All,
            Self::Any => Self::Any,
            Self::Parity => Self::Parity,
            Self::AtLeast(config) => Self::AtLeast(*config),
            Self::Select => Self::Select,
            Self::Merge => Self::Merge,
            Self::Coalesce => Self::Coalesce,
            Self::Zip => Self::Zip,
            Self::PulseGate => Self::PulseGate,
            Self::PulseSelect => Self::PulseSelect,
            Self::PulseRoute => Self::PulseRoute,
            Self::Toggle(config) => Self::Toggle(*config),
            Self::PulseDelay(config) => Self::PulseDelay(*config),
        }
    }
}

impl<D> PartialEq for NodeKind<D> {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Constant(left), Self::Constant(right)) => left.value() == right.value(),
            (Self::Not, Self::Not)
            | (Self::All, Self::All)
            | (Self::Any, Self::Any)
            | (Self::Parity, Self::Parity)
            | (Self::Select, Self::Select)
            | (Self::Merge, Self::Merge)
            | (Self::Coalesce, Self::Coalesce)
            | (Self::Zip, Self::Zip)
            | (Self::PulseGate, Self::PulseGate)
            | (Self::PulseSelect, Self::PulseSelect)
            | (Self::PulseRoute, Self::PulseRoute) => true,
            (Self::AtLeast(left), Self::AtLeast(right)) => left == right,
            (Self::Toggle(left), Self::Toggle(right)) => left == right,
            (Self::PulseDelay(left), Self::PulseDelay(right)) => left == right,
            _ => false,
        }
    }
}

impl<D> Eq for NodeKind<D> {}

impl<D> fmt::Debug for NodeKind<D> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Constant(config) => formatter.debug_tuple("Constant").field(config).finish(),
            Self::Not => formatter.write_str("Not"),
            Self::All => formatter.write_str("All"),
            Self::Any => formatter.write_str("Any"),
            Self::Parity => formatter.write_str("Parity"),
            Self::AtLeast(config) => formatter.debug_tuple("AtLeast").field(config).finish(),
            Self::Select => formatter.write_str("Select"),
            Self::Merge => formatter.write_str("Merge"),
            Self::Coalesce => formatter.write_str("Coalesce"),
            Self::Zip => formatter.write_str("Zip"),
            Self::PulseGate => formatter.write_str("PulseGate"),
            Self::PulseSelect => formatter.write_str("PulseSelect"),
            Self::PulseRoute => formatter.write_str("PulseRoute"),
            Self::Toggle(config) => formatter.debug_tuple("Toggle").field(config).finish(),
            Self::PulseDelay(config) => formatter.debug_tuple("PulseDelay").field(config).finish(),
        }
    }
}

impl<D> NodeKind<D> {
    /// Creates the constant node kind with its configured level.
    #[must_use]
    pub const fn constant(value: LogicLevel) -> Self {
        Self::Constant(ConstantConfig::new(value))
    }

    /// Creates the level-inverter node kind.
    #[must_use]
    pub const fn not() -> Self {
        Self::Not
    }

    /// Creates the total variadic level-conjunction node kind.
    #[must_use]
    pub const fn all() -> Self {
        Self::All
    }

    /// Creates the total variadic level-disjunction node kind.
    #[must_use]
    pub const fn any() -> Self {
        Self::Any
    }

    /// Creates the total variadic odd-parity node kind.
    #[must_use]
    pub const fn parity() -> Self {
        Self::Parity
    }

    /// Creates the total variadic threshold node kind.
    #[must_use]
    pub const fn at_least(threshold: u64) -> Self {
        Self::AtLeast(AtLeastConfig::new(threshold))
    }

    /// Creates the fixed level branch-selector node kind.
    #[must_use]
    pub const fn select() -> Self {
        Self::Select
    }

    /// Creates the total variadic pulse-merge node kind.
    #[must_use]
    pub const fn merge() -> Self {
        Self::Merge
    }

    /// Creates the unary pulse-presence bound node kind.
    #[must_use]
    pub const fn coalesce() -> Self {
        Self::Coalesce
    }

    /// Creates the nonempty variadic pulse-grouping node kind.
    #[must_use]
    pub const fn zip() -> Self {
        Self::Zip
    }

    /// Creates the fixed High-enabled pulse-gate node kind.
    #[must_use]
    pub const fn pulse_gate() -> Self {
        Self::PulseGate
    }

    /// Creates the fixed level-controlled pulse-selector node kind.
    #[must_use]
    pub const fn pulse_select() -> Self {
        Self::PulseSelect
    }

    /// Creates the fixed level-controlled pulse-router node kind.
    #[must_use]
    pub const fn pulse_route() -> Self {
        Self::PulseRoute
    }

    /// Creates a pulse-controlled Toggle with explicit declared initial state.
    #[must_use]
    pub const fn toggle(initial: LogicLevel) -> Self {
        Self::Toggle(ToggleConfig::new(initial))
    }

    /// Creates a PulseDelay with the supplied positive delay.
    #[must_use]
    pub const fn pulse_delay(delay: NonZeroSpan<D>) -> Self {
        Self::PulseDelay(PulseDelayConfig::new(delay))
    }
}

/// The semantic configuration of an authored [`NodeKind::Constant`] claim.
pub struct ConstantConfig<D> {
    value: LogicLevel,
    domain: PhantomData<fn() -> D>,
}

impl<D> Clone for ConstantConfig<D> {
    fn clone(&self) -> Self {
        Self::new(self.value)
    }
}

impl<D> PartialEq for ConstantConfig<D> {
    fn eq(&self, other: &Self) -> bool {
        self.value == other.value
    }
}

impl<D> Eq for ConstantConfig<D> {}

impl<D> fmt::Debug for ConstantConfig<D> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ConstantConfig")
            .field("value", &self.value)
            .finish()
    }
}

impl<D> ConstantConfig<D> {
    /// Creates a constant configuration with the supplied established level.
    #[must_use]
    pub const fn new(value: LogicLevel) -> Self {
        Self {
            value,
            domain: PhantomData,
        }
    }

    /// Returns the configured constant level.
    #[must_use]
    pub const fn value(&self) -> LogicLevel {
        self.value
    }
}

/// The semantic configuration of an authored [`NodeKind::AtLeast`] claim.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AtLeastConfig {
    /// The number of High input ports required for a High result.
    pub threshold: u64,
}

impl AtLeastConfig {
    /// Creates a threshold configuration.
    #[must_use]
    pub const fn new(threshold: u64) -> Self {
        Self { threshold }
    }
}

/// The semantic configuration of an authored [`NodeKind::Toggle`] claim.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ToggleConfig {
    /// The declared stored level used as previous state by the first reaction.
    pub initial: LogicLevel,
}

/// The semantic configuration of an authored [`NodeKind::PulseDelay`] claim.
pub struct PulseDelayConfig<D> {
    /// The strictly positive delay between input and reproduced output.
    pub delay: NonZeroSpan<D>,
}

impl<D> Clone for PulseDelayConfig<D> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<D> Copy for PulseDelayConfig<D> {}

impl<D> PartialEq for PulseDelayConfig<D> {
    fn eq(&self, other: &Self) -> bool {
        self.delay == other.delay
    }
}

impl<D> Eq for PulseDelayConfig<D> {}

impl<D> Hash for PulseDelayConfig<D> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.delay.hash(state);
    }
}

impl<D> fmt::Debug for PulseDelayConfig<D> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PulseDelayConfig")
            .field("delay", &self.delay)
            .finish()
    }
}

impl<D> PulseDelayConfig<D> {
    /// Creates a PulseDelay configuration with a statically nonzero delay.
    #[must_use]
    pub const fn new(delay: NonZeroSpan<D>) -> Self {
        Self { delay }
    }
}

impl ToggleConfig {
    /// Creates a Toggle configuration with explicit declared initial state.
    #[must_use]
    pub const fn new(initial: LogicLevel) -> Self {
        Self { initial }
    }
}

/// The semantic role of one input port.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum InputPortRole {
    /// An ordinary fixed or variadic level input.
    Input,
    /// The selector input of [`NodeKind::Select`].
    Selector,
    /// The branch selected when the selector is Low.
    WhenLow,
    /// The branch selected when the selector is High.
    WhenHigh,
    /// The pulse control input of [`NodeKind::Toggle`].
    Toggle,
    /// The pulse input of [`NodeKind::PulseDelay`].
    PulseDelay,
    /// The pulse batch controlled by a level-controlled pulse primitive.
    Pulses,
    /// The High-enabled control of [`NodeKind::PulseGate`].
    Enable,
}

/// The semantic role of one output port.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum OutputPortRole {
    /// The ordinary output of a one-output primitive.
    Output,
    /// The [`NodeKind::PulseRoute`] output selected by a Low control.
    WhenLow,
    /// The [`NodeKind::PulseRoute`] output selected by a High control.
    WhenHigh,
}

/// The claimed typed port identities of one node.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodePorts {
    inputs: Vec<AnyInPortKey>,
    input_roles: Vec<InputPortRole>,
    outputs: Vec<AnyOutPortKey>,
    output_roles: Vec<OutputPortRole>,
}

impl NodePorts {
    /// Creates a port claim without enforcing any node-kind shape.
    #[must_use]
    pub fn new(inputs: Vec<AnyInPortKey>, outputs: Vec<AnyOutPortKey>) -> Self {
        let input_roles = vec![InputPortRole::Input; inputs.len()];
        let output_roles = vec![OutputPortRole::Output; outputs.len()];
        Self {
            inputs,
            input_roles,
            outputs,
            output_roles,
        }
    }

    /// Creates a port claim with explicit per-input semantic roles.
    ///
    /// Role and input sequences are retained losslessly even when their lengths
    /// or contents are invalid for the claimed node kind.
    #[must_use]
    pub fn with_input_roles(
        inputs: Vec<AnyInPortKey>,
        input_roles: Vec<InputPortRole>,
        outputs: Vec<AnyOutPortKey>,
    ) -> Self {
        let output_roles = vec![OutputPortRole::Output; outputs.len()];
        Self {
            inputs,
            input_roles,
            outputs,
            output_roles,
        }
    }

    /// Creates a port claim with explicit per-input and per-output semantic roles.
    ///
    /// Role and port sequences are retained losslessly even when their lengths
    /// or contents are invalid for the claimed node kind.
    #[must_use]
    pub fn with_roles(
        inputs: Vec<AnyInPortKey>,
        input_roles: Vec<InputPortRole>,
        outputs: Vec<AnyOutPortKey>,
        output_roles: Vec<OutputPortRole>,
    ) -> Self {
        Self {
            inputs,
            input_roles,
            outputs,
            output_roles,
        }
    }

    /// Returns every claimed input port in supplied order.
    #[must_use]
    pub fn inputs(&self) -> &[AnyInPortKey] {
        &self.inputs
    }

    /// Returns the semantic-role claims corresponding to the input sequence.
    #[must_use]
    pub fn input_roles(&self) -> &[InputPortRole] {
        &self.input_roles
    }

    /// Returns every claimed output port in supplied order.
    #[must_use]
    pub fn outputs(&self) -> &[AnyOutPortKey] {
        &self.outputs
    }

    /// Returns the semantic-role claims corresponding to the output sequence.
    #[must_use]
    pub fn output_roles(&self) -> &[OutputPortRole] {
        &self.output_roles
    }

    /// Consumes the port claim into its input and output sequences.
    #[must_use]
    pub fn into_parts(
        self,
    ) -> (
        Vec<AnyInPortKey>,
        Vec<InputPortRole>,
        Vec<AnyOutPortKey>,
        Vec<OutputPortRole>,
    ) {
        (
            self.inputs,
            self.input_roles,
            self.outputs,
            self.output_roles,
        )
    }
}

/// An endpoint identity retained in a connection claim.
///
/// Unlike a validated connection, either endpoint position can contain any
/// structural endpoint category so that a direction-invalid authored claim is
/// available to later validation.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionEndpoint {
    /// An external network input.
    ExternalInput(AnyExternalInputKey),
    /// A node input port.
    NodeInput(AnyInPortKey),
    /// A node output port.
    NodeOutput(AnyOutPortKey),
    /// A public input of the reusable module currently being authored.
    ModuleInput(AnyModuleInputKey),
    /// One exact public output of one contained module instance.
    ModuleOutput {
        instance: ModuleInstanceKey,
        output: AnyModuleOutputKey,
    },
    /// An external network output.
    ExternalOutput(AnyExternalOutputKey),
}

impl ConnectionEndpoint {
    /// Creates an external-input endpoint claim.
    #[must_use]
    pub const fn external_input(key: AnyExternalInputKey) -> Self {
        Self::ExternalInput(key)
    }

    /// Creates a node-input endpoint claim.
    #[must_use]
    pub const fn node_input(key: AnyInPortKey) -> Self {
        Self::NodeInput(key)
    }

    /// Creates a node-output endpoint claim.
    #[must_use]
    pub const fn node_output(key: AnyOutPortKey) -> Self {
        Self::NodeOutput(key)
    }

    /// Creates a containing-module public-input source claim.
    #[must_use]
    pub const fn module_input(key: AnyModuleInputKey) -> Self {
        Self::ModuleInput(key)
    }

    /// Creates an exact contained module-output source claim.
    #[must_use]
    pub const fn module_output(instance: ModuleInstanceKey, output: AnyModuleOutputKey) -> Self {
        Self::ModuleOutput { instance, output }
    }

    /// Creates an external-output endpoint claim.
    #[must_use]
    pub const fn external_output(key: AnyExternalOutputKey) -> Self {
        Self::ExternalOutput(key)
    }

    /// Returns the signal kind retained by this endpoint identity.
    #[must_use]
    pub const fn kind(self) -> SignalKind {
        match self {
            Self::ExternalInput(key) => key.kind(),
            Self::NodeInput(key) => key.kind(),
            Self::NodeOutput(key) => key.kind(),
            Self::ModuleInput(key) => key.kind(),
            Self::ModuleOutput { output, .. } => output.kind(),
            Self::ExternalOutput(key) => key.kind(),
        }
    }
}

impl From<AnyExternalInputKey> for ConnectionEndpoint {
    fn from(key: AnyExternalInputKey) -> Self {
        Self::ExternalInput(key)
    }
}

impl From<AnyInPortKey> for ConnectionEndpoint {
    fn from(key: AnyInPortKey) -> Self {
        Self::NodeInput(key)
    }
}

impl From<AnyOutPortKey> for ConnectionEndpoint {
    fn from(key: AnyOutPortKey) -> Self {
        Self::NodeOutput(key)
    }
}

impl From<AnyExternalOutputKey> for ConnectionEndpoint {
    fn from(key: AnyExternalOutputKey) -> Self {
        Self::ExternalOutput(key)
    }
}

macro_rules! endpoint_from_typed_keys {
    ($signal:ty) => {
        impl From<ExternalInputKey<$signal>> for ConnectionEndpoint {
            fn from(key: ExternalInputKey<$signal>) -> Self {
                Self::ExternalInput(key.into())
            }
        }

        impl From<InPortKey<$signal>> for ConnectionEndpoint {
            fn from(key: InPortKey<$signal>) -> Self {
                Self::NodeInput(key.into())
            }
        }

        impl From<OutPortKey<$signal>> for ConnectionEndpoint {
            fn from(key: OutPortKey<$signal>) -> Self {
                Self::NodeOutput(key.into())
            }
        }

        impl From<ExternalOutputKey<$signal>> for ConnectionEndpoint {
            fn from(key: ExternalOutputKey<$signal>) -> Self {
                Self::ExternalOutput(key.into())
            }
        }
    };
}

endpoint_from_typed_keys!(Level);
endpoint_from_typed_keys!(Pulse);

/// One stable-keyed authored connection claim.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectionDef {
    key: ConnectionKey,
    from: ConnectionEndpoint,
    to: ConnectionEndpoint,
    meta: DiagnosticMeta,
}

impl ConnectionDef {
    /// Creates a connection claim without validating endpoint direction or kind.
    #[must_use]
    pub const fn new(
        key: ConnectionKey,
        from: ConnectionEndpoint,
        to: ConnectionEndpoint,
        meta: DiagnosticMeta,
    ) -> Self {
        Self {
            key,
            from,
            to,
            meta,
        }
    }

    /// Returns the connection's stable authored identity.
    #[must_use]
    pub const fn key(&self) -> ConnectionKey {
        self.key
    }

    /// Returns the claimed source-position endpoint.
    #[must_use]
    pub const fn from(&self) -> ConnectionEndpoint {
        self.from
    }

    /// Returns the claimed destination-position endpoint.
    #[must_use]
    pub const fn to(&self) -> ConnectionEndpoint {
        self.to
    }

    /// Returns presentation metadata attached to this connection.
    #[must_use]
    pub const fn meta(&self) -> &DiagnosticMeta {
        &self.meta
    }

    /// Consumes this connection claim into its complete owned parts.
    #[must_use]
    pub fn into_parts(
        self,
    ) -> (
        ConnectionKey,
        ConnectionEndpoint,
        ConnectionEndpoint,
        DiagnosticMeta,
    ) {
        (self.key, self.from, self.to, self.meta)
    }
}

/// One stable-keyed external input boundary claim.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalInputDef {
    key: AnyExternalInputKey,
    meta: DiagnosticMeta,
}

impl ExternalInputDef {
    /// Creates an external input boundary without validation.
    #[must_use]
    pub const fn new(key: AnyExternalInputKey, meta: DiagnosticMeta) -> Self {
        Self { key, meta }
    }

    /// Returns the stable typed external-input identity.
    #[must_use]
    pub const fn key(&self) -> AnyExternalInputKey {
        self.key
    }

    /// Returns presentation metadata attached to this boundary.
    #[must_use]
    pub const fn meta(&self) -> &DiagnosticMeta {
        &self.meta
    }
}

/// One stable-keyed external output boundary claim.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalOutputDef {
    key: AnyExternalOutputKey,
    source: AnySignalSourceKey,
    meta: DiagnosticMeta,
}

impl ExternalOutputDef {
    /// Creates an external output boundary without validating its source.
    #[must_use]
    pub const fn new(
        key: AnyExternalOutputKey,
        source: AnySignalSourceKey,
        meta: DiagnosticMeta,
    ) -> Self {
        Self { key, source, meta }
    }

    /// Returns the stable typed external-output identity.
    #[must_use]
    pub const fn key(&self) -> AnyExternalOutputKey {
        self.key
    }

    /// Returns the claimed exposed internal signal source.
    #[must_use]
    pub const fn source(&self) -> AnySignalSourceKey {
        self.source
    }

    /// Returns presentation metadata attached to this boundary.
    #[must_use]
    pub const fn meta(&self) -> &DiagnosticMeta {
        &self.meta
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::key::{
        ExternalInputKey, ExternalOutputKey, InPortKey, ModuleInputKey, ModuleOutputKey,
        OutPortKey, SignalSourceKey,
    };

    fn meta(name: &str) -> DiagnosticMeta {
        DiagnosticMeta {
            name: Some(name.into()),
            ..DiagnosticMeta::default()
        }
    }

    fn representative_module<D>() -> UncheckedModule<D> {
        let level_input = ModuleInputKey::<Level>::from_u128(10);
        let pulse_input = ModuleInputKey::<Pulse>::from_u128(11);
        let level_output = ModuleOutputKey::<Level>::from_u128(12);
        let pulse_output = ModuleOutputKey::<Pulse>::from_u128(13);
        let not_input = InPortKey::<Level>::from_u128(20);
        let not_output = OutPortKey::<Level>::from_u128(21);
        let merge_input = InPortKey::<Pulse>::from_u128(22);
        let merge_output = OutPortKey::<Pulse>::from_u128(23);

        UncheckedModule::new_user(
            meta("mixed module"),
            vec![
                ModuleInputDef::new(level_input.into(), meta("level in")),
                ModuleInputDef::new(pulse_input.into(), meta("pulse in")),
            ],
            vec![
                ModuleOutputDef::new(level_output.into(), meta("level out")),
                ModuleOutputDef::new(pulse_output.into(), meta("pulse out")),
            ],
            vec![
                ModuleInterfaceMapping::input(level_input.into(), not_input.into()),
                ModuleInterfaceMapping::input(pulse_input.into(), merge_input.into()),
                ModuleInterfaceMapping::output(level_output.into(), not_output.into()),
                ModuleInterfaceMapping::output(pulse_output.into(), merge_output.into()),
            ],
            vec![
                NodeDef::new(
                    NodeKey::from_u128(30),
                    NodeKind::Not,
                    NodePorts::new(vec![not_input.into()], vec![not_output.into()]),
                    meta("inverter"),
                ),
                NodeDef::new(
                    NodeKey::from_u128(31),
                    NodeKind::Merge,
                    NodePorts::new(vec![merge_input.into()], vec![merge_output.into()]),
                    meta("pulse merge"),
                ),
            ],
            vec![ConnectionDef::new(
                ConnectionKey::from_u128(40),
                not_output.into(),
                not_input.into(),
                meta("retained malformed feedback"),
            )],
        )
    }

    #[test]
    fn represents_a_valid_restricted_level_definition() {
        let constant_output = OutPortKey::<Level>::from_u128(10);
        let not_input = InPortKey::<Level>::from_u128(11);
        let not_output = OutPortKey::<Level>::from_u128(12);
        let network = UncheckedNetwork::<()>::new(
            NetworkKey::from_u128(1),
            crate::identity::TimeDomainId::from_u128(1),
            meta("network"),
            vec![
                NodeDef::new(
                    NodeKey::from_u128(2),
                    NodeKind::Constant(ConstantConfig::new(LogicLevel::High)),
                    NodePorts::new(vec![], vec![constant_output.into()]),
                    meta("constant"),
                ),
                NodeDef::new(
                    NodeKey::from_u128(3),
                    NodeKind::Not,
                    NodePorts::new(vec![not_input.into()], vec![not_output.into()]),
                    meta("not"),
                ),
            ],
            vec![],
            vec![ExternalOutputDef::new(
                ExternalOutputKey::<Level>::from_u128(4).into(),
                SignalSourceKey::NodeOutput(not_output).into(),
                meta("result"),
            )],
            vec![ConnectionDef::new(
                ConnectionKey::from_u128(5),
                constant_output.into(),
                not_input.into(),
                meta("constant feeds not"),
            )],
        );

        assert_eq!(network.key(), NetworkKey::from_u128(1));
        assert_eq!(network.nodes().len(), 2);
        assert_eq!(network.connections().len(), 1);
        assert_eq!(network.meta().name.as_deref(), Some("network"));
        assert!(matches!(
            network.nodes()[0].kind(),
            NodeKind::Constant(config) if config.value() == LogicLevel::High
        ));
    }

    #[test]
    fn retains_level_and_pulse_endpoint_kinds() {
        let level_input = ExternalInputKey::<Level>::from_u128(1);
        let pulse_input = ExternalInputKey::<Pulse>::from_u128(2);
        let level_output = OutPortKey::<Level>::from_u128(3);
        let pulse_port = InPortKey::<Pulse>::from_u128(4);

        let level = ConnectionEndpoint::from(AnyExternalInputKey::from(level_input));
        let pulse = ConnectionEndpoint::from(AnyExternalInputKey::from(pulse_input));
        assert_eq!(level.kind(), SignalKind::Level);
        assert_eq!(pulse.kind(), SignalKind::Pulse);

        let ports = NodePorts::new(vec![pulse_port.into()], vec![level_output.into()]);
        assert_eq!(ports.inputs()[0].kind(), SignalKind::Pulse);
        assert_eq!(ports.outputs()[0].kind(), SignalKind::Level);
    }

    #[test]
    fn attaches_metadata_to_authored_records() {
        let connection = ConnectionDef::new(
            ConnectionKey::from_u128(1),
            OutPortKey::<Level>::from_u128(2).into(),
            InPortKey::<Level>::from_u128(3).into(),
            meta("wire"),
        );
        let input = ExternalInputDef::new(
            ExternalInputKey::<Pulse>::from_u128(4).into(),
            meta("trigger"),
        );

        assert_eq!(connection.meta().name.as_deref(), Some("wire"));
        assert_eq!(input.meta().name.as_deref(), Some("trigger"));
    }

    #[test]
    fn directly_constructs_malformed_claims_without_repair() {
        let malformed = ConnectionDef::new(
            ConnectionKey::from_u128(1),
            InPortKey::<Pulse>::from_u128(2).into(),
            ExternalOutputKey::<Level>::from_u128(3).into(),
            DiagnosticMeta::default(),
        );
        let node = NodeDef::<()>::new(
            NodeKey::from_u128(4),
            NodeKind::Not,
            NodePorts::new(vec![], vec![]),
            DiagnosticMeta::default(),
        );

        assert!(matches!(malformed.from(), ConnectionEndpoint::NodeInput(_)));
        assert!(matches!(
            malformed.to(),
            ConnectionEndpoint::ExternalOutput(_)
        ));
        assert!(node.ports().inputs().is_empty());
        assert!(node.ports().outputs().is_empty());
    }

    #[test]
    fn retains_duplicate_keys_as_distinct_claims() {
        let duplicate = ExternalInputKey::<Level>::from_u128(7);
        let network = UncheckedNetwork::<()>::new(
            NetworkKey::from_u128(1),
            crate::identity::TimeDomainId::from_u128(1),
            DiagnosticMeta::default(),
            vec![],
            vec![
                ExternalInputDef::new(duplicate.into(), meta("first")),
                ExternalInputDef::new(duplicate.into(), meta("second")),
            ],
            vec![],
            vec![],
        );

        assert_eq!(network.external_inputs().len(), 2);
        assert_eq!(
            network.external_inputs()[0].key(),
            network.external_inputs()[1].key()
        );
        assert_eq!(
            network.external_inputs()[0].meta().name.as_deref(),
            Some("first")
        );
        assert_eq!(
            network.external_inputs()[1].meta().name.as_deref(),
            Some("second")
        );
    }

    #[test]
    fn represents_a_mixed_user_module_with_ordinary_private_structure() {
        let module = representative_module::<()>();

        assert_eq!(module.origin(), UncheckedModuleOrigin::User);
        assert_eq!(module.meta().name.as_deref(), Some("mixed module"));
        assert_eq!(module.inputs().len(), 2);
        assert_eq!(module.outputs().len(), 2);
        assert_eq!(module.mappings().len(), 4);
        assert_eq!(module.nodes().len(), 2);
        assert_eq!(module.connections().len(), 1);
        assert_eq!(module.inputs()[0].key().kind(), SignalKind::Level);
        assert_eq!(module.inputs()[1].key().kind(), SignalKind::Pulse);
        assert_eq!(module.outputs()[0].key().kind(), SignalKind::Level);
        assert_eq!(module.outputs()[1].key().kind(), SignalKind::Pulse);
        assert!(matches!(module.nodes()[0].kind(), NodeKind::Not));
        assert!(matches!(module.nodes()[1].kind(), NodeKind::Merge));
        assert_eq!(module.connections()[0].key(), ConnectionKey::from_u128(40));
        assert_eq!(module.inputs()[0].meta().name.as_deref(), Some("level in"));
        assert_eq!(
            module.outputs()[1].meta().name.as_deref(),
            Some("pulse out")
        );
    }

    #[test]
    fn retains_malformed_module_claims_without_repair() {
        let duplicate_input = ModuleInputKey::<Level>::from_u128(1);
        let incomplete_output = ModuleOutputKey::<Pulse>::from_u128(2);
        let duplicate_node = NodeKey::from_u128(3);
        let wrong_kind_target = InPortKey::<Pulse>::from_u128(4);
        let direction_invalid_source = InPortKey::<Level>::from_u128(5);
        let malformed_connection = ConnectionDef::new(
            ConnectionKey::from_u128(6),
            InPortKey::<Pulse>::from_u128(7).into(),
            OutPortKey::<Level>::from_u128(8).into(),
            meta("wrong directions"),
        );
        let module = UncheckedModule::<()>::new_user(
            DiagnosticMeta::default(),
            vec![
                ModuleInputDef::new(duplicate_input.into(), meta("first")),
                ModuleInputDef::new(duplicate_input.into(), meta("second")),
            ],
            vec![ModuleOutputDef::new(
                incomplete_output.into(),
                meta("unmapped"),
            )],
            vec![
                ModuleInterfaceMapping::input(duplicate_input.into(), wrong_kind_target.into()),
                ModuleInterfaceMapping::output(
                    ModuleOutputKey::<Level>::from_u128(99).into(),
                    direction_invalid_source.into(),
                ),
            ],
            vec![
                NodeDef::new(
                    duplicate_node,
                    NodeKind::Not,
                    NodePorts::new(vec![], vec![]),
                    meta("first node"),
                ),
                NodeDef::new(
                    duplicate_node,
                    NodeKind::Merge,
                    NodePorts::new(vec![], vec![]),
                    meta("second node"),
                ),
            ],
            vec![malformed_connection],
        );

        assert_eq!(module.inputs().len(), 2);
        assert_eq!(module.inputs()[0].key(), module.inputs()[1].key());
        assert_eq!(module.nodes().len(), 2);
        assert_eq!(module.nodes()[0].key(), module.nodes()[1].key());
        assert_eq!(module.mappings().len(), 2);
        assert!(matches!(
            module.mappings()[0],
            ModuleInterfaceMapping::Input { input, target }
                if input.kind() == SignalKind::Level
                    && target.kind() == SignalKind::Pulse
        ));
        assert!(matches!(
            module.mappings()[1],
            ModuleInterfaceMapping::Output {
                source: ConnectionEndpoint::NodeInput(_),
                ..
            }
        ));
        assert_eq!(module.outputs().len(), 1);
        assert_eq!(module.outputs()[0].key(), incomplete_output.into());
        assert_eq!(module.connections().len(), 1);
        assert!(matches!(
            module.connections()[0].from(),
            ConnectionEndpoint::NodeInput(_)
        ));
        assert!(matches!(
            module.connections()[0].to(),
            ConnectionEndpoint::NodeOutput(_)
        ));
    }

    #[test]
    fn raw_module_equality_preserves_order_and_metadata_without_rebinding_keys() {
        let original = representative_module::<()>();
        assert_eq!(original, original.clone());

        let mut reordered_inputs = original.inputs().to_vec();
        reordered_inputs.reverse();
        let reordered = UncheckedModule::new_user(
            original.meta().clone(),
            reordered_inputs,
            original.outputs().to_vec(),
            original.mappings().to_vec(),
            original.nodes().to_vec(),
            original.connections().to_vec(),
        );
        assert_ne!(original, reordered);

        let renamed = UncheckedModule::new_user(
            meta("renamed module"),
            original.inputs().to_vec(),
            original.outputs().to_vec(),
            original.mappings().to_vec(),
            original.nodes().to_vec(),
            original.connections().to_vec(),
        );
        assert_ne!(original, renamed);
        assert_eq!(original.inputs()[0].key(), renamed.inputs()[0].key());
        assert_eq!(original.nodes()[0].key(), renamed.nodes()[0].key());
    }

    #[test]
    fn module_value_traits_do_not_require_domain_traits() {
        enum TraitHostileDomain {}

        fn assert_value_traits<T: Clone + fmt::Debug + Eq>() {}

        assert_value_traits::<UncheckedModule<TraitHostileDomain>>();
        let empty = UncheckedModule::<TraitHostileDomain>::new_user(
            DiagnosticMeta::default(),
            vec![],
            vec![],
            vec![],
            vec![],
            vec![],
        );
        assert_eq!(empty, empty.clone());
        assert!(empty.inputs().is_empty());
        assert!(empty.outputs().is_empty());
        assert!(empty.nodes().is_empty());
    }
}
