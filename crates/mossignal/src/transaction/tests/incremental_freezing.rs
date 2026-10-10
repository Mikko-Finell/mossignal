//! Fixed new work over complete retained ancestry, independent of observer work.
use super::*;
use crate::causal_work;
use crate::key::{NetworkKey, NodeKey};
use crate::signal::LogicLevel;
use crate::time::Time;
use crate::{NetworkBuilder, TimeDomainId};

fn compiled() -> crate::CompiledNetwork<()> {
    let mut b = NetworkBuilder::with_key(NetworkKey::from_u128(1), TimeDomainId::from_u128(2));
    let s = b.constant(LogicLevel::High);
    b.level_output("value", s).unwrap();
    b.finish()
        .require_artifact()
        .unwrap()
        .compile()
        .require_artifact()
        .unwrap()
}

fn old_view(c: &crate::CompiledNetwork<()>, count: usize) -> (ProvenanceView<()>, CauseRef) {
    let scope = crate::causal_store::fresh_scope(None);
    let mut records = crate::causal_store::Records::new(scope);
    let mut tail = records.push(
        c,
        ProvenanceRecord::ReadyTransaction {
            at: ReactionStamp::from_parts(Time::from_ticks(0), 0),
            revision: NetworkRevision::from_value(0),
        },
    );
    for i in 1..count {
        tail = records.push(
            c,
            ProvenanceRecord::Checkpoint {
                fact: (i as u64).to_be_bytes().to_vec(),
                supporters: vec![tail],
            },
        );
    }
    let view = ProvenanceView {
        scope,
        records: Arc::new(records),
    };
    (view, tail)
}

fn no_old_freeze(w: causal_work::Work) {
    assert_eq!(w.freeze.old_records_indexed, 0, "{w:?}");
    assert_eq!(w.freeze.scratch_slots, 0, "{w:?}");
    assert_eq!(w.freeze.cached_payload_visits, 0, "{w:?}");
    assert_eq!(w.freeze.cached_payload_insertions, 0, "{w:?}");
    assert_eq!(w.freeze.old_source_visits, 0, "{w:?}");
}

#[test]
fn incremental_freezing_fixed_addition_does_not_walk_old_ancestry() {
    let c = compiled();
    let mut measurements = Vec::new();
    for history in [1, 10, 1000] {
        let (old, tail) = old_view(&c, history);
        let original = crate::state_digest::cause_content(&old, tail);
        let scope = crate::causal_store::fresh_scope(Some(old.scope));
        let mut records = old.records.fork(scope);
        causal_work::reset();
        let first = records.push(
            &c,
            ProvenanceRecord::Derived {
                subject: ProvenanceSubject::Node(NodeKey::from_u128(100)),
                supporters: vec![tail, tail],
            },
        );
        let duplicate = records.push(
            &c,
            ProvenanceRecord::Derived {
                subject: ProvenanceSubject::Node(NodeKey::from_u128(100)),
                supporters: vec![tail, tail],
            },
        );
        let last = records.push(
            &c,
            ProvenanceRecord::Checkpoint {
                fact: vec![9],
                supporters: vec![first, duplicate, tail],
            },
        );
        let view = ProvenanceView {
            scope,
            records: Arc::new(records),
        };
        let w = causal_work::read();
        assert_eq!(w.nodes_created, 3);
        assert_eq!(w.canonical_encodes, 3);
        assert_eq!(w.canonical_hashes, 3);
        assert_eq!(w.predecessors_owned, 7);
        assert_eq!(w.freeze.new_nodes, 3);
        assert_eq!(w.freeze.predecessor_reads, 7);
        assert_eq!(w.freeze.consistency_lookups, 3);
        assert!(w.freeze.consistency_path_visits <= 3 * 513);
        assert!(w.freeze.consistency_nodes_created <= 3 * 258);
        assert!(view.records.shared_node(&old.records, tail));
        assert_eq!(crate::state_digest::cause_content(&view, tail), original);
        assert_eq!(
            crate::state_digest::cause_content(&view, first),
            crate::state_digest::cause_content(&view, duplicate)
        );
        let owned = view.owned_roots(&[last]);
        assert_eq!(owned.len(), history + 3);
        crate::state_digest_reference::assert_view(&c, &view);
        eprintln!("incremental freeze history={history}: {w:?}");
        measurements.push(w);
    }
    for w in measurements {
        no_old_freeze(w);
    }
}

#[test]
fn incremental_freezing_genuine_combinational_ancestry_has_fixed_new_work() {
    for history in [1, 10, 1000] {
        let mut b = crate::NetworkBuilder::<()>::new(TimeDomainId::from_u128(2));
        let mut signal = b.constant(LogicLevel::High);
        for _ in 0..history {
            signal = b.all([signal, signal]).unwrap();
        }
        b.level_output("value", signal).unwrap();
        let c = b
            .finish()
            .require_artifact()
            .unwrap()
            .compile()
            .require_artifact()
            .unwrap();
        let mut m = c.spawn(super::cause_remapping_tests::policy());
        let r = m
            .apply(Transaction::initialize(
                Time::from_ticks(0),
                m.revision(),
                c.input_snapshot().finish().unwrap(),
            ))
            .unwrap();
        let view = r.provenance();
        let retained = view.len();
        let tail = *m.store.output_causes.values().next().unwrap();
        let before = crate::state_digest::cause_content(view, tail);
        let scope = crate::causal_store::fresh_scope(Some(view.scope));
        let mut records = view.records.fork(scope);
        causal_work::reset();
        let cause = records.push(
            &c,
            ProvenanceRecord::Derived {
                subject: ProvenanceSubject::Node(c.graph().nodes()[0].key()),
                supporters: vec![tail, tail],
            },
        );
        let work = causal_work::read();
        no_old_freeze(work);
        assert_eq!(work.nodes_created, 1);
        assert_eq!(work.canonical_encodes, 1);
        assert_eq!(work.canonical_hashes, 1);
        assert_eq!(work.predecessors_owned, 2);
        assert_eq!(work.freeze.new_nodes, 1);
        assert_eq!(work.freeze.predecessor_reads, 2);
        assert_eq!(work.freeze.consistency_lookups, 1);
        assert!(work.freeze.consistency_path_visits <= 513);
        let held = ProvenanceView {
            scope,
            records: Arc::new(records),
        };
        assert!(held.inspect(cause).is_ok());
        assert_eq!(crate::state_digest::cause_content(&held, tail), before);
        drop(r);
        drop(m);
        assert!(held.inspect(tail).is_ok());
        drop(held);
        assert_eq!(causal_work::live_nodes(), 0);
        assert_eq!(causal_work::live_content_nodes(), 0);
        eprintln!(
            "incremental freeze genuine ancestry={history}, retained={}: {work:?}",
            retained
        );
    }
}

#[test]
fn incremental_freezing_shared_readers_see_exclusively_completed_content() {
    let c = compiled();
    let (view, tail) = old_view(&c, 10);
    for position in 0..view.len() {
        assert!(view.records.canonical(position).get().is_some());
        assert!(view.records.source(position).is_some());
    }
    let expected = crate::state_digest::cause_content(&view, tail);
    std::thread::scope(|scope| {
        for _ in 0..4 {
            let view = &view;
            let expected = &expected;
            scope.spawn(move || {
                for _ in 0..100 {
                    assert_eq!(&crate::state_digest::cause_content(view, tail), expected);
                }
            });
        }
    });
    drop(view);
    assert_eq!(causal_work::live_nodes(), 0);
    assert_eq!(causal_work::live_content_nodes(), 0);
}

#[test]
fn incremental_freezing_real_shared_pulse_contributions_keep_multiplicity() {
    use crate::signal::{Pulse, PulseCount};
    let input = crate::key::ExternalInputKey::<Pulse>::from_u128(1);
    let mut b = crate::NetworkBuilder::<()>::new(TimeDomainId::from_u128(2));
    let p = b
        .add_pulse_input(input, crate::metadata::DiagnosticMeta::default())
        .unwrap();
    let merged = b.merge([p, p]).unwrap();
    b.pulse_output("merged", merged).unwrap();
    let c = b
        .finish()
        .require_artifact()
        .unwrap()
        .compile()
        .require_artifact()
        .unwrap();
    let mut m = c.spawn(super::cause_remapping_tests::policy());
    causal_work::reset();
    let r = m
        .apply(Transaction::initialize(
            Time::from_ticks(0),
            m.revision(),
            c.input_snapshot()
                .pulse(input, PulseCount::new(2))
                .unwrap()
                .finish()
                .unwrap(),
        ))
        .unwrap();
    let w = causal_work::read();
    no_old_freeze(w);
    assert_eq!(w.freeze.new_nodes, w.nodes_created);
    let contributions = r
        .provenance()
        .records
        .iter()
        .find_map(|record| match record {
            ProvenanceRecord::PulseDerived {
                contributions,
                result,
                ..
            } if result.get() == 4 => Some(contributions),
            _ => None,
        })
        .unwrap();
    assert_eq!(contributions.len(), 2);
    assert!(contributions.iter().all(|c| c.count().get() == 2));
    assert_eq!(contributions[0].cause(), contributions[1].cause());
    assert_ne!(contributions[0].port(), contributions[1].port());
    crate::state_digest_reference::assert_view(&c, r.provenance());
}

#[test]
fn incremental_freezing_qualified_source_bytes_survive_changed_target_slots_and_restore() {
    use super::cause_remapping_tests::{fixture, round_trip, seeded};
    let source = fixture(1);
    let mut m = seeded(&source);
    let prior = m.store.provenance.as_ref().unwrap().clone();
    let explicit = crate::state_digest_reference::canonical_view_in_source(&source, &prior);
    for i in 0..prior.len() {
        let cause = prior.records.cause(i);
        let (digest, bytes) = crate::state_digest::cause_content(&prior, cause);
        assert_eq!(explicit.cause(cause), digest);
        assert_eq!(explicit.records[&digest], bytes);
    }
    let roots: Vec<_> = m
        .store
        .standard_causes
        .values()
        .flat_map(|s| s.retained_causes())
        .collect();
    assert!(!roots.is_empty());
    let p = m
        .prepare_patch(
            m.patch()
                .add_node(crate::authored::NodeDef::new(
                    NodeKey::from_u128(5),
                    crate::authored::NodeKind::constant(LogicLevel::High),
                    crate::authored::NodePorts::new(
                        vec![],
                        vec![
                            crate::key::OutPortKey::<crate::signal::Level>::from_u128(9_000_000)
                                .into(),
                        ],
                    ),
                    crate::metadata::DiagnosticMeta::default(),
                ))
                .unwrap()
                .finish(),
        )
        .require_artifact()
        .unwrap();
    let target = p.resulting_compiled().clone();
    let before = source.current_cause_slots();
    let after = target.current_cause_slots();
    assert!(before.iter().any(|(old, subject)| {
        subject
            .windows(b"module_node".len())
            .any(|s| s == b"module_node")
            && after.iter().any(|(new, s)| s == subject && old != new)
    }));
    let tx = Transaction::advance(
        Time::from_ticks(5),
        m.revision(),
        p.input_delta().finish().unwrap(),
    )
    .with_patch(p, crate::ReconfigurationPolicy::RejectStateLoss)
    .unwrap();
    causal_work::reset();
    let r = m.apply(tx).unwrap();
    let w = causal_work::read();
    no_old_freeze(w);
    assert!(w.source_translation_lookups > 0 && w.freeze.new_nodes > 0);
    assert_eq!(w.freeze.new_nodes, w.nodes_created);
    for cause in roots {
        let payload = &explicit.records[&explicit.cause(cause)];
        assert!(r.provenance().records.iter().any(|record|matches!(record,ProvenanceRecord::Checkpoint{fact,..} if fact.windows(payload.len()).any(|s|s==payload))));
    }
    for i in 0..prior.len() {
        let cause = prior.records.cause(i);
        assert_eq!(
            crate::state_digest::cause_content(&prior, cause).0,
            explicit.cause(cause)
        );
    }
    let mut restored = round_trip(&m);
    assert_eq!(m.snapshot(), restored.snapshot());
    for machine in [&mut m, &mut restored] {
        let tx = Transaction::advance(
            Time::from_ticks(8),
            machine.revision(),
            machine.compiled.input_delta().finish().unwrap(),
        );
        causal_work::reset();
        drop(machine.apply(tx).unwrap());
        no_old_freeze(causal_work::read());
    }
    assert_eq!(m.snapshot(), restored.snapshot());
    eprintln!("incremental freeze qualified source: {w:?}");
}

#[test]
fn incremental_freezing_zero_new_views_do_no_freeze_work() {
    let c = compiled();
    let mut measurements = Vec::new();
    for history in [1, 10, 1000] {
        let (old, tail) = old_view(&c, history);
        let scope = crate::causal_store::fresh_scope(Some(old.scope));
        let view = ProvenanceView {
            scope,
            records: Arc::new(old.records.fork(scope)),
        };
        causal_work::reset();
        let selected = view.owned_roots(&[tail]);
        assert_eq!(selected.len(), history);
        let w = causal_work::read();
        assert_eq!(w.nodes_created, 0);
        assert_eq!(w.canonical_encodes, 0);
        assert!(view.records.shared_node(&old.records, tail));
        eprintln!("incremental freeze zero history={history}: {w:?}");
        measurements.push(w);
    }
    for w in measurements {
        no_old_freeze(w);
        assert_eq!(w.freeze, causal_work::FreezeWork::default());
    }
}
