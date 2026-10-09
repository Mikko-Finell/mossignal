//! Exact-definition adapters and coherent source/target publication over core transactions.
use mossignal::authored::{ExternalInputDef, ExternalOutputDef};
use mossignal::diagnostics::DiagnosticCode;
use mossignal::key::{ExternalInputKey, ExternalOutputKey, NetworkKey, NodeKey};
use mossignal::metadata::DiagnosticMeta;
use mossignal::signal::{Level, LogicLevel, Pulse, PulseCount};
use mossignal::time::{NonZeroSpan, Time};
use mossignal::{
    BindingSet, BoundMachine, CompiledNetwork, DecodePolicy, InputObservation, NetworkBuilder,
    OutputEvent, PersistenceContext, PreparedPatch, ProjectedOutputEvent, PulseDelayConfig,
    ReconfigurationPolicy, RuntimePolicy, StructuralSubjectRef, TimeDomainId, ToggleConfig,
    Transaction, decode_snapshot, encode_snapshot,
};

#[derive(Debug, PartialEq, Eq)]
enum Domain {}
type Bound = BoundMachine<Domain, &'static str, &'static str>;
type Bindings = BindingSet<&'static str, &'static str>;
const PRESSED: ExternalInputKey<Level> = ExternalInputKey::from_u128(1);
const TRIP: ExternalInputKey<Pulse> = ExternalInputKey::from_u128(2);
const EXTRA: ExternalInputKey<Level> = ExternalInputKey::from_u128(3);
const LAMP: ExternalOutputKey<Level> = ExternalOutputKey::from_u128(100);
const DELAY: ExternalOutputKey<Pulse> = ExternalOutputKey::from_u128(101);
const LONG: ExternalOutputKey<Pulse> = ExternalOutputKey::from_u128(102);
const DIRECT: ExternalOutputKey<Pulse> = ExternalOutputKey::from_u128(103);
const NEW_DELAY: ExternalOutputKey<Pulse> = ExternalOutputKey::from_u128(201);
const NEW_LAMP: ExternalOutputKey<Level> = ExternalOutputKey::from_u128(202);

#[derive(Debug, PartialEq, Eq)]
enum EventPayload {
    Established(LogicLevel),
    Changed(LogicLevel, LogicLevel),
    Pulsed(PulseCount),
}
// Cause references are scoped to their owned result. Compare semantic fields
// across independent executions; compare exact causes inside each projection below.
type EventFact = (
    mossignal::key::AnyExternalOutputKey,
    &'static str,
    mossignal::ReactionStamp<Domain>,
    mossignal::NetworkRevision,
    EventPayload,
);
fn event_facts(events: &[ProjectedOutputEvent<Domain, &'static str>]) -> Vec<EventFact> {
    events
        .iter()
        .map(|event| {
            let (output, revision, payload) = match event {
                ProjectedOutputEvent::LevelEstablished {
                    output,
                    value,
                    revision,
                    ..
                } => (*output, *revision, EventPayload::Established(*value)),
                ProjectedOutputEvent::LevelChanged {
                    output,
                    from,
                    to,
                    revision,
                    ..
                } => (*output, *revision, EventPayload::Changed(*from, *to)),
                ProjectedOutputEvent::Pulsed {
                    output,
                    count,
                    revision,
                    ..
                } => (*output, *revision, EventPayload::Pulsed(*count)),
            };
            (event.endpoint(), output, event.stamp(), revision, payload)
        })
        .collect()
}

fn policy() -> RuntimePolicy {
    RuntimePolicy::builder()
        .max_internal_reactions(100)
        .max_evaluated_operations(10_000)
        .max_pending_events(100)
        .max_events_created_per_transaction(1000)
        .max_required_provenance_growth(10_000)
        .build()
        .unwrap()
}
fn circuit() -> CompiledNetwork<Domain> {
    let mut b = NetworkBuilder::with_key(NetworkKey::from_u128(10), TimeDomainId::from_u128(11));
    b.add_level_input(PRESSED, DiagnosticMeta::default())
        .unwrap();
    let trip = b.add_pulse_input(TRIP, DiagnosticMeta::default()).unwrap();
    let lamp = b
        .add_toggle(
            NodeKey::from_u128(10),
            trip,
            ToggleConfig::new(LogicLevel::Low),
            DiagnosticMeta::default(),
        )
        .unwrap()
        .into_outputs();
    b.add_level_output(LAMP, lamp, DiagnosticMeta::default())
        .unwrap();
    for (node, delay, output) in [(20, 5, DELAY), (30, 20, LONG)] {
        let delayed = b
            .add_pulse_delay(
                NodeKey::from_u128(node),
                trip,
                PulseDelayConfig::new(NonZeroSpan::from_ticks(delay).unwrap()),
                DiagnosticMeta::default(),
            )
            .unwrap()
            .into_outputs();
        b.add_pulse_output(output, delayed, DiagnosticMeta::default())
            .unwrap();
    }
    b.add_pulse_output(DIRECT, trip, DiagnosticMeta::default())
        .unwrap();
    b.finish()
        .require_artifact()
        .unwrap()
        .compile()
        .require_artifact()
        .unwrap()
}
fn source_bindings(
    c: &CompiledNetwork<Domain>,
    lamp: &'static str,
    delay: &'static str,
) -> Bindings {
    BindingSet::builder(c)
        .bind_input(PRESSED, "pressed")
        .unwrap()
        .bind_input(TRIP, "trip")
        .unwrap()
        .bind_output(LAMP, lamp)
        .unwrap()
        .bind_output(DELAY, delay)
        .unwrap()
        .bind_output(LONG, "long")
        .unwrap()
        .bind_output(DIRECT, "direct")
        .unwrap()
        .finish()
        .unwrap()
}
fn initialized(c: &CompiledNetwork<Domain>) -> Bound {
    let mut b = BoundMachine::spawn(c, policy(), source_bindings(c, "lamp", "old-delay")).unwrap();
    b.initialize(
        Time::from_ticks(0),
        [InputObservation::Level {
            input: "pressed",
            value: LogicLevel::Low,
        }],
    )
    .unwrap();
    b
}
fn pulse(b: &mut Bound, at: u64, count: u64) {
    b.advance(
        Time::from_ticks(at),
        [InputObservation::Pulse {
            input: "trip",
            count: PulseCount::new(count),
        }],
    )
    .unwrap();
}
fn replacement(b: &Bound) -> PreparedPatch<Domain> {
    let outputs = b.machine().compiled().graph().external_outputs();
    let delayed = outputs
        .iter()
        .find(|o| o.key() == DELAY.into())
        .unwrap()
        .source();
    let lamp = outputs
        .iter()
        .find(|o| o.key() == LAMP.into())
        .unwrap()
        .source();
    b.prepare_patch(
        b.machine()
            .patch()
            .remove_external_input(PRESSED.into())
            .unwrap()
            .add_external_input(ExternalInputDef::new(
                EXTRA.into(),
                DiagnosticMeta::default(),
            ))
            .unwrap()
            .remove_external_output(DELAY.into())
            .unwrap()
            .add_external_output(ExternalOutputDef::new(
                NEW_DELAY.into(),
                delayed,
                DiagnosticMeta::default(),
            ))
            .unwrap()
            .add_external_output(ExternalOutputDef::new(
                NEW_LAMP.into(),
                lamp,
                DiagnosticMeta::default(),
            ))
            .unwrap()
            .finish(),
    )
    .require_artifact()
    .unwrap()
}
fn target_bindings(c: &CompiledNetwork<Domain>) -> Bindings {
    BindingSet::builder(c)
        .bind_input(EXTRA, "extra")
        .unwrap()
        .bind_input(TRIP, "trip")
        .unwrap()
        .bind_output(LAMP, "relabeled-lamp")
        .unwrap()
        .bind_output(NEW_DELAY, "new-delay")
        .unwrap()
        .bind_output(NEW_LAMP, "new-lamp")
        .unwrap()
        .bind_output(LONG, "long")
        .unwrap()
        .bind_output(DIRECT, "direct")
        .unwrap()
        .finish()
        .unwrap()
}
fn patch_transaction(b: &Bound, prepared: PreparedPatch<Domain>, at: u64) -> Transaction<Domain> {
    let input = target_bindings(prepared.resulting_compiled())
        .input_projector(prepared.resulting_compiled())
        .unwrap()
        .delta_from([InputObservation::Level {
            input: "extra",
            value: LogicLevel::High,
        }])
        .unwrap();
    Transaction::advance(Time::from_ticks(at), b.machine().revision(), input)
        .with_patch(prepared, ReconfigurationPolicy::AllowReportedStateLoss)
        .unwrap()
}

#[test]
fn same_time_progress_and_map_only_rebind_preserve_freshness_and_owned_labels() {
    let c = circuit();
    let mut b = initialized(&c);
    let first = b
        .advance(
            Time::from_ticks(0),
            [InputObservation::Pulse {
                input: "trip",
                count: PulseCount::ONE,
            }],
        )
        .unwrap();
    pulse(&mut b, 0, 1);
    assert_eq!(b.output_level(&"lamp").unwrap(), LogicLevel::Low);
    assert_eq!(b.machine().last_reaction().unwrap().order(), 2);
    let before = b.machine().snapshot();
    let tx = Transaction::advance(
        Time::from_ticks(5),
        b.machine().revision(),
        c.input_delta().finish().unwrap(),
    )
    .expect_execution_state(b.machine().execution_state_digest());
    let forecast = b.machine().forecast(tx.clone()).unwrap();
    b.rebind(source_bindings(&c, "new-lamp-label", "new-delay-label"))
        .unwrap();
    assert_eq!(b.machine().snapshot(), before);
    assert!(b.output_level(&"lamp").is_err());
    assert_eq!(b.output_level(&"new-lamp-label").unwrap(), LogicLevel::Low);
    let due = b.apply(tx).unwrap();
    assert_eq!(
        due.ordinary().after_execution_digest(),
        forecast.result().after_execution_digest()
    );
    assert!(
        matches!(due.projected_output_events(), [ProjectedOutputEvent::Pulsed {
        endpoint, output: "new-delay-label", count, .. }] if *endpoint == DELAY && count.get() == 2)
    );
    assert!(first.projected_output_events().iter().any(|e| matches!(e,
        ProjectedOutputEvent::LevelChanged { endpoint, output: "lamp", stamp, .. } if *endpoint == LAMP && stamp.order() == 1)));
    for event in first.projected_output_events() {
        let cause = match event {
            ProjectedOutputEvent::LevelChanged { cause, .. }
            | ProjectedOutputEvent::Pulsed { cause, .. }
            | ProjectedOutputEvent::LevelEstablished { cause, .. } => *cause,
        };
        assert!(first.ordinary().provenance().inspect(cause).is_ok());
    }
    assert!(
        b.advance(Time::from_ticks(5), [])
            .unwrap()
            .projected_output_events()
            .is_empty()
    );
    let (machine, bindings) = b.into_parts();
    assert_eq!(machine.last_reaction().unwrap().order(), 1);
    assert_eq!(bindings.output_identifier(LAMP), Some(&"new-lamp-label"));
}

#[test]
fn patch_projects_removed_source_outputs_then_target_outputs_and_preserves_other_work() {
    let c = circuit();
    let mut b = initialized(&c);
    pulse(&mut b, 0, 1);
    pulse(&mut b, 1, 2);
    pulse(&mut b, 2, 1);
    assert_eq!(b.output_level(&"lamp").unwrap(), LogicLevel::Low);
    let before_pending = b.machine().inspect_pending_events().unwrap();
    let prepared = replacement(&b);
    let target = target_bindings(prepared.resulting_compiled());
    let tx = patch_transaction(&b, prepared, 6);
    // One ordinary forecast is an equivalence oracle, never an implementation step.
    let forecast = b.machine().forecast(tx.clone()).unwrap();
    let result = b.apply_reconfigured(tx, target).unwrap();
    assert_eq!(
        result.ordinary().after_execution_digest(),
        forecast.result().after_execution_digest()
    );
    let source_map = source_bindings(&c, "lamp", "old-delay");
    let expected = forecast
        .result()
        .output_events()
        .iter()
        .map(|event| {
            let map = if event.at().ticks() < 6 {
                &source_map
            } else {
                b.bindings()
            };
            map.project_output_event(event).unwrap()
        })
        .collect::<Vec<_>>();
    assert_eq!(
        event_facts(result.projected_output_events()),
        event_facts(&expected)
    );
    let source_revision = result.ordinary().before_revision();
    let target_revision = result.ordinary().after_revision();
    assert_ne!(source_revision, target_revision);
    assert!(
        matches!(&result.projected_output_events()[0], ProjectedOutputEvent::Pulsed {
        endpoint, output: "old-delay", count, stamp, revision, ..
    } if *endpoint == DELAY && count.get() == 1 && stamp.time().ticks() == 5 && *revision == source_revision)
    );
    assert!(result.projected_output_events().iter().any(
        |e| matches!(e, ProjectedOutputEvent::Pulsed {
        endpoint, output: "new-delay", count, stamp, revision, ..
    } if *endpoint == NEW_DELAY && count.get() == 2 && stamp.time().ticks() == 6 && *revision == target_revision)
    ));
    assert!(result.projected_output_events().iter().any(
        |e| matches!(e, ProjectedOutputEvent::LevelEstablished {
        endpoint, output: "new-lamp", value: LogicLevel::Low, revision, ..
    } if *endpoint == NEW_LAMP && *revision == target_revision)
    ));
    for (ordinary, projected) in result
        .ordinary()
        .output_events()
        .iter()
        .zip(result.projected_output_events())
    {
        assert_eq!(ordinary.stamp(), projected.stamp());
        assert_eq!(ordinary.at(), projected.at());
        let (endpoint, cause, revision): (mossignal::key::AnyExternalOutputKey, _, _) =
            match ordinary {
                OutputEvent::LevelEstablished {
                    output,
                    cause,
                    revision,
                    ..
                }
                | OutputEvent::LevelChanged {
                    output,
                    cause,
                    revision,
                    ..
                } => ((*output).into(), *cause, *revision),
                OutputEvent::Pulsed {
                    output,
                    cause,
                    revision,
                    ..
                } => ((*output).into(), *cause, *revision),
                _ => panic!("new ordinary event variants require lossless projection coverage"),
            };
        assert_eq!(endpoint, projected.endpoint());
        let (projected_cause, projected_revision) = match projected {
            ProjectedOutputEvent::LevelEstablished {
                cause, revision, ..
            }
            | ProjectedOutputEvent::LevelChanged {
                cause, revision, ..
            }
            | ProjectedOutputEvent::Pulsed {
                cause, revision, ..
            } => (*cause, *revision),
        };
        assert_eq!((projected_cause, projected_revision), (cause, revision));
        assert!(result.ordinary().provenance().inspect(cause).is_ok());
    }
    assert_eq!(b.output_level(&"relabeled-lamp").unwrap(), LogicLevel::Low);
    let pending = b.machine().inspect_pending_events().unwrap();
    for event in before_pending
        .iter()
        .filter(|event| event.deadline.ticks() > 6)
    {
        assert!(
            pending.iter().any(|candidate| candidate.event == event.event
                && candidate.owner == event.owner && candidate.origin == event.origin
                && candidate.origin_stamp == event.origin_stamp && candidate.revision == event.revision
                && candidate.deadline == event.deadline
                && matches!((&candidate.payload, &event.payload),
                    (mossignal::PendingPayload::Pulse(a), mossignal::PendingPayload::Pulse(b)) if a == b)),
            "unrelated pending identity and causes must survive"
        );
    }
    let due = b.advance(Time::from_ticks(7), []).unwrap();
    assert!(
        matches!(due.projected_output_events(), [ProjectedOutputEvent::Pulsed {
        endpoint, output: "new-delay", count, .. }] if *endpoint == NEW_DELAY && count.get() == 1)
    );
    // Historical source output remains usable after the endpoint was removed and new reactions committed.
    assert!(matches!(
        &result.projected_output_events()[0],
        ProjectedOutputEvent::Pulsed {
            output: "old-delay",
            ..
        }
    ));
}

#[test]
fn exact_definition_mismatch_rejects_even_when_input_schema_matches() {
    let c = circuit();
    let mut b = initialized(&c);
    let prepared = b
        .prepare_patch(
            b.machine()
                .patch()
                .remove_external_output(DELAY.into())
                .unwrap()
                .finish(),
        )
        .require_artifact()
        .unwrap();
    let changed = prepared.resulting_compiled();
    assert_eq!(
        c.input_schema_fingerprint(),
        changed.input_schema_fingerprint()
    );
    assert_ne!(c.fingerprint(), changed.fingerprint());
    let wrong = BindingSet::builder(changed)
        .bind_input(PRESSED, "pressed")
        .unwrap()
        .bind_input(TRIP, "trip")
        .unwrap()
        .bind_output(LAMP, "lamp")
        .unwrap()
        .bind_output(LONG, "long")
        .unwrap()
        .bind_output(DIRECT, "direct")
        .unwrap()
        .finish()
        .unwrap();
    let before = b.machine().snapshot();
    assert_eq!(
        b.rebind(wrong).unwrap_err().code(),
        DiagnosticCode::BindingStaleSchema
    );
    assert_eq!(b.machine().snapshot(), before);
    assert_eq!(b.bindings().output_identifier(DELAY), Some(&"old-delay"));
}

#[test]
fn invalid_context_incomplete_maps_and_stale_patch_reject_before_deadlines() {
    let c = circuit();
    let mut b = initialized(&c);
    pulse(&mut b, 0, 1);
    let prepared = replacement(&b);
    let target = target_bindings(prepared.resulting_compiled());
    let tx = patch_transaction(&b, prepared.clone(), 6);
    let before = b.machine().snapshot();
    assert_eq!(
        b.apply(tx.clone()).err().unwrap().code(),
        DiagnosticCode::BindingInvalidReconfigurationContext
    );
    let no_patch = Transaction::advance(
        Time::from_ticks(6),
        b.machine().revision(),
        c.input_delta().finish().unwrap(),
    );
    assert_eq!(
        b.apply_reconfigured(no_patch, target.clone())
            .err()
            .unwrap()
            .code(),
        DiagnosticCode::BindingInvalidReconfigurationContext
    );
    let incomplete = BindingSet::builder(prepared.resulting_compiled())
        .finish()
        .unwrap();
    assert_eq!(
        b.apply_reconfigured(tx.clone(), incomplete)
            .err()
            .unwrap()
            .code(),
        DiagnosticCode::BindingMissingRequiredBinding
    );
    assert_eq!(
        b.apply_reconfigured(tx.clone(), source_bindings(&c, "lamp", "old-delay"))
            .err()
            .unwrap()
            .code(),
        DiagnosticCode::BindingStaleSchema
    );
    let loss_rejected = tx
        .clone()
        .with_patch(prepared.clone(), ReconfigurationPolicy::RejectStateLoss)
        .unwrap();
    assert_eq!(
        b.apply_reconfigured(loss_rejected, target.clone())
            .err()
            .unwrap()
            .code(),
        DiagnosticCode::ReconfigurationStateLossRejected
    );
    let omitted = prepared.input_delta().finish().unwrap();
    assert!(
        Transaction::advance(Time::from_ticks(6), b.machine().revision(), omitted)
            .with_patch(prepared.clone(), ReconfigurationPolicy::RejectStateLoss)
            .is_err()
    );
    let foreign = c.input_delta().finish().unwrap();
    assert!(
        Transaction::advance(Time::from_ticks(6), b.machine().revision(), foreign)
            .with_patch(prepared, ReconfigurationPolicy::RejectStateLoss)
            .is_err()
    );
    assert_eq!(b.machine().snapshot(), before);
    assert_eq!(b.bindings().output_identifier(DELAY), Some(&"old-delay"));
    // A same-time settlement changes freshness, while leaving the prepared base revision intact.
    let guarded = tx
        .clone()
        .expect_execution_state(b.machine().execution_state_digest());
    b.advance(Time::from_ticks(0), []).unwrap();
    let before = b.machine().snapshot();
    assert_eq!(
        b.apply_reconfigured(guarded, target.clone())
            .err()
            .unwrap()
            .code(),
        DiagnosticCode::RuntimeStaleExecutionState
    );
    assert_eq!(b.machine().snapshot(), before);
    b.apply_reconfigured(tx.clone(), target.clone()).unwrap();
    let after = b.machine().snapshot();
    assert_eq!(
        b.apply_reconfigured(tx, target).err().unwrap().code(),
        DiagnosticCode::RuntimeStaleRevision
    );
    assert_eq!(b.machine().snapshot(), after);
    assert_eq!(
        b.bindings().output_identifier(NEW_DELAY),
        Some(&"new-delay")
    );
}

#[test]
fn initialization_with_a_patch_uses_only_target_bindings_and_preserves_lifecycle_checks() {
    let c = circuit();
    let mut b =
        BoundMachine::spawn(&c, policy(), source_bindings(&c, "lamp", "old-delay")).unwrap();
    let prepared = replacement(&b);
    let target = target_bindings(prepared.resulting_compiled());
    let input = target
        .input_projector(prepared.resulting_compiled())
        .unwrap()
        .snapshot_from([InputObservation::Level {
            input: "extra",
            value: LogicLevel::High,
        }])
        .unwrap();
    let tx = Transaction::initialize(Time::from_ticks(8), b.machine().revision(), input)
        .with_patch(prepared, ReconfigurationPolicy::RejectStateLoss)
        .unwrap();
    let before = b.machine().snapshot();
    assert_eq!(
        b.advance(Time::from_ticks(8), []).err().unwrap().code(),
        DiagnosticCode::LifecycleDeltaBeforeInitialization
    );
    assert_eq!(b.machine().snapshot(), before);
    let result = b.apply_reconfigured(tx, target).unwrap();
    assert!(result.projected_output_events().iter().all(|event| matches!(event,
        ProjectedOutputEvent::LevelEstablished { output: "relabeled-lamp" | "new-lamp", revision, .. }
            if *revision == b.machine().revision())));
    assert_eq!(result.projected_output_events().len(), 2);
    let before = b.machine().snapshot();
    assert_eq!(
        b.initialize(
            Time::from_ticks(8),
            [InputObservation::Level {
                input: "extra",
                value: LogicLevel::Low
            }]
        )
        .err()
        .unwrap()
        .code(),
        DiagnosticCode::LifecycleAlreadyInitialized
    );
    assert_eq!(b.machine().snapshot(), before);
    pulse(&mut b, 8, 1);
    assert_eq!(b.output_level(&"relabeled-lamp").unwrap(), LogicLevel::High);
}

#[test]
fn a_prepared_patch_from_another_definition_rejects_at_the_same_revision() {
    let c = circuit();
    let original = initialized(&c);
    let prepared = replacement(&original);
    let target = target_bindings(prepared.resulting_compiled());
    let tx = patch_transaction(&original, prepared, 6);
    let different = original
        .prepare_patch(
            original
                .machine()
                .patch()
                .remove_external_output(LONG.into())
                .unwrap()
                .finish(),
        )
        .require_artifact()
        .unwrap()
        .resulting_compiled()
        .clone();
    let bindings = BindingSet::builder(&different)
        .bind_input(PRESSED, "pressed")
        .unwrap()
        .bind_input(TRIP, "trip")
        .unwrap()
        .bind_output(LAMP, "lamp")
        .unwrap()
        .bind_output(DELAY, "old-delay")
        .unwrap()
        .bind_output(DIRECT, "direct")
        .unwrap()
        .finish()
        .unwrap();
    let mut b = BoundMachine::spawn(&different, policy(), bindings).unwrap();
    b.initialize(
        Time::from_ticks(0),
        [InputObservation::Level {
            input: "pressed",
            value: LogicLevel::Low,
        }],
    )
    .unwrap();
    pulse(&mut b, 0, 1);
    assert_eq!(b.machine().revision(), original.machine().revision());
    let before = b.machine().snapshot();
    assert_eq!(
        b.apply_reconfigured(tx, target).err().unwrap().code(),
        DiagnosticCode::ReconfigurationStalePreparedPatch
    );
    assert_eq!(b.machine().snapshot(), before);
    assert_eq!(b.bindings().output_identifier(DELAY), Some(&"old-delay"));
}

#[test]
fn bindings_attach_to_a_restored_patched_machine_across_the_encoded_boundary() {
    let c = circuit();
    let mut b = initialized(&c);
    pulse(&mut b, 0, 1);
    let prepared = b
        .prepare_patch(
            b.machine()
                .patch()
                .set_diagnostic_meta(
                    StructuralSubjectRef::Network(c.network_key()),
                    DiagnosticMeta {
                        name: Some("renamed".into()),
                        ..DiagnosticMeta::default()
                    },
                )
                .unwrap()
                .finish(),
        )
        .require_artifact()
        .unwrap();
    let input = prepared.input_delta().finish().unwrap();
    let tx = Transaction::advance(Time::from_ticks(0), b.machine().revision(), input)
        .with_patch(prepared, ReconfigurationPolicy::RejectStateLoss)
        .unwrap();
    b.apply_reconfigured(tx, source_bindings(&c, "lamp", "old-delay"))
        .unwrap();
    let context = PersistenceContext::new(c.time_domain_id());
    let encoded = encode_snapshot(&context, &b.machine().snapshot()).unwrap();
    let limits = DecodePolicy::new(
        8_000_000, 64, 2_000_000, 8_000_000, 200_000, 10_000, 10_000, 10_000, 1000, 10_000,
        100_000, 100_000, 10_000, 1000, 1_000_000,
    );
    let decoded = decode_snapshot(&context, encoded.as_bytes(), &limits).unwrap();
    let restored = b.machine().compiled().restore(decoded, policy()).unwrap();
    let mut attached =
        BoundMachine::new(restored, source_bindings(&c, "lamp", "old-delay")).unwrap();
    assert_eq!(attached.machine().snapshot(), b.machine().snapshot());
    let ordinary = b.advance(Time::from_ticks(5), []).unwrap();
    let resumed = attached.advance(Time::from_ticks(5), []).unwrap();
    assert_eq!(
        event_facts(resumed.projected_output_events()),
        event_facts(ordinary.projected_output_events())
    );
    assert_eq!(attached.machine().snapshot(), b.machine().snapshot());
}
