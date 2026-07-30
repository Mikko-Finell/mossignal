use mossignal::authored::{
    ConnectionDef, ExternalInputDef, ExternalOutputDef, NodeDef, NodeKind, NodePorts,
    UncheckedNetwork,
};
use mossignal::diagnostics::DiagnosticCode;
use mossignal::key::{
    ConnectionKey, ExternalInputKey, ExternalOutputKey, InPortKey, ModuleInputKey,
    ModuleInstanceKey, ModuleOutputKey, NetworkKey, NodeKey, OutPortKey, SignalSourceKey,
};
use mossignal::metadata::DiagnosticMeta;
use mossignal::signal::{Level, Pulse, PulseCount};
use mossignal::time::Time;
use mossignal::{
    CauseInspection, ModuleBuilder, NetworkBuilder, NodeSubject, OutputEvent, ProvenanceSubject,
    RuntimeFailureEvidence, RuntimePolicy, RuntimePolicyLimit, TimeDomainId, Transaction,
};

#[derive(Debug, PartialEq, Eq)]
enum TestDomain {}

fn policy() -> RuntimePolicy {
    RuntimePolicy::builder()
        .max_internal_reactions(10_000)
        .max_evaluated_operations(10_000)
        .max_pending_events(10_000)
        .max_events_created_per_transaction(10_000)
        .max_required_provenance_growth(10_000)
        .build()
        .unwrap_or_else(|failure| panic!("complete policy must build: {failure}"))
}

#[test]
fn coalesce_bounds_complete_batches_and_retains_the_input_cause() {
    let mut builder = NetworkBuilder::<TestDomain>::with_key(
        NetworkKey::from_u128(1),
        TimeDomainId::from_u128(2),
    );
    let input_key = ExternalInputKey::<Pulse>::from_u128(3);
    let input = builder
        .add_pulse_input(input_key, DiagnosticMeta::default())
        .unwrap_or_else(|failure| panic!("input must author: {failure:?}"));
    let node = NodeKey::from_u128(4);
    let input_port = InPortKey::<Pulse>::from_u128(5);
    let bounded = builder
        .add_coalesce_with_ports(
            node,
            input_port,
            OutPortKey::from_u128(6),
            input,
            DiagnosticMeta::default(),
        )
        .unwrap_or_else(|failure| panic!("coalesce must author: {failure:?}"))
        .into_outputs();
    let output = ExternalOutputKey::<Pulse>::from_u128(7);
    builder
        .add_pulse_output(output, bounded, DiagnosticMeta::default())
        .unwrap_or_else(|failure| panic!("output must author: {failure:?}"));
    let compiled = builder
        .finish()
        .require_artifact()
        .unwrap_or_else(|failure| panic!("coalesce must validate: {failure:?}"))
        .compile()
        .require_artifact()
        .unwrap_or_else(|failure| panic!("coalesce must compile: {failure:?}"));

    let snapshot = compiled
        .input_snapshot()
        .pulse(input_key, PulseCount::new(u64::MAX))
        .and_then(|builder| builder.finish())
        .unwrap_or_else(|failure| panic!("snapshot must build: {failure}"));
    let mut machine = compiled.spawn(policy());
    let initialized = machine
        .apply(Transaction::initialize(
            Time::from_ticks(0),
            machine.revision(),
            snapshot,
        ))
        .unwrap_or_else(|failure| panic!("coalesce must initialize: {failure}"));
    let [OutputEvent::Pulsed { count, cause, .. }] = initialized.output_events() else {
        panic!("positive coalesce input must emit one event");
    };
    assert_eq!(*count, PulseCount::ONE);
    let CauseInspection::Derived { supporters, .. } = initialized
        .provenance()
        .inspect(*cause)
        .unwrap_or_else(|failure| panic!("output cause must resolve: {failure}"))
    else {
        panic!("external output cause must be derived");
    };
    let node_cause = supporters
        .iter()
        .find_map(|supporter| {
            matches!(
                initialized.provenance().inspect(*supporter),
                Ok(CauseInspection::PulseDerived {
                    subject: ProvenanceSubject::Node(subject),
                    ..
                }) if subject == node
            )
            .then_some(*supporter)
        })
        .unwrap_or_else(|| panic!("coalesce node cause must support the output"));
    let CauseInspection::PulseDerived {
        contributions,
        result,
        ..
    } = initialized
        .provenance()
        .inspect(node_cause)
        .unwrap_or_else(|failure| panic!("coalesce cause must resolve: {failure}"))
    else {
        panic!("coalesce must retain grouped pulse provenance");
    };
    assert_eq!(result, PulseCount::ONE);
    assert_eq!(contributions.len(), 1);
    assert_eq!(contributions[0].count(), PulseCount::new(u64::MAX));

    for (ticks, input_count) in [(1, 1), (2, 2), (3, 17)] {
        let delta = compiled
            .input_delta()
            .pulse(input_key, PulseCount::new(input_count))
            .and_then(|builder| builder.finish())
            .unwrap_or_else(|failure| panic!("positive delta must build: {failure}"));
        assert!(matches!(
            machine
                .apply(Transaction::advance(
                    Time::from_ticks(ticks),
                    machine.revision(),
                    delta,
                ))
                .unwrap_or_else(|failure| panic!("positive reaction must apply: {failure}"))
                .output_events(),
            [OutputEvent::Pulsed { count, .. }] if *count == PulseCount::ONE
        ));
    }

    let omitted = compiled
        .input_delta()
        .finish()
        .unwrap_or_else(|failure| panic!("empty delta must build: {failure}"));
    let advanced = machine
        .apply(Transaction::advance(
            Time::from_ticks(4),
            machine.revision(),
            omitted,
        ))
        .unwrap_or_else(|failure| panic!("empty reaction must apply: {failure}"));
    assert!(advanced.output_events().is_empty());
}

#[test]
fn zip_uses_every_port_minimum_warns_for_duplicates_and_never_buffers() {
    let mut builder = NetworkBuilder::<TestDomain>::new(TimeDomainId::from_u128(10));
    let (left_key, left) = builder.pulse_input("left");
    let (right_key, right) = builder.pulse_input("right");
    let ports = [
        InPortKey::<Pulse>::from_u128(20),
        InPortKey::<Pulse>::from_u128(21),
        InPortKey::<Pulse>::from_u128(22),
    ];
    let zipped = builder
        .add_zip_with_ports(
            NodeKey::from_u128(23),
            OutPortKey::from_u128(24),
            [(ports[2], left), (ports[0], right), (ports[1], left)],
            DiagnosticMeta::default(),
        )
        .unwrap_or_else(|failure| panic!("zip must author: {failure:?}"))
        .into_outputs();
    let output = builder
        .pulse_output("out", zipped)
        .unwrap_or_else(|failure| panic!("output must author: {failure:?}"));
    let report = builder.finish();
    assert!(report.diagnostics().iter().any(|diagnostic| {
        diagnostic.problem().code() == DiagnosticCode::ValidationDuplicateSource
    }));
    let compiled = report
        .require_artifact()
        .unwrap_or_else(|failure| panic!("zip must validate: {failure:?}"))
        .compile()
        .require_artifact()
        .unwrap_or_else(|failure| panic!("zip must compile: {failure:?}"));
    let snapshot = compiled
        .input_snapshot()
        .pulse(left_key, PulseCount::new(5))
        .and_then(|builder| builder.pulse(right_key, PulseCount::new(2)))
        .and_then(|builder| builder.finish())
        .unwrap_or_else(|failure| panic!("snapshot must build: {failure}"));
    let mut machine = compiled.spawn(policy());
    let result = machine
        .apply(Transaction::initialize(
            Time::from_ticks(0),
            machine.revision(),
            snapshot,
        ))
        .unwrap_or_else(|failure| panic!("zip must initialize: {failure}"));
    assert!(matches!(
        result.output_events(),
        [OutputEvent::Pulsed {
            output: actual,
            count,
            ..
        }] if *actual == output && *count == PulseCount::new(2)
    ));
    let OutputEvent::Pulsed { cause, .. } = &result.output_events()[0] else {
        panic!("Zip must emit a pulse event");
    };
    let CauseInspection::Derived { supporters, .. } = result
        .provenance()
        .inspect(*cause)
        .unwrap_or_else(|failure| panic!("Zip output cause must resolve: {failure}"))
    else {
        panic!("Zip output must retain a derived cause");
    };
    let (_, contributions, grouped_result) = supporters
        .iter()
        .find_map(|supporter| match result.provenance().inspect(*supporter) {
            Ok(CauseInspection::PulseDerived {
                subject: ProvenanceSubject::Node(subject),
                contributions,
                result,
                ..
            }) => Some((subject, contributions, result)),
            _ => None,
        })
        .unwrap_or_else(|| panic!("Zip node cause must support the output"));
    assert_eq!(grouped_result, PulseCount::new(2));
    assert_eq!(
        contributions
            .iter()
            .map(|contribution| (contribution.port().clone(), contribution.count()))
            .collect::<Vec<_>>(),
        vec![
            (
                mossignal::PulsePortSubject::Port(ports[0]),
                PulseCount::new(2)
            ),
            (
                mossignal::PulsePortSubject::Port(ports[1]),
                PulseCount::new(5)
            ),
            (
                mossignal::PulsePortSubject::Port(ports[2]),
                PulseCount::new(5)
            ),
        ]
    );

    let left_only = compiled
        .input_delta()
        .pulse(left_key, PulseCount::new(4))
        .and_then(|builder| builder.finish())
        .unwrap_or_else(|failure| panic!("left-only delta must build: {failure}"));
    assert!(
        machine
            .apply(Transaction::advance(
                Time::from_ticks(1),
                machine.revision(),
                left_only,
            ))
            .unwrap_or_else(|failure| panic!("left-only reaction must apply: {failure}"))
            .output_events()
            .is_empty()
    );
    let right_only = compiled
        .input_delta()
        .pulse(right_key, PulseCount::new(3))
        .and_then(|builder| builder.finish())
        .unwrap_or_else(|failure| panic!("right-only delta must build: {failure}"));
    assert!(
        machine
            .apply(Transaction::advance(
                Time::from_ticks(2),
                machine.revision(),
                right_only,
            ))
            .unwrap_or_else(|failure| panic!("right-only reaction must apply: {failure}"))
            .output_events()
            .is_empty()
    );

    let maximum = compiled
        .input_delta()
        .pulse(left_key, PulseCount::new(u64::MAX))
        .and_then(|builder| builder.pulse(right_key, PulseCount::new(u64::MAX)))
        .and_then(|builder| builder.finish())
        .unwrap_or_else(|failure| panic!("maximum delta must build: {failure}"));
    assert!(matches!(
        machine
            .apply(Transaction::advance(
                Time::from_ticks(3),
                machine.revision(),
                maximum,
            ))
            .unwrap_or_else(|failure| panic!("maximum reaction must apply: {failure}"))
            .output_events(),
        [OutputEvent::Pulsed { count, .. }] if *count == PulseCount::new(u64::MAX)
    ));
}

#[test]
fn zero_zip_is_blocking_and_unary_zip_is_a_valid_advisory() {
    let mut empty = NetworkBuilder::<TestDomain>::new(TimeDomainId::from_u128(30));
    let zipped = empty
        .zip([])
        .unwrap_or_else(|failure| panic!("empty zip must remain authorable: {failure:?}"));
    empty
        .pulse_output("out", zipped)
        .unwrap_or_else(|failure| panic!("empty zip output must author: {failure:?}"));
    let report = empty.finish();
    assert!(report.artifact().is_none());
    assert!(report.diagnostics().iter().any(|diagnostic| {
        diagnostic.problem().code() == DiagnosticCode::ValidationInvalidVariadicArity
    }));
    assert!(!report.diagnostics().iter().any(|diagnostic| {
        diagnostic.problem().code() == DiagnosticCode::ValidationEmptyVariadicNode
    }));

    let mut unary = NetworkBuilder::<TestDomain>::new(TimeDomainId::from_u128(31));
    let (input_key, input) = unary.pulse_input("in");
    let zipped = unary
        .zip([input])
        .unwrap_or_else(|failure| panic!("unary zip must author: {failure:?}"));
    let output = unary
        .pulse_output("out", zipped)
        .unwrap_or_else(|failure| panic!("unary zip output must author: {failure:?}"));
    let report = unary.finish();
    assert!(report.diagnostics().iter().any(|diagnostic| {
        diagnostic.problem().code() == DiagnosticCode::ValidationUnaryDegenerateNode
    }));
    let compiled = report
        .require_artifact()
        .unwrap_or_else(|failure| panic!("unary zip must validate: {failure:?}"))
        .compile()
        .require_artifact()
        .unwrap_or_else(|failure| panic!("unary zip must compile: {failure:?}"));
    let snapshot = compiled
        .input_snapshot()
        .pulse(input_key, PulseCount::new(u64::MAX))
        .and_then(|builder| builder.finish())
        .unwrap_or_else(|failure| panic!("snapshot must build: {failure}"));
    let mut machine = compiled.spawn(policy());
    let result = machine
        .apply(Transaction::initialize(
            Time::from_ticks(0),
            machine.revision(),
            snapshot,
        ))
        .unwrap_or_else(|failure| panic!("unary zip must initialize: {failure}"));
    assert!(matches!(
        result.output_events(),
        [OutputEvent::Pulsed {
            output: actual,
            count,
            ..
        }] if *actual == output && *count == PulseCount::new(u64::MAX)
    ));
}

fn dynamic_zip() -> UncheckedNetwork<TestDomain> {
    let first = ExternalInputKey::<Pulse>::from_u128(1);
    let second = ExternalInputKey::<Pulse>::from_u128(2);
    let first_port = InPortKey::<Pulse>::from_u128(3);
    let second_port = InPortKey::<Pulse>::from_u128(4);
    let node_output = OutPortKey::<Pulse>::from_u128(5);
    UncheckedNetwork::new(
        NetworkKey::from_u128(40),
        TimeDomainId::from_u128(41),
        DiagnosticMeta::default(),
        vec![NodeDef::new(
            NodeKey::from_u128(6),
            NodeKind::zip(),
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
            ExternalOutputKey::<Pulse>::from_u128(7).into(),
            SignalSourceKey::NodeOutput(node_output).into(),
            DiagnosticMeta::default(),
        )],
        vec![
            ConnectionDef::new(
                ConnectionKey::from_u128(0),
                first.into(),
                first_port.into(),
                DiagnosticMeta::default(),
            ),
            ConnectionDef::new(
                ConnectionKey::from_u128(1),
                second.into(),
                second_port.into(),
                DiagnosticMeta::default(),
            ),
        ],
    )
}

#[test]
fn typed_dynamic_and_module_paths_share_structure_and_identity_rules() {
    let mut typed = NetworkBuilder::<TestDomain>::with_key(
        NetworkKey::from_u128(40),
        TimeDomainId::from_u128(41),
    );
    let first = typed
        .add_pulse_input(ExternalInputKey::from_u128(1), DiagnosticMeta::default())
        .unwrap_or_else(|failure| panic!("first input must author: {failure:?}"));
    let second = typed
        .add_pulse_input(ExternalInputKey::from_u128(2), DiagnosticMeta::default())
        .unwrap_or_else(|failure| panic!("second input must author: {failure:?}"));
    let zipped = typed
        .add_zip_with_ports(
            NodeKey::from_u128(6),
            OutPortKey::from_u128(5),
            [
                (InPortKey::from_u128(3), first),
                (InPortKey::from_u128(4), second),
            ],
            DiagnosticMeta::default(),
        )
        .unwrap_or_else(|failure| panic!("typed zip must author: {failure:?}"))
        .into_outputs();
    typed
        .add_pulse_output(
            ExternalOutputKey::from_u128(7),
            zipped,
            DiagnosticMeta::default(),
        )
        .unwrap_or_else(|failure| panic!("typed output must author: {failure:?}"));
    let typed = typed
        .finish()
        .require_artifact()
        .unwrap_or_else(|failure| panic!("typed zip must validate: {failure:?}"));
    let dynamic = dynamic_zip()
        .validate()
        .require_artifact()
        .unwrap_or_else(|failure| panic!("dynamic zip must validate: {failure:?}"));
    assert_eq!(typed.fingerprint(), dynamic.fingerprint());

    fn module_fingerprint(zip: bool) -> mossignal::ModuleFingerprint {
        let mut module = ModuleBuilder::<TestDomain>::new();
        let (_, first) = module.pulse_input("first");
        let (_, second) = module.pulse_input("second");
        let combined = if zip {
            module
                .zip([first, second])
                .unwrap_or_else(|failure| panic!("module Zip must author: {failure:?}"))
        } else {
            module
                .merge([first, second])
                .unwrap_or_else(|failure| panic!("module Merge must author: {failure:?}"))
        };
        let result = module
            .coalesce(combined)
            .unwrap_or_else(|failure| panic!("module Coalesce must author: {failure:?}"));
        module
            .pulse_output("result", result)
            .unwrap_or_else(|failure| panic!("module output must author: {failure:?}"));
        module
            .finish()
            .require_artifact()
            .unwrap_or_else(|failure| panic!("module must validate: {failure:?}"))
            .fingerprint()
    }
    assert_ne!(module_fingerprint(true), module_fingerprint(false));
}

#[test]
fn malformed_coalesce_and_current_reaction_cycles_are_structured() {
    let pulse_output = OutPortKey::<Pulse>::from_u128(1);
    let malformed = UncheckedNetwork::<TestDomain>::new(
        NetworkKey::from_u128(50),
        TimeDomainId::from_u128(51),
        DiagnosticMeta::default(),
        vec![NodeDef::new(
            NodeKey::from_u128(2),
            NodeKind::coalesce(),
            NodePorts::new(Vec::new(), vec![pulse_output.into()]),
            DiagnosticMeta::default(),
        )],
        Vec::new(),
        Vec::new(),
        Vec::new(),
    )
    .validate();
    assert!(malformed.artifact().is_none());
    assert!(malformed.diagnostics().iter().any(|diagnostic| {
        diagnostic.problem().code() == DiagnosticCode::ValidationInvalidFixedArity
    }));
    assert!(malformed.diagnostics().iter().any(|diagnostic| {
        diagnostic.problem().code() == DiagnosticCode::ValidationMissingRequiredInput
    }));

    let malformed_zip = UncheckedNetwork::<TestDomain>::new(
        NetworkKey::from_u128(58),
        TimeDomainId::from_u128(59),
        DiagnosticMeta::default(),
        vec![NodeDef::new(
            NodeKey::from_u128(14),
            NodeKind::zip(),
            NodePorts::new(Vec::new(), Vec::new()),
            DiagnosticMeta::default(),
        )],
        Vec::new(),
        Vec::new(),
        Vec::new(),
    )
    .validate();
    assert!(malformed_zip.artifact().is_none());
    assert!(malformed_zip.diagnostics().iter().any(|diagnostic| {
        diagnostic.problem().code() == DiagnosticCode::ValidationInvalidFixedArity
    }));
    assert!(malformed_zip.diagnostics().iter().any(|diagnostic| {
        diagnostic.problem().code() == DiagnosticCode::ValidationInvalidVariadicArity
    }));

    let wrong_kind = UncheckedNetwork::<TestDomain>::new(
        NetworkKey::from_u128(52),
        TimeDomainId::from_u128(53),
        DiagnosticMeta::default(),
        vec![NodeDef::new(
            NodeKey::from_u128(3),
            NodeKind::coalesce(),
            NodePorts::new(
                vec![InPortKey::<Level>::from_u128(4).into()],
                vec![OutPortKey::<Level>::from_u128(5).into()],
            ),
            DiagnosticMeta::default(),
        )],
        Vec::new(),
        Vec::new(),
        Vec::new(),
    )
    .validate();
    assert!(wrong_kind.artifact().is_none());
    assert_eq!(
        wrong_kind
            .diagnostics()
            .iter()
            .filter(|diagnostic| {
                diagnostic.problem().code() == DiagnosticCode::ValidationInvalidFixedArity
            })
            .count(),
        2
    );

    let input = InPortKey::<Pulse>::from_u128(6);
    let output = OutPortKey::<Pulse>::from_u128(7);
    let cycle = UncheckedNetwork::<TestDomain>::new(
        NetworkKey::from_u128(54),
        TimeDomainId::from_u128(55),
        DiagnosticMeta::default(),
        vec![NodeDef::new(
            NodeKey::from_u128(8),
            NodeKind::zip(),
            NodePorts::new(vec![input.into()], vec![output.into()]),
            DiagnosticMeta::default(),
        )],
        Vec::new(),
        Vec::new(),
        vec![ConnectionDef::new(
            ConnectionKey::from_u128(9),
            output.into(),
            input.into(),
            DiagnosticMeta::default(),
        )],
    )
    .validate();
    assert!(cycle.artifact().is_none());
    assert!(cycle.diagnostics().iter().any(|diagnostic| {
        diagnostic.problem().code() == DiagnosticCode::ValidationCurrentReactionCycle
    }));

    let coalesce_input = InPortKey::<Pulse>::from_u128(10);
    let coalesce_output = OutPortKey::<Pulse>::from_u128(11);
    let coalesce_cycle = UncheckedNetwork::<TestDomain>::new(
        NetworkKey::from_u128(56),
        TimeDomainId::from_u128(57),
        DiagnosticMeta::default(),
        vec![NodeDef::new(
            NodeKey::from_u128(12),
            NodeKind::coalesce(),
            NodePorts::new(vec![coalesce_input.into()], vec![coalesce_output.into()]),
            DiagnosticMeta::default(),
        )],
        Vec::new(),
        Vec::new(),
        vec![ConnectionDef::new(
            ConnectionKey::from_u128(13),
            coalesce_output.into(),
            coalesce_input.into(),
            DiagnosticMeta::default(),
        )],
    )
    .validate();
    assert!(coalesce_cycle.artifact().is_none());
    assert!(coalesce_cycle.diagnostics().iter().any(|diagnostic| {
        diagnostic.problem().code() == DiagnosticCode::ValidationCurrentReactionCycle
    }));
}

#[test]
fn network_identity_distinguishes_merge_coalesce_and_zip() {
    fn fingerprint(kind: NodeKind<TestDomain>) -> mossignal::NetworkFingerprint {
        let input = ExternalInputKey::<Pulse>::from_u128(1);
        let port = InPortKey::<Pulse>::from_u128(2);
        let output = OutPortKey::<Pulse>::from_u128(3);
        UncheckedNetwork::new(
            NetworkKey::from_u128(60),
            TimeDomainId::from_u128(61),
            DiagnosticMeta::default(),
            vec![NodeDef::new(
                NodeKey::from_u128(4),
                kind,
                NodePorts::new(vec![port.into()], vec![output.into()]),
                DiagnosticMeta::default(),
            )],
            vec![ExternalInputDef::new(
                input.into(),
                DiagnosticMeta::default(),
            )],
            Vec::new(),
            vec![ConnectionDef::new(
                ConnectionKey::from_u128(5),
                input.into(),
                port.into(),
                DiagnosticMeta::default(),
            )],
        )
        .validate()
        .require_artifact()
        .unwrap_or_else(|failure| panic!("identity fixture must validate: {failure:?}"))
        .fingerprint()
    }

    let merge = fingerprint(NodeKind::merge());
    let coalesce = fingerprint(NodeKind::coalesce());
    let zip = fingerprint(NodeKind::zip());
    assert_ne!(merge, coalesce);
    assert_ne!(merge, zip);
    assert_ne!(coalesce, zip);
}

#[test]
fn upstream_overflow_and_operation_budget_reject_the_complete_graph_atomically() {
    let mut builder = NetworkBuilder::<TestDomain>::new(TimeDomainId::from_u128(70));
    let (left_key, left) = builder.pulse_input("left");
    let (right_key, right) = builder.pulse_input("right");
    let merge_node = NodeKey::from_u128(71);
    let merged = builder
        .add_merge(merge_node, [left, right], DiagnosticMeta::default())
        .unwrap_or_else(|failure| panic!("Merge must author: {failure:?}"))
        .into_outputs();
    let bounded = builder
        .coalesce(merged)
        .unwrap_or_else(|failure| panic!("Coalesce must author: {failure:?}"));
    let zipped = builder
        .zip([merged, left])
        .unwrap_or_else(|failure| panic!("Zip must author: {failure:?}"));
    builder
        .pulse_output("bounded", bounded)
        .unwrap_or_else(|failure| panic!("Coalesce output must author: {failure:?}"));
    builder
        .pulse_output("zipped", zipped)
        .unwrap_or_else(|failure| panic!("Zip output must author: {failure:?}"));
    let compiled = builder
        .finish()
        .require_artifact()
        .unwrap_or_else(|failure| panic!("combined graph must validate: {failure:?}"))
        .compile()
        .require_artifact()
        .unwrap_or_else(|failure| panic!("combined graph must compile: {failure:?}"));

    let initial = compiled
        .input_snapshot()
        .finish()
        .unwrap_or_else(|failure| panic!("zero snapshot must build: {failure}"));
    let mut machine = compiled.spawn(policy());
    machine
        .apply(Transaction::initialize(
            Time::from_ticks(0),
            machine.revision(),
            initial,
        ))
        .unwrap_or_else(|failure| panic!("zero reaction must initialize: {failure}"));
    let before_status = machine.status();
    let before_revision = machine.revision();
    let before_now = machine.now();
    let before_schedule = machine.schedule();
    let overflow = compiled
        .input_delta()
        .pulse(left_key, PulseCount::new(u64::MAX))
        .and_then(|builder| builder.pulse(right_key, PulseCount::ONE))
        .and_then(|builder| builder.finish())
        .unwrap_or_else(|failure| panic!("overflow delta must build: {failure}"));
    let failure = machine
        .apply(Transaction::advance(
            Time::from_ticks(1),
            machine.revision(),
            overflow,
        ))
        .expect_err("upstream Merge overflow must reject downstream Coalesce and Zip");
    assert_eq!(
        failure.evidence(),
        &RuntimeFailureEvidence::PulseCountOverflow {
            node: NodeSubject::Node(merge_node),
        }
    );
    assert_eq!(machine.status(), before_status);
    assert_eq!(machine.revision(), before_revision);
    assert_eq!(machine.now(), before_now);
    assert_eq!(machine.schedule(), before_schedule);

    let recovered = compiled
        .input_delta()
        .pulse(left_key, PulseCount::new(2))
        .and_then(|builder| builder.pulse(right_key, PulseCount::new(3)))
        .and_then(|builder| builder.finish())
        .unwrap_or_else(|failure| panic!("recovery delta must build: {failure}"));
    let mut recovered_counts = machine
        .apply(Transaction::advance(
            Time::from_ticks(1),
            machine.revision(),
            recovered,
        ))
        .unwrap_or_else(|failure| panic!("reconvergent graph must recover: {failure}"))
        .output_events()
        .iter()
        .map(|event| match event {
            OutputEvent::Pulsed { count, .. } => *count,
            OutputEvent::LevelChanged { .. } => panic!("pulse graph must not emit a level"),
            _ => panic!("pulse graph emitted an unsupported event kind"),
        })
        .collect::<Vec<_>>();
    recovered_counts.sort();
    assert_eq!(recovered_counts, vec![PulseCount::ONE, PulseCount::new(2)]);

    let zero_operation_policy = RuntimePolicy::builder()
        .max_internal_reactions(10)
        .max_evaluated_operations(0)
        .max_pending_events(10)
        .max_events_created_per_transaction(10)
        .max_required_provenance_growth(100)
        .build()
        .unwrap_or_else(|failure| panic!("zero-operation policy must build: {failure}"));
    let snapshot = compiled
        .input_snapshot()
        .finish()
        .unwrap_or_else(|failure| panic!("budget snapshot must build: {failure}"));
    let mut budgeted = compiled.spawn(zero_operation_policy);
    let before_status = budgeted.status();
    let before_revision = budgeted.revision();
    let before_now = budgeted.now();
    let failure = budgeted
        .apply(Transaction::initialize(
            Time::from_ticks(0),
            budgeted.revision(),
            snapshot,
        ))
        .expect_err("zero evaluated-operation budget must reject the graph");
    assert_eq!(
        failure.evidence(),
        &RuntimeFailureEvidence::BudgetExceeded {
            budget: RuntimePolicyLimit::MaxEvaluatedOperations,
            limit: 0,
            consumed: 10,
        }
    );
    assert_eq!(budgeted.status(), before_status);
    assert_eq!(budgeted.revision(), before_revision);
    assert_eq!(budgeted.now(), before_now);
}

#[test]
fn module_wrapped_zip_and_coalesce_execute_through_the_ordinary_machine_path() {
    let first_input = ModuleInputKey::<Pulse>::from_u128(1);
    let second_input = ModuleInputKey::<Pulse>::from_u128(2);
    let module_output = ModuleOutputKey::<Pulse>::from_u128(3);
    let mut module = ModuleBuilder::<TestDomain>::new();
    let first = module
        .add_pulse_input(first_input, DiagnosticMeta::default())
        .unwrap_or_else(|failure| panic!("first module input must author: {failure:?}"));
    let second = module
        .add_pulse_input(second_input, DiagnosticMeta::default())
        .unwrap_or_else(|failure| panic!("second module input must author: {failure:?}"));
    let zipped = module
        .zip([first, second])
        .unwrap_or_else(|failure| panic!("module Zip must author: {failure:?}"));
    let bounded = module
        .coalesce(zipped)
        .unwrap_or_else(|failure| panic!("module Coalesce must author: {failure:?}"));
    module
        .add_pulse_output(module_output, bounded, DiagnosticMeta::default())
        .unwrap_or_else(|failure| panic!("module output must author: {failure:?}"));
    let module = module
        .finish()
        .require_artifact()
        .unwrap_or_else(|failure| panic!("module must validate: {failure:?}"));

    let first_external = ExternalInputKey::<Pulse>::from_u128(10);
    let second_external = ExternalInputKey::<Pulse>::from_u128(11);
    let external_output = ExternalOutputKey::<Pulse>::from_u128(12);
    let instance = ModuleInstanceKey::from_u128(13);
    let mut network = NetworkBuilder::<TestDomain>::new(TimeDomainId::from_u128(14));
    let first = network
        .add_pulse_input(first_external, DiagnosticMeta::default())
        .unwrap_or_else(|failure| panic!("first network input must author: {failure:?}"));
    let second = network
        .add_pulse_input(second_external, DiagnosticMeta::default())
        .unwrap_or_else(|failure| panic!("second network input must author: {failure:?}"));
    let added = network
        .instantiate(&module, instance, DiagnosticMeta::default())
        .unwrap_or_else(|failure| panic!("instance must begin: {failure:?}"))
        .bind_pulse(first_input, first)
        .and_then(|builder| builder.bind_pulse(second_input, second))
        .and_then(|builder| builder.finish())
        .unwrap_or_else(|failure| panic!("instance must bind: {failure:?}"));
    network
        .add_pulse_output(
            external_output,
            added
                .pulse_output(module_output)
                .unwrap_or_else(|failure| panic!("module output must resolve: {failure:?}")),
            DiagnosticMeta::default(),
        )
        .unwrap_or_else(|failure| panic!("network output must author: {failure:?}"));
    let compiled = network
        .finish()
        .require_artifact()
        .unwrap_or_else(|failure| panic!("wrapped network must validate: {failure:?}"))
        .compile()
        .require_artifact()
        .unwrap_or_else(|failure| panic!("wrapped network must compile: {failure:?}"));
    let snapshot = compiled
        .input_snapshot()
        .pulse(first_external, PulseCount::new(5))
        .and_then(|builder| builder.pulse(second_external, PulseCount::new(2)))
        .and_then(|builder| builder.finish())
        .unwrap_or_else(|failure| panic!("wrapped snapshot must build: {failure}"));
    let mut machine = compiled.spawn(policy());
    let result = machine
        .apply(Transaction::initialize(
            Time::from_ticks(0),
            machine.revision(),
            snapshot,
        ))
        .unwrap_or_else(|failure| panic!("wrapped network must initialize: {failure}"));
    assert!(matches!(
        result.output_events(),
        [OutputEvent::Pulsed {
            output,
            count,
            ..
        }] if *output == external_output && *count == PulseCount::ONE
    ));
}
