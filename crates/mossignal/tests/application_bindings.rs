use mossignal::diagnostics::{DiagnosticCode, Responsibility, Severity, SubjectRef};
use mossignal::key::{ExternalInputKey, ExternalOutputKey, NetworkKey};
use mossignal::metadata::DiagnosticMeta;
use mossignal::signal::{Level, LogicLevel, Pulse, PulseCount};
use mossignal::time::Time;
use mossignal::{
    BindingSet, BoundApplyFailure, BoundMachine, BoundOutputFailure, InputObservation,
    InputProjectionFailure, NetworkBuilder, ProjectedOutputEvent, RuntimePolicy, TimeDomainId,
    Transaction,
};

#[derive(Debug, PartialEq, Eq)]
enum Domain {}

#[derive(Debug, Clone, PartialEq, Eq)]
enum InputId {
    Level,
    Pulse,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum OutputId {
    Level,
    Pulse,
}

struct Fixture {
    compiled: mossignal::CompiledNetwork<Domain>,
    level_input: ExternalInputKey<Level>,
    pulse_input: ExternalInputKey<Pulse>,
    level_output: ExternalOutputKey<Level>,
    pulse_output: ExternalOutputKey<Pulse>,
}

fn fixture(network: u128, extra_level: bool) -> Fixture {
    let mut builder =
        NetworkBuilder::with_key(NetworkKey::from_u128(network), TimeDomainId::from_u128(7));
    let level_input = ExternalInputKey::from_u128(10);
    let pulse_input = ExternalInputKey::from_u128(11);
    let level = builder
        .add_level_input(level_input, DiagnosticMeta::default())
        .unwrap_or_else(|failure| panic!("level input must author: {failure:?}"));
    let pulse = builder
        .add_pulse_input(pulse_input, DiagnosticMeta::default())
        .unwrap_or_else(|failure| panic!("pulse input must author: {failure:?}"));
    if extra_level {
        builder
            .add_level_input(ExternalInputKey::from_u128(12), DiagnosticMeta::default())
            .unwrap_or_else(|failure| panic!("extra input must author: {failure:?}"));
    }
    let level_output = ExternalOutputKey::from_u128(20);
    let pulse_output = ExternalOutputKey::from_u128(21);
    builder
        .add_level_output(level_output, level, DiagnosticMeta::default())
        .unwrap_or_else(|failure| panic!("level output must author: {failure:?}"));
    builder
        .add_pulse_output(pulse_output, pulse, DiagnosticMeta::default())
        .unwrap_or_else(|failure| panic!("pulse output must author: {failure:?}"));
    let compiled = builder
        .finish()
        .require_artifact()
        .unwrap_or_else(|failure| panic!("network must validate: {failure:?}"))
        .compile()
        .require_artifact()
        .unwrap_or_else(|failure| panic!("network must compile: {failure:?}"));
    Fixture {
        compiled,
        level_input,
        pulse_input,
        level_output,
        pulse_output,
    }
}

fn policy() -> RuntimePolicy {
    RuntimePolicy::builder()
        .max_internal_reactions(100)
        .max_evaluated_operations(100)
        .max_pending_events(100)
        .max_events_created_per_transaction(100)
        .max_required_provenance_growth(100)
        .build()
        .unwrap_or_else(|failure| panic!("policy must build: {failure}"))
}

fn bindings(fixture: &Fixture, reverse: bool) -> BindingSet<InputId, OutputId> {
    let builder = BindingSet::builder(&fixture.compiled);
    let builder = if reverse {
        builder
            .bind_output(fixture.pulse_output, OutputId::Pulse)
            .and_then(|builder| builder.bind_input(fixture.pulse_input, InputId::Pulse))
            .and_then(|builder| builder.bind_output(fixture.level_output, OutputId::Level))
            .and_then(|builder| builder.bind_input(fixture.level_input, InputId::Level))
    } else {
        builder
            .bind_input(fixture.level_input, InputId::Level)
            .and_then(|builder| builder.bind_output(fixture.level_output, OutputId::Level))
            .and_then(|builder| builder.bind_input(fixture.pulse_input, InputId::Pulse))
            .and_then(|builder| builder.bind_output(fixture.pulse_output, OutputId::Pulse))
    };
    builder
        .and_then(|builder| builder.finish())
        .unwrap_or_else(|failure| panic!("complete bindings must build: {failure}"))
}

#[test]
fn bindings_are_bidirectional_and_insertion_order_independent_without_identifier_ordering() {
    let fixture = fixture(1, false);
    let forward = bindings(&fixture, false);
    let reverse = bindings(&fixture, true);

    assert_eq!(
        forward.input_endpoint(&InputId::Level),
        reverse.input_endpoint(&InputId::Level)
    );
    assert_eq!(
        forward.output_endpoint(&OutputId::Pulse),
        reverse.output_endpoint(&OutputId::Pulse)
    );
    assert_eq!(
        forward.input_identifier(fixture.pulse_input),
        Some(&InputId::Pulse)
    );
    assert_eq!(
        forward.output_identifier(fixture.level_output),
        Some(&OutputId::Level)
    );
    assert_eq!(
        forward.network_fingerprint(),
        fixture.compiled.fingerprint()
    );
    assert_eq!(
        forward.input_schema_fingerprint(),
        fixture.compiled.input_schema_fingerprint()
    );
}

#[test]
fn construction_and_compatibility_failures_use_binding_catalogue_codes() {
    let primary = fixture(1, false);
    let unknown = BindingSet::<InputId, OutputId>::builder(&primary.compiled)
        .bind_input(ExternalInputKey::<Level>::from_u128(999), InputId::Level)
        .err()
        .unwrap_or_else(|| panic!("unknown endpoint must fail"));
    assert_eq!(unknown.code(), DiagnosticCode::BindingUnknownEndpoint);
    assert_eq!(unknown.code().as_str(), "binding.unknown_endpoint");
    assert_eq!(unknown.severity(), Severity::Error);
    assert_eq!(unknown.responsibility(), Responsibility::CallerInput);
    assert!(matches!(
        unknown.problem::<Domain>().primary(),
        SubjectRef::Binding(_)
    ));

    let wrong_kind = BindingSet::<InputId, OutputId>::builder(&primary.compiled)
        .bind_input(
            ExternalInputKey::<Level>::from_u128(primary.pulse_input.as_u128()),
            InputId::Pulse,
        )
        .err()
        .unwrap_or_else(|| panic!("wrong-kind endpoint must fail"));
    assert_eq!(wrong_kind.code(), DiagnosticCode::BindingWrongSignalKind);

    let duplicate = BindingSet::<InputId, OutputId>::builder(&primary.compiled)
        .bind_input(primary.level_input, InputId::Level)
        .and_then(|builder| builder.bind_input(primary.level_input, InputId::Pulse))
        .err()
        .unwrap_or_else(|| panic!("duplicate endpoint must fail"));
    assert_eq!(duplicate.code(), DiagnosticCode::BindingDuplicateEndpoint);

    let duplicate_external = BindingSet::<InputId, OutputId>::builder(&primary.compiled)
        .bind_input(primary.level_input, InputId::Level)
        .and_then(|builder| builder.bind_input(primary.pulse_input, InputId::Level))
        .err()
        .unwrap_or_else(|| panic!("duplicate caller identifier must fail"));
    assert_eq!(
        duplicate_external.code(),
        DiagnosticCode::BindingDuplicateExternalKey
    );

    let partial = BindingSet::<InputId, OutputId>::builder(&primary.compiled)
        .bind_input(primary.level_input, InputId::Level)
        .and_then(|builder| builder.finish())
        .unwrap_or_else(|failure| panic!("partial set may finish: {failure}"));
    let missing = partial
        .input_projector(&primary.compiled)
        .err()
        .unwrap_or_else(|| panic!("complete projector must reject missing input"));
    assert_eq!(
        missing.code(),
        DiagnosticCode::BindingMissingRequiredBinding
    );

    let foreign = fixture(2, false);
    let wrong_network = bindings(&primary, false)
        .input_projector(&foreign.compiled)
        .err()
        .unwrap_or_else(|| panic!("foreign compiled network must fail"));
    assert_eq!(wrong_network.code(), DiagnosticCode::BindingWrongNetwork);
}

#[test]
fn projector_builds_the_same_canonical_snapshot_and_delta_as_structural_keys() {
    let fixture = fixture(1, false);
    let bindings = bindings(&fixture, false);
    let projected_snapshot = bindings
        .input_projector(&fixture.compiled)
        .and_then(|projector| {
            projector
                .snapshot_from([
                    InputObservation::Pulse {
                        input: InputId::Pulse,
                        count: PulseCount::new(3),
                    },
                    InputObservation::Level {
                        input: InputId::Level,
                        value: LogicLevel::High,
                    },
                ])
                .map_err(|_| panic!("caller snapshot must project"))
        })
        .unwrap_or_else(|failure| panic!("projector must build: {failure}"));
    let direct_snapshot = fixture
        .compiled
        .input_snapshot()
        .pulse(fixture.pulse_input, PulseCount::new(3))
        .and_then(|builder| builder.set(fixture.level_input, LogicLevel::High))
        .and_then(|builder| builder.finish())
        .unwrap_or_else(|failure| panic!("direct snapshot must build: {failure}"));
    assert_eq!(projected_snapshot, direct_snapshot);

    let projected_delta = bindings
        .input_projector(&fixture.compiled)
        .unwrap_or_else(|failure| panic!("projector must build: {failure}"))
        .delta_from([InputObservation::Level {
            input: InputId::Level,
            value: LogicLevel::Low,
        }])
        .unwrap_or_else(|_| panic!("caller delta must project"));
    let direct_delta = fixture
        .compiled
        .input_delta()
        .set(fixture.level_input, LogicLevel::Low)
        .and_then(|builder| builder.finish())
        .unwrap_or_else(|failure| panic!("direct delta must build: {failure}"));
    assert_eq!(projected_delta, direct_delta);

    assert!(matches!(
        bindings
            .input_projector(&fixture.compiled)
            .unwrap_or_else(|failure| panic!("projector must build: {failure}"))
            .delta_from([InputObservation::Level {
                input: InputId::Pulse,
                value: LogicLevel::High
            }]),
        Err(InputProjectionFailure::WrongSignalKind { .. })
    ));
}

#[test]
fn bound_machine_delegates_and_projects_level_and_pulse_events_losslessly() {
    let fixture = fixture(1, false);
    let mut direct = fixture.compiled.spawn(policy());
    let direct_snapshot = fixture
        .compiled
        .input_snapshot()
        .set(fixture.level_input, LogicLevel::High)
        .and_then(|builder| builder.pulse(fixture.pulse_input, PulseCount::new(2)))
        .and_then(|builder| builder.finish())
        .unwrap_or_else(|failure| panic!("direct snapshot must build: {failure}"));
    let direct_result = direct
        .apply(Transaction::initialize(
            Time::from_ticks(5),
            direct.revision(),
            direct_snapshot,
        ))
        .unwrap_or_else(|failure| panic!("direct initialization must apply: {failure}"));

    let mut bound = BoundMachine::spawn(&fixture.compiled, policy(), bindings(&fixture, false))
        .unwrap_or_else(|failure| panic!("bound machine must construct: {failure}"));
    let bound_result = bound
        .initialize(
            Time::from_ticks(5),
            [
                InputObservation::Pulse {
                    input: InputId::Pulse,
                    count: PulseCount::new(2),
                },
                InputObservation::Level {
                    input: InputId::Level,
                    value: LogicLevel::High,
                },
            ],
        )
        .unwrap_or_else(|failure| match failure {
            BoundApplyFailure::Runtime(failure) => panic!("bound runtime failed: {failure}"),
            _ => panic!("bound projection failed"),
        });

    assert_eq!(bound.machine().status(), direct.status());
    assert_eq!(bound.machine().now(), direct.now());
    assert_eq!(bound.machine().revision(), direct.revision());
    assert_eq!(
        bound_result.ordinary().requested_time(),
        direct_result.requested_time()
    );
    assert_eq!(bound_result.ordinary().schedule(), direct_result.schedule());
    assert_eq!(bound_result.projected_output_events().len(), 2);
    assert!(matches!(
        &bound_result.projected_output_events()[0],
        ProjectedOutputEvent::LevelEstablished { output: OutputId::Level, value: LogicLevel::High, at, .. }
        if *at == Time::from_ticks(5)
    ));
    assert!(matches!(
        &bound_result.projected_output_events()[1],
        ProjectedOutputEvent::Pulsed { output: OutputId::Pulse, count, at, .. }
        if *count == PulseCount::new(2) && *at == Time::from_ticks(5)
    ));
    for event in bound_result.projected_output_events() {
        let cause = match event {
            ProjectedOutputEvent::LevelEstablished { cause, .. }
            | ProjectedOutputEvent::LevelChanged { cause, .. }
            | ProjectedOutputEvent::Pulsed { cause, .. } => *cause,
        };
        assert!(bound_result.ordinary().provenance().inspect(cause).is_ok());
    }

    let direct_delta = fixture
        .compiled
        .input_delta()
        .set(fixture.level_input, LogicLevel::Low)
        .and_then(|builder| builder.finish())
        .unwrap_or_else(|failure| panic!("direct delta must build: {failure}"));
    let direct_advance = direct
        .apply(Transaction::advance(
            Time::from_ticks(6),
            direct.revision(),
            direct_delta,
        ))
        .unwrap_or_else(|failure| panic!("direct advance must apply: {failure}"));
    let bound_advance = bound
        .advance(
            Time::from_ticks(6),
            [InputObservation::Level {
                input: InputId::Level,
                value: LogicLevel::Low,
            }],
        )
        .unwrap_or_else(|_| panic!("bound advance must apply"));
    assert_eq!(bound.machine().now(), direct.now());
    assert_eq!(
        bound_advance.ordinary().schedule(),
        direct_advance.schedule()
    );
    assert!(matches!(
        bound_advance.projected_output_events(),
        [ProjectedOutputEvent::LevelChanged {
            output: OutputId::Level,
            from: LogicLevel::High,
            to: LogicLevel::Low,
            at,
            ..
        }] if *at == Time::from_ticks(6)
    ));
    assert_eq!(bound.output_level(&OutputId::Level), Ok(LogicLevel::Low));
}

#[test]
fn bound_current_output_is_level_only_and_failed_projection_is_machine_atomic() {
    let fixture = fixture(1, false);
    let mut bound = BoundMachine::spawn(&fixture.compiled, policy(), bindings(&fixture, false))
        .unwrap_or_else(|failure| panic!("bound machine must construct: {failure}"));
    assert_eq!(
        bound.output_level(&OutputId::Level),
        Err(BoundOutputFailure::NotInitialized)
    );
    assert_eq!(
        bound.output_level(&OutputId::Pulse),
        Err(BoundOutputFailure::WrongSignalKind)
    );

    let before = (
        bound.machine().status(),
        bound.machine().now(),
        bound.machine().revision(),
    );
    let result = bound.initialize(
        Time::from_ticks(0),
        [InputObservation::Level {
            input: InputId::Pulse,
            value: LogicLevel::High,
        }],
    );
    assert!(matches!(
        result,
        Err(BoundApplyFailure::Projection(
            InputProjectionFailure::WrongSignalKind { .. }
        ))
    ));
    assert_eq!(
        (
            bound.machine().status(),
            bound.machine().now(),
            bound.machine().revision()
        ),
        before
    );

    bound
        .initialize(
            Time::from_ticks(1),
            [InputObservation::Level {
                input: InputId::Level,
                value: LogicLevel::High,
            }],
        )
        .unwrap_or_else(|_| panic!("valid initialization must apply"));
    let before_runtime_failure = (
        bound.machine().status(),
        bound.machine().now(),
        bound.machine().revision(),
        bound.output_level(&OutputId::Level),
    );
    assert!(matches!(
        bound.advance(Time::from_ticks(1), Vec::<InputObservation<InputId>>::new(),),
        Err(BoundApplyFailure::Runtime(_))
    ));
    assert_eq!(
        (
            bound.machine().status(),
            bound.machine().now(),
            bound.machine().revision(),
            bound.output_level(&OutputId::Level),
        ),
        before_runtime_failure
    );
}
