//! Finite real-path evidence for ms-iri; all setup and oracles are outside intervals.
use super::*;
use crate::key::{ExternalInputKey, NetworkKey, NodeKey};
use crate::metadata::DiagnosticMeta;
use crate::signal::{Level, Pulse, PulseCount};
use crate::time::{NonZeroSpan, Time};
use crate::{
    EdgeConfig, EdgeInitialization, Explain, FirstEmissionPolicy, LevelSetResetConfig,
    NetworkBuilder, OutputEvent, PeriodicConfig, ReenablePhasePolicy, RuntimePolicy, TimeDomainId,
    ToggleConfig, Transaction, TransactionResult,
};

const INPUT: ExternalInputKey<Level> = ExternalInputKey::from_u128(1);
const VISIBLE: ExternalInputKey<Pulse> = ExternalInputKey::from_u128(2);
const SET: ExternalInputKey<Level> = ExternalInputKey::from_u128(3);
const RESET: ExternalInputKey<Level> = ExternalInputKey::from_u128(4);
const OWNER: NodeKey = NodeKey::from_u128(100);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Family {
    QuietEdge,
    GenuineToggle,
}

fn fixture(family: Family, width: usize, episode: bool) -> CompiledNetwork<()> {
    fixture_with_inspection(family, width, episode, false)
}

fn fixture_with_inspection(
    family: Family,
    width: usize,
    episode: bool,
    inspection: bool,
) -> CompiledNetwork<()> {
    let mut b = NetworkBuilder::with_key(NetworkKey::from_u128(1), TimeDomainId::from_u128(2));
    let level = b.add_level_input(INPUT, DiagnosticMeta::default()).unwrap();
    let visible = b
        .add_pulse_input(VISIBLE, DiagnosticMeta::default())
        .unwrap();
    let pulses = (family == Family::GenuineToggle).then(|| {
        b.periodic(
            level,
            PeriodicConfig::new(
                NonZeroSpan::from_ticks(1).unwrap(),
                FirstEmissionPolicy::AfterFirstPeriod,
                ReenablePhasePolicy::PreservePhase,
            ),
        )
        .unwrap()
    });
    for owner in 0..width {
        let key = NodeKey::from_u128(100 + owner as u128);
        match pulses {
            Some(pulse) => {
                b.add_toggle(
                    key,
                    pulse,
                    ToggleConfig::new(LogicLevel::Low),
                    DiagnosticMeta::default(),
                )
                .unwrap();
            }
            None => {
                b.add_any_edge(
                    key,
                    level,
                    EdgeConfig::new(EdgeInitialization::Baseline),
                    DiagnosticMeta::default(),
                )
                .unwrap();
            }
        }
    }
    if inspection {
        b.add_any_edge(
            NodeKey::from_u128(9000),
            level,
            EdgeConfig::new(EdgeInitialization::Baseline),
            DiagnosticMeta::default(),
        )
        .unwrap();
    }
    if episode {
        let set = b.add_level_input(SET, DiagnosticMeta::default()).unwrap();
        let reset = b.add_level_input(RESET, DiagnosticMeta::default()).unwrap();
        b.level_set_reset_latch(
            set,
            reset,
            LevelSetResetConfig::new(LogicLevel::Low, ConflictPolicy::RetainAndDiagnose),
        )
        .unwrap();
    }
    b.pulse_output("independent", visible).unwrap();
    b.finish()
        .require_artifact()
        .unwrap()
        .compile()
        .require_artifact()
        .unwrap()
}

fn policy() -> RuntimePolicy {
    RuntimePolicy::builder()
        .max_internal_reactions(2000)
        .max_evaluated_operations(100_000)
        .max_pending_events(100)
        .max_events_created_per_transaction(2000)
        .max_required_provenance_growth(50_000)
        .build()
        .unwrap()
}

fn initialized(c: &CompiledNetwork<()>, family: Family, episode: Option<bool>) -> Machine<()> {
    let mut m = c.spawn(policy());
    let mut input = c
        .input_snapshot()
        .set(
            INPUT,
            if family == Family::QuietEdge {
                LogicLevel::Low
            } else {
                LogicLevel::High
            },
        )
        .unwrap();
    if let Some(active) = episode {
        let value = if active {
            LogicLevel::High
        } else {
            LogicLevel::Low
        };
        input = input.set(SET, value).unwrap().set(RESET, value).unwrap();
    }
    m.apply(Transaction::initialize(
        Time::from_ticks(0),
        m.revision(),
        input.finish().unwrap(),
    ))
    .unwrap();
    m
}

fn transaction(m: &Machine<()>, at: u64, visible: bool) -> Transaction<()> {
    let mut input = m.compiled().input_delta();
    if visible {
        input = input.pulse(VISIBLE, PulseCount::ONE).unwrap();
    }
    Transaction::advance(Time::from_ticks(at), m.revision(), input.finish().unwrap())
}

fn build_history(m: &mut Machine<()>, family: Family, from: u64, to: u64) {
    if from > to {
        return;
    }
    if family == Family::QuietEdge {
        for at in from..=to {
            m.apply(transaction(m, at, false)).unwrap();
        }
    } else {
        let result = m.apply(transaction(m, to, false)).unwrap();
        assert_eq!(result.processed_reactions().len(), (to - from + 1) as usize);
    }
}

// Independent handle-set walk, rather than the projection's ordinal bitmap.
fn walk(view: &ProvenanceView<()>, roots: &[CauseRef]) -> (BTreeSet<CauseRef>, usize) {
    let mut seen = BTreeSet::new();
    let mut work = roots.to_vec();
    let mut predecessors = 0;
    while let Some(cause) = work.pop() {
        assert!(view.inspect(cause).is_ok());
        if seen.insert(cause) {
            let position = view.records().position(cause).unwrap();
            let parents = view.records()[position].predecessor_causes();
            predecessors += parents.len();
            work.extend(parents);
        }
    }
    (seen, predecessors)
}

#[derive(Debug, Default)]
struct Inventory {
    indexed: usize,
    roots: usize,
    visits: usize,
    predecessors: usize,
    max_fan_in: usize,
    indexed_content: BTreeMap<[u8; 32], Vec<u8>>,
    emitted: BTreeMap<[u8; 32], Vec<u8>>,
}

impl Inventory {
    fn add(&mut self, view: &ProvenanceView<()>, roots: &[CauseRef]) {
        self.indexed += view.len();
        self.roots += roots.len();
        let (reached, predecessors) = walk(view, roots);
        self.visits += reached.len();
        self.predecessors += predecessors;
        for (position, record) in view.records().iter().enumerate() {
            self.max_fan_in = self.max_fan_in.max(record.predecessor_causes().len());
            let content = view.records().canonical(position).get().unwrap();
            if let Some(old) = self
                .indexed_content
                .insert(content.digest, content.payload.as_ref().clone())
            {
                assert_eq!(old, *content.payload);
            }
            if reached.contains(&view.records().cause(position)) {
                self.emitted
                    .insert(content.digest, content.payload.as_ref().clone());
            }
        }
    }
    fn machine(m: &Machine<()>) -> Self {
        let mut result = Self::default();
        result.add(m.store.provenance.as_ref().unwrap(), &machine_roots(m));
        for episode in m.store.active_episodes.values() {
            result.add(episode.provenance(), &[episode.cause()]);
        }
        result
    }
    fn payload_bytes(&self) -> usize {
        self.emitted.values().map(Vec::len).sum()
    }
    fn check(&self, w: crate::projection_work::Work) {
        assert_eq!(w.records_indexed, self.indexed);
        assert_eq!(w.closure_roots, self.roots);
        assert_eq!(w.closure_visits, self.visits);
        assert_eq!(w.predecessor_entries, self.predecessors);
        assert_eq!(w.closure_pops, self.roots + self.predecessors);
        assert_eq!(w.content_inserts, self.indexed);
        assert_eq!(w.content_new, self.indexed_content.len());
        assert_eq!(w.content_equal, self.indexed - self.indexed_content.len());
        assert_eq!(w.records_emitted, self.emitted.len());
        assert_eq!(w.bytes_emitted, self.payload_bytes());
    }
}

fn reset() {
    crate::projection_work::reset();
    crate::causal_work::reset();
    crate::execution_work::reset();
}

fn inputs(m: &Machine<()>) -> (Vec<u8>, Vec<u8>) {
    let expected = (
        crate::state_digest_reference::execution_digest_input(m, 4, 4),
        crate::state_digest_reference::observable_digest_input(m, 4, 4),
    );
    assert_eq!(execution_digest_input(m, 4, 4), expected.0);
    assert_eq!(observable_digest_input(m, 4, 4), expected.1);
    expected
}

fn raw(m: &Machine<()>, label: &str) -> crate::projection_work::Work {
    let inventory = Inventory::machine(m);
    let (e, o) = inputs(m);
    let roots = crate::causal_roots::machine(m);
    eprintln!(
        "MS_IRI {label} metadata ops={} roots={} root_entries={} machine_view={} episode_views={} pending={} fan_in={} unique_indexed={} unique_emitted={} payload_bytes={} E_bytes={} O_bytes={}",
        m.compiled.operation_count(),
        roots.len(),
        inventory.roots,
        m.store.provenance.as_ref().unwrap().len(),
        m.store
            .active_episodes
            .values()
            .map(|e| e.provenance().len())
            .sum::<usize>(),
        m.store.pending_events.values().map(Vec::len).sum::<usize>(),
        inventory.max_fan_in,
        inventory.indexed_content.len(),
        inventory.emitted.len(),
        inventory.payload_bytes(),
        e.len(),
        o.len()
    );
    reset();
    let actual = execution_state_digest(m);
    let ew = crate::projection_work::read();
    assert_eq!(actual, m.execution_state_digest());
    assert_eq!(actual.as_bytes(), *blake3::hash(&e).as_bytes());
    assert_eq!(
        (
            ew.execution_builds,
            ew.execution_hashes,
            ew.execution_hash_bytes
        ),
        (1, 1, e.len())
    );
    assert_eq!(
        ew.records_indexed + ew.closure_visits + ew.content_inserts + ew.bytes_emitted,
        0
    );
    assert!(ew.direct_resolutions > 0 && ew.execution_hash_bytes > 0);
    assert_eq!(
        ew,
        crate::projection_work::Work {
            execution_builds: 1,
            execution_hashes: 1,
            execution_hash_bytes: e.len(),
            direct_resolutions: ew.direct_resolutions,
            ..Default::default()
        }
    );
    assert_eq!(
        crate::causal_work::read(),
        crate::causal_work::Work::default()
    );
    eprintln!("MS_IRI {label} raw_E {ew:?}");
    reset();
    let actual = observable_state_digest(m);
    let ow = crate::projection_work::read();
    assert_eq!(actual, m.observable_state_digest());
    assert_eq!(actual.as_bytes(), *blake3::hash(&o).as_bytes());
    assert_eq!(
        (
            ow.observable_builds,
            ow.observable_hashes,
            ow.observable_hash_bytes
        ),
        (1, 1, o.len())
    );
    assert!(
        ow.closure_roots > 0
            && ow.closure_pops > 0
            && ow.predecessor_entries > 0
            && ow.content_new > 0
    );
    assert!(ow.bytes_emitted > 0 && ow.observable_hash_bytes > ow.bytes_emitted);
    inventory.check(ow);
    eprintln!("MS_IRI {label} raw_O {ow:?}");
    let cached = (m.execution_state_digest(), m.observable_state_digest());
    reset();
    for _ in 0..10 {
        assert_eq!(m.execution_state_digest(), cached.0);
        assert_eq!(m.observable_state_digest(), cached.1);
    }
    assert_eq!(
        crate::projection_work::read(),
        crate::projection_work::Work::default()
    );
    assert_eq!(
        crate::causal_work::read(),
        crate::causal_work::Work::default()
    );
    ow
}

fn result_manifest(result: &TransactionResult<()>) {
    let roots: Vec<_> = result
        .output_events()
        .iter()
        .map(|event| match event {
            OutputEvent::LevelEstablished { cause, .. }
            | OutputEvent::LevelChanged { cause, .. }
            | OutputEvent::Pulsed { cause, .. } => *cause,
        })
        .chain(
            result
                .diagnostic_episode_changes()
                .iter()
                .map(crate::DiagnosticEpisodeChange::cause),
        )
        .collect();
    let (expected, _) = walk(result.provenance(), &roots);
    let actual = (0..result.provenance().len())
        .map(|p| result.provenance().records().cause(p))
        .collect();
    assert_eq!(expected, actual);
}

fn warm(m: &Machine<()>, visible: bool, label: &str) -> crate::projection_work::Work {
    // This base copy is setup; ordinary apply's own successor copy stays in the interval.
    let mut candidate = m.duplicate_for_staging();
    let tx = transaction(&candidate, m.now().unwrap().ticks() + 1, visible);
    reset();
    let result = candidate.apply(tx).unwrap();
    let p = crate::projection_work::read();
    let c = crate::causal_work::read();
    let x = crate::execution_work::read();
    let reactions = result.processed_reactions().len();
    eprintln!(
        "MS_IRI {label} warm visible={visible} reactions={reactions} op_charge={} outputs={} result_records={} episodes={} projection={p:?} causal={c:?} execution={x:?}",
        reactions * candidate.compiled.operation_count(),
        result.output_events().len(),
        result.provenance().len(),
        result.diagnostic_episode_changes().len()
    );
    // apply already checks full uncached inputs/digests before publication;
    // that oracle is excluded from accounting. Only fresh lengths are needed here.
    let (e, o) = (
        execution_digest_input(&candidate, 4, 4),
        observable_digest_input(&candidate, 4, 4),
    );
    assert_eq!(
        (
            p.execution_builds,
            p.observable_builds,
            p.execution_hashes,
            p.observable_hashes
        ),
        (1, 1, 1, 1)
    );
    assert_eq!(p.execution_hash_bytes, e.len());
    assert_eq!(p.observable_hash_bytes, o.len());
    Inventory::machine(&candidate).check(p);
    assert!(c.logical_growth > 0 && c.freeze.new_nodes > 0 && c.catalogue_handles_copied > 0);
    assert_eq!(x.staged_stores_cloned, 1);
    // Ordinary preparation moves working collections out of its private successor.
    // These counters charge only the extra copies in the clone reference path.
    assert_eq!(x.working_collections_cloned, 0);
    assert_eq!(x.working_items_cloned, 0);
    assert_eq!(c.closure_records_visited, p.closure_visits);
    assert_eq!(c.closure_bytes_emitted, p.bytes_emitted);
    assert_eq!(result.output_events().len(), usize::from(visible));
    assert!(result.diagnostic_episode_changes().is_empty());
    result_manifest(&result);
    assert_eq!(
        c.content_selection.calls,
        c.content_selection.empty
            + c.content_selection.full
            + c.content_selection.dense
            + c.content_selection.sparse
    );
    assert!(c.content_selection.visits <= c.membership_content_visits);
    assert!(c.content_selection.nodes_created <= c.membership_content_nodes_created);
    if visible {
        assert_eq!(result.provenance().len(), 3);
        assert!(c.result_selection.records_selected > 0 && c.result_selection.root_visits > 0);
        assert_eq!(
            c.result_selection,
            crate::causal_work::ResultSelectionWork {
                root_visits: 1,
                handles_visited: 3,
                records_selected: 3,
                content_visits: 6,
                content_nodes_created: 6,
            }
        );
    } else {
        assert_eq!(
            c.result_selection,
            crate::causal_work::ResultSelectionWork::default()
        );
    }
    p
}

fn artifacts(m: &Machine<()>, label: &str) -> crate::Explanation<()> {
    reset();
    let snapshot = m.snapshot();
    let p = crate::projection_work::read();
    let c = crate::causal_work::read();
    eprintln!(
        "MS_IRI {label} snapshot bytes={} projection={p:?} causal={c:?}",
        snapshot.artifact_bytes().len()
    );
    assert_eq!(
        p.execution_hash_bytes + p.observable_hash_bytes + p.execution_builds + p.observable_builds,
        0
    );
    assert!(p.records_emitted > 0);
    assert_eq!(
        snapshot.artifact_bytes(),
        crate::persistence::encode_snapshot(
            &crate::PersistenceContext::new(TimeDomainId::from_u128(2)),
            &snapshot
        )
        .unwrap()
        .as_bytes()
    );
    reset();
    let explanation = m.explain(Explain::CurrentNode(OWNER)).unwrap();
    let p = crate::projection_work::read();
    let c = crate::causal_work::read();
    let graph = &explanation.causal;
    let (expected, _) = walk(graph.provenance(), &graph.roots);
    assert_eq!(
        graph.edges.iter().map(|e| e.cause).collect::<BTreeSet<_>>(),
        expected
    );
    eprintln!(
        "MS_IRI {label} explanation roots={} nodes={} edges={} owned={} projection={p:?} causal={c:?}",
        graph.roots.len(),
        graph.edges.len(),
        graph
            .edges
            .iter()
            .map(|e| e.supporters.len())
            .sum::<usize>(),
        graph.provenance().len()
    );
    assert_eq!(p, crate::projection_work::Work::default());
    assert!(c.owned_closure_records_visited > 0);
    let restored = m.compiled().restore(snapshot, policy()).unwrap();
    assert_eq!(inputs(&restored), inputs(m));
    explanation
}

fn content_selection_reduction(full: bool) {
    let c = fixture(Family::GenuineToggle, 1, false);
    let mut m = initialized(&c, Family::GenuineToggle, None);
    build_history(&mut m, Family::GenuineToggle, 1, 10);
    let view = m.store.provenance.as_ref().unwrap();
    let count = view.len() - usize::from(!full);
    // Every preceding source position is selected; the final record has no later consumer.
    let roots: Vec<_> = (0..count).map(|p| view.records().cause(p)).collect();
    let (expected, _) = walk(view, &roots);
    assert_eq!(expected.len(), count);
    let digests: BTreeSet<_> = (0..count)
        .map(|p| view.records().canonical(p).get().unwrap().digest)
        .collect();
    crate::causal_work::reset();
    let selected = view.owned_roots(&roots);
    let work = crate::causal_work::read();
    eprintln!(
        "MS_QJR regression full={full} source={} selected={count} unique={} visits={} nodes={}",
        view.len(),
        digests.len(),
        work.membership_content_visits,
        work.membership_content_nodes_created
    );
    assert_eq!(selected.len(), count);
    assert_eq!(
        selected.records().content_for_test(),
        (0..count)
            .map(|p| {
                let canonical = view.records().canonical(p).get().unwrap();
                (canonical.digest, Arc::clone(&canonical.payload))
            })
            .collect()
    );
    assert_eq!(
        (0..selected.len())
            .map(|p| selected.records().cause(p))
            .collect::<BTreeSet<_>>(),
        expected
    );
    if full {
        assert_eq!(work.membership_content_nodes_created, 0);
    } else {
        assert!(
            work.membership_content_nodes_created < digests.len(),
            "{work:?}"
        );
    }
}

#[test]
fn content_selection_full_genuine_closure_does_not_rebuild_content() {
    content_selection_reduction(true);
}

#[test]
fn content_selection_nearly_full_genuine_closure_reuses_content() {
    content_selection_reduction(false);
}

#[test]
fn content_selection_direct_genuine_and_quiet_matrix_preserves_complete_content() {
    let live = crate::causal_work::live_nodes();
    let content_live = crate::causal_work::live_content_nodes();
    for (family, width, histories) in [
        (Family::GenuineToggle, 1, vec![1, 10, 1000]),
        (Family::GenuineToggle, 4, vec![10, 1000]),
        (Family::QuietEdge, 1, vec![10, 1000]),
    ] {
        let c = fixture(family, width, false);
        for history in histories {
            let mut m = initialized(&c, family, None);
            build_history(&mut m, family, 1, history);
            let original = inputs(&m);
            let snapshot = m.snapshot();
            let view = m.store.provenance.as_ref().unwrap();
            let unique = view.records().content_for_test().len();
            for full in [true, false] {
                let count = view.len() - usize::from(!full);
                let roots: Vec<_> = (0..count).map(|p| view.records().cause(p)).collect();
                let (expected, _) = walk(view, &roots);
                let bytes: BTreeMap<_, _> = expected
                    .iter()
                    .map(|cause| {
                        let canonical = view
                            .records()
                            .canonical(view.records().position(*cause).unwrap())
                            .get()
                            .unwrap();
                        (canonical.digest, Arc::clone(&canonical.payload))
                    })
                    .collect();
                crate::causal_work::reset();
                let selected = view.owned_roots(&roots);
                let w = crate::causal_work::read();
                let work = w.content_selection;
                eprintln!(
                    "MS_QJR direct family={family:?} width={width} history={history} full={full} source={} selected={} unique={} work={work:?}",
                    view.len(),
                    selected.len(),
                    bytes.len()
                );
                assert_eq!(selected.len(), count);
                assert_eq!(
                    (0..selected.len())
                        .map(|p| selected.records().cause(p))
                        .collect::<BTreeSet<_>>(),
                    expected
                );
                assert_eq!(selected.records().content_for_test(), bytes);
                assert_eq!(work.insertions + work.leaves_created, 0);
                if full {
                    assert_eq!(work.roots_shared, 1);
                    assert_eq!(work.visits + work.nodes_created + work.keys_inserted, 0);
                } else {
                    assert_eq!(work.dense, 1);
                    assert_eq!(work.keys_inserted, count);
                    assert_eq!(work.visits, 2 * unique - 1);
                    assert!(work.visits < 4 * count);
                    assert!(work.nodes_created <= (unique - 1).min(256 * (unique - bytes.len())));
                }
                crate::state_digest_reference::assert_view(&c, &selected);
                if family == Family::GenuineToggle {
                    let times: BTreeSet<_> = selected
                        .records()
                        .iter()
                        .filter_map(|r| match r {
                            ProvenanceRecord::ReadyTransaction { at, .. } => {
                                Some(at.time().ticks())
                            }
                            _ => None,
                        })
                        .collect();
                    assert_eq!(times, (1..=history).collect());
                }
            }
            assert_eq!(inputs(&m), original);
            assert_eq!(m.snapshot(), snapshot);
        }
    }
    assert_eq!(crate::causal_work::live_nodes(), live);
    assert_eq!(crate::causal_work::live_content_nodes(), content_live);
}

#[test]
fn content_selection_targeted_inspection_avoids_unrelated_genuine_history() {
    let c = fixture_with_inspection(Family::GenuineToggle, 1, false, true);
    let mut sizes = Vec::new();
    for history in [1, 10, 1000] {
        let mut m = initialized(&c, Family::GenuineToggle, None);
        build_history(&mut m, Family::GenuineToggle, 1, history);
        let before = inputs(&m);
        let source = m.store.provenance.as_ref().unwrap().len();
        reset();
        let inspection = m.inspect_node(NodeKey::from_u128(9000)).unwrap();
        let w = crate::causal_work::read();
        let projection = crate::projection_work::read();
        let selected = inspection.provenance().len();
        let work = w.content_selection;
        eprintln!(
            "MS_QJR small history={history} source={source} selected={selected} work={work:?}"
        );
        assert_eq!(work.sparse, 1);
        assert_eq!(work.empty + work.full + work.dense, 0);
        assert_eq!(work.keys_inserted, 0);
        assert_eq!(work.insertions, selected);
        // A rebuilt selected-only compressed trie has at most k leaves and k-1 branches.
        assert!(work.visits <= selected * (2 * selected + 1));
        assert!(work.nodes_created <= selected * (selected + 1));
        assert_eq!(projection, crate::projection_work::Work::default());
        let contents: BTreeMap<_, _> = inspection
            .provenance()
            .records()
            .iter()
            .enumerate()
            .map(|(p, _)| {
                let record = inspection
                    .provenance()
                    .records()
                    .canonical(p)
                    .get()
                    .unwrap();
                (record.digest, Arc::clone(&record.payload))
            })
            .collect();
        assert_eq!(
            inspection.provenance().records().content_for_test(),
            contents
        );
        assert_eq!(inputs(&m), before);
        sizes.push(selected);
        drop(m);
        for p in 0..inspection.provenance().len() {
            assert!(
                inspection
                    .provenance()
                    .inspect(inspection.provenance().records().cause(p))
                    .is_ok()
            );
        }
        drop(inspection);
        drop(contents);
        assert_eq!(crate::causal_work::live_nodes(), 0);
        assert_eq!(crate::causal_work::live_content_nodes(), 0);
    }
    assert!(sizes.iter().all(|size| *size == sizes[0]));
}

#[test]
fn unchanged_digest_cost_primary_matrix() {
    for family in [Family::QuietEdge, Family::GenuineToggle] {
        for width in [1, 4] {
            let c = fixture(family, width, false);
            let mut previous = None;
            for history in [1, 10, 1000] {
                let mut m = initialized(&c, family, None);
                build_history(&mut m, family, 1, history);
                let label = format!("family={family:?} width={width} history={history}");
                let p = raw(&m, &label);
                if family == Family::GenuineToggle {
                    let view = m.store.provenance.as_ref().unwrap();
                    let (ancestry, _) = walk(view, &[m.store.toggle_inversion_causes[&OWNER]]);
                    let times: BTreeSet<_> = ancestry
                        .into_iter()
                        .filter_map(|cause| {
                            match &view.records()[view.records().position(cause).unwrap()] {
                                ProvenanceRecord::ReadyTransaction { at, .. } => {
                                    Some(at.time().ticks())
                                }
                                _ => None,
                            }
                        })
                        .collect();
                    assert_eq!(times, (1..=history).collect());
                }
                let empty = warm(&m, false, &label);
                let visible = warm(&m, true, &label);
                assert_eq!(empty.execution_builds, visible.execution_builds);
                if let Some((records, bytes)) = previous {
                    if family == Family::QuietEdge {
                        assert_eq!(p.records_emitted, records);
                        // Only time/stamp scalar encoding widens; there is no hidden chain.
                        assert!(p.bytes_emitted <= bytes + 32 * width);
                    } else {
                        assert!(p.records_emitted > records && p.bytes_emitted > bytes);
                    }
                }
                previous = Some((p.records_emitted, p.bytes_emitted));
                let explanation = (width == 1 && history != 1).then(|| artifacts(&m, &label));
                drop(m);
                if let Some(explanation) = explanation {
                    let graph = explanation.causal;
                    let (expected, _) = walk(graph.provenance(), &graph.roots);
                    assert_eq!(
                        graph.edges.iter().map(|e| e.cause).collect::<BTreeSet<_>>(),
                        expected
                    );
                }
            }
        }
    }
}

#[test]
fn unchanged_digest_cost_old_artifacts_are_independent() {
    for family in [Family::QuietEdge, Family::GenuineToggle] {
        let c = fixture(family, 1, false);
        for history in [10, 1000] {
            let mut previous = None;
            for held in [false, true] {
                let live = crate::causal_work::live_nodes();
                let mut m = initialized(&c, family, None);
                let result = m.apply(transaction(&m, 1, true)).unwrap();
                let inspection = m.inspect_node(OWNER).unwrap();
                let event = match result.output_events()[0] {
                    OutputEvent::Pulsed { cause, .. } => cause,
                    _ => panic!("independent event is a pulse"),
                };
                let position = result.provenance().records().position(event).unwrap();
                let weak = Arc::downgrade(
                    &result
                        .provenance()
                        .records()
                        .canonical(position)
                        .get()
                        .unwrap()
                        .payload,
                );
                let artifacts = held.then_some((result, inspection));
                build_history(&mut m, family, 2, history);
                assert_eq!(weak.upgrade().is_some(), held);
                let label = format!("family={family:?} width=1 history={history} held={held}");
                let p = raw(&m, &label);
                let w = warm(&m, true, &label);
                let manifest: Vec<_> = crate::causal_roots::machine(&m)
                    .into_iter()
                    .map(|cause| cause_content(m.store.provenance.as_ref().unwrap(), cause))
                    .collect();
                let values = (
                    inputs(&m),
                    manifest.into_iter().collect::<BTreeSet<_>>(),
                    p,
                    w,
                    m.snapshot().artifact_bytes().to_vec(),
                );
                if let Some(previous) = &previous {
                    assert_eq!(&values, previous);
                }
                previous = Some(values);
                let current_nodes = crate::causal_work::live_nodes() - live;
                drop(m);
                if let Some((result, inspection)) = &artifacts {
                    result_manifest(result);
                    assert!(result.provenance().explain_cause(event).is_ok());
                    for position in 0..inspection.provenance().len() {
                        assert!(
                            inspection
                                .provenance()
                                .inspect(inspection.provenance().records().cause(position))
                                .is_ok()
                        );
                    }
                }
                eprintln!(
                    "MS_IRI {label} ownership current_live={current_nodes} after_machine={} old_payload={}",
                    crate::causal_work::live_nodes() - live,
                    weak.upgrade().is_some()
                );
                drop(artifacts);
                assert!(weak.upgrade().is_none());
                assert_eq!(crate::causal_work::live_nodes(), live);
            }
        }
    }
}

#[test]
fn unchanged_digest_cost_episode_view_probe() {
    let c = fixture(Family::QuietEdge, 1, true);
    let mut measurements = Vec::new();
    for active in [false, true] {
        let mut m = initialized(&c, Family::QuietEdge, Some(active));
        build_history(&mut m, Family::QuietEdge, 1, 10);
        assert_eq!(m.store.active_episodes.len(), usize::from(active));
        let label = format!("family=QuietEdge width=1 history=10 episode={active}");
        let p = raw(&m, &label);
        warm(&m, false, &label);
        measurements.push(p);
    }
    assert!(measurements[1].content_equal > measurements[0].content_equal);
    assert!(measurements[1].records_indexed > measurements[0].records_indexed);
}
