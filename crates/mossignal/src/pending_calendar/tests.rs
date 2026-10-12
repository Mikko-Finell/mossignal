//! Indexed reads compared with ordinary pending inspection and lifecycle state.
#![cfg(test)]
use crate::key::{ExternalInputKey, ExternalOutputKey, InPortKey, NodeKey, OutPortKey};
use crate::metadata::DiagnosticMeta;
use crate::signal::{Pulse, PulseCount};
use crate::time::{NonZeroSpan, Time};
use crate::{
    Machine, NetworkBuilder, OutputEvent, PulseDelayConfig, PulseDelayInspectionFailure,
    RuntimePolicy, TimeDomainId, Transaction,
};

fn policy() -> RuntimePolicy {
    RuntimePolicy::builder()
        .max_internal_reactions(1000)
        .max_evaluated_operations(100_000)
        .max_pending_events(1000)
        .max_events_created_per_transaction(10_000)
        .max_required_provenance_growth(100_000)
        .build()
        .expect("explicit finite policy")
}
fn fixture(
    nodes: u128,
) -> (
    crate::CompiledNetwork<()>,
    ExternalInputKey<Pulse>,
    ExternalOutputKey<Pulse>,
) {
    let mut builder = NetworkBuilder::new(TimeDomainId::from_u128(1));
    let input = ExternalInputKey::from_u128(1);
    let incoming = builder
        .add_pulse_input(input, DiagnosticMeta::default())
        .expect("input");
    let output = ExternalOutputKey::from_u128(2);
    for n in 1..=nodes {
        let pulse = builder
            .add_pulse_delay_with_ports(
                NodeKey::from_u128(n),
                InPortKey::from_u128(n * 2),
                OutPortKey::from_u128(n * 2 + 1),
                incoming,
                PulseDelayConfig::new(NonZeroSpan::from_ticks(10).expect("delay")),
                DiagnosticMeta::default(),
            )
            .expect("delay")
            .into_outputs();
        if n == 1 {
            builder
                .add_pulse_output(output, pulse, DiagnosticMeta::default())
                .expect("output");
        }
    }
    (
        builder
            .finish()
            .require_artifact()
            .expect("validated")
            .compile()
            .require_artifact()
            .expect("compiled"),
        input,
        output,
    )
}
fn initialize(
    compiled: &crate::CompiledNetwork<()>,
    input: ExternalInputKey<Pulse>,
    count: u64,
) -> Machine<()> {
    let mut machine = compiled.spawn(policy());
    machine
        .apply(Transaction::initialize(
            Time::from_ticks(0),
            machine.revision(),
            compiled
                .input_snapshot()
                .pulse(input, PulseCount::new(count))
                .expect("pulse")
                .finish()
                .expect("snapshot"),
        ))
        .expect("initialize");
    machine
}
fn pulse(
    machine: &mut Machine<()>,
    input: ExternalInputKey<Pulse>,
    at: u64,
    count: u64,
) -> crate::TransactionResult<()> {
    let delta = machine
        .compiled()
        .input_delta()
        .pulse(input, PulseCount::new(count))
        .expect("pulse")
        .finish()
        .expect("delta");
    machine
        .apply(Transaction::advance(
            Time::from_ticks(at),
            machine.revision(),
            delta,
        ))
        .expect("advance")
}
fn assert_summary(machine: &Machine<()>, count: u64, deadline: Option<u64>) {
    let summary = machine
        .inspect_pulse_delay_summary(NodeKey::from_u128(1))
        .expect("summary");
    assert_eq!(summary.pending_count().get(), count);
    assert_eq!(summary.next_deadline().map(Time::ticks), deadline);
    let full = machine
        .inspect_pulse_delay(NodeKey::from_u128(1))
        .expect("full oracle");
    assert_eq!(
        full.pending()
            .iter()
            .map(|event| event.count().get())
            .sum::<u64>(),
        count
    );
    assert_eq!(full.next_deadline(), summary.next_deadline());
}

#[test]
fn groups_deadlines_completion_zero_input_and_pure_reads_match_full_inspection() {
    let (compiled, input, _) = fixture(1);
    let mut machine = initialize(&compiled, input, 2);
    pulse(&mut machine, input, 0, 3);
    pulse(&mut machine, input, 1, 4);
    let before = machine.snapshot();
    let digests = (
        machine.execution_state_digest(),
        machine.observable_state_digest(),
        machine.last_reaction(),
    );
    assert_summary(&machine, 9, Some(10));
    assert_eq!(machine.snapshot(), before);
    assert_eq!(
        (
            machine.execution_state_digest(),
            machine.observable_state_digest(),
            machine.last_reaction()
        ),
        digests
    );
    pulse(&mut machine, input, 10, 0);
    assert_summary(&machine, 4, Some(11));
    pulse(&mut machine, input, 11, 0);
    assert_summary(&machine, 0, None);
    pulse(&mut machine, input, 1_000_000, 0);
    let before = machine.snapshot();
    assert_summary(&machine, 0, None);
    assert_eq!(machine.snapshot(), before);
}

#[test]
fn summary_exhaustion_never_rejects_valid_groups_or_changes_future_output() {
    let (compiled, input, output) = fixture(1);
    let mut machine = initialize(&compiled, input, u64::MAX);
    pulse(&mut machine, input, 1, u64::MAX);
    let before = machine.snapshot();
    assert!(matches!(
        machine.inspect_pulse_delay_summary(NodeKey::from_u128(1)),
        Err(PulseDelayInspectionFailure::PendingCountOverflow(_))
    ));
    assert_eq!(
        machine
            .inspect_pulse_delay(NodeKey::from_u128(1))
            .expect("valid full state")
            .pending()
            .len(),
        2
    );
    assert_eq!(machine.snapshot(), before);
    for at in [10, 11] {
        let result = pulse(&mut machine, input, at, 0);
        assert!(
            matches!(result.output_events(), [OutputEvent::Pulsed { output: actual, count, .. }]
            if *actual == output && count.get() == u64::MAX)
        );
        assert_summary(
            &machine,
            if at == 10 { u64::MAX } else { 0 },
            if at == 10 { Some(11) } else { None },
        );
    }
}

#[test]
fn failed_equal_deadline_overflow_keeps_calendar_index_and_committed_snapshot() {
    let (compiled, input, _) = fixture(1);
    let mut machine = initialize(&compiled, input, u64::MAX);
    pulse(&mut machine, input, 0, 1);
    let before = machine.snapshot();
    let delta = compiled.input_delta().finish().expect("empty");
    assert!(
        machine
            .apply(Transaction::advance(
                Time::from_ticks(10),
                machine.revision(),
                delta
            ))
            .is_err()
    );
    assert_eq!(machine.snapshot(), before);
    assert!(matches!(
        machine.inspect_pulse_delay_summary(NodeKey::from_u128(1)),
        Err(PulseDelayInspectionFailure::PendingCountOverflow(_))
    ));
}

#[test]
fn restoration_and_forecast_derive_the_same_summary_without_mutating_live_state() {
    let (compiled, input, _) = fixture(1);
    let mut machine = initialize(&compiled, input, 2);
    pulse(&mut machine, input, 1, 3);
    let saved = machine.snapshot();
    let restored = compiled.restore(saved.clone(), policy()).expect("restored");
    assert_summary(&restored, 5, Some(10));
    assert_eq!(restored.snapshot(), saved);
    let forecast = restored
        .forecast(Transaction::advance(
            Time::from_ticks(10),
            restored.revision(),
            compiled.input_delta().finish().expect("delta"),
        ))
        .expect("forecast");
    let summary = forecast
        .state()
        .inspect_pulse_delay_summary(NodeKey::from_u128(1))
        .expect("forecast summary");
    assert_eq!(summary.pending_count().get(), 3);
    assert_eq!(summary.next_deadline(), Some(Time::from_ticks(11)));
    assert_eq!(restored.snapshot(), saved);
    let fresh = compiled.spawn(policy());
    assert!(matches!(
        fresh.inspect_pulse_delay_summary(NodeKey::from_u128(1)),
        Err(PulseDelayInspectionFailure::NotInitialized)
    ));
}

#[test]
fn indexed_query_work_is_independent_of_unrelated_pending_groups_and_provenance() {
    for nodes in [1, 16, 64] {
        let (compiled, input, _) = fixture(nodes);
        let machine = initialize(&compiled, input, 2);
        crate::execution_work::reset();
        crate::causal_work::reset();
        let summary = machine
            .inspect_pulse_delay_summary(NodeKey::from_u128(1))
            .expect("indexed read");
        assert_eq!(summary.pending_count().get(), 2);
        assert_eq!(
            crate::execution_work::read(),
            crate::execution_work::Work {
                pulse_summary_lookups: 1,
                ..Default::default()
            }
        );
        assert_eq!(
            crate::causal_work::read(),
            crate::causal_work::Work::default()
        );
    }
}

#[test]
fn unknown_and_wrong_kind_fail_before_returning_a_summary() {
    let (compiled, input, _) = fixture(1);
    let machine = initialize(&compiled, input, 0);
    assert!(matches!(
        machine.inspect_pulse_delay_summary(NodeKey::from_u128(999)),
        Err(PulseDelayInspectionFailure::UnknownNode(_))
    ));
    let mut builder = NetworkBuilder::<()>::new(TimeDomainId::from_u128(1));
    let incoming = builder
        .add_pulse_input(ExternalInputKey::from_u128(1), DiagnosticMeta::default())
        .expect("input");
    builder
        .add_toggle_with_ports(
            NodeKey::from_u128(2),
            InPortKey::from_u128(2),
            OutPortKey::from_u128(3),
            incoming,
            crate::ToggleConfig {
                initial: crate::signal::LogicLevel::Low,
            },
            DiagnosticMeta::default(),
        )
        .expect("toggle");
    let compiled = builder
        .finish()
        .require_artifact()
        .expect("validated")
        .compile()
        .require_artifact()
        .expect("compiled");
    let machine = compiled.spawn(policy());
    assert!(matches!(
        machine.inspect_pulse_delay_summary(NodeKey::from_u128(2)),
        Err(PulseDelayInspectionFailure::NotPulseDelay(_))
    ));
}

#[test]
fn retuning_preserves_old_deadlines_then_cancellation_and_removal_retire_indexed_work() {
    use crate::{NodeMigrationDirective, PulseDelayMigration, ReconfigurationPolicy};
    let (compiled, input, _) = fixture(2);
    let mut machine = initialize(&compiled, input, 2);
    pulse(&mut machine, input, 1, 3);
    let node = NodeKey::from_u128(1);
    let old = compiled
        .graph()
        .nodes()
        .iter()
        .find(|definition| definition.key() == node)
        .expect("node")
        .clone();
    let replacement = crate::authored::NodeDef::new(
        node,
        crate::authored::NodeKind::pulse_delay(NonZeroSpan::from_ticks(20).expect("delay")),
        old.ports().clone(),
        DiagnosticMeta::default(),
    );
    let patch = machine
        .patch()
        .replace_node(
            node,
            replacement,
            NodeMigrationDirective::PulseDelay(PulseDelayMigration::PreserveDeadlines),
        )
        .expect("retune")
        .finish();
    let prepared = machine
        .prepare_patch(patch)
        .require_artifact()
        .expect("prepared");
    let delta = prepared
        .resulting_compiled()
        .input_delta()
        .finish()
        .expect("delta");
    machine
        .apply(
            Transaction::advance(Time::from_ticks(2), machine.revision(), delta)
                .with_patch(prepared, ReconfigurationPolicy::RejectStateLoss)
                .expect("transaction"),
        )
        .expect("retune publication");
    assert_summary(&machine, 5, Some(10));
    pulse(&mut machine, input, 2, 7);
    assert_summary(&machine, 12, Some(10));
    pulse(&mut machine, input, 10, 0);
    assert_summary(&machine, 10, Some(11));
    pulse(&mut machine, input, 11, 0);
    assert_summary(&machine, 7, Some(22));

    let removed = NodeKey::from_u128(2);
    let incoming =
        crate::authored::ConnectionEndpoint::node_input(InPortKey::<Pulse>::from_u128(4).into());
    let connection = machine
        .compiled()
        .graph()
        .connections()
        .iter()
        .find(|c| c.to() == incoming)
        .expect("removed node connection")
        .key();
    let patch = machine
        .patch()
        .remove_connection(connection)
        .expect("disconnect removed node")
        .remove_node(removed)
        .expect("remove")
        .finish();
    let prepared = machine
        .prepare_patch(patch)
        .require_artifact()
        .expect("remove prepared");
    let delta = prepared
        .resulting_compiled()
        .input_delta()
        .finish()
        .expect("delta");
    machine
        .apply(
            Transaction::advance(Time::from_ticks(12), machine.revision(), delta)
                .with_patch(prepared, ReconfigurationPolicy::AllowReportedStateLoss)
                .expect("transaction"),
        )
        .expect("removal");
    assert_eq!(
        machine.store.pending_events.pulse_totals(removed),
        (0, None)
    );
    assert!(matches!(
        machine.inspect_pulse_delay_summary(removed),
        Err(PulseDelayInspectionFailure::UnknownNode(_))
    ));
    assert_summary(&machine, 7, Some(22));

    let current = machine
        .compiled()
        .graph()
        .nodes()
        .iter()
        .find(|definition| definition.key() == node)
        .expect("node")
        .clone();
    let patch = machine
        .patch()
        .replace_node(
            node,
            current,
            NodeMigrationDirective::PulseDelay(PulseDelayMigration::CancelPending),
        )
        .expect("cancel")
        .finish();
    let prepared = machine
        .prepare_patch(patch)
        .require_artifact()
        .expect("cancel prepared");
    let delta = prepared
        .resulting_compiled()
        .input_delta()
        .finish()
        .expect("delta");
    machine
        .apply(
            Transaction::advance(Time::from_ticks(12), machine.revision(), delta)
                .with_patch(prepared, ReconfigurationPolicy::AllowReportedStateLoss)
                .expect("transaction"),
        )
        .expect("cancel work");
    assert_summary(&machine, 0, None);
    pulse(&mut machine, input, 22, 0);
    assert_summary(&machine, 0, None);
}

#[test]
fn rebuilding_a_zero_count_obligation_preserves_calendar_shape_without_panicking() {
    let (compiled, input, _) = fixture(1);
    let machine = initialize(&compiled, input, 1);
    let mut event = machine.store.pending_events.values().next().expect("batch")[0];
    if let crate::machine::PendingEvent::PulseDelay(group) = &mut event {
        group.count = PulseCount::ZERO;
    }
    // PulseCount's complete domain includes zero. Index construction must not
    // introduce a new panic/admission law for a structurally supplied obligation.
    let mut calendar =
        crate::pending_calendar::PendingCalendar::from_events(std::collections::BTreeMap::from([
            (Time::from_ticks(10), vec![event]),
        ]));
    assert_eq!(
        calendar.pulse_totals(NodeKey::from_u128(1)),
        (0, Some(Time::from_ticks(10)))
    );
    assert_eq!(
        calendar.remove(&Time::from_ticks(10)).expect("batch").len(),
        1
    );
    assert_eq!(calendar.pulse_totals(NodeKey::from_u128(1)), (0, None));
}
