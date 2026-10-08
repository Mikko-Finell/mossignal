//! Non-publishing forecast of one transaction.

use mossignal::key::{
    ExternalInputKey, ExternalOutputKey, InPortKey, NetworkKey, NodeKey, OutPortKey,
};
use mossignal::metadata::DiagnosticMeta;
use mossignal::signal::{Level, LogicLevel, PulseCount};
use mossignal::time::{NonZeroSpan, Time};
use mossignal::{
    DiagnosticEpisodeChange, ForecastBasis, ForecastState, Machine, MachineSnapshot,
    NetworkBuilder, OutputEvent, PulseDelayConfig, RuntimeFailure, RuntimePolicy, Schedule,
    ScheduleFailure, TimeDomainId, Transaction, TransactionResult,
};

#[derive(Debug, PartialEq, Eq)]
enum Domain {}

fn policy() -> RuntimePolicy {
    RuntimePolicy::builder()
        .max_internal_reactions(1_000)
        .max_evaluated_operations(100_000)
        .max_pending_events(1_000)
        .max_events_created_per_transaction(10_000)
        .max_required_provenance_growth(100_000)
        .build()
        .unwrap_or_else(|failure| panic!("policy must build: {failure}"))
}

fn not_gate() -> (
    mossignal::CompiledNetwork<Domain>,
    ExternalInputKey<Level>,
    ExternalOutputKey<Level>,
) {
    let mut builder =
        NetworkBuilder::with_key(NetworkKey::from_u128(7), TimeDomainId::from_u128(11));
    let input = ExternalInputKey::from_u128(1);
    let signal = builder
        .add_level_input(input, DiagnosticMeta::default())
        .unwrap_or_else(|failure| panic!("level input must author: {failure:?}"));
    let inverted = builder
        .add_not_with_ports(
            NodeKey::from_u128(2),
            InPortKey::from_u128(3),
            OutPortKey::from_u128(4),
            signal,
            DiagnosticMeta::default(),
        )
        .unwrap_or_else(|failure| panic!("Not must author: {failure:?}"))
        .into_outputs();
    let output = ExternalOutputKey::from_u128(5);
    builder
        .add_level_output(output, inverted, DiagnosticMeta::default())
        .unwrap_or_else(|failure| panic!("level output must author: {failure:?}"));
    let compiled = builder
        .finish()
        .require_artifact()
        .unwrap_or_else(|failure| panic!("Not must validate: {failure:?}"))
        .compile()
        .require_artifact()
        .unwrap_or_else(|failure| panic!("Not must compile: {failure:?}"));
    (compiled, input, output)
}

fn level_snapshot(
    compiled: &mossignal::CompiledNetwork<Domain>,
    input: ExternalInputKey<Level>,
    level: LogicLevel,
) -> mossignal::InputSnapshot<Domain> {
    compiled
        .input_snapshot()
        .set(input, level)
        .unwrap_or_else(|failure| panic!("level observation must bind: {failure}"))
        .finish()
        .unwrap_or_else(|failure| panic!("snapshot must finish: {failure}"))
}

fn level_delta(
    compiled: &mossignal::CompiledNetwork<Domain>,
    input: ExternalInputKey<Level>,
    level: LogicLevel,
) -> mossignal::InputDelta<Domain> {
    compiled
        .input_delta()
        .set(input, level)
        .unwrap_or_else(|failure| panic!("level delta must bind: {failure}"))
        .finish()
        .unwrap_or_else(|failure| panic!("delta must finish: {failure}"))
}

fn assert_same_view(machine: &Machine<Domain>, state: &ForecastState<Domain>) {
    assert_eq!(machine.status(), state.status());
    assert_eq!(machine.now(), state.now());
    assert_eq!(machine.revision(), state.revision());
    assert_eq!(machine.fingerprint(), state.fingerprint());
    assert_eq!(
        machine.execution_state_digest(),
        state.execution_state_digest()
    );
    assert_eq!(
        machine.observable_state_digest(),
        state.observable_state_digest()
    );
    assert_eq!(machine.runtime_policy_id(), state.runtime_policy_id());
    assert_eq!(machine.schedule(), state.schedule());
    assert_eq!(machine.next_deadline(), state.next_deadline());
    assert_eq!(machine.snapshot(), state.snapshot());
}

fn assert_unchanged(before: &MachineSnapshot<Domain>, machine: &Machine<Domain>) {
    assert_eq!(machine.snapshot(), *before);
    assert_eq!(
        machine.execution_state_digest(),
        before.execution_state_digest()
    );
    assert_eq!(
        machine.observable_state_digest(),
        before.observable_state_digest()
    );
}

fn assert_same_result(forecast: &TransactionResult<Domain>, applied: &TransactionResult<Domain>) {
    assert_eq!(forecast.requested_time(), applied.requested_time());
    assert_eq!(forecast.before_revision(), applied.before_revision());
    assert_eq!(forecast.after_revision(), applied.after_revision());
    assert_eq!(
        forecast.before_execution_digest(),
        applied.before_execution_digest()
    );
    assert_eq!(
        forecast.after_execution_digest(),
        applied.after_execution_digest()
    );
    assert_eq!(
        forecast.after_observable_digest(),
        applied.after_observable_digest()
    );
    assert_eq!(forecast.schedule(), applied.schedule());
    assert_eq!(forecast.provenance().len(), applied.provenance().len());
    assert_eq!(forecast.occurrences(), applied.occurrences());
    assert_same_events(forecast.output_events(), applied.output_events());
    assert_eq!(
        forecast.diagnostic_episode_changes().len(),
        applied.diagnostic_episode_changes().len()
    );
    for (left, right) in forecast
        .diagnostic_episode_changes()
        .iter()
        .zip(applied.diagnostic_episode_changes())
    {
        assert_same_episode(left, right);
    }
}

fn assert_same_events(left: &[OutputEvent<Domain>], right: &[OutputEvent<Domain>]) {
    assert_eq!(left.len(), right.len());
    for (left, right) in left.iter().zip(right) {
        match (left, right) {
            (
                OutputEvent::LevelEstablished {
                    output: left_output,
                    value: left_value,
                    stamp: left_at,
                    cause: left_cause,
                    revision: left_revision,
                },
                OutputEvent::LevelEstablished {
                    output: right_output,
                    value: right_value,
                    stamp: right_at,
                    cause: right_cause,
                    revision: right_revision,
                },
            ) => {
                assert_eq!(left_output, right_output);
                assert_eq!(left_value, right_value);
                assert_eq!(left_at, right_at);
                assert_eq!(left_cause, right_cause);
                assert_eq!(left_revision, right_revision);
            }
            (
                OutputEvent::LevelChanged {
                    output: left_output,
                    from: left_from,
                    to: left_to,
                    stamp: left_at,
                    cause: left_cause,
                    revision: left_revision,
                },
                OutputEvent::LevelChanged {
                    output: right_output,
                    from: right_from,
                    to: right_to,
                    stamp: right_at,
                    cause: right_cause,
                    revision: right_revision,
                },
            ) => {
                assert_eq!(left_output, right_output);
                assert_eq!(left_from, right_from);
                assert_eq!(left_to, right_to);
                assert_eq!(left_at, right_at);
                assert_eq!(left_cause, right_cause);
                assert_eq!(left_revision, right_revision);
            }
            (
                OutputEvent::Pulsed {
                    output: left_output,
                    count: left_count,
                    stamp: left_at,
                    cause: left_cause,
                    revision: left_revision,
                },
                OutputEvent::Pulsed {
                    output: right_output,
                    count: right_count,
                    stamp: right_at,
                    cause: right_cause,
                    revision: right_revision,
                },
            ) => {
                assert_eq!(left_output, right_output);
                assert_eq!(left_count, right_count);
                assert_eq!(left_at, right_at);
                assert_eq!(left_cause, right_cause);
                assert_eq!(left_revision, right_revision);
            }
            _ => panic!("forecast and apply emitted different output events"),
        }
    }
}

fn assert_same_episode(
    left: &DiagnosticEpisodeChange<Domain>,
    right: &DiagnosticEpisodeChange<Domain>,
) {
    assert_eq!(left.identity(), right.identity());
    assert_eq!(left.kind(), right.kind());
    assert_eq!(left.at(), right.at());
    assert_eq!(left.cause(), right.cause());
}

fn assert_basis(basis: &ForecastBasis<Domain>, machine: &Machine<Domain>, at: u64) {
    assert_eq!(basis.revision, machine.revision());
    assert_eq!(basis.execution_digest, machine.execution_state_digest());
    assert_eq!(basis.requested_time, Time::from_ticks(at));
    assert_eq!(basis.runtime_policy_id, machine.runtime_policy_id());
}

fn assert_same_failure(forecast: &RuntimeFailure<Domain>, applied: &RuntimeFailure<Domain>) {
    assert_eq!(forecast.evidence(), applied.evidence());
    assert_eq!(forecast.code(), applied.code());
    assert_eq!(forecast.severity(), applied.severity());
    assert_eq!(forecast.responsibility(), applied.responsibility());
}

#[test]
fn successful_forecast_matches_later_apply_and_leaves_the_machine_unchanged() {
    let (compiled, input, _output) = not_gate();
    let mut machine = compiled.spawn(policy());
    let before = machine.snapshot();

    let forecast = machine
        .forecast(Transaction::initialize(
            Time::from_ticks(0),
            machine.revision(),
            level_snapshot(&compiled, input, LogicLevel::High),
        ))
        .unwrap_or_else(|failure| panic!("initialization forecast must succeed: {failure}"));
    assert_unchanged(&before, &machine);
    assert_eq!(machine.schedule(), Err(ScheduleFailure::NotInitialized));
    assert_basis(forecast.basis(), &machine, 0);
    assert_eq!(forecast.state().schedule(), Ok(Schedule::Dormant));
    let (forecast_result, forecast_state, basis) = forecast.into_parts();
    assert_eq!(basis.requested_time, Time::from_ticks(0));

    let applied = machine
        .apply(Transaction::initialize(
            Time::from_ticks(0),
            machine.revision(),
            level_snapshot(&compiled, input, LogicLevel::High),
        ))
        .unwrap_or_else(|failure| panic!("initialization must succeed: {failure}"));
    assert_same_result(&forecast_result, &applied);
    assert_same_view(&machine, &forecast_state);

    let before_advance = machine.snapshot();
    let advance = machine
        .forecast(Transaction::advance(
            Time::from_ticks(1),
            machine.revision(),
            level_delta(&compiled, input, LogicLevel::Low),
        ))
        .unwrap_or_else(|failure| panic!("advance forecast must succeed: {failure}"));
    assert_unchanged(&before_advance, &machine);
    assert_basis(advance.basis(), &machine, 1);
    assert_ne!(
        advance.basis().execution_digest,
        advance.result().after_execution_digest()
    );
    let applied_advance = machine
        .apply(Transaction::advance(
            Time::from_ticks(1),
            machine.revision(),
            level_delta(&compiled, input, LogicLevel::Low),
        ))
        .unwrap_or_else(|failure| panic!("advance must succeed: {failure}"));
    assert_same_result(advance.result(), &applied_advance);
    assert_same_view(&machine, advance.state());
}

#[test]
fn forecast_failure_matches_apply_and_publishes_nothing() {
    let (compiled, input, _output) = not_gate();
    let mut machine = compiled.spawn(policy());
    let before = machine.snapshot();
    let forecast = machine
        .forecast(Transaction::advance(
            Time::from_ticks(1),
            machine.revision(),
            level_delta(&compiled, input, LogicLevel::Low),
        ))
        .expect_err("advance before initialization must fail");
    assert_unchanged(&before, &machine);
    let applied = machine
        .apply(Transaction::advance(
            Time::from_ticks(1),
            machine.revision(),
            level_delta(&compiled, input, LogicLevel::Low),
        ))
        .expect_err("the same advance must fail");
    assert_same_failure(&forecast, &applied);
    assert_unchanged(&before, &machine);

    machine
        .apply(Transaction::initialize(
            Time::from_ticks(1),
            machine.revision(),
            level_snapshot(&compiled, input, LogicLevel::Low),
        ))
        .unwrap_or_else(|failure| panic!("initialization must succeed: {failure}"));
    let ready = machine.snapshot();
    let forecast = machine
        .forecast(Transaction::advance(
            Time::from_ticks(0),
            machine.revision(),
            level_delta(&compiled, input, LogicLevel::High),
        ))
        .expect_err("a earlier time must fail");
    assert_unchanged(&ready, &machine);
    let applied = machine
        .apply(Transaction::advance(
            Time::from_ticks(0),
            machine.revision(),
            level_delta(&compiled, input, LogicLevel::High),
        ))
        .expect_err("the same earlier time must fail");
    assert_same_failure(&forecast, &applied);
    assert_unchanged(&ready, &machine);
}

#[test]
fn pulse_delay_forecast_keeps_the_original_calendar_unchanged() {
    let mut builder =
        NetworkBuilder::with_key(NetworkKey::from_u128(21), TimeDomainId::from_u128(22));
    let input = ExternalInputKey::from_u128(10);
    let signal = builder
        .add_pulse_input(input, DiagnosticMeta::default())
        .unwrap_or_else(|failure| panic!("pulse input must author: {failure:?}"));
    let delayed = builder
        .add_pulse_delay_with_ports(
            NodeKey::from_u128(20),
            InPortKey::from_u128(30),
            OutPortKey::from_u128(31),
            signal,
            PulseDelayConfig::new(
                NonZeroSpan::from_ticks(5)
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
    let initial = compiled
        .input_snapshot()
        .pulse(input, PulseCount::ONE)
        .unwrap_or_else(|failure| panic!("pulse observation must bind: {failure}"))
        .finish()
        .unwrap_or_else(|failure| panic!("snapshot must finish: {failure}"));
    let repeated = compiled
        .input_snapshot()
        .pulse(input, PulseCount::ONE)
        .unwrap_or_else(|failure| panic!("pulse observation must bind: {failure}"))
        .finish()
        .unwrap_or_else(|failure| panic!("snapshot must finish: {failure}"));

    let mut machine = compiled.spawn(policy());
    let before = machine.snapshot();
    let forecast = machine
        .forecast(Transaction::initialize(
            Time::from_ticks(0),
            machine.revision(),
            initial,
        ))
        .unwrap_or_else(|failure| panic!("delay forecast must succeed: {failure}"));
    assert_unchanged(&before, &machine);
    assert_eq!(machine.schedule(), Err(ScheduleFailure::NotInitialized));
    assert_eq!(
        forecast.state().schedule(),
        Ok(Schedule::WakeAt(Time::from_ticks(5)))
    );
    assert_eq!(
        forecast.state().next_deadline(),
        Ok(Some(Time::from_ticks(5)))
    );
    let node = NodeKey::from_u128(20);
    let inspected = forecast
        .state()
        .inspect_pulse_delay(node)
        .unwrap_or_else(|failure| panic!("forecast inspection must succeed: {failure:?}"));
    assert_eq!(inspected.node(), node);

    let applied = machine
        .apply(Transaction::initialize(
            Time::from_ticks(0),
            machine.revision(),
            repeated,
        ))
        .unwrap_or_else(|failure| panic!("delay initialization must succeed: {failure}"));
    assert_same_result(forecast.result(), &applied);
    assert_same_view(&machine, forecast.state());
    let applied_inspection = machine
        .inspect_pulse_delay(node)
        .unwrap_or_else(|failure| panic!("applied inspection must succeed: {failure:?}"));
    assert_eq!(inspected.node(), applied_inspection.node());
    assert_eq!(inspected.delay(), applied_inspection.delay());
    assert_eq!(inspected.revision(), applied_inspection.revision());
    assert_eq!(inspected.at(), applied_inspection.at());
    assert_eq!(
        inspected.next_deadline(),
        applied_inspection.next_deadline()
    );
    assert_eq!(
        inspected.pending().len(),
        applied_inspection.pending().len()
    );
    for (left, right) in inspected.pending().iter().zip(applied_inspection.pending()) {
        assert_eq!(left.event(), right.event());
        assert_eq!(left.deadline(), right.deadline());
        assert_eq!(left.count(), right.count());
        assert_eq!(left.cause(), right.cause());
    }
}
