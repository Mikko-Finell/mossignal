use mossignal::key::{
    ExternalInputKey, ExternalOutputKey, InPortKey, NetworkKey, NodeKey, OutPortKey,
};
use mossignal::metadata::DiagnosticMeta;
use mossignal::signal::{Pulse, PulseCount};
use mossignal::time::{NonZeroSpan, Time};
use mossignal::{
    NetworkBuilder, OutputEvent, PulseDelayConfig, RuntimeFailureEvidence, RuntimePolicy,
    RuntimePolicyLimit, Schedule, TimeDomainId, Transaction,
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
    assert_eq!(pending.origin(), Time::from_ticks(10));
    assert_eq!(pending.deadline(), Time::from_ticks(15));
    assert_eq!(pending.count(), PulseCount::new(3));
    assert!(initialized.provenance().inspect(pending.cause()).is_ok());

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
    }
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
    assert_eq!(failure.evidence(), &RuntimeFailureEvidence::TimeOverflow);
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
