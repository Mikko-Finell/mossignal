//! Read one committed occurrence, follow an active condition, and consume its end.

use mossignal::diagnostics::{DiagnosticCode, ProblemEvidence};
use mossignal::key::{ExternalInputKey, NodeKey};
use mossignal::metadata::DiagnosticMeta;
use mossignal::signal::{LogicLevel, Pulse, PulseCount};
use mossignal::time::Time;
use mossignal::{
    ConflictPolicy, DiagnosticEpisodeChangeKind, DiagnosticScope, LevelSetResetConfig,
    NetworkBuilder, PulseSetResetConfig, ReconfigurationPolicy, RuntimePolicy, TimeDomainId,
    Transaction, TransactionResult,
};

fn consume_changes(result: &TransactionResult<()>, scope: &DiagnosticScope) {
    for change in result.diagnostic_episode_changes_for(scope) {
        match change.kind() {
            DiagnosticEpisodeChangeKind::Began => {
                println!("condition began at {}", change.at().ticks())
            }
            DiagnosticEpisodeChangeKind::Changed => {
                println!("condition evidence changed at {}", change.at().ticks())
            }
            DiagnosticEpisodeChangeKind::Resolved => {
                println!("condition resolved at {}", change.at().ticks())
            }
            DiagnosticEpisodeChangeKind::Terminated => {
                println!("condition owner was removed at {}", change.at().ticks())
            }
            _ => {}
        }
        assert!(result.provenance().inspect(change.cause()).is_ok());
    }
}

fn main() {
    let mut builder = NetworkBuilder::<()>::new(TimeDomainId::from_u128(1));
    let (set, set_signal) = builder.level_input("set");
    let (reset, reset_signal) = builder.level_input("reset");
    let pulse_set = ExternalInputKey::<Pulse>::from_u128(2);
    let pulse_reset = ExternalInputKey::<Pulse>::from_u128(3);
    let ps = builder
        .add_pulse_input(pulse_set, DiagnosticMeta::default())
        .unwrap();
    let pr = builder
        .add_pulse_input(pulse_reset, DiagnosticMeta::default())
        .unwrap();
    let persistent_node = NodeKey::from_u128(4);
    let transient_node = NodeKey::from_u128(5);
    builder
        .add_level_set_reset_latch(
            persistent_node,
            set_signal,
            reset_signal,
            LevelSetResetConfig::new(LogicLevel::Low, ConflictPolicy::RetainAndDiagnose),
            DiagnosticMeta::default(),
        )
        .unwrap();
    builder
        .add_pulse_set_reset_latch(
            transient_node,
            ps,
            pr,
            PulseSetResetConfig::new(LogicLevel::Low, ConflictPolicy::RetainAndDiagnose),
            DiagnosticMeta::default(),
        )
        .unwrap();
    let network = builder
        .finish()
        .require_artifact()
        .unwrap()
        .compile()
        .require_artifact()
        .unwrap();
    let policy = RuntimePolicy::builder()
        .max_internal_reactions(100)
        .max_evaluated_operations(1000)
        .max_pending_events(100)
        .max_events_created_per_transaction(100)
        .max_required_provenance_growth(1000)
        .build()
        .unwrap();
    let mut machine = network.spawn(policy);
    let active_scope = DiagnosticScope::node(persistent_node);
    let snapshot = network
        .input_snapshot()
        .set(set, LogicLevel::High)
        .unwrap()
        .set(reset, LogicLevel::High)
        .unwrap()
        .pulse(pulse_set, PulseCount::new(2))
        .unwrap()
        .pulse(pulse_reset, PulseCount::new(3))
        .unwrap()
        .finish()
        .unwrap();
    let began = machine
        .apply(Transaction::initialize(
            Time::from_ticks(0),
            machine.revision(),
            snapshot,
        ))
        .unwrap();

    // Occurrences belong to this result. Their exact structured control evidence
    // remains available after the machine advances; they create no active episode.
    let transient_scope = DiagnosticScope::node(transient_node);
    let occurrences = began.occurrences_for(&transient_scope);
    assert_eq!(occurrences.len(), 1);
    assert_eq!(
        occurrences[0].problem().code(),
        DiagnosticCode::RuntimePulseLatchConflictRetained
    );
    if let ProblemEvidence::RuntimePulseLatchConflictRetained { evidence, .. } =
        occurrences[0].problem().evidence()
    {
        println!(
            "transient conflict at {}: {:?}",
            occurrences[0].at().ticks(),
            evidence.controls
        );
    }
    consume_changes(&began, &active_scope);

    let active = machine
        .active_diagnostic_episodes_for(&active_scope)
        .unwrap()
        .remove(0);
    assert_eq!(active.began_at().ticks(), 0);
    let quiet = machine
        .apply(Transaction::advance(
            Time::from_ticks(1),
            machine.revision(),
            network.input_delta().finish().unwrap(),
        ))
        .unwrap();
    assert!(
        quiet
            .diagnostic_episode_changes_for(&active_scope)
            .is_empty()
    );
    assert_eq!(
        machine
            .active_diagnostic_episodes_for(&active_scope)
            .unwrap()[0]
            .identity(),
        active.identity()
    );

    // Clearing the actual conflict resolves the condition through ordinary execution.
    let delta = network
        .input_delta()
        .set(reset, LogicLevel::Low)
        .unwrap()
        .finish()
        .unwrap();
    let resolved = machine
        .apply(Transaction::advance(
            Time::from_ticks(2),
            machine.revision(),
            delta,
        ))
        .unwrap();
    assert_eq!(
        resolved.diagnostic_episode_changes_for(&active_scope)[0].kind(),
        DiagnosticEpisodeChangeKind::Resolved
    );
    consume_changes(&resolved, &active_scope);
    assert!(
        machine
            .active_diagnostic_episodes_for(&active_scope)
            .unwrap()
            .is_empty()
    );

    let delta = network
        .input_delta()
        .set(reset, LogicLevel::High)
        .unwrap()
        .finish()
        .unwrap();
    machine
        .apply(Transaction::advance(
            Time::from_ticks(3),
            machine.revision(),
            delta,
        ))
        .unwrap();
    let definition = machine.inspect_node(persistent_node).unwrap().definition;
    let mut patch = machine.patch().remove_node(persistent_node).unwrap();
    for connection in network.graph().connections() {
        if matches!(connection.to(), mossignal::authored::ConnectionEndpoint::NodeInput(port) if definition.ports().inputs().contains(&port))
        {
            patch = patch.remove_connection(connection.key()).unwrap();
        }
    }
    let prepared = machine
        .prepare_patch(patch.finish())
        .require_artifact()
        .unwrap();
    let delta = prepared.input_delta().finish().unwrap();
    let terminated = machine
        .apply(
            Transaction::advance(Time::from_ticks(4), machine.revision(), delta)
                .with_patch(prepared, ReconfigurationPolicy::AllowReportedStateLoss)
                .unwrap(),
        )
        .unwrap();
    assert_eq!(
        terminated.diagnostic_episode_changes_for(&active_scope)[0].kind(),
        DiagnosticEpisodeChangeKind::Terminated
    );
    consume_changes(&terminated, &active_scope);
    assert!(
        machine
            .active_diagnostic_episodes_for(&active_scope)
            .is_err()
    );
    assert!(active.provenance().inspect(active.cause()).is_ok());
}

#[test]
fn diagnostic_usage_runs() {
    main();
}
