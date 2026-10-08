use std::collections::BTreeSet;

use mossignal::authored::{
    ConnectionDef, ExternalInputDef, ExternalOutputDef, InputPortRole, NodeDef, NodeKind,
    NodePorts, UncheckedNetwork,
};
use mossignal::diagnostics::DiagnosticCode;
use mossignal::key::{
    ConnectionKey, ExternalInputKey, ExternalOutputKey, InPortKey, ModuleInputKey,
    ModuleInstanceKey, ModuleOutputKey, NetworkKey, NodeKey, OutPortKey, SignalSourceKey,
};
use mossignal::metadata::DiagnosticMeta;
use mossignal::signal::{Level, LogicLevel, Pulse, PulseCount};
use mossignal::time::Time;
use mossignal::{
    CauseInspection, CauseRef, CompiledNetwork, EdgeConfig, EdgeDetectorInspectionFailure,
    EdgeDetectorKind, EdgeInitialization, EdgeObservation, ModuleBuilder, NetworkBuilder,
    OutputEvent, ProvenanceView, RuntimeFailureEvidence, RuntimePolicy, RuntimePolicyLimit,
    TimeDomainId, ToggleConfig, Transaction,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TestDomain {}

fn policy(operation_limit: u64) -> RuntimePolicy {
    RuntimePolicy::builder()
        .max_internal_reactions(100)
        .max_evaluated_operations(operation_limit)
        .max_pending_events(100)
        .max_events_created_per_transaction(100)
        .max_required_provenance_growth(10_000)
        .build()
        .unwrap_or_else(|failure| panic!("complete policy must build: {failure}"))
}

struct EdgeFixture {
    compiled: CompiledNetwork<TestDomain>,
    input: ExternalInputKey<Level>,
    output: ExternalOutputKey<Pulse>,
    node: NodeKey,
}

fn edge_fixture(detector: EdgeDetectorKind, config: EdgeConfig) -> EdgeFixture {
    let input = ExternalInputKey::<Level>::from_u128(10);
    let output = ExternalOutputKey::<Pulse>::from_u128(40);
    let node = NodeKey::from_u128(20);
    let mut builder = NetworkBuilder::<TestDomain>::with_key(
        NetworkKey::from_u128(1),
        TimeDomainId::from_u128(2),
    );
    let signal = builder
        .add_level_input(input, DiagnosticMeta::default())
        .unwrap_or_else(|failure| panic!("level input must author: {failure:?}"));
    let edge = match detector {
        EdgeDetectorKind::Rising => builder.add_rising_edge_with_ports(
            node,
            InPortKey::from_u128(30),
            OutPortKey::from_u128(31),
            signal,
            config,
            DiagnosticMeta::default(),
        ),
        EdgeDetectorKind::Falling => builder.add_falling_edge_with_ports(
            node,
            InPortKey::from_u128(30),
            OutPortKey::from_u128(31),
            signal,
            config,
            DiagnosticMeta::default(),
        ),
        EdgeDetectorKind::Any => builder.add_any_edge_with_ports(
            node,
            InPortKey::from_u128(30),
            OutPortKey::from_u128(31),
            signal,
            config,
            DiagnosticMeta::default(),
        ),
    }
    .unwrap_or_else(|failure| panic!("edge detector must author: {failure:?}"))
    .into_outputs();
    builder
        .add_pulse_output(output, edge, DiagnosticMeta::default())
        .unwrap_or_else(|failure| panic!("pulse output must author: {failure:?}"));
    let compiled = builder
        .finish()
        .require_artifact()
        .unwrap_or_else(|failure| panic!("edge detector must validate: {failure:?}"))
        .compile()
        .require_artifact()
        .unwrap_or_else(|failure| panic!("edge detector must compile: {failure:?}"));
    EdgeFixture {
        compiled,
        input,
        output,
        node,
    }
}

fn snapshot(fixture: &EdgeFixture, value: LogicLevel) -> mossignal::InputSnapshot<TestDomain> {
    fixture
        .compiled
        .input_snapshot()
        .set(fixture.input, value)
        .and_then(mossignal::InputSnapshotBuilder::finish)
        .unwrap_or_else(|failure| panic!("complete snapshot must build: {failure}"))
}

fn delta(fixture: &EdgeFixture, value: LogicLevel) -> mossignal::InputDelta<TestDomain> {
    fixture
        .compiled
        .input_delta()
        .set(fixture.input, value)
        .and_then(mossignal::InputDeltaBuilder::finish)
        .unwrap_or_else(|failure| panic!("delta must build: {failure}"))
}

fn assert_pulse(
    events: &[OutputEvent<TestDomain>],
    output: ExternalOutputKey<Pulse>,
    emitted: bool,
) {
    if emitted {
        assert!(matches!(
            events,
            [OutputEvent::Pulsed { output: actual, count, .. }]
                if *actual == output && *count == PulseCount::ONE
        ));
    } else {
        assert!(events.is_empty());
    }
}

#[test]
fn family_exhausts_previous_observations_current_levels_and_successors() {
    for detector in [
        EdgeDetectorKind::Rising,
        EdgeDetectorKind::Falling,
        EdgeDetectorKind::Any,
    ] {
        for first in [LogicLevel::Low, LogicLevel::High] {
            let fixture = edge_fixture(detector, EdgeConfig::new(EdgeInitialization::Baseline));
            let mut machine = fixture.compiled.spawn(policy(10_000));
            let definition = machine
                .inspect_edge_detector_definition(fixture.node)
                .unwrap_or_else(|failure| panic!("definition must inspect: {failure:?}"));
            assert_eq!(definition.detector(), detector);
            assert_eq!(definition.initialization(), EdgeInitialization::Baseline);
            assert_eq!(
                machine.inspect_edge_detector(fixture.node),
                Err(EdgeDetectorInspectionFailure::NotInitialized)
            );

            let initialized = machine
                .apply(Transaction::initialize(
                    Time::from_ticks(0),
                    machine.revision(),
                    snapshot(&fixture, first),
                ))
                .unwrap_or_else(|failure| panic!("baseline must initialize: {failure}"));
            assert_pulse(initialized.output_events(), fixture.output, false);
            let inspected = machine
                .inspect_edge_detector(fixture.node)
                .unwrap_or_else(|failure| panic!("initialized edge must inspect: {failure:?}"));
            assert_eq!(inspected.committed(), EdgeObservation::Established(first));
            assert_eq!(inspected.input(), first);

            for (step, current) in [LogicLevel::Low, LogicLevel::High].into_iter().enumerate() {
                let previous = machine
                    .inspect_edge_detector(fixture.node)
                    .unwrap()
                    .committed();
                let emitted = detector.emits(previous, current);
                let result = machine
                    .apply(Transaction::advance(
                        Time::from_ticks(step as u64 + 1),
                        machine.revision(),
                        delta(&fixture, current),
                    ))
                    .unwrap_or_else(|failure| panic!("edge reaction must advance: {failure}"));
                assert_pulse(result.output_events(), fixture.output, emitted);
                let inspected = machine.inspect_edge_detector(fixture.node).unwrap();
                assert_eq!(inspected.committed(), EdgeObservation::Established(current));
                assert_eq!(inspected.input(), current);
                assert!(
                    result
                        .provenance()
                        .inspect(inspected.observation_cause())
                        .is_ok()
                );
            }
        }
    }
}

#[test]
fn edge_pulse_activity_is_result_owned_while_remembered_state_remains_inspectable() {
    let fixture = edge_fixture(
        EdgeDetectorKind::Rising,
        EdgeConfig::new(EdgeInitialization::Baseline),
    );
    let mut machine = fixture.compiled.spawn(policy(10_000));
    machine
        .apply(Transaction::initialize(
            Time::from_ticks(0),
            machine.revision(),
            snapshot(&fixture, LogicLevel::Low),
        ))
        .unwrap_or_else(|failure| panic!("edge baseline must initialize: {failure}"));

    let emitted = machine
        .apply(Transaction::advance(
            Time::from_ticks(1),
            machine.revision(),
            delta(&fixture, LogicLevel::High),
        ))
        .unwrap_or_else(|failure| panic!("rising edge must advance: {failure}"));
    let [
        OutputEvent::Pulsed {
            output,
            count,
            stamp: at,
            cause,
            revision,
        },
    ] = emitted.output_events()
    else {
        panic!("the rising edge must publish exactly one pulse event");
    };
    assert_eq!(*output, fixture.output);
    assert_eq!(*count, PulseCount::ONE);
    assert_eq!(at.time(), Time::from_ticks(1));
    assert_eq!(*revision, machine.revision());
    assert!(emitted.provenance().inspect(*cause).is_ok());

    let later = machine
        .apply(Transaction::advance(
            Time::from_ticks(2),
            machine.revision(),
            delta(&fixture, LogicLevel::High),
        ))
        .unwrap_or_else(|failure| panic!("steady input must advance: {failure}"));
    assert!(later.output_events().is_empty());
    let inspected = machine
        .inspect_edge_detector(fixture.node)
        .unwrap_or_else(|failure| panic!("remembered edge state must inspect: {failure:?}"));
    assert_eq!(
        inspected.committed(),
        EdgeObservation::Established(LogicLevel::High)
    );
    assert_eq!(inspected.input(), LogicLevel::High);
    assert_eq!(inspected.at(), Time::from_ticks(2));
    assert!(
        later
            .provenance()
            .inspect(inspected.observation_cause())
            .is_ok()
    );
    assert!(emitted.provenance().inspect(*cause).is_ok());
}

#[test]
fn assume_policy_compares_the_first_observation_normally() {
    for detector in [
        EdgeDetectorKind::Rising,
        EdgeDetectorKind::Falling,
        EdgeDetectorKind::Any,
    ] {
        for assumed in [LogicLevel::Low, LogicLevel::High] {
            for current in [LogicLevel::Low, LogicLevel::High] {
                let fixture = edge_fixture(
                    detector,
                    EdgeConfig::new(EdgeInitialization::Assume(assumed)),
                );
                let mut machine = fixture.compiled.spawn(policy(10_000));
                let result = machine
                    .apply(Transaction::initialize(
                        Time::from_ticks(7),
                        machine.revision(),
                        snapshot(&fixture, current),
                    ))
                    .unwrap_or_else(|failure| panic!("assumed edge must initialize: {failure}"));
                let emitted = detector.emits(EdgeObservation::Established(assumed), current);
                assert_pulse(result.output_events(), fixture.output, emitted);
                let inspection = machine.inspect_edge_detector(fixture.node).unwrap();
                assert_eq!(
                    inspection.initialization(),
                    EdgeInitialization::Assume(assumed)
                );
                assert_eq!(
                    inspection.committed(),
                    EdgeObservation::Established(current)
                );

                let next = current.invert();
                let ready = machine
                    .apply(Transaction::advance(
                        Time::from_ticks(8),
                        machine.revision(),
                        delta(&fixture, next),
                    ))
                    .unwrap_or_else(|failure| panic!("assumed edge must advance: {failure}"));
                let ready_emitted = detector.emits(inspection.committed(), next);
                assert_pulse(ready.output_events(), fixture.output, ready_emitted);
                let ready_inspection = machine.inspect_edge_detector(fixture.node).unwrap();
                assert_eq!(
                    ready_inspection.committed(),
                    EdgeObservation::Established(next)
                );
                assert_eq!(ready_inspection.input(), next);
                assert!(
                    ready
                        .provenance()
                        .inspect(ready_inspection.observation_cause())
                        .is_ok()
                );
            }
        }
    }
}

fn reachable_levels(
    provenance: &ProvenanceView<TestDomain>,
    root: CauseRef,
) -> BTreeSet<(ExternalInputKey<Level>, LogicLevel)> {
    let mut pending = vec![root];
    let mut visited = BTreeSet::new();
    let mut levels = BTreeSet::new();
    while let Some(cause) = pending.pop() {
        if !visited.insert(cause) {
            continue;
        }
        match provenance
            .inspect(cause)
            .unwrap_or_else(|failure| panic!("cause must resolve: {failure}"))
        {
            CauseInspection::ExternalObservation { input, value, .. } => {
                levels.insert((input, value));
            }
            CauseInspection::Derived { supporters, .. }
            | CauseInspection::PulseDerived { supporters, .. }
            | CauseInspection::PendingPulseDelay { supporters, .. } => {
                pending.extend_from_slice(supporters);
            }
            CauseInspection::InitializationTransaction { .. }
            | CauseInspection::ReadyTransaction { .. }
            | CauseInspection::ExternalPulseObservation { .. } => {}
            _ => {}
        }
    }
    levels
}

#[test]
fn emitted_edge_provenance_reaches_current_and_remembered_observations() {
    let fixture = edge_fixture(
        EdgeDetectorKind::Rising,
        EdgeConfig::new(EdgeInitialization::Baseline),
    );
    let mut machine = fixture.compiled.spawn(policy(10_000));
    machine
        .apply(Transaction::initialize(
            Time::from_ticks(0),
            machine.revision(),
            snapshot(&fixture, LogicLevel::Low),
        ))
        .unwrap();
    let result = machine
        .apply(Transaction::advance(
            Time::from_ticks(1),
            machine.revision(),
            delta(&fixture, LogicLevel::High),
        ))
        .unwrap();
    let [OutputEvent::Pulsed { cause, .. }] = result.output_events() else {
        panic!("rising transition must emit exactly one pulse event")
    };
    let roots = reachable_levels(result.provenance(), *cause);
    assert_eq!(
        roots,
        BTreeSet::from([
            (fixture.input, LogicLevel::Low),
            (fixture.input, LogicLevel::High),
        ])
    );
}

#[test]
fn stateful_chain_uses_current_outputs_and_commits_all_successors_once() {
    let pulse_input = ExternalInputKey::<Pulse>::from_u128(10);
    let edge_output = ExternalOutputKey::<Pulse>::from_u128(40);
    let level_output = ExternalOutputKey::<Level>::from_u128(41);
    let mut builder = NetworkBuilder::<TestDomain>::new(TimeDomainId::from_u128(2));
    let pulses = builder
        .add_pulse_input(pulse_input, DiagnosticMeta::default())
        .unwrap();
    let first = builder
        .toggle(pulses, ToggleConfig::new(LogicLevel::Low))
        .unwrap();
    let edge = builder
        .rising_edge(
            first,
            EdgeConfig::new(EdgeInitialization::Assume(LogicLevel::Low)),
        )
        .unwrap();
    let second = builder
        .toggle(edge, ToggleConfig::new(LogicLevel::Low))
        .unwrap();
    builder
        .add_pulse_output(edge_output, edge, DiagnosticMeta::default())
        .and_then(|()| builder.add_level_output(level_output, second, DiagnosticMeta::default()))
        .unwrap();
    let compiled = builder
        .finish()
        .require_artifact()
        .unwrap()
        .compile()
        .require_artifact()
        .unwrap();
    let initial = compiled
        .input_snapshot()
        .pulse(pulse_input, PulseCount::ONE)
        .and_then(mossignal::InputSnapshotBuilder::finish)
        .unwrap();
    let mut machine = compiled.spawn(policy(10_000));
    let result = machine
        .apply(Transaction::initialize(
            Time::from_ticks(0),
            machine.revision(),
            initial,
        ))
        .unwrap();
    assert!(result.output_events().iter().any(|event| matches!(
        event,
        OutputEvent::Pulsed { output, count, .. }
            if *output == edge_output && *count == PulseCount::ONE
    )));
    assert_eq!(machine.output_level(level_output), Some(LogicLevel::High));
}

#[test]
fn edge_detector_observes_only_the_settled_reconvergent_level() {
    let left = ExternalInputKey::<Level>::from_u128(10);
    let right = ExternalInputKey::<Level>::from_u128(11);
    let output = ExternalOutputKey::<Pulse>::from_u128(40);
    let mut builder = NetworkBuilder::<TestDomain>::new(TimeDomainId::from_u128(2));
    let left_signal = builder
        .add_level_input(left, DiagnosticMeta::default())
        .unwrap();
    let right_signal = builder
        .add_level_input(right, DiagnosticMeta::default())
        .unwrap();
    let any = builder.any([left_signal, right_signal]).unwrap();
    let edge = builder
        .any_edge(any, EdgeConfig::new(EdgeInitialization::Baseline))
        .unwrap();
    builder
        .add_pulse_output(output, edge, DiagnosticMeta::default())
        .unwrap();
    let compiled = builder
        .finish()
        .require_artifact()
        .unwrap()
        .compile()
        .require_artifact()
        .unwrap();
    let initial = compiled
        .input_snapshot()
        .set(left, LogicLevel::High)
        .and_then(|builder| builder.set(right, LogicLevel::Low))
        .and_then(mossignal::InputSnapshotBuilder::finish)
        .unwrap();
    let mut machine = compiled.spawn(policy(10_000));
    machine
        .apply(Transaction::initialize(
            Time::from_ticks(0),
            machine.revision(),
            initial,
        ))
        .unwrap();
    let simultaneous = compiled
        .input_delta()
        .set(left, LogicLevel::Low)
        .and_then(|builder| builder.set(right, LogicLevel::High))
        .and_then(mossignal::InputDeltaBuilder::finish)
        .unwrap();
    let result = machine
        .apply(Transaction::advance(
            Time::from_ticks(1),
            machine.revision(),
            simultaneous,
        ))
        .unwrap();
    assert!(result.output_events().is_empty());
}

#[test]
fn dynamic_shape_cycle_and_typed_module_paths_are_coherent() {
    let level = ExternalInputKey::<Level>::from_u128(10);
    let edge_input = InPortKey::<Level>::from_u128(20);
    let edge_output = OutPortKey::<Pulse>::from_u128(21);
    let node = NodeKey::from_u128(30);
    let output = ExternalOutputKey::<Pulse>::from_u128(40);
    let config = EdgeConfig::new(EdgeInitialization::Assume(LogicLevel::Low));
    let direct = UncheckedNetwork::<TestDomain>::new(
        NetworkKey::from_u128(1),
        TimeDomainId::from_u128(2),
        DiagnosticMeta::default(),
        vec![NodeDef::new(
            node,
            NodeKind::rising_edge(config),
            NodePorts::new(vec![edge_input.into()], vec![edge_output.into()]),
            DiagnosticMeta::default(),
        )],
        vec![ExternalInputDef::new(
            level.into(),
            DiagnosticMeta::default(),
        )],
        vec![ExternalOutputDef::new(
            output.into(),
            SignalSourceKey::NodeOutput(edge_output).into(),
            DiagnosticMeta::default(),
        )],
        vec![ConnectionDef::new(
            ConnectionKey::from_u128(50),
            level.into(),
            edge_input.into(),
            DiagnosticMeta::default(),
        )],
    );
    let _validated_direct = direct
        .clone()
        .validate()
        .require_artifact()
        .unwrap_or_else(|failure| panic!("direct edge must validate: {failure:?}"));

    let mut typed = NetworkBuilder::<TestDomain>::with_key(
        NetworkKey::from_u128(1),
        TimeDomainId::from_u128(2),
    );
    let signal = typed
        .add_level_input(level, DiagnosticMeta::default())
        .unwrap();
    let edge = typed
        .add_rising_edge_with_ports(
            node,
            edge_input,
            edge_output,
            signal,
            config,
            DiagnosticMeta::default(),
        )
        .unwrap()
        .into_outputs();
    typed
        .add_pulse_output(output, edge, DiagnosticMeta::default())
        .unwrap();
    let typed_unchecked = typed.into_unchecked();
    assert!(
        matches!(typed_unchecked.nodes()[0].kind(), NodeKind::RisingEdge(actual) if *actual == config)
    );
    assert_eq!(
        typed_unchecked.nodes()[0].ports(),
        direct.nodes()[0].ports()
    );
    let key_aligned_direct = UncheckedNetwork::<TestDomain>::new(
        NetworkKey::from_u128(1),
        TimeDomainId::from_u128(2),
        DiagnosticMeta::default(),
        direct.nodes().to_vec(),
        direct.external_inputs().to_vec(),
        direct.external_outputs().to_vec(),
        vec![ConnectionDef::new(
            typed_unchecked.connections()[0].key(),
            level.into(),
            edge_input.into(),
            DiagnosticMeta::default(),
        )],
    );
    assert_eq!(
        typed_unchecked
            .validate()
            .require_artifact()
            .unwrap()
            .fingerprint(),
        key_aligned_direct
            .validate()
            .require_artifact()
            .unwrap()
            .fingerprint()
    );

    let malformed = UncheckedNetwork::<TestDomain>::new(
        NetworkKey::from_u128(1),
        TimeDomainId::from_u128(2),
        DiagnosticMeta::default(),
        vec![NodeDef::new(
            node,
            NodeKind::rising_edge(config),
            NodePorts::with_input_roles(
                vec![InPortKey::<Pulse>::from_u128(20).into()],
                vec![InputPortRole::Input],
                vec![OutPortKey::<Level>::from_u128(21).into()],
            ),
            DiagnosticMeta::default(),
        )],
        Vec::new(),
        Vec::new(),
        Vec::new(),
    )
    .validate();
    assert!(malformed.artifact().is_none());
    assert!(malformed.diagnostics().iter().any(|finding| {
        finding.problem().code() == DiagnosticCode::ValidationInvalidFixedArity
    }));
    assert!(malformed.diagnostics().iter().any(|finding| {
        finding.problem().code() == DiagnosticCode::ValidationMissingRequiredInput
    }));

    let second_level = ExternalInputKey::<Level>::from_u128(11);
    let multiply_driven = UncheckedNetwork::<TestDomain>::new(
        NetworkKey::from_u128(4),
        TimeDomainId::from_u128(2),
        DiagnosticMeta::default(),
        vec![NodeDef::new(
            node,
            NodeKind::rising_edge(config),
            NodePorts::new(vec![edge_input.into()], vec![edge_output.into()]),
            DiagnosticMeta::default(),
        )],
        vec![
            ExternalInputDef::new(level.into(), DiagnosticMeta::default()),
            ExternalInputDef::new(second_level.into(), DiagnosticMeta::default()),
        ],
        Vec::new(),
        vec![
            ConnectionDef::new(
                ConnectionKey::from_u128(51),
                level.into(),
                edge_input.into(),
                DiagnosticMeta::default(),
            ),
            ConnectionDef::new(
                ConnectionKey::from_u128(52),
                second_level.into(),
                edge_input.into(),
                DiagnosticMeta::default(),
            ),
        ],
    )
    .validate();
    assert!(multiply_driven.diagnostics().iter().any(|finding| {
        finding.problem().code() == DiagnosticCode::ValidationUnsupportedMultipleDrivers
    }));

    let duplicate_node = UncheckedNetwork::<TestDomain>::new(
        NetworkKey::from_u128(5),
        TimeDomainId::from_u128(2),
        DiagnosticMeta::default(),
        vec![
            NodeDef::new(
                node,
                NodeKind::rising_edge(config),
                NodePorts::new(vec![edge_input.into()], vec![edge_output.into()]),
                DiagnosticMeta::default(),
            ),
            NodeDef::new(
                node,
                NodeKind::falling_edge(config),
                NodePorts::new(
                    vec![InPortKey::<Level>::from_u128(22).into()],
                    vec![OutPortKey::<Pulse>::from_u128(23).into()],
                ),
                DiagnosticMeta::default(),
            ),
        ],
        Vec::new(),
        Vec::new(),
        Vec::new(),
    )
    .validate();
    assert!(
        duplicate_node
            .diagnostics()
            .iter()
            .any(|finding| { finding.problem().code() == DiagnosticCode::ValidationDuplicateKey })
    );

    let pulse_source = ExternalInputKey::<Pulse>::from_u128(12);
    let wrong_connection_kind = UncheckedNetwork::<TestDomain>::new(
        NetworkKey::from_u128(6),
        TimeDomainId::from_u128(2),
        DiagnosticMeta::default(),
        vec![NodeDef::new(
            node,
            NodeKind::rising_edge(config),
            NodePorts::new(vec![edge_input.into()], vec![edge_output.into()]),
            DiagnosticMeta::default(),
        )],
        vec![ExternalInputDef::new(
            pulse_source.into(),
            DiagnosticMeta::default(),
        )],
        Vec::new(),
        vec![ConnectionDef::new(
            ConnectionKey::from_u128(53),
            pulse_source.into(),
            edge_input.into(),
            DiagnosticMeta::default(),
        )],
    )
    .validate();
    assert!(wrong_connection_kind.diagnostics().iter().any(|finding| {
        finding.problem().code() == DiagnosticCode::ValidationSignalKindMismatch
    }));

    let toggle_input = InPortKey::<Pulse>::from_u128(60);
    let toggle_output = OutPortKey::<Level>::from_u128(61);
    let cyclic = UncheckedNetwork::<TestDomain>::new(
        NetworkKey::from_u128(3),
        TimeDomainId::from_u128(2),
        DiagnosticMeta::default(),
        vec![
            NodeDef::new(
                node,
                NodeKind::rising_edge(config),
                NodePorts::new(vec![edge_input.into()], vec![edge_output.into()]),
                DiagnosticMeta::default(),
            ),
            NodeDef::new(
                NodeKey::from_u128(31),
                NodeKind::toggle(LogicLevel::Low),
                NodePorts::with_input_roles(
                    vec![toggle_input.into()],
                    vec![InputPortRole::Toggle],
                    vec![toggle_output.into()],
                ),
                DiagnosticMeta::default(),
            ),
        ],
        Vec::new(),
        Vec::new(),
        vec![
            ConnectionDef::new(
                ConnectionKey::from_u128(70),
                edge_output.into(),
                toggle_input.into(),
                DiagnosticMeta::default(),
            ),
            ConnectionDef::new(
                ConnectionKey::from_u128(71),
                toggle_output.into(),
                edge_input.into(),
                DiagnosticMeta::default(),
            ),
        ],
    )
    .validate();
    assert!(cyclic.artifact().is_none());
    assert!(cyclic.diagnostics().iter().any(|finding| {
        finding.problem().code() == DiagnosticCode::ValidationCurrentReactionCycle
    }));

    let module_input = ModuleInputKey::<Level>::from_u128(80);
    let module_output = ModuleOutputKey::<Pulse>::from_u128(81);
    let mut module = ModuleBuilder::<TestDomain>::new();
    let signal = module
        .add_level_input(module_input, DiagnosticMeta::default())
        .unwrap();
    let edge = module
        .add_rising_edge_with_ports(
            NodeKey::from_u128(82),
            InPortKey::from_u128(83),
            OutPortKey::from_u128(84),
            signal,
            config,
            DiagnosticMeta::default(),
        )
        .unwrap()
        .into_outputs();
    module
        .add_pulse_output(module_output, edge, DiagnosticMeta::default())
        .unwrap();
    let module = module.finish().require_artifact().unwrap();
    let rising_module_fingerprint = module.fingerprint();
    let mut alternative_module = ModuleBuilder::<TestDomain>::new();
    let alternative_signal = alternative_module
        .add_level_input(module_input, DiagnosticMeta::default())
        .unwrap();
    let alternative_edge = alternative_module
        .add_falling_edge_with_ports(
            NodeKey::from_u128(82),
            InPortKey::from_u128(83),
            OutPortKey::from_u128(84),
            alternative_signal,
            config,
            DiagnosticMeta::default(),
        )
        .unwrap()
        .into_outputs();
    alternative_module
        .add_pulse_output(module_output, alternative_edge, DiagnosticMeta::default())
        .unwrap();
    assert_ne!(
        rising_module_fingerprint,
        alternative_module
            .finish()
            .require_artifact()
            .unwrap()
            .fingerprint(),
        "the detector kind participates in module identity"
    );
    let mut network = NetworkBuilder::<TestDomain>::new(TimeDomainId::from_u128(2));
    let signal = network
        .add_level_input(level, DiagnosticMeta::default())
        .unwrap();
    let instance = ModuleInstanceKey::from_u128(90);
    let added = network
        .instantiate(&module, instance, DiagnosticMeta::default())
        .unwrap()
        .bind_level(module_input, signal)
        .unwrap()
        .finish()
        .unwrap();
    network
        .add_pulse_output(
            output,
            added.pulse_output(module_output).unwrap(),
            DiagnosticMeta::default(),
        )
        .unwrap();
    let compiled = network
        .finish()
        .require_artifact()
        .unwrap()
        .compile()
        .require_artifact()
        .unwrap();
    let mut machine = compiled.spawn(policy(10_000));
    let initial = compiled
        .input_snapshot()
        .set(level, LogicLevel::High)
        .and_then(mossignal::InputSnapshotBuilder::finish)
        .unwrap();
    let result = machine
        .apply(Transaction::initialize(
            Time::from_ticks(0),
            machine.revision(),
            initial,
        ))
        .unwrap();
    assert_pulse(result.output_events(), output, true);
    let inspection = machine.inspect_module(instance).unwrap();
    let edge = inspection
        .nodes()
        .iter()
        .find(|node| node.node().node() == NodeKey::from_u128(82))
        .unwrap();
    assert_eq!(
        edge.edge_observation(),
        Some(EdgeObservation::Established(LogicLevel::High))
    );
    assert!(edge.edge_observation_cause().is_some());
}

#[test]
fn edge_observation_and_provenance_are_failure_atomic() {
    let level = ExternalInputKey::<Level>::from_u128(10);
    let left = mossignal::key::ExternalInputKey::<Pulse>::from_u128(11);
    let right = mossignal::key::ExternalInputKey::<Pulse>::from_u128(12);
    let node = NodeKey::from_u128(20);
    let mut builder = NetworkBuilder::<TestDomain>::new(TimeDomainId::from_u128(2));
    let level_signal = builder
        .add_level_input(level, DiagnosticMeta::default())
        .unwrap();
    let left_signal = builder
        .add_pulse_input(left, DiagnosticMeta::default())
        .unwrap();
    let right_signal = builder
        .add_pulse_input(right, DiagnosticMeta::default())
        .unwrap();
    let edge = builder
        .add_rising_edge(
            node,
            level_signal,
            EdgeConfig::new(EdgeInitialization::Baseline),
            DiagnosticMeta::default(),
        )
        .unwrap()
        .into_outputs();
    let _overflow = builder.merge([left_signal, right_signal]).unwrap();
    builder
        .pulse_output("edge", edge)
        .unwrap_or_else(|failure| panic!("edge output must author: {failure:?}"));
    let compiled = builder
        .finish()
        .require_artifact()
        .unwrap()
        .compile()
        .require_artifact()
        .unwrap();
    let mut machine = compiled.spawn(policy(10_000));
    let initial = compiled
        .input_snapshot()
        .set(level, LogicLevel::Low)
        .and_then(mossignal::InputSnapshotBuilder::finish)
        .unwrap();
    machine
        .apply(Transaction::initialize(
            Time::from_ticks(0),
            machine.revision(),
            initial,
        ))
        .unwrap();
    let before = machine.inspect_edge_detector(node).unwrap();
    let failing = compiled
        .input_delta()
        .set(level, LogicLevel::High)
        .and_then(|builder| builder.pulse(left, PulseCount::new(u64::MAX)))
        .and_then(|builder| builder.pulse(right, PulseCount::ONE))
        .and_then(mossignal::InputDeltaBuilder::finish)
        .unwrap();
    assert!(
        machine
            .apply(Transaction::advance(
                Time::from_ticks(1),
                machine.revision(),
                failing,
            ))
            .is_err()
    );
    let after = machine.inspect_edge_detector(node).unwrap();
    assert_eq!(after.at(), before.at());
    assert_eq!(after.committed(), before.committed());
    assert_eq!(after.observation_cause(), before.observation_cause());

    let zero_operations = RuntimePolicy::builder()
        .max_internal_reactions(100)
        .max_evaluated_operations(0)
        .max_pending_events(100)
        .max_events_created_per_transaction(100)
        .max_required_provenance_growth(10_000)
        .build()
        .unwrap();
    let budget_snapshot = compiled
        .input_snapshot()
        .set(level, LogicLevel::High)
        .and_then(mossignal::InputSnapshotBuilder::finish)
        .unwrap();
    let mut budgeted = compiled.spawn(zero_operations);
    let budget_before = (budgeted.status(), budgeted.revision(), budgeted.now());
    let failure = budgeted
        .apply(Transaction::initialize(
            Time::from_ticks(0),
            budgeted.revision(),
            budget_snapshot,
        ))
        .expect_err("zero operation budget must reject edge evaluation");
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
        budget_before
    );
    assert_eq!(
        budgeted.inspect_edge_detector(node),
        Err(EdgeDetectorInspectionFailure::NotInitialized)
    );
}
