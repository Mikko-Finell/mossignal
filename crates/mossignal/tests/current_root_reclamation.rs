use mossignal::key::*;
use mossignal::metadata::DiagnosticMeta;
use mossignal::signal::{Level, LogicLevel, PulseCount};
use mossignal::time::NonZeroSpan;
use mossignal::time::Time;
use mossignal::*;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
#[path = "support/causal.rs"]
mod causal;

fn policy() -> RuntimePolicy {
    RuntimePolicy::builder()
        .max_internal_reactions(100)
        .max_evaluated_operations(100_000)
        .max_pending_events(100)
        .max_events_created_per_transaction(10_000)
        .max_required_provenance_growth(100_000)
        .build()
        .unwrap()
}
fn compiled() -> CompiledNetwork<()> {
    let mut b = NetworkBuilder::with_key(NetworkKey::from_u128(1), TimeDomainId::from_u128(2));
    let p = b
        .add_pulse_input(ExternalInputKey::from_u128(10), DiagnosticMeta::default())
        .unwrap();
    for (module, reset, output) in [(20, 11, 30), (21, 12, 31)] {
        let r = b
            .add_pulse_input(
                ExternalInputKey::from_u128(reset),
                DiagnosticMeta::default(),
            )
            .unwrap();
        let out = b
            .add_pulse_resettable_toggle(
                ModuleInstanceKey::from_u128(module),
                p,
                r,
                LogicLevel::Low,
                DiagnosticMeta::default(),
            )
            .unwrap()
            .into_outputs();
        b.add_level_output(
            ExternalOutputKey::from_u128(output),
            out,
            DiagnosticMeta::default(),
        )
        .unwrap();
    }
    let q = b
        .add_pulse_input(ExternalInputKey::from_u128(13), DiagnosticMeta::default())
        .unwrap();
    let delayed = b
        .add_pulse_delay(
            NodeKey::from_u128(22),
            q,
            PulseDelayConfig::new(NonZeroSpan::from_ticks(100).unwrap()),
            DiagnosticMeta::default(),
        )
        .unwrap()
        .into_outputs();
    b.add_pulse_output(
        ExternalOutputKey::from_u128(32),
        delayed,
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
fn apply(
    c: &CompiledNetwork<()>,
    m: &mut Machine<()>,
    at: u64,
    p: u64,
    r1: u64,
    r2: u64,
) -> ReplayFrame<()> {
    let input = c
        .input_delta()
        .pulse(ExternalInputKey::from_u128(10), PulseCount::new(p))
        .unwrap()
        .pulse(ExternalInputKey::from_u128(11), PulseCount::new(r1))
        .unwrap()
        .pulse(ExternalInputKey::from_u128(12), PulseCount::new(r2))
        .unwrap()
        .finish()
        .unwrap();
    m.apply_recorded(Transaction::advance(
        Time::from_ticks(at),
        m.revision(),
        input,
    ))
    .unwrap()
    .frame()
    .clone()
}
fn history(c: &CompiledNetwork<()>, left: bool) -> Machine<()> {
    history_recorded(c, left).0
}
fn history_recorded(c: &CompiledNetwork<()>, left: bool) -> (Machine<()>, Vec<ReplayFrame<()>>) {
    let mut m = c.spawn(policy());
    let input = c
        .input_snapshot()
        .pulse(ExternalInputKey::from_u128(13), PulseCount::new(1))
        .unwrap()
        .finish()
        .unwrap();
    let init = m
        .apply_recorded(Transaction::initialize(
            Time::from_ticks(0),
            m.revision(),
            input,
        ))
        .unwrap()
        .frame()
        .clone();
    let frames = vec![
        init,
        apply(c, &mut m, 1, 2, 0, 0),
        apply(c, &mut m, 2, 2, u64::from(left), u64::from(!left)),
        apply(c, &mut m, 3, 0, 1, 1),
        apply(c, &mut m, 4, 0, 0, 0),
    ];
    (m, frames)
}
type Sem = Arc<causal::SemanticCause>;
fn roots(m: &Machine<()>) -> (BTreeSet<Sem>, BTreeMap<u128, Sem>) {
    let mut all = BTreeSet::new();
    let mut roles = BTreeMap::new();
    for module in [20, 21] {
        let view = m
            .inspect_module(ModuleInstanceKey::from_u128(module))
            .unwrap();
        let s = view.stateful_standard().unwrap();
        assert_eq!(s.state, LogicLevel::Low);
        let toggle = s.latest_accepted_toggle_cause.unwrap();
        roles.insert(module, causal::semantic(&s.provenance, toggle));
        for cause in [
            s.latest_reset_cause,
            s.latest_accepted_toggle_cause,
            s.latest_capture_cause,
        ]
        .into_iter()
        .flatten()
        .chain(s.public_causes.iter().map(|(_, v)| *v))
        .chain(s.internal_causes.iter().map(|(_, v)| *v))
        {
            all.insert(causal::semantic(&s.provenance, cause));
        }
        for n in view.nodes() {
            let n = m.inspect_qualified_node(n.node().clone()).unwrap();
            let roots = [
                Some(n.last_reaction_cause),
                n.current_support,
                n.latest_transition,
            ]
            .into_iter()
            .flatten()
            .chain(n.inputs.iter().filter_map(|v| v.current_support))
            .chain(n.outputs.iter().filter_map(|v| v.current_support));
            for cause in roots {
                all.insert(causal::semantic(n.provenance(), cause));
            }
        }
    }
    for output in [30, 31] {
        let o = m
            .inspect_output(ExternalOutputKey::<Level>::from_u128(output))
            .unwrap();
        for cause in [o.current_support, o.latest_transition]
            .into_iter()
            .flatten()
        {
            all.insert(causal::semantic(o.provenance(), cause));
        }
    }
    let n = m.inspect_node(NodeKey::from_u128(22)).unwrap();
    for cause in [
        Some(n.last_reaction_cause),
        n.current_support,
        n.latest_transition,
    ]
    .into_iter()
    .flatten()
    .chain(n.inputs.iter().filter_map(|v| v.current_support))
    .chain(n.outputs.iter().filter_map(|v| v.current_support))
    {
        all.insert(causal::semantic(n.provenance(), cause));
    }
    for p in m.inspect_pending_events().unwrap() {
        all.insert(causal::semantic(p.provenance(), p.cause));
    }
    (all, roles)
}
fn preserve(m: &mut Machine<()>, at: u64, output: u128) -> CompiledNetwork<()> {
    let source = m.compiled().graph().external_outputs()[0].source();
    let patch = m
        .patch()
        .add_external_output(mossignal::authored::ExternalOutputDef::new(
            ExternalOutputKey::<Level>::from_u128(output).into(),
            source,
            DiagnosticMeta::default(),
        ))
        .unwrap()
        .finish();
    let prepared = m.prepare_patch(patch).require_artifact().unwrap();
    let target = prepared.resulting_compiled().clone();
    let input = target.input_delta().finish().unwrap();
    drop(
        m.apply(
            Transaction::advance(Time::from_ticks(at), m.revision(), input)
                .with_patch(prepared, ReconfigurationPolicy::RejectStateLoss)
                .unwrap(),
        )
        .unwrap(),
    );
    target
}
fn persisted(frame: &ReplayFrame<()>, c: &CompiledNetwork<()>) -> ReplayFrame<()> {
    let context = PersistenceContext::new(c.time_domain_id());
    let limits = DecodePolicy::new(
        8_000_000, 64, 2_000_000, 8_000_000, 200_000, 10_000, 10_000, 10_000, 1_000, 10_000,
        100_000, 100_000, 10_000, 8, 1_000_000,
    );
    let bytes = encode_replay_frame(&context, frame).unwrap();
    decode_replay_frame(&context, bytes.as_bytes(), &limits).unwrap()
}
#[test]
fn zero_pulses_and_omission_have_equal_current_roles_and_encoded_replay() {
    let c = compiled();
    let mut explicit = c.spawn(policy());
    let mut omitted = c.spawn(policy());
    let mut replayed = c.spawn(policy());
    let mut input = c.input_snapshot();
    for key in 10..=13 {
        input = input
            .pulse(ExternalInputKey::from_u128(key), PulseCount::ZERO)
            .unwrap();
    }
    let transaction = Transaction::initialize(
        Time::from_ticks(0),
        explicit.revision(),
        input.finish().unwrap(),
    );
    let forecast = explicit.forecast(transaction.clone()).unwrap();
    let frame = explicit
        .apply_recorded(transaction)
        .unwrap()
        .frame()
        .clone();
    omitted
        .apply(Transaction::initialize(
            Time::from_ticks(0),
            omitted.revision(),
            c.input_snapshot().finish().unwrap(),
        ))
        .unwrap();
    assert_eq!(
        explicit.snapshot(),
        omitted.snapshot(),
        "zero initialization"
    );
    assert_eq!(explicit.snapshot(), forecast.state().snapshot());
    replayed.replay(&[persisted(&frame, &c)]).unwrap();
    assert_eq!(explicit.snapshot(), replayed.snapshot());
    for at in 1..=2 {
        let mut input = c.input_delta();
        for key in 10..=13 {
            input = input
                .pulse(ExternalInputKey::from_u128(key), PulseCount::ZERO)
                .unwrap();
        }
        let transaction = Transaction::advance(
            Time::from_ticks(at),
            explicit.revision(),
            input.finish().unwrap(),
        );
        let forecast = explicit.forecast(transaction.clone()).unwrap();
        let frame = explicit
            .apply_recorded(transaction)
            .unwrap()
            .frame()
            .clone();
        omitted
            .apply(Transaction::advance(
                Time::from_ticks(at),
                omitted.revision(),
                c.input_delta().finish().unwrap(),
            ))
            .unwrap();
        assert_eq!(explicit.snapshot(), omitted.snapshot(), "zero delta");
        assert_eq!(explicit.snapshot(), forecast.state().snapshot());
        replayed.replay(&[persisted(&frame, &c)]).unwrap();
        assert_eq!(explicit.snapshot(), replayed.snapshot());
        assert_eq!(
            explicit.execution_state_digest(),
            omitted.execution_state_digest()
        );
        assert_eq!(
            explicit.observable_state_digest(),
            omitted.observable_state_digest()
        );
        explicit = c.restore(explicit.snapshot(), policy()).unwrap();
    }
}
#[test]
fn current_roles_determine_future_migration_and_freshness() {
    let c = compiled();
    let (mut a, frames) = history_recorded(&c, true);
    let mut replayed = c.spawn(policy());
    for frame in &frames {
        let decoded = persisted(frame, &c);
        replayed.replay(std::slice::from_ref(&decoded)).unwrap();
    }
    assert_eq!(a.snapshot(), replayed.snapshot());
    assert_eq!(roots(&a), roots(&replayed));
    let mut b = history(&c, false);
    let (ar, aa) = roots(&a);
    let (br, ba) = roots(&b);
    assert_eq!(
        ar, br,
        "unlabeled current root sets and complete ancestry must coincide"
    );
    assert_ne!(
        aa, ba,
        "qualified latest-accepted-toggle role assignments must differ"
    );
    assert_ne!(
        a.execution_state_digest(),
        b.execution_state_digest(),
        "future-determining roles must enter E3"
    );
    assert_ne!(a.observable_state_digest(), b.observable_state_digest());
    assert_ne!(a.snapshot(), b.snapshot());
    let mut restored = c.restore(a.snapshot(), policy()).unwrap();
    assert_eq!(
        a.snapshot(),
        restored.snapshot(),
        "exact current-role artifact roundtrip"
    );
    assert_eq!(
        a.execution_state_digest(),
        restored.execution_state_digest()
    );
    println!(
        "before continuation: {} identical unlabeled roots and ancestry; role assignments differ; v3 role bindings distinguish otherwise identical root unions",
        ar.len()
    );
    apply(&c, &mut a, 5, 2, 0, 1);
    apply(&c, &mut b, 5, 2, 0, 1);
    apply(&c, &mut a, 6, 0, 0, 0);
    apply(&c, &mut b, 6, 0, 0, 0);
    apply(&c, &mut restored, 5, 2, 0, 1);
    apply(&c, &mut restored, 6, 0, 0, 0);
    apply(&c, &mut replayed, 5, 2, 0, 1);
    apply(&c, &mut replayed, 6, 0, 0, 0);
    assert_eq!(a.snapshot(), replayed.snapshot());
    assert_eq!(roots(&a), roots(&restored));
    assert_eq!(a.snapshot(), restored.snapshot());
    let (ar, _) = roots(&a);
    let (br, _) = roots(&b);
    assert_ne!(
        ar, br,
        "selective role replacement exposes the lost association"
    );
    println!("after identical selective acceptance and quiet reaction: source root sets differ");
    let before_a = a.inspect_pending_events().unwrap().remove(0);
    let before_b = b.inspect_pending_events().unwrap().remove(0);
    assert_eq!(
        causal::semantic(before_a.provenance(), before_a.cause),
        causal::semantic(before_b.provenance(), before_b.cause)
    );
    drop((before_a, before_b));
    let ca = preserve(&mut a, 7, 33);
    let cb = preserve(&mut b, 7, 33);
    let cr = preserve(&mut restored, 7, 33);
    let cp = preserve(&mut replayed, 7, 33);
    assert_eq!(a.snapshot(), replayed.snapshot());
    assert_eq!(a.snapshot(), restored.snapshot());
    let pa = a.inspect_pending_events().unwrap().remove(0);
    let pb = b.inspect_pending_events().unwrap().remove(0);
    assert_eq!(pa.event, pb.event);
    assert_eq!(pa.owner, pb.owner);
    assert_eq!(pa.origin, pb.origin);
    assert_eq!(pa.deadline, pb.deadline);
    assert_eq!(pa.revision, pb.revision);
    assert!(matches!(pa.payload, PendingPayload::Pulse(count) if count == PulseCount::new(1)));
    assert!(matches!(pb.payload, PendingPayload::Pulse(count) if count == PulseCount::new(1)));
    assert_ne!(
        causal::semantic(pa.provenance(), pa.cause),
        causal::semantic(pb.provenance(), pb.cause)
    );
    assert_ne!(a.execution_state_digest(), b.execution_state_digest());
    println!(
        "after identical preserving patch: pending identity/origin/deadline/count agree; pending causes and execution digests differ"
    );
    let guard = a.execution_state_digest();
    let before_b = b.snapshot();
    drop(
        a.apply(
            Transaction::advance(
                Time::from_ticks(8),
                a.revision(),
                ca.input_delta().finish().unwrap(),
            )
            .expect_execution_state(guard),
        )
        .unwrap(),
    );
    let failed = b
        .apply(
            Transaction::advance(
                Time::from_ticks(8),
                b.revision(),
                cb.input_delta().finish().unwrap(),
            )
            .expect_execution_state(guard),
        )
        .err()
        .unwrap();
    assert_eq!(failed.code().as_str(), "runtime.stale_execution_state");
    assert_eq!(before_b, b.snapshot());
    drop(
        restored
            .apply(
                Transaction::advance(
                    Time::from_ticks(8),
                    restored.revision(),
                    cr.input_delta().finish().unwrap(),
                )
                .expect_execution_state(guard),
            )
            .unwrap(),
    );
    replayed
        .apply(
            Transaction::advance(
                Time::from_ticks(8),
                replayed.revision(),
                cp.input_delta().finish().unwrap(),
            )
            .expect_execution_state(guard),
        )
        .unwrap();
    assert_eq!(a.snapshot(), replayed.snapshot());
    let mut second_restore = ca.restore(a.snapshot(), policy()).unwrap();
    let ca = preserve(&mut a, 9, 34);
    let _ = preserve(&mut restored, 9, 34);
    let _ = preserve(&mut second_restore, 9, 34);
    let _ = preserve(&mut replayed, 9, 34);
    assert_eq!(a.snapshot(), replayed.snapshot());
    assert_eq!(a.snapshot(), restored.snapshot());
    assert_eq!(a.snapshot(), second_restore.snapshot());
    apply(&ca, &mut a, 10, 2, 1, 0);
    apply(&ca, &mut second_restore, 10, 2, 1, 0);
    assert_eq!(roots(&a), roots(&second_restore));
    assert_eq!(a.snapshot(), second_restore.snapshot());
    println!(
        "same expected-execution guard: first machine accepts; second rejects runtime.stale_execution_state atomically"
    );
}
