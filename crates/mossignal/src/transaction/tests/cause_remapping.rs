//! Actual identity-work boundaries and independent membership/lifetime controls.
use super::*;
use crate::causal_work::{self, ObsoleteRemaps, Work};
use crate::key::{ExternalInputKey, ModuleInstanceKey, NodeKey};
use crate::metadata::DiagnosticMeta;
use crate::signal::{Level, LogicLevel, Pulse, PulseCount};
use crate::time::{NonZeroSpan, Time};
use crate::*;

const SET: ExternalInputKey<Level> = ExternalInputKey::from_u128(1);
const RESET: ExternalInputKey<Level> = ExternalInputKey::from_u128(2);
const ENABLE: ExternalInputKey<Level> = ExternalInputKey::from_u128(3);
const PULSE: ExternalInputKey<Pulse> = ExternalInputKey::from_u128(4);
const RESET_PULSE: ExternalInputKey<Pulse> = ExternalInputKey::from_u128(5);
const LATCH: NodeKey = NodeKey::from_u128(12);

pub(super) fn policy() -> RuntimePolicy {
    RuntimePolicy::builder()
        .max_internal_reactions(100)
        .max_evaluated_operations(100_000)
        .max_pending_events(1000)
        .max_events_created_per_transaction(2000)
        .max_required_provenance_growth(20_000)
        .build()
        .unwrap()
}

fn span(ticks: u64) -> NonZeroSpan<()> {
    NonZeroSpan::from_ticks(ticks).unwrap()
}

pub(super) fn fixture(groups: u64) -> CompiledNetwork<()> {
    let mut b = NetworkBuilder::with_key(
        crate::key::NetworkKey::from_u128(1),
        TimeDomainId::from_u128(2),
    );
    let set = b.add_level_input(SET, DiagnosticMeta::default()).unwrap();
    let reset = b.add_level_input(RESET, DiagnosticMeta::default()).unwrap();
    let enable = b
        .add_level_input(ENABLE, DiagnosticMeta::default())
        .unwrap();
    let pulse = b.add_pulse_input(PULSE, DiagnosticMeta::default()).unwrap();
    let reset_pulse = b
        .add_pulse_input(RESET_PULSE, DiagnosticMeta::default())
        .unwrap();
    let delayed_set = b
        .add_transport_delay(
            NodeKey::from_u128(10),
            set,
            TransportDelayConfig::new(span(2), LogicLevel::Low),
            DiagnosticMeta::default(),
        )
        .unwrap()
        .into_outputs();
    let delayed_reset = b
        .add_transport_delay(
            NodeKey::from_u128(11),
            reset,
            TransportDelayConfig::new(span(3), LogicLevel::Low),
            DiagnosticMeta::default(),
        )
        .unwrap()
        .into_outputs();
    let latch = b
        .add_level_set_reset_latch(
            LATCH,
            delayed_set,
            delayed_reset,
            LevelSetResetConfig::new(LogicLevel::Low, ConflictPolicy::RetainAndDiagnose),
            DiagnosticMeta::default(),
        )
        .unwrap()
        .into_outputs();
    b.level_output("latch", latch).unwrap();
    let edge = b
        .any_edge(delayed_set, EdgeConfig::new(EdgeInitialization::Baseline))
        .unwrap();
    b.pulse_output("edge", edge).unwrap();
    let inertial = b
        .inertial_delay(set, InertialDelayConfig::new(span(6), LogicLevel::Low))
        .unwrap();
    b.level_output("inertial", inertial).unwrap();
    let periodic = b
        .periodic(
            enable,
            PeriodicConfig::new(
                span(7),
                FirstEmissionPolicy::AfterFirstPeriod,
                ReenablePhasePolicy::PreservePhase,
            ),
        )
        .unwrap();
    b.pulse_output("periodic", periodic).unwrap();
    let short = b
        .pulse_delay(pulse, PulseDelayConfig::new(span(2)))
        .unwrap();
    b.pulse_output("short", short).unwrap();
    for i in 0..groups {
        let future = b
            .pulse_delay(pulse, PulseDelayConfig::new(span(100 + i)))
            .unwrap();
        b.pulse_output(format!("future-{i}"), future).unwrap();
    }
    let pulse_toggle = b
        .add_pulse_resettable_toggle(
            ModuleInstanceKey::from_u128(100),
            pulse,
            reset_pulse,
            LogicLevel::Low,
            DiagnosticMeta::default(),
        )
        .unwrap()
        .into_outputs();
    let level_toggle = b
        .add_level_resettable_toggle(
            ModuleInstanceKey::from_u128(101),
            pulse,
            reset,
            LogicLevel::Low,
            DiagnosticMeta::default(),
        )
        .unwrap()
        .into_outputs();
    let held = b
        .add_level_resettable_sample_hold(
            ModuleInstanceKey::from_u128(102),
            set,
            pulse,
            reset,
            LogicLevel::Low,
            LogicLevel::Low,
            DiagnosticMeta::default(),
        )
        .unwrap()
        .into_outputs();
    b.level_output("pulse-toggle", pulse_toggle).unwrap();
    b.level_output("level-toggle", level_toggle).unwrap();
    b.level_output("held", held).unwrap();
    b.finish()
        .require_artifact()
        .unwrap()
        .compile()
        .require_artifact()
        .unwrap()
}

fn initialization(m: &Machine<()>) -> Transaction<()> {
    Transaction::initialize(
        Time::from_ticks(0),
        m.revision(),
        m.compiled
            .input_snapshot()
            .set(SET, LogicLevel::High)
            .unwrap()
            .set(RESET, LogicLevel::High)
            .unwrap()
            .set(ENABLE, LogicLevel::High)
            .unwrap()
            .pulse(PULSE, PulseCount::new(2))
            .unwrap()
            .finish()
            .unwrap(),
    )
}

fn advance(m: &Machine<()>, at: u64) -> Transaction<()> {
    Transaction::advance(
        Time::from_ticks(at),
        m.revision(),
        m.compiled.input_delta().finish().unwrap(),
    )
}

pub(super) fn seeded(c: &CompiledNetwork<()>) -> Machine<()> {
    let mut m = c.spawn(policy());
    drop(m.apply(initialization(&m)).unwrap());
    let tx = Transaction::advance(
        Time::from_ticks(1),
        m.revision(),
        c.input_delta()
            .set(RESET, LogicLevel::Low)
            .unwrap()
            .pulse(PULSE, PulseCount::new(2))
            .unwrap()
            .pulse(RESET_PULSE, PulseCount::ONE)
            .unwrap()
            .finish()
            .unwrap(),
    );
    drop(m.apply(tx).unwrap());
    assert_eq!(m.store.standard_causes.len(), 3);
    assert!(
        m.store
            .standard_causes
            .values()
            .all(|r| r.latest_reset.is_some())
    );
    assert_eq!(
        m.store
            .standard_causes
            .values()
            .filter(|r| r.latest_toggle.is_some())
            .count(),
        2
    );
    assert_eq!(
        m.store
            .standard_causes
            .values()
            .filter(|r| r.latest_capture.is_some())
            .count(),
        1
    );
    m
}

fn no_obsolete(work: Work) {
    assert_eq!(work.obsolete_remaps, ObsoleteRemaps::default(), "{work:?}");
    assert_eq!(work.different_lineage_remaps, 0);
    assert_eq!(work.freeze.old_records_indexed, 0);
    assert_eq!(work.freeze.scratch_slots, 0);
    assert_eq!(work.freeze.cached_payload_visits, 0);
    assert_eq!(work.freeze.cached_payload_insertions, 0);
    assert_eq!(work.freeze.old_source_visits, 0);
    assert_eq!(work.freeze.new_nodes, work.nodes_created);
}

fn check_jump(r: &TransactionResult<()>) {
    assert_eq!(
        r.processed_reactions()
            .iter()
            .map(|s| s.time().ticks())
            .collect::<Vec<_>>(),
        [2, 3, 4, 5]
    );
    assert_eq!(
        r.diagnostic_episode_changes()
            .iter()
            .map(|c| (c.kind(), c.at().ticks()))
            .collect::<Vec<_>>(),
        [
            (DiagnosticEpisodeChangeKind::Began, 3),
            (DiagnosticEpisodeChangeKind::Resolved, 4)
        ]
    );
    assert!(
        r.output_events()
            .iter()
            .filter(|e| e.at().ticks() < 5)
            .count()
            >= 4
    );
    for cause in r.output_events().iter().map(output_event_cause).chain(
        r.diagnostic_episode_changes()
            .iter()
            .map(DiagnosticEpisodeChange::cause),
    ) {
        assert!(r.provenance().inspect(cause).is_ok());
    }
}

#[test]
fn ordinary_remap_initialization_has_zero_identity_work() {
    let mut measurements = Vec::new();
    for groups in [1, 8, 32] {
        let c = fixture(groups);
        let mut m = c.spawn(policy());
        let before = super::tests::observe(&m);
        causal_work::reset();
        let forecast = m.forecast(initialization(&m)).unwrap();
        let fw = causal_work::read();
        assert_eq!(super::tests::observe(&m), before);
        causal_work::reset();
        let result = m.apply(initialization(&m)).unwrap();
        let aw = causal_work::read();
        assert_eq!(forecast.state().snapshot(), m.snapshot());
        assert_eq!(
            forecast.result().after_observable_digest(),
            result.after_observable_digest()
        );
        assert_eq!(
            m.store.pending_events.values().map(Vec::len).sum::<usize>(),
            groups as usize + 5
        );
        eprintln!("ordinary remap init groups={groups}, forecast={fw:?}, apply={aw:?}");
        measurements.extend([fw, aw]);
    }
    for work in measurements {
        no_obsolete(work);
    }
}

#[test]
fn ordinary_remap_deadlines_and_future_groups_have_zero_identity_work() {
    let mut measurements = Vec::new();
    for groups in [1, 8, 32] {
        let c = fixture(groups);
        let mut m = seeded(&c);
        let before = super::tests::observe(&m);
        let prior = m.store.provenance.as_ref().unwrap().clone();
        let input = m.store.input_causes[&SET];
        let future = m.store.pending_events[&Time::from_ticks(100)][0]
            .identity()
            .5;
        let standard = m.store.standard_causes.clone();
        causal_work::reset();
        let forecast = m.forecast(advance(&m, 5)).unwrap();
        let fw = causal_work::read();
        assert_eq!(super::tests::observe(&m), before);
        check_jump(forecast.result());
        causal_work::reset();
        let result = m.apply(advance(&m, 5)).unwrap();
        let aw = causal_work::read();
        check_jump(&result);
        assert_eq!(forecast.state().snapshot(), m.snapshot());
        assert_eq!(m.store.standard_causes, standard);
        assert_eq!(m.store.input_causes[&SET], input);
        assert_eq!(
            m.store.pending_events[&Time::from_ticks(100)][0]
                .identity()
                .5,
            future
        );
        let current = m.store.provenance.as_ref().unwrap();
        for cause in [input, future]
            .into_iter()
            .chain(standard.values().flat_map(|r| r.retained_causes()))
        {
            assert!(current.records.shared_node(prior.records(), cause));
            assert!(current.inspect(cause).is_ok());
        }
        assert_eq!(
            m.store.pending_events.values().map(Vec::len).sum::<usize>(),
            2 * groups as usize + 2
        );
        crate::state_digest_reference::assert_machine(&m);
        eprintln!("ordinary remap jump groups={groups}, forecast={fw:?}, apply={aw:?}");
        measurements.extend([fw, aw]);
    }
    for work in measurements {
        no_obsolete(work);
    }
}

pub(super) fn round_trip(m: &Machine<()>) -> Machine<()> {
    let context = PersistenceContext::new(m.compiled.time_domain_id());
    let encoded = encode_snapshot(&context, &m.snapshot()).unwrap();
    let snapshot = decode_snapshot(
        &context,
        encoded.as_bytes(),
        &DecodePolicy::new(
            8_000_000, 64, 2_000_000, 8_000_000, 200_000, 10_000, 10_000, 10_000, 1_000, 10_000,
            100_000, 100_000, 10_000, 1_000, 1_000_000,
        ),
    )
    .unwrap();
    m.compiled.restore(snapshot, policy()).unwrap()
}

#[test]
fn ordinary_remap_cancellation_and_all_due_families_keep_real_support() {
    let c = fixture(1);
    let mut m = seeded(&c);
    drop(m.apply(advance(&m, 5)).unwrap());
    let canceled: Vec<_> = [6, 7]
        .into_iter()
        .map(|at| {
            m.store.pending_events[&Time::from_ticks(at)][0]
                .identity()
                .5
        })
        .collect();
    let tx = Transaction::advance(
        Time::from_ticks(5),
        m.revision(),
        c.input_delta()
            .set(SET, LogicLevel::Low)
            .unwrap()
            .set(ENABLE, LogicLevel::Low)
            .unwrap()
            .finish()
            .unwrap(),
    );
    causal_work::reset();
    drop(m.apply(tx).unwrap());
    let work = causal_work::read();
    no_obsolete(work);
    assert_eq!(m.store.inertial_cancellation_causes.len(), 1);
    assert_eq!(m.store.periodic_cancellation_causes.len(), 1);
    for (cause, prior) in m
        .store
        .inertial_cancellation_causes
        .values()
        .chain(m.store.periodic_cancellation_causes.values())
        .zip(canceled)
    {
        let CauseInspection::Derived { supporters, .. } = m
            .store
            .provenance
            .as_ref()
            .unwrap()
            .inspect(*cause)
            .unwrap()
        else {
            panic!("cancellation derivation")
        };
        assert!(supporters.contains(&prior));
    }
    let tx = Transaction::advance(
        Time::from_ticks(5),
        m.revision(),
        c.input_delta()
            .set(SET, LogicLevel::High)
            .unwrap()
            .set(ENABLE, LogicLevel::High)
            .unwrap()
            .finish()
            .unwrap(),
    );
    causal_work::reset();
    drop(m.apply(tx).unwrap());
    no_obsolete(causal_work::read());
    assert!(
        m.store.pending_events[&Time::from_ticks(11)]
            .iter()
            .any(|e| matches!(e, PendingEvent::Inertial(_)))
    );
    assert!(
        m.store.pending_events[&Time::from_ticks(7)]
            .iter()
            .any(|e| matches!(e, PendingEvent::Periodic(_)))
    );
    let before = super::tests::observe(&m);
    causal_work::reset();
    let forecast = m.forecast(advance(&m, 12)).unwrap();
    no_obsolete(causal_work::read());
    assert_eq!(super::tests::observe(&m), before);
    causal_work::reset();
    let result = m.apply(advance(&m, 12)).unwrap();
    let work = causal_work::read();
    no_obsolete(work);
    assert_eq!(
        result
            .processed_reactions()
            .iter()
            .map(|s| s.time().ticks())
            .collect::<Vec<_>>(),
        [7, 11, 12]
    );
    assert_eq!(forecast.state().snapshot(), m.snapshot());
    assert!(work.predecessors_owned > 0 && work.successor_cause_entries_copied > 0);
    assert_eq!(work.logical_growth, work.nodes_created as u64);
    for cause in result.output_events().iter().map(output_event_cause) {
        assert!(result.provenance().inspect(cause).is_ok());
    }
    crate::state_digest_reference::assert_machine(&m);
    eprintln!("ordinary remap cancellation/due work={work:?}");
}

pub(super) fn preserving_patch(m: &Machine<()>, at: u64) -> Transaction<()> {
    let p = m
        .prepare_patch(
            m.patch()
                .set_diagnostic_meta(
                    StructuralSubjectRef::Module(ModuleInstanceKey::from_u128(101)),
                    DiagnosticMeta {
                        name: Some(format!("qualified-preserve-{:?}", m.revision())),
                        ..DiagnosticMeta::default()
                    },
                )
                .unwrap()
                .finish(),
        )
        .require_artifact()
        .unwrap();
    Transaction::advance(
        Time::from_ticks(at),
        m.revision(),
        p.input_delta().finish().unwrap(),
    )
    .with_patch(p, ReconfigurationPolicy::RejectStateLoss)
    .unwrap()
}

#[test]
fn ordinary_remap_restore_and_qualified_source_translations_remain_real() {
    let c = fixture(1);
    let mut direct = seeded(&c);
    let old_scope = direct.store.provenance.as_ref().unwrap().scope;
    causal_work::reset();
    let mut restored = round_trip(&direct);
    let restore_work = causal_work::read();
    assert!(restore_work.different_lineage_remaps > 0);
    assert_eq!(restore_work.freeze.new_nodes, restore_work.nodes_created);
    assert!(restore_work.freeze.new_nodes > 0);
    assert_eq!(restore_work.freeze.scratch_slots, 0);
    assert_eq!(restore_work.obsolete_remaps, ObsoleteRemaps::default());
    assert!(!crate::causal_store::same_lineage(
        old_scope,
        restored.store.provenance.as_ref().unwrap().scope
    ));
    assert_eq!(direct.snapshot(), restored.snapshot());
    for m in [&mut direct, &mut restored] {
        let tx = preserving_patch(m, 5); // Source deadlines 2,3,4 before new-lineage target.
        causal_work::reset();
        let result = m.apply(tx).unwrap();
        let work = causal_work::read();
        no_obsolete(work);
        assert!(work.source_translation_lookups > 0);
        assert_eq!(work.old_record_rewrites, 0); // First patch wraps ordinary source facts.
        assert!(result.migration().is_some());
        check_jump(&result);
        assert!(m.store.provenance.as_ref().unwrap().records.iter().any(
            |r| matches!(r,ProvenanceRecord::Checkpoint {fact,..} if fact.windows(b"module_node".len()).any(|part| part==b"module_node"))
        ));
        assert!(!crate::causal_store::same_lineage(
            old_scope,
            m.store.provenance.as_ref().unwrap().scope
        ));
        eprintln!("ordinary remap preserved qualified patch work={work:?}");
        // A second patch translates existing checkpoint predecessor handles.
        let tx = preserving_patch(m, 5);
        causal_work::reset();
        drop(m.apply(tx).unwrap());
        let work = causal_work::read();
        no_obsolete(work);
        assert!(work.old_record_rewrites > 0 && work.source_translation_lookups > 0);
        eprintln!("ordinary remap repeated checkpoint patch work={work:?}");
    }
    assert_eq!(direct.snapshot(), restored.snapshot());
    restored = round_trip(&restored);
    for at in [8, 15, 101] {
        let before = super::tests::observe(&direct);
        causal_work::reset();
        let forecast = direct.forecast(advance(&direct, at)).unwrap();
        no_obsolete(causal_work::read());
        assert_eq!(super::tests::observe(&direct), before);
        for m in [&mut direct, &mut restored] {
            causal_work::reset();
            drop(m.apply(advance(m, at)).unwrap());
            no_obsolete(causal_work::read());
        }
        assert_eq!(direct.snapshot(), restored.snapshot());
        assert_eq!(forecast.state().snapshot(), direct.snapshot());
        crate::state_digest_reference::assert_machine(&direct);
    }
    eprintln!("ordinary remap restoration work={restore_work:?}");
}

#[test]
fn ordinary_remap_held_causes_outlive_machine_without_changing_work() {
    let c = fixture(1);
    let run = |hold: bool| {
        let mut m = seeded(&c);
        let result = m.apply(advance(&m, 3)).unwrap();
        let node = m.inspect_node(LATCH).unwrap();
        let episode = m
            .active_diagnostic_episodes()
            .unwrap()
            .into_iter()
            .next()
            .unwrap()
            .clone();
        let module = m.inspect_module(ModuleInstanceKey::from_u128(102)).unwrap();
        let forecast = m.forecast(advance(&m, 4)).unwrap();
        let causes: Vec<_> = result
            .output_events()
            .iter()
            .map(output_event_cause)
            .chain(
                result
                    .diagnostic_episode_changes()
                    .iter()
                    .map(DiagnosticEpisodeChange::cause),
            )
            .collect();
        assert!(!causes.is_empty());
        let original = crate::state_digest_reference::canonical_view(&c, result.provenance());
        let owned = hold.then_some((result, node, episode, module, forecast));
        for at in [5, 8, 15] {
            drop(m.apply(advance(&m, at)).unwrap());
        }
        causal_work::reset();
        drop(m.apply(advance(&m, 20)).unwrap());
        let work = causal_work::read();
        no_obsolete(work);
        let snapshot = m.snapshot().artifact_bytes().to_vec();
        let manifest = crate::causal_roots::machine(&m)
            .into_iter()
            .map(|cause| {
                crate::state_digest::cause_content(m.store.provenance.as_ref().unwrap(), cause).0
            })
            .collect::<BTreeSet<_>>();
        let live = causal_work::live_nodes();
        drop(m);
        if let Some((result, node, episode, module, forecast)) = &owned {
            assert_eq!(
                crate::state_digest_reference::canonical_view(&c, result.provenance()).records,
                original.records
            );
            for cause in &causes {
                assert!(result.provenance().inspect(*cause).is_ok());
            }
            assert!(node.provenance().inspect(node.last_reaction_cause).is_ok());
            assert!(episode.provenance().inspect(episode.cause()).is_ok());
            let history = module.stateful_standard().unwrap();
            for cause in [history.latest_reset_cause, history.latest_capture_cause]
                .into_iter()
                .flatten()
            {
                assert!(history.provenance.inspect(cause).is_ok());
            }
            for cause in forecast
                .result()
                .diagnostic_episode_changes()
                .iter()
                .map(DiagnosticEpisodeChange::cause)
            {
                assert!(forecast.result().provenance().inspect(cause).is_ok());
            }
            assert_eq!(forecast.state().now(), Some(Time::from_ticks(4)));
            assert!(causal_work::live_nodes() > 0);
        }
        drop(owned);
        assert_eq!(causal_work::live_nodes(), 0);
        assert_eq!(causal_work::live_content_nodes(), 0);
        (snapshot, manifest, work, live)
    };
    let dropped = run(false);
    let held = run(true);
    assert_eq!(dropped.0, held.0);
    assert_eq!(dropped.1, held.1);
    let mut dropped_work = dropped.2;
    let mut held_work = held.2;
    // Releasing externally held physical nodes is not semantic machine work.
    dropped_work.nodes_released = 0;
    held_work.nodes_released = 0;
    assert_eq!(dropped_work, held_work);
    assert!(held.3 > dropped.3);
}

#[test]
fn ordinary_remap_preparation_faults_preserve_complete_warm_predecessor() {
    let c = fixture(1);
    for patch in [false, true] {
        for fault in [
            causal_work::Fault::StateExtraction,
            causal_work::Fault::Provenance,
            causal_work::Fault::DigestPreparation,
            causal_work::Fault::Result,
            causal_work::Fault::Projection,
        ] {
            let mut m = seeded(&c);
            let held = m.inspect_module(ModuleInstanceKey::from_u128(102)).unwrap();
            let cause = m.store.pending_events[&Time::from_ticks(100)][0]
                .identity()
                .5;
            let view = m.store.provenance.as_ref().unwrap().clone();
            let before = super::tests::observe(&m);
            let live = causal_work::live_nodes();
            let live_content = causal_work::live_content_nodes();
            let tx = if patch {
                preserving_patch(&m, 5)
            } else {
                advance(&m, 5)
            };
            causal_work::reset();
            causal_work::inject(fault);
            assert!(m.apply(tx).is_err());
            let work = causal_work::read();
            no_obsolete(work);
            // Extraction rejects before deadlines; provenance rejects during the
            // first deadline. Publication faults reach the completed patch, too.
            if fault == causal_work::Fault::StateExtraction {
                assert_eq!(work.nodes_created, 0);
            } else {
                assert!(work.nodes_created > 0);
            }
            if matches!(
                fault,
                causal_work::Fault::DigestPreparation
                    | causal_work::Fault::Result
                    | causal_work::Fault::Projection
            ) {
                assert!(work.result_selection.root_visits > 0);
                assert!(work.result_selection.records_selected > 0);
            } else {
                assert_eq!(
                    work.result_selection,
                    causal_work::ResultSelectionWork::default()
                );
            }
            if patch
                && matches!(
                    fault,
                    causal_work::Fault::DigestPreparation
                        | causal_work::Fault::Result
                        | causal_work::Fault::Projection
                )
            {
                assert!(work.source_translation_lookups > 0);
            }
            assert_eq!(super::tests::observe(&m), before);
            assert_eq!(causal_work::live_nodes(), live);
            assert_eq!(causal_work::live_content_nodes(), live_content);
            assert!(view.inspect(cause).is_ok());
            let history = held.stateful_standard().unwrap();
            assert!(
                history
                    .provenance
                    .inspect(history.latest_capture_cause.unwrap())
                    .is_ok()
            );
        }
    }
}
