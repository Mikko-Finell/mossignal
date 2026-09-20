use mossignal::authored::{
    ConnectionDef, ExternalInputDef, ExternalOutputDef, InputPortRole, NodeDef, NodeKind,
    NodePorts, UncheckedNetwork,
};
use mossignal::key::{
    ConnectionKey, ExternalInputKey, ExternalOutputKey, InPortKey, NetworkKey, NodeKey, OutPortKey,
    SignalSourceKey,
};
use mossignal::metadata::DiagnosticMeta;
use mossignal::signal::{Level, LogicLevel, Pulse, PulseCount};
use mossignal::time::{NonZeroSpan, Time};
use mossignal::{
    CauseInspection, CauseRef, InertialDelayConfig, NetworkBuilder, NodeSubject, OutputEvent,
    ProvenanceView, PulseDelayConfig, RuntimeFailureEvidence, RuntimePolicy, RuntimePolicyLimit,
    Schedule, TimeDomainId, Transaction, TransportDelayConfig,
};

#[derive(Debug, PartialEq)]
enum TestDomain {}

fn policy(values: [u64; 5]) -> RuntimePolicy {
    RuntimePolicy::builder()
        .max_internal_reactions(values[0])
        .max_evaluated_operations(values[1])
        .max_pending_events(values[2])
        .max_events_created_per_transaction(values[3])
        .max_required_provenance_growth(values[4])
        .build()
        .unwrap_or_else(|failure| panic!("complete temporal policy must build: {failure}"))
}

fn generous_policy() -> RuntimePolicy {
    policy([1_000, 100_000, 1_000, 10_000, 100_000])
}

struct DelayFixture {
    compiled: mossignal::CompiledNetwork<TestDomain>,
    input: ExternalInputKey<Pulse>,
    output: ExternalOutputKey<Pulse>,
    node: NodeKey,
}

fn delay_fixture(delay_ticks: u64) -> DelayFixture {
    let mut builder =
        NetworkBuilder::with_key(NetworkKey::from_u128(1), TimeDomainId::from_u128(2));
    let input = ExternalInputKey::from_u128(10);
    let signal = builder
        .add_pulse_input(input, DiagnosticMeta::default())
        .unwrap_or_else(|failure| panic!("pulse input must author: {failure:?}"));
    let node = NodeKey::from_u128(20);
    let delayed = builder
        .add_pulse_delay_with_ports(
            node,
            InPortKey::from_u128(30),
            OutPortKey::from_u128(31),
            signal,
            PulseDelayConfig::new(
                NonZeroSpan::from_ticks(delay_ticks)
                    .unwrap_or_else(|failure| panic!("delay must be positive: {failure}")),
            ),
            DiagnosticMeta::default(),
        )
        .unwrap_or_else(|failure| panic!("PulseDelay must author: {failure:?}"))
        .into_outputs();
    let output = ExternalOutputKey::from_u128(40);
    builder
        .add_pulse_output(output, delayed, DiagnosticMeta::default())
        .unwrap_or_else(|failure| panic!("pulse output must author: {failure:?}"));
    let compiled = builder
        .finish()
        .require_artifact()
        .unwrap_or_else(|failure| panic!("PulseDelay must validate: {failure:?}"))
        .compile()
        .require_artifact()
        .unwrap_or_else(|failure| panic!("PulseDelay must compile: {failure:?}"));
    DelayFixture {
        compiled,
        input,
        output,
        node,
    }
}

struct TransportFixture {
    compiled: mossignal::CompiledNetwork<TestDomain>,
    input: ExternalInputKey<Level>,
    output: ExternalOutputKey<Level>,
    node: NodeKey,
}

fn transport_fixture(initial: LogicLevel, delay_ticks: u64) -> TransportFixture {
    let mut builder =
        NetworkBuilder::with_key(NetworkKey::from_u128(101), TimeDomainId::from_u128(2));
    let input = ExternalInputKey::from_u128(110);
    let signal = builder
        .add_level_input(input, DiagnosticMeta::default())
        .unwrap_or_else(|failure| panic!("level input must author: {failure:?}"));
    let node = NodeKey::from_u128(120);
    let delayed = builder
        .add_transport_delay_with_ports(
            node,
            InPortKey::from_u128(130),
            OutPortKey::from_u128(131),
            signal,
            TransportDelayConfig::new(
                NonZeroSpan::from_ticks(delay_ticks)
                    .unwrap_or_else(|failure| panic!("delay must be positive: {failure}")),
                initial,
            ),
            DiagnosticMeta::default(),
        )
        .unwrap_or_else(|failure| panic!("TransportDelay must author: {failure:?}"))
        .into_outputs();
    let output = ExternalOutputKey::from_u128(140);
    builder
        .add_level_output(output, delayed, DiagnosticMeta::default())
        .unwrap_or_else(|failure| panic!("level output must author: {failure:?}"));
    let compiled = builder
        .finish()
        .require_artifact()
        .unwrap_or_else(|failure| panic!("TransportDelay must validate: {failure:?}"))
        .compile()
        .require_artifact()
        .unwrap_or_else(|failure| panic!("TransportDelay must compile: {failure:?}"));
    TransportFixture {
        compiled,
        input,
        output,
        node,
    }
}

struct InertialFixture {
    compiled: mossignal::CompiledNetwork<TestDomain>,
    input: ExternalInputKey<Level>,
    output: ExternalOutputKey<Level>,
    node: NodeKey,
}

fn inertial_fixture(initial: LogicLevel, delay_ticks: u64) -> InertialFixture {
    let mut builder =
        NetworkBuilder::with_key(NetworkKey::from_u128(301), TimeDomainId::from_u128(2));
    let input = ExternalInputKey::from_u128(310);
    let signal = builder
        .add_level_input(input, DiagnosticMeta::default())
        .unwrap_or_else(|failure| panic!("level input must author: {failure:?}"));
    let node = NodeKey::from_u128(320);
    let delayed = builder
        .add_inertial_delay_with_ports(
            node,
            InPortKey::from_u128(330),
            OutPortKey::from_u128(331),
            signal,
            InertialDelayConfig::new(
                NonZeroSpan::from_ticks(delay_ticks)
                    .unwrap_or_else(|failure| panic!("delay must be positive: {failure}")),
                initial,
            ),
            DiagnosticMeta::default(),
        )
        .unwrap_or_else(|failure| panic!("InertialDelay must author: {failure:?}"))
        .into_outputs();
    let output = ExternalOutputKey::from_u128(340);
    builder
        .add_level_output(output, delayed, DiagnosticMeta::default())
        .unwrap_or_else(|failure| panic!("level output must author: {failure:?}"));
    let compiled = builder
        .finish()
        .require_artifact()
        .unwrap_or_else(|failure| panic!("InertialDelay must validate: {failure:?}"))
        .compile()
        .require_artifact()
        .unwrap_or_else(|failure| panic!("InertialDelay must compile: {failure:?}"));
    InertialFixture {
        compiled,
        input,
        output,
        node,
    }
}

fn dynamic_inertial_fixture(initial: LogicLevel, delay_ticks: u64) -> InertialFixture {
    let input = ExternalInputKey::<Level>::from_u128(310);
    let node = NodeKey::from_u128(320);
    let input_port = InPortKey::<Level>::from_u128(330);
    let output_port = OutPortKey::<Level>::from_u128(331);
    let output = ExternalOutputKey::<Level>::from_u128(340);
    let network = UncheckedNetwork::new(
        NetworkKey::from_u128(301),
        TimeDomainId::from_u128(2),
        DiagnosticMeta::default(),
        vec![NodeDef::new(
            node,
            NodeKind::inertial_delay(
                NonZeroSpan::from_ticks(delay_ticks)
                    .unwrap_or_else(|failure| panic!("delay must be positive: {failure}")),
                initial,
            ),
            NodePorts::with_input_roles(
                vec![input_port.into()],
                vec![InputPortRole::InertialDelay],
                vec![output_port.into()],
            ),
            DiagnosticMeta::default(),
        )],
        vec![ExternalInputDef::new(
            input.into(),
            DiagnosticMeta::default(),
        )],
        vec![ExternalOutputDef::new(
            output.into(),
            SignalSourceKey::NodeOutput(output_port).into(),
            DiagnosticMeta::default(),
        )],
        vec![ConnectionDef::new(
            ConnectionKey::from_u128(0),
            input.into(),
            input_port.into(),
            DiagnosticMeta::default(),
        )],
    );
    let compiled = network
        .validate()
        .require_artifact()
        .unwrap_or_else(|failure| panic!("dynamic InertialDelay must validate: {failure:?}"))
        .compile()
        .require_artifact()
        .unwrap_or_else(|failure| panic!("dynamic InertialDelay must compile: {failure:?}"));
    InertialFixture {
        compiled,
        input,
        output,
        node,
    }
}

fn transport_chain_fixture(first_delay: u64, second_delay: u64) -> TransportFixture {
    let mut builder =
        NetworkBuilder::with_key(NetworkKey::from_u128(201), TimeDomainId::from_u128(2));
    let input = ExternalInputKey::from_u128(210);
    let signal = builder
        .add_level_input(input, DiagnosticMeta::default())
        .unwrap();
    let first = builder
        .add_transport_delay(
            NodeKey::from_u128(220),
            signal,
            TransportDelayConfig::new(
                NonZeroSpan::from_ticks(first_delay).unwrap(),
                LogicLevel::Low,
            ),
            DiagnosticMeta::default(),
        )
        .unwrap()
        .into_outputs();
    let node = NodeKey::from_u128(221);
    let second = builder
        .add_transport_delay(
            node,
            first,
            TransportDelayConfig::new(
                NonZeroSpan::from_ticks(second_delay).unwrap(),
                LogicLevel::Low,
            ),
            DiagnosticMeta::default(),
        )
        .unwrap()
        .into_outputs();
    let output = ExternalOutputKey::from_u128(240);
    builder
        .add_level_output(output, second, DiagnosticMeta::default())
        .unwrap();
    let compiled = builder
        .finish()
        .require_artifact()
        .unwrap()
        .compile()
        .require_artifact()
        .unwrap();
    TransportFixture {
        compiled,
        input,
        output,
        node,
    }
}

fn equal_deadline_fixture(reverse: bool) -> mossignal::CompiledNetwork<TestDomain> {
    let external = ExternalInputKey::<Pulse>::from_u128(10);
    let nodes = [(20, 30, 40, 50, 60), (21, 31, 41, 51, 61)];
    let mut node_defs = nodes
        .iter()
        .map(|(node, input, output, _, _)| {
            NodeDef::new(
                NodeKey::from_u128(*node),
                NodeKind::pulse_delay(NonZeroSpan::from_ticks(5).unwrap()),
                NodePorts::with_input_roles(
                    vec![InPortKey::<Pulse>::from_u128(*input).into()],
                    vec![InputPortRole::PulseDelay],
                    vec![OutPortKey::<Pulse>::from_u128(*output).into()],
                ),
                DiagnosticMeta::default(),
            )
        })
        .collect::<Vec<_>>();
    let mut connections = nodes
        .iter()
        .map(|(_, input, _, connection, _)| {
            ConnectionDef::new(
                ConnectionKey::from_u128(*connection),
                external.into(),
                InPortKey::<Pulse>::from_u128(*input).into(),
                DiagnosticMeta::default(),
            )
        })
        .collect::<Vec<_>>();
    let mut outputs = nodes
        .iter()
        .map(|(_, _, output, _, external_output)| {
            ExternalOutputDef::new(
                ExternalOutputKey::<Pulse>::from_u128(*external_output).into(),
                SignalSourceKey::NodeOutput(OutPortKey::<Pulse>::from_u128(*output)).into(),
                DiagnosticMeta::default(),
            )
        })
        .collect::<Vec<_>>();
    if reverse {
        node_defs.reverse();
        connections.reverse();
        outputs.reverse();
    }
    UncheckedNetwork::new(
        NetworkKey::from_u128(1),
        TimeDomainId::from_u128(2),
        DiagnosticMeta::default(),
        node_defs,
        vec![ExternalInputDef::new(
            external.into(),
            DiagnosticMeta::default(),
        )],
        outputs,
        connections,
    )
    .validate()
    .require_artifact()
    .unwrap()
    .compile()
    .require_artifact()
    .unwrap()
}

fn pulse_events<D>(events: &[OutputEvent<D>]) -> Vec<(u128, u64, u64)> {
    events
        .iter()
        .map(|event| match event {
            OutputEvent::Pulsed {
                output, count, at, ..
            } => (output.as_u128(), count.get(), at.ticks()),
            _ => panic!("temporal fixture must publish only pulse events"),
        })
        .collect()
}

fn cause_reaches_pending<D>(
    provenance: &ProvenanceView<D>,
    cause: CauseRef,
    expected_serial: u64,
) -> bool {
    match provenance
        .inspect(cause)
        .unwrap_or_else(|failure| panic!("causal supporter must resolve: {failure}"))
    {
        CauseInspection::PendingPulseDelay {
            event, supporters, ..
        } => {
            event.value() == expected_serial
                || supporters
                    .iter()
                    .any(|supporter| cause_reaches_pending(provenance, *supporter, expected_serial))
        }
        CauseInspection::Derived { supporters, .. }
        | CauseInspection::PulseDerived { supporters, .. } => supporters
            .iter()
            .any(|supporter| cause_reaches_pending(provenance, *supporter, expected_serial)),
        CauseInspection::InitializationTransaction { .. }
        | CauseInspection::ReadyTransaction { .. }
        | CauseInspection::ExternalObservation { .. }
        | CauseInspection::ExternalPulseObservation { .. } => false,
        _ => false,
    }
}

#[test]
fn pulse_delay_schedules_exact_future_work_and_fires_once() {
    let DelayFixture {
        compiled,
        input,
        output,
        node,
    } = delay_fixture(5);
    let mut machine = compiled.spawn(generous_policy());
    assert_eq!(
        machine.schedule(),
        Err(mossignal::ScheduleFailure::NotInitialized)
    );
    assert!(matches!(
        machine.inspect_pulse_delay(node),
        Err(mossignal::PulseDelayInspectionFailure::NotInitialized)
    ));

    let snapshot = compiled
        .input_snapshot()
        .pulse(input, PulseCount::new(3))
        .and_then(mossignal::InputSnapshotBuilder::finish)
        .unwrap_or_else(|failure| panic!("initial pulse must build: {failure}"));
    let initialized = machine
        .apply(Transaction::initialize(
            Time::from_ticks(10),
            machine.revision(),
            snapshot,
        ))
        .unwrap_or_else(|failure| panic!("initialization must apply: {failure}"));
    assert!(initialized.output_events().is_empty());
    assert_eq!(
        initialized.schedule(),
        Schedule::WakeAt(Time::from_ticks(15))
    );
    assert_eq!(machine.next_deadline(), Ok(Some(Time::from_ticks(15))));

    let inspection = machine
        .inspect_pulse_delay(node)
        .unwrap_or_else(|failure| panic!("pending delay must inspect: {failure:?}"));
    assert_eq!(inspection.node(), node);
    assert_eq!(inspection.delay().ticks(), 5);
    assert_eq!(inspection.at(), Time::from_ticks(10));
    assert_eq!(inspection.next_deadline(), Some(Time::from_ticks(15)));
    assert_eq!(inspection.pending().len(), 1);
    let pending = &inspection.pending()[0];
    assert_eq!(pending.event().value(), 0);
    assert_eq!(pending.node(), node);
    assert_eq!(pending.origin(), Time::from_ticks(10));
    assert_eq!(pending.deadline(), Time::from_ticks(15));
    assert_eq!(pending.count(), PulseCount::new(3));
    let CauseInspection::PendingPulseDelay {
        event,
        owner,
        origin,
        deadline,
        count,
        revision,
        supporters,
    } = initialized
        .provenance()
        .inspect(pending.cause())
        .unwrap_or_else(|failure| panic!("pending cause must resolve: {failure}"))
    else {
        panic!("pending cause must preserve the temporal obligation")
    };
    assert_eq!(event, pending.event());
    assert_eq!(owner, &NodeSubject::Node(node));
    assert_eq!(origin, pending.origin());
    assert_eq!(deadline, pending.deadline());
    assert_eq!(count, pending.count());
    assert_eq!(revision, pending.revision());
    assert!(!supporters.is_empty());

    let result = machine
        .apply(Transaction::advance(
            Time::from_ticks(15),
            machine.revision(),
            compiled
                .input_delta()
                .finish()
                .unwrap_or_else(|failure| panic!("empty delta must build: {failure}")),
        ))
        .unwrap_or_else(|failure| panic!("due deadline must apply: {failure}"));
    assert_eq!(
        pulse_events(result.output_events()),
        vec![(output.as_u128(), 3, 15)]
    );
    assert_eq!(result.schedule(), Schedule::Dormant);
    assert_eq!(machine.schedule(), Ok(Schedule::Dormant));
    assert!(
        machine
            .inspect_pulse_delay(node)
            .unwrap()
            .pending()
            .is_empty()
    );
    for event in result.output_events() {
        let OutputEvent::Pulsed { cause, .. } = event else {
            unreachable!()
        };
        assert!(result.provenance().inspect(*cause).is_ok());
        assert!(cause_reaches_pending(
            result.provenance(),
            *cause,
            pending.event().value()
        ));
    }
}

#[test]
fn transport_delay_preserves_reversals_and_due_work_at_target_time() {
    let TransportFixture {
        compiled,
        input,
        output,
        node,
    } = transport_fixture(LogicLevel::Low, 5);
    let mut machine = compiled.spawn(generous_policy());
    let initialized = machine
        .apply(Transaction::initialize(
            Time::from_ticks(10),
            machine.revision(),
            compiled
                .input_snapshot()
                .set(input, LogicLevel::High)
                .unwrap()
                .finish()
                .unwrap(),
        ))
        .unwrap();
    assert!(matches!(
        initialized.output_events(),
        [OutputEvent::LevelEstablished {
            output: actual,
            value: LogicLevel::Low,
            at,
            ..
        }] if *actual == output && *at == Time::from_ticks(10)
    ));
    assert_eq!(machine.next_deadline(), Ok(Some(Time::from_ticks(15))));
    let inspection = machine.inspect_transport_delay(node).unwrap();
    assert_eq!(inspection.initial(), LogicLevel::Low);
    assert_eq!(inspection.remembered_input(), LogicLevel::High);
    assert_eq!(inspection.committed(), LogicLevel::Low);
    assert_eq!(inspection.input(), LogicLevel::High);
    assert_eq!(inspection.pending().len(), 1);
    assert_eq!(inspection.pending()[0].target(), LogicLevel::High);
    assert_eq!(inspection.pending()[0].origin(), Time::from_ticks(10));

    let reversal = machine
        .apply(Transaction::advance(
            Time::from_ticks(12),
            machine.revision(),
            compiled
                .input_delta()
                .set(input, LogicLevel::Low)
                .unwrap()
                .finish()
                .unwrap(),
        ))
        .unwrap();
    assert!(reversal.output_events().is_empty());

    let jumped = machine
        .apply(Transaction::advance(
            Time::from_ticks(17),
            machine.revision(),
            compiled
                .input_delta()
                .set(input, LogicLevel::High)
                .unwrap()
                .finish()
                .unwrap(),
        ))
        .unwrap();
    let changes = jumped
        .output_events()
        .iter()
        .map(|event| match event {
            OutputEvent::LevelChanged {
                output: actual,
                to,
                at,
                ..
            } => (*actual, *to, *at),
            _ => panic!("jumped TransportDelay must publish only level changes"),
        })
        .collect::<Vec<_>>();
    assert_eq!(
        changes,
        vec![
            (output, LogicLevel::High, Time::from_ticks(15)),
            (output, LogicLevel::Low, Time::from_ticks(17)),
        ]
    );
    let final_inspection = machine.inspect_transport_delay(node).unwrap();
    assert_eq!(final_inspection.remembered_input(), LogicLevel::High);
    assert_eq!(final_inspection.committed(), LogicLevel::Low);
    assert_eq!(final_inspection.input(), LogicLevel::High);
    assert_eq!(final_inspection.pending().len(), 1);
    assert_eq!(
        final_inspection.pending()[0].deadline(),
        Time::from_ticks(22)
    );
}

#[test]
fn inertial_delay_cancels_replaces_and_matures_one_candidate() {
    let InertialFixture {
        compiled,
        input,
        output,
        node,
    } = inertial_fixture(LogicLevel::Low, 5);
    let mut machine = compiled.spawn(generous_policy());
    let initialized = machine
        .apply(Transaction::initialize(
            Time::from_ticks(10),
            machine.revision(),
            compiled
                .input_snapshot()
                .set(input, LogicLevel::High)
                .unwrap()
                .finish()
                .unwrap(),
        ))
        .unwrap();
    assert!(matches!(
        initialized.output_events(),
        [OutputEvent::LevelEstablished {
            output: actual,
            value: LogicLevel::Low,
            at,
            ..
        }] if *actual == output && *at == Time::from_ticks(10)
    ));
    let first = machine.inspect_inertial_delay(node).unwrap();
    assert_eq!(first.remembered_input(), LogicLevel::High);
    assert_eq!(first.committed(), LogicLevel::Low);
    assert_eq!(
        first.pending().map(|pending| pending.target()),
        Some(LogicLevel::High)
    );
    assert_eq!(first.next_deadline(), Some(Time::from_ticks(15)));
    assert_eq!(first.pending().unwrap().event().value(), 0);

    let canceled = machine
        .apply(Transaction::advance(
            Time::from_ticks(12),
            machine.revision(),
            compiled
                .input_delta()
                .set(input, LogicLevel::Low)
                .unwrap()
                .finish()
                .unwrap(),
        ))
        .unwrap();
    assert!(canceled.output_events().is_empty());
    assert_eq!(machine.schedule(), Ok(Schedule::Dormant));
    let canceled_inspection = machine.inspect_inertial_delay(node).unwrap();
    assert!(canceled_inspection.pending().is_none());
    let cancellation = canceled_inspection
        .last_cancellation()
        .unwrap_or_else(|| panic!("cancellation must remain inspectable"));
    assert!(matches!(
        canceled_inspection.provenance().inspect(cancellation).unwrap(),
        CauseInspection::PendingInertialDelay { event, target: LogicLevel::High, .. }
            if event.value() == 0
    ));

    let replaced = machine
        .apply(Transaction::advance(
            Time::from_ticks(13),
            machine.revision(),
            compiled
                .input_delta()
                .set(input, LogicLevel::High)
                .unwrap()
                .finish()
                .unwrap(),
        ))
        .unwrap();
    assert!(replaced.output_events().is_empty());
    let replacement = machine.inspect_inertial_delay(node).unwrap();
    assert_eq!(
        replacement.pending().map(|pending| pending.target()),
        Some(LogicLevel::High)
    );
    assert_eq!(replacement.next_deadline(), Some(Time::from_ticks(18)));
    assert_eq!(replacement.pending().unwrap().event().value(), 1);

    let matured = machine
        .apply(Transaction::advance(
            Time::from_ticks(18),
            machine.revision(),
            compiled.input_delta().finish().unwrap(),
        ))
        .unwrap();
    assert!(matches!(
        matured.output_events(),
        [OutputEvent::LevelChanged {
            output: actual,
            from: LogicLevel::Low,
            to: LogicLevel::High,
            at,
            ..
        }] if *actual == output && *at == Time::from_ticks(18)
    ));
    let final_inspection = machine.inspect_inertial_delay(node).unwrap();
    assert_eq!(final_inspection.committed(), LogicLevel::High);
    assert_eq!(final_inspection.remembered_input(), LogicLevel::High);
    assert!(final_inspection.pending().is_none());
}

#[test]
fn inertial_delay_exact_deadline_matures_before_opposite_replacement() {
    let InertialFixture {
        compiled,
        input,
        output,
        node,
    } = inertial_fixture(LogicLevel::Low, 5);
    let mut machine = compiled.spawn(generous_policy());
    machine
        .apply(Transaction::initialize(
            Time::from_ticks(0),
            machine.revision(),
            compiled
                .input_snapshot()
                .set(input, LogicLevel::High)
                .unwrap()
                .finish()
                .unwrap(),
        ))
        .unwrap();
    let exact = machine
        .apply(Transaction::advance(
            Time::from_ticks(5),
            machine.revision(),
            compiled
                .input_delta()
                .set(input, LogicLevel::Low)
                .unwrap()
                .finish()
                .unwrap(),
        ))
        .unwrap();
    assert!(matches!(
        exact.output_events(),
        [OutputEvent::LevelChanged {
            output: actual,
            from: LogicLevel::Low,
            to: LogicLevel::High,
            at,
            ..
        }] if *actual == output && *at == Time::from_ticks(5)
    ));
    let pending = machine.inspect_inertial_delay(node).unwrap();
    assert_eq!(pending.committed(), LogicLevel::High);
    assert_eq!(pending.remembered_input(), LogicLevel::Low);
    assert_eq!(
        pending.pending().map(|event| event.target()),
        Some(LogicLevel::Low)
    );
    assert_eq!(pending.next_deadline(), Some(Time::from_ticks(10)));

    let after = machine
        .apply(Transaction::advance(
            Time::from_ticks(10),
            machine.revision(),
            compiled.input_delta().finish().unwrap(),
        ))
        .unwrap();
    assert!(matches!(
        after.output_events(),
        [OutputEvent::LevelChanged {
            output: actual,
            from: LogicLevel::High,
            to: LogicLevel::Low,
            at,
            ..
        }] if *actual == output && *at == Time::from_ticks(10)
    ));
    assert!(
        machine
            .inspect_inertial_delay(node)
            .unwrap()
            .pending()
            .is_none()
    );
}

#[test]
fn inertial_delay_identity_tracks_semantic_configuration() {
    let first = inertial_fixture(LogicLevel::Low, 5).compiled;
    let second = inertial_fixture(LogicLevel::Low, 6).compiled;
    let third = inertial_fixture(LogicLevel::High, 5).compiled;
    assert_ne!(first.fingerprint(), second.fingerprint());
    assert_ne!(first.fingerprint(), third.fingerprint());
}

#[test]
fn inertial_delay_dynamic_authoring_matches_typed_behavior() {
    let typed = inertial_fixture(LogicLevel::Low, 5);
    let dynamic = dynamic_inertial_fixture(LogicLevel::Low, 5);
    assert_eq!(typed.compiled.fingerprint(), dynamic.compiled.fingerprint());

    let mut machine = dynamic.compiled.spawn(generous_policy());
    machine
        .apply(Transaction::initialize(
            Time::from_ticks(0),
            machine.revision(),
            dynamic
                .compiled
                .input_snapshot()
                .set(dynamic.input, LogicLevel::High)
                .unwrap()
                .finish()
                .unwrap(),
        ))
        .unwrap();
    let inspection = machine.inspect_inertial_delay(dynamic.node).unwrap();
    assert_eq!(inspection.committed(), LogicLevel::Low);
    assert_eq!(
        inspection.pending().map(|pending| pending.target()),
        Some(LogicLevel::High)
    );
    assert_eq!(inspection.next_deadline(), Some(Time::from_ticks(5)));
}

#[test]
fn transport_delay_initial_state_is_shared_by_input_memory_and_output() {
    for initial in [LogicLevel::Low, LogicLevel::High] {
        for first in [LogicLevel::Low, LogicLevel::High] {
            let fixture = transport_fixture(initial, 7);
            let mut machine = fixture.compiled.spawn(generous_policy());
            let result = machine
                .apply(Transaction::initialize(
                    Time::from_ticks(11),
                    machine.revision(),
                    fixture
                        .compiled
                        .input_snapshot()
                        .set(fixture.input, first)
                        .unwrap()
                        .finish()
                        .unwrap(),
                ))
                .unwrap();
            assert!(matches!(result.output_events(),
                [OutputEvent::LevelEstablished { value, at, .. }]
                    if *value == initial && *at == Time::from_ticks(11)
            ));
            let observed = machine.inspect_transport_delay(fixture.node).unwrap();
            assert_eq!(observed.initial(), initial);
            assert_eq!(observed.output(), initial);
            assert_eq!(observed.remembered_input(), first);
            assert_eq!(observed.pending().len(), usize::from(first != initial));
            if first != initial {
                assert_eq!(observed.pending()[0].target(), first);
                assert_eq!(observed.pending()[0].deadline(), Time::from_ticks(18));
            }
        }
    }
}

#[test]
fn transport_delay_matches_reference_recurrence_across_input_histories() {
    for initial in [LogicLevel::Low, LogicLevel::High] {
        for delay in [1_u64, 3, 7] {
            let fixture = transport_fixture(initial, delay);
            for seed in 0_u64..8 {
                let mut machine = fixture.compiled.spawn(generous_policy());
                let mut time = seed + 2;
                let mut remembered = if seed % 2 == 0 {
                    initial
                } else {
                    initial.invert()
                };
                let mut output = initial;
                let mut pending = Vec::<(u64, u64, LogicLevel)>::new();
                if remembered != initial {
                    pending.push((time, time + delay, remembered));
                }
                machine
                    .apply(Transaction::initialize(
                        Time::from_ticks(time),
                        machine.revision(),
                        fixture
                            .compiled
                            .input_snapshot()
                            .set(fixture.input, remembered)
                            .unwrap()
                            .finish()
                            .unwrap(),
                    ))
                    .unwrap();
                for step in 0_u64..20 {
                    time += 1 + ((seed * 3 + step * 5) % 4);
                    let mut expected_events = Vec::new();
                    pending.sort_by_key(|(_, deadline, _)| *deadline);
                    while pending
                        .first()
                        .is_some_and(|(_, deadline, _)| *deadline <= time)
                    {
                        let (_, deadline, target) = pending.remove(0);
                        if target != output {
                            expected_events.push((deadline, target));
                        }
                        output = target;
                    }
                    let input = if (seed + step * 7) % 3 == 0 {
                        remembered.invert()
                    } else {
                        remembered
                    };
                    if input != remembered {
                        pending.push((time, time + delay, input));
                    }
                    remembered = input;
                    let result = machine
                        .apply(Transaction::advance(
                            Time::from_ticks(time),
                            machine.revision(),
                            fixture
                                .compiled
                                .input_delta()
                                .set(fixture.input, input)
                                .unwrap()
                                .finish()
                                .unwrap(),
                        ))
                        .unwrap();
                    let actual_events = result
                        .output_events()
                        .iter()
                        .map(|event| match event {
                            OutputEvent::LevelChanged { at, to, .. } => (at.ticks(), *to),
                            _ => panic!("reference history may publish only LevelChanged events"),
                        })
                        .collect::<Vec<_>>();
                    assert_eq!(
                        actual_events, expected_events,
                        "initial={initial:?}, delay={delay}, seed={seed}, step={step}"
                    );
                    let observed = machine.inspect_transport_delay(fixture.node).unwrap();
                    assert_eq!(observed.output(), output);
                    assert_eq!(observed.remembered_input(), remembered);
                    let mut expected_pending = pending.clone();
                    expected_pending.sort_by_key(|(_, deadline, _)| *deadline);
                    let actual_pending = observed
                        .pending()
                        .iter()
                        .map(|event| {
                            (
                                event.origin().ticks(),
                                event.deadline().ticks(),
                                event.target(),
                            )
                        })
                        .collect::<Vec<_>>();
                    assert_eq!(actual_pending, expected_pending);
                    assert_eq!(
                        machine.next_deadline().unwrap().map(|next| next.ticks()),
                        expected_pending.first().map(|(_, deadline, _)| *deadline)
                    );
                }
            }
        }
    }
}

#[test]
fn transport_delay_latest_output_transition_survives_later_reactions() {
    let fixture = transport_fixture(LogicLevel::Low, 5);
    let mut machine = fixture.compiled.spawn(generous_policy());
    machine
        .apply(Transaction::initialize(
            Time::from_ticks(0),
            machine.revision(),
            fixture
                .compiled
                .input_snapshot()
                .set(fixture.input, LogicLevel::High)
                .unwrap()
                .finish()
                .unwrap(),
        ))
        .unwrap();
    let initial = machine.inspect_transport_delay(fixture.node).unwrap();
    assert_eq!(initial.latest_transition(), initial.current_support());

    machine
        .apply(Transaction::advance(
            Time::from_ticks(5),
            machine.revision(),
            fixture.compiled.input_delta().finish().unwrap(),
        ))
        .unwrap();
    let matured = machine.inspect_transport_delay(fixture.node).unwrap();
    assert_eq!(matured.output(), LogicLevel::High);
    assert_eq!(matured.latest_transition(), matured.current_support());
    machine
        .apply(Transaction::advance(
            Time::from_ticks(6),
            machine.revision(),
            fixture.compiled.input_delta().finish().unwrap(),
        ))
        .unwrap();
    let retained = machine.inspect_transport_delay(fixture.node).unwrap();
    assert_ne!(retained.current_support(), retained.latest_transition());
    assert!(matches!(
        retained.provenance().inspect(retained.current_support()).unwrap(),
        CauseInspection::Derived { supporters, .. }
            if supporters.contains(&retained.latest_transition())
    ));
    assert!(matches!(
        retained.provenance().inspect(retained.latest_transition()).unwrap(),
        CauseInspection::Derived { supporters, .. }
            if supporters.iter().any(|cause| matches!(
                retained.provenance().inspect(*cause).unwrap(),
                CauseInspection::PendingTransportDelay { .. }
            ))
    ));
}

#[test]
fn mixed_temporal_events_allocate_keys_in_stable_owner_order() {
    let mut builder = NetworkBuilder::<TestDomain>::new(TimeDomainId::from_u128(2));
    let level_input = ExternalInputKey::<Level>::from_u128(1);
    let pulse_input = ExternalInputKey::<Pulse>::from_u128(2);
    let level = builder
        .add_level_input(level_input, DiagnosticMeta::default())
        .unwrap();
    let pulse = builder
        .add_pulse_input(pulse_input, DiagnosticMeta::default())
        .unwrap();
    let pulse_node = NodeKey::from_u128(200);
    let pulse_output = builder
        .add_pulse_delay(
            pulse_node,
            pulse,
            PulseDelayConfig::new(NonZeroSpan::from_ticks(5).unwrap()),
            DiagnosticMeta::default(),
        )
        .unwrap()
        .into_outputs();
    let transport_node = NodeKey::from_u128(100);
    let level_output = builder
        .add_transport_delay(
            transport_node,
            level,
            TransportDelayConfig::new(NonZeroSpan::from_ticks(5).unwrap(), LogicLevel::Low),
            DiagnosticMeta::default(),
        )
        .unwrap()
        .into_outputs();
    builder
        .add_pulse_output(
            ExternalOutputKey::from_u128(3),
            pulse_output,
            DiagnosticMeta::default(),
        )
        .unwrap();
    builder
        .add_level_output(
            ExternalOutputKey::from_u128(4),
            level_output,
            DiagnosticMeta::default(),
        )
        .unwrap();
    let compiled = builder
        .finish()
        .require_artifact()
        .unwrap()
        .compile()
        .require_artifact()
        .unwrap();
    let mut constrained = compiled.spawn(policy([100, 10_000, 1, 100, 100_000]));
    let failure = constrained
        .apply(Transaction::initialize(
            Time::from_ticks(0),
            constrained.revision(),
            compiled
                .input_snapshot()
                .set(level_input, LogicLevel::High)
                .unwrap()
                .pulse(pulse_input, PulseCount::ONE)
                .unwrap()
                .finish()
                .unwrap(),
        ))
        .expect_err("the shared pending-event budget counts both temporal kinds");
    assert!(matches!(
        failure.evidence(),
        RuntimeFailureEvidence::BudgetExceeded {
            budget: RuntimePolicyLimit::MaxPendingEvents,
            consumed: 2,
            ..
        }
    ));
    assert!(!constrained.is_initialized());
    let mut machine = compiled.spawn(generous_policy());
    machine
        .apply(Transaction::initialize(
            Time::from_ticks(0),
            machine.revision(),
            compiled
                .input_snapshot()
                .set(level_input, LogicLevel::High)
                .unwrap()
                .pulse(pulse_input, PulseCount::ONE)
                .unwrap()
                .finish()
                .unwrap(),
        ))
        .unwrap();
    assert_eq!(
        machine
            .inspect_transport_delay(transport_node)
            .unwrap()
            .pending()[0]
            .event()
            .value(),
        0
    );
    assert_eq!(
        machine.inspect_pulse_delay(pulse_node).unwrap().pending()[0]
            .event()
            .value(),
        1
    );
}

#[test]
fn transport_chain_direct_jump_matches_internal_deadline_steps() {
    let fixture = transport_chain_fixture(3, 4);
    let mut jumped = fixture.compiled.spawn(generous_policy());
    let mut stepped = fixture.compiled.spawn(generous_policy());
    for machine in [&mut jumped, &mut stepped] {
        machine
            .apply(Transaction::initialize(
                Time::from_ticks(2),
                machine.revision(),
                fixture
                    .compiled
                    .input_snapshot()
                    .set(fixture.input, LogicLevel::High)
                    .unwrap()
                    .finish()
                    .unwrap(),
            ))
            .unwrap();
    }
    let direct = jumped
        .apply(Transaction::advance(
            Time::from_ticks(10),
            jumped.revision(),
            fixture.compiled.input_delta().finish().unwrap(),
        ))
        .unwrap();
    let at_first = stepped
        .apply(Transaction::advance(
            Time::from_ticks(5),
            stepped.revision(),
            fixture.compiled.input_delta().finish().unwrap(),
        ))
        .unwrap();
    assert!(at_first.output_events().is_empty());
    assert_eq!(
        stepped
            .inspect_transport_delay(fixture.node)
            .unwrap()
            .pending()[0]
            .deadline(),
        Time::from_ticks(9)
    );
    let at_second = stepped
        .apply(Transaction::advance(
            Time::from_ticks(9),
            stepped.revision(),
            fixture.compiled.input_delta().finish().unwrap(),
        ))
        .unwrap();
    let at_target = stepped
        .apply(Transaction::advance(
            Time::from_ticks(10),
            stepped.revision(),
            fixture.compiled.input_delta().finish().unwrap(),
        ))
        .unwrap();
    assert!(at_target.output_events().is_empty());
    for events in [direct.output_events(), at_second.output_events()] {
        assert!(matches!(events,
            [OutputEvent::LevelChanged { output, to: LogicLevel::High, at, .. }]
                if *output == fixture.output && *at == Time::from_ticks(9)
        ));
    }
    for machine in [&jumped, &stepped] {
        let observed = machine.inspect_transport_delay(fixture.node).unwrap();
        assert_eq!(observed.output(), LogicLevel::High);
        assert_eq!(observed.remembered_input(), LogicLevel::High);
        assert!(observed.pending().is_empty());
        assert_eq!(machine.schedule(), Ok(Schedule::Dormant));
        assert!(matches!(
            observed.provenance().inspect(observed.current_support()).unwrap(),
            CauseInspection::Derived { supporters, .. }
                if supporters.contains(&observed.latest_transition())
        ));
    }
}

#[test]
fn transport_chain_late_overflow_rolls_back_earlier_internal_deadline() {
    let fixture = transport_chain_fixture(2, 5);
    let mut machine = fixture.compiled.spawn(generous_policy());
    let start = Time::from_ticks(u64::MAX - 6);
    machine
        .apply(Transaction::initialize(
            start,
            machine.revision(),
            fixture
                .compiled
                .input_snapshot()
                .set(fixture.input, LogicLevel::High)
                .unwrap()
                .finish()
                .unwrap(),
        ))
        .unwrap();
    let before = machine
        .inspect_transport_delay(NodeKey::from_u128(220))
        .unwrap();
    let pending_key = before.pending()[0].event();
    let failure = machine
        .apply(Transaction::advance(
            Time::from_ticks(u64::MAX - 3),
            machine.revision(),
            fixture.compiled.input_delta().finish().unwrap(),
        ))
        .expect_err("second TransportDelay scheduling overflows after the first internal deadline");
    assert!(matches!(
        failure.evidence(),
        RuntimeFailureEvidence::TransportTimeOverflow { .. }
    ));
    assert_eq!(machine.now(), Some(start));
    assert_eq!(
        machine.next_deadline(),
        Ok(Some(Time::from_ticks(u64::MAX - 4)))
    );
    let after = machine
        .inspect_transport_delay(NodeKey::from_u128(220))
        .unwrap();
    assert_eq!(after.output(), LogicLevel::Low);
    assert_eq!(after.pending()[0].event(), pending_key);
    assert_eq!(
        machine
            .inspect_transport_delay(fixture.node)
            .unwrap()
            .output(),
        LogicLevel::Low
    );
}

#[test]
fn transport_delay_overflow_and_event_budget_are_atomic() {
    let TransportFixture {
        compiled,
        input,
        node,
        ..
    } = transport_fixture(LogicLevel::Low, 2);
    let mut overflow = compiled.spawn(generous_policy());
    let failure = overflow
        .apply(Transaction::initialize(
            Time::from_ticks(u64::MAX - 1),
            overflow.revision(),
            compiled
                .input_snapshot()
                .set(input, LogicLevel::High)
                .unwrap()
                .finish()
                .unwrap(),
        ))
        .expect_err("TransportDelay deadline overflow must reject initialization");
    assert_eq!(
        failure.evidence(),
        &RuntimeFailureEvidence::TransportTimeOverflow {
            node: NodeSubject::Node(node),
            origin_ticks: u64::MAX - 1,
            delay_ticks: 2,
        }
    );
    assert_eq!(overflow.now(), None);
    assert_eq!(
        overflow.next_deadline(),
        Err(mossignal::ScheduleFailure::NotInitialized)
    );

    let mut budget = compiled.spawn(policy([100, 10_000, 0, 0, 10_000]));
    let failure = budget
        .apply(Transaction::initialize(
            Time::from_ticks(0),
            budget.revision(),
            compiled
                .input_snapshot()
                .set(input, LogicLevel::High)
                .unwrap()
                .finish()
                .unwrap(),
        ))
        .expect_err("TransportDelay event budget must reject initialization");
    assert!(matches!(
        failure.evidence(),
        RuntimeFailureEvidence::BudgetExceeded {
            budget: RuntimePolicyLimit::MaxEventsCreatedPerTransaction,
            consumed: 1,
            ..
        }
    ));
    assert!(!budget.is_initialized());
}

#[test]
fn pulse_delay_reproduces_zero_one_two_and_larger_initial_counts() {
    let DelayFixture {
        compiled,
        input,
        output,
        ..
    } = delay_fixture(7);
    for count in [0_u64, 1, 2, 9] {
        let mut machine = compiled.spawn(generous_policy());
        let mut snapshot = compiled.input_snapshot();
        if count != 0 {
            snapshot = snapshot.pulse(input, PulseCount::new(count)).unwrap();
        }
        let initialized = machine
            .apply(Transaction::initialize(
                Time::from_ticks(13),
                machine.revision(),
                snapshot.finish().unwrap(),
            ))
            .unwrap();
        assert!(initialized.output_events().is_empty());
        if count == 0 {
            assert_eq!(machine.schedule(), Ok(Schedule::Dormant));
        } else {
            assert_eq!(
                machine.schedule(),
                Ok(Schedule::WakeAt(Time::from_ticks(20)))
            );
            let result = machine
                .apply(Transaction::advance(
                    Time::from_ticks(20),
                    machine.revision(),
                    compiled.input_delta().finish().unwrap(),
                ))
                .unwrap();
            assert_eq!(
                pulse_events(result.output_events()),
                vec![(output.as_u128(), count, 20)]
            );
        }
    }
}

#[test]
fn chained_delays_schedule_from_internal_deadlines_and_retain_event_identity() {
    let mut builder = NetworkBuilder::<TestDomain>::new(TimeDomainId::from_u128(90));
    let (input, pulse) = builder.pulse_input("in");
    let first_node = NodeKey::from_u128(100);
    let first = builder
        .add_pulse_delay_with_ports(
            first_node,
            InPortKey::from_u128(101),
            OutPortKey::from_u128(102),
            pulse,
            PulseDelayConfig::new(NonZeroSpan::from_ticks(2).unwrap()),
            DiagnosticMeta::default(),
        )
        .unwrap()
        .into_outputs();
    let second_node = NodeKey::from_u128(200);
    let second = builder
        .add_pulse_delay_with_ports(
            second_node,
            InPortKey::from_u128(201),
            OutPortKey::from_u128(202),
            first,
            PulseDelayConfig::new(NonZeroSpan::from_ticks(3).unwrap()),
            DiagnosticMeta::default(),
        )
        .unwrap()
        .into_outputs();
    let output = builder.pulse_output("out", second).unwrap();
    let compiled = builder
        .finish()
        .require_artifact()
        .unwrap()
        .compile()
        .require_artifact()
        .unwrap();
    let mut machine = compiled.spawn(generous_policy());
    machine
        .apply(Transaction::initialize(
            Time::from_ticks(5),
            machine.revision(),
            compiled
                .input_snapshot()
                .pulse(input, PulseCount::new(4))
                .and_then(mossignal::InputSnapshotBuilder::finish)
                .unwrap(),
        ))
        .unwrap();
    let first_event = machine.inspect_pulse_delay(first_node).unwrap().pending()[0].event();
    assert_eq!(first_event.value(), 0);
    assert_eq!(
        machine.schedule(),
        Ok(Schedule::WakeAt(Time::from_ticks(7)))
    );

    let result = machine
        .apply(Transaction::advance(
            Time::from_ticks(12),
            machine.revision(),
            compiled.input_delta().finish().unwrap(),
        ))
        .unwrap();
    assert_eq!(
        pulse_events(result.output_events()),
        vec![(output.as_u128(), 4, 10)]
    );
    assert_eq!(machine.schedule(), Ok(Schedule::Dormant));
    let OutputEvent::Pulsed { cause, .. } = &result.output_events()[0] else {
        unreachable!()
    };
    assert!(cause_reaches_pending(result.provenance(), *cause, 1));
}

#[test]
fn equal_deadline_fanout_is_insertion_invariant_and_allocates_by_stable_node() {
    let forward = equal_deadline_fixture(false);
    let reverse = equal_deadline_fixture(true);
    assert_eq!(forward.fingerprint(), reverse.fingerprint());

    let mut outcomes = Vec::new();
    for compiled in [&forward, &reverse] {
        let mut machine = compiled.spawn(generous_policy());
        let initialized = machine
            .apply(Transaction::initialize(
                Time::from_ticks(0),
                machine.revision(),
                compiled
                    .input_snapshot()
                    .pulse(ExternalInputKey::from_u128(10), PulseCount::new(2))
                    .and_then(mossignal::InputSnapshotBuilder::finish)
                    .unwrap(),
            ))
            .unwrap();
        assert!(initialized.output_events().is_empty());
        assert_eq!(
            machine
                .inspect_pulse_delay(NodeKey::from_u128(20))
                .unwrap()
                .pending()[0]
                .event()
                .value(),
            0
        );
        assert_eq!(
            machine
                .inspect_pulse_delay(NodeKey::from_u128(21))
                .unwrap()
                .pending()[0]
                .event()
                .value(),
            1
        );
        let result = machine
            .apply(Transaction::advance(
                Time::from_ticks(5),
                machine.revision(),
                compiled.input_delta().finish().unwrap(),
            ))
            .unwrap();
        outcomes.push(pulse_events(result.output_events()));
    }
    assert_eq!(outcomes[0], outcomes[1]);
    assert_eq!(outcomes[0], vec![(60, 2, 5), (61, 2, 5)]);
}

#[test]
fn direct_jump_retains_every_internal_deadline_event_and_provenance() {
    let DelayFixture {
        compiled,
        input,
        output,
        ..
    } = delay_fixture(2);
    let initialize = || {
        let mut machine = compiled.spawn(generous_policy());
        let snapshot = compiled
            .input_snapshot()
            .pulse(input, PulseCount::ONE)
            .and_then(mossignal::InputSnapshotBuilder::finish)
            .unwrap();
        machine
            .apply(Transaction::initialize(
                Time::from_ticks(0),
                machine.revision(),
                snapshot,
            ))
            .unwrap();
        let second = compiled
            .input_delta()
            .pulse(input, PulseCount::new(2))
            .and_then(mossignal::InputDeltaBuilder::finish)
            .unwrap();
        machine
            .apply(Transaction::advance(
                Time::from_ticks(1),
                machine.revision(),
                second,
            ))
            .unwrap();
        machine
    };

    let mut direct = initialize();
    let direct_result = direct
        .apply(Transaction::advance(
            Time::from_ticks(5),
            direct.revision(),
            compiled.input_delta().finish().unwrap(),
        ))
        .unwrap_or_else(|failure| panic!("direct jump must apply: {failure}"));
    assert_eq!(
        pulse_events(direct_result.output_events()),
        vec![(output.as_u128(), 1, 2), (output.as_u128(), 2, 3)]
    );
    for event in direct_result.output_events() {
        let OutputEvent::Pulsed { cause, .. } = event else {
            unreachable!()
        };
        assert!(direct_result.provenance().inspect(*cause).is_ok());
    }
    assert_eq!(direct.now(), Some(Time::from_ticks(5)));
    assert_eq!(direct.schedule(), Ok(Schedule::Dormant));

    let mut stepwise = initialize();
    let first = stepwise
        .apply(Transaction::advance(
            Time::from_ticks(2),
            stepwise.revision(),
            compiled.input_delta().finish().unwrap(),
        ))
        .unwrap();
    let second = stepwise
        .apply(Transaction::advance(
            Time::from_ticks(3),
            stepwise.revision(),
            compiled.input_delta().finish().unwrap(),
        ))
        .unwrap();
    let final_result = stepwise
        .apply(Transaction::advance(
            Time::from_ticks(5),
            stepwise.revision(),
            compiled.input_delta().finish().unwrap(),
        ))
        .unwrap();
    let mut stepwise_events = pulse_events(first.output_events());
    stepwise_events.extend(pulse_events(second.output_events()));
    stepwise_events.extend(pulse_events(final_result.output_events()));
    assert_eq!(stepwise_events, pulse_events(direct_result.output_events()));
    assert_eq!(stepwise.now(), direct.now());
    assert_eq!(stepwise.schedule(), direct.schedule());
}

#[test]
fn due_group_and_new_same_time_input_remain_distinct() {
    let DelayFixture {
        compiled, input, ..
    } = delay_fixture(2);
    let mut machine = compiled.spawn(generous_policy());
    let initial = compiled
        .input_snapshot()
        .pulse(input, PulseCount::new(2))
        .and_then(mossignal::InputSnapshotBuilder::finish)
        .unwrap();
    machine
        .apply(Transaction::initialize(
            Time::from_ticks(0),
            machine.revision(),
            initial,
        ))
        .unwrap();
    let same_time = compiled
        .input_delta()
        .pulse(input, PulseCount::new(3))
        .and_then(mossignal::InputDeltaBuilder::finish)
        .unwrap();
    let due = machine
        .apply(Transaction::advance(
            Time::from_ticks(2),
            machine.revision(),
            same_time,
        ))
        .unwrap();
    assert_eq!(pulse_events(due.output_events())[0].1, 2);
    assert_eq!(due.schedule(), Schedule::WakeAt(Time::from_ticks(4)));
    let later = machine
        .apply(Transaction::advance(
            Time::from_ticks(4),
            machine.revision(),
            compiled.input_delta().finish().unwrap(),
        ))
        .unwrap();
    assert_eq!(pulse_events(later.output_events())[0].1, 3);
}

#[test]
fn upstream_pulse_operation_settles_before_future_proposal_without_immediate_output() {
    let mut builder = NetworkBuilder::<TestDomain>::new(TimeDomainId::from_u128(9));
    let (input, pulse) = builder.pulse_input("in");
    let merged = builder.merge([pulse]).unwrap();
    let delayed = builder
        .pulse_delay(
            merged,
            PulseDelayConfig::new(NonZeroSpan::from_ticks(3).unwrap()),
        )
        .unwrap();
    let output = builder.pulse_output("out", delayed).unwrap();
    let compiled = builder
        .finish()
        .require_artifact()
        .unwrap()
        .compile()
        .require_artifact()
        .unwrap();
    let mut machine = compiled.spawn(generous_policy());
    let snapshot = compiled
        .input_snapshot()
        .pulse(input, PulseCount::new(4))
        .and_then(mossignal::InputSnapshotBuilder::finish)
        .unwrap();
    let initialized = machine
        .apply(Transaction::initialize(
            Time::from_ticks(7),
            machine.revision(),
            snapshot,
        ))
        .unwrap();
    assert!(initialized.output_events().is_empty());
    assert_eq!(
        initialized.schedule(),
        Schedule::WakeAt(Time::from_ticks(10))
    );
    let due = machine
        .apply(Transaction::advance(
            Time::from_ticks(10),
            machine.revision(),
            compiled.input_delta().finish().unwrap(),
        ))
        .unwrap();
    assert_eq!(
        pulse_events(due.output_events()),
        vec![(output.as_u128(), 4, 10)]
    );
}

#[test]
fn temporal_budget_and_time_overflow_failures_preserve_calendar_and_identity() {
    let DelayFixture {
        compiled,
        input,
        node,
        ..
    } = delay_fixture(2);
    let mut overflow = compiled.spawn(generous_policy());
    let snapshot = compiled
        .input_snapshot()
        .pulse(input, PulseCount::ONE)
        .and_then(mossignal::InputSnapshotBuilder::finish)
        .unwrap();
    let failure = overflow
        .apply(Transaction::initialize(
            Time::from_ticks(u64::MAX - 1),
            overflow.revision(),
            snapshot,
        ))
        .expect_err("deadline overflow must reject initialization");
    assert_eq!(
        failure.evidence(),
        &RuntimeFailureEvidence::TimeOverflow {
            node: NodeSubject::Node(node),
            origin_ticks: u64::MAX - 1,
            delay_ticks: 2,
        }
    );
    assert_eq!(overflow.now(), None);
    let retry = compiled
        .input_snapshot()
        .pulse(input, PulseCount::ONE)
        .and_then(mossignal::InputSnapshotBuilder::finish)
        .unwrap();
    overflow
        .apply(Transaction::initialize(
            Time::from_ticks(0),
            overflow.revision(),
            retry,
        ))
        .unwrap();
    assert_eq!(
        overflow.inspect_pulse_delay(node).unwrap().pending()[0]
            .event()
            .value(),
        0
    );

    let constrained = policy([1, 100_000, 100, 100, 100_000]);
    let mut machine = compiled.spawn(constrained);
    let initial = compiled
        .input_snapshot()
        .pulse(input, PulseCount::ONE)
        .and_then(mossignal::InputSnapshotBuilder::finish)
        .unwrap();
    machine
        .apply(Transaction::initialize(
            Time::from_ticks(0),
            machine.revision(),
            initial,
        ))
        .unwrap();
    let before_now = machine.now();
    let before_schedule = machine.schedule();
    let before_pending = machine.inspect_pulse_delay(node).unwrap().pending()[0].event();
    let failure = machine
        .apply(Transaction::advance(
            Time::from_ticks(5),
            machine.revision(),
            compiled.input_delta().finish().unwrap(),
        ))
        .expect_err("internal deadline plus target must exceed one-reaction budget");
    assert!(matches!(
        failure.evidence(),
        RuntimeFailureEvidence::BudgetExceeded {
            budget: RuntimePolicyLimit::MaxInternalReactions,
            consumed: 2,
            ..
        }
    ));
    assert_eq!(machine.now(), before_now);
    assert_eq!(machine.schedule(), before_schedule);
    assert_eq!(
        machine.inspect_pulse_delay(node).unwrap().pending()[0].event(),
        before_pending
    );
}

#[test]
fn every_temporal_budget_accepts_exact_limit_and_rejects_one_below_atomically() {
    let DelayFixture {
        compiled,
        input,
        node,
        ..
    } = delay_fixture(2);

    for (budget, field) in [
        (RuntimePolicyLimit::MaxPendingEvents, 2_usize),
        (RuntimePolicyLimit::MaxEventsCreatedPerTransaction, 3_usize),
    ] {
        for limit in [0_u64, 1, 2] {
            let mut values = [100, 100_000, 100, 100, 100_000];
            values[field] = limit;
            let mut machine = compiled.spawn(policy(values));
            let result = machine.apply(Transaction::initialize(
                Time::from_ticks(0),
                machine.revision(),
                compiled
                    .input_snapshot()
                    .pulse(input, PulseCount::ONE)
                    .and_then(mossignal::InputSnapshotBuilder::finish)
                    .unwrap(),
            ));
            if limit == 0 {
                assert_eq!(
                    result.unwrap_err().evidence(),
                    &RuntimeFailureEvidence::BudgetExceeded {
                        budget,
                        limit,
                        consumed: 1,
                    }
                );
                assert_eq!(machine.now(), None);
                assert_eq!(
                    machine.schedule(),
                    Err(mossignal::ScheduleFailure::NotInitialized)
                );
            } else {
                result.unwrap();
                assert_eq!(
                    machine.schedule(),
                    Ok(Schedule::WakeAt(Time::from_ticks(2)))
                );
            }
        }
    }

    for (budget, consumed, limits) in [
        (RuntimePolicyLimit::MaxInternalReactions, 2_u64, [1, 2, 3]),
        (RuntimePolicyLimit::MaxEvaluatedOperations, 8_u64, [7, 8, 9]),
    ] {
        for limit in limits {
            let mut values = [100, 100_000, 100, 100, 100_000];
            match budget {
                RuntimePolicyLimit::MaxInternalReactions => values[0] = limit,
                RuntimePolicyLimit::MaxEvaluatedOperations => values[1] = limit,
                _ => unreachable!(),
            }
            let mut machine = compiled.spawn(policy(values));
            machine
                .apply(Transaction::initialize(
                    Time::from_ticks(0),
                    machine.revision(),
                    compiled
                        .input_snapshot()
                        .pulse(input, PulseCount::ONE)
                        .and_then(mossignal::InputSnapshotBuilder::finish)
                        .unwrap(),
                ))
                .unwrap();
            let before_now = machine.now();
            let before_schedule = machine.schedule();
            let before_event = machine.inspect_pulse_delay(node).unwrap().pending()[0].event();
            let result = machine.apply(Transaction::advance(
                Time::from_ticks(5),
                machine.revision(),
                compiled.input_delta().finish().unwrap(),
            ));
            if limit < consumed {
                assert_eq!(
                    result.unwrap_err().evidence(),
                    &RuntimeFailureEvidence::BudgetExceeded {
                        budget,
                        limit,
                        consumed,
                    }
                );
                assert_eq!(machine.now(), before_now);
                assert_eq!(machine.schedule(), before_schedule);
                assert_eq!(
                    machine.inspect_pulse_delay(node).unwrap().pending()[0].event(),
                    before_event
                );
            } else {
                result.unwrap();
                assert_eq!(machine.now(), Some(Time::from_ticks(5)));
            }
        }
    }

    for limit in [5_u64, 6, 7] {
        let mut machine = compiled.spawn(policy([100, 100_000, 100, 100, limit]));
        machine
            .apply(Transaction::initialize(
                Time::from_ticks(0),
                machine.revision(),
                compiled.input_snapshot().finish().unwrap(),
            ))
            .unwrap();
        let before_now = machine.now();
        let result = machine.apply(Transaction::advance(
            Time::from_ticks(1),
            machine.revision(),
            compiled
                .input_delta()
                .pulse(input, PulseCount::ONE)
                .and_then(mossignal::InputDeltaBuilder::finish)
                .unwrap(),
        ));
        if limit == 5 {
            assert_eq!(
                result.unwrap_err().evidence(),
                &RuntimeFailureEvidence::BudgetExceeded {
                    budget: RuntimePolicyLimit::MaxRequiredProvenanceGrowth,
                    limit,
                    consumed: 6,
                }
            );
            assert_eq!(machine.now(), before_now);
            assert_eq!(machine.schedule(), Ok(Schedule::Dormant));
        } else {
            result.unwrap();
            assert_eq!(
                machine.schedule(),
                Ok(Schedule::WakeAt(Time::from_ticks(3)))
            );
        }
    }
}
