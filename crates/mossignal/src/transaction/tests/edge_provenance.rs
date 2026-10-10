//! Bounded semantic premises, ownership and production work for ms-49a.2.
use super::*;
use crate::causal_work;
use crate::key::{ExternalInputKey, ExternalOutputKey, NodeKey};
use crate::metadata::DiagnosticMeta;
use crate::signal::{Level, LogicLevel, Pulse, PulseCount};
use crate::time::Time;
use crate::{EdgeConfig, EdgeDetectorKind, EdgeInitialization, NetworkBuilder, RuntimePolicy};

const EDGE: NodeKey = NodeKey::from_u128(20);
const INPUT: ExternalInputKey<Level> = ExternalInputKey::from_u128(10);
const PULSE: ExternalInputKey<Pulse> = ExternalInputKey::from_u128(11);

fn policy(growth: u64) -> RuntimePolicy {
    RuntimePolicy::builder()
        .max_internal_reactions(100)
        .max_evaluated_operations(100_000)
        .max_pending_events(100)
        .max_events_created_per_transaction(100)
        .max_required_provenance_growth(growth)
        .build()
        .unwrap()
}

fn fixture(
    kind: EdgeDetectorKind,
    init: EdgeInitialization,
    level: LogicLevel,
    constant: bool,
) -> crate::CompiledNetwork<()> {
    let mut b = NetworkBuilder::with_key(
        crate::key::NetworkKey::from_u128(1),
        crate::TimeDomainId::from_u128(2),
    );
    let level = if constant {
        b.constant(level)
    } else {
        b.add_level_input(INPUT, DiagnosticMeta::default()).unwrap()
    };
    let pulse = b.add_pulse_input(PULSE, DiagnosticMeta::default()).unwrap();
    let config = EdgeConfig::new(init);
    let edge = match kind {
        EdgeDetectorKind::Rising => {
            b.add_rising_edge(EDGE, level, config, DiagnosticMeta::default())
        }
        EdgeDetectorKind::Falling => {
            b.add_falling_edge(EDGE, level, config, DiagnosticMeta::default())
        }
        EdgeDetectorKind::Any => b.add_any_edge(EDGE, level, config, DiagnosticMeta::default()),
    }
    .unwrap()
    .into_outputs();
    b.add_pulse_output(
        ExternalOutputKey::from_u128(30),
        edge,
        DiagnosticMeta::default(),
    )
    .unwrap();
    b.add_pulse_output(
        ExternalOutputKey::from_u128(31),
        pulse,
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

fn initialize(
    c: &crate::CompiledNetwork<()>,
    level: LogicLevel,
    constant: bool,
    growth: u64,
) -> Machine<()> {
    let mut m = c.spawn(policy(growth));
    let snapshot = c.input_snapshot();
    let snapshot = if constant {
        snapshot
    } else {
        snapshot.set(INPUT, level).unwrap()
    };
    m.apply(Transaction::initialize(
        Time::from_ticks(0),
        m.revision(),
        snapshot.finish().unwrap(),
    ))
    .unwrap();
    m
}

fn advance(m: &mut Machine<()>, at: u64) -> TransactionResult<()> {
    let delta = m
        .compiled
        .input_delta()
        .pulse(PULSE, PulseCount::ONE)
        .unwrap()
        .finish()
        .unwrap();
    m.apply(Transaction::advance(
        Time::from_ticks(at),
        m.revision(),
        delta,
    ))
    .unwrap()
}

fn digest(m: &Machine<()>, cause: CauseRef) -> [u8; 32] {
    crate::state_digest::cause_content(m.store.provenance.as_ref().unwrap(), cause).0
}

fn parents(m: &Machine<()>, cause: CauseRef) -> BTreeSet<[u8; 32]> {
    let view = m.store.provenance.as_ref().unwrap();
    let CauseInspection::Derived {
        subject,
        supporters,
    } = view.inspect(cause).unwrap()
    else {
        panic!("edge fact is Derived")
    };
    assert_eq!(subject, crate::ProvenanceSubject::Node(EDGE));
    supporters.iter().map(|cause| digest(m, *cause)).collect()
}

// This oracle constructs support sets from supplied roles and the admitted
// reaction. It does not call evaluation, append_evaluation_provenance or roots.
fn assert_support(m: &Machine<()>, previous: Option<[u8; 32]>) {
    let view = m.store.provenance.as_ref().unwrap();
    let reaction = view.records.iter().enumerate().find_map(|(position, record)| {
        if matches!(record, ProvenanceRecord::ReadyTransaction { at, .. } | ProvenanceRecord::InitializationTransaction { at, .. } if Some(*at) == m.store.last_reaction) {
            Some(view.records.cause(position))
        } else { None }
    }).unwrap();
    let input = m.store.operation_causes[m.compiled.edge_input_operation(EDGE).unwrap()];
    let observation = m.store.edge_observation_causes[&EDGE];
    let comparison = m.store.operation_causes[m.compiled.node_operation(EDGE).unwrap()];
    let expected_memory = BTreeSet::from([digest(m, reaction), digest(m, input)]);
    let mut expected_comparison = expected_memory.clone();
    expected_comparison.extend(previous);
    assert_eq!(parents(m, observation), expected_memory);
    assert_eq!(parents(m, comparison), expected_comparison);
}

#[test]
fn edge_observation_constant_and_external_matrices_have_bounded_production_work() {
    for constant in [false, true] {
        for kind in [
            EdgeDetectorKind::Rising,
            EdgeDetectorKind::Falling,
            EdgeDetectorKind::Any,
        ] {
            for init in [
                EdgeInitialization::Baseline,
                EdgeInitialization::Assume(LogicLevel::Low),
                EdgeInitialization::Assume(LogicLevel::High),
            ] {
                for level in [LogicLevel::Low, LogicLevel::High] {
                    let c = fixture(kind, init, level, constant);
                    let mut measured = Vec::new();
                    for length in [1, 10, 1000] {
                        let mut m = initialize(&c, level, constant, 1000);
                        assert_support(&m, None); // C/M may canonically coalesce.
                        for at in 1..=length {
                            let previous = digest(&m, m.store.edge_observation_causes[&EDGE]);
                            drop(advance(&mut m, at));
                            if at == length {
                                assert_support(&m, Some(previous));
                            }
                        }
                        causal_work::reset();
                        drop(advance(&mut m, 2000));
                        let work = causal_work::read(); // Before observers/reference.
                        let catalogue = m.store.provenance.as_ref().unwrap().len();
                        if kind == EdgeDetectorKind::Any
                            && init == EdgeInitialization::Baseline
                            && level == LogicLevel::Low
                            && length == 1
                        {
                            eprintln!(
                                "edge quiet production: constant={constant}, catalogue={catalogue}, work={work:?}, live={}",
                                causal_work::live_nodes()
                            );
                        }
                        assert_eq!(work.old_record_rewrites, 0);
                        assert_eq!(work.scope_records_hashed, 0);
                        assert_eq!(work.nodes_created, if constant { 7 } else { 6 });
                        assert_eq!(work.logical_growth, work.nodes_created as u64);
                        measured.push((
                            catalogue,
                            work.catalogue_handles_copied,
                            work.membership_entries_copied,
                            work.owned_closure_records_visited,
                            work.closure_records_visited,
                            work.closure_records_emitted,
                            work.canonical_encodes,
                            work.canonical_hashes,
                        ));
                        crate::state_digest_reference::assert_machine(&m);
                    }
                    assert_eq!(
                        measured[0], measured[1],
                        "{kind:?}/{init:?}/{level:?}/{constant}"
                    );
                    assert_eq!(measured[1], measured[2]);
                }
            }
        }
    }
}

#[test]
fn edge_observation_exact_logical_budget_includes_coalesced_facts() {
    // Initialization: transaction + input + C + M + two output facts = 6.
    // Ready with unrelated pulse: transaction + pulse + C + M + two outputs = 6.
    for init in [
        EdgeInitialization::Baseline,
        EdgeInitialization::Assume(LogicLevel::Low),
        EdgeInitialization::Assume(LogicLevel::High),
    ] {
        let c = fixture(EdgeDetectorKind::Any, init, LogicLevel::Low, false);
        for limit in [5, 6, 7] {
            let mut m = c.spawn(policy(limit));
            let before = m.snapshot().artifact_bytes().to_vec();
            let input = c
                .input_snapshot()
                .set(INPUT, LogicLevel::Low)
                .unwrap()
                .finish()
                .unwrap();
            causal_work::reset();
            let initialized = m.apply(Transaction::initialize(
                Time::from_ticks(0),
                m.revision(),
                input,
            ));
            assert_eq!(causal_work::read().nodes_created, 6);
            assert_eq!(causal_work::read().logical_growth, 6);
            if limit == 5 {
                assert!(matches!(
                    initialized.unwrap_err().evidence(),
                    RuntimeFailureEvidence::BudgetExceeded {
                        budget: RuntimePolicyLimit::MaxRequiredProvenanceGrowth,
                        consumed: 6,
                        limit: 5
                    }
                ));
                assert_eq!(m.snapshot().artifact_bytes(), before);
            } else {
                drop(initialized.unwrap());
                assert_support(&m, None);
                causal_work::reset();
                drop(advance(&mut m, 1));
                assert_eq!(causal_work::read().logical_growth, 6);
            }
        }
    }
}

#[test]
fn edge_observation_owned_artifacts_do_not_enlarge_machine_work_and_outlive_it() {
    let c = fixture(
        EdgeDetectorKind::Any,
        EdgeInitialization::Baseline,
        LogicLevel::Low,
        false,
    );
    let run = |held: bool| {
        let mut m = initialize(&c, LogicLevel::Low, false, 1000);
        let transition = Transaction::advance(
            Time::from_ticks(1),
            m.revision(),
            c.input_delta()
                .set(INPUT, LogicLevel::High)
                .unwrap()
                .finish()
                .unwrap(),
        );
        let result = m.apply(transition).unwrap();
        let inspection = m.inspect_node(EDGE).unwrap();
        let forecast = m
            .forecast(Transaction::advance(
                Time::from_ticks(2),
                m.revision(),
                c.input_delta()
                    .set(INPUT, LogicLevel::Low)
                    .unwrap()
                    .finish()
                    .unwrap(),
            ))
            .unwrap();
        let output = result
            .output_events()
            .iter()
            .find_map(|event| match event {
                OutputEvent::Pulsed { output, cause, .. } if output.as_u128() == 30 => Some(*cause),
                _ => None,
            })
            .unwrap();
        let before = crate::state_digest_reference::canonical_view(&c, result.provenance());
        let owned = held.then_some((result, inspection, forecast));
        for at in 2..=1000 {
            drop(advance(&mut m, at));
        }
        causal_work::reset();
        drop(advance(&mut m, 2000));
        let work = causal_work::read();
        let snapshot = m.snapshot().artifact_bytes().to_vec();
        let roots = m.store.provenance.as_ref().unwrap().len();
        let live_with_machine = causal_work::live_nodes();
        if let Some((result, inspection, forecast)) = &owned {
            assert_eq!(
                crate::state_digest_reference::canonical_view(&c, result.provenance()).records,
                before.records
            );
            assert!(
                inspection
                    .provenance()
                    .inspect(inspection.last_reaction_cause)
                    .is_ok()
            );
            assert!(!forecast.result().provenance().is_empty());
        }
        drop(m);
        if let Some((result, inspection, forecast)) = &owned {
            assert!(result.provenance().inspect(output).is_ok());
            assert_eq!(
                crate::state_digest_reference::canonical_view(&c, result.provenance()).records,
                before.records
            );
            assert!(
                inspection
                    .provenance()
                    .inspect(inspection.last_reaction_cause)
                    .is_ok()
            );
            assert!(
                forecast
                    .result()
                    .output_events()
                    .iter()
                    .any(|event| matches!(event, OutputEvent::Pulsed { .. }))
            );
            assert!(causal_work::live_nodes() > 0);
        }
        drop(owned);
        assert_eq!(causal_work::live_nodes(), 0);
        (
            snapshot,
            roots,
            work.nodes_created,
            work.catalogue_handles_copied,
            work.membership_entries_copied,
            work.owned_closure_records_visited,
            work.closure_records_visited,
            live_with_machine,
        )
    };
    let dropped = run(false);
    let held = run(true);
    assert_eq!(dropped.0, held.0);
    assert_eq!(
        (
            dropped.1, dropped.2, dropped.3, dropped.4, dropped.5, dropped.6
        ),
        (held.1, held.2, held.3, held.4, held.5, held.6)
    );
    assert!(held.7 > dropped.7);
}

#[test]
fn edge_observation_preserves_genuine_cumulative_toggle_ancestry() {
    let mut b = NetworkBuilder::<()>::new(crate::TimeDomainId::from_u128(2));
    let pulse = b.add_pulse_input(PULSE, DiagnosticMeta::default()).unwrap();
    let toggle = b
        .toggle(pulse, crate::ToggleConfig::new(LogicLevel::Low))
        .unwrap();
    let edge = b
        .add_any_edge(
            EDGE,
            toggle,
            EdgeConfig::new(EdgeInitialization::Baseline),
            DiagnosticMeta::default(),
        )
        .unwrap()
        .into_outputs();
    b.pulse_output("edge", edge).unwrap();
    let c = b
        .finish()
        .require_artifact()
        .unwrap()
        .compile()
        .require_artifact()
        .unwrap();
    let mut m = c.spawn(policy(1000));
    m.apply(Transaction::initialize(
        Time::from_ticks(0),
        m.revision(),
        c.input_snapshot().finish().unwrap(),
    ))
    .unwrap();
    let mut counts = Vec::new();
    for at in 1..=10 {
        let previous = digest(&m, m.store.edge_observation_causes[&EDGE]);
        let delta = c
            .input_delta()
            .pulse(PULSE, PulseCount::ONE)
            .unwrap()
            .finish()
            .unwrap();
        drop(
            m.apply(Transaction::advance(
                Time::from_ticks(at),
                m.revision(),
                delta,
            ))
            .unwrap(),
        );
        assert_support(&m, Some(previous));
        let view = m.store.provenance.as_ref().unwrap();
        let mut pending = vec![m.store.edge_observation_causes[&EDGE]];
        let mut seen = BTreeSet::new();
        let mut observations = 0;
        while let Some(cause) = pending.pop() {
            if !seen.insert(cause) {
                continue;
            }
            match view.inspect(cause).unwrap() {
                CauseInspection::Derived { supporters, .. } => {
                    pending.extend_from_slice(supporters)
                }
                CauseInspection::ExternalPulseObservation { count, .. } => {
                    assert_eq!(count.get(), 1);
                    observations += 1;
                }
                _ => {}
            }
        }
        assert_eq!(observations, at); // Real toggles keep all genuine past observations.
        counts.push(view.len());
    }
    assert!(counts.windows(2).all(|pair| pair[1] > pair[0]));
}

#[test]
fn edge_observation_candidate_faults_restore_complete_warm_predecessor() {
    let c = fixture(
        EdgeDetectorKind::Any,
        EdgeInitialization::Baseline,
        LogicLevel::Low,
        false,
    );
    for ready in [false, true] {
        for patch in [false, true] {
            for fault in [
                causal_work::Fault::StateExtraction,
                causal_work::Fault::Provenance,
                causal_work::Fault::DigestPreparation,
                causal_work::Fault::Result,
            ] {
                let mut m = if ready {
                    initialize(&c, LogicLevel::Low, false, 1000)
                } else {
                    c.spawn(policy(1000))
                };
                let held = ready.then(|| m.inspect_node(EDGE).unwrap());
                let before = super::tests::observe(&m);
                let mut tx = if ready {
                    Transaction::advance(
                        Time::from_ticks(5),
                        m.revision(),
                        c.input_delta()
                            .set(INPUT, LogicLevel::High)
                            .unwrap()
                            .finish()
                            .unwrap(),
                    )
                } else {
                    Transaction::initialize(
                        Time::from_ticks(0),
                        m.revision(),
                        c.input_snapshot()
                            .set(INPUT, LogicLevel::High)
                            .unwrap()
                            .finish()
                            .unwrap(),
                    )
                };
                if patch {
                    let p = m
                        .prepare_patch(
                            m.patch()
                                .set_diagnostic_meta(
                                    crate::StructuralSubjectRef::Node(EDGE),
                                    DiagnosticMeta {
                                        name: Some("patched".to_owned()),
                                        ..DiagnosticMeta::default()
                                    },
                                )
                                .unwrap()
                                .finish(),
                        )
                        .require_artifact()
                        .unwrap();
                    tx = tx
                        .with_patch(p, crate::ReconfigurationPolicy::RejectStateLoss)
                        .unwrap();
                }
                let live = causal_work::live_nodes();
                causal_work::inject(fault);
                assert!(m.apply(tx).is_err());
                assert_eq!(super::tests::observe(&m), before);
                assert_eq!(causal_work::live_nodes(), live);
                if let Some(held) = held {
                    assert!(held.provenance().inspect(held.last_reaction_cause).is_ok());
                }
            }
        }
    }
}

#[test]
fn edge_observation_distinct_roles_match_exact_p4_snapshot_vector() {
    let c = fixture(
        EdgeDetectorKind::Any,
        EdgeInitialization::Baseline,
        LogicLevel::Low,
        false,
    );
    let mut m = initialize(&c, LogicLevel::Low, false, 1000);
    for at in 1..=2 {
        let previous = digest(&m, m.store.edge_observation_causes[&EDGE]);
        drop(
            m.apply(Transaction::advance(
                Time::from_ticks(at),
                m.revision(),
                c.input_delta()
                    .set(INPUT, LogicLevel::High)
                    .unwrap()
                    .finish()
                    .unwrap(),
            ))
            .unwrap(),
        );
        assert_support(&m, Some(previous));
    }
    crate::state_digest_reference::assert_machine(&m);
    let actual: String = m
        .snapshot()
        .artifact_bytes()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/golden/machine_snapshot_edge_roles_v4.hex");
    assert_eq!(actual, std::fs::read_to_string(path).unwrap().trim());
}

#[test]
fn edge_observation_deadline_outer_transaction_has_an_exact_append_ledger() {
    use crate::time::NonZeroSpan;
    let mut b = NetworkBuilder::<()>::with_key(
        crate::key::NetworkKey::from_u128(1),
        crate::TimeDomainId::from_u128(2),
    );
    let input = b.add_level_input(INPUT, DiagnosticMeta::default()).unwrap();
    let pulse = b.add_pulse_input(PULSE, DiagnosticMeta::default()).unwrap();
    let transport = b
        .add_transport_delay(
            NodeKey::from_u128(12),
            input,
            crate::TransportDelayConfig::new(NonZeroSpan::from_ticks(5).unwrap(), LogicLevel::Low),
            DiagnosticMeta::default(),
        )
        .unwrap()
        .into_outputs();
    let edge = b
        .add_any_edge(
            EDGE,
            transport,
            EdgeConfig::new(EdgeInitialization::Baseline),
            DiagnosticMeta::default(),
        )
        .unwrap()
        .into_outputs();
    b.add_pulse_output(
        ExternalOutputKey::from_u128(30),
        edge,
        DiagnosticMeta::default(),
    )
    .unwrap();
    b.add_pulse_output(
        ExternalOutputKey::from_u128(31),
        pulse,
        DiagnosticMeta::default(),
    )
    .unwrap();
    b.add_level_output(
        ExternalOutputKey::from_u128(32),
        transport,
        DiagnosticMeta::default(),
    )
    .unwrap();
    let c = b
        .finish()
        .require_artifact()
        .unwrap()
        .compile()
        .require_artifact()
        .unwrap();
    for at in [5, 6] {
        for patched in [false, true] {
            let mut probe = initialize(&c, LogicLevel::High, false, 1000);
            let transaction = |m: &Machine<()>| {
                let mut tx = Transaction::advance(
                    Time::from_ticks(at),
                    m.revision(),
                    c.input_delta().finish().unwrap(),
                );
                if patched {
                    let patch = m
                        .prepare_patch(
                            m.patch()
                                .set_diagnostic_meta(
                                    crate::StructuralSubjectRef::Node(EDGE),
                                    DiagnosticMeta {
                                        name: Some("deadline patch".to_owned()),
                                        ..DiagnosticMeta::default()
                                    },
                                )
                                .unwrap()
                                .finish(),
                        )
                        .require_artifact()
                        .unwrap();
                    tx = tx
                        .with_patch(patch, crate::ReconfigurationPolicy::RejectStateLoss)
                        .unwrap();
                }
                tx
            };
            causal_work::reset();
            let result = probe.apply(transaction(&probe)).unwrap();
            let work = causal_work::read();
            // Deadline settlement: T + transport + C/M + two pulse outputs +
            // newly established level output = 7. A later quiet target adds 6.
            // Exact-deadline migration adds topology + fresh input + baseline +
            // edge + transport + pending migration + pending record = 7; target
            // settlement costs 7. Later patch: source due 7 + migration without
            // pending 5 + quiet target 6. SOURCE replacements are uncharged.
            let expected = if patched {
                if at == 5 { 14 } else { 18 }
            } else if at == 5 {
                7
            } else {
                13
            };
            assert_eq!(
                work.logical_growth, expected,
                "at={at}, patch={patched}, work={work:?}"
            );
            for limit in [expected - 1, expected, expected + 1] {
                let mut m = initialize(&c, LogicLevel::High, false, 1000);
                m.policy = policy(limit);
                let before = super::tests::observe(&m);
                let live = causal_work::live_nodes();
                let tx = transaction(&m);
                causal_work::reset();
                let applied = m.apply(tx);
                assert_eq!(causal_work::read().logical_growth, expected);
                if limit < expected {
                    assert!(applied.is_err());
                    assert_eq!(super::tests::observe(&m), before);
                    assert_eq!(causal_work::live_nodes(), live);
                } else {
                    let applied = applied.unwrap();
                    assert_eq!(applied.output_events().len(), result.output_events().len());
                    crate::state_digest_reference::assert_machine(&m);
                }
            }
        }
    }
}

#[test]
fn edge_observation_initialization_patch_exact_budget_charges_both_roles() {
    let c = fixture(
        EdgeDetectorKind::Any,
        EdgeInitialization::Baseline,
        LogicLevel::Low,
        false,
    );
    for limit in [7, 8, 9] {
        let mut m = c.spawn(policy(limit));
        let before = super::tests::observe(&m);
        let patch = m
            .prepare_patch(
                m.patch()
                    .set_diagnostic_meta(
                        crate::StructuralSubjectRef::Node(EDGE),
                        DiagnosticMeta {
                            name: Some("initial ledger".to_owned()),
                            ..DiagnosticMeta::default()
                        },
                    )
                    .unwrap()
                    .finish(),
            )
            .require_artifact()
            .unwrap();
        let input = patch
            .resulting_compiled()
            .input_snapshot()
            .set(INPUT, LogicLevel::Low)
            .unwrap()
            .finish()
            .unwrap();
        causal_work::reset();
        let result = m.apply(
            Transaction::initialize(Time::from_ticks(0), m.revision(), input)
                .with_patch(patch, crate::ReconfigurationPolicy::RejectStateLoss)
                .unwrap(),
        );
        // Initialization + TopologyChange + declared checkpoint + observation
        // + two logical edge roles + two output facts = 8, even coalesced C/M.
        assert_eq!(causal_work::read().logical_growth, 8);
        assert_eq!(causal_work::read().nodes_created, 8);
        if limit < 8 {
            assert!(result.is_err());
            assert_eq!(super::tests::observe(&m), before);
        } else {
            drop(result.unwrap());
            crate::state_digest_reference::assert_machine(&m);
        }
    }
}

#[test]
fn edge_observation_overflow_preserves_complete_predecessor_and_held_view() {
    let mut b = NetworkBuilder::<()>::new(crate::TimeDomainId::from_u128(2));
    let level = b.add_level_input(INPUT, DiagnosticMeta::default()).unwrap();
    let left = b.add_pulse_input(PULSE, DiagnosticMeta::default()).unwrap();
    let right_key = ExternalInputKey::<Pulse>::from_u128(12);
    let right = b
        .add_pulse_input(right_key, DiagnosticMeta::default())
        .unwrap();
    b.add_any_edge(
        EDGE,
        level,
        EdgeConfig::new(EdgeInitialization::Baseline),
        DiagnosticMeta::default(),
    )
    .unwrap();
    b.merge([left, right]).unwrap();
    let c = b
        .finish()
        .require_artifact()
        .unwrap()
        .compile()
        .require_artifact()
        .unwrap();
    let mut m = initialize(&c, LogicLevel::Low, false, 1000);
    let held = m.inspect_node(EDGE).unwrap();
    let before = super::tests::observe(&m);
    let live = causal_work::live_nodes();
    let input = c
        .input_delta()
        .set(INPUT, LogicLevel::High)
        .unwrap()
        .pulse(PULSE, PulseCount::new(u64::MAX))
        .unwrap()
        .pulse(right_key, PulseCount::ONE)
        .unwrap()
        .finish()
        .unwrap();
    let failure = m
        .apply(Transaction::advance(
            Time::from_ticks(1),
            m.revision(),
            input,
        ))
        .unwrap_err();
    assert!(matches!(
        failure.evidence(),
        RuntimeFailureEvidence::PulseCountOverflow { .. }
    ));
    assert_eq!(super::tests::observe(&m), before);
    assert_eq!(causal_work::live_nodes(), live);
    assert!(held.provenance().inspect(held.last_reaction_cause).is_ok());
}
