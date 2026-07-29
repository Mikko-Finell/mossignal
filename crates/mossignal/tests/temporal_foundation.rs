use mossignal::authored::{
    ConnectionDef, ExternalInputDef, ExternalOutputDef, InputPortRole, NodeDef, NodeKind,
    NodePorts, UncheckedNetwork,
};
use mossignal::key::{
    ConnectionKey, ExternalInputKey, ExternalOutputKey, InPortKey, NetworkKey, NodeKey, OutPortKey,
    SignalSourceKey,
};
use mossignal::metadata::DiagnosticMeta;
use mossignal::signal::{Pulse, PulseCount};
use mossignal::time::{NonZeroSpan, Time};
use mossignal::{
    CauseInspection, CauseRef, NetworkBuilder, NodeSubject, OutputEvent, ProvenanceView,
    PulseDelayConfig, RuntimeFailureEvidence, RuntimePolicy, RuntimePolicyLimit, Schedule,
    TimeDomainId, Transaction,
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
