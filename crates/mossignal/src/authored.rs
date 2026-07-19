//! Lossless authored network definitions awaiting structural validation.
//!
//! This module deliberately represents structure only. It does not validate,
//! compile, or execute a network.

use crate::key::{
    AnyExternalInputKey, AnyExternalOutputKey, AnyInPortKey, AnyOutPortKey, AnySignalSourceKey,
    ConnectionKey, ExternalInputKey, ExternalOutputKey, InPortKey, NetworkKey, NodeKey, OutPortKey,
};
use crate::metadata::DiagnosticMeta;
use crate::signal::{Level, LogicLevel, Pulse, SignalKind};
use core::marker::PhantomData;

/// An owned, stable-keyed network definition that may be structurally invalid.
///
/// Its collections retain every supplied claim in caller order. Structural
/// validation is a later, one-way transition and is intentionally absent here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UncheckedNetwork<D> {
    key: NetworkKey,
    meta: DiagnosticMeta,
    nodes: Vec<NodeDef<D>>,
    external_inputs: Vec<ExternalInputDef>,
    external_outputs: Vec<ExternalOutputDef>,
    connections: Vec<ConnectionDef>,
}

impl<D> UncheckedNetwork<D> {
    /// Creates an unchecked definition without interpreting or validating it.
    #[must_use]
    pub fn new(
        key: NetworkKey,
        meta: DiagnosticMeta,
        nodes: Vec<NodeDef<D>>,
        external_inputs: Vec<ExternalInputDef>,
        external_outputs: Vec<ExternalOutputDef>,
        connections: Vec<ConnectionDef>,
    ) -> Self {
        // SPEC: docs/specs/contracts/authored-network-definition.yaml "malformed-claims-retained"
        // Claim vectors must remain lossless so duplicate and malformed input reaches validation.
        Self {
            key,
            meta,
            nodes,
            external_inputs,
            external_outputs,
            connections,
        }
    }

    /// Returns the network's stable authored identity.
    #[must_use]
    pub const fn key(&self) -> NetworkKey {
        self.key
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
}

/// One stable-keyed authored node claim.
#[derive(Debug, Clone, PartialEq, Eq)]
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

/// The restricted initial closed set of authored node kinds.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NodeKind<D> {
    /// A level constant with no inputs and one level output after validation.
    Constant(ConstantConfig<D>),
    /// A level inverter with one level input and one level output after validation.
    Not,
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
}

/// The semantic configuration of an authored [`NodeKind::Constant`] claim.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConstantConfig<D> {
    value: LogicLevel,
    domain: PhantomData<fn() -> D>,
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

/// The claimed typed port identities of one node.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodePorts {
    inputs: Vec<AnyInPortKey>,
    outputs: Vec<AnyOutPortKey>,
}

impl NodePorts {
    /// Creates a port claim without enforcing any node-kind shape.
    #[must_use]
    pub fn new(inputs: Vec<AnyInPortKey>, outputs: Vec<AnyOutPortKey>) -> Self {
        Self { inputs, outputs }
    }

    /// Returns every claimed input port in supplied order.
    #[must_use]
    pub fn inputs(&self) -> &[AnyInPortKey] {
        &self.inputs
    }

    /// Returns every claimed output port in supplied order.
    #[must_use]
    pub fn outputs(&self) -> &[AnyOutPortKey] {
        &self.outputs
    }

    /// Consumes the port claim into its input and output sequences.
    #[must_use]
    pub fn into_parts(self) -> (Vec<AnyInPortKey>, Vec<AnyOutPortKey>) {
        (self.inputs, self.outputs)
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
    use crate::key::{ExternalInputKey, ExternalOutputKey, InPortKey, OutPortKey, SignalSourceKey};

    fn meta(name: &str) -> DiagnosticMeta {
        DiagnosticMeta {
            name: Some(name.into()),
            ..DiagnosticMeta::default()
        }
    }

    #[test]
    fn represents_a_valid_restricted_level_definition() {
        let constant_output = OutPortKey::<Level>::from_u128(10);
        let not_input = InPortKey::<Level>::from_u128(11);
        let not_output = OutPortKey::<Level>::from_u128(12);
        let network = UncheckedNetwork::<()>::new(
            NetworkKey::from_u128(1),
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
}
