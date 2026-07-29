use mossignal::diagnostics::DiagnosticCode;
use mossignal::key::{
    ExternalInputKey, ExternalOutputKey, InPortKey, NetworkKey, NodeKey, OutPortKey,
};
use mossignal::metadata::DiagnosticMeta;
use mossignal::signal::{Level, LogicLevel, Pulse, PulseCount, SignalKind};
use mossignal::time::Time;
use mossignal::{
    CauseInspection, InputBuildFailure, NetworkBuilder, NodeSubject, OutputEvent,
    ProvenanceSubject, PulsePortSubject, RuntimeFailureEvidence, RuntimePolicy, TimeDomainId,
    Transaction,
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
        .unwrap_or_else(|failure| panic!("complete test policy must build: {failure}"))
}

struct PulseFixture {
    compiled: mossignal::CompiledNetwork<TestDomain>,
    left: ExternalInputKey<Pulse>,
    right: ExternalInputKey<Pulse>,
    low_output: ExternalOutputKey<Pulse>,
    high_output: ExternalOutputKey<Pulse>,
    node: NodeKey,
    ports: [InPortKey<Pulse>; 3],
}

fn pulse_network(duplicate_first_input: bool) -> PulseFixture {
    let mut builder =
        NetworkBuilder::with_key(NetworkKey::from_u128(1), TimeDomainId::from_u128(2));
    let left_key = ExternalInputKey::from_u128(10);
    let right_key = ExternalInputKey::from_u128(11);
    let left = builder
        .add_pulse_input(left_key, DiagnosticMeta::default())
        .unwrap_or_else(|failure| panic!("left input must author: {failure:?}"));
    let right = builder
        .add_pulse_input(right_key, DiagnosticMeta::default())
        .unwrap_or_else(|failure| panic!("right input must author: {failure:?}"));
    let node = NodeKey::from_u128(20);
    let ports = [
        InPortKey::from_u128(30),
        InPortKey::from_u128(31),
        InPortKey::from_u128(32),
    ];
    let third = if duplicate_first_input { left } else { right };
    let merged = builder
        .add_merge_with_ports(
            node,
            OutPortKey::from_u128(40),
            [(ports[1], right), (ports[0], left), (ports[2], third)],
            DiagnosticMeta::default(),
        )
        .unwrap_or_else(|failure| panic!("merge must author: {failure:?}"))
        .into_outputs();
    let high_output = ExternalOutputKey::from_u128(51);
    let low_output = ExternalOutputKey::from_u128(50);
    builder
        .add_pulse_output(high_output, merged, DiagnosticMeta::default())
        .unwrap_or_else(|failure| panic!("first fanout output must author: {failure:?}"));
    builder
        .add_pulse_output(low_output, merged, DiagnosticMeta::default())
        .unwrap_or_else(|failure| panic!("second fanout output must author: {failure:?}"));
    let report = builder.finish();
    if duplicate_first_input {
        assert!(report.diagnostics().iter().any(|diagnostic| {
            diagnostic.problem().code() == DiagnosticCode::ValidationDuplicateSource
        }));
    }
    let validated = report
        .require_artifact()
        .unwrap_or_else(|failure| panic!("pulse network must validate: {failure:?}"));
    let compiled = validated
        .compile()
        .require_artifact()
        .unwrap_or_else(|failure| panic!("pulse network must compile: {failure:?}"));
    PulseFixture {
        compiled,
        left: left_key,
        right: right_key,
        low_output,
        high_output,
        node,
        ports,
    }
}

#[test]
fn merge_sums_stable_ports_preserves_duplicate_multiplicity_and_fans_out() {
    let PulseFixture {
        compiled,
        left,
        right,
        low_output,
        high_output,
        node,
        ports,
    } = pulse_network(true);
    let snapshot = compiled
        .input_snapshot()
        .pulse(left, PulseCount::new(2))
        .and_then(|builder| builder.pulse(right, PulseCount::new(3)))
        .and_then(|builder| builder.finish())
        .unwrap_or_else(|failure| panic!("pulse snapshot must build: {failure}"));
    let mut machine = compiled.spawn(policy());
    let result = machine
        .apply(Transaction::initialize(
            Time::from_ticks(0),
            machine.revision(),
            snapshot,
        ))
        .unwrap_or_else(|failure| panic!("pulse initialization must apply: {failure}"));

    let events = result.output_events();
    assert_eq!(events.len(), 2);
    let mut merge_cause = None;
    for (event, expected_output) in events.iter().zip([low_output, high_output]) {
        let OutputEvent::Pulsed {
            output,
            count,
            cause,
            ..
        } = event
        else {
            panic!("pulse fanout must emit only Pulsed events");
        };
        assert_eq!((*output, *count), (expected_output, PulseCount::new(7)));
        let CauseInspection::Derived {
            subject: ProvenanceSubject::PulseExternalOutput(subject_output),
            supporters,
        } = result
            .provenance()
            .inspect(*cause)
            .unwrap_or_else(|failure| panic!("output cause must resolve: {failure}"))
        else {
            panic!("pulse output cause must retain its derived subject");
        };
        assert_eq!(subject_output, expected_output);
        for supporter in supporters {
            if matches!(
                result.provenance().inspect(*supporter),
                Ok(CauseInspection::PulseDerived {
                    subject: ProvenanceSubject::Node(subject),
                    ..
                }) if subject == node
            ) {
                merge_cause = Some(*supporter);
            }
        }
    }

    let CauseInspection::PulseDerived {
        subject: ProvenanceSubject::Node(subject_node),
        contributions,
        result: merge_count,
        supporters,
    } = result
        .provenance()
        .inspect(merge_cause.unwrap_or_else(|| panic!("merge cause must exist")))
        .unwrap_or_else(|failure| panic!("merge cause must resolve: {failure}"))
    else {
        panic!("merge cause must retain grouped contribution evidence");
    };
    assert_eq!(subject_node, node);
    assert_eq!(merge_count, PulseCount::new(7));
    assert_eq!(
        contributions
            .iter()
            .map(|contribution| (contribution.port().clone(), contribution.count()))
            .collect::<Vec<_>>(),
        vec![
            (PulsePortSubject::Port(ports[0]), PulseCount::new(2)),
            (PulsePortSubject::Port(ports[1]), PulseCount::new(3)),
            (PulsePortSubject::Port(ports[2]), PulseCount::new(2)),
        ]
    );
    assert_eq!(
        supporters.len(),
        3,
        "transaction plus two distinct pulse roots support the merge"
    );
}

#[test]
fn omitted_and_explicit_zero_pulses_do_not_persist_or_emit() {
    let PulseFixture { compiled, left, .. } = pulse_network(true);
    let initial = compiled
        .input_snapshot()
        .pulse(left, PulseCount::ONE)
        .and_then(|builder| builder.finish())
        .unwrap_or_else(|failure| panic!("initial snapshot must build: {failure}"));
    let mut machine = compiled.spawn(policy());
    let initialized = machine
        .apply(Transaction::initialize(
            Time::from_ticks(0),
            machine.revision(),
            initial,
        ))
        .unwrap_or_else(|failure| panic!("initial pulse must apply: {failure}"));
    assert_eq!(initialized.output_events().len(), 2);

    let omitted = compiled
        .input_delta()
        .finish()
        .unwrap_or_else(|failure| panic!("omitted pulse delta must build: {failure}"));
    let omitted_result = machine
        .apply(Transaction::advance(
            Time::from_ticks(1),
            machine.revision(),
            omitted,
        ))
        .unwrap_or_else(|failure| panic!("omitted pulse must apply: {failure}"));
    assert!(omitted_result.output_events().is_empty());

    let explicit_zero = compiled
        .input_delta()
        .pulse(left, PulseCount::ZERO)
        .and_then(|builder| builder.finish())
        .unwrap_or_else(|failure| panic!("explicit zero delta must build: {failure}"));
    let zero_result = machine
        .apply(Transaction::advance(
            Time::from_ticks(2),
            machine.revision(),
            explicit_zero,
        ))
        .unwrap_or_else(|failure| panic!("zero pulse must apply: {failure}"));
    assert!(zero_result.output_events().is_empty());
}

#[test]
fn merge_overflow_rejects_the_transaction_atomically() {
    let PulseFixture {
        compiled,
        left,
        right,
        node,
        ..
    } = pulse_network(false);
    let initial = compiled
        .input_snapshot()
        .finish()
        .unwrap_or_else(|failure| panic!("empty pulse snapshot must build: {failure}"));
    let mut machine = compiled.spawn(policy());
    machine
        .apply(Transaction::initialize(
            Time::from_ticks(0),
            machine.revision(),
            initial,
        ))
        .unwrap_or_else(|failure| panic!("zero initialization must apply: {failure}"));
    let before_status = machine.status();
    let before_revision = machine.revision();
    let overflowing = compiled
        .input_delta()
        .pulse(left, PulseCount::new(u64::MAX))
        .and_then(|builder| builder.pulse(right, PulseCount::ONE))
        .and_then(|builder| builder.finish())
        .unwrap_or_else(|failure| panic!("overflowing delta must still build: {failure}"));
    let failure = machine
        .apply(Transaction::advance(
            Time::from_ticks(1),
            machine.revision(),
            overflowing,
        ))
        .expect_err("overflow must reject the transaction");
    assert_eq!(
        failure.evidence(),
        &RuntimeFailureEvidence::PulseCountOverflow {
            node: NodeSubject::Node(node),
        }
    );
    assert_eq!(machine.status(), before_status);
    assert_eq!(machine.revision(), before_revision);
}

#[test]
fn empty_merge_has_the_total_zero_law() {
    let mut builder = NetworkBuilder::<TestDomain>::new(TimeDomainId::from_u128(9));
    let merged = builder
        .merge([])
        .unwrap_or_else(|failure| panic!("empty merge must author: {failure:?}"));
    builder
        .pulse_output("out", merged)
        .unwrap_or_else(|failure| panic!("empty merge output must author: {failure:?}"));
    let report = builder.finish();
    assert!(report.diagnostics().iter().any(|diagnostic| {
        diagnostic.problem().code() == DiagnosticCode::ValidationEmptyVariadicNode
    }));
    let compiled = report
        .require_artifact()
        .unwrap_or_else(|failure| panic!("empty merge must validate: {failure:?}"))
        .compile()
        .require_artifact()
        .unwrap_or_else(|failure| panic!("empty merge must compile: {failure:?}"));
    let input = compiled
        .input_snapshot()
        .finish()
        .unwrap_or_else(|failure| panic!("empty snapshot must build: {failure}"));
    let mut machine = compiled.spawn(policy());
    let result = machine
        .apply(Transaction::initialize(
            Time::from_ticks(0),
            machine.revision(),
            input,
        ))
        .unwrap_or_else(|failure| panic!("empty merge must evaluate: {failure}"));
    assert!(result.output_events().is_empty());
}

#[test]
fn unary_merge_forwards_the_complete_count() {
    let mut builder = NetworkBuilder::<TestDomain>::new(TimeDomainId::from_u128(12));
    let (input, pulse) = builder.pulse_input("in");
    let merged = builder
        .merge([pulse])
        .unwrap_or_else(|failure| panic!("unary merge must author: {failure:?}"));
    let output = builder
        .pulse_output("out", merged)
        .unwrap_or_else(|failure| panic!("unary merge output must author: {failure:?}"));
    let report = builder.finish();
    assert!(report.diagnostics().iter().any(|diagnostic| {
        diagnostic.problem().code() == DiagnosticCode::ValidationUnaryDegenerateNode
    }));
    let compiled = report
        .require_artifact()
        .unwrap_or_else(|failure| panic!("unary merge must validate: {failure:?}"))
        .compile()
        .require_artifact()
        .unwrap_or_else(|failure| panic!("unary merge must compile: {failure:?}"));
    let snapshot = compiled
        .input_snapshot()
        .pulse(input, PulseCount::new(1_000_000))
        .and_then(|builder| builder.finish())
        .unwrap_or_else(|failure| panic!("unary pulse snapshot must build: {failure}"));
    let mut machine = compiled.spawn(policy());
    let result = machine
        .apply(Transaction::initialize(
            Time::from_ticks(0),
            machine.revision(),
            snapshot,
        ))
        .unwrap_or_else(|failure| panic!("unary merge must evaluate: {failure}"));
    assert!(matches!(
        result.output_events(),
        [OutputEvent::Pulsed {
            output: actual_output,
            count,
            ..
        }] if *actual_output == output && *count == PulseCount::new(1_000_000)
    ));
}

#[test]
fn merge_port_permutation_preserves_count() {
    let mut observations = Vec::new();
    for reverse_ports in [false, true] {
        let mut builder = NetworkBuilder::<TestDomain>::with_key(
            NetworkKey::from_u128(70),
            TimeDomainId::from_u128(71),
        );
        let first_key = ExternalInputKey::<Pulse>::from_u128(1);
        let second_key = ExternalInputKey::<Pulse>::from_u128(2);
        let first = builder
            .add_pulse_input(first_key, DiagnosticMeta::default())
            .unwrap_or_else(|failure| panic!("first input must author: {failure:?}"));
        let second = builder
            .add_pulse_input(second_key, DiagnosticMeta::default())
            .unwrap_or_else(|failure| panic!("second input must author: {failure:?}"));
        let first_port = InPortKey::<Pulse>::from_u128(10);
        let second_port = InPortKey::<Pulse>::from_u128(11);
        let inputs = if reverse_ports {
            [(second_port, second), (first_port, first)]
        } else {
            [(first_port, first), (second_port, second)]
        };
        let merged = builder
            .add_merge_with_ports(
                NodeKey::from_u128(20),
                OutPortKey::from_u128(21),
                inputs,
                DiagnosticMeta::default(),
            )
            .unwrap_or_else(|failure| panic!("merge must author: {failure:?}"))
            .into_outputs();
        let output = ExternalOutputKey::<Pulse>::from_u128(30);
        builder
            .add_pulse_output(output, merged, DiagnosticMeta::default())
            .unwrap_or_else(|failure| panic!("output must author: {failure:?}"));
        let validated = builder
            .finish()
            .require_artifact()
            .unwrap_or_else(|failure| panic!("permuted merge must validate: {failure:?}"));
        let compiled = validated
            .compile()
            .require_artifact()
            .unwrap_or_else(|failure| panic!("permuted merge must compile: {failure:?}"));
        let snapshot = compiled
            .input_snapshot()
            .pulse(first_key, PulseCount::new(2))
            .and_then(|builder| builder.pulse(second_key, PulseCount::new(5)))
            .and_then(|builder| builder.finish())
            .unwrap_or_else(|failure| panic!("permuted snapshot must build: {failure}"));
        let mut machine = compiled.spawn(policy());
        let result = machine
            .apply(Transaction::initialize(
                Time::from_ticks(0),
                machine.revision(),
                snapshot,
            ))
            .unwrap_or_else(|failure| panic!("permuted merge must apply: {failure}"));
        let [
            OutputEvent::Pulsed {
                output: actual_output,
                count,
                ..
            },
        ] = result.output_events()
        else {
            panic!("permuted merge must emit one pulse event");
        };
        observations.push((*actual_output, *count));
    }
    assert_eq!(observations[0], observations[1]);
    assert_eq!(observations[0].1, PulseCount::new(7));
}

#[test]
fn pulse_input_builders_reject_unknown_duplicate_conflicting_and_wrong_kind_keys() {
    let shared_payload = 12;
    let pulse_key = ExternalInputKey::<Pulse>::from_u128(shared_payload);
    let level_key = ExternalInputKey::<Level>::from_u128(13);
    let mut builder = NetworkBuilder::<TestDomain>::new(TimeDomainId::from_u128(10));
    builder
        .add_pulse_input(pulse_key, DiagnosticMeta::default())
        .unwrap_or_else(|failure| panic!("pulse input must author: {failure:?}"));
    builder
        .add_level_input(level_key, DiagnosticMeta::default())
        .unwrap_or_else(|failure| panic!("level input must author: {failure:?}"));
    let compiled = builder
        .finish()
        .require_artifact()
        .unwrap_or_else(|failure| panic!("mixed input network must validate: {failure:?}"))
        .compile()
        .require_artifact()
        .unwrap_or_else(|failure| panic!("mixed input network must compile: {failure:?}"));

    let duplicate = compiled
        .input_snapshot()
        .pulse(pulse_key, PulseCount::ONE)
        .and_then(|builder| builder.pulse(pulse_key, PulseCount::ONE))
        .expect_err("duplicate pulse observation must fail");
    assert_eq!(
        duplicate,
        InputBuildFailure::DuplicatePulseObservation {
            input: pulse_key,
            count: PulseCount::ONE,
        }
    );
    let conflicting = compiled
        .input_delta()
        .pulse(pulse_key, PulseCount::ONE)
        .and_then(|builder| builder.pulse(pulse_key, PulseCount::new(2)))
        .expect_err("conflicting pulse observation must fail");
    assert_eq!(
        conflicting,
        InputBuildFailure::ConflictingPulseObservation {
            input: pulse_key,
            first: PulseCount::ONE,
            second: PulseCount::new(2),
        }
    );
    let unknown_key = ExternalInputKey::<Pulse>::from_u128(99);
    assert_eq!(
        compiled
            .input_delta()
            .pulse(unknown_key, PulseCount::ONE)
            .expect_err("unknown pulse input must fail"),
        InputBuildFailure::UnknownPulseInput { input: unknown_key }
    );
    let mut level_only = NetworkBuilder::<TestDomain>::new(TimeDomainId::from_u128(11));
    level_only
        .add_level_input(
            ExternalInputKey::from_u128(shared_payload),
            DiagnosticMeta::default(),
        )
        .unwrap_or_else(|failure| panic!("level-only input must author: {failure:?}"));
    let level_only = level_only
        .finish()
        .require_artifact()
        .unwrap_or_else(|failure| panic!("level-only network must validate: {failure:?}"))
        .compile()
        .require_artifact()
        .unwrap_or_else(|failure| panic!("level-only network must compile: {failure:?}"));
    assert_eq!(
        level_only
            .input_delta()
            .pulse(pulse_key, PulseCount::ONE)
            .expect_err("using a level payload as a pulse key must fail"),
        InputBuildFailure::WrongSignalKind {
            input: pulse_key.into(),
            expected: SignalKind::Level,
            actual: SignalKind::Pulse,
        }
    );
}

#[test]
fn mixed_same_time_events_and_pulse_input_order_are_deterministic() {
    let mut builder = NetworkBuilder::<TestDomain>::with_key(
        NetworkKey::from_u128(100),
        TimeDomainId::from_u128(200),
    );
    let level_input = ExternalInputKey::<Level>::from_u128(1);
    let pulse_input = ExternalInputKey::<Pulse>::from_u128(2);
    let level = builder
        .add_level_input(level_input, DiagnosticMeta::default())
        .unwrap_or_else(|failure| panic!("level input must author: {failure:?}"));
    let pulse = builder
        .add_pulse_input(pulse_input, DiagnosticMeta::default())
        .unwrap_or_else(|failure| panic!("pulse input must author: {failure:?}"));
    let merged = builder
        .merge([pulse])
        .unwrap_or_else(|failure| panic!("unary merge must author: {failure:?}"));
    let level_output = ExternalOutputKey::<Level>::from_u128(3);
    let pulse_output = ExternalOutputKey::<Pulse>::from_u128(4);
    builder
        .add_level_output(level_output, level, DiagnosticMeta::default())
        .unwrap_or_else(|failure| panic!("level output must author: {failure:?}"));
    builder
        .add_pulse_output(pulse_output, merged, DiagnosticMeta::default())
        .unwrap_or_else(|failure| panic!("pulse output must author: {failure:?}"));
    let compiled = builder
        .finish()
        .require_artifact()
        .unwrap_or_else(|failure| panic!("mixed network must validate: {failure:?}"))
        .compile()
        .require_artifact()
        .unwrap_or_else(|failure| panic!("mixed network must compile: {failure:?}"));

    let mut observations = Vec::new();
    for reverse_input_order in [false, true] {
        let snapshot = if reverse_input_order {
            compiled
                .input_snapshot()
                .pulse(pulse_input, PulseCount::new(2))
                .and_then(|builder| builder.set(level_input, LogicLevel::Low))
        } else {
            compiled
                .input_snapshot()
                .set(level_input, LogicLevel::Low)
                .and_then(|builder| builder.pulse(pulse_input, PulseCount::new(2)))
        }
        .and_then(|builder| builder.finish())
        .unwrap_or_else(|failure| panic!("mixed snapshot must build: {failure}"));
        let mut machine = compiled.spawn(policy());
        let initialized = machine
            .apply(Transaction::initialize(
                Time::from_ticks(0),
                machine.revision(),
                snapshot,
            ))
            .unwrap_or_else(|failure| panic!("mixed initialization must apply: {failure}"));
        let [
            OutputEvent::LevelEstablished {
                output: actual_level_output,
                value,
                cause: level_cause,
                ..
            },
            OutputEvent::Pulsed {
                output: actual_pulse_output,
                count,
                cause: pulse_cause,
                ..
            },
        ] = initialized.output_events()
        else {
            panic!("same-time initialization events must be Level then Pulse");
        };
        assert_eq!(
            (*actual_level_output, *value),
            (level_output, LogicLevel::Low)
        );
        assert_eq!(
            (*actual_pulse_output, *count),
            (pulse_output, PulseCount::new(2))
        );

        let delta = if reverse_input_order {
            compiled
                .input_delta()
                .pulse(pulse_input, PulseCount::ONE)
                .and_then(|builder| builder.set(level_input, LogicLevel::High))
        } else {
            compiled
                .input_delta()
                .set(level_input, LogicLevel::High)
                .and_then(|builder| builder.pulse(pulse_input, PulseCount::ONE))
        }
        .and_then(|builder| builder.finish())
        .unwrap_or_else(|failure| panic!("mixed delta must build: {failure}"));
        let advanced = machine
            .apply(Transaction::advance(
                Time::from_ticks(1),
                machine.revision(),
                delta,
            ))
            .unwrap_or_else(|failure| panic!("mixed advance must apply: {failure}"));
        let [
            OutputEvent::LevelChanged {
                output: actual_level_output,
                from,
                to,
                cause: changed_cause,
                ..
            },
            OutputEvent::Pulsed {
                output: actual_pulse_output,
                count,
                cause: advanced_pulse_cause,
                ..
            },
        ] = advanced.output_events()
        else {
            panic!("same-time ready events must be Level then Pulse");
        };
        assert_eq!(
            (*actual_level_output, *from, *to),
            (level_output, LogicLevel::Low, LogicLevel::High)
        );
        assert_eq!(
            (*actual_pulse_output, *count),
            (pulse_output, PulseCount::ONE)
        );
        observations.push((
            (*level_cause, *pulse_cause),
            (*changed_cause, *advanced_pulse_cause),
        ));
    }
    assert_eq!(observations[0], observations[1]);
}
