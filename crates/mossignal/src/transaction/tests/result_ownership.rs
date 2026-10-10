//! Result membership is checked independently of shared publication/reference paths.
use super::*;
use crate::causal_work;
use crate::key::{ExternalInputKey, NodeKey};
use crate::metadata::DiagnosticMeta;
use crate::signal::{Level, LogicLevel, Pulse, PulseCount};
use crate::time::NonZeroSpan;
use crate::{
    FirstEmissionPolicy, NetworkBuilder, PeriodicConfig, ReenablePhasePolicy, TimeDomainId,
    ToggleConfig,
};

const HIDDEN: ExternalInputKey<Level> = ExternalInputKey::from_u128(1);
const VISIBLE: ExternalInputKey<Pulse> = ExternalInputKey::from_u128(2);
const TOGGLE: NodeKey = NodeKey::from_u128(10);

fn assert_manifest(r: &TransactionResult<()>) -> BTreeSet<CauseRef> {
    // Independent graph walk: do not call owned_roots or a publication helper.
    let mut pending: Vec<_> = r
        .output_events()
        .iter()
        .map(output_event_cause)
        .chain(
            r.diagnostic_episode_changes()
                .iter()
                .map(crate::DiagnosticEpisodeChange::cause),
        )
        .collect();
    let mut expected = BTreeSet::new();
    while let Some(cause) = pending.pop() {
        assert!(r.provenance().inspect(cause).is_ok());
        if expected.insert(cause) {
            let position = r.provenance().records.position(cause).unwrap();
            pending.extend(r.provenance().records[position].predecessor_causes());
        }
    }
    let actual = (0..r.provenance().len())
        .map(|p| r.provenance().records.cause(p))
        .collect();
    assert_eq!(expected, actual);
    expected
}

fn fixture() -> crate::CompiledNetwork<()> {
    let mut b = NetworkBuilder::new(TimeDomainId::from_u128(2));
    let enable = b
        .add_level_input(HIDDEN, DiagnosticMeta::default())
        .unwrap();
    let hidden = b
        .periodic(
            enable,
            PeriodicConfig::new(
                NonZeroSpan::from_ticks(1).unwrap(),
                FirstEmissionPolicy::AfterFirstPeriod,
                ReenablePhasePolicy::PreservePhase,
            ),
        )
        .unwrap();
    let visible = b
        .add_pulse_input(VISIBLE, DiagnosticMeta::default())
        .unwrap();
    b.add_toggle(
        TOGGLE,
        hidden,
        ToggleConfig::new(LogicLevel::Low),
        DiagnosticMeta::default(),
    )
    .unwrap();
    b.pulse_output("independent", visible).unwrap();
    b.finish()
        .require_artifact()
        .unwrap()
        .compile()
        .require_artifact()
        .unwrap()
}

fn seeded(c: &crate::CompiledNetwork<()>, history: u64) -> Machine<()> {
    let policy = crate::RuntimePolicy::builder()
        .max_internal_reactions(2000)
        .max_evaluated_operations(100_000)
        .max_pending_events(100)
        .max_events_created_per_transaction(2000)
        .max_required_provenance_growth(20_000)
        .build()
        .unwrap();
    let mut m = c.spawn(policy);
    drop(
        m.apply(Transaction::initialize(
            Time::from_ticks(0),
            m.revision(),
            c.input_snapshot()
                .set(HIDDEN, LogicLevel::High)
                .unwrap()
                .finish()
                .unwrap(),
        ))
        .unwrap(),
    );
    let built = m
        .apply(Transaction::advance(
            Time::from_ticks(history),
            m.revision(),
            c.input_delta().finish().unwrap(),
        ))
        .unwrap();
    assert!(built.processed_reactions().len() >= history as usize);
    drop(built);
    let hidden = m.inspect_toggle(TOGGLE).unwrap();
    assert!(hidden.provenance().len() > history as usize);
    m
}

#[test]
fn result_ownership_empty_stream_excludes_genuine_unrelated_history() {
    let c = fixture();
    let mut sizes = Vec::new();
    for history in [1, 10, 1000] {
        let mut m = seeded(&c, history);
        let hidden = m.inspect_toggle(TOGGLE).unwrap();
        let cause = hidden.latest_inversion().unwrap();
        causal_work::reset();
        let r = m
            .apply(Transaction::advance(
                Time::from_ticks(history + 1),
                m.revision(),
                c.input_delta().finish().unwrap(),
            ))
            .unwrap();
        let work = causal_work::read();
        assert_eq!(
            work.result_selection,
            causal_work::ResultSelectionWork::default()
        );
        assert!(work.logical_growth > 0 && work.catalogue_handles_copied > history as usize);
        assert_manifest(&r);
        eprintln!(
            "empty history={history}: selection={:?}, other closure visits={}, catalogue copies={}, growth={}",
            work.result_selection,
            work.owned_closure_records_visited,
            work.catalogue_handles_copied,
            work.logical_growth
        );
        assert!(r.output_events().is_empty());
        assert!(r.diagnostic_episode_changes().is_empty());
        assert!(hidden.provenance().inspect(cause).is_ok());
        sizes.push((
            history,
            r.provenance().len(),
            r.provenance().inspect(cause).is_ok(),
        ));
    }
    eprintln!("empty result ownership: {sizes:?}");
    assert!(
        sizes.iter().all(|(_, size, leaked)| *size == 0 && !leaked),
        "{sizes:?}"
    );
}

#[test]
fn result_ownership_fixed_event_excludes_genuine_unrelated_history() {
    let c = fixture();
    let mut sizes = Vec::new();
    for history in [1, 10, 1000] {
        let mut m = seeded(&c, history);
        let hidden = m.inspect_toggle(TOGGLE).unwrap();
        let cause = hidden.latest_inversion().unwrap();
        causal_work::reset();
        let r = m
            .apply(Transaction::advance(
                Time::from_ticks(history + 1),
                m.revision(),
                c.input_delta()
                    .pulse(VISIBLE, PulseCount::ONE)
                    .unwrap()
                    .finish()
                    .unwrap(),
            ))
            .unwrap();
        let work = causal_work::read();
        assert_eq!(r.output_events().len(), 1);
        let event = output_event_cause(&r.output_events()[0]);
        let expected = assert_manifest(&r);
        assert!(expected.contains(&event));
        assert_eq!(work.result_selection.root_visits, 1);
        assert_eq!(work.result_selection.records_selected, 3);
        assert_eq!(work.result_selection.handles_visited, 3);
        assert!(
            work.result_selection.content_visits > 0
                && work.result_selection.content_nodes_created > 0
        );
        assert!(work.logical_growth > 0 && work.catalogue_handles_copied > history as usize);
        eprintln!(
            "fixed event history={history}: selection={:?}, other closure visits={}, catalogue copies={}, growth={}",
            work.result_selection,
            work.owned_closure_records_visited - work.result_selection.records_selected,
            work.catalogue_handles_copied,
            work.logical_growth
        );
        sizes.push((
            history,
            r.provenance().len(),
            expected.len(),
            r.provenance().inspect(cause).is_ok(),
        ));
    }
    eprintln!("fixed event ownership: {sizes:?}");
    assert!(
        sizes
            .iter()
            .all(|(_, actual, expected, leaked)| actual == expected && !leaked),
        "{sizes:?}"
    );
}

#[test]
fn result_ownership_empty_forecast_and_views_release_unrelated_payload_and_index() {
    let c = fixture();
    let mut m = seeded(&c, 10);
    let before = super::tests::observe(&m);
    let forecast = m
        .forecast(Transaction::advance(
            Time::from_ticks(11),
            m.revision(),
            c.input_delta().finish().unwrap(),
        ))
        .unwrap();
    assert_eq!(super::tests::observe(&m), before);
    let (result, state, _) = forecast.into_parts();
    assert_manifest(&result);
    assert!(result.provenance().is_empty());
    let current = state.inspect_toggle(TOGGLE).unwrap();
    let cause = current.latest_inversion().unwrap();
    let view = current.provenance().clone();
    let position = view.records.position(cause).unwrap();
    let payload = Arc::downgrade(&view.records.canonical(position).get().unwrap().payload);
    drop(
        m.apply(Transaction::advance(
            Time::from_ticks(12),
            m.revision(),
            c.input_delta().finish().unwrap(),
        ))
        .unwrap(),
    );
    drop(state);
    drop(m);
    assert!(payload.upgrade().is_some());
    assert!(view.inspect(cause).is_ok());
    drop(current);
    assert!(view.inspect(cause).is_ok());
    drop(view);
    assert!(payload.upgrade().is_none());
    assert_eq!(causal_work::live_nodes(), 0);
    assert_eq!(causal_work::live_content_nodes(), 0);
    assert!(result.provenance().is_empty());
}

#[test]
fn result_ownership_deadlines_and_patch_stream_outlive_separate_current_inspections() {
    let c = super::cause_remapping_tests::fixture(1);
    let mut m = super::cause_remapping_tests::seeded(&c);
    let node = m.inspect_node(NodeKey::from_u128(12)).unwrap();
    let pending = m.inspect_pending_events().unwrap();
    assert!(!pending.is_empty());
    let source_revision = m.revision();
    let tx = super::cause_remapping_tests::preserving_patch(&m, 5);
    let before = super::tests::observe(&m);
    let forecast = m.forecast(tx.clone()).unwrap();
    assert_eq!(super::tests::observe(&m), before);
    assert_manifest(forecast.result());
    let r = m.apply(tx).unwrap();
    let causes = assert_manifest(&r);
    assert!(!causes.is_empty());
    assert_eq!(forecast.state().snapshot(), m.snapshot());
    assert_eq!(
        r.after_execution_digest(),
        forecast.result().after_execution_digest()
    );
    assert!(r.output_events().iter().any(|e| matches!(e, OutputEvent::Pulsed { stamp, revision, .. } if stamp.time().ticks()<5 && *revision==source_revision)));
    assert!(!r.diagnostic_episode_changes().is_empty());
    let owned = r.provenance().clone();
    let explanation = r.explain_output_event(0).unwrap();
    let event = output_event_cause(&r.output_events()[0]);
    let position = owned.records.position(event).unwrap();
    let payload = Arc::downgrade(&owned.records.canonical(position).get().unwrap().payload);
    for at in [8, 15, 101] {
        drop(
            m.apply(Transaction::advance(
                Time::from_ticks(at),
                m.revision(),
                m.compiled.input_delta().finish().unwrap(),
            ))
            .unwrap(),
        );
    }
    crate::state_digest_reference::assert_machine(&m);
    drop(m);
    drop(forecast);
    assert!(node.provenance().inspect(node.last_reaction_cause).is_ok());
    for p in &pending {
        assert!(p.provenance().inspect(p.cause).is_ok());
    }
    for cause in causes {
        assert!(owned.inspect(cause).is_ok());
    }
    drop(r);
    drop(node);
    drop(pending);
    drop(owned);
    assert!(payload.upgrade().is_some());
    assert!(explanation.causal.provenance().inspect(event).is_ok());
    drop(explanation);
    assert!(payload.upgrade().is_none());
    assert_eq!(causal_work::live_nodes(), 0);
    assert_eq!(causal_work::live_content_nodes(), 0);
}

#[test]
fn result_ownership_occurrences_and_unchanged_active_episodes_add_no_roots() {
    use crate::{ConflictPolicy, LevelSetResetConfig, PulseSetResetConfig};
    let mut b = NetworkBuilder::new(TimeDomainId::from_u128(2));
    let (set_key, set) = b.level_input("set");
    let (reset_key, reset) = b.level_input("reset");
    b.level_set_reset_latch(
        set,
        reset,
        LevelSetResetConfig::new(LogicLevel::Low, ConflictPolicy::RetainAndDiagnose),
    )
    .unwrap();
    let (ps_key, ps) = b.pulse_input("pulse-set");
    let (pr_key, pr) = b.pulse_input("pulse-reset");
    b.pulse_set_reset_latch(
        ps,
        pr,
        PulseSetResetConfig::new(LogicLevel::Low, ConflictPolicy::RetainAndDiagnose),
    )
    .unwrap();
    let c = b
        .finish()
        .require_artifact()
        .unwrap()
        .compile()
        .require_artifact()
        .unwrap();
    let mut m = c.spawn(super::cause_remapping_tests::policy());
    let began = m
        .apply(Transaction::initialize(
            Time::from_ticks(0),
            m.revision(),
            c.input_snapshot()
                .set(set_key, LogicLevel::High)
                .unwrap()
                .set(reset_key, LogicLevel::High)
                .unwrap()
                .finish()
                .unwrap(),
        ))
        .unwrap();
    assert_eq!(began.diagnostic_episode_changes().len(), 1);
    assert_manifest(&began);
    let active = m.active_diagnostic_episodes().unwrap().pop().unwrap();
    let quiet = m
        .apply(Transaction::advance(
            Time::from_ticks(1),
            m.revision(),
            c.input_delta()
                .pulse(ps_key, PulseCount::ONE)
                .unwrap()
                .pulse(pr_key, PulseCount::ONE)
                .unwrap()
                .finish()
                .unwrap(),
        ))
        .unwrap();
    assert_eq!(quiet.occurrences().len(), 1);
    assert!(quiet.diagnostic_episode_changes().is_empty());
    assert_manifest(&quiet);
    assert!(quiet.provenance().is_empty());
    let resolved = m
        .apply(Transaction::advance(
            Time::from_ticks(2),
            m.revision(),
            c.input_delta()
                .set(reset_key, LogicLevel::Low)
                .unwrap()
                .finish()
                .unwrap(),
        ))
        .unwrap();
    assert_eq!(resolved.diagnostic_episode_changes().len(), 1);
    assert_manifest(&resolved);
    drop(m);
    assert!(active.provenance().inspect(active.cause()).is_ok());
    assert_manifest(&began);
    assert_manifest(&resolved);
    assert_eq!(quiet.occurrences().len(), 1);
}
