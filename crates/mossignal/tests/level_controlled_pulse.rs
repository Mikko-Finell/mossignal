use std::collections::{BTreeMap, BTreeSet};

use mossignal::authored::{
    ConnectionDef, ExternalInputDef, ExternalOutputDef, InputPortRole, NodeDef, NodeKind,
    NodePorts, OutputPortRole, UncheckedNetwork,
};
use mossignal::diagnostics::{DiagnosticCode, ProblemEvidence, RequiredInputRef};
use mossignal::key::{
    ConnectionKey, ExternalInputKey, ExternalOutputKey, InPortKey, ModuleInputKey,
    ModuleInstanceKey, ModuleOutputKey, NetworkKey, NodeKey, OutPortKey, SignalSourceKey,
};
use mossignal::metadata::DiagnosticMeta;
use mossignal::signal::{Level, LogicLevel, Pulse, PulseCount, SignalKind};
use mossignal::time::Time;
use mossignal::{
    CauseInspection, CauseRef, CompiledNetwork, ModuleBuilder, NetworkBuilder, OutputEvent,
    ProvenanceView, RuntimeFailureEvidence, RuntimePolicy, RuntimePolicyLimit, TimeDomainId,
    Transaction,
};

#[derive(Debug, PartialEq, Eq)]
enum TestDomain {}

type LevelObservations = BTreeSet<(ExternalInputKey<Level>, LogicLevel)>;
type PulseObservations = BTreeSet<(ExternalInputKey<Pulse>, PulseCount)>;

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

struct FamilyFixture {
    compiled: CompiledNetwork<TestDomain>,
    selector: ExternalInputKey<Level>,
    gate_pulses: ExternalInputKey<Pulse>,
    low_pulses: ExternalInputKey<Pulse>,
    high_pulses: ExternalInputKey<Pulse>,
    gate_output: ExternalOutputKey<Pulse>,
    select_output: ExternalOutputKey<Pulse>,
    route_low: ExternalOutputKey<Pulse>,
    route_high: ExternalOutputKey<Pulse>,
}

fn family_fixture() -> FamilyFixture {
    let mut builder = NetworkBuilder::<TestDomain>::with_key(
        NetworkKey::from_u128(1),
        TimeDomainId::from_u128(2),
    );
    let (selector, selector_signal) = builder.level_input("selector");
    let (gate_pulses, gate_signal) = builder.pulse_input("gate-pulses");
    let (low_pulses, low_signal) = builder.pulse_input("low-pulses");
    let (high_pulses, high_signal) = builder.pulse_input("high-pulses");
    let gate = builder
        .pulse_gate(gate_signal, selector_signal)
        .unwrap_or_else(|failure| panic!("PulseGate must author: {failure:?}"));
    let selected = builder
        .pulse_select(selector_signal, low_signal, high_signal)
        .unwrap_or_else(|failure| panic!("PulseSelect must author: {failure:?}"));
    let routed = builder
        .pulse_route(selector_signal, gate_signal)
        .unwrap_or_else(|failure| panic!("PulseRoute must author: {failure:?}"));
    let gate_output = builder
        .pulse_output("gate", gate)
        .unwrap_or_else(|failure| panic!("gate output must author: {failure:?}"));
    let select_output = builder
        .pulse_output("select", selected)
        .unwrap_or_else(|failure| panic!("select output must author: {failure:?}"));
    let route_low = builder
        .pulse_output("route-low", routed.when_low)
        .unwrap_or_else(|failure| panic!("low route output must author: {failure:?}"));
    let route_high = builder
        .pulse_output("route-high", routed.when_high)
        .unwrap_or_else(|failure| panic!("high route output must author: {failure:?}"));
    let compiled = builder
        .finish()
        .require_artifact()
        .unwrap_or_else(|failure| panic!("family must validate: {failure:?}"))
        .compile()
        .require_artifact()
        .unwrap_or_else(|failure| panic!("family must compile: {failure:?}"));
    FamilyFixture {
        compiled,
        selector,
        gate_pulses,
        low_pulses,
        high_pulses,
        gate_output,
        select_output,
        route_low,
        route_high,
    }
}

fn observed_counts(
    events: &[OutputEvent<TestDomain>],
) -> BTreeMap<ExternalOutputKey<Pulse>, PulseCount> {
    events
        .iter()
        .filter_map(|event| match event {
            OutputEvent::Pulsed { output, count, .. } => Some((*output, *count)),
            _ => None,
        })
        .collect()
}

fn observed_output_order(events: &[OutputEvent<TestDomain>]) -> Vec<ExternalOutputKey<Pulse>> {
    events
        .iter()
        .filter_map(|event| match event {
            OutputEvent::Pulsed { output, .. } => Some(*output),
            _ => None,
        })
        .collect()
}

fn expected_counts(
    fixture: &FamilyFixture,
    control: LogicLevel,
    count: PulseCount,
) -> BTreeMap<ExternalOutputKey<Pulse>, PulseCount> {
    if count == PulseCount::ZERO {
        return BTreeMap::new();
    }
    let mut expected = BTreeMap::from([(fixture.select_output, count)]);
    if control.is_high() {
        expected.insert(fixture.gate_output, count);
        expected.insert(fixture.route_high, count);
    } else {
        expected.insert(fixture.route_low, count);
    }
    expected
}

#[test]
fn family_exhausts_controls_and_counts_across_initialization_and_ready_reactions() {
    let fixture = family_fixture();
    for control in [LogicLevel::Low, LogicLevel::High] {
        for count in [
            PulseCount::ZERO,
            PulseCount::ONE,
            PulseCount::new(2),
            PulseCount::new(17),
            PulseCount::new(u64::MAX),
        ] {
            let unselected = if count == PulseCount::new(u64::MAX) {
                PulseCount::new(17)
            } else {
                PulseCount::new(u64::MAX)
            };
            let (low, high) = if control.is_low() {
                (count, unselected)
            } else {
                (unselected, count)
            };
            let snapshot = fixture
                .compiled
                .input_snapshot()
                .set(fixture.selector, control)
                .and_then(|builder| builder.pulse(fixture.gate_pulses, count))
                .and_then(|builder| builder.pulse(fixture.low_pulses, low))
                .and_then(|builder| builder.pulse(fixture.high_pulses, high))
                .and_then(|builder| builder.finish())
                .unwrap_or_else(|failure| panic!("snapshot must build: {failure}"));
            let mut machine = fixture.compiled.spawn(policy());
            let initialized = machine
                .apply(Transaction::initialize(
                    Time::from_ticks(0),
                    machine.revision(),
                    snapshot,
                ))
                .unwrap_or_else(|failure| panic!("family must initialize: {failure}"));
            let expected = expected_counts(&fixture, control, count);
            assert_eq!(observed_counts(initialized.output_events()), expected);
            assert_eq!(
                observed_output_order(initialized.output_events()),
                expected.keys().copied().collect::<Vec<_>>()
            );

            let next_control = control.invert();
            let (low, high) = if next_control.is_low() {
                (count, unselected)
            } else {
                (unselected, count)
            };
            let simultaneous = fixture
                .compiled
                .input_delta()
                .set(fixture.selector, next_control)
                .and_then(|builder| builder.pulse(fixture.gate_pulses, count))
                .and_then(|builder| builder.pulse(fixture.low_pulses, low))
                .and_then(|builder| builder.pulse(fixture.high_pulses, high))
                .and_then(|builder| builder.finish())
                .unwrap_or_else(|failure| panic!("delta must build: {failure}"));
            let advanced = machine
                .apply(Transaction::advance(
                    Time::from_ticks(1),
                    machine.revision(),
                    simultaneous,
                ))
                .unwrap_or_else(|failure| panic!("family must advance: {failure}"));
            let expected = expected_counts(&fixture, next_control, count);
            assert_eq!(observed_counts(advanced.output_events()), expected);
            assert_eq!(
                observed_output_order(advanced.output_events()),
                expected.keys().copied().collect::<Vec<_>>()
            );

            let empty = fixture
                .compiled
                .input_delta()
                .finish()
                .unwrap_or_else(|failure| panic!("empty delta must build: {failure}"));
            let expired = machine
                .apply(Transaction::advance(
                    Time::from_ticks(2),
                    machine.revision(),
                    empty,
                ))
                .unwrap_or_else(|failure| panic!("empty reaction must advance: {failure}"));
            assert!(expired.output_events().is_empty());
        }
    }
}

fn reachable_observations(
    provenance: &ProvenanceView<TestDomain>,
    root: CauseRef,
) -> (LevelObservations, PulseObservations) {
    let mut pending = vec![root];
    let mut visited = BTreeSet::new();
    let mut levels = BTreeSet::new();
    let mut pulses = BTreeSet::new();
    while let Some(cause) = pending.pop() {
        if !visited.insert(cause) {
            continue;
        }
        match provenance
            .inspect(cause)
            .unwrap_or_else(|failure| panic!("cause must resolve: {failure}"))
        {
            CauseInspection::ExternalObservation { input, value } => {
                levels.insert((input, value));
            }
            CauseInspection::ExternalPulseObservation { input, count } => {
                pulses.insert((input, count));
            }
            CauseInspection::Derived { supporters, .. }
            | CauseInspection::PulseDerived { supporters, .. }
            | CauseInspection::PendingPulseDelay { supporters, .. } => {
                pending.extend_from_slice(supporters);
            }
            _ => {}
        }
    }
    (levels, pulses)
}

#[test]
fn every_emitted_family_event_retains_current_control_and_selected_batch_provenance() {
    let fixture = family_fixture();
    let snapshot = fixture
        .compiled
        .input_snapshot()
        .set(fixture.selector, LogicLevel::High)
        .and_then(|builder| builder.pulse(fixture.gate_pulses, PulseCount::new(17)))
        .and_then(|builder| builder.pulse(fixture.low_pulses, PulseCount::new(2)))
        .and_then(|builder| builder.pulse(fixture.high_pulses, PulseCount::new(17)))
        .and_then(|builder| builder.finish())
        .unwrap_or_else(|failure| panic!("snapshot must build: {failure}"));
    let mut machine = fixture.compiled.spawn(policy());
    let result = machine
        .apply(Transaction::initialize(
            Time::from_ticks(0),
            machine.revision(),
            snapshot,
        ))
        .unwrap_or_else(|failure| panic!("family must initialize: {failure}"));
    assert_eq!(result.output_events().len(), 3);
    for event in result.output_events() {
        let OutputEvent::Pulsed {
            output,
            cause,
            count,
            ..
        } = event
        else {
            panic!("family exposes only pulse outputs");
        };
        assert_eq!(*count, PulseCount::new(17));
        let (levels, pulses) = reachable_observations(result.provenance(), *cause);
        assert!(levels.contains(&(fixture.selector, LogicLevel::High)));
        assert!(
            pulses
                .iter()
                .any(|(_, count)| *count == PulseCount::new(17))
        );
        if *output == fixture.select_output {
            assert!(
                !pulses.contains(&(fixture.low_pulses, PulseCount::new(2))),
                "the suppressed branch is not an emitted contributor"
            );
        }
    }
}

#[test]
fn exact_dynamic_route_roles_match_typed_identity_and_role_defects_block() {
    let selector = ExternalInputKey::<Level>::from_u128(10);
    let pulses = ExternalInputKey::<Pulse>::from_u128(11);
    let selector_port = InPortKey::<Level>::from_u128(20);
    let pulses_port = InPortKey::<Pulse>::from_u128(21);
    let when_low = OutPortKey::<Pulse>::from_u128(30);
    let when_high = OutPortKey::<Pulse>::from_u128(31);
    let node = NodeKey::from_u128(2);
    let definition = |input_roles, output_roles| {
        UncheckedNetwork::<TestDomain>::new(
            NetworkKey::from_u128(1),
            TimeDomainId::from_u128(2),
            DiagnosticMeta::default(),
            vec![NodeDef::new(
                node,
                NodeKind::pulse_route(),
                NodePorts::with_roles(
                    vec![selector_port.into(), pulses_port.into()],
                    input_roles,
                    vec![when_low.into(), when_high.into()],
                    output_roles,
                ),
                DiagnosticMeta::default(),
            )],
            vec![
                ExternalInputDef::new(selector.into(), DiagnosticMeta::default()),
                ExternalInputDef::new(pulses.into(), DiagnosticMeta::default()),
            ],
            vec![
                ExternalOutputDef::new(
                    ExternalOutputKey::<Pulse>::from_u128(40).into(),
                    SignalSourceKey::NodeOutput(when_low).into(),
                    DiagnosticMeta::default(),
                ),
                ExternalOutputDef::new(
                    ExternalOutputKey::<Pulse>::from_u128(41).into(),
                    SignalSourceKey::NodeOutput(when_high).into(),
                    DiagnosticMeta::default(),
                ),
            ],
            vec![
                ConnectionDef::new(
                    ConnectionKey::from_u128(0),
                    selector.into(),
                    selector_port.into(),
                    DiagnosticMeta::default(),
                ),
                ConnectionDef::new(
                    ConnectionKey::from_u128(1),
                    pulses.into(),
                    pulses_port.into(),
                    DiagnosticMeta::default(),
                ),
            ],
        )
    };
    let valid = definition(
        vec![InputPortRole::Selector, InputPortRole::Pulses],
        vec![OutputPortRole::WhenLow, OutputPortRole::WhenHigh],
    )
    .validate()
    .require_artifact()
    .unwrap_or_else(|failure| panic!("dynamic route must validate: {failure:?}"));
    let swapped_output_roles = definition(
        vec![InputPortRole::Selector, InputPortRole::Pulses],
        vec![OutputPortRole::WhenHigh, OutputPortRole::WhenLow],
    )
    .validate()
    .require_artifact()
    .unwrap_or_else(|failure| panic!("swapped route roles remain structurally valid: {failure:?}"));
    assert_ne!(
        valid.fingerprint(),
        swapped_output_roles.fingerprint(),
        "stable route output roles participate in identity"
    );

    let mut typed = NetworkBuilder::<TestDomain>::with_key(
        NetworkKey::from_u128(1),
        TimeDomainId::from_u128(2),
    );
    let selector_signal = typed
        .add_level_input(selector, DiagnosticMeta::default())
        .unwrap_or_else(|failure| panic!("typed selector must author: {failure:?}"));
    let pulse_signal = typed
        .add_pulse_input(pulses, DiagnosticMeta::default())
        .unwrap_or_else(|failure| panic!("typed pulse must author: {failure:?}"));
    let routed = typed
        .add_pulse_route_with_ports(
            node,
            selector_port,
            pulses_port,
            when_low,
            when_high,
            selector_signal,
            pulse_signal,
            DiagnosticMeta::default(),
        )
        .unwrap_or_else(|failure| panic!("typed route must author: {failure:?}"))
        .into_outputs();
    typed
        .add_pulse_output(
            ExternalOutputKey::from_u128(40),
            routed.when_low,
            DiagnosticMeta::default(),
        )
        .and_then(|()| {
            typed.add_pulse_output(
                ExternalOutputKey::from_u128(41),
                routed.when_high,
                DiagnosticMeta::default(),
            )
        })
        .unwrap_or_else(|failure| panic!("typed outputs must author: {failure:?}"));
    let typed = typed
        .finish()
        .require_artifact()
        .unwrap_or_else(|failure| panic!("typed route must validate: {failure:?}"));
    assert_eq!(typed.fingerprint(), valid.fingerprint());

    for (validated, expected_output) in [
        (&valid, ExternalOutputKey::<Pulse>::from_u128(40)),
        (
            &swapped_output_roles,
            ExternalOutputKey::<Pulse>::from_u128(41),
        ),
    ] {
        let compiled = validated
            .compile_ref()
            .require_artifact()
            .unwrap_or_else(|failure| panic!("dynamic route must compile: {failure:?}"));
        let snapshot = compiled
            .input_snapshot()
            .set(selector, LogicLevel::Low)
            .and_then(|builder| builder.pulse(pulses, PulseCount::new(9)))
            .and_then(|builder| builder.finish())
            .unwrap_or_else(|failure| panic!("dynamic route snapshot must build: {failure}"));
        let mut machine = compiled.spawn(policy());
        let result = machine
            .apply(Transaction::initialize(
                Time::from_ticks(0),
                machine.revision(),
                snapshot,
            ))
            .unwrap_or_else(|failure| panic!("dynamic route must initialize: {failure}"));
        assert!(matches!(
            result.output_events(),
            [OutputEvent::Pulsed { output, count, .. }]
                if *output == expected_output && *count == PulseCount::new(9)
        ));
    }

    for malformed in [
        definition(
            vec![InputPortRole::Selector, InputPortRole::Selector],
            vec![OutputPortRole::WhenLow, OutputPortRole::WhenHigh],
        ),
        definition(
            vec![InputPortRole::Selector, InputPortRole::Pulses],
            vec![OutputPortRole::WhenLow, OutputPortRole::WhenLow],
        ),
    ] {
        let report = malformed.validate();
        assert!(report.artifact().is_none());
        assert!(report.diagnostics().iter().any(|diagnostic| {
            diagnostic.problem().code() == DiagnosticCode::ValidationInvalidFixedArity
        }));
    }

    let route_report = |pulse_drivers: Vec<ConnectionDef>| {
        let mut connections = vec![ConnectionDef::new(
            ConnectionKey::from_u128(70),
            selector.into(),
            selector_port.into(),
            DiagnosticMeta::default(),
        )];
        connections.extend(pulse_drivers);
        UncheckedNetwork::<TestDomain>::new(
            NetworkKey::from_u128(60),
            TimeDomainId::from_u128(61),
            DiagnosticMeta::default(),
            vec![NodeDef::new(
                node,
                NodeKind::pulse_route(),
                NodePorts::with_roles(
                    vec![selector_port.into(), pulses_port.into()],
                    vec![InputPortRole::Selector, InputPortRole::Pulses],
                    vec![when_low.into(), when_high.into()],
                    vec![OutputPortRole::WhenLow, OutputPortRole::WhenHigh],
                ),
                DiagnosticMeta::default(),
            )],
            vec![
                ExternalInputDef::new(selector.into(), DiagnosticMeta::default()),
                ExternalInputDef::new(pulses.into(), DiagnosticMeta::default()),
            ],
            Vec::new(),
            connections,
        )
        .validate()
    };
    let multiple = route_report(vec![
        ConnectionDef::new(
            ConnectionKey::from_u128(71),
            pulses.into(),
            pulses_port.into(),
            DiagnosticMeta::default(),
        ),
        ConnectionDef::new(
            ConnectionKey::from_u128(72),
            when_low.into(),
            pulses_port.into(),
            DiagnosticMeta::default(),
        ),
    ]);
    assert!(multiple.artifact().is_none());
    assert!(multiple.diagnostics().iter().any(|diagnostic| {
        diagnostic.problem().code() == DiagnosticCode::ValidationUnsupportedMultipleDrivers
    }));
    let cyclic = route_report(vec![ConnectionDef::new(
        ConnectionKey::from_u128(72),
        when_low.into(),
        pulses_port.into(),
        DiagnosticMeta::default(),
    )]);
    assert!(cyclic.artifact().is_none());
    assert!(
        cyclic
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.problem().code()
                == DiagnosticCode::ValidationCurrentReactionCycle)
    );
}

#[test]
fn absent_mixed_kind_ports_report_every_required_input() {
    let cases = [
        (
            NodeKind::pulse_gate(),
            NodePorts::new(Vec::new(), vec![OutPortKey::<Pulse>::from_u128(10).into()]),
            vec![
                (InputPortRole::Pulses, SignalKind::Pulse),
                (InputPortRole::Enable, SignalKind::Level),
            ],
        ),
        (
            NodeKind::pulse_select(),
            NodePorts::new(Vec::new(), vec![OutPortKey::<Pulse>::from_u128(11).into()]),
            vec![
                (InputPortRole::Selector, SignalKind::Level),
                (InputPortRole::WhenLow, SignalKind::Pulse),
                (InputPortRole::WhenHigh, SignalKind::Pulse),
            ],
        ),
        (
            NodeKind::pulse_route(),
            NodePorts::with_roles(
                Vec::new(),
                Vec::new(),
                vec![
                    OutPortKey::<Pulse>::from_u128(12).into(),
                    OutPortKey::<Pulse>::from_u128(13).into(),
                ],
                vec![OutputPortRole::WhenLow, OutputPortRole::WhenHigh],
            ),
            vec![
                (InputPortRole::Selector, SignalKind::Level),
                (InputPortRole::Pulses, SignalKind::Pulse),
            ],
        ),
    ];

    for (index, (kind, ports, mut expected_kinds)) in cases.into_iter().enumerate() {
        let report = UncheckedNetwork::<TestDomain>::new(
            NetworkKey::from_u128(80 + index as u128),
            TimeDomainId::from_u128(90),
            DiagnosticMeta::default(),
            vec![NodeDef::new(
                NodeKey::from_u128(100 + index as u128),
                kind,
                ports,
                DiagnosticMeta::default(),
            )],
            Vec::new(),
            Vec::new(),
            Vec::new(),
        )
        .validate();
        let mut encountered = report
            .diagnostics()
            .iter()
            .filter_map(|diagnostic| match diagnostic.problem().evidence() {
                ProblemEvidence::ValidationMissingRequiredInput {
                    required: RequiredInputRef::Role(required),
                    expected_kind,
                    ..
                } => Some((*required, *expected_kind)),
                _ => None,
            })
            .collect::<Vec<_>>();
        expected_kinds.sort();
        encountered.sort();
        assert_eq!(encountered, expected_kinds);
    }
}

#[test]
fn module_builder_exposes_and_executes_the_complete_family() {
    let selector_input = ModuleInputKey::<Level>::from_u128(1);
    let gate_input = ModuleInputKey::<Pulse>::from_u128(2);
    let low_input = ModuleInputKey::<Pulse>::from_u128(3);
    let high_input = ModuleInputKey::<Pulse>::from_u128(4);
    let gate_output = ModuleOutputKey::<Pulse>::from_u128(5);
    let select_output = ModuleOutputKey::<Pulse>::from_u128(6);
    let route_low = ModuleOutputKey::<Pulse>::from_u128(7);
    let route_high = ModuleOutputKey::<Pulse>::from_u128(8);
    let mut module = ModuleBuilder::<TestDomain>::new();
    let selector = module
        .add_level_input(selector_input, DiagnosticMeta::default())
        .unwrap_or_else(|failure| panic!("module selector must author: {failure:?}"));
    let gate_pulses = module
        .add_pulse_input(gate_input, DiagnosticMeta::default())
        .unwrap_or_else(|failure| panic!("module gate input must author: {failure:?}"));
    let low_pulses = module
        .add_pulse_input(low_input, DiagnosticMeta::default())
        .unwrap_or_else(|failure| panic!("module low input must author: {failure:?}"));
    let high_pulses = module
        .add_pulse_input(high_input, DiagnosticMeta::default())
        .unwrap_or_else(|failure| panic!("module high input must author: {failure:?}"));
    let gated = module
        .pulse_gate(gate_pulses, selector)
        .unwrap_or_else(|failure| panic!("module gate must author: {failure:?}"));
    let selected = module
        .pulse_select(selector, low_pulses, high_pulses)
        .unwrap_or_else(|failure| panic!("module select must author: {failure:?}"));
    let routed = module
        .pulse_route(selector, gate_pulses)
        .unwrap_or_else(|failure| panic!("module route must author: {failure:?}"));
    module
        .add_pulse_output(gate_output, gated, DiagnosticMeta::default())
        .and_then(|()| module.add_pulse_output(select_output, selected, DiagnosticMeta::default()))
        .and_then(|()| {
            module.add_pulse_output(route_low, routed.when_low, DiagnosticMeta::default())
        })
        .and_then(|()| {
            module.add_pulse_output(route_high, routed.when_high, DiagnosticMeta::default())
        })
        .unwrap_or_else(|failure| panic!("module outputs must author: {failure:?}"));
    let module = module
        .finish()
        .require_artifact()
        .unwrap_or_else(|failure| panic!("family module must validate: {failure:?}"));

    let selector_external = ExternalInputKey::<Level>::from_u128(10);
    let gate_external = ExternalInputKey::<Pulse>::from_u128(11);
    let low_external = ExternalInputKey::<Pulse>::from_u128(12);
    let high_external = ExternalInputKey::<Pulse>::from_u128(13);
    let outputs = [
        ExternalOutputKey::<Pulse>::from_u128(20),
        ExternalOutputKey::<Pulse>::from_u128(21),
        ExternalOutputKey::<Pulse>::from_u128(22),
        ExternalOutputKey::<Pulse>::from_u128(23),
    ];
    let mut network = NetworkBuilder::<TestDomain>::new(TimeDomainId::from_u128(30));
    let selector = network
        .add_level_input(selector_external, DiagnosticMeta::default())
        .unwrap_or_else(|failure| panic!("selector must author: {failure:?}"));
    let gate = network
        .add_pulse_input(gate_external, DiagnosticMeta::default())
        .unwrap_or_else(|failure| panic!("gate input must author: {failure:?}"));
    let low = network
        .add_pulse_input(low_external, DiagnosticMeta::default())
        .unwrap_or_else(|failure| panic!("low input must author: {failure:?}"));
    let high = network
        .add_pulse_input(high_external, DiagnosticMeta::default())
        .unwrap_or_else(|failure| panic!("high input must author: {failure:?}"));
    let instance = network
        .instantiate(
            &module,
            ModuleInstanceKey::from_u128(40),
            DiagnosticMeta::default(),
        )
        .unwrap_or_else(|failure| panic!("module instance must begin: {failure:?}"))
        .bind_level(selector_input, selector)
        .and_then(|builder| builder.bind_pulse(gate_input, gate))
        .and_then(|builder| builder.bind_pulse(low_input, low))
        .and_then(|builder| builder.bind_pulse(high_input, high))
        .and_then(|builder| builder.finish())
        .unwrap_or_else(|failure| panic!("module instance must bind: {failure:?}"));
    for (external, module_output) in
        outputs
            .into_iter()
            .zip([gate_output, select_output, route_low, route_high])
    {
        network
            .add_pulse_output(
                external,
                instance
                    .pulse_output(module_output)
                    .unwrap_or_else(|failure| panic!("module output must resolve: {failure:?}")),
                DiagnosticMeta::default(),
            )
            .unwrap_or_else(|failure| panic!("external output must author: {failure:?}"));
    }
    let compiled = network
        .finish()
        .require_artifact()
        .unwrap_or_else(|failure| panic!("wrapped family must validate: {failure:?}"))
        .compile()
        .require_artifact()
        .unwrap_or_else(|failure| panic!("wrapped family must compile: {failure:?}"));
    let snapshot = compiled
        .input_snapshot()
        .set(selector_external, LogicLevel::High)
        .and_then(|builder| builder.pulse(gate_external, PulseCount::new(17)))
        .and_then(|builder| builder.pulse(low_external, PulseCount::new(2)))
        .and_then(|builder| builder.pulse(high_external, PulseCount::new(17)))
        .and_then(|builder| builder.finish())
        .unwrap_or_else(|failure| panic!("wrapped snapshot must build: {failure}"));
    let mut machine = compiled.spawn(policy());
    let result = machine
        .apply(Transaction::initialize(
            Time::from_ticks(0),
            machine.revision(),
            snapshot,
        ))
        .unwrap_or_else(|failure| panic!("wrapped family must initialize: {failure}"));
    assert_eq!(
        observed_counts(result.output_events()),
        BTreeMap::from([
            (outputs[0], PulseCount::new(17)),
            (outputs[1], PulseCount::new(17)),
            (outputs[3], PulseCount::new(17)),
        ])
    );
}

#[test]
fn upstream_overflow_and_operation_budget_reject_all_three_nodes_atomically() {
    let mut builder = NetworkBuilder::<TestDomain>::new(TimeDomainId::from_u128(70));
    let (selector_key, selector) = builder.level_input("selector");
    let (left_key, left) = builder.pulse_input("left");
    let (right_key, right) = builder.pulse_input("right");
    let merge_node = NodeKey::from_u128(71);
    let merged = builder
        .add_merge(merge_node, [left, right], DiagnosticMeta::default())
        .unwrap_or_else(|failure| panic!("Merge must author: {failure:?}"))
        .into_outputs();
    let gated = builder
        .pulse_gate(merged, selector)
        .unwrap_or_else(|failure| panic!("PulseGate must author: {failure:?}"));
    let selected = builder
        .pulse_select(selector, left, merged)
        .unwrap_or_else(|failure| panic!("PulseSelect must author: {failure:?}"));
    let routed = builder
        .pulse_route(selector, merged)
        .unwrap_or_else(|failure| panic!("PulseRoute must author: {failure:?}"));
    for (name, signal) in [
        ("gate", gated),
        ("select", selected),
        ("route-low", routed.when_low),
        ("route-high", routed.when_high),
    ] {
        builder
            .pulse_output(name, signal)
            .unwrap_or_else(|failure| panic!("output must author: {failure:?}"));
    }
    let compiled = builder
        .finish()
        .require_artifact()
        .unwrap_or_else(|failure| panic!("failure fixture must validate: {failure:?}"))
        .compile()
        .require_artifact()
        .unwrap_or_else(|failure| panic!("failure fixture must compile: {failure:?}"));
    let snapshot = compiled
        .input_snapshot()
        .set(selector_key, LogicLevel::Low)
        .and_then(|builder| builder.finish())
        .unwrap_or_else(|failure| panic!("initial snapshot must build: {failure}"));
    let mut machine = compiled.spawn(policy());
    machine
        .apply(Transaction::initialize(
            Time::from_ticks(0),
            machine.revision(),
            snapshot,
        ))
        .unwrap_or_else(|failure| panic!("zero reaction must initialize: {failure}"));
    let before = (machine.status(), machine.revision(), machine.now());
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
        .expect_err("upstream overflow must reject the complete family");
    assert_eq!(
        failure.evidence(),
        &RuntimeFailureEvidence::PulseCountOverflow {
            node: mossignal::NodeSubject::Node(merge_node),
        }
    );
    assert_eq!(
        (machine.status(), machine.revision(), machine.now()),
        before
    );

    let zero_operations = RuntimePolicy::builder()
        .max_internal_reactions(10)
        .max_evaluated_operations(0)
        .max_pending_events(10)
        .max_events_created_per_transaction(10)
        .max_required_provenance_growth(100)
        .build()
        .unwrap_or_else(|failure| panic!("zero-operation policy must build: {failure}"));
    let snapshot = compiled
        .input_snapshot()
        .set(selector_key, LogicLevel::High)
        .and_then(|builder| builder.finish())
        .unwrap_or_else(|failure| panic!("budget snapshot must build: {failure}"));
    let mut budgeted = compiled.spawn(zero_operations);
    let before = (budgeted.status(), budgeted.revision(), budgeted.now());
    let failure = budgeted
        .apply(Transaction::initialize(
            Time::from_ticks(0),
            budgeted.revision(),
            snapshot,
        ))
        .expect_err("zero evaluated-operation budget must reject the family");
    assert!(matches!(
        failure.evidence(),
        RuntimeFailureEvidence::BudgetExceeded {
            budget: RuntimePolicyLimit::MaxEvaluatedOperations,
            limit: 0,
            ..
        }
    ));
    assert_eq!(
        (budgeted.status(), budgeted.revision(), budgeted.now()),
        before
    );
}
