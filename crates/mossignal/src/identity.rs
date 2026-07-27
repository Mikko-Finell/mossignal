//! Stable time-domain and semantic fingerprint identities.

use crate::authored::{ConnectionEndpoint, NodeKind, UncheckedNetwork};
use crate::key::{
    AnyExternalInputKey, AnyExternalOutputKey, AnyInPortKey, AnyOutPortKey, AnySignalSourceKey,
    SignalSourceKey,
};
use crate::signal::{LogicLevel, SignalKind};
use core::fmt;

const NETWORK_DOMAIN: &str = "mossignal/network_fingerprint/v1";
const INPUT_SCHEMA_DOMAIN: &str = "mossignal/input_schema_fingerprint/v1";

/// Caller-owned persistent identity for the meaning of one logical tick.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TimeDomainId(u128);

impl TimeDomainId {
    /// Reconstructs a caller-owned time-domain identity from its 128-bit value.
    #[must_use]
    pub const fn from_u128(value: u128) -> Self {
        Self(value)
    }

    /// Returns the exact 128-bit value used by the canonical representation.
    #[must_use]
    pub const fn as_u128(self) -> u128 {
        self.0
    }

    /// Returns the canonical unsigned big-endian representation.
    #[must_use]
    pub const fn to_be_bytes(self) -> [u8; 16] {
        self.0.to_be_bytes()
    }
}

impl fmt::Debug for TimeDomainId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "TimeDomainId({self})")
    }
}

impl fmt::Display for TimeDomainId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{:032x}", self.0)
    }
}

macro_rules! fingerprint {
    ($name:ident, $doc:literal) => {
        #[doc = $doc]
        #[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub struct $name([u8; 32]);

        impl $name {
            #[must_use]
            pub const fn as_bytes(self) -> [u8; 32] {
                self.0
            }

            const fn from_digest(bytes: [u8; 32]) -> Self {
                Self(bytes)
            }
        }

        impl fmt::Debug for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(formatter, "{}({})", stringify!($name), self)
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                for byte in self.0 {
                    write!(formatter, "{byte:02x}")?;
                }
                Ok(())
            }
        }
    };
}

fingerprint!(
    NetworkFingerprint,
    "The opaque semantic identity of one validated network."
);
fingerprint!(
    InputSchemaFingerprint,
    "The opaque identity of one complete external level-input schema."
);

pub(crate) fn fingerprints<D>(
    network: &UncheckedNetwork<D>,
) -> (NetworkFingerprint, InputSchemaFingerprint) {
    let network_bytes = network_digest_input(network);
    let input_schema_bytes = input_schema_digest_input(network);
    (
        NetworkFingerprint::from_digest(*blake3::hash(&network_bytes).as_bytes()),
        InputSchemaFingerprint::from_digest(*blake3::hash(&input_schema_bytes).as_bytes()),
    )
}

#[cfg(test)]
pub(crate) fn canonical_inputs<D>(network: &UncheckedNetwork<D>) -> (Vec<u8>, Vec<u8>) {
    (
        network_digest_input(network),
        input_schema_digest_input(network),
    )
}

fn network_digest_input<D>(network: &UncheckedNetwork<D>) -> Vec<u8> {
    let mut writer = Cbor::default();
    writer.record_start(3);
    writer.field("domain", |writer| writer.text(NETWORK_DOMAIN));
    writer.field("payload", |writer| network_payload(writer, network));
    writer.field("version", |writer| writer.uint(1));
    writer.finish()
}

fn input_schema_digest_input<D>(network: &UncheckedNetwork<D>) -> Vec<u8> {
    let mut writer = Cbor::default();
    writer.record_start(3);
    writer.field("domain", |writer| writer.text(INPUT_SCHEMA_DOMAIN));
    writer.field("payload", |writer| input_schema_payload(writer, network));
    writer.field("version", |writer| writer.uint(1));
    writer.finish()
}

fn network_payload<D>(writer: &mut Cbor, network: &UncheckedNetwork<D>) {
    // SPEC: docs/specs/persistence_canonical_encoding_and_compatibility_spec.md §54.2 "Restricted network-fingerprint projection version 1"
    // This exact record omits validation and private compilation data.
    writer.record_start(9);
    writer.field("built_in_node_semantics_version", |writer| writer.uint(1));
    writer.field("connections", |writer| {
        let mut connections: Vec<_> = network.connections().iter().collect();
        connections.sort_by_key(|connection| connection.key().as_u128());
        writer.array_start(connections.len());
        for connection in connections {
            writer.record_start(3);
            writer.field("key", |writer| writer.key(connection.key().as_u128()));
            writer.field("source", |writer| source(writer, connection.from()));
            writer.field("target", |writer| target(writer, connection.to()));
        }
    });
    writer.field("core_semantics_version", |writer| writer.uint(1));
    writer.field("external_inputs", |writer| external_inputs(writer, network));
    writer.field("external_outputs", |writer| {
        external_outputs(writer, network)
    });
    writer.field("network_key", |writer| writer.key(network.key().as_u128()));
    writer.field("nodes", |writer| nodes(writer, network));
    writer.field("ports", |writer| ports(writer, network));
    writer.field("time_domain_id", |writer| {
        writer.bytes(&network.time_domain_id().to_be_bytes())
    });
}

fn input_schema_payload<D>(writer: &mut Cbor, network: &UncheckedNetwork<D>) {
    writer.record_start(1);
    writer.field("inputs", |writer| {
        let mut inputs: Vec<_> = network.external_inputs().iter().collect();
        inputs.sort_by_key(|input| external_input_order(input.key()));
        writer.array_start(inputs.len());
        for input in inputs {
            let key = level_external_input(input.key());
            writer.record_start(3);
            writer.field("establishment", |writer| writer.variant_null("required"));
            writer.field("key", |writer| writer.key(key));
            writer.field("signal_kind", |writer| writer.variant_null("level"));
        }
    });
}

fn nodes<D>(writer: &mut Cbor, network: &UncheckedNetwork<D>) {
    let mut nodes: Vec<_> = network.nodes().iter().collect();
    nodes.sort_by_key(|node| node.key().as_u128());
    writer.array_start(nodes.len());
    for node in nodes {
        writer.record_start(2);
        writer.field("key", |writer| writer.key(node.key().as_u128()));
        writer.field("kind", |writer| match node.kind() {
            NodeKind::Constant(config) => {
                writer.variant_start("constant");
                writer.record_start(1);
                writer.field("value", |writer| logic_level(writer, config.value()));
            }
            NodeKind::Not => writer.variant_null("not"),
        });
    }
}

fn ports<D>(writer: &mut Cbor, network: &UncheckedNetwork<D>) {
    let mut ports = Vec::new();
    for node in network.nodes() {
        ports.extend(
            node.ports()
                .inputs()
                .iter()
                .map(|key| (true, key.kind(), level_in_port(*key), node.key().as_u128())),
        );
        ports.extend(node.ports().outputs().iter().map(|key| {
            (
                false,
                key.kind(),
                level_out_port(*key),
                node.key().as_u128(),
            )
        }));
    }
    ports.sort_by_key(|(input, kind, key, _)| (signal_kind_tag(*kind), u8::from(!*input), *key));
    writer.array_start(ports.len());
    for (input, kind, key, owner) in ports {
        writer.record_start(5);
        writer.field("direction", |writer| {
            writer.variant_null(if input { "input" } else { "output" });
        });
        writer.field("key", |writer| writer.key(key));
        writer.field("owner", |writer| writer.key(owner));
        writer.field("semantic_role", |writer| {
            writer.variant_null(if input { "input" } else { "output" });
        });
        writer.field("signal_kind", |writer| signal_kind(writer, kind));
    }
}

fn external_inputs<D>(writer: &mut Cbor, network: &UncheckedNetwork<D>) {
    let mut inputs: Vec<_> = network.external_inputs().iter().collect();
    inputs.sort_by_key(|input| external_input_order(input.key()));
    writer.array_start(inputs.len());
    for input in inputs {
        let key = level_external_input(input.key());
        writer.record_start(2);
        writer.field("key", |writer| writer.key(key));
        writer.field("signal_kind", |writer| writer.variant_null("level"));
    }
}

fn external_outputs<D>(writer: &mut Cbor, network: &UncheckedNetwork<D>) {
    let mut outputs: Vec<_> = network.external_outputs().iter().collect();
    outputs.sort_by_key(|output| external_output_order(output.key()));
    writer.array_start(outputs.len());
    for output in outputs {
        let key = level_external_output(output.key());
        writer.record_start(3);
        writer.field("key", |writer| writer.key(key));
        writer.field("signal_kind", |writer| writer.variant_null("level"));
        writer.field("source", |writer| {
            source_from_signal(writer, output.source())
        });
    }
}

fn source(writer: &mut Cbor, endpoint: ConnectionEndpoint) {
    match endpoint {
        ConnectionEndpoint::ExternalInput(key) => source_external_input(writer, key),
        ConnectionEndpoint::NodeOutput(key) => source_out_port(writer, key),
        ConnectionEndpoint::NodeInput(_) | ConnectionEndpoint::ExternalOutput(_) => {
            panic!(
                "validated restricted connection source must be an external input or output port"
            )
        }
    }
}

fn source_from_signal(writer: &mut Cbor, source: AnySignalSourceKey) {
    match source {
        AnySignalSourceKey::Level(SignalSourceKey::ExternalInput(key)) => {
            source_external_input(writer, key.into());
        }
        AnySignalSourceKey::Level(SignalSourceKey::NodeOutput(key)) => {
            source_out_port(writer, key.into())
        }
        AnySignalSourceKey::Pulse(_) => {
            panic!("validated restricted network must contain only level signals")
        }
    }
}

fn source_external_input(writer: &mut Cbor, key: AnyExternalInputKey) {
    writer.variant_start("external_input");
    writer.record_start(2);
    writer.field("key", |writer| writer.key(level_external_input(key)));
    writer.field("signal_kind", |writer| writer.variant_null("level"));
}

fn source_out_port(writer: &mut Cbor, key: AnyOutPortKey) {
    writer.variant_start("out_port");
    writer.record_start(2);
    writer.field("key", |writer| writer.key(level_out_port(key)));
    writer.field("signal_kind", |writer| writer.variant_null("level"));
}

fn target(writer: &mut Cbor, endpoint: ConnectionEndpoint) {
    let ConnectionEndpoint::NodeInput(key) = endpoint else {
        panic!("validated restricted connection target must be an input port");
    };
    writer.variant_start("in_port");
    writer.record_start(2);
    writer.field("key", |writer| writer.key(level_in_port(key)));
    writer.field("signal_kind", |writer| writer.variant_null("level"));
}

fn logic_level(writer: &mut Cbor, value: LogicLevel) {
    writer.variant_null(match value {
        LogicLevel::Low => "low",
        LogicLevel::High => "high",
    });
}

fn signal_kind(writer: &mut Cbor, kind: SignalKind) {
    match kind {
        SignalKind::Level => writer.variant_null("level"),
        SignalKind::Pulse => panic!("validated restricted network must contain only level signals"),
    }
}

fn signal_kind_tag(kind: SignalKind) -> u8 {
    match kind {
        SignalKind::Level => 0,
        SignalKind::Pulse => 1,
    }
}

fn level_in_port(key: AnyInPortKey) -> u128 {
    match key {
        AnyInPortKey::Level(key) => key.as_u128(),
        AnyInPortKey::Pulse(_) => {
            panic!("validated restricted network must contain only level signals")
        }
    }
}

fn level_out_port(key: AnyOutPortKey) -> u128 {
    match key {
        AnyOutPortKey::Level(key) => key.as_u128(),
        AnyOutPortKey::Pulse(_) => {
            panic!("validated restricted network must contain only level signals")
        }
    }
}

fn external_input_order(key: AnyExternalInputKey) -> (u8, u128) {
    (signal_kind_tag(key.kind()), level_external_input(key))
}

fn external_output_order(key: AnyExternalOutputKey) -> (u8, u128) {
    (signal_kind_tag(key.kind()), level_external_output(key))
}

fn level_external_input(key: AnyExternalInputKey) -> u128 {
    match key {
        AnyExternalInputKey::Level(key) => key.as_u128(),
        AnyExternalInputKey::Pulse(_) => {
            panic!("validated restricted network must contain only level signals")
        }
    }
}

fn level_external_output(key: AnyExternalOutputKey) -> u128 {
    match key {
        AnyExternalOutputKey::Level(key) => key.as_u128(),
        AnyExternalOutputKey::Pulse(_) => {
            panic!("validated restricted network must contain only level signals")
        }
    }
}

#[derive(Default)]
pub(crate) struct Cbor(Vec<u8>);

impl Cbor {
    pub(crate) fn finish(self) -> Vec<u8> {
        self.0
    }

    pub(crate) fn uint(&mut self, value: u64) {
        self.major(0, value);
    }

    fn bytes(&mut self, value: &[u8]) {
        self.major(2, value.len() as u64);
        self.0.extend_from_slice(value);
    }

    pub(crate) fn text(&mut self, value: &str) {
        self.major(3, value.len() as u64);
        self.0.extend_from_slice(value.as_bytes());
    }

    fn array_start(&mut self, length: usize) {
        self.major(4, length as u64);
    }

    pub(crate) fn record_start(&mut self, fields: usize) {
        self.array_start(fields);
    }

    pub(crate) fn field(&mut self, name: &str, value: impl FnOnce(&mut Self)) {
        self.array_start(2);
        self.text(name);
        value(self);
    }

    fn variant_start(&mut self, name: &str) {
        self.array_start(2);
        self.text(name);
    }

    fn variant_null(&mut self, name: &str) {
        self.variant_start(name);
        self.null();
    }

    fn key(&mut self, value: u128) {
        self.bytes(&value.to_be_bytes());
    }

    fn null(&mut self) {
        self.0.push(0xf6);
    }

    fn major(&mut self, major: u8, value: u64) {
        let initial = major << 5;
        if value <= 23 {
            self.0.push(initial | value as u8);
        } else if u8::try_from(value).is_ok() {
            self.0.extend([initial | 24, value as u8]);
        } else if u16::try_from(value).is_ok() {
            self.0.push(initial | 25);
            self.0.extend_from_slice(&(value as u16).to_be_bytes());
        } else if u32::try_from(value).is_ok() {
            self.0.push(initial | 26);
            self.0.extend_from_slice(&(value as u32).to_be_bytes());
        } else {
            self.0.push(initial | 27);
            self.0.extend_from_slice(&value.to_be_bytes());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::authored::{ConnectionDef, ExternalInputDef, ExternalOutputDef, NodeDef, NodePorts};
    use crate::key::{
        ConnectionKey, ExternalInputKey, ExternalOutputKey, InPortKey, NetworkKey, NodeKey,
        OutPortKey,
    };
    use crate::metadata::DiagnosticMeta;
    use crate::signal::Level;

    fn meta(name: &str) -> DiagnosticMeta {
        DiagnosticMeta {
            name: Some(name.into()),
            description: Some(format!("{name} description")),
            ..DiagnosticMeta::default()
        }
    }

    fn minimal(time_domain_id: TimeDomainId) -> UncheckedNetwork<()> {
        UncheckedNetwork::new(
            NetworkKey::from_u128(1),
            time_domain_id,
            DiagnosticMeta::default(),
            vec![],
            vec![],
            vec![],
            vec![],
        )
    }

    fn nontrivial(
        time_domain_id: TimeDomainId,
        network_key: u128,
        constant: LogicLevel,
        connection_uses_external_input: bool,
        extra_input: bool,
        reverse: bool,
        with_metadata: bool,
    ) -> UncheckedNetwork<()> {
        let constant_output = OutPortKey::<Level>::from_u128(11);
        let first_input = InPortKey::<Level>::from_u128(21);
        let first_output = OutPortKey::<Level>::from_u128(22);
        let second_input = InPortKey::<Level>::from_u128(31);
        let second_output = OutPortKey::<Level>::from_u128(32);
        let external_one = ExternalInputKey::<Level>::from_u128(40);
        let external_two = ExternalInputKey::<Level>::from_u128(41);
        let mut nodes = vec![
            NodeDef::new(
                NodeKey::from_u128(10),
                NodeKind::constant(constant),
                NodePorts::new(vec![], vec![constant_output.into()]),
                if with_metadata {
                    meta("constant")
                } else {
                    DiagnosticMeta::default()
                },
            ),
            NodeDef::new(
                NodeKey::from_u128(20),
                NodeKind::not(),
                NodePorts::new(vec![first_input.into()], vec![first_output.into()]),
                if with_metadata {
                    meta("first inverter")
                } else {
                    DiagnosticMeta::default()
                },
            ),
            NodeDef::new(
                NodeKey::from_u128(30),
                NodeKind::not(),
                NodePorts::new(vec![second_input.into()], vec![second_output.into()]),
                if with_metadata {
                    meta("second inverter")
                } else {
                    DiagnosticMeta::default()
                },
            ),
        ];
        let mut inputs = vec![
            ExternalInputDef::new(
                external_one.into(),
                if with_metadata {
                    meta("first input")
                } else {
                    DiagnosticMeta::default()
                },
            ),
            ExternalInputDef::new(
                external_two.into(),
                if with_metadata {
                    meta("second input")
                } else {
                    DiagnosticMeta::default()
                },
            ),
        ];
        if extra_input {
            inputs.push(ExternalInputDef::new(
                ExternalInputKey::<Level>::from_u128(42).into(),
                if with_metadata {
                    meta("third input")
                } else {
                    DiagnosticMeta::default()
                },
            ));
        }
        let mut outputs = vec![
            ExternalOutputDef::new(
                ExternalOutputKey::<Level>::from_u128(50).into(),
                SignalSourceKey::NodeOutput(first_output).into(),
                if with_metadata {
                    meta("first output")
                } else {
                    DiagnosticMeta::default()
                },
            ),
            ExternalOutputDef::new(
                ExternalOutputKey::<Level>::from_u128(51).into(),
                SignalSourceKey::NodeOutput(second_output).into(),
                if with_metadata {
                    meta("second output")
                } else {
                    DiagnosticMeta::default()
                },
            ),
        ];
        let mut connections = vec![
            ConnectionDef::new(
                ConnectionKey::from_u128(60),
                if connection_uses_external_input {
                    external_one.into()
                } else {
                    constant_output.into()
                },
                first_input.into(),
                if with_metadata {
                    meta("first connection")
                } else {
                    DiagnosticMeta::default()
                },
            ),
            ConnectionDef::new(
                ConnectionKey::from_u128(61),
                external_two.into(),
                second_input.into(),
                if with_metadata {
                    meta("second connection")
                } else {
                    DiagnosticMeta::default()
                },
            ),
        ];
        if reverse {
            nodes.reverse();
            inputs.reverse();
            outputs.reverse();
            connections.reverse();
        }
        UncheckedNetwork::new(
            NetworkKey::from_u128(network_key),
            time_domain_id,
            if with_metadata {
                meta("network")
            } else {
                DiagnosticMeta::default()
            },
            nodes,
            inputs,
            outputs,
            connections,
        )
    }

    fn golden_nontrivial(time_domain_id: TimeDomainId) -> UncheckedNetwork<()> {
        let constant_output = OutPortKey::<Level>::from_u128(11);
        let not_input = InPortKey::<Level>::from_u128(21);
        let not_output = OutPortKey::<Level>::from_u128(22);
        UncheckedNetwork::new(
            NetworkKey::from_u128(1),
            time_domain_id,
            DiagnosticMeta::default(),
            vec![
                NodeDef::new(
                    NodeKey::from_u128(10),
                    NodeKind::constant(LogicLevel::High),
                    NodePorts::new(vec![], vec![constant_output.into()]),
                    DiagnosticMeta::default(),
                ),
                NodeDef::new(
                    NodeKey::from_u128(20),
                    NodeKind::not(),
                    NodePorts::new(vec![not_input.into()], vec![not_output.into()]),
                    DiagnosticMeta::default(),
                ),
            ],
            vec![],
            vec![ExternalOutputDef::new(
                ExternalOutputKey::<Level>::from_u128(30).into(),
                SignalSourceKey::NodeOutput(not_output).into(),
                DiagnosticMeta::default(),
            )],
            vec![ConnectionDef::new(
                ConnectionKey::from_u128(40),
                constant_output.into(),
                not_input.into(),
                DiagnosticMeta::default(),
            )],
        )
    }

    fn validated_fingerprints(
        network: UncheckedNetwork<()>,
    ) -> (NetworkFingerprint, InputSchemaFingerprint) {
        let report = network.validate();
        let artifact = report
            .artifact()
            .expect("fixture must be a valid restricted network");
        (artifact.fingerprint(), artifact.input_schema_fingerprint())
    }

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|byte| format!("{byte:02x}")).collect()
    }

    #[test]
    fn time_domain_and_fingerprints_are_stable_value_types() {
        let domain = TimeDomainId::from_u128(0x0102);
        assert_eq!(domain.to_be_bytes()[14..], [1, 2]);
        assert_eq!(domain.to_string(), "00000000000000000000000000000102");
        let network = minimal(domain).validate();
        let artifact = network.artifact().expect("minimal network validates");
        assert_eq!(artifact.time_domain_id(), domain);
        assert_eq!(artifact.fingerprint().to_string().len(), 64);
        assert_eq!(artifact.input_schema_fingerprint().to_string().len(), 64);
    }

    #[test]
    fn canonical_bytes_and_digests_have_versioned_golden_vectors() {
        let minimal = minimal(TimeDomainId::from_u128(2));
        let (minimal_network, minimal_input) = canonical_inputs(&minimal);
        assert_eq!(
            hex(&minimal_network),
            "838266646f6d61696e78206d6f737369676e616c2f6e6574776f726b5f66696e6765727072696e742f763182677061796c6f61648982781f6275696c745f696e5f6e6f64655f73656d616e746963735f76657273696f6e01826b636f6e6e656374696f6e73808276636f72655f73656d616e746963735f76657273696f6e01826f65787465726e616c5f696e7075747380827065787465726e616c5f6f75747075747380826b6e6574776f726b5f6b6579500000000000000000000000000000000182656e6f646573808265706f72747380826e74696d655f646f6d61696e5f69645000000000000000000000000000000002826776657273696f6e01",
            "replace with the hand-reviewed minimal network digest input"
        );
        assert_eq!(
            hex(&minimal_input),
            "838266646f6d61696e78256d6f737369676e616c2f696e7075745f736368656d615f66696e6765727072696e742f763182677061796c6f6164818266696e7075747380826776657273696f6e01",
            "replace with the hand-reviewed minimal input-schema digest input"
        );
        let minimal = minimal.validate();
        let minimal = minimal.artifact().expect("minimal network validates");
        assert_eq!(
            minimal.fingerprint().to_string(),
            "30707f69a3e090b4ae3d0fabdb52748a80bef09e2a57e29ce51d70794dc23cfd"
        );
        assert_eq!(
            minimal.input_schema_fingerprint().to_string(),
            "ceba01d43015569fca3e6da40896081f805654d75c08449dbdebc47b9528b797"
        );
        let nontrivial = golden_nontrivial(TimeDomainId::from_u128(2));
        let (nontrivial_network, nontrivial_input) = canonical_inputs(&nontrivial);
        assert_eq!(
            hex(&nontrivial_network),
            "838266646f6d61696e78206d6f737369676e616c2f6e6574776f726b5f66696e6765727072696e742f763182677061796c6f61648982781f6275696c745f696e5f6e6f64655f73656d616e746963735f76657273696f6e01826b636f6e6e656374696f6e73818382636b657950000000000000000000000000000000288266736f7572636582686f75745f706f72748282636b6579500000000000000000000000000000000b826b7369676e616c5f6b696e6482656c6576656cf682667461726765748267696e5f706f72748282636b65795000000000000000000000000000000015826b7369676e616c5f6b696e6482656c6576656cf68276636f72655f73656d616e746963735f76657273696f6e01826f65787465726e616c5f696e7075747380827065787465726e616c5f6f757470757473818382636b6579500000000000000000000000000000001e826b7369676e616c5f6b696e6482656c6576656cf68266736f7572636582686f75745f706f72748282636b65795000000000000000000000000000000016826b7369676e616c5f6b696e6482656c6576656cf6826b6e6574776f726b5f6b6579500000000000000000000000000000000182656e6f646573828282636b6579500000000000000000000000000000000a82646b696e648268636f6e7374616e7481826576616c7565826468696768f68282636b6579500000000000000000000000000000001482646b696e6482636e6f74f68265706f72747383858269646972656374696f6e8265696e707574f682636b6579500000000000000000000000000000001582656f776e65725000000000000000000000000000000014826d73656d616e7469635f726f6c658265696e707574f6826b7369676e616c5f6b696e6482656c6576656cf6858269646972656374696f6e82666f7574707574f682636b6579500000000000000000000000000000000b82656f776e6572500000000000000000000000000000000a826d73656d616e7469635f726f6c6582666f7574707574f6826b7369676e616c5f6b696e6482656c6576656cf6858269646972656374696f6e82666f7574707574f682636b6579500000000000000000000000000000001682656f776e65725000000000000000000000000000000014826d73656d616e7469635f726f6c6582666f7574707574f6826b7369676e616c5f6b696e6482656c6576656cf6826e74696d655f646f6d61696e5f69645000000000000000000000000000000002826776657273696f6e01",
            "replace with the hand-reviewed nontrivial network digest input"
        );
        assert_eq!(
            hex(&nontrivial_input),
            "838266646f6d61696e78256d6f737369676e616c2f696e7075745f736368656d615f66696e6765727072696e742f763182677061796c6f6164818266696e7075747380826776657273696f6e01",
            "replace with the hand-reviewed nontrivial input-schema digest input"
        );
        let validated = nontrivial.validate();
        let artifact = validated.artifact().expect("fixture validates");
        assert_eq!(
            artifact.fingerprint().to_string(),
            "5f99000eaf6776ae3e403d13ad8553dd26eec0bab5364d72138f0726e3f2a3e4"
        );
        assert_eq!(
            artifact.input_schema_fingerprint().to_string(),
            "ceba01d43015569fca3e6da40896081f805654d75c08449dbdebc47b9528b797"
        );
    }

    #[test]
    fn semantic_identity_is_permutation_invariant_and_metadata_free() {
        let domain = TimeDomainId::from_u128(9);
        let base = nontrivial(domain, 1, LogicLevel::High, false, false, false, false);
        let permuted = nontrivial(domain, 1, LogicLevel::High, false, false, true, false);
        let metadata = nontrivial(domain, 1, LogicLevel::High, false, false, false, true);
        let base = validated_fingerprints(base);
        let permuted = validated_fingerprints(permuted);
        let metadata = validated_fingerprints(metadata);
        assert_eq!(base, permuted);
        assert_eq!(base, metadata);
    }

    #[test]
    fn semantic_fingerprint_tracks_network_semantics_but_not_schema_independent_topology() {
        let domain = TimeDomainId::from_u128(9);
        let base = validated_fingerprints(nontrivial(
            domain,
            1,
            LogicLevel::High,
            false,
            false,
            false,
            false,
        ));
        let changed_time = validated_fingerprints(nontrivial(
            TimeDomainId::from_u128(10),
            1,
            LogicLevel::High,
            false,
            false,
            false,
            false,
        ));
        let changed_key = validated_fingerprints(nontrivial(
            domain,
            2,
            LogicLevel::High,
            false,
            false,
            false,
            false,
        ));
        let changed_constant = validated_fingerprints(nontrivial(
            domain,
            1,
            LogicLevel::Low,
            false,
            false,
            false,
            false,
        ));
        let changed_connection = validated_fingerprints(nontrivial(
            domain,
            1,
            LogicLevel::High,
            true,
            false,
            false,
            false,
        ));
        let changed_schema = validated_fingerprints(nontrivial(
            domain,
            1,
            LogicLevel::High,
            false,
            true,
            false,
            false,
        ));
        assert_ne!(base.0, changed_time.0);
        assert_ne!(base.0, changed_key.0);
        assert_ne!(base.0, changed_constant.0);
        assert_ne!(base.0, changed_connection.0);
        assert_eq!(base.1, changed_connection.1);
        assert_ne!(base.1, changed_schema.1);
    }

    #[test]
    fn equal_payloads_remain_separated_by_digest_domain() {
        let payload = [0x80];
        let mut network = Cbor::default();
        network.record_start(3);
        network.field("domain", |writer| writer.text(NETWORK_DOMAIN));
        network.field("payload", |writer| writer.0.extend(payload));
        network.field("version", |writer| writer.uint(1));
        let mut input = Cbor::default();
        input.record_start(3);
        input.field("domain", |writer| writer.text(INPUT_SCHEMA_DOMAIN));
        input.field("payload", |writer| writer.0.extend(payload));
        input.field("version", |writer| writer.uint(1));
        assert_ne!(
            blake3::hash(&network.finish()),
            blake3::hash(&input.finish())
        );
    }

    #[test]
    fn unsupported_pulse_claims_cannot_publish_restricted_fingerprints() {
        let network = UncheckedNetwork::<()>::new(
            NetworkKey::from_u128(1),
            TimeDomainId::from_u128(2),
            DiagnosticMeta::default(),
            vec![],
            vec![ExternalInputDef::new(
                crate::key::ExternalInputKey::<crate::signal::Pulse>::from_u128(3).into(),
                DiagnosticMeta::default(),
            )],
            vec![],
            vec![],
        );
        assert!(network.validate().artifact().is_none());
    }
}
