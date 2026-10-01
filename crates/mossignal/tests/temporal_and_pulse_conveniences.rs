use mossignal::authored::{ConnectionEndpoint, NodeKind, UncheckedModule, UncheckedNetwork};
use mossignal::diagnostics::DiagnosticCode;
use mossignal::key::NetworkKey;
use mossignal::signal::{Level, LogicLevel, Pulse, PulseCount};
use mossignal::time::{NonZeroSpan, Time};
use mossignal::{
    AuthoringFailure, InertialDelayConfig, ModuleBuilder, NetworkBuilder, NodeSubject, OutputEvent,
    RuntimeFailureEvidence, RuntimePolicy, Signal, TimeDomainId, Transaction,
};

#[derive(Debug, PartialEq)]
enum TestDomain {}

const DOMAIN: u128 = 0x50;

fn domain() -> TimeDomainId {
    TimeDomainId::from_u128(DOMAIN)
}

fn span(ticks: u64) -> NonZeroSpan<TestDomain> {
    NonZeroSpan::from_ticks(ticks)
        .unwrap_or_else(|failure| panic!("debounce delay must be positive: {failure}"))
}

fn config(initial: LogicLevel) -> InertialDelayConfig<TestDomain> {
    InertialDelayConfig::new(span(5), initial)
}

fn policy() -> RuntimePolicy {
    RuntimePolicy::builder()
        .max_internal_reactions(1_000)
        .max_evaluated_operations(100_000)
        .max_pending_events(1_000)
        .max_events_created_per_transaction(10_000)
        .max_required_provenance_growth(100_000)
        .build()
        .unwrap_or_else(|failure| panic!("complete runtime policy must build: {failure}"))
}

fn opposite(level: LogicLevel) -> LogicLevel {
    match level {
        LogicLevel::Low => LogicLevel::High,
        LogicLevel::High => LogicLevel::Low,
    }
}

#[derive(Debug, PartialEq, Eq)]
enum LevelObservation {
    Established {
        value: LogicLevel,
        at: u64,
    },
    Changed {
        from: LogicLevel,
        to: LogicLevel,
        at: u64,
    },
}

#[derive(Debug, PartialEq, Eq)]
struct InertialObservation {
    delay_ticks: u64,
    initial: LogicLevel,
    remembered: LogicLevel,
    committed: LogicLevel,
    pending_target: Option<LogicLevel>,
    deadline: Option<u64>,
}

#[test]
fn debounce_matches_direct_inertial_delay_on_both_builders() {
    for initial in [LogicLevel::Low, LogicLevel::High] {
        let convenience = debounce_network(true, initial);
        let direct = debounce_network(false, initial);
        assert_eq!(convenience, direct);
        assert!(convenience.module_instances().is_empty());
        assert_eq!(convenience.nodes().len(), 1);
        assert!(matches!(
            convenience.nodes()[0].kind(),
            NodeKind::InertialDelay(actual) if *actual == config(initial)
        ));
        assert_same_network_identity(convenience, direct);

        let convenience = debounce_module(true, initial);
        let direct = debounce_module(false, initial);
        assert_eq!(convenience, direct);
        assert!(convenience.module_instances().is_empty());
        assert_eq!(convenience.nodes().len(), 1);
        assert_same_module_identity(convenience, direct);
    }
}

#[test]
fn debounce_temporal_behavior_matches_direct_inertial_delay() {
    for initial in [LogicLevel::Low, LogicLevel::High] {
        let convenience = observe_debounce(true, initial);
        let direct = observe_debounce(false, initial);
        assert_eq!(convenience, direct);
        let (after_init, after_maturity) = convenience;
        assert_eq!(after_init.initial, initial);
        assert_eq!(after_init.delay_ticks, 5);
        assert_eq!(after_init.committed, initial);
        assert_eq!(after_init.remembered, opposite(initial));
        assert_eq!(after_init.pending_target, Some(opposite(initial)));
        assert_eq!(after_init.deadline, Some(5));
        assert_eq!(after_maturity.committed, opposite(initial));
        assert_eq!(after_maturity.pending_target, None);
        assert_eq!(after_maturity.deadline, None);
    }
}

#[test]
fn any_pulse_matches_merge_then_coalesce_for_every_listed_arity() {
    let cases = [
        PulseCase::Empty,
        PulseCase::Unary,
        PulseCase::Ordered,
        PulseCase::Duplicate,
    ];
    for case in cases {
        let convenience = any_pulse_network(true, case);
        let direct = any_pulse_network(false, case);
        assert_eq!(convenience, direct);
        assert_merge_then_coalesce(&convenience, case);
        assert_same_network_identity(convenience, direct);

        let convenience = any_pulse_module(true, case);
        let direct = any_pulse_module(false, case);
        assert_eq!(convenience, direct);
        assert!(convenience.module_instances().is_empty());
        assert_eq!(convenience.nodes().len(), 2);
        assert_same_module_identity(convenience, direct);
    }
}

#[test]
fn any_pulse_preserves_primitive_diagnostics_and_duplicate_multiplicity() {
    for case in [PulseCase::Empty, PulseCase::Unary, PulseCase::Duplicate] {
        let convenience = any_pulse_network(true, case).validate();
        let direct = any_pulse_network(false, case).validate();
        assert!(convenience.artifact().is_some());
        assert_eq!(convenience.diagnostics(), direct.diagnostics());
        let codes = convenience
            .diagnostics()
            .iter()
            .map(|diagnostic| diagnostic.problem().code())
            .collect::<Vec<_>>();
        let expected = match case {
            PulseCase::Empty => DiagnosticCode::ValidationEmptyVariadicNode,
            PulseCase::Unary => DiagnosticCode::ValidationUnaryDegenerateNode,
            PulseCase::Duplicate => DiagnosticCode::ValidationDuplicateSource,
            PulseCase::Ordered => unreachable!("ordered inputs are not an advisory case"),
        };
        assert!(codes.contains(&expected));
    }

    assert_eq!(
        duplicate_presence(true),
        duplicate_presence(false),
        "duplicate-source merge multiplicity must survive until Coalesce"
    );
    assert_eq!(duplicate_presence(true), PulseCount::ONE);
}

#[test]
fn any_pulse_merge_overflow_rolls_back_instead_of_emitting_presence() {
    let convenience = overflowing_any_pulse(true);
    let direct = overflowing_any_pulse(false);
    assert_eq!(convenience.evidence, direct.evidence);
    assert!(matches!(
        convenience.evidence,
        RuntimeFailureEvidence::PulseCountOverflow {
            node: NodeSubject::Node(_),
            ..
        }
    ));
    assert_eq!(convenience.recovered, PulseCount::ONE);
    assert_eq!(direct.recovered, PulseCount::ONE);
}

#[test]
fn foreign_signals_leave_both_builders_unchanged() {
    assert_eq!(
        network_after_rejected_foreign_signals(),
        network_without_rejected_foreign_signals()
    );
    assert_eq!(
        module_after_rejected_foreign_signals(),
        module_without_rejected_foreign_signals()
    );
}

#[derive(Clone, Copy)]
enum PulseCase {
    Empty,
    Unary,
    Ordered,
    Duplicate,
}

fn debounce_network(convenience: bool, initial: LogicLevel) -> UncheckedNetwork<TestDomain> {
    let mut builder = NetworkBuilder::with_key(NetworkKey::from_u128(1), domain());
    let signal = builder.level_input("input").1;
    let delayed = author_debounce(&mut builder, signal, initial, convenience);
    builder
        .level_output("output", delayed)
        .unwrap_or_else(|failure| panic!("debounce output must author: {failure:?}"));
    builder.into_unchecked()
}

fn debounce_module(convenience: bool, initial: LogicLevel) -> UncheckedModule<TestDomain> {
    let mut builder = ModuleBuilder::new();
    let signal = builder.level_input("input").1;
    let delayed = author_module_debounce(&mut builder, signal, initial, convenience);
    builder
        .level_output("output", delayed)
        .unwrap_or_else(|failure| panic!("module debounce output must author: {failure:?}"));
    builder.into_unchecked()
}

fn author_debounce(
    builder: &mut NetworkBuilder<TestDomain>,
    signal: Signal<Level>,
    initial: LogicLevel,
    convenience: bool,
) -> Signal<Level> {
    let authored = if convenience {
        builder.debounce(signal, config(initial))
    } else {
        builder.inertial_delay(signal, config(initial))
    };
    authored.unwrap_or_else(|failure| panic!("debounce lowering must author: {failure:?}"))
}

fn author_module_debounce(
    builder: &mut ModuleBuilder<TestDomain>,
    signal: Signal<Level>,
    initial: LogicLevel,
    convenience: bool,
) -> Signal<Level> {
    let authored = if convenience {
        builder.debounce(signal, config(initial))
    } else {
        builder.inertial_delay(signal, config(initial))
    };
    authored.unwrap_or_else(|failure| panic!("module debounce lowering must author: {failure:?}"))
}

fn observe_debounce(
    convenience: bool,
    initial: LogicLevel,
) -> (InertialObservation, InertialObservation) {
    let mut builder = NetworkBuilder::with_key(NetworkKey::from_u128(2), domain());
    let (input, signal) = builder.level_input("input");
    let delayed = author_debounce(&mut builder, signal, initial, convenience);
    let output = builder
        .level_output("output", delayed)
        .unwrap_or_else(|failure| panic!("observed debounce output must author: {failure:?}"));
    let definition = builder.into_unchecked();
    let node = definition.nodes()[0].key();
    let compiled = definition
        .validate()
        .require_artifact()
        .unwrap_or_else(|failure| panic!("debounce network must validate: {failure:?}"))
        .compile()
        .require_artifact()
        .unwrap_or_else(|failure| panic!("debounce network must compile: {failure:?}"));
    let mut machine = compiled.spawn(policy());
    let initialized = machine
        .apply(Transaction::initialize(
            Time::from_ticks(0),
            machine.revision(),
            compiled
                .input_snapshot()
                .set(input, opposite(initial))
                .unwrap_or_else(|failure| panic!("debounce snapshot must accept input: {failure}"))
                .finish()
                .unwrap_or_else(|failure| panic!("debounce snapshot must build: {failure}")),
        ))
        .unwrap_or_else(|failure| panic!("debounce initialization must succeed: {failure}"));
    assert_eq!(
        level_observations(initialized.output_events(), output),
        vec![LevelObservation::Established {
            value: initial,
            at: 0,
        }]
    );
    let after_init = inertial_observation(&machine, node);
    let matured = machine
        .apply(Transaction::advance(
            Time::from_ticks(5),
            machine.revision(),
            compiled
                .input_delta()
                .finish()
                .unwrap_or_else(|failure| panic!("empty debounce delta must build: {failure}")),
        ))
        .unwrap_or_else(|failure| panic!("debounce maturity must succeed: {failure}"));
    assert_eq!(
        level_observations(matured.output_events(), output),
        vec![LevelObservation::Changed {
            from: initial,
            to: opposite(initial),
            at: 5,
        }]
    );
    let after_maturity = inertial_observation(&machine, node);
    (after_init, after_maturity)
}

fn inertial_observation(
    machine: &mossignal::Machine<TestDomain>,
    node: mossignal::key::NodeKey,
) -> InertialObservation {
    let inspection = machine
        .inspect_inertial_delay(node)
        .unwrap_or_else(|failure| {
            panic!("debounce inspection must expose InertialDelay: {failure:?}")
        });
    InertialObservation {
        delay_ticks: inspection.delay().ticks(),
        initial: inspection.initial(),
        remembered: inspection.remembered_input(),
        committed: inspection.committed(),
        pending_target: inspection.pending().map(|pending| pending.target()),
        deadline: inspection.next_deadline().map(|time| time.ticks()),
    }
}

fn level_observations(
    events: &[OutputEvent<TestDomain>],
    output: mossignal::key::ExternalOutputKey<Level>,
) -> Vec<LevelObservation> {
    events
        .iter()
        .map(|event| match event {
            OutputEvent::LevelEstablished {
                output: actual,
                value,
                at,
                ..
            } if *actual == output => LevelObservation::Established {
                value: *value,
                at: at.ticks(),
            },
            OutputEvent::LevelChanged {
                output: actual,
                from,
                to,
                at,
                ..
            } if *actual == output => LevelObservation::Changed {
                from: *from,
                to: *to,
                at: at.ticks(),
            },
            _ => panic!("debounce must emit only its level output events"),
        })
        .collect()
}

fn any_pulse_network(convenience: bool, case: PulseCase) -> UncheckedNetwork<TestDomain> {
    let mut builder = NetworkBuilder::with_key(NetworkKey::from_u128(3), domain());
    let inputs = pulse_inputs(&mut builder, case);
    let result = author_any_pulse(&mut builder, &inputs, convenience);
    builder
        .pulse_output("output", result)
        .unwrap_or_else(|failure| panic!("any_pulse output must author: {failure:?}"));
    builder.into_unchecked()
}

fn any_pulse_module(convenience: bool, case: PulseCase) -> UncheckedModule<TestDomain> {
    let mut builder = ModuleBuilder::new();
    let inputs = module_pulse_inputs(&mut builder, case);
    let result = author_module_any_pulse(&mut builder, &inputs, convenience);
    builder
        .pulse_output("output", result)
        .unwrap_or_else(|failure| panic!("module any_pulse output must author: {failure:?}"));
    builder.into_unchecked()
}

fn pulse_inputs(builder: &mut NetworkBuilder<TestDomain>, case: PulseCase) -> Vec<Signal<Pulse>> {
    match case {
        PulseCase::Empty => Vec::new(),
        PulseCase::Unary => vec![builder.pulse_input("only").1],
        PulseCase::Ordered => ["first", "second", "third"]
            .into_iter()
            .map(|name| builder.pulse_input(name).1)
            .collect(),
        PulseCase::Duplicate => {
            let signal = builder.pulse_input("duplicated").1;
            vec![signal, signal]
        }
    }
}

fn module_pulse_inputs(
    builder: &mut ModuleBuilder<TestDomain>,
    case: PulseCase,
) -> Vec<Signal<Pulse>> {
    match case {
        PulseCase::Empty => Vec::new(),
        PulseCase::Unary => vec![builder.pulse_input("only").1],
        PulseCase::Ordered => ["first", "second", "third"]
            .into_iter()
            .map(|name| builder.pulse_input(name).1)
            .collect(),
        PulseCase::Duplicate => {
            let signal = builder.pulse_input("duplicated").1;
            vec![signal, signal]
        }
    }
}

fn author_any_pulse(
    builder: &mut NetworkBuilder<TestDomain>,
    inputs: &[Signal<Pulse>],
    convenience: bool,
) -> Signal<Pulse> {
    let authored = if convenience {
        builder.any_pulse(inputs.iter().copied())
    } else {
        direct_any_pulse(builder, inputs)
    };
    authored.unwrap_or_else(|failure| panic!("any_pulse lowering must author: {failure:?}"))
}

fn direct_any_pulse(
    builder: &mut NetworkBuilder<TestDomain>,
    inputs: &[Signal<Pulse>],
) -> Result<Signal<Pulse>, AuthoringFailure> {
    let merged = builder.merge(inputs.iter().copied())?;
    builder.coalesce(merged)
}

fn author_module_any_pulse(
    builder: &mut ModuleBuilder<TestDomain>,
    inputs: &[Signal<Pulse>],
    convenience: bool,
) -> Signal<Pulse> {
    let authored = if convenience {
        builder.any_pulse(inputs.iter().copied())
    } else {
        builder
            .merge(inputs.iter().copied())
            .and_then(|merged| builder.coalesce(merged))
    };
    authored.unwrap_or_else(|failure| panic!("module any_pulse lowering must author: {failure:?}"))
}

fn assert_merge_then_coalesce(definition: &UncheckedNetwork<TestDomain>, case: PulseCase) {
    assert!(definition.module_instances().is_empty());
    assert_eq!(definition.nodes().len(), 2);
    assert!(matches!(definition.nodes()[0].kind(), NodeKind::Merge));
    assert!(matches!(definition.nodes()[1].kind(), NodeKind::Coalesce));
    let merge_ports = definition.nodes()[0].ports().inputs();
    let expected_arity = match case {
        PulseCase::Empty => 0,
        PulseCase::Unary => 1,
        PulseCase::Ordered => 3,
        PulseCase::Duplicate => 2,
    };
    assert_eq!(merge_ports.len(), expected_arity);
    let sources = merge_ports
        .iter()
        .map(|port| {
            definition
                .connections()
                .iter()
                .find(|connection| connection.to() == ConnectionEndpoint::node_input(*port))
                .unwrap_or_else(|| panic!("every Merge port must have one connection"))
                .from()
        })
        .collect::<Vec<_>>();
    let expected_sources = definition
        .external_inputs()
        .iter()
        .map(|input| ConnectionEndpoint::external_input(input.key()))
        .collect::<Vec<_>>();
    let expected_sources = if matches!(case, PulseCase::Duplicate) {
        expected_sources.repeat(2)
    } else {
        expected_sources
    };
    assert_eq!(sources, expected_sources);
    assert_eq!(definition.nodes()[1].ports().inputs().len(), 1);
}

fn duplicate_presence(convenience: bool) -> PulseCount {
    let mut builder = NetworkBuilder::with_key(NetworkKey::from_u128(4), domain());
    let (input, signal) = builder.pulse_input("duplicated");
    let result = author_any_pulse(&mut builder, &[signal, signal], convenience);
    builder
        .pulse_output("output", result)
        .unwrap_or_else(|failure| panic!("duplicate any_pulse output must author: {failure:?}"));
    let compiled = builder
        .finish()
        .require_artifact()
        .unwrap_or_else(|failure| panic!("duplicate any_pulse must validate: {failure:?}"))
        .compile()
        .require_artifact()
        .unwrap_or_else(|failure| panic!("duplicate any_pulse must compile: {failure:?}"));
    let mut machine = compiled.spawn(policy());
    let initialized = machine
        .apply(Transaction::initialize(
            Time::from_ticks(0),
            machine.revision(),
            compiled
                .input_snapshot()
                .pulse(input, PulseCount::new(3))
                .unwrap_or_else(|failure| {
                    panic!("duplicate pulse snapshot must accept input: {failure}")
                })
                .finish()
                .unwrap_or_else(|failure| panic!("duplicate pulse snapshot must build: {failure}")),
        ))
        .unwrap_or_else(|failure| panic!("duplicate any_pulse must initialize: {failure}"));
    match initialized.output_events() {
        [OutputEvent::Pulsed { count, .. }] => *count,
        other => panic!("duplicate any_pulse must emit one pulse event, got {other:?}"),
    }
}

struct OverflowObservation {
    evidence: RuntimeFailureEvidence,
    recovered: PulseCount,
}

fn overflowing_any_pulse(convenience: bool) -> OverflowObservation {
    let mut builder = NetworkBuilder::with_key(NetworkKey::from_u128(5), domain());
    let (left, left_signal) = builder.pulse_input("left");
    let (right, right_signal) = builder.pulse_input("right");
    let result = author_any_pulse(&mut builder, &[left_signal, right_signal], convenience);
    builder
        .pulse_output("output", result)
        .unwrap_or_else(|failure| panic!("overflow any_pulse output must author: {failure:?}"));
    let definition = builder.into_unchecked();
    let merge_node = definition.nodes()[0].key();
    let compiled = definition
        .validate()
        .require_artifact()
        .unwrap_or_else(|failure| panic!("overflow any_pulse must validate: {failure:?}"))
        .compile()
        .require_artifact()
        .unwrap_or_else(|failure| panic!("overflow any_pulse must compile: {failure:?}"));
    let mut machine = compiled.spawn(policy());
    machine
        .apply(Transaction::initialize(
            Time::from_ticks(0),
            machine.revision(),
            compiled
                .input_snapshot()
                .finish()
                .unwrap_or_else(|failure| panic!("zero pulse snapshot must build: {failure}")),
        ))
        .unwrap_or_else(|failure| panic!("zero any_pulse initialization must succeed: {failure}"));
    let before_status = machine.status();
    let before_revision = machine.revision();
    let before_now = machine.now();
    let failure = machine
        .apply(Transaction::advance(
            Time::from_ticks(1),
            machine.revision(),
            compiled
                .input_delta()
                .pulse(left, PulseCount::new(u64::MAX))
                .and_then(|builder| builder.pulse(right, PulseCount::ONE))
                .and_then(|builder| builder.finish())
                .unwrap_or_else(|failure| panic!("overflow delta must build: {failure}")),
        ))
        .expect_err("Merge overflow must reject any_pulse rather than emit presence");
    assert!(matches!(
        failure.evidence(),
        RuntimeFailureEvidence::PulseCountOverflow {
            node: NodeSubject::Node(node),
            ..
        } if *node == merge_node
    ));
    assert_eq!(machine.status(), before_status);
    assert_eq!(machine.revision(), before_revision);
    assert_eq!(machine.now(), before_now);
    let recovered = machine
        .apply(Transaction::advance(
            Time::from_ticks(1),
            machine.revision(),
            compiled
                .input_delta()
                .pulse(left, PulseCount::new(2))
                .and_then(|builder| builder.pulse(right, PulseCount::new(3)))
                .and_then(|builder| builder.finish())
                .unwrap_or_else(|failure| panic!("recovery delta must build: {failure}")),
        ))
        .unwrap_or_else(|failure| panic!("representable any_pulse sum must recover: {failure}"));
    let recovered = match recovered.output_events() {
        [OutputEvent::Pulsed { count, .. }] => *count,
        other => panic!("recovered any_pulse must emit one presence pulse, got {other:?}"),
    };
    OverflowObservation {
        evidence: failure.evidence().clone(),
        recovered,
    }
}

fn network_after_rejected_foreign_signals() -> UncheckedNetwork<TestDomain> {
    let mut foreign = NetworkBuilder::<TestDomain>::new(domain());
    let foreign_level = foreign.level_input("foreign-level").1;
    let foreign_pulse = foreign.pulse_input("foreign-pulse").1;
    let mut builder = NetworkBuilder::with_key(NetworkKey::from_u128(6), domain());
    let local_level = builder.level_input("local-level").1;
    let local_pulse = builder.pulse_input("local-pulse").1;
    assert_foreign_level(builder.debounce(foreign_level, config(LogicLevel::Low)));
    assert_foreign_pulse(builder.any_pulse([local_pulse, foreign_pulse]));
    assert_foreign_pulse(builder.any_pulse([foreign_pulse, local_pulse]));
    builder
        .inertial_delay(local_level, config(LogicLevel::High))
        .unwrap_or_else(|failure| panic!("later inertial delay must author: {failure:?}"));
    builder.into_unchecked()
}

fn network_without_rejected_foreign_signals() -> UncheckedNetwork<TestDomain> {
    let mut builder = NetworkBuilder::with_key(NetworkKey::from_u128(6), domain());
    let local_level = builder.level_input("local-level").1;
    let _local_pulse = builder.pulse_input("local-pulse").1;
    builder
        .inertial_delay(local_level, config(LogicLevel::High))
        .unwrap_or_else(|failure| panic!("baseline inertial delay must author: {failure:?}"));
    builder.into_unchecked()
}

fn module_after_rejected_foreign_signals() -> UncheckedModule<TestDomain> {
    let mut foreign = ModuleBuilder::<TestDomain>::new();
    let foreign_level = foreign.level_input("foreign-level").1;
    let foreign_pulse = foreign.pulse_input("foreign-pulse").1;
    let mut builder = ModuleBuilder::new();
    let local_level = builder.level_input("local-level").1;
    let local_pulse = builder.pulse_input("local-pulse").1;
    assert_foreign_level(builder.debounce(foreign_level, config(LogicLevel::Low)));
    assert_foreign_pulse(builder.any_pulse([local_pulse, foreign_pulse]));
    assert_foreign_pulse(builder.any_pulse([foreign_pulse, local_pulse]));
    builder
        .inertial_delay(local_level, config(LogicLevel::High))
        .unwrap_or_else(|failure| panic!("later module inertial delay must author: {failure:?}"));
    builder.into_unchecked()
}

fn module_without_rejected_foreign_signals() -> UncheckedModule<TestDomain> {
    let mut builder = ModuleBuilder::new();
    let local_level = builder.level_input("local-level").1;
    let _local_pulse = builder.pulse_input("local-pulse").1;
    builder
        .inertial_delay(local_level, config(LogicLevel::High))
        .unwrap_or_else(|failure| {
            panic!("baseline module inertial delay must author: {failure:?}")
        });
    builder.into_unchecked()
}

fn assert_same_network_identity(
    convenience: UncheckedNetwork<TestDomain>,
    direct: UncheckedNetwork<TestDomain>,
) {
    let convenience = convenience.validate();
    let direct = direct.validate();
    assert_eq!(convenience.diagnostics(), direct.diagnostics());
    assert_eq!(
        convenience
            .artifact()
            .unwrap_or_else(|| panic!("convenience network must validate"))
            .fingerprint(),
        direct
            .artifact()
            .unwrap_or_else(|| panic!("direct network must validate"))
            .fingerprint()
    );
}

fn assert_same_module_identity(
    convenience: UncheckedModule<TestDomain>,
    direct: UncheckedModule<TestDomain>,
) {
    let convenience = convenience.validate();
    let direct = direct.validate();
    assert_eq!(convenience.diagnostics(), direct.diagnostics());
    assert_eq!(
        convenience
            .artifact()
            .unwrap_or_else(|| panic!("convenience module must validate"))
            .fingerprint(),
        direct
            .artifact()
            .unwrap_or_else(|| panic!("direct module must validate"))
            .fingerprint()
    );
}

fn assert_foreign_level(result: Result<Signal<Level>, AuthoringFailure>) {
    assert!(matches!(
        result,
        Err(AuthoringFailure::ForeignSignal { .. })
    ));
}

fn assert_foreign_pulse(result: Result<Signal<Pulse>, AuthoringFailure>) {
    assert!(matches!(
        result,
        Err(AuthoringFailure::ForeignSignal { .. })
    ));
}
