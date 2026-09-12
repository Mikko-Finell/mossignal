use mossignal::diagnostics::{
    DiagnosticCode, NodeEvidence, ProblemDelivery, ProblemEvidence, Responsibility, Severity,
};
use mossignal::key::{ExternalInputKey, ExternalOutputKey, NetworkKey, NodeKey};
use mossignal::metadata::DiagnosticMeta;
use mossignal::signal::{Level, LogicLevel, PulseCount};
use mossignal::time::Time;
use mossignal::{
    ConflictControls, ConflictPolicy, DiagnosticEpisodeChangeKind as Change, EdgeConfig,
    EdgeInitialization, LevelSetResetConfig, NetworkBuilder, RuntimePolicy, TimeDomainId,
    Transaction,
};

// Ownership must not add Clone bounds to the caller's time domain.
#[derive(Debug, PartialEq, Eq)]
struct Domain;
const LOW: LogicLevel = LogicLevel::Low;
const HIGH: LogicLevel = LogicLevel::High;
const POLICIES: [ConflictPolicy; 4] = [
    ConflictPolicy::SetDominant,
    ConflictPolicy::ResetDominant,
    ConflictPolicy::RetainAndDiagnose,
    ConflictPolicy::RejectTransaction,
];
fn policy(events: u64) -> RuntimePolicy {
    RuntimePolicy::builder()
        .max_internal_reactions(1000)
        .max_evaluated_operations(100_000)
        .max_pending_events(1000)
        .max_events_created_per_transaction(events)
        .max_required_provenance_growth(100_000)
        .build()
        .unwrap()
}
struct Fixture {
    compiled: mossignal::CompiledNetwork<Domain>,
    set: ExternalInputKey<Level>,
    reset: ExternalInputKey<Level>,
    output: ExternalOutputKey<Level>,
    node: NodeKey,
}
fn fixture(initial: LogicLevel, conflict: ConflictPolicy) -> Fixture {
    let mut b = NetworkBuilder::with_key(NetworkKey::from_u128(1), TimeDomainId::from_u128(2));
    let (set, s) = b.level_input("set");
    let (reset, r) = b.level_input("reset");
    let node = NodeKey::from_u128(100);
    let state = b
        .add_level_set_reset_latch(
            node,
            s,
            r,
            LevelSetResetConfig::new(initial, conflict),
            DiagnosticMeta::default(),
        )
        .unwrap()
        .into_outputs();
    let output = b.level_output("state", state).unwrap();
    let compiled = b
        .finish()
        .require_artifact()
        .unwrap()
        .compile()
        .require_artifact()
        .unwrap();
    Fixture {
        compiled,
        set,
        reset,
        output,
        node,
    }
}
fn snapshot(f: &Fixture, set: LogicLevel, reset: LogicLevel) -> mossignal::InputSnapshot<Domain> {
    f.compiled
        .input_snapshot()
        .set(f.set, set)
        .unwrap()
        .set(f.reset, reset)
        .unwrap()
        .finish()
        .unwrap()
}
fn delta(f: &Fixture, set: LogicLevel, reset: LogicLevel) -> mossignal::InputDelta<Domain> {
    f.compiled
        .input_delta()
        .set(f.set, set)
        .unwrap()
        .set(f.reset, reset)
        .unwrap()
        .finish()
        .unwrap()
}
fn expected(
    previous: LogicLevel,
    set: LogicLevel,
    reset: LogicLevel,
    policy: ConflictPolicy,
) -> Option<LogicLevel> {
    match (set.is_high(), reset.is_high()) {
        (false, false) => Some(previous),
        (true, false) => Some(HIGH),
        (false, true) => Some(LOW),
        (true, true) => match policy {
            ConflictPolicy::SetDominant => Some(HIGH),
            ConflictPolicy::ResetDominant => Some(LOW),
            ConflictPolicy::RetainAndDiagnose => Some(previous),
            ConflictPolicy::RejectTransaction => None,
        },
    }
}

#[test]
fn exhausts_initial_and_ready_control_state_policy_laws() {
    for initial in [LOW, HIGH] {
        for set in [LOW, HIGH] {
            for reset in [LOW, HIGH] {
                for conflict in POLICIES {
                    let f = fixture(initial, conflict);
                    let mut m = f.compiled.spawn(policy(1000));
                    let result = m.apply(Transaction::initialize(
                        Time::from_ticks(5),
                        m.revision(),
                        snapshot(&f, set, reset),
                    ));
                    if let Some(value) = expected(initial, set, reset, conflict) {
                        let result = result.unwrap();
                        assert_eq!(m.output_level(f.output), Some(value));
                        let inspection = m.inspect_level_set_reset_latch(f.node).unwrap();
                        assert_eq!(
                            (inspection.committed(), inspection.set(), inspection.reset()),
                            (value, set, reset)
                        );
                        assert!(
                            result
                                .provenance()
                                .inspect(inspection.latest_establishment())
                                .is_ok()
                        );
                        let active = set == HIGH
                            && reset == HIGH
                            && conflict == ConflictPolicy::RetainAndDiagnose;
                        assert_eq!(
                            result.diagnostic_episode_changes().len(),
                            usize::from(active)
                        );
                        assert_eq!(
                            m.active_diagnostic_episodes().unwrap().len(),
                            usize::from(active)
                        );
                        assert!(result.occurrences().is_empty());
                        let again = m
                            .apply(Transaction::advance(
                                Time::from_ticks(6),
                                m.revision(),
                                delta(&f, set, reset),
                            ))
                            .unwrap();
                        assert_eq!(
                            m.output_level(f.output),
                            expected(value, set, reset, conflict)
                        );
                        assert!(again.diagnostic_episode_changes().is_empty());
                    } else {
                        let error = result.unwrap_err();
                        assert_eq!(
                            error.code(),
                            DiagnosticCode::RuntimeLevelLatchConflictRejected
                        );
                        assert_eq!(error.responsibility(), Responsibility::SemanticRejection);
                        assert_eq!(error.severity(), Severity::Error);
                        match error.problem().evidence() {
                            ProblemEvidence::RuntimeLevelLatchConflictRejected {
                                evidence, ..
                            } => {
                                assert_eq!(evidence.previous, initial);
                                assert_eq!(
                                    evidence.controls,
                                    ConflictControls::Level {
                                        set: HIGH,
                                        reset: HIGH
                                    }
                                );
                                assert_eq!(evidence.at_ticks, 5);
                            }
                            other => panic!("wrong conflict evidence: {other:?}"),
                        }
                        assert!(!m.is_initialized());
                        assert!(m.active_diagnostic_episodes().is_err());
                        // The same rejection must also preserve a ready machine.
                        m.apply(Transaction::initialize(
                            Time::from_ticks(5),
                            m.revision(),
                            snapshot(&f, LOW, LOW),
                        ))
                        .unwrap();
                        assert_eq!(
                            m.apply(Transaction::advance(
                                Time::from_ticks(6),
                                m.revision(),
                                delta(&f, HIGH, HIGH)
                            ))
                            .unwrap_err()
                            .code(),
                            DiagnosticCode::RuntimeLevelLatchConflictRejected
                        );
                        assert_eq!(m.now(), Some(Time::from_ticks(5)));
                        assert_eq!(m.output_level(f.output), Some(initial));
                    }
                }
            }
        }
    }
}

#[test]
fn exhausts_ready_laws_from_both_previous_levels() {
    // Bounds: one latch, both previous levels, four controls, four policies, one advance.
    for previous in [LOW, HIGH] {
        for set in [LOW, HIGH] {
            for reset in [LOW, HIGH] {
                for conflict in POLICIES {
                    let f = fixture(previous, conflict);
                    let mut m = f.compiled.spawn(policy(1000));
                    m.apply(Transaction::initialize(
                        Time::from_ticks(0),
                        m.revision(),
                        snapshot(&f, LOW, LOW),
                    ))
                    .unwrap();
                    let outcome = m.apply(Transaction::advance(
                        Time::from_ticks(1),
                        m.revision(),
                        delta(&f, set, reset),
                    ));
                    if let Some(value) = expected(previous, set, reset, conflict) {
                        let result = outcome.unwrap();
                        assert_eq!(m.output_level(f.output), Some(value));
                        assert_eq!(
                            m.inspect_level_set_reset_latch(f.node).unwrap().committed(),
                            value
                        );
                        let retained = set == HIGH
                            && reset == HIGH
                            && conflict == ConflictPolicy::RetainAndDiagnose;
                        assert_eq!(
                            result.diagnostic_episode_changes().len(),
                            usize::from(retained)
                        );
                        if retained {
                            let change = &result.diagnostic_episode_changes()[0];
                            assert_eq!(change.kind(), Change::Began);
                            match change.after().unwrap().evidence() {
                                ProblemEvidence::RuntimeLevelLatchConflictRetained {
                                    evidence,
                                    ..
                                } => {
                                    assert_eq!(evidence.previous, previous);
                                    assert_eq!(evidence.at_ticks, 1);
                                }
                                other => panic!("wrong retained evidence: {other:?}"),
                            }
                        }
                    } else {
                        assert_eq!(
                            outcome.unwrap_err().code(),
                            DiagnosticCode::RuntimeLevelLatchConflictRejected
                        );
                        assert_eq!(m.output_level(f.output), Some(previous));
                        assert_eq!(m.now(), Some(Time::from_ticks(0)));
                        assert!(m.active_diagnostic_episodes().unwrap().is_empty());
                    }
                }
            }
        }
    }
}

#[test]
fn episodes_begin_remain_quiet_resolve_and_begin_again_with_owned_evidence() {
    let f = fixture(LOW, ConflictPolicy::RetainAndDiagnose);
    let mut m = f.compiled.spawn(policy(1000));
    assert_eq!(
        m.active_diagnostic_episodes().unwrap_err().code(),
        DiagnosticCode::LifecycleNotInitialized
    );
    assert_eq!(
        m.inspect_level_set_reset_latch_definition(f.node)
            .unwrap()
            .initial(),
        LOW
    );
    let began = m
        .apply(Transaction::initialize(
            Time::from_ticks(2),
            m.revision(),
            snapshot(&f, HIGH, HIGH),
        ))
        .unwrap();
    let active = m.active_diagnostic_episodes().unwrap().remove(0);
    let event = &began.diagnostic_episode_changes()[0];
    assert_eq!(event.kind(), Change::Began);
    assert!(event.before().is_none());
    assert_eq!(event.at(), Time::from_ticks(2));
    assert_eq!(event.identity(), active.identity());
    assert_eq!(
        active.condition().code(),
        DiagnosticCode::RuntimeLevelLatchConflictRetained
    );
    assert!(
        active
            .current()
            .code()
            .allows_delivery(ProblemDelivery::PersistentEpisode)
    );
    assert!(
        !active
            .current()
            .code()
            .allows_delivery(ProblemDelivery::RuntimeOccurrence)
    );
    assert_eq!(active.current().severity(), Severity::Warning);
    assert_eq!(active.current().responsibility(), Responsibility::Advisory);
    for t in [3, 4] {
        let input = if t == 3 {
            f.compiled.input_delta().finish().unwrap()
        } else {
            delta(&f, HIGH, HIGH)
        };
        let quiet = m
            .apply(Transaction::advance(
                Time::from_ticks(t),
                m.revision(),
                input,
            ))
            .unwrap();
        assert!(quiet.diagnostic_episode_changes().is_empty());
        let now = m.active_diagnostic_episodes().unwrap().remove(0);
        assert_eq!(now.began_at(), Time::from_ticks(2));
        assert_eq!(now.last_material_change(), Time::from_ticks(2));
        assert_eq!(now.identity(), active.identity());
        assert_eq!(now.current(), active.current());
    }
    let resolved = m
        .apply(Transaction::advance(
            Time::from_ticks(5),
            m.revision(),
            delta(&f, LOW, HIGH),
        ))
        .unwrap();
    let event = &resolved.diagnostic_episode_changes()[0];
    assert_eq!(event.kind(), Change::Resolved);
    assert!(event.after().is_none());
    assert_eq!(event.before(), Some(active.current()));
    assert!(m.active_diagnostic_episodes().unwrap().is_empty());
    let new = m
        .apply(Transaction::advance(
            Time::from_ticks(6),
            m.revision(),
            delta(&f, HIGH, HIGH),
        ))
        .unwrap();
    assert_eq!(new.diagnostic_episode_changes()[0].kind(), Change::Began);
    let later = m.active_diagnostic_episodes().unwrap().remove(0);
    assert_eq!(later.identity(), active.identity()); // identity denotes the stable condition, not an event serial
    assert_eq!(later.began_at(), Time::from_ticks(6));
    assert!(active.provenance().inspect(active.cause()).is_ok());
    assert!(
        began
            .provenance()
            .inspect(began.diagnostic_episode_changes()[0].cause())
            .is_ok()
    );
    assert!(
        resolved
            .provenance()
            .inspect(resolved.diagnostic_episode_changes()[0].cause())
            .is_ok()
    );
    match active.current().evidence() {
        ProblemEvidence::RuntimeLevelLatchConflictRetained { evidence, .. } => {
            assert_eq!(evidence.node, NodeEvidence::Node(f.node));
            assert_eq!(evidence.at_ticks, 2);
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn ordinary_history_and_same_value_requests_preserve_state_and_causes() {
    let f = fixture(LOW, ConflictPolicy::SetDominant);
    let mut m = f.compiled.spawn(policy(1000));
    m.apply(Transaction::initialize(
        Time::from_ticks(0),
        m.revision(),
        snapshot(&f, LOW, LOW),
    ))
    .unwrap();
    for (t, set, reset, value, established_at) in [
        (1, HIGH, LOW, HIGH, 1),
        (2, HIGH, LOW, HIGH, 2),
        (3, LOW, LOW, HIGH, 2),
        (4, LOW, HIGH, LOW, 4),
        (5, LOW, LOW, LOW, 4),
    ] {
        let result = m
            .apply(Transaction::advance(
                Time::from_ticks(t),
                m.revision(),
                delta(&f, set, reset),
            ))
            .unwrap();
        let view = m.inspect_level_set_reset_latch(f.node).unwrap();
        assert_eq!(view.committed(), value);
        assert!(
            result
                .provenance()
                .inspect(view.latest_establishment())
                .is_ok()
        );
        let mossignal::CauseInspection::Derived { supporters, .. } = result
            .provenance()
            .inspect(view.latest_establishment())
            .unwrap()
        else {
            panic!("the latch's stored-level cause must be a derived record");
        };
        let transaction_times = supporters
            .iter()
            .filter_map(|cause| match result.provenance().inspect(*cause).unwrap() {
                mossignal::CauseInspection::ReadyTransaction { at, .. } => Some(at.ticks()),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(transaction_times, vec![established_at]);
    }
}

#[test]
fn fully_settled_upstream_state_drives_same_reaction_edge_detection() {
    let mut b =
        NetworkBuilder::<Domain>::with_key(NetworkKey::from_u128(10), TimeDomainId::from_u128(2));
    let (pulse, p) = b.pulse_input("toggle");
    let set = b.toggle(p, mossignal::ToggleConfig::new(LOW)).unwrap();
    let reset = b.constant(LOW);
    let latch = b
        .level_set_reset_latch(
            set,
            reset,
            LevelSetResetConfig::new(LOW, ConflictPolicy::RetainAndDiagnose),
        )
        .unwrap();
    let edge = b
        .any_edge(latch, EdgeConfig::new(EdgeInitialization::Assume(LOW)))
        .unwrap();
    let output = b.pulse_output("edge", edge).unwrap();
    let c = b
        .finish()
        .require_artifact()
        .unwrap()
        .compile()
        .require_artifact()
        .unwrap();
    let mut m = c.spawn(policy(1000));
    let r = m
        .apply(Transaction::initialize(
            Time::from_ticks(0),
            m.revision(),
            c.input_snapshot()
                .pulse(pulse, PulseCount::ONE)
                .unwrap()
                .finish()
                .unwrap(),
        ))
        .unwrap();
    assert!(r.output_events().iter().any(|e|matches!(e,mossignal::OutputEvent::Pulsed{output:key,count,..} if *key==output && *count==PulseCount::ONE)));
}

#[test]
fn episode_publication_obeys_event_budget_and_failure_atomicity() {
    let f = fixture(LOW, ConflictPolicy::RetainAndDiagnose);
    // Initialization creates one output establishment and one episode change.
    for limit in [1, 2, 3] {
        let mut m = f.compiled.spawn(policy(limit));
        let r = m.apply(Transaction::initialize(
            Time::from_ticks(0),
            m.revision(),
            snapshot(&f, HIGH, HIGH),
        ));
        assert_eq!(r.is_ok(), limit >= 2);
        if limit == 1 {
            assert_eq!(r.unwrap_err().code(), DiagnosticCode::RuntimeBudgetExceeded);
            assert!(!m.is_initialized());
        }
    }
}

#[test]
fn internal_deadlines_begin_and_resolve_in_one_outer_result_with_stepwise_equivalence() {
    let mut b =
        NetworkBuilder::<Domain>::with_key(NetworkKey::from_u128(20), TimeDomainId::from_u128(2));
    let (input, p) = b.pulse_input("schedule");
    let a = b
        .pulse_delay(
            p,
            mossignal::PulseDelayConfig::new(mossignal::time::NonZeroSpan::from_ticks(2).unwrap()),
        )
        .unwrap();
    let z = b
        .pulse_delay(
            p,
            mossignal::PulseDelayConfig::new(mossignal::time::NonZeroSpan::from_ticks(4).unwrap()),
        )
        .unwrap();
    let pulses = b.merge([a, z]).unwrap();
    let control = b.toggle(pulses, mossignal::ToggleConfig::new(LOW)).unwrap();
    let high = b.constant(HIGH);
    let latch = b
        .level_set_reset_latch(
            control,
            high,
            LevelSetResetConfig::new(LOW, ConflictPolicy::RetainAndDiagnose),
        )
        .unwrap();
    b.level_output("state", latch).unwrap();
    let c = b
        .finish()
        .require_artifact()
        .unwrap()
        .compile()
        .require_artifact()
        .unwrap();
    let init = || {
        c.input_snapshot()
            .pulse(input, PulseCount::ONE)
            .unwrap()
            .finish()
            .unwrap()
    };
    let mut direct = c.spawn(policy(1000));
    let mut step = c.spawn(policy(1000));
    for m in [&mut direct, &mut step] {
        m.apply(Transaction::initialize(
            Time::from_ticks(0),
            m.revision(),
            init(),
        ))
        .unwrap();
    }
    let result = direct
        .apply(Transaction::advance(
            Time::from_ticks(5),
            direct.revision(),
            c.input_delta().finish().unwrap(),
        ))
        .unwrap();
    let changes = |r: &mossignal::TransactionResult<Domain>| {
        r.diagnostic_episode_changes()
            .iter()
            .map(|e| (e.kind(), e.at().ticks(), e.identity()))
            .collect::<Vec<_>>()
    };
    let mut steps = Vec::new();
    for t in [2, 4, 5] {
        let r = step
            .apply(Transaction::advance(
                Time::from_ticks(t),
                step.revision(),
                c.input_delta().finish().unwrap(),
            ))
            .unwrap();
        steps.extend(changes(&r));
    }
    assert_eq!(changes(&result), steps);
    assert_eq!(
        result
            .diagnostic_episode_changes()
            .iter()
            .map(|e| (e.kind(), e.at().ticks()))
            .collect::<Vec<_>>(),
        vec![(Change::Began, 2), (Change::Resolved, 4)]
    );
    assert!(direct.active_diagnostic_episodes().unwrap().is_empty());
    assert!(step.active_diagnostic_episodes().unwrap().is_empty());
    assert_eq!(direct.schedule(), step.schedule());
    for e in result.diagnostic_episode_changes() {
        assert!(result.provenance().inspect(e.cause()).is_ok());
    }
}

fn module(initial: LogicLevel, conflict: ConflictPolicy) -> mossignal::ModuleDef<Domain> {
    use mossignal::key::{ModuleInputKey, ModuleOutputKey};
    let mut b = mossignal::ModuleBuilder::new();
    let s = b
        .add_level_input(ModuleInputKey::from_u128(1), DiagnosticMeta::default())
        .unwrap();
    let r = b
        .add_level_input(ModuleInputKey::from_u128(2), DiagnosticMeta::default())
        .unwrap();
    let node = b
        .add_level_set_reset_latch(
            NodeKey::from_u128(10),
            s,
            r,
            LevelSetResetConfig::new(initial, conflict),
            DiagnosticMeta::default(),
        )
        .unwrap();
    b.add_level_output(
        ModuleOutputKey::from_u128(3),
        node.into_outputs(),
        DiagnosticMeta::default(),
    )
    .unwrap();
    b.finish().require_artifact().unwrap()
}

#[test]
fn nested_module_episodes_have_independent_qualified_identity_and_deterministic_order() {
    use mossignal::key::{ModuleInputKey, ModuleInstanceKey, ModuleOutputKey};
    let leaf = module(LOW, ConflictPolicy::RetainAndDiagnose);
    let mut parent = mossignal::ModuleBuilder::new();
    let s = parent
        .add_level_input(ModuleInputKey::from_u128(1), DiagnosticMeta::default())
        .unwrap();
    let r = parent
        .add_level_input(ModuleInputKey::from_u128(2), DiagnosticMeta::default())
        .unwrap();
    let inner = parent
        .instantiate(
            &leaf,
            ModuleInstanceKey::from_u128(50),
            DiagnosticMeta::default(),
        )
        .unwrap()
        .bind_level(ModuleInputKey::from_u128(1), s)
        .unwrap()
        .bind_level(ModuleInputKey::from_u128(2), r)
        .unwrap()
        .finish()
        .unwrap();
    parent
        .add_level_output(
            ModuleOutputKey::from_u128(3),
            inner.level_output(ModuleOutputKey::from_u128(3)).unwrap(),
            DiagnosticMeta::default(),
        )
        .unwrap();
    let parent = parent.finish().require_artifact().unwrap();
    let run = |reverse: bool| {
        let mut b = NetworkBuilder::with_key(NetworkKey::from_u128(70), TimeDomainId::from_u128(2));
        let set = ExternalInputKey::from_u128(1);
        let reset = ExternalInputKey::from_u128(2);
        let s = b.add_level_input(set, DiagnosticMeta::default()).unwrap();
        let r = b.add_level_input(reset, DiagnosticMeta::default()).unwrap();
        for id in if reverse { [200, 100] } else { [100, 200] } {
            let i = b
                .instantiate(
                    &parent,
                    ModuleInstanceKey::from_u128(id),
                    DiagnosticMeta::default(),
                )
                .unwrap()
                .bind_level(ModuleInputKey::from_u128(1), s)
                .unwrap()
                .bind_level(ModuleInputKey::from_u128(2), r)
                .unwrap()
                .finish()
                .unwrap();
            b.add_level_output(
                ExternalOutputKey::from_u128(id),
                i.level_output(ModuleOutputKey::from_u128(3)).unwrap(),
                DiagnosticMeta::default(),
            )
            .unwrap();
        }
        let c = b
            .finish()
            .require_artifact()
            .unwrap()
            .compile()
            .require_artifact()
            .unwrap();
        let mut m = c.spawn(policy(1000));
        let result = m
            .apply(Transaction::initialize(
                Time::from_ticks(1),
                m.revision(),
                c.input_snapshot()
                    .set(set, HIGH)
                    .unwrap()
                    .set(reset, HIGH)
                    .unwrap()
                    .finish()
                    .unwrap(),
            ))
            .unwrap();
        let episodes = m.active_diagnostic_episodes().unwrap();
        assert_eq!(episodes.len(), 2);
        assert_ne!(episodes[0].identity(), episodes[1].identity());
        for (id, episode) in [100, 200].into_iter().zip(&episodes) {
            match episode.current().evidence() {
                ProblemEvidence::RuntimeLevelLatchConflictRetained { evidence, .. } => assert_eq!(
                    evidence.node,
                    NodeEvidence::Qualified {
                        instances: vec![
                            ModuleInstanceKey::from_u128(id),
                            ModuleInstanceKey::from_u128(50)
                        ],
                        node: NodeKey::from_u128(10)
                    }
                ),
                other => panic!("{other:?}"),
            }
            assert!(episode.provenance().inspect(episode.cause()).is_ok());
            let inspect = m.inspect_module(ModuleInstanceKey::from_u128(id)).unwrap();
            let node = inspect
                .nodes()
                .iter()
                .find(|n| n.node().node() == NodeKey::from_u128(10))
                .unwrap();
            assert_eq!(node.level_set_reset_state(), Some(LOW));
            assert!(node.level_set_reset_cause().is_some());
            assert!(node.pulse_set_reset_cause().is_none());
        }
        for change in result.diagnostic_episode_changes() {
            assert!(result.provenance().inspect(change.cause()).is_ok());
        }
        assert!(result.occurrences().is_empty());
        (
            c.fingerprint(),
            episodes
                .into_iter()
                .map(|e| e.identity())
                .collect::<Vec<_>>(),
        )
    };
    assert_eq!(run(false), run(true));
}

#[test]
fn network_and_module_identities_include_initial_state_and_policy() {
    let mut networks = std::collections::BTreeSet::new();
    let mut modules = std::collections::BTreeSet::new();
    for initial in [LOW, HIGH] {
        for conflict in POLICIES {
            assert!(networks.insert(fixture(initial, conflict).compiled.fingerprint()));
            assert!(modules.insert(module(initial, conflict).fingerprint()));
        }
    }
    assert_eq!(networks.len(), 8);
    assert_eq!(modules.len(), 8);
}

#[test]
fn direct_and_module_latches_keep_independent_state_and_episodes() {
    use mossignal::key::{ModuleInputKey, ModuleInstanceKey, ModuleOutputKey};
    let leaf = module(LOW, ConflictPolicy::RetainAndDiagnose);
    let mut b = NetworkBuilder::with_key(NetworkKey::from_u128(71), TimeDomainId::from_u128(2));
    let (set, s) = b.level_input("first set");
    let (reset, r) = b.level_input("first reset");
    let high = b.constant(HIGH);
    b.add_level_set_reset_latch(
        NodeKey::from_u128(10),
        high,
        high,
        LevelSetResetConfig::new(LOW, ConflictPolicy::RetainAndDiagnose),
        DiagnosticMeta::default(),
    )
    .unwrap();
    for (id, controls) in [(100, [s, r]), (200, [high, high])] {
        let instance = b
            .instantiate(
                &leaf,
                ModuleInstanceKey::from_u128(id),
                DiagnosticMeta::default(),
            )
            .unwrap()
            .bind_level(ModuleInputKey::from_u128(1), controls[0])
            .unwrap()
            .bind_level(ModuleInputKey::from_u128(2), controls[1])
            .unwrap()
            .finish()
            .unwrap();
        b.add_level_output(
            ExternalOutputKey::from_u128(id),
            instance
                .level_output(ModuleOutputKey::from_u128(3))
                .unwrap(),
            DiagnosticMeta::default(),
        )
        .unwrap();
    }
    let c = b
        .finish()
        .require_artifact()
        .unwrap()
        .compile()
        .require_artifact()
        .unwrap();
    let mut m = c.spawn(policy(1000));
    m.apply(Transaction::initialize(
        Time::from_ticks(0),
        m.revision(),
        c.input_snapshot()
            .set(set, HIGH)
            .unwrap()
            .set(reset, HIGH)
            .unwrap()
            .finish()
            .unwrap(),
    ))
    .unwrap();
    let original = m.active_diagnostic_episodes().unwrap();
    assert_eq!(original.len(), 3);
    let first = original
        .iter()
        .find(|e| {
            matches!(e.condition().owner(), mossignal::NodeSubject::Qualified(node)
        if node.instances() == [ModuleInstanceKey::from_u128(100)])
        })
        .unwrap();
    let resolved = m
        .apply(Transaction::advance(
            Time::from_ticks(1),
            m.revision(),
            c.input_delta().set(reset, LOW).unwrap().finish().unwrap(),
        ))
        .unwrap();
    assert_eq!(resolved.diagnostic_episode_changes().len(), 1);
    let change = &resolved.diagnostic_episode_changes()[0];
    assert_eq!(change.kind(), Change::Resolved);
    assert_eq!(change.identity(), first.identity());
    let remaining = m.active_diagnostic_episodes().unwrap();
    assert_eq!(remaining.len(), 2);
    assert!(remaining.iter().all(|e| e.identity() != first.identity()));
    for episode in &remaining {
        let before = original
            .iter()
            .find(|e| e.identity() == episode.identity())
            .unwrap();
        assert_eq!(episode.current(), before.current());
        assert_eq!(episode.began_at(), before.began_at());
    }
    assert_eq!(
        m.output_level(ExternalOutputKey::from_u128(100)),
        Some(HIGH)
    );
    assert_eq!(m.output_level(ExternalOutputKey::from_u128(200)), Some(LOW));
    assert_eq!(
        m.inspect_level_set_reset_latch(NodeKey::from_u128(10))
            .unwrap()
            .committed(),
        LOW
    );
}

#[test]
fn module_condition_keys_ignore_private_flattened_node_allocation() {
    use mossignal::key::{ModuleInputKey, ModuleInstanceKey};
    let leaf = module(LOW, ConflictPolicy::RetainAndDiagnose);
    let run = |extra: bool| {
        let mut b = NetworkBuilder::with_key(NetworkKey::from_u128(72), TimeDomainId::from_u128(2));
        let high = b
            .add_constant(NodeKey::from_u128(500), HIGH, DiagnosticMeta::default())
            .unwrap()
            .into_outputs();
        if extra {
            // Claiming private lowering's first candidate shifts its flat key, not the owner.
            b.add_constant(NodeKey::from_u128(0), LOW, DiagnosticMeta::default())
                .unwrap();
        }
        b.instantiate(
            &leaf,
            ModuleInstanceKey::from_u128(100),
            DiagnosticMeta::default(),
        )
        .unwrap()
        .bind_level(ModuleInputKey::from_u128(1), high)
        .unwrap()
        .bind_level(ModuleInputKey::from_u128(2), high)
        .unwrap()
        .finish()
        .unwrap();
        let c = b
            .finish()
            .require_artifact()
            .unwrap()
            .compile()
            .require_artifact()
            .unwrap();
        let mut m = c.spawn(policy(1000));
        m.apply(Transaction::initialize(
            Time::from_ticks(0),
            m.revision(),
            c.input_snapshot().finish().unwrap(),
        ))
        .unwrap();
        m.active_diagnostic_episodes().unwrap().remove(0)
    };
    let ordinary = run(false);
    let shifted = run(true);
    assert_eq!(ordinary.identity(), shifted.identity());
    assert_eq!(ordinary.condition(), shifted.condition());
}

fn dynamic(
    roles: Vec<mossignal::authored::InputPortRole>,
    reverse: bool,
    feedback: Option<usize>,
) -> mossignal::authored::UncheckedNetwork<Domain> {
    use mossignal::authored::{
        ConnectionDef, ConnectionEndpoint, ExternalInputDef, ExternalOutputDef, NodeDef, NodeKind,
        NodePorts, UncheckedNetwork,
    };
    use mossignal::key::{ConnectionKey, InPortKey, OutPortKey, SignalSourceKey};
    let inputs = [
        ExternalInputKey::<Level>::from_u128(1),
        ExternalInputKey::from_u128(2),
    ];
    let ports = [InPortKey::<Level>::from_u128(4), InPortKey::from_u128(5)];
    let output = OutPortKey::<Level>::from_u128(6);
    let mut connections = (0..2)
        .map(|i| {
            ConnectionDef::new(
                ConnectionKey::from_u128(10 + i as u128),
                if feedback == Some(i) {
                    ConnectionEndpoint::node_output(output.into())
                } else {
                    ConnectionEndpoint::external_input(inputs[i].into())
                },
                ConnectionEndpoint::node_input(ports[i].into()),
                DiagnosticMeta::default(),
            )
        })
        .collect::<Vec<_>>();
    if reverse {
        connections.reverse();
    }
    UncheckedNetwork::new(
        NetworkKey::from_u128(8),
        TimeDomainId::from_u128(2),
        DiagnosticMeta::default(),
        vec![NodeDef::new(
            NodeKey::from_u128(3),
            NodeKind::level_set_reset_latch(LevelSetResetConfig::new(
                LOW,
                ConflictPolicy::SetDominant,
            )),
            NodePorts::with_input_roles(
                ports.into_iter().map(Into::into).collect(),
                roles,
                vec![output.into()],
            ),
            DiagnosticMeta::default(),
        )],
        inputs
            .into_iter()
            .map(|i| ExternalInputDef::new(i.into(), DiagnosticMeta::default()))
            .collect(),
        vec![ExternalOutputDef::new(
            ExternalOutputKey::<Level>::from_u128(7).into(),
            SignalSourceKey::NodeOutput(output).into(),
            DiagnosticMeta::default(),
        )],
        connections,
    )
}

#[test]
fn dynamic_roles_law_identity_and_both_current_dependency_cycles_are_checked() {
    use mossignal::authored::InputPortRole::{Reset, Set};
    let mut fingerprints = Vec::new();
    for reverse in [false, true] {
        let c = dynamic(vec![Set, Reset], reverse, None)
            .validate()
            .require_artifact()
            .unwrap()
            .compile()
            .require_artifact()
            .unwrap();
        fingerprints.push(c.fingerprint());
        let mut m = c.spawn(policy(1000));
        m.apply(Transaction::initialize(
            Time::from_ticks(0),
            m.revision(),
            c.input_snapshot()
                .set(ExternalInputKey::from_u128(1), HIGH)
                .unwrap()
                .set(ExternalInputKey::from_u128(2), LOW)
                .unwrap()
                .finish()
                .unwrap(),
        ))
        .unwrap();
        assert_eq!(m.output_level(ExternalOutputKey::from_u128(7)), Some(HIGH));
    }
    assert_eq!(fingerprints[0], fingerprints[1]);
    assert_ne!(
        fingerprints[0],
        dynamic(vec![Reset, Set], false, None)
            .validate()
            .require_artifact()
            .unwrap()
            .fingerprint()
    );
    assert!(
        dynamic(vec![Set, Set], false, None)
            .validate()
            .require_artifact()
            .is_err()
    );
    assert!(
        dynamic(vec![Set], false, None)
            .validate()
            .require_artifact()
            .is_err()
    );
    for feedback in [0, 1] {
        let r = dynamic(vec![Set, Reset], false, Some(feedback)).validate();
        assert!(
            r.diagnostics()
                .iter()
                .any(|d| d.problem().code() == DiagnosticCode::ValidationCurrentReactionCycle)
        );
    }
}

#[test]
fn typed_foreign_signals_and_duplicate_fixed_identities_are_rejected() {
    use mossignal::key::{InPortKey, OutPortKey};
    let mut a = NetworkBuilder::<Domain>::new(TimeDomainId::from_u128(1));
    let local = a.constant(LOW);
    let mut b = NetworkBuilder::<Domain>::new(TimeDomainId::from_u128(1));
    let foreign = b.constant(HIGH);
    let config = LevelSetResetConfig::new(LOW, ConflictPolicy::SetDominant);
    assert!(a.level_set_reset_latch(local, foreign, config).is_err());
    assert!(
        a.add_level_set_reset_latch_with_ports(
            NodeKey::from_u128(999),
            InPortKey::from_u128(100),
            InPortKey::from_u128(100),
            OutPortKey::from_u128(100),
            local,
            local,
            config,
            DiagnosticMeta::default()
        )
        .is_err()
    );
    a.add_level_set_reset_latch(
        NodeKey::from_u128(999),
        local,
        local,
        config,
        DiagnosticMeta::default(),
    )
    .unwrap();
    assert!(
        a.add_level_set_reset_latch(
            NodeKey::from_u128(999),
            local,
            local,
            config,
            DiagnosticMeta::default()
        )
        .is_err()
    );
}

#[test]
fn malformed_level_latch_control_kind_is_a_structural_failure() {
    use mossignal::authored::{InputPortRole, NodeDef, NodeKind, NodePorts, UncheckedNetwork};
    use mossignal::key::{InPortKey, OutPortKey};
    let malformed = UncheckedNetwork::<Domain>::new(
        NetworkKey::from_u128(1),
        TimeDomainId::from_u128(2),
        DiagnosticMeta::default(),
        vec![NodeDef::new(
            NodeKey::from_u128(1),
            NodeKind::level_set_reset_latch(LevelSetResetConfig::new(
                LOW,
                ConflictPolicy::RetainAndDiagnose,
            )),
            NodePorts::with_input_roles(
                vec![
                    InPortKey::<mossignal::signal::Pulse>::from_u128(1).into(),
                    InPortKey::<Level>::from_u128(2).into(),
                ],
                vec![InputPortRole::Set, InputPortRole::Reset],
                vec![OutPortKey::<Level>::from_u128(3).into()],
            ),
            DiagnosticMeta::default(),
        )],
        vec![],
        vec![],
        vec![],
    );
    let report = malformed.validate();
    assert!(
        report
            .diagnostics()
            .iter()
            .any(|d| d.problem().code() == DiagnosticCode::ValidationInvalidFixedArity)
    );
    assert!(report.require_artifact().is_err());
}

#[test]
fn episode_identity_ignores_other_nodes_and_private_state_slot_positions() {
    use mossignal::key::{InPortKey, OutPortKey};
    let run = |extra: bool| {
        let mut b = NetworkBuilder::<Domain>::with_key(
            NetworkKey::from_u128(123),
            TimeDomainId::from_u128(2),
        );
        let s = b
            .add_level_input(ExternalInputKey::from_u128(1), DiagnosticMeta::default())
            .unwrap();
        let r = b
            .add_level_input(ExternalInputKey::from_u128(2), DiagnosticMeta::default())
            .unwrap();
        if extra {
            let (_, p) = b.pulse_input("unrelated");
            b.add_toggle(
                NodeKey::from_u128(1),
                p,
                mossignal::ToggleConfig::new(HIGH),
                DiagnosticMeta::default(),
            )
            .unwrap();
        }
        b.add_level_set_reset_latch_with_ports(
            NodeKey::from_u128(100),
            InPortKey::from_u128(100),
            InPortKey::from_u128(101),
            OutPortKey::from_u128(100),
            s,
            r,
            LevelSetResetConfig::new(LOW, ConflictPolicy::RetainAndDiagnose),
            DiagnosticMeta::default(),
        )
        .unwrap();
        let c = b
            .finish()
            .require_artifact()
            .unwrap()
            .compile()
            .require_artifact()
            .unwrap();
        let mut m = c.spawn(policy(1000));
        m.apply(Transaction::initialize(
            Time::from_ticks(1),
            m.revision(),
            c.input_snapshot()
                .set(ExternalInputKey::from_u128(1), HIGH)
                .unwrap()
                .set(ExternalInputKey::from_u128(2), HIGH)
                .unwrap()
                .finish()
                .unwrap(),
        ))
        .unwrap();
        (
            m.active_diagnostic_episodes().unwrap().remove(0).identity(),
            c.fingerprint(),
        )
    };
    let simple = run(false);
    let expanded = run(true);
    assert_eq!(simple.0, expanded.0);
    assert_ne!(simple.1, expanded.1);
}
