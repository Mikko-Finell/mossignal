use mossignal::diagnostics::{DiagnosticCode, ProblemEvidence, SubjectRef};
use mossignal::key::*;
use mossignal::metadata::DiagnosticMeta;
use mossignal::signal::{LogicLevel, PulseCount};
use mossignal::time::Time;
use mossignal::*;

fn policy(events: u64) -> RuntimePolicy {
    RuntimePolicy::builder()
        .max_internal_reactions(100)
        .max_evaluated_operations(100_000)
        .max_pending_events(100)
        .max_events_created_per_transaction(events)
        .max_required_provenance_growth(100_000)
        .build()
        .unwrap()
}
fn meta() -> DiagnosticMeta {
    DiagnosticMeta::default()
}
fn compile(builder: NetworkBuilder<()>) -> CompiledNetwork<()> {
    builder
        .finish()
        .require_artifact()
        .unwrap()
        .compile()
        .require_artifact()
        .unwrap()
}

fn leaf() -> ModuleDef<()> {
    let mut builder = ModuleBuilder::new();
    let set = builder
        .add_level_input(ModuleInputKey::from_u128(1), meta())
        .unwrap();
    let reset = builder
        .add_level_input(ModuleInputKey::from_u128(2), meta())
        .unwrap();
    let ps = builder
        .add_pulse_input(ModuleInputKey::from_u128(3), meta())
        .unwrap();
    let pr = builder
        .add_pulse_input(ModuleInputKey::from_u128(4), meta())
        .unwrap();
    builder
        .level_set_reset_latch(
            set,
            reset,
            LevelSetResetConfig::new(LogicLevel::Low, ConflictPolicy::RetainAndDiagnose),
        )
        .unwrap();
    builder
        .pulse_set_reset_latch(
            ps,
            pr,
            PulseSetResetConfig::new(LogicLevel::Low, ConflictPolicy::RetainAndDiagnose),
        )
        .unwrap();
    builder.finish().require_artifact().unwrap()
}

#[test]
fn nested_scopes_keep_primitive_ownership_and_owned_records_without_affecting_state() {
    let leaf = leaf();
    let mut outer = ModuleBuilder::new();
    let set = outer
        .add_level_input(ModuleInputKey::from_u128(1), meta())
        .unwrap();
    let reset = outer
        .add_level_input(ModuleInputKey::from_u128(2), meta())
        .unwrap();
    let ps = outer
        .add_pulse_input(ModuleInputKey::from_u128(3), meta())
        .unwrap();
    let pr = outer
        .add_pulse_input(ModuleInputKey::from_u128(4), meta())
        .unwrap();
    for key in [100, 101] {
        outer
            .instantiate(&leaf, ModuleInstanceKey::from_u128(key), meta())
            .unwrap()
            .bind_level(ModuleInputKey::from_u128(1), set)
            .unwrap()
            .bind_level(ModuleInputKey::from_u128(2), reset)
            .unwrap()
            .bind_pulse(ModuleInputKey::from_u128(3), ps)
            .unwrap()
            .bind_pulse(ModuleInputKey::from_u128(4), pr)
            .unwrap()
            .finish()
            .unwrap();
    }
    let outer = outer.finish().require_artifact().unwrap();
    let mut builder = NetworkBuilder::new(TimeDomainId::from_u128(1));
    let (s, set) = builder.level_input("set");
    let (r, reset) = builder.level_input("reset");
    let (p, ps) = builder.pulse_input("pulse set");
    let (q, pr) = builder.pulse_input("pulse reset");
    for key in [200, 201] {
        builder
            .instantiate(&outer, ModuleInstanceKey::from_u128(key), meta())
            .unwrap()
            .bind_level(ModuleInputKey::from_u128(1), set)
            .unwrap()
            .bind_level(ModuleInputKey::from_u128(2), reset)
            .unwrap()
            .bind_pulse(ModuleInputKey::from_u128(3), ps)
            .unwrap()
            .bind_pulse(ModuleInputKey::from_u128(4), pr)
            .unwrap()
            .finish()
            .unwrap();
    }
    let compiled = compile(builder);
    let mut machine = compiled.spawn(policy(100));
    let scope = DiagnosticScope::module(ModuleInstanceKey::from_u128(200));
    assert_eq!(
        machine
            .active_diagnostic_episodes_for(&scope)
            .err()
            .unwrap()
            .code(),
        DiagnosticCode::LifecycleNotInitialized
    );
    let snapshot = compiled
        .input_snapshot()
        .set(s, LogicLevel::High)
        .unwrap()
        .set(r, LogicLevel::High)
        .unwrap()
        .pulse(p, PulseCount::new(2))
        .unwrap()
        .pulse(q, PulseCount::new(5))
        .unwrap()
        .finish()
        .unwrap();
    let result = machine
        .apply(Transaction::initialize(
            Time::from_ticks(0),
            machine.revision(),
            snapshot,
        ))
        .unwrap();
    let before = machine.snapshot();
    assert_eq!(result.occurrences().len(), 4);
    assert_eq!(result.occurrences_for(&scope).len(), 2);
    assert_eq!(result.diagnostic_episode_changes_for(&scope).len(), 2);
    let active = machine.active_diagnostic_episodes_for(&scope).unwrap();
    assert_eq!(active.len(), 2);
    let inner = DiagnosticScope::Module(
        QualifiedModuleRef::from_instances(vec![
            ModuleInstanceKey::from_u128(200),
            ModuleInstanceKey::from_u128(100),
        ])
        .unwrap(),
    );
    assert_eq!(result.occurrences_for(&inner).len(), 1);
    assert_eq!(
        machine
            .active_diagnostic_episodes_for(&inner)
            .unwrap()
            .len(),
        1
    );
    let record = result.occurrences_for(&inner)[0];
    let SubjectRef::QualifiedNode(owner) = record.problem().primary() else {
        panic!("primitive qualified owner required");
    };
    let node_scope = DiagnosticScope::Node(NodeSubject::Qualified(owner.clone()));
    assert_eq!(result.occurrences_for(&node_scope).len(), 1);
    assert!(
        machine
            .active_diagnostic_episodes_for(&node_scope)
            .unwrap()
            .is_empty()
    );
    let ProblemEvidence::RuntimePulseLatchConflictRetained { evidence, .. } =
        record.problem().evidence()
    else {
        panic!("exact typed evidence required");
    };
    assert_eq!(
        evidence.controls,
        ConflictControls::Pulse {
            set: PulseCount::new(2),
            reset: PulseCount::new(5)
        }
    );
    let persistent = DiagnosticScope::Node(active[0].condition().owner().clone());
    assert_eq!(
        machine
            .active_diagnostic_episodes_for(&persistent)
            .unwrap()
            .len(),
        1
    );
    let absent = DiagnosticScope::module(ModuleInstanceKey::from_u128(999));
    assert!(result.occurrences_for(&absent).is_empty());
    assert_eq!(
        machine
            .active_diagnostic_episodes_for(&absent)
            .err()
            .unwrap()
            .code(),
        DiagnosticCode::InspectionUnknownSubject
    );
    assert_eq!(machine.snapshot(), before);

    let transaction = Transaction::advance(
        Time::from_ticks(1),
        machine.revision(),
        compiled
            .input_delta()
            .set(r, LogicLevel::Low)
            .unwrap()
            .finish()
            .unwrap(),
    );
    let forecast = machine.forecast(transaction.clone()).unwrap();
    assert!(
        forecast
            .state()
            .active_diagnostic_episodes_for(&scope)
            .unwrap()
            .is_empty()
    );
    assert_eq!(machine.snapshot(), before);
    let resolved = machine.apply(transaction).unwrap();
    assert_eq!(resolved.diagnostic_episode_changes_for(&scope).len(), 2);
    for change in resolved.diagnostic_episode_changes_for(&scope) {
        assert_eq!(change.kind(), DiagnosticEpisodeChangeKind::Resolved);
        assert!(resolved.provenance().inspect(change.cause()).is_ok());
    }
    for record in active {
        assert_eq!(record.began_at().ticks(), 0);
        assert!(record.provenance().inspect(record.cause()).is_ok());
    }
    assert_eq!(result.occurrences_for(&inner).len(), 1);
}

#[test]
fn post_patch_snapshot_and_patch_free_replay_preserve_diagnostic_lifecycle() {
    let mut builder =
        NetworkBuilder::with_key(NetworkKey::from_u128(1), TimeDomainId::from_u128(2));
    let (set, s) = builder.level_input("set");
    let (reset, r) = builder.level_input("reset");
    let (ps, p) = builder.pulse_input("pulse set");
    let (pr, q) = builder.pulse_input("pulse reset");
    let node = NodeKey::from_u128(20);
    builder
        .add_level_set_reset_latch(
            node,
            s,
            r,
            LevelSetResetConfig::new(LogicLevel::Low, ConflictPolicy::RetainAndDiagnose),
            meta(),
        )
        .unwrap();
    builder
        .add_pulse_set_reset_latch(
            NodeKey::from_u128(21),
            p,
            q,
            PulseSetResetConfig::new(LogicLevel::Low, ConflictPolicy::RetainAndDiagnose),
            meta(),
        )
        .unwrap();
    let compiled = compile(builder);
    let mut machine = compiled.spawn(policy(100));
    machine
        .apply(Transaction::initialize(
            Time::from_ticks(0),
            machine.revision(),
            compiled
                .input_snapshot()
                .set(set, LogicLevel::High)
                .unwrap()
                .set(reset, LogicLevel::High)
                .unwrap()
                .finish()
                .unwrap(),
        ))
        .unwrap();
    let scope = DiagnosticScope::node(node);
    let active = machine
        .active_diagnostic_episodes_for(&scope)
        .unwrap()
        .remove(0);
    let patch = machine
        .patch()
        .set_diagnostic_meta(
            StructuralSubjectRef::Network(compiled.network_key()),
            DiagnosticMeta {
                name: Some("post patch".to_owned()),
                ..meta()
            },
        )
        .unwrap()
        .finish();
    let prepared = machine.prepare_patch(patch).require_artifact().unwrap();
    let target = prepared.resulting_compiled().clone();
    let migrated = machine
        .apply(
            Transaction::advance(
                Time::from_ticks(1),
                machine.revision(),
                prepared.input_delta().finish().unwrap(),
            )
            .with_patch(prepared, ReconfigurationPolicy::RejectStateLoss)
            .unwrap(),
        )
        .unwrap();
    let changed = migrated.diagnostic_episode_changes_for(&scope);
    assert_eq!(changed.len(), 1);
    assert_eq!(changed[0].kind(), DiagnosticEpisodeChangeKind::Changed);
    assert_eq!(changed[0].identity(), active.identity());
    assert!(migrated.provenance().inspect(changed[0].cause()).is_ok());
    assert_eq!(
        machine.active_diagnostic_episodes_for(&scope).unwrap()[0].identity(),
        active.identity()
    );
    assert_eq!(
        machine.active_diagnostic_episodes_for(&scope).unwrap()[0]
            .began_at()
            .ticks(),
        0
    );
    assert_eq!(
        machine.active_diagnostic_episodes_for(&scope).unwrap()[0]
            .last_material_change()
            .ticks(),
        1
    );
    let snapshot = machine.snapshot();
    let mut recorded = target.restore(snapshot.clone(), policy(100)).unwrap();
    let transactions = [
        Transaction::advance(
            Time::from_ticks(2),
            machine.revision(),
            target.input_delta().finish().unwrap(),
        ),
        Transaction::advance(
            Time::from_ticks(3),
            machine.revision(),
            target
                .input_delta()
                .set(reset, LogicLevel::Low)
                .unwrap()
                .finish()
                .unwrap(),
        ),
        Transaction::advance(
            Time::from_ticks(4),
            machine.revision(),
            target
                .input_delta()
                .set(reset, LogicLevel::High)
                .unwrap()
                .pulse(ps, PulseCount::new(2))
                .unwrap()
                .pulse(pr, PulseCount::new(7))
                .unwrap()
                .finish()
                .unwrap(),
        ),
    ];
    let mut direct = target.restore(snapshot.clone(), policy(100)).unwrap();
    let expected = transactions
        .iter()
        .cloned()
        .map(|transaction| direct.apply(transaction).unwrap())
        .collect::<Vec<_>>();
    let log = record_replay_log(&mut recorded, transactions).unwrap();
    let mut replayed = target.restore(snapshot, policy(100)).unwrap();
    let results = replayed.replay_log(&log).unwrap();
    for (actual, expected) in results.iter().zip(&expected) {
        assert_eq!(actual.occurrences(), expected.occurrences());
        let normalize = |result: &TransactionResult<()>| {
            result
                .diagnostic_episode_changes()
                .iter()
                .map(|change| {
                    (
                        change.identity(),
                        change.kind(),
                        change.at(),
                        change.before().cloned(),
                        change.after().cloned(),
                    )
                })
                .collect::<Vec<_>>()
        };
        assert_eq!(normalize(actual), normalize(expected));
        for change in actual.diagnostic_episode_changes_for(&scope) {
            assert!(actual.provenance().inspect(change.cause()).is_ok());
        }
    }
    assert!(results[0].diagnostic_episode_changes_for(&scope).is_empty());
    assert_eq!(
        results[1].diagnostic_episode_changes_for(&scope)[0].kind(),
        DiagnosticEpisodeChangeKind::Resolved
    );
    assert_eq!(
        results[2].diagnostic_episode_changes_for(&scope)[0].kind(),
        DiagnosticEpisodeChangeKind::Began
    );
    assert_eq!(results[2].occurrences().len(), 1);
    assert_eq!(
        direct.execution_state_digest(),
        replayed.execution_state_digest()
    );
    assert_eq!(
        direct.observable_state_digest(),
        replayed.observable_state_digest()
    );
    assert_eq!(
        replayed.active_diagnostic_episodes_for(&scope).unwrap()[0]
            .began_at()
            .ticks(),
        4
    );
}

#[test]
fn rejected_publication_leaves_scoped_active_state_unchanged() {
    let mut builder = NetworkBuilder::new(TimeDomainId::from_u128(1));
    let (set, s) = builder.level_input("set");
    let (reset, r) = builder.level_input("reset");
    let node = NodeKey::from_u128(2);
    builder
        .add_level_set_reset_latch(
            node,
            s,
            r,
            LevelSetResetConfig::new(LogicLevel::Low, ConflictPolicy::RetainAndDiagnose),
            meta(),
        )
        .unwrap();
    let compiled = compile(builder);
    let mut machine = compiled.spawn(policy(0));
    let scope = DiagnosticScope::node(node);
    machine
        .apply(Transaction::initialize(
            Time::from_ticks(0),
            machine.revision(),
            compiled
                .input_snapshot()
                .set(set, LogicLevel::Low)
                .unwrap()
                .set(reset, LogicLevel::Low)
                .unwrap()
                .finish()
                .unwrap(),
        ))
        .unwrap();
    let before = machine.snapshot();
    let failure = machine
        .apply(Transaction::advance(
            Time::from_ticks(1),
            machine.revision(),
            compiled
                .input_delta()
                .set(set, LogicLevel::High)
                .unwrap()
                .set(reset, LogicLevel::High)
                .unwrap()
                .finish()
                .unwrap(),
        ))
        .unwrap_err();
    assert_eq!(failure.code(), DiagnosticCode::RuntimeBudgetExceeded);
    assert_eq!(machine.snapshot(), before);
    assert!(
        machine
            .active_diagnostic_episodes_for(&scope)
            .unwrap()
            .is_empty()
    );
}
