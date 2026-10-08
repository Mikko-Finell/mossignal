//! A button pulse turns a lamp on after five caller-owned logical ticks.
//! Save while the pulse is pending, restore, preview the deadline, then commit.

use mossignal::key::{
    ExternalInputKey, ExternalOutputKey, InPortKey, NetworkKey, NodeKey, OutPortKey,
};
use mossignal::metadata::DiagnosticMeta;
use mossignal::signal::{Level, LogicLevel, Pulse, PulseCount};
use mossignal::time::{NonZeroSpan, Time};
use mossignal::{
    DecodePolicy, NetworkBuilder, NodeSubject, OutputEvent, PendingPayload, PersistenceContext,
    PulseDelayConfig, RuntimePolicy, TimeDomainId, ToggleConfig, Transaction, TransactionResult,
    decode_snapshot, encode_snapshot,
};

#[derive(Debug, PartialEq, Eq)]
struct Ticks;

fn assert_lamp_changed(result: &TransactionResult<Ticks>, lamp: ExternalOutputKey<Level>) {
    assert!(matches!(
        result.output_events(),
        [OutputEvent::LevelChanged { output, from: LogicLevel::Low,
            to: LogicLevel::High, at, .. }] if *output == lamp && at.ticks() == 5
    ));
}

fn main() {
    // The host chooses the time domain and stable identities. No wall clock runs here.
    let mut builder =
        NetworkBuilder::<Ticks>::with_key(NetworkKey::from_u128(1), TimeDomainId::from_u128(2));
    let button = ExternalInputKey::<Pulse>::from_u128(3);
    let delay_node = NodeKey::from_u128(4);
    let lamp = ExternalOutputKey::<Level>::from_u128(6);
    let pressed = builder
        .add_pulse_input(button, DiagnosticMeta::default())
        .unwrap();
    let delayed = builder
        .add_pulse_delay_with_ports(
            delay_node,
            InPortKey::from_u128(11),
            OutPortKey::from_u128(12),
            pressed,
            PulseDelayConfig {
                delay: NonZeroSpan::from_ticks(5).unwrap(),
            },
            DiagnosticMeta::default(),
        )
        .unwrap()
        .into_outputs();
    let lit = builder
        .add_toggle_with_ports(
            NodeKey::from_u128(5),
            InPortKey::from_u128(13),
            OutPortKey::from_u128(14),
            delayed,
            ToggleConfig {
                initial: LogicLevel::Low,
            },
            DiagnosticMeta::default(),
        )
        .unwrap()
        .into_outputs();
    builder
        .add_level_output(lamp, lit, DiagnosticMeta::default())
        .unwrap();
    let network = builder
        .finish()
        .require_artifact()
        .unwrap()
        .compile()
        .require_artifact()
        .unwrap();

    // These finite limits are ample for this example, not a recommended application policy.
    let policy = RuntimePolicy::builder()
        .max_internal_reactions(100)
        .max_evaluated_operations(1000)
        .max_pending_events(100)
        .max_events_created_per_transaction(100)
        .max_required_provenance_growth(1000)
        .build()
        .unwrap();
    let mut original = network.spawn(policy.clone());
    let inputs = network
        .input_snapshot()
        .pulse(button, PulseCount::new(1))
        .unwrap()
        .finish()
        .unwrap();
    original
        .apply(Transaction::initialize(
            Time::from_ticks(0),
            original.revision(),
            inputs,
        ))
        .unwrap();
    assert_eq!(original.output_level(lamp), Some(LogicLevel::Low));
    assert_eq!(original.next_deadline().unwrap(), Some(Time::from_ticks(5)));

    // Advancing time does not repeat the button pulse. It is reaction-scoped.
    let quiet = original
        .apply(Transaction::advance(
            Time::from_ticks(2),
            original.revision(),
            network.input_delta().finish().unwrap(),
        ))
        .unwrap();
    assert!(quiet.output_events().is_empty());
    let pending = original.inspect_pending_events().unwrap();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].owner, NodeSubject::Node(delay_node));
    assert_eq!(pending[0].origin.ticks(), 0);
    assert_eq!(pending[0].deadline.ticks(), 5);
    assert!(matches!(pending[0].payload, PendingPayload::Pulse(count) if count.get() == 1));
    println!("tick 2: lamp is Low; one pulse is pending for tick 5");

    // InputSnapshot initializes inputs; MachineSnapshot preserves the whole machine.
    // The host owns storage. This demonstration keeps the saved artifact in memory.
    let saved = original.snapshot();
    let context = PersistenceContext::new(network.time_domain_id());
    let bytes = encode_snapshot(&context, &saved).unwrap();
    let limits = DecodePolicy::new(
        1_000_000, // total artifact bytes
        64,        // nesting depth
        16_384,    // text bytes
        1_000_000, // byte-string bytes
        10_000,    // collection items
        16,        // nodes
        32,        // ports
        32,        // connections
        0,         // module instances
        16,        // pending events
        1000,      // provenance records
        2000,      // provenance edges
        0,         // diagnostic records
        0,         // replay frames
        0,         // optional history bytes
    );
    let decoded = decode_snapshot(&context, bytes.as_bytes(), &limits).unwrap();
    let mut resumed = network.restore(decoded, policy).unwrap();
    assert_eq!(resumed.snapshot(), saved);
    assert_eq!(resumed.now(), Some(Time::from_ticks(2)));
    assert_eq!(resumed.output_level(lamp), Some(LogicLevel::Low));
    let restored_pending = resumed.inspect_pending(pending[0].event).unwrap();
    assert_eq!(restored_pending.deadline, pending[0].deadline);
    assert!(matches!(restored_pending.payload, PendingPayload::Pulse(count) if count.get() == 1));
    println!("restored tick 2 with the same pending event and stored lamp state");

    // A forecast is a preview. Committing it requires an explicit new apply call.
    let due = Transaction::advance(
        Time::from_ticks(5),
        resumed.revision(),
        network.input_delta().finish().unwrap(),
    );
    let before_forecast = resumed.snapshot();
    let preview = resumed.forecast(due.clone()).unwrap();
    assert_eq!(resumed.snapshot(), before_forecast);
    assert_lamp_changed(preview.result(), lamp);
    assert_eq!(
        preview.state().inspect_output(lamp).unwrap().level,
        Some(LogicLevel::High)
    );
    println!("forecast tick 5: lamp would become High; live machine is still at tick 2");

    let committed = resumed.apply(due.clone()).unwrap();
    let uninterrupted = original.apply(due).unwrap();
    assert_lamp_changed(&committed, lamp);
    assert_lamp_changed(&uninterrupted, lamp);
    assert_eq!(resumed.snapshot(), original.snapshot());
    assert_eq!(
        resumed.execution_state_digest(),
        preview.state().execution_state_digest()
    );
    assert_eq!(
        resumed.observable_state_digest(),
        preview.state().observable_state_digest()
    );
    assert!(resumed.inspect_pending_events().unwrap().is_empty());

    let observation = resumed.inspect_output(lamp).unwrap();
    assert_eq!(observation.at.ticks(), 5);
    assert_eq!(observation.level, Some(LogicLevel::High));
    assert!(
        observation
            .provenance()
            .inspect(observation.latest_transition.unwrap())
            .is_ok()
    );
    // This earlier owned observation retains its cause even after the event fires.
    assert!(pending[0].provenance().inspect(pending[0].cause).is_ok());
    println!("committed tick 5: lamp is High; resumed and uninterrupted execution agree");
}

#[test]
fn lifecycle_usage_runs() {
    main();
}
