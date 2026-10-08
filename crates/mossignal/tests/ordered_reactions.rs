//! Public occurrence identity through input, native pulse algebra, forecast and replay.
use mossignal::key::{ExternalInputKey, ExternalOutputKey, NetworkKey, NodeKey};
use mossignal::metadata::DiagnosticMeta;
use mossignal::signal::{LogicLevel, Pulse, PulseCount};
use mossignal::time::{NonZeroSpan, Time};
use mossignal::{
    CauseInspection, CompiledNetwork, DecodePolicy, NetworkBuilder, OutputEvent,
    PersistenceContext, PulseDelayConfig, RuntimePolicy, TimeDomainId, ToggleConfig, Transaction,
    decode_replay_log, encode_replay_log, record_replay_log,
};

#[derive(Debug, PartialEq, Eq)]
enum Domain {}
fn policy() -> RuntimePolicy {
    RuntimePolicy::builder()
        .max_internal_reactions(100)
        .max_evaluated_operations(10_000)
        .max_pending_events(100)
        .max_events_created_per_transaction(1_000)
        .max_required_provenance_growth(10_000)
        .build()
        .unwrap()
}
fn limits() -> DecodePolicy {
    DecodePolicy::new(
        8_000_000, 64, 2_000_000, 8_000_000, 200_000, 10_000, 10_000, 10_000, 1_000, 10_000,
        100_000, 100_000, 10_000, 1_000, 1_000_000,
    )
}
fn circuit() -> CompiledNetwork<Domain> {
    let mut b = NetworkBuilder::with_key(NetworkKey::from_u128(1), TimeDomainId::from_u128(2));
    let signal = b
        .add_pulse_input(ExternalInputKey::from_u128(1), DiagnosticMeta::default())
        .unwrap();
    let toggle = b
        .add_toggle(
            NodeKey::from_u128(2),
            signal,
            ToggleConfig::new(LogicLevel::Low),
            DiagnosticMeta::default(),
        )
        .unwrap()
        .into_outputs();
    b.add_level_output(
        ExternalOutputKey::from_u128(3),
        toggle,
        DiagnosticMeta::default(),
    )
    .unwrap();
    let delayed = b
        .add_pulse_delay(
            NodeKey::from_u128(4),
            signal,
            PulseDelayConfig::new(NonZeroSpan::from_ticks(5).unwrap()),
            DiagnosticMeta::default(),
        )
        .unwrap()
        .into_outputs();
    b.add_pulse_output(
        ExternalOutputKey::from_u128(5),
        delayed,
        DiagnosticMeta::default(),
    )
    .unwrap();
    let parity = b
        .add_toggle(
            NodeKey::from_u128(6),
            delayed,
            ToggleConfig::new(LogicLevel::Low),
            DiagnosticMeta::default(),
        )
        .unwrap()
        .into_outputs();
    b.add_level_output(
        ExternalOutputKey::from_u128(7),
        parity,
        DiagnosticMeta::default(),
    )
    .unwrap();
    b.add_pulse_output(
        ExternalOutputKey::from_u128(8),
        signal,
        DiagnosticMeta::default(),
    )
    .unwrap();
    b.finish()
        .require_artifact()
        .unwrap()
        .compile()
        .require_artifact()
        .unwrap()
}
fn delta(c: &CompiledNetwork<Domain>, count: u64) -> mossignal::InputDelta<Domain> {
    let b = c.input_delta();
    if count == 0 {
        b.finish().unwrap()
    } else {
        b.pulse(
            ExternalInputKey::<Pulse>::from_u128(1),
            PulseCount::new(count),
        )
        .unwrap()
        .finish()
        .unwrap()
    }
}
fn initialize(c: &CompiledNetwork<Domain>, m: &mut mossignal::Machine<Domain>) {
    m.apply(Transaction::initialize(
        Time::from_ticks(0),
        m.revision(),
        c.input_snapshot().finish().unwrap(),
    ))
    .unwrap();
}
fn at(
    c: &CompiledNetwork<Domain>,
    m: &mossignal::Machine<Domain>,
    time: u64,
    count: u64,
) -> Transaction<Domain> {
    Transaction::advance(Time::from_ticks(time), m.revision(), delta(c, count))
}

#[test]
fn separate_calls_and_one_batch_have_native_toggle_and_delay_semantics() {
    let c = circuit();
    let mut m = c.spawn(policy());
    initialize(&c, &mut m);
    let first = m.apply(at(&c, &m, 0, 1)).unwrap();
    let second = m.apply(at(&c, &m, 0, 1)).unwrap();
    assert_eq!(
        m.output_level(ExternalOutputKey::from_u128(3)),
        Some(LogicLevel::Low)
    );
    for (result, order) in [(&first, 1), (&second, 2)] {
        let event = result
            .output_events()
            .iter()
            .find(|e| matches!(e,OutputEvent::Pulsed { output, .. } if output.as_u128()==8))
            .unwrap();
        assert_eq!(event.stamp().order(), order);
        let OutputEvent::Pulsed { cause, .. } = event else {
            unreachable!()
        };
        let CauseInspection::Derived { supporters, .. } =
            result.provenance().inspect(*cause).unwrap()
        else {
            panic!("export must retain its derived support")
        };
        assert!(supporters.iter().any(|root| matches!(result.provenance().inspect(*root).unwrap(), CauseInspection::ExternalPulseObservation { stamp, count, .. } if stamp==event.stamp() && count==PulseCount::ONE)));
        assert!(supporters.iter().any(|root| matches!(result.provenance().inspect(*root).unwrap(), CauseInspection::ReadyTransaction { at, .. } if at==event.stamp())));
    }
    assert_ne!(first.processed_reactions(), second.processed_reactions());
    let due = m.apply(at(&c, &m, 5, 0)).unwrap();
    assert!(matches!(due.output_events(),[OutputEvent::Pulsed { count, .. }] if count.get()==2));
    assert_eq!(
        m.output_level(ExternalOutputKey::from_u128(7)),
        Some(LogicLevel::Low)
    );
    assert!(
        m.apply(at(&c, &m, 5, 0))
            .unwrap()
            .output_events()
            .is_empty()
    );
    let mut batch = c.spawn(policy());
    initialize(&c, &mut batch);
    let simultaneous = batch.apply(at(&c, &batch, 0, 2)).unwrap();
    assert!(
        simultaneous
            .output_events()
            .iter()
            .all(|event| matches!(event, OutputEvent::Pulsed { .. }))
    );
}

#[test]
fn forecast_and_empty_calls_use_exact_freshness_without_live_allocation() {
    let c = circuit();
    let mut m = c.spawn(policy());
    initialize(&c, &mut m);
    let before = m.snapshot();
    let tx = at(&c, &m, 0, 1).expect_execution_state(m.execution_state_digest());
    let preview = m.forecast(tx.clone()).unwrap();
    assert_eq!(m.snapshot(), before);
    assert_eq!(preview.state().last_reaction().unwrap().order(), 1);
    m.apply(at(&c, &m, 0, 0)).unwrap();
    let committed = m.snapshot();
    assert_eq!(
        m.apply(tx).unwrap_err().code().as_str(),
        "runtime.stale_execution_state"
    );
    assert_eq!(m.snapshot(), committed);
    let tx = at(&c, &m, 0, 1);
    let preview = m.forecast(tx.clone()).unwrap();
    let real = m.apply(tx).unwrap();
    assert_eq!(
        preview.result().processed_reactions(),
        real.processed_reactions()
    );
    assert_eq!(
        preview.result().after_execution_digest(),
        real.after_execution_digest()
    );
}

#[test]
fn repeated_time_replay_crosses_the_encoded_boundary_in_frame_order() {
    let c = circuit();
    let mut m = c.spawn(policy());
    let revision = m.revision();
    let transactions = vec![
        Transaction::initialize(
            Time::from_ticks(0),
            revision,
            c.input_snapshot().finish().unwrap(),
        ),
        Transaction::advance(Time::from_ticks(0), revision, delta(&c, 1)),
        Transaction::advance(Time::from_ticks(0), revision, delta(&c, 1)),
        Transaction::advance(Time::from_ticks(5), revision, delta(&c, 0)),
        Transaction::advance(Time::from_ticks(5), revision, delta(&c, 0)),
    ];
    let log = record_replay_log(&mut m, transactions).unwrap();
    let context = PersistenceContext::new(c.time_domain_id());
    let bytes = encode_replay_log(&context, &log).unwrap();
    let decoded = decode_replay_log(&context, bytes.as_bytes(), &limits()).unwrap();
    let mut replayed = c.spawn(policy());
    replayed.replay_log(&decoded).unwrap();
    assert_eq!(replayed.snapshot(), m.snapshot());
    assert_eq!(replayed.last_reaction().unwrap().order(), 1);
    let mut restored = c.restore(m.snapshot(), policy()).unwrap();
    let next = restored.apply(at(&c, &restored, 5, 0)).unwrap();
    assert_eq!(next.processed_reactions()[0].order(), 2);
}

#[test]
fn late_time_failure_after_candidate_deadlines_publishes_no_occurrence() {
    let c = circuit();
    let mut m = c.spawn(policy());
    initialize(&c, &mut m);
    m.apply(at(&c, &m, 0, 1)).unwrap();
    let before = m.snapshot();
    let failure = m.apply(at(&c, &m, u64::MAX, 1)).unwrap_err();
    assert_eq!(failure.code().as_str(), "runtime.time_overflow");
    assert_eq!(m.snapshot(), before);
    let result = m.apply(at(&c, &m, 5, 0)).unwrap();
    assert_eq!(result.processed_reactions()[0].order(), 0);
}

#[test]
fn bounded_ordered_histories_match_an_independent_parity_and_maturity_model() {
    let c = circuit();
    for history in 0..64 {
        let mut machine = c.spawn(policy());
        initialize(&c, &mut machine);
        let mut total = 0;
        for step in 0..3 {
            let count = (history >> (step * 2)) & 3;
            let tx = at(&c, &machine, 0, count);
            let before = machine.snapshot();
            let preview = machine.forecast(tx.clone()).unwrap();
            assert_eq!(machine.snapshot(), before);
            let real = machine.apply(tx).unwrap();
            total += count;
            let expected = if total % 2 == 0 {
                LogicLevel::Low
            } else {
                LogicLevel::High
            };
            assert_eq!(
                machine.output_level(ExternalOutputKey::from_u128(3)),
                Some(expected)
            );
            assert_eq!(real.processed_reactions()[0].order(), step + 1);
            assert_eq!(
                real.after_execution_digest(),
                preview.result().after_execution_digest()
            );
        }
        let mut restored = c.restore(machine.snapshot(), policy()).unwrap();
        let result = restored.apply(at(&c, &restored, 5, 0)).unwrap();
        let due_count: u64 = result
            .output_events()
            .iter()
            .filter_map(|event| match event {
                OutputEvent::Pulsed { output, count, .. } if output.as_u128() == 5 => {
                    Some(count.get())
                }
                _ => None,
            })
            .sum();
        assert_eq!(due_count, total);
        assert_eq!(
            restored.output_level(ExternalOutputKey::from_u128(7)),
            Some(if total % 2 == 0 {
                LogicLevel::Low
            } else {
                LogicLevel::High
            })
        );
        assert!(
            restored
                .apply(at(&c, &restored, 5, 0))
                .unwrap()
                .output_events()
                .is_empty()
        );
    }
}
