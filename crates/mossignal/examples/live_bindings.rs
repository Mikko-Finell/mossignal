//! Keep one private live circuit while caller labels and topology change.
//! Earlier deadline output keeps its producing label even after endpoint removal.
use mossignal::authored::{ExternalInputDef, ExternalOutputDef};
use mossignal::key::{ExternalInputKey, ExternalOutputKey, NetworkKey};
use mossignal::metadata::DiagnosticMeta;
use mossignal::signal::{Level, LogicLevel, Pulse, PulseCount};
use mossignal::time::{NonZeroSpan, Time};
use mossignal::{
    BindingSet, BoundMachine, InputObservation, NetworkBuilder, ProjectedOutputEvent,
    PulseDelayConfig, ReconfigurationPolicy, RuntimePolicy, TimeDomainId, ToggleConfig,
    Transaction,
};

#[derive(Debug, PartialEq, Eq)]
struct Ticks;

fn main() {
    let mut builder =
        NetworkBuilder::<Ticks>::with_key(NetworkKey::from_u128(1), TimeDomainId::from_u128(2));
    let (trip, pulse) = builder.pulse_input("trip");
    let toggle = builder
        .toggle(pulse, ToggleConfig::new(LogicLevel::Low))
        .unwrap();
    let lamp = builder.level_output("lamp", toggle).unwrap();
    let delayed = builder
        .pulse_delay(
            pulse,
            PulseDelayConfig::new(NonZeroSpan::from_ticks(5).unwrap()),
        )
        .unwrap();
    let delay = builder.pulse_output("delay", delayed).unwrap();
    let compiled = builder
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
        .max_required_provenance_growth(10_000)
        .build()
        .unwrap();
    let source = BindingSet::builder(&compiled)
        .bind_input(trip, "trip")
        .unwrap()
        .bind_output(lamp, "lamp")
        .unwrap()
        .bind_output(delay, "old-delay")
        .unwrap()
        .finish()
        .unwrap();
    let mut bound = BoundMachine::new(compiled.spawn(policy), source).unwrap();
    bound.initialize(Time::from_ticks(0), []).unwrap();
    let first = bound
        .advance(
            Time::from_ticks(0),
            [InputObservation::Pulse {
                input: "trip",
                count: PulseCount::ONE,
            }],
        )
        .unwrap();
    bound
        .advance(
            Time::from_ticks(0),
            [InputObservation::Pulse {
                input: "trip",
                count: PulseCount::ONE,
            }],
        )
        .unwrap();
    assert_eq!(bound.output_level(&"lamp").unwrap(), LogicLevel::Low);
    assert_eq!(bound.machine().last_reaction().unwrap().order(), 2);

    // Renaming caller identifiers does not consume an occurrence or replay a Pulse.
    let before = bound.machine().snapshot();
    let renamed = BindingSet::builder(&compiled)
        .bind_input(trip, "trip")
        .unwrap()
        .bind_output(lamp, "renamed-lamp")
        .unwrap()
        .bind_output(delay, "old-delay")
        .unwrap()
        .finish()
        .unwrap();
    bound.rebind(renamed).unwrap();
    assert_eq!(bound.machine().snapshot(), before);
    assert!(matches!(
        &first.projected_output_events()[0],
        ProjectedOutputEvent::LevelChanged {
            output: "lamp",
            to: LogicLevel::High,
            ..
        }
    ));

    // Preparing a patch is read-only. The ordinary input projector uses the target
    // definition, and set supplies the new Level's required initial valuation.
    let extra = ExternalInputKey::<Level>::from_u128(100);
    let new_delay = ExternalOutputKey::<Pulse>::from_u128(101);
    let delayed_source = compiled
        .graph()
        .external_outputs()
        .iter()
        .find(|o| o.key() == delay.into())
        .unwrap()
        .source();
    let prepared = bound
        .prepare_patch(
            bound
                .machine()
                .patch()
                .add_external_input(ExternalInputDef::new(
                    extra.into(),
                    DiagnosticMeta::default(),
                ))
                .unwrap()
                .remove_external_output(delay.into())
                .unwrap()
                .add_external_output(ExternalOutputDef::new(
                    new_delay.into(),
                    delayed_source,
                    DiagnosticMeta::default(),
                ))
                .unwrap()
                .finish(),
        )
        .require_artifact()
        .unwrap();
    let target = BindingSet::builder(prepared.resulting_compiled())
        .bind_input(trip, "trip")
        .unwrap()
        .bind_input(extra, "enabled")
        .unwrap()
        .bind_output(lamp, "edited-lamp")
        .unwrap()
        .bind_output(new_delay, "new-delay")
        .unwrap()
        .finish()
        .unwrap();
    let input = target
        .input_projector(prepared.resulting_compiled())
        .unwrap()
        .delta_from([
            InputObservation::Level {
                input: "enabled",
                value: LogicLevel::High,
            },
            InputObservation::Pulse {
                input: "trip",
                count: PulseCount::ONE,
            },
        ])
        .unwrap();
    let tx = Transaction::advance(Time::from_ticks(6), bound.machine().revision(), input)
        .expect_execution_state(bound.machine().execution_state_digest())
        .with_patch(prepared, ReconfigurationPolicy::RejectStateLoss)
        .unwrap();

    // The core successor, complete target map and both output projections are ready
    // before either live part is published. A host can now consume owned output.
    let edited = bound.apply_reconfigured(tx, target).unwrap();
    assert!(
        matches!(&edited.projected_output_events()[0], ProjectedOutputEvent::Pulsed {
        endpoint, output: "old-delay", count, stamp, revision, ..
    } if *endpoint == delay && count.get() == 2 && stamp.time().ticks() == 5
        && *revision == edited.ordinary().before_revision())
    );
    assert!(
        matches!(&edited.projected_output_events()[1], ProjectedOutputEvent::LevelChanged {
        endpoint, output: "edited-lamp", to: LogicLevel::High, stamp, revision, ..
    } if *endpoint == lamp && stamp.time().ticks() == 6
        && *revision == edited.ordinary().after_revision())
    );
    let due = bound.advance(Time::from_ticks(11), []).unwrap();
    assert!(
        matches!(due.projected_output_events(), [ProjectedOutputEvent::Pulsed {
        endpoint, output: "new-delay", count, .. }] if *endpoint == new_delay && count.get() == 1)
    );

    // Results own their original labels and ordinary provenance after later edits.
    let cause = match &edited.projected_output_events()[0] {
        ProjectedOutputEvent::Pulsed { cause, .. } => *cause,
        _ => unreachable!(),
    };
    assert!(edited.ordinary().provenance().inspect(cause).is_ok());
    let (machine, bindings) = bound.into_parts();
    assert_eq!(machine.now(), Some(Time::from_ticks(11)));
    assert_eq!(bindings.output_identifier(new_delay), Some(&"new-delay"));
    println!("tick 5 retained old-delay; tick 6 used edited-lamp; tick 11 used new-delay");
}

#[test]
fn live_binding_usage_runs() {
    main();
}
