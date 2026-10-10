//! Ownership preparation refinement. The reference copies collections instead of
//! extracting them, but deliberately shares the evaluator and publication logic.

use crate::key::{ExternalInputKey, NodeKey};
use crate::metadata::DiagnosticMeta;
use crate::signal::{Level, LogicLevel, Pulse, PulseCount};
use crate::time::{NonZeroSpan, Time};
use crate::*;

fn policy() -> RuntimePolicy {
    RuntimePolicy::builder()
        .max_internal_reactions(100)
        .max_evaluated_operations(100_000)
        .max_pending_events(100)
        .max_events_created_per_transaction(1_000)
        .max_required_provenance_growth(100_000)
        .build()
        .unwrap()
}

pub(super) fn fixture() -> CompiledNetwork<()> {
    let mut b = NetworkBuilder::new(TimeDomainId::from_u128(2));
    let pulse = b
        .add_pulse_input(ExternalInputKey::from_u128(1), DiagnosticMeta::default())
        .unwrap();
    let enable = b
        .add_level_input(ExternalInputKey::from_u128(2), DiagnosticMeta::default())
        .unwrap();
    let reset = b
        .add_level_input(ExternalInputKey::from_u128(3), DiagnosticMeta::default())
        .unwrap();
    let toggle = b
        .add_toggle(
            NodeKey::from_u128(10),
            pulse,
            ToggleConfig::new(LogicLevel::Low),
            DiagnosticMeta::default(),
        )
        .unwrap()
        .into_outputs();
    b.level_output("toggle", toggle).unwrap();
    let periodic = b
        .periodic(
            enable,
            PeriodicConfig::new(
                NonZeroSpan::from_ticks(5).unwrap(),
                FirstEmissionPolicy::AfterFirstPeriod,
                ReenablePhasePolicy::PreservePhase,
            ),
        )
        .unwrap();
    b.pulse_output("periodic", periodic).unwrap();
    let delayed = b
        .pulse_delay(
            pulse,
            PulseDelayConfig::new(NonZeroSpan::from_ticks(7).unwrap()),
        )
        .unwrap();
    b.pulse_output("delayed", delayed).unwrap();
    let latch = b
        .level_set_reset_latch(
            enable,
            reset,
            LevelSetResetConfig::new(LogicLevel::Low, ConflictPolicy::RetainAndDiagnose),
        )
        .unwrap();
    b.level_output("latch", latch).unwrap();
    b.finish()
        .require_artifact()
        .unwrap()
        .compile()
        .require_artifact()
        .unwrap()
}

pub(super) fn initialize(machine: &Machine<()>) -> Transaction<()> {
    Transaction::initialize(
        Time::from_ticks(0),
        machine.revision(),
        machine
            .compiled()
            .input_snapshot()
            .set(ExternalInputKey::from_u128(2), LogicLevel::High)
            .unwrap()
            .set(ExternalInputKey::from_u128(3), LogicLevel::High)
            .unwrap()
            .pulse(ExternalInputKey::from_u128(1), PulseCount::ONE)
            .unwrap()
            .finish()
            .unwrap(),
    )
}

fn advance(machine: &Machine<()>, at: u64, count: u64, reset: LogicLevel) -> Transaction<()> {
    Transaction::advance(
        Time::from_ticks(at),
        machine.revision(),
        machine
            .compiled()
            .input_delta()
            .set(ExternalInputKey::from_u128(3), reset)
            .unwrap()
            .pulse(ExternalInputKey::from_u128(1), PulseCount::new(count))
            .unwrap()
            .finish()
            .unwrap(),
    )
}

fn assert_results(
    network: &CompiledNetwork<()>,
    a: &TransactionResult<()>,
    b: &TransactionResult<()>,
) {
    assert_eq!(a.requested_time(), b.requested_time());
    assert_eq!(a.processed_reactions(), b.processed_reactions());
    assert_eq!(a.before_revision(), b.before_revision());
    assert_eq!(a.after_revision(), b.after_revision());
    assert_eq!(a.before_execution_digest(), b.before_execution_digest());
    assert_eq!(a.after_execution_digest(), b.after_execution_digest());
    assert_eq!(a.after_observable_digest(), b.after_observable_digest());
    assert_eq!(a.schedule(), b.schedule());
    assert_eq!(a.occurrences(), b.occurrences());
    assert_eq!(a.migration(), b.migration());
    let av = crate::state_digest_reference::canonical_view(network, a.provenance());
    let bv = crate::state_digest_reference::canonical_view(network, b.provenance());
    assert_eq!(av.records, bv.records);
    let events = |r: &TransactionResult<()>, v: &crate::state_digest_reference::CanonicalView| {
        r.output_events()
            .iter()
            .map(|event| match event {
                OutputEvent::LevelEstablished {
                    output,
                    value,
                    stamp,
                    cause,
                    revision,
                } => (
                    0,
                    output.as_u128(),
                    *value,
                    *value,
                    0,
                    *stamp,
                    *revision,
                    v.cause(*cause),
                ),
                OutputEvent::LevelChanged {
                    output,
                    from,
                    to,
                    stamp,
                    cause,
                    revision,
                } => (
                    1,
                    output.as_u128(),
                    *from,
                    *to,
                    0,
                    *stamp,
                    *revision,
                    v.cause(*cause),
                ),
                OutputEvent::Pulsed {
                    output,
                    count,
                    stamp,
                    cause,
                    revision,
                } => (
                    2,
                    output.as_u128(),
                    LogicLevel::Low,
                    LogicLevel::Low,
                    count.get(),
                    *stamp,
                    *revision,
                    v.cause(*cause),
                ),
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(events(a, &av), events(b, &bv));
    let episodes = |r: &TransactionResult<()>, v: &crate::state_digest_reference::CanonicalView| {
        r.diagnostic_episode_changes()
            .iter()
            .map(|change| {
                (
                    change.identity(),
                    change.kind(),
                    change.stamp(),
                    change.before().cloned(),
                    change.after().cloned(),
                    v.cause(change.cause()),
                )
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(episodes(a, &av), episodes(b, &bv));
}

fn compare(machine: &mut Machine<()>, reference: &mut Machine<()>, transaction: Transaction<()>) {
    assert_eq!(machine.snapshot(), reference.snapshot());
    let before = machine.execution_state_digest();
    let before_revision = machine.revision();
    let source = machine.snapshot();
    let optimized = machine.stage(transaction.clone());
    let copied = reference.stage_clone_reference(transaction);
    assert_eq!(machine.snapshot(), source);
    assert_eq!(reference.snapshot(), source);
    match (optimized, copied) {
        (Ok(a), Ok(b)) => {
            assert_eq!(a.result.before_execution_digest(), before);
            assert_eq!(a.result.before_revision(), before_revision);
            assert_results(a.successor.compiled(), &a.result, &b.result);
            assert_eq!(a.successor.snapshot(), b.successor.snapshot());
            assert_eq!(a.successor.status(), b.successor.status());
            assert_eq!(a.successor.last_reaction(), b.successor.last_reaction());
            assert_eq!(
                a.successor.execution_state_digest(),
                crate::state_digest_reference::execution_state_digest(&a.successor)
            );
            assert_eq!(
                a.successor.observable_state_digest(),
                crate::state_digest_reference::observable_state_digest(&a.successor)
            );
            a.publish(machine);
            b.publish(reference);
        }
        (Err(a), Err(b)) => {
            assert_eq!(a.evidence(), b.evidence());
            assert_eq!(a.problem(), b.problem());
        }
        (a, b) => panic!(
            "preparations diverged: optimized={:?}, copied={:?}",
            a.err(),
            b.err()
        ),
    }
}

#[test]
fn clone_preparation_matches_temporal_episode_patch_reset_and_restore_continuations() {
    let compiled = fixture();
    let mut machine = compiled.spawn(policy());
    let mut reference = compiled.spawn(policy());
    let init = initialize(&machine);
    compare(&mut machine, &mut reference, init);
    for (at, count, reset) in [
        (0, 0, LogicLevel::High),
        (1, 2, LogicLevel::High),
        (6, 3, LogicLevel::Low),
        (6, 0, LogicLevel::High),
    ] {
        let tx = advance(&machine, at, count, reset);
        compare(&mut machine, &mut reference, tx);
    }
    // Several source deadlines run before the preserving patch; the result uses
    // the predecessor's original identity, not a drained intermediate candidate.
    let prepared = machine
        .prepare_patch(
            machine
                .patch()
                .set_diagnostic_meta(
                    StructuralSubjectRef::Network(machine.compiled().network_key()),
                    DiagnosticMeta {
                        name: Some("preserved".to_owned()),
                        ..DiagnosticMeta::default()
                    },
                )
                .unwrap()
                .finish(),
        )
        .require_artifact()
        .unwrap();
    let tx = Transaction::advance(
        Time::from_ticks(16),
        machine.revision(),
        prepared.input_delta().finish().unwrap(),
    )
    .with_patch(prepared, ReconfigurationPolicy::RejectStateLoss)
    .unwrap();
    compare(&mut machine, &mut reference, tx);
    // Reset a real stored toggle while preserving the recurring calendar.
    let node = machine
        .compiled()
        .graph()
        .nodes()
        .iter()
        .find(|n| n.key() == NodeKey::from_u128(10))
        .unwrap()
        .clone();
    let prepared = machine
        .prepare_patch(
            machine
                .patch()
                .replace_node(NodeKey::from_u128(10), node, NodeMigrationDirective::Reset)
                .unwrap()
                .finish(),
        )
        .require_artifact()
        .unwrap();
    let tx = Transaction::advance(
        Time::from_ticks(21),
        machine.revision(),
        prepared.input_delta().finish().unwrap(),
    )
    .with_patch(prepared, ReconfigurationPolicy::AllowReportedStateLoss)
    .unwrap();
    compare(&mut machine, &mut reference, tx);
    machine = machine
        .compiled()
        .restore(machine.snapshot(), policy())
        .unwrap();
    reference = reference
        .compiled()
        .restore(reference.snapshot(), policy())
        .unwrap();
    let tx = advance(&machine, 27, 4, LogicLevel::Low);
    compare(&mut machine, &mut reference, tx);
}

#[test]
fn clone_preparation_matches_initialization_patch_and_structured_rejections() {
    let compiled = fixture();
    let mut machine = compiled.spawn(policy());
    let mut reference = compiled.spawn(policy());
    let invalid = advance(&machine, 0, 1, LogicLevel::Low);
    compare(&mut machine, &mut reference, invalid);
    let prepared = machine
        .prepare_patch(
            machine
                .patch()
                .set_diagnostic_meta(
                    StructuralSubjectRef::Network(compiled.network_key()),
                    DiagnosticMeta {
                        name: Some("initial".to_owned()),
                        ..DiagnosticMeta::default()
                    },
                )
                .unwrap()
                .finish(),
        )
        .require_artifact()
        .unwrap();
    let input = prepared
        .input_snapshot()
        .set(ExternalInputKey::<Level>::from_u128(2), LogicLevel::High)
        .unwrap()
        .set(ExternalInputKey::from_u128(3), LogicLevel::High)
        .unwrap()
        .pulse(ExternalInputKey::<Pulse>::from_u128(1), PulseCount::ONE)
        .unwrap()
        .finish()
        .unwrap();
    let tx = Transaction::initialize(Time::from_ticks(0), machine.revision(), input)
        .with_patch(prepared, ReconfigurationPolicy::RejectStateLoss)
        .unwrap();
    compare(&mut machine, &mut reference, tx);
    let repeated = initialize(&machine);
    compare(&mut machine, &mut reference, repeated);
    let ready = advance(&machine, 11, 1, LogicLevel::Low);
    compare(&mut machine, &mut reference, ready);
    let regressed = advance(&machine, 10, 1, LogicLevel::Low);
    compare(&mut machine, &mut reference, regressed);
    let stale = Transaction::advance(
        Time::from_ticks(20),
        NetworkRevision::from_value(0),
        machine.compiled().input_delta().finish().unwrap(),
    );
    compare(&mut machine, &mut reference, stale);
}
