use std::collections::BTreeSet;

use mossignal::authored::{
    ConnectionDef, ExternalInputDef, ExternalOutputDef, InputPortRole, NodeDef, NodeKind,
    NodePorts, PulseDelayConfig, UncheckedNetwork,
};
use mossignal::diagnostics::{
    DiagnosticCode, NodeEvidence, ProblemEvidence, Responsibility, Severity,
};
use mossignal::key::{
    ConnectionKey, ExternalInputKey, ExternalOutputKey, InPortKey, ModuleInputKey,
    ModuleInstanceKey, ModuleOutputKey, NetworkKey, NodeKey, OutPortKey, SignalSourceKey,
};
use mossignal::metadata::DiagnosticMeta;
use mossignal::signal::{Level, LogicLevel, Pulse, PulseCount};
use mossignal::time::{NonZeroSpan, Time};
use mossignal::{
    CauseInspection, ConflictPolicy, EdgeConfig, EdgeInitialization, ModuleBuilder, NetworkBuilder,
    OutputEvent, PulseSetResetConfig, RuntimeFailureEvidence, RuntimePolicy, Schedule,
    TimeDomainId, Transaction,
};

#[derive(Debug, PartialEq, Eq)]
enum TestDomain {}

fn policy() -> RuntimePolicy {
    RuntimePolicy::builder()
        .max_internal_reactions(10_000)
        .max_evaluated_operations(100_000)
        .max_pending_events(10_000)
        .max_events_created_per_transaction(10_000)
        .max_required_provenance_growth(100_000)
        .build()
        .unwrap_or_else(|failure| panic!("complete policy must build: {failure}"))
}

struct LatchFixture {
    compiled: mossignal::CompiledNetwork<TestDomain>,
    set: ExternalInputKey<Pulse>,
    reset: ExternalInputKey<Pulse>,
    output: ExternalOutputKey<Level>,
    node: NodeKey,
}

fn latch_fixture(initial: LogicLevel, conflict: ConflictPolicy) -> LatchFixture {
    let mut builder = NetworkBuilder::<TestDomain>::with_key(
        NetworkKey::from_u128(1),
        TimeDomainId::from_u128(2),
    );
    let (set, set_signal) = builder.pulse_input("set");
    let (reset, reset_signal) = builder.pulse_input("reset");
    let node = NodeKey::from_u128(100);
    let state = builder
        .add_pulse_set_reset_latch(
            node,
            set_signal,
            reset_signal,
            PulseSetResetConfig::new(initial, conflict),
            DiagnosticMeta::default(),
        )
        .unwrap_or_else(|failure| panic!("latch must author: {failure:?}"))
        .into_outputs();
    let output = builder
        .level_output("state", state)
        .unwrap_or_else(|failure| panic!("output must author: {failure:?}"));
    let compiled = builder
        .finish()
        .require_artifact()
        .unwrap_or_else(|failure| panic!("latch must validate: {failure:?}"))
        .compile()
        .require_artifact()
        .unwrap_or_else(|failure| panic!("latch must compile: {failure:?}"));
    LatchFixture {
        compiled,
        set,
        reset,
        output,
        node,
    }
}

fn snapshot(
    fixture: &LatchFixture,
    set: PulseCount,
    reset: PulseCount,
) -> mossignal::InputSnapshot<TestDomain> {
    fixture
        .compiled
        .input_snapshot()
        .pulse(fixture.set, set)
        .and_then(|builder| builder.pulse(fixture.reset, reset))
        .and_then(mossignal::InputSnapshotBuilder::finish)
        .unwrap_or_else(|failure| panic!("snapshot must build: {failure}"))
}

fn delta(
    fixture: &LatchFixture,
    set: PulseCount,
    reset: PulseCount,
) -> mossignal::InputDelta<TestDomain> {
    fixture
        .compiled
        .input_delta()
        .pulse(fixture.set, set)
        .and_then(|builder| builder.pulse(fixture.reset, reset))
        .and_then(mossignal::InputDeltaBuilder::finish)
        .unwrap_or_else(|failure| panic!("delta must build: {failure}"))
}

fn expected(
    previous: LogicLevel,
    set: bool,
    reset: bool,
    policy: ConflictPolicy,
) -> Option<LogicLevel> {
    match (set, reset) {
        (false, false) => Some(previous),
        (true, false) => Some(LogicLevel::High),
        (false, true) => Some(LogicLevel::Low),
        (true, true) => match policy {
            ConflictPolicy::SetDominant => Some(LogicLevel::High),
            ConflictPolicy::ResetDominant => Some(LogicLevel::Low),
            ConflictPolicy::RetainAndDiagnose => Some(previous),
            ConflictPolicy::RejectTransaction => None,
        },
    }
}

#[test]
fn inspection_separates_declared_configuration_from_committed_runtime_state() {
    let fixture = latch_fixture(LogicLevel::High, ConflictPolicy::ResetDominant);
    let mut machine = fixture.compiled.spawn(policy());
    let definition = machine
        .inspect_pulse_set_reset_latch_definition(fixture.node)
        .unwrap_or_else(|failure| panic!("latch definition must inspect: {failure:?}"));
    assert_eq!(definition.node(), fixture.node);
    assert_eq!(definition.initial(), LogicLevel::High);
    assert_eq!(definition.conflict(), ConflictPolicy::ResetDominant);
    assert_eq!(
        machine.inspect_pulse_set_reset_latch(fixture.node),
        Err(mossignal::PulseSetResetLatchInspectionFailure::NotInitialized)
    );

    machine
        .apply(Transaction::initialize(
            Time::from_ticks(6),
            machine.revision(),
            snapshot(&fixture, PulseCount::ZERO, PulseCount::ONE),
        ))
        .unwrap_or_else(|failure| panic!("latch must initialize: {failure}"));
    let committed = machine
        .inspect_pulse_set_reset_latch(fixture.node)
        .unwrap_or_else(|failure| panic!("committed latch must inspect: {failure:?}"));
    assert_eq!(committed.initial(), LogicLevel::High);
    assert_eq!(committed.committed(), LogicLevel::Low);
    assert_eq!(committed.at(), Time::from_ticks(6));
    assert_eq!(committed.revision(), machine.revision());
}

#[test]
fn exhausts_previous_controls_and_all_conflict_policies() {
    let policies = [
        ConflictPolicy::SetDominant,
        ConflictPolicy::ResetDominant,
        ConflictPolicy::RetainAndDiagnose,
        ConflictPolicy::RejectTransaction,
    ];
    for previous in [LogicLevel::Low, LogicLevel::High] {
        for set in [false, true] {
            for reset in [false, true] {
                for conflict in policies {
                    let fixture = latch_fixture(previous, conflict);
                    let mut machine = fixture.compiled.spawn(policy());
                    let applied = machine.apply(Transaction::initialize(
                        Time::from_ticks(7),
                        machine.revision(),
                        snapshot(
                            &fixture,
                            if set {
                                PulseCount::ONE
                            } else {
                                PulseCount::ZERO
                            },
                            if reset {
                                PulseCount::ONE
                            } else {
                                PulseCount::ZERO
                            },
                        ),
                    ));
                    match expected(previous, set, reset, conflict) {
                        Some(result) => {
                            let result_artifact = applied.unwrap_or_else(|failure| {
                                panic!("conforming case must commit: {failure}")
                            });
                            assert_eq!(machine.output_level(fixture.output), Some(result));
                            assert_eq!(
                                machine
                                    .inspect_pulse_set_reset_latch(fixture.node)
                                    .unwrap_or_else(|failure| panic!(
                                        "latch must inspect: {failure:?}"
                                    ))
                                    .committed(),
                                result
                            );
                            let occurrence_expected =
                                set && reset && conflict == ConflictPolicy::RetainAndDiagnose;
                            assert_eq!(
                                result_artifact.occurrences().len(),
                                usize::from(occurrence_expected)
                            );
                        }
                        None => {
                            let failure = applied.expect_err("rejecting conflict must fail");
                            assert_eq!(
                                failure.code(),
                                DiagnosticCode::RuntimePulseLatchConflictRejected
                            );
                            assert!(!machine.is_initialized());
                            match failure.evidence() {
                                RuntimeFailureEvidence::PulseLatchConflict {
                                    node,
                                    policy,
                                    previous: found_previous,
                                    set_count,
                                    reset_count,
                                    at_ticks,
                                    ..
                                } => {
                                    assert_eq!(node, &mossignal::NodeSubject::Node(fixture.node));
                                    assert_eq!(*policy, conflict);
                                    assert_eq!(*found_previous, previous);
                                    assert_eq!(*set_count, PulseCount::ONE);
                                    assert_eq!(*reset_count, PulseCount::ONE);
                                    assert_eq!(*at_ticks, 7);
                                }
                                other => panic!("unexpected rejection evidence: {other:?}"),
                            }
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn positive_multiplicity_is_presence_equivalent_but_conflict_evidence_is_exact() {
    let counts = [1, 2, 3, 18, u64::MAX];
    for count in counts {
        let fixture = latch_fixture(LogicLevel::Low, ConflictPolicy::SetDominant);
        let mut machine = fixture.compiled.spawn(policy());
        machine
            .apply(Transaction::initialize(
                Time::from_ticks(0),
                machine.revision(),
                snapshot(&fixture, PulseCount::new(count), PulseCount::ZERO),
            ))
            .unwrap_or_else(|failure| panic!("set presence must commit: {failure}"));
        assert_eq!(machine.output_level(fixture.output), Some(LogicLevel::High));

        let fixture = latch_fixture(LogicLevel::High, ConflictPolicy::ResetDominant);
        let mut machine = fixture.compiled.spawn(policy());
        machine
            .apply(Transaction::initialize(
                Time::from_ticks(0),
                machine.revision(),
                snapshot(&fixture, PulseCount::ZERO, PulseCount::new(count)),
            ))
            .unwrap_or_else(|failure| panic!("reset presence must commit: {failure}"));
        assert_eq!(machine.output_level(fixture.output), Some(LogicLevel::Low));
    }

    for (set_count, reset_count) in [(1, 2), (2, 3), (18, u64::MAX)] {
        let fixture = latch_fixture(LogicLevel::High, ConflictPolicy::RetainAndDiagnose);
        let mut machine = fixture.compiled.spawn(policy());
        let result = machine
            .apply(Transaction::initialize(
                Time::from_ticks(23),
                machine.revision(),
                snapshot(
                    &fixture,
                    PulseCount::new(set_count),
                    PulseCount::new(reset_count),
                ),
            ))
            .unwrap_or_else(|failure| panic!("retained conflict must commit: {failure}"));
        let [occurrence] = result.occurrences() else {
            panic!("one latch conflict must publish exactly one occurrence");
        };
        assert_eq!(occurrence.at(), Time::from_ticks(23));
        assert_eq!(occurrence.revision(), machine.revision());
        assert_eq!(
            occurrence.problem().code(),
            DiagnosticCode::RuntimePulseLatchConflictRetained
        );
        assert_eq!(occurrence.problem().severity(), Severity::Warning);
        assert_eq!(
            occurrence.problem().responsibility(),
            Responsibility::Advisory
        );
        match occurrence.problem().evidence() {
            ProblemEvidence::RuntimePulseLatchConflictRetained { evidence, .. } => {
                assert_eq!(evidence.node, NodeEvidence::Node(fixture.node));
                assert_eq!(evidence.policy, ConflictPolicy::RetainAndDiagnose);
                assert_eq!(evidence.previous, LogicLevel::High);
                assert_eq!(
                    evidence.controls,
                    mossignal::ConflictControls::Pulse {
                        set: PulseCount::new(set_count),
                        reset: PulseCount::new(reset_count)
                    }
                );
                assert_eq!(evidence.at_ticks, 23);
                assert_eq!(evidence.revision, machine.revision());
            }
            other => panic!("unexpected occurrence evidence: {other:?}"),
        }
    }
}

#[test]
fn ready_reactions_set_reset_retain_and_repeat_transient_conflicts() {
    let fixture = latch_fixture(LogicLevel::Low, ConflictPolicy::RetainAndDiagnose);
    let mut machine = fixture.compiled.spawn(policy());
    let initialized = machine
        .apply(Transaction::initialize(
            Time::from_ticks(0),
            machine.revision(),
            snapshot(&fixture, PulseCount::ZERO, PulseCount::ZERO),
        ))
        .unwrap_or_else(|failure| panic!("initialization must commit: {failure}"));
    assert!(initialized.occurrences().is_empty());

    let set = machine
        .apply(Transaction::advance(
            Time::from_ticks(1),
            machine.revision(),
            delta(&fixture, PulseCount::new(2), PulseCount::ZERO),
        ))
        .unwrap_or_else(|failure| panic!("set reaction must commit: {failure}"));
    assert!(set.occurrences().is_empty());
    assert_eq!(machine.output_level(fixture.output), Some(LogicLevel::High));
    let set_cause = machine
        .inspect_pulse_set_reset_latch(fixture.node)
        .unwrap_or_else(|failure| panic!("set latch must inspect: {failure:?}"))
        .latest_establishment();
    match set
        .provenance()
        .inspect(set_cause)
        .unwrap_or_else(|failure| panic!("set cause must resolve: {failure}"))
    {
        CauseInspection::PulseControlledLevel {
            contributions,
            result,
            ..
        } => {
            assert_eq!(result, LogicLevel::High);
            assert_eq!(
                contributions
                    .iter()
                    .map(|entry| entry.count())
                    .collect::<Vec<_>>(),
                vec![PulseCount::new(2), PulseCount::ZERO]
            );
        }
        _ => panic!("unexpected set provenance"),
    }

    let repeated_set = machine
        .apply(Transaction::advance(
            Time::from_ticks(2),
            machine.revision(),
            delta(&fixture, PulseCount::new(3), PulseCount::ZERO),
        ))
        .unwrap_or_else(|failure| panic!("same-value set reaction must commit: {failure}"));
    assert!(repeated_set.occurrences().is_empty());
    assert_eq!(machine.output_level(fixture.output), Some(LogicLevel::High));
    let repeated_set_cause = machine
        .inspect_pulse_set_reset_latch(fixture.node)
        .unwrap_or_else(|failure| panic!("repeated set latch must inspect: {failure:?}"))
        .latest_establishment();
    assert_ne!(
        repeated_set_cause, set_cause,
        "an explicit same-value control refreshes the state-establishing cause"
    );
    assert!(
        repeated_set
            .provenance()
            .inspect(repeated_set_cause)
            .is_ok()
    );

    let reset = machine
        .apply(Transaction::advance(
            Time::from_ticks(3),
            machine.revision(),
            delta(&fixture, PulseCount::ZERO, PulseCount::new(4)),
        ))
        .unwrap_or_else(|failure| panic!("reset reaction must commit: {failure}"));
    assert!(reset.occurrences().is_empty());
    assert_eq!(machine.output_level(fixture.output), Some(LogicLevel::Low));

    for at in [4, 5] {
        let conflict = machine
            .apply(Transaction::advance(
                Time::from_ticks(at),
                machine.revision(),
                delta(&fixture, PulseCount::ONE, PulseCount::ONE),
            ))
            .unwrap_or_else(|failure| panic!("retained conflict must commit: {failure}"));
        assert_eq!(conflict.occurrences().len(), 1);
        assert_eq!(conflict.occurrences()[0].at(), Time::from_ticks(at));
        assert_eq!(machine.output_level(fixture.output), Some(LogicLevel::Low));
    }
}

#[test]
fn current_latch_output_is_visible_to_an_any_edge_in_the_same_reaction() {
    let mut builder = NetworkBuilder::<TestDomain>::new(TimeDomainId::from_u128(30));
    let (set_key, set) = builder.pulse_input("set");
    let (reset_key, reset) = builder.pulse_input("reset");
    let state = builder
        .pulse_set_reset_latch(
            set,
            reset,
            PulseSetResetConfig::new(LogicLevel::Low, ConflictPolicy::SetDominant),
        )
        .unwrap_or_else(|failure| panic!("latch must author: {failure:?}"));
    let edge = builder
        .any_edge(
            state,
            EdgeConfig::new(EdgeInitialization::Assume(LogicLevel::Low)),
        )
        .unwrap_or_else(|failure| panic!("edge must author: {failure:?}"));
    let output = builder
        .pulse_output("edge", edge)
        .unwrap_or_else(|failure| panic!("pulse output must author: {failure:?}"));
    let compiled = builder
        .finish()
        .require_artifact()
        .unwrap_or_else(|failure| panic!("chain must validate: {failure:?}"))
        .compile()
        .require_artifact()
        .unwrap_or_else(|failure| panic!("chain must compile: {failure:?}"));
    let snapshot = compiled
        .input_snapshot()
        .pulse(set_key, PulseCount::ONE)
        .and_then(|builder| builder.pulse(reset_key, PulseCount::ZERO))
        .and_then(mossignal::InputSnapshotBuilder::finish)
        .unwrap_or_else(|failure| panic!("snapshot must build: {failure}"));
    let mut machine = compiled.spawn(policy());
    let result = machine
        .apply(Transaction::initialize(
            Time::from_ticks(0),
            machine.revision(),
            snapshot,
        ))
        .unwrap_or_else(|failure| panic!("chain must initialize: {failure}"));
    assert!(result.output_events().iter().any(|event| matches!(
        event,
        OutputEvent::Pulsed {
            output: found,
            count: PulseCount::ONE,
            ..
        } if *found == output
    )));
}

struct TemporalFixture {
    compiled: mossignal::CompiledNetwork<TestDomain>,
    delayed_set: ExternalInputKey<Pulse>,
    delayed_reset: ExternalInputKey<Pulse>,
    direct_set: ExternalInputKey<Pulse>,
    direct_reset: ExternalInputKey<Pulse>,
    delayed_latch: NodeKey,
    direct_latch: NodeKey,
    delayed_output: ExternalOutputKey<Level>,
    direct_output: ExternalOutputKey<Level>,
}

fn temporal_fixture(direct_policy: ConflictPolicy) -> TemporalFixture {
    let mut builder = NetworkBuilder::<TestDomain>::new(TimeDomainId::from_u128(40));
    let (delayed_set, delayed_set_signal) = builder.pulse_input("delayed-set");
    let (delayed_reset, delayed_reset_signal) = builder.pulse_input("delayed-reset");
    let (direct_set, direct_set_signal) = builder.pulse_input("direct-set");
    let (direct_reset, direct_reset_signal) = builder.pulse_input("direct-reset");
    let delay = PulseDelayConfig::new(
        NonZeroSpan::from_ticks(5)
            .unwrap_or_else(|failure| panic!("positive delay must build: {failure}")),
    );
    let delayed_set_signal = builder
        .pulse_delay(delayed_set_signal, delay)
        .unwrap_or_else(|failure| panic!("set delay must author: {failure:?}"));
    let delayed_reset_signal = builder
        .pulse_delay(delayed_reset_signal, delay)
        .unwrap_or_else(|failure| panic!("reset delay must author: {failure:?}"));
    let delayed_latch = NodeKey::from_u128(100);
    let delayed_state = builder
        .add_pulse_set_reset_latch(
            delayed_latch,
            delayed_set_signal,
            delayed_reset_signal,
            PulseSetResetConfig::new(LogicLevel::Low, ConflictPolicy::RetainAndDiagnose),
            DiagnosticMeta::default(),
        )
        .unwrap_or_else(|failure| panic!("delayed latch must author: {failure:?}"))
        .into_outputs();
    let direct_latch = NodeKey::from_u128(200);
    let direct_state = builder
        .add_pulse_set_reset_latch(
            direct_latch,
            direct_set_signal,
            direct_reset_signal,
            PulseSetResetConfig::new(LogicLevel::Low, direct_policy),
            DiagnosticMeta::default(),
        )
        .unwrap_or_else(|failure| panic!("direct latch must author: {failure:?}"))
        .into_outputs();
    let delayed_output = builder
        .level_output("delayed", delayed_state)
        .unwrap_or_else(|failure| panic!("delayed output must author: {failure:?}"));
    let direct_output = builder
        .level_output("direct", direct_state)
        .unwrap_or_else(|failure| panic!("direct output must author: {failure:?}"));
    let compiled = builder
        .finish()
        .require_artifact()
        .unwrap_or_else(|failure| panic!("temporal fixture must validate: {failure:?}"))
        .compile()
        .require_artifact()
        .unwrap_or_else(|failure| panic!("temporal fixture must compile: {failure:?}"));
    TemporalFixture {
        compiled,
        delayed_set,
        delayed_reset,
        direct_set,
        direct_reset,
        delayed_latch,
        direct_latch,
        delayed_output,
        direct_output,
    }
}

fn initialize_temporal(fixture: &TemporalFixture) -> mossignal::Machine<TestDomain> {
    let initial = fixture
        .compiled
        .input_snapshot()
        .pulse(fixture.delayed_set, PulseCount::ONE)
        .and_then(|builder| builder.pulse(fixture.delayed_reset, PulseCount::ONE))
        .and_then(|builder| builder.pulse(fixture.direct_set, PulseCount::ZERO))
        .and_then(|builder| builder.pulse(fixture.direct_reset, PulseCount::ZERO))
        .and_then(mossignal::InputSnapshotBuilder::finish)
        .unwrap_or_else(|failure| panic!("temporal snapshot must build: {failure}"));
    let mut machine = fixture.compiled.spawn(policy());
    let initialized = machine
        .apply(Transaction::initialize(
            Time::from_ticks(0),
            machine.revision(),
            initial,
        ))
        .unwrap_or_else(|failure| panic!("temporal fixture must initialize: {failure}"));
    assert_eq!(
        initialized.schedule(),
        Schedule::WakeAt(Time::from_ticks(5))
    );
    machine
}

#[test]
fn deadline_and_target_time_occurrences_keep_chronological_order_and_provenance() {
    let fixture = temporal_fixture(ConflictPolicy::RetainAndDiagnose);
    let mut machine = initialize_temporal(&fixture);
    let current = fixture
        .compiled
        .input_delta()
        .pulse(fixture.direct_set, PulseCount::ONE)
        .and_then(|builder| builder.pulse(fixture.direct_reset, PulseCount::ONE))
        .and_then(mossignal::InputDeltaBuilder::finish)
        .unwrap_or_else(|failure| panic!("target delta must build: {failure}"));
    let result = machine
        .apply(Transaction::advance(
            Time::from_ticks(10),
            machine.revision(),
            current,
        ))
        .unwrap_or_else(|failure| panic!("outer transaction must commit: {failure}"));
    assert_eq!(
        result
            .occurrences()
            .iter()
            .map(|occurrence| occurrence.at().ticks())
            .collect::<Vec<_>>(),
        vec![5, 10]
    );
    let delayed = machine
        .inspect_pulse_set_reset_latch(fixture.delayed_latch)
        .unwrap_or_else(|failure| panic!("delayed latch must inspect: {failure:?}"));
    match result
        .provenance()
        .inspect(delayed.latest_establishment())
        .unwrap_or_else(|failure| panic!("state cause must resolve: {failure}"))
    {
        CauseInspection::PulseControlledLevel {
            contributions,
            result,
            ..
        } => {
            assert_eq!(result, LogicLevel::Low);
            assert_eq!(contributions.len(), 2);
            assert!(
                contributions
                    .iter()
                    .all(|entry| entry.count() == PulseCount::ZERO)
            );
        }
        _ => panic!("unexpected latch provenance"),
    }
}

#[test]
fn a_late_reject_discards_earlier_candidate_occurrences_and_all_state() {
    let fixture = temporal_fixture(ConflictPolicy::RejectTransaction);
    let mut machine = initialize_temporal(&fixture);
    let before_delayed = machine
        .inspect_pulse_set_reset_latch(fixture.delayed_latch)
        .unwrap_or_else(|failure| panic!("delayed latch must inspect: {failure:?}"));
    let before_direct = machine
        .inspect_pulse_set_reset_latch(fixture.direct_latch)
        .unwrap_or_else(|failure| panic!("direct latch must inspect: {failure:?}"));
    let current = fixture
        .compiled
        .input_delta()
        .pulse(fixture.direct_set, PulseCount::ONE)
        .and_then(|builder| builder.pulse(fixture.direct_reset, PulseCount::ONE))
        .and_then(mossignal::InputDeltaBuilder::finish)
        .unwrap_or_else(|failure| panic!("target delta must build: {failure}"));
    let failure = machine
        .apply(Transaction::advance(
            Time::from_ticks(10),
            machine.revision(),
            current,
        ))
        .expect_err("target conflict must reject the whole outer transaction");
    assert_eq!(
        failure.code(),
        DiagnosticCode::RuntimePulseLatchConflictRejected
    );
    assert_eq!(machine.now(), Some(Time::from_ticks(0)));
    assert_eq!(
        machine.schedule(),
        Ok(Schedule::WakeAt(Time::from_ticks(5)))
    );
    assert_eq!(
        machine.output_level(fixture.delayed_output),
        Some(LogicLevel::Low)
    );
    assert_eq!(
        machine.output_level(fixture.direct_output),
        Some(LogicLevel::Low)
    );
    assert_eq!(
        machine
            .inspect_pulse_set_reset_latch(fixture.delayed_latch)
            .unwrap_or_else(|failure| panic!("delayed latch must inspect: {failure:?}"))
            .latest_establishment(),
        before_delayed.latest_establishment()
    );
    assert_eq!(
        machine
            .inspect_pulse_set_reset_latch(fixture.direct_latch)
            .unwrap_or_else(|failure| panic!("direct latch must inspect: {failure:?}"))
            .latest_establishment(),
        before_direct.latest_establishment()
    );
}

#[test]
fn simultaneous_occurrences_use_stable_node_order_not_authoring_order() {
    let mut builder = NetworkBuilder::<TestDomain>::new(TimeDomainId::from_u128(50));
    let (set_key, set) = builder.pulse_input("set");
    let (reset_key, reset) = builder.pulse_input("reset");
    for node in [NodeKey::from_u128(20), NodeKey::from_u128(10)] {
        builder
            .add_pulse_set_reset_latch(
                node,
                set,
                reset,
                PulseSetResetConfig::new(LogicLevel::Low, ConflictPolicy::RetainAndDiagnose),
                DiagnosticMeta::default(),
            )
            .unwrap_or_else(|failure| panic!("latch must author: {failure:?}"));
    }
    let compiled = builder
        .finish()
        .require_artifact()
        .unwrap_or_else(|failure| panic!("network must validate: {failure:?}"))
        .compile()
        .require_artifact()
        .unwrap_or_else(|failure| panic!("network must compile: {failure:?}"));
    let snapshot = compiled
        .input_snapshot()
        .pulse(set_key, PulseCount::ONE)
        .and_then(|builder| builder.pulse(reset_key, PulseCount::ONE))
        .and_then(mossignal::InputSnapshotBuilder::finish)
        .unwrap_or_else(|failure| panic!("snapshot must build: {failure}"));
    let mut machine = compiled.spawn(policy());
    let result = machine
        .apply(Transaction::initialize(
            Time::from_ticks(0),
            machine.revision(),
            snapshot,
        ))
        .unwrap_or_else(|failure| panic!("network must initialize: {failure}"));
    let nodes = result
        .occurrences()
        .iter()
        .map(|occurrence| match occurrence.problem().evidence() {
            ProblemEvidence::RuntimePulseLatchConflictRetained {
                evidence:
                    mossignal::ConflictEvidence {
                        node: NodeEvidence::Node(node),
                        ..
                    },
                ..
            } => *node,
            other => panic!("unexpected occurrence evidence: {other:?}"),
        })
        .collect::<Vec<_>>();
    assert_eq!(nodes, vec![NodeKey::from_u128(10), NodeKey::from_u128(20)]);
}

#[test]
fn module_local_occurrence_retains_the_qualified_primitive_subject() {
    let module_set = ModuleInputKey::<Pulse>::from_u128(1);
    let module_reset = ModuleInputKey::<Pulse>::from_u128(2);
    let module_output = ModuleOutputKey::<Level>::from_u128(3);
    let latch = NodeKey::from_u128(10);
    let mut module = ModuleBuilder::<TestDomain>::new();
    let set = module
        .add_pulse_input(module_set, DiagnosticMeta::default())
        .unwrap_or_else(|failure| panic!("module set must author: {failure:?}"));
    let reset = module
        .add_pulse_input(module_reset, DiagnosticMeta::default())
        .unwrap_or_else(|failure| panic!("module reset must author: {failure:?}"));
    let state = module
        .add_pulse_set_reset_latch(
            latch,
            set,
            reset,
            PulseSetResetConfig::new(LogicLevel::Low, ConflictPolicy::RetainAndDiagnose),
            DiagnosticMeta::default(),
        )
        .unwrap_or_else(|failure| panic!("module latch must author: {failure:?}"))
        .into_outputs();
    module
        .add_level_output(module_output, state, DiagnosticMeta::default())
        .unwrap_or_else(|failure| panic!("module output must author: {failure:?}"));
    let module = module
        .finish()
        .require_artifact()
        .unwrap_or_else(|failure| panic!("module must validate: {failure:?}"));

    let instance = ModuleInstanceKey::from_u128(100);
    let mut network = NetworkBuilder::<TestDomain>::new(TimeDomainId::from_u128(60));
    let (set_key, set) = network.pulse_input("set");
    let (reset_key, reset) = network.pulse_input("reset");
    let added = network
        .instantiate(&module, instance, DiagnosticMeta::default())
        .unwrap_or_else(|failure| panic!("instance must begin: {failure:?}"))
        .bind_pulse(module_set, set)
        .and_then(|builder| builder.bind_pulse(module_reset, reset))
        .and_then(|builder| builder.finish())
        .unwrap_or_else(|failure| panic!("instance must bind: {failure:?}"));
    network
        .level_output(
            "state",
            added
                .level_output(module_output)
                .unwrap_or_else(|failure| panic!("module output must exist: {failure:?}")),
        )
        .unwrap_or_else(|failure| panic!("network output must author: {failure:?}"));
    let compiled = network
        .finish()
        .require_artifact()
        .unwrap_or_else(|failure| panic!("network must validate: {failure:?}"))
        .compile()
        .require_artifact()
        .unwrap_or_else(|failure| panic!("network must compile: {failure:?}"));
    let snapshot = compiled
        .input_snapshot()
        .pulse(set_key, PulseCount::ONE)
        .and_then(|builder| builder.pulse(reset_key, PulseCount::ONE))
        .and_then(mossignal::InputSnapshotBuilder::finish)
        .unwrap_or_else(|failure| panic!("snapshot must build: {failure}"));
    let mut machine = compiled.spawn(policy());
    let result = machine
        .apply(Transaction::initialize(
            Time::from_ticks(0),
            machine.revision(),
            snapshot,
        ))
        .unwrap_or_else(|failure| panic!("module network must initialize: {failure}"));
    let [occurrence] = result.occurrences() else {
        panic!("one module-local latch must emit one occurrence");
    };
    match occurrence.problem().evidence() {
        ProblemEvidence::RuntimePulseLatchConflictRetained { evidence, .. } => assert_eq!(
            evidence.node,
            NodeEvidence::Qualified {
                instances: vec![instance],
                node: latch,
            }
        ),
        other => panic!("unexpected module occurrence evidence: {other:?}"),
    }
    let inspected = machine
        .inspect_module(instance)
        .unwrap_or_else(|failure| panic!("module must inspect: {failure:?}"));
    let latch = inspected
        .nodes()
        .iter()
        .find(|node| node.node().node() == NodeKey::from_u128(10))
        .unwrap_or_else(|| panic!("module latch must remain inspectable"));
    assert_eq!(latch.pulse_set_reset_state(), Some(LogicLevel::Low));
    assert!(latch.pulse_set_reset_cause().is_some());
}

#[test]
fn latch_identity_is_sensitive_to_initial_state_and_every_policy() {
    let mut fingerprints = BTreeSet::new();
    for initial in [LogicLevel::Low, LogicLevel::High] {
        for conflict in [
            ConflictPolicy::SetDominant,
            ConflictPolicy::ResetDominant,
            ConflictPolicy::RetainAndDiagnose,
            ConflictPolicy::RejectTransaction,
        ] {
            assert!(fingerprints.insert(latch_fixture(initial, conflict).compiled.fingerprint()));
        }
    }
    assert_eq!(fingerprints.len(), 8);
}

#[test]
fn a_valid_dynamic_latch_uses_the_same_law_as_typed_and_explicit_authoring() {
    let set = ExternalInputKey::<Pulse>::from_u128(1);
    let reset = ExternalInputKey::<Pulse>::from_u128(2);
    let node = NodeKey::from_u128(3);
    let set_port = InPortKey::<Pulse>::from_u128(4);
    let reset_port = InPortKey::<Pulse>::from_u128(5);
    let output_port = OutPortKey::<Level>::from_u128(6);
    let output = ExternalOutputKey::<Level>::from_u128(7);
    let dynamic = UncheckedNetwork::new(
        NetworkKey::from_u128(8),
        TimeDomainId::from_u128(9),
        DiagnosticMeta::default(),
        vec![NodeDef::new(
            node,
            NodeKind::<TestDomain>::pulse_set_reset_latch(PulseSetResetConfig::new(
                LogicLevel::Low,
                ConflictPolicy::SetDominant,
            )),
            NodePorts::with_input_roles(
                vec![set_port.into(), reset_port.into()],
                vec![InputPortRole::Set, InputPortRole::Reset],
                vec![output_port.into()],
            ),
            DiagnosticMeta::default(),
        )],
        vec![
            ExternalInputDef::new(set.into(), DiagnosticMeta::default()),
            ExternalInputDef::new(reset.into(), DiagnosticMeta::default()),
        ],
        vec![ExternalOutputDef::new(
            output.into(),
            SignalSourceKey::NodeOutput(output_port).into(),
            DiagnosticMeta::default(),
        )],
        vec![
            ConnectionDef::new(
                ConnectionKey::from_u128(10),
                mossignal::authored::ConnectionEndpoint::external_input(set.into()),
                mossignal::authored::ConnectionEndpoint::node_input(set_port.into()),
                DiagnosticMeta::default(),
            ),
            ConnectionDef::new(
                ConnectionKey::from_u128(11),
                mossignal::authored::ConnectionEndpoint::external_input(reset.into()),
                mossignal::authored::ConnectionEndpoint::node_input(reset_port.into()),
                DiagnosticMeta::default(),
            ),
        ],
    );
    let compiled = dynamic
        .validate()
        .require_artifact()
        .unwrap_or_else(|failure| panic!("dynamic latch must validate: {failure:?}"))
        .compile()
        .require_artifact()
        .unwrap_or_else(|failure| panic!("dynamic latch must compile: {failure:?}"));
    let inputs = compiled
        .input_snapshot()
        .pulse(set, PulseCount::new(4))
        .and_then(|builder| builder.pulse(reset, PulseCount::ZERO))
        .and_then(mossignal::InputSnapshotBuilder::finish)
        .unwrap_or_else(|failure| panic!("dynamic inputs must build: {failure}"));
    let mut machine = compiled.spawn(policy());
    machine
        .apply(Transaction::initialize(
            Time::from_ticks(0),
            machine.revision(),
            inputs,
        ))
        .unwrap_or_else(|failure| panic!("dynamic latch must initialize: {failure}"));
    assert_eq!(machine.output_level(output), Some(LogicLevel::High));
    assert_eq!(
        machine
            .inspect_pulse_set_reset_latch(node)
            .unwrap_or_else(|failure| panic!("dynamic latch must inspect: {failure:?}"))
            .committed(),
        LogicLevel::High
    );
}

#[test]
fn both_current_control_dependencies_participate_in_cycle_rejection() {
    let reset = ExternalInputKey::<Pulse>::from_u128(1);
    let latch = NodeKey::from_u128(2);
    let set_port = InPortKey::<Pulse>::from_u128(3);
    let reset_port = InPortKey::<Pulse>::from_u128(4);
    let latch_output = OutPortKey::<Level>::from_u128(5);
    let edge = NodeKey::from_u128(6);
    let edge_input = InPortKey::<Level>::from_u128(7);
    let edge_output = OutPortKey::<Pulse>::from_u128(8);
    let cyclic = UncheckedNetwork::new(
        NetworkKey::from_u128(9),
        TimeDomainId::from_u128(10),
        DiagnosticMeta::default(),
        vec![
            NodeDef::new(
                latch,
                NodeKind::<TestDomain>::pulse_set_reset_latch(PulseSetResetConfig::new(
                    LogicLevel::Low,
                    ConflictPolicy::RetainAndDiagnose,
                )),
                NodePorts::with_input_roles(
                    vec![set_port.into(), reset_port.into()],
                    vec![InputPortRole::Set, InputPortRole::Reset],
                    vec![latch_output.into()],
                ),
                DiagnosticMeta::default(),
            ),
            NodeDef::new(
                edge,
                NodeKind::any_edge(EdgeConfig::new(EdgeInitialization::Assume(LogicLevel::Low))),
                NodePorts::new(vec![edge_input.into()], vec![edge_output.into()]),
                DiagnosticMeta::default(),
            ),
        ],
        vec![ExternalInputDef::new(
            reset.into(),
            DiagnosticMeta::default(),
        )],
        Vec::new(),
        vec![
            ConnectionDef::new(
                ConnectionKey::from_u128(11),
                mossignal::authored::ConnectionEndpoint::node_output(edge_output.into()),
                mossignal::authored::ConnectionEndpoint::node_input(set_port.into()),
                DiagnosticMeta::default(),
            ),
            ConnectionDef::new(
                ConnectionKey::from_u128(12),
                mossignal::authored::ConnectionEndpoint::external_input(reset.into()),
                mossignal::authored::ConnectionEndpoint::node_input(reset_port.into()),
                DiagnosticMeta::default(),
            ),
            ConnectionDef::new(
                ConnectionKey::from_u128(13),
                mossignal::authored::ConnectionEndpoint::node_output(latch_output.into()),
                mossignal::authored::ConnectionEndpoint::node_input(edge_input.into()),
                DiagnosticMeta::default(),
            ),
        ],
    )
    .validate();
    assert!(cyclic.artifact().is_none());
    assert!(cyclic.diagnostics().iter().any(|diagnostic| {
        diagnostic.problem().code() == DiagnosticCode::ValidationCurrentReactionCycle
    }));
}

#[test]
fn malformed_dynamic_latch_shape_roles_and_kinds_are_rejected_without_normalization() {
    let set = ExternalInputKey::<Pulse>::from_u128(1);
    let reset = ExternalInputKey::<Pulse>::from_u128(2);
    let node = NodeKey::from_u128(3);
    let set_port = InPortKey::<Pulse>::from_u128(4);
    let reset_port = InPortKey::<Pulse>::from_u128(5);
    let output_port = OutPortKey::<Level>::from_u128(6);
    let output = ExternalOutputKey::<Level>::from_u128(7);
    let malformed = UncheckedNetwork::new(
        NetworkKey::from_u128(8),
        TimeDomainId::from_u128(9),
        DiagnosticMeta::default(),
        vec![NodeDef::new(
            node,
            NodeKind::<TestDomain>::pulse_set_reset_latch(PulseSetResetConfig::new(
                LogicLevel::Low,
                ConflictPolicy::SetDominant,
            )),
            NodePorts::with_input_roles(
                vec![set_port.into(), reset_port.into()],
                vec![InputPortRole::Set, InputPortRole::Set],
                vec![output_port.into()],
            ),
            DiagnosticMeta::default(),
        )],
        vec![
            ExternalInputDef::new(set.into(), DiagnosticMeta::default()),
            ExternalInputDef::new(reset.into(), DiagnosticMeta::default()),
        ],
        vec![ExternalOutputDef::new(
            output.into(),
            SignalSourceKey::NodeOutput(output_port).into(),
            DiagnosticMeta::default(),
        )],
        vec![
            ConnectionDef::new(
                ConnectionKey::from_u128(10),
                mossignal::authored::ConnectionEndpoint::external_input(set.into()),
                mossignal::authored::ConnectionEndpoint::node_input(set_port.into()),
                DiagnosticMeta::default(),
            ),
            ConnectionDef::new(
                ConnectionKey::from_u128(11),
                mossignal::authored::ConnectionEndpoint::external_input(reset.into()),
                mossignal::authored::ConnectionEndpoint::node_input(reset_port.into()),
                DiagnosticMeta::default(),
            ),
        ],
    );
    let report = malformed.validate();
    assert!(report.artifact().is_none());
    assert!(report.diagnostics().iter().any(|diagnostic| matches!(
        diagnostic.problem().evidence(),
        ProblemEvidence::ValidationMissingRequiredInput { .. }
            | ProblemEvidence::ValidationInvalidFixedArity { .. }
    )));

    let only_set = InPortKey::<Pulse>::from_u128(20);
    let missing_reset = UncheckedNetwork::new(
        NetworkKey::from_u128(21),
        TimeDomainId::from_u128(22),
        DiagnosticMeta::default(),
        vec![NodeDef::new(
            NodeKey::from_u128(23),
            NodeKind::<TestDomain>::pulse_set_reset_latch(PulseSetResetConfig::new(
                LogicLevel::Low,
                ConflictPolicy::SetDominant,
            )),
            NodePorts::with_input_roles(
                vec![only_set.into()],
                vec![InputPortRole::Set],
                vec![OutPortKey::<Level>::from_u128(24).into()],
            ),
            DiagnosticMeta::default(),
        )],
        vec![ExternalInputDef::new(set.into(), DiagnosticMeta::default())],
        Vec::new(),
        vec![ConnectionDef::new(
            ConnectionKey::from_u128(25),
            mossignal::authored::ConnectionEndpoint::external_input(set.into()),
            mossignal::authored::ConnectionEndpoint::node_input(only_set.into()),
            DiagnosticMeta::default(),
        )],
    )
    .validate();
    assert!(missing_reset.artifact().is_none());
    assert!(missing_reset.diagnostics().iter().any(|diagnostic| {
        diagnostic.problem().code() == DiagnosticCode::ValidationInvalidFixedArity
    }));

    let level_set = ExternalInputKey::<Pulse>::from_u128(30);
    let level_reset = ExternalInputKey::<Pulse>::from_u128(31);
    let level_set_port = InPortKey::<Level>::from_u128(32);
    let level_reset_port = InPortKey::<Level>::from_u128(33);
    let wrong_kind = UncheckedNetwork::new(
        NetworkKey::from_u128(34),
        TimeDomainId::from_u128(35),
        DiagnosticMeta::default(),
        vec![NodeDef::new(
            NodeKey::from_u128(36),
            NodeKind::<TestDomain>::pulse_set_reset_latch(PulseSetResetConfig::new(
                LogicLevel::Low,
                ConflictPolicy::SetDominant,
            )),
            NodePorts::with_input_roles(
                vec![level_set_port.into(), level_reset_port.into()],
                vec![InputPortRole::Set, InputPortRole::Reset],
                vec![OutPortKey::<Level>::from_u128(37).into()],
            ),
            DiagnosticMeta::default(),
        )],
        vec![
            ExternalInputDef::new(level_set.into(), DiagnosticMeta::default()),
            ExternalInputDef::new(level_reset.into(), DiagnosticMeta::default()),
        ],
        Vec::new(),
        vec![
            ConnectionDef::new(
                ConnectionKey::from_u128(38),
                mossignal::authored::ConnectionEndpoint::external_input(level_set.into()),
                mossignal::authored::ConnectionEndpoint::node_input(level_set_port.into()),
                DiagnosticMeta::default(),
            ),
            ConnectionDef::new(
                ConnectionKey::from_u128(39),
                mossignal::authored::ConnectionEndpoint::external_input(level_reset.into()),
                mossignal::authored::ConnectionEndpoint::node_input(level_reset_port.into()),
                DiagnosticMeta::default(),
            ),
        ],
    )
    .validate();
    assert!(wrong_kind.artifact().is_none());
    assert!(wrong_kind.diagnostics().iter().any(|diagnostic| {
        diagnostic.problem().code() == DiagnosticCode::ValidationSignalKindMismatch
    }));
}
