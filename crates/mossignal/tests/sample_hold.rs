//! SampleHold conformance: fixed kinds/roles, initialization, current dependencies,
//! output/successor law, unordered pulse presence, inspection and rooted capture causality.
//! No SampleHold-owned due work, scheduling, conflicts or diagnostic episodes apply.
//! Snapshot/replay and topology/migration conformance await those owning systems.
use mossignal::diagnostics::{DiagnosticCode, ProblemEvidence};
use mossignal::key::*;
use mossignal::metadata::DiagnosticMeta;
use mossignal::signal::{Level, LogicLevel, Pulse, PulseCount};
use mossignal::time::{NonZeroSpan, Time};
use mossignal::{
    CauseInspection, CauseRef, CompiledNetwork, EdgeConfig, EdgeInitialization, Machine,
    NetworkBuilder, NodeSubject, OutputEvent, ProvenanceView, RuntimePolicy, SampleHoldConfig,
    SampleHoldInspection, TimeDomainId, ToggleConfig, Transaction, TransactionResult,
};
use std::collections::BTreeSet;

// Owned inspection must not require Clone on the caller's time-domain marker.
#[derive(Debug, PartialEq, Eq)]
struct Domain;
const LOW: LogicLevel = LogicLevel::Low;
const HIGH: LogicLevel = LogicLevel::High;
fn policy() -> RuntimePolicy {
    RuntimePolicy::builder()
        .max_internal_reactions(1000)
        .max_evaluated_operations(100_000)
        .max_pending_events(1000)
        .max_events_created_per_transaction(10_000)
        .max_required_provenance_growth(100_000)
        .build()
        .unwrap()
}
fn compile(builder: NetworkBuilder<Domain>) -> CompiledNetwork<Domain> {
    builder
        .finish()
        .require_artifact()
        .unwrap()
        .compile()
        .require_artifact()
        .unwrap()
}
struct Fixture {
    c: CompiledNetwork<Domain>,
    value: ExternalInputKey<Level>,
    sample: ExternalInputKey<Pulse>,
    output: ExternalOutputKey<Level>,
    node: NodeKey,
}
fn fixture(initial: LogicLevel) -> Fixture {
    let mut b = NetworkBuilder::with_key(NetworkKey::from_u128(1), TimeDomainId::from_u128(2));
    let value = ExternalInputKey::from_u128(1);
    let sample = ExternalInputKey::from_u128(2);
    let v = b.add_level_input(value, DiagnosticMeta::default()).unwrap();
    let p = b
        .add_pulse_input(sample, DiagnosticMeta::default())
        .unwrap();
    let node = NodeKey::from_u128(10);
    let state = b
        .add_sample_hold_with_ports(
            node,
            InPortKey::from_u128(20),
            InPortKey::from_u128(21),
            OutPortKey::from_u128(30),
            v,
            p,
            SampleHoldConfig::new(initial),
            DiagnosticMeta::default(),
        )
        .unwrap()
        .into_outputs();
    let output = ExternalOutputKey::from_u128(40);
    b.add_level_output(output, state, DiagnosticMeta::default())
        .unwrap();
    Fixture {
        c: compile(b),
        value,
        sample,
        output,
        node,
    }
}
fn initialize(
    f: &Fixture,
    value: LogicLevel,
    count: u64,
) -> (Machine<Domain>, TransactionResult<Domain>) {
    let mut m = f.c.spawn(policy());
    let r = m
        .apply(Transaction::initialize(
            Time::from_ticks(5),
            m.revision(),
            f.c.input_snapshot()
                .set(f.value, value)
                .unwrap()
                .pulse(f.sample, PulseCount::new(count))
                .unwrap()
                .finish()
                .unwrap(),
        ))
        .unwrap();
    (m, r)
}
fn advance(
    f: &Fixture,
    m: &mut Machine<Domain>,
    at: u64,
    value: LogicLevel,
    count: u64,
) -> TransactionResult<Domain> {
    m.apply(Transaction::advance(
        Time::from_ticks(at),
        m.revision(),
        f.c.input_delta()
            .set(f.value, value)
            .unwrap()
            .pulse(f.sample, PulseCount::new(count))
            .unwrap()
            .finish()
            .unwrap(),
    ))
    .unwrap()
}
#[derive(Debug, Default, PartialEq, Eq)]
struct Facts {
    levels: BTreeSet<(u128, LogicLevel)>,
    pulses: BTreeSet<(u128, u64)>,
    times: BTreeSet<u64>,
}
fn facts(view: &ProvenanceView<Domain>, root: CauseRef) -> Facts {
    let mut out = Facts::default();
    let mut todo = vec![root];
    let mut visited = BTreeSet::new();
    while let Some(cause) = todo.pop() {
        if !visited.insert(cause) {
            continue;
        }
        match view.inspect(cause).unwrap() {
            CauseInspection::InitializationTransaction { at, .. }
            | CauseInspection::ReadyTransaction { at, .. } => {
                out.times.insert(at.time().ticks());
            }
            CauseInspection::ExternalObservation { input, value, .. } => {
                out.levels.insert((input.as_u128(), value));
            }
            CauseInspection::ExternalPulseObservation { input, count, .. } => {
                out.pulses.insert((input.as_u128(), count.get()));
            }
            CauseInspection::Derived { supporters, .. }
            | CauseInspection::PulseDerived { supporters, .. }
            | CauseInspection::PulseControlledLevel { supporters, .. }
            | CauseInspection::PendingPulseDelay { supporters, .. } => {
                todo.extend_from_slice(supporters)
            }
            _ => panic!("unexpected cause kind"),
        }
    }
    out
}
fn establishment_time(inspection: &SampleHoldInspection<Domain>) -> u64 {
    *facts(inspection.provenance(), inspection.latest_establishment())
        .times
        .last()
        .unwrap()
}
#[test]
fn independent_initialization_and_ready_laws_exhaust_values_and_sample_presence() {
    for previous in [LOW, HIGH] {
        for value in [LOW, HIGH] {
            for count in [0, 1, 2, u64::MAX] {
                let f = fixture(previous);
                let expected = if count == 0 { previous } else { value };
                let (m, result) = initialize(&f, value, count);
                assert_eq!(m.output_level(f.output), Some(expected));
                let i = m.inspect_sample_hold(f.node).unwrap();
                assert_eq!(
                    (i.initial(), i.committed(), i.value(), i.at().ticks()),
                    (previous, expected, value, 5)
                );
                assert_eq!(i.node(), &NodeSubject::Node(f.node));
                assert_eq!(i.revision(), m.revision());
                assert!(
                    matches!(result.output_events(),[OutputEvent::LevelEstablished { value: found, .. }] if *found==expected)
                );
                assert!(result.occurrences().is_empty());
                assert!(result.diagnostic_episode_changes().is_empty());
                assert!(m.active_diagnostic_episodes().unwrap().is_empty());
                let (mut ready, _) = initialize(&f, previous, 0);
                let result = advance(&f, &mut ready, 6, value, count);
                assert_eq!(ready.output_level(f.output), Some(expected));
                let i = ready.inspect_sample_hold(f.node).unwrap();
                assert_eq!(i.committed(), expected);
                assert_eq!(establishment_time(&i), if count == 0 { 5 } else { 6 });
                assert_eq!(
                    result.output_events().len(),
                    usize::from(previous != expected)
                );
                let absent = ready
                    .apply(Transaction::advance(
                        Time::from_ticks(7),
                        ready.revision(),
                        f.c.input_delta().finish().unwrap(),
                    ))
                    .unwrap();
                assert_eq!(ready.output_level(f.output), Some(expected));
                assert!(absent.output_events().is_empty());
            }
        }
    }
}
#[test]
fn inspection_rejects_structural_and_lifecycle_misuse() {
    let f = fixture(HIGH);
    let m = f.c.spawn(policy());
    let definition = m.inspect_sample_hold_definition(f.node).unwrap();
    assert_eq!((definition.node(), definition.initial()), (f.node, HIGH));
    assert_eq!(
        m.inspect_sample_hold(f.node).unwrap_err().code(),
        DiagnosticCode::LifecycleNotInitialized
    );
    assert_eq!(
        m.inspect_sample_hold_definition(NodeKey::from_u128(999))
            .unwrap_err()
            .code(),
        DiagnosticCode::InspectionUnknownSubject
    );
    let mut b = NetworkBuilder::<Domain>::new(TimeDomainId::from_u128(2));
    let wrong = NodeKey::from_u128(50);
    b.add_constant(wrong, LOW, DiagnosticMeta::default())
        .unwrap();
    let c = compile(b);
    let m = c.spawn(policy());
    assert_eq!(
        m.inspect_sample_hold_definition(wrong).unwrap_err().code(),
        DiagnosticCode::InspectionWrongSubjectKind
    );
}
#[test]
fn captures_same_values_and_retention_keep_distinct_owned_causal_facts() {
    let f = fixture(LOW);
    let (mut m, initial) = initialize(&f, LOW, 0);
    let captured = advance(&f, &mut m, 6, HIGH, 3);
    let old = m.inspect_sample_hold(f.node).unwrap().clone();
    assert_eq!(establishment_time(&old), 6);
    let r = advance(&f, &mut m, 7, HIGH, 2);
    assert!(r.output_events().is_empty());
    let same = m.inspect_sample_hold(f.node).unwrap();
    assert_eq!(establishment_time(&same), 7);
    assert_eq!(
        *facts(same.provenance(), m.output_cause(f.output).unwrap())
            .times
            .last()
            .unwrap(),
        6
    );
    let r = advance(&f, &mut m, 8, LOW, 0);
    assert!(r.output_events().is_empty());
    let held = m.inspect_sample_hold(f.node).unwrap();
    assert_eq!((held.value(), held.committed()), (LOW, HIGH));
    assert_eq!(establishment_time(&held), 7);
    assert!(
        facts(held.provenance(), held.current_support())
            .levels
            .contains(&(f.value.as_u128(), LOW))
    );
    assert!(
        facts(held.provenance(), held.latest_establishment())
            .pulses
            .contains(&(f.sample.as_u128(), 2))
    );
    advance(&f, &mut m, 9, LOW, 1);
    assert_eq!((old.committed(), old.value()), (HIGH, HIGH));
    assert_eq!(establishment_time(&old), 6);
    for r in [&initial, &captured] {
        for event in r.output_events() {
            let cause = match event {
                OutputEvent::LevelEstablished { cause, .. }
                | OutputEvent::LevelChanged { cause, .. }
                | OutputEvent::Pulsed { cause, .. } => *cause,
                _ => panic!("unexpected event kind"),
            };
            assert!(!facts(r.provenance(), cause).times.is_empty());
        }
    }
}
#[test]
fn upstream_toggle_is_sampled_in_the_same_reaction_and_feeds_falling_edge() {
    let mut b = NetworkBuilder::<Domain>::new(TimeDomainId::from_u128(2));
    let (input, p) = b.pulse_input("sample and toggle");
    let value = b.toggle(p, ToggleConfig::new(HIGH)).unwrap();
    let held = b
        .sample_hold(value, p, SampleHoldConfig::new(HIGH))
        .unwrap();
    let falling = b
        .falling_edge(held, EdgeConfig::new(EdgeInitialization::Baseline))
        .unwrap();
    let output = b.level_output("held", held).unwrap();
    let edge = b.pulse_output("falling", falling).unwrap();
    let c = compile(b);
    let mut m = c.spawn(policy());
    m.apply(Transaction::initialize(
        Time::from_ticks(0),
        m.revision(),
        c.input_snapshot().finish().unwrap(),
    ))
    .unwrap();
    let mut reference = true;
    for (at, count) in [0, 1, 2, 3, 0, 2, 1].into_iter().enumerate() {
        let before = reference;
        if count % 2 == 1 {
            reference = !reference;
        }
        let r = m
            .apply(Transaction::advance(
                Time::from_ticks(at as u64 + 1),
                m.revision(),
                c.input_delta()
                    .pulse(input, PulseCount::new(count))
                    .unwrap()
                    .finish()
                    .unwrap(),
            ))
            .unwrap();
        assert_eq!(
            m.output_level(output),
            Some(if reference { HIGH } else { LOW })
        );
        let emitted = r
            .output_events()
            .iter()
            .filter_map(|e| match e {
                OutputEvent::Pulsed { output, count, .. } if *output == edge => Some(count.get()),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(
            emitted,
            if before && !reference {
                vec![1]
            } else {
                vec![]
            }
        );
    }
}

#[derive(Clone, Copy)]
enum Defect {
    None,
    WrongKind,
    WrongRole,
    MissingPort,
    ExtraPort,
    MissingOutput,
    MissingInput,
    ExtraDriver,
    DuplicateNode,
    ValueCycle,
    SampleCycle,
}
fn dynamic(
    initial: LogicLevel,
    reverse: bool,
    defect: Defect,
) -> mossignal::authored::UncheckedNetwork<Domain> {
    use mossignal::authored::{
        ConnectionDef, ConnectionEndpoint, ExternalInputDef, ExternalOutputDef, InputPortRole,
        NodeDef, NodeKind, NodePorts, UncheckedNetwork,
    };
    let value = ExternalInputKey::<Level>::from_u128(1);
    let sample = ExternalInputKey::<Pulse>::from_u128(2);
    let vp = InPortKey::<Level>::from_u128(20);
    let sp = InPortKey::<Pulse>::from_u128(21);
    let output = OutPortKey::<Level>::from_u128(30);
    let mut ports = vec![vp.into(), sp.into()];
    let mut roles = vec![InputPortRole::Value, InputPortRole::Sample];
    if matches!(defect, Defect::WrongKind) {
        ports[1] = InPortKey::<Level>::from_u128(21).into();
    }
    if matches!(defect, Defect::WrongRole) {
        roles[1] = InputPortRole::Value;
    }
    if matches!(defect, Defect::MissingPort) {
        ports.pop();
        roles.pop();
    }
    if matches!(defect, Defect::ExtraPort) {
        ports.push(InPortKey::<Level>::from_u128(22).into());
        roles.push(InputPortRole::Value);
    }
    if reverse {
        ports.reverse();
        roles.reverse();
    }
    let mut nodes = vec![NodeDef::new(
        NodeKey::from_u128(10),
        NodeKind::sample_hold(SampleHoldConfig::new(initial)),
        NodePorts::with_input_roles(
            ports,
            roles,
            if matches!(defect, Defect::MissingOutput) {
                vec![]
            } else {
                vec![output.into()]
            },
        ),
        DiagnosticMeta::default(),
    )];
    let mut inputs = vec![
        ExternalInputDef::new(value.into(), DiagnosticMeta::default()),
        ExternalInputDef::new(sample.into(), DiagnosticMeta::default()),
    ];
    let mut connections = vec![
        ConnectionDef::new(
            ConnectionKey::from_u128(0),
            if matches!(defect, Defect::ValueCycle) {
                output.into()
            } else {
                value.into()
            },
            vp.into(),
            DiagnosticMeta::default(),
        ),
        ConnectionDef::new(
            ConnectionKey::from_u128(1),
            sample.into(),
            sp.into(),
            DiagnosticMeta::default(),
        ),
    ];
    if matches!(defect, Defect::MissingInput) {
        connections.pop();
    }
    if matches!(defect, Defect::ExtraDriver) {
        connections.push(ConnectionDef::new(
            ConnectionKey::from_u128(2),
            value.into(),
            vp.into(),
            DiagnosticMeta::default(),
        ));
    }
    if matches!(defect, Defect::DuplicateNode) {
        nodes.push(nodes[0].clone());
    }
    if matches!(defect, Defect::SampleCycle) {
        let edge_in = InPortKey::<Level>::from_u128(22);
        let edge_out = OutPortKey::<Pulse>::from_u128(31);
        nodes.push(NodeDef::new(
            NodeKey::from_u128(11),
            NodeKind::falling_edge(EdgeConfig::new(EdgeInitialization::Baseline)),
            NodePorts::new(vec![edge_in.into()], vec![edge_out.into()]),
            DiagnosticMeta::default(),
        ));
        connections[1] = ConnectionDef::new(
            ConnectionKey::from_u128(1),
            edge_out.into(),
            sp.into(),
            DiagnosticMeta::default(),
        );
        connections.push(ConnectionDef::new(
            ConnectionKey::from_u128(2),
            ConnectionEndpoint::node_output(output.into()),
            edge_in.into(),
            DiagnosticMeta::default(),
        ));
    }
    if reverse {
        nodes.reverse();
        inputs.reverse();
        connections.reverse();
    }
    UncheckedNetwork::new(
        NetworkKey::from_u128(1),
        TimeDomainId::from_u128(2),
        DiagnosticMeta::default(),
        nodes,
        inputs,
        vec![ExternalOutputDef::new(
            ExternalOutputKey::<Level>::from_u128(40).into(),
            SignalSourceKey::NodeOutput(output).into(),
            DiagnosticMeta::default(),
        )],
        connections,
    )
}
#[test]
fn typed_and_dynamic_laws_identity_and_validation_agree() {
    for initial in [LOW, HIGH] {
        let typed = fixture(initial);
        for reverse in [false, true] {
            let c = dynamic(initial, reverse, Defect::None)
                .validate()
                .require_artifact()
                .unwrap()
                .compile()
                .require_artifact()
                .unwrap();
            assert_eq!(typed.c.fingerprint(), c.fingerprint());
            let f = Fixture {
                c,
                value: typed.value,
                sample: typed.sample,
                output: typed.output,
                node: typed.node,
            };
            for value in [LOW, HIGH] {
                for count in [0, 1, 2] {
                    let (mut m, _) = initialize(&f, initial, 0);
                    advance(&f, &mut m, 6, value, count);
                    assert_eq!(
                        m.output_level(f.output),
                        Some(if count == 0 { initial } else { value })
                    );
                }
            }
        }
    }
    assert_ne!(fixture(LOW).c.fingerprint(), fixture(HIGH).c.fingerprint());
    for (defect, code) in [
        (
            Defect::WrongKind,
            DiagnosticCode::ValidationInvalidFixedArity,
        ),
        (
            Defect::WrongRole,
            DiagnosticCode::ValidationInvalidFixedArity,
        ),
        (
            Defect::MissingPort,
            DiagnosticCode::ValidationInvalidFixedArity,
        ),
        (
            Defect::ExtraPort,
            DiagnosticCode::ValidationInvalidFixedArity,
        ),
        (
            Defect::MissingOutput,
            DiagnosticCode::ValidationInvalidFixedArity,
        ),
        (
            Defect::MissingInput,
            DiagnosticCode::ValidationMissingRequiredInput,
        ),
        (
            Defect::ExtraDriver,
            DiagnosticCode::ValidationUnsupportedMultipleDrivers,
        ),
        (
            Defect::DuplicateNode,
            DiagnosticCode::ValidationDuplicateKey,
        ),
    ] {
        for reverse in [false, true] {
            let report = dynamic(LOW, reverse, defect).validate();
            assert!(
                report
                    .diagnostics()
                    .iter()
                    .any(|d| d.problem().code() == code),
                "malformed definition must report {code:?}: {:?}",
                report.diagnostics()
            );
            assert!(report.require_artifact().is_err());
        }
    }
    for defect in [Defect::ValueCycle, Defect::SampleCycle] {
        let reports = [false, true].map(|reverse| dynamic(LOW, reverse, defect).validate());
        let mut witnesses = Vec::new();
        for report in &reports {
            let cycles = report
                .diagnostics()
                .iter()
                .filter(|d| d.problem().code() == DiagnosticCode::ValidationCurrentReactionCycle)
                .collect::<Vec<_>>();
            assert_eq!(cycles.len(), 1);
            assert!(matches!(
                cycles[0].problem().evidence(),
                ProblemEvidence::ValidationCurrentReactionCycle { .. }
            ));
            witnesses.push(cycles[0].problem());
        }
        assert_eq!(witnesses[0], witnesses[1]);
        for report in reports {
            assert!(report.require_artifact().is_err());
        }
    }
}
#[test]
fn typed_construction_rejects_foreign_signals_and_each_duplicate_identity() {
    let mut a = NetworkBuilder::<Domain>::new(TimeDomainId::from_u128(2));
    let v = a.constant(LOW);
    let (_, p) = a.pulse_input("local");
    let mut b = NetworkBuilder::<Domain>::new(TimeDomainId::from_u128(2));
    let foreign_v = b.constant(HIGH);
    let (_, foreign_p) = b.pulse_input("foreign");
    let cfg = SampleHoldConfig::new(LOW);
    assert_eq!(
        a.sample_hold(foreign_v, p, cfg).unwrap_err().code(),
        DiagnosticCode::AuthoringForeignSignal
    );
    assert_eq!(
        a.sample_hold(v, foreign_p, cfg).unwrap_err().code(),
        DiagnosticCode::AuthoringForeignSignal
    );
    a.add_sample_hold_with_ports(
        NodeKey::from_u128(100),
        InPortKey::from_u128(100),
        InPortKey::from_u128(100),
        OutPortKey::from_u128(100),
        v,
        p,
        cfg,
        DiagnosticMeta::default(),
    )
    .unwrap();
    // Equal numeric keys in distinct Level/Pulse key domains are legal; actual same-kind collisions are not.
    for (node, vp, sp, op) in [
        (100, 101, 101, 101),
        (101, 100, 101, 101),
        (101, 101, 100, 101),
        (101, 101, 101, 100),
    ] {
        assert!(
            a.add_sample_hold_with_ports(
                NodeKey::from_u128(node),
                InPortKey::from_u128(vp),
                InPortKey::from_u128(sp),
                OutPortKey::from_u128(op),
                v,
                p,
                cfg,
                DiagnosticMeta::default()
            )
            .is_err()
        );
    }
    a.add_sample_hold(
        NodeKey::from_u128(101),
        v,
        p,
        cfg,
        DiagnosticMeta::default(),
    )
    .unwrap();
    assert!(a.finish().require_artifact().is_ok());
}
#[test]
fn grouped_sample_contributors_and_sampled_value_remain_explainable() {
    let mut b = NetworkBuilder::<Domain>::new(TimeDomainId::from_u128(2));
    let (value, v) = b.level_input("value");
    let (a, pa) = b.pulse_input("a");
    let (z, pz) = b.pulse_input("z");
    let p = b.merge([pz, pa]).unwrap();
    let node = NodeKey::from_u128(100);
    b.add_sample_hold(
        node,
        v,
        p,
        SampleHoldConfig::new(LOW),
        DiagnosticMeta::default(),
    )
    .unwrap();
    let c = compile(b);
    let mut m = c.spawn(policy());
    m.apply(Transaction::initialize(
        Time::from_ticks(9),
        m.revision(),
        c.input_snapshot()
            .set(value, HIGH)
            .unwrap()
            .pulse(a, PulseCount::new(2))
            .unwrap()
            .pulse(z, PulseCount::new(3))
            .unwrap()
            .finish()
            .unwrap(),
    ))
    .unwrap();
    let i = m.inspect_sample_hold(node).unwrap();
    match i.provenance().inspect(i.latest_establishment()).unwrap() {
        CauseInspection::PulseControlledLevel {
            contributions,
            result,
            ..
        } => {
            assert_eq!(result, HIGH);
            assert_eq!(contributions.len(), 1);
            assert_eq!(contributions[0].count(), PulseCount::new(5));
        }
        _ => panic!("expected grouped sample capture"),
    }
    let roots = facts(i.provenance(), i.latest_establishment());
    assert_eq!(roots.levels, BTreeSet::from([(value.as_u128(), HIGH)]));
    assert_eq!(
        roots.pulses,
        BTreeSet::from([(a.as_u128(), 2), (z.as_u128(), 3)])
    );
    assert_eq!(roots.times, BTreeSet::from([9]));
}
#[test]
fn bounded_capture_histories_match_an_independent_reference_recurrence() {
    for initial in [LOW, HIGH] {
        let f = fixture(initial);
        for history in 0..216 {
            let (mut m, _) = initialize(&f, initial, 0);
            let mut held = initial;
            let mut digits = history;
            for at in 6..9 {
                let choice = digits % 6;
                digits /= 6;
                let value = if choice % 2 == 0 { LOW } else { HIGH };
                let count = [0, 1, 3][choice / 2];
                let before = held;
                if count > 0 {
                    held = value;
                }
                let r = advance(&f, &mut m, at, value, count);
                assert_eq!(m.output_level(f.output), Some(held));
                assert_eq!(r.output_events().len(), usize::from(before != held));
                assert_eq!(m.inspect_sample_hold(f.node).unwrap().committed(), held);
            }
        }
    }
}

#[test]
fn composed_capture_histories_are_invariant_under_definition_and_input_permutations() {
    use mossignal::authored::UncheckedNetwork;
    // Bound: two SampleHolds separated by Not, all four initial state pairs,
    // all 8^3 three-reaction Level/sample-presence histories, and two orderings.
    for first_initial in [LOW, HIGH] {
        for second_initial in [LOW, HIGH] {
            let mut b = NetworkBuilder::<Domain>::new(TimeDomainId::from_u128(2));
            let (value, v) = b.level_input("value");
            let (first_sample, p) = b.pulse_input("first sample");
            let (second_sample, q) = b.pulse_input("second sample");
            let first = b
                .sample_hold(v, p, SampleHoldConfig::new(first_initial))
                .unwrap();
            let inverted = b.not(first).unwrap();
            let second = b
                .sample_hold(inverted, q, SampleHoldConfig::new(second_initial))
                .unwrap();
            let outputs = [
                b.level_output("first", first).unwrap(),
                b.level_output("second", second).unwrap(),
            ];
            let raw = b.into_unchecked();
            let compiled = [false, true].map(|reverse| {
                let mut nodes = raw.nodes().to_vec();
                let mut inputs = raw.external_inputs().to_vec();
                let mut outputs = raw.external_outputs().to_vec();
                let mut connections = raw.connections().to_vec();
                if reverse {
                    nodes.reverse();
                    inputs.reverse();
                    outputs.reverse();
                    connections.reverse();
                }
                UncheckedNetwork::new(
                    raw.key(),
                    raw.time_domain_id(),
                    raw.meta().clone(),
                    nodes,
                    inputs,
                    outputs,
                    connections,
                )
                .validate()
                .require_artifact()
                .unwrap()
                .compile()
                .require_artifact()
                .unwrap()
            });
            assert_eq!(compiled[0].fingerprint(), compiled[1].fingerprint());
            for history in 0..512 {
                let mut machines = compiled.each_ref().map(|c| {
                    let mut m = c.spawn(policy());
                    m.apply(Transaction::initialize(
                        Time::from_ticks(0),
                        m.revision(),
                        c.input_snapshot()
                            .set(value, LOW)
                            .unwrap()
                            .finish()
                            .unwrap(),
                    ))
                    .unwrap();
                    m
                });
                let mut held = [first_initial, second_initial];
                let mut digits = history;
                for at in 1..=3 {
                    let choice = digits % 8;
                    digits /= 8;
                    let v = if choice & 1 == 0 { LOW } else { HIGH };
                    let p = PulseCount::new(u64::from(choice & 2 != 0));
                    let q = PulseCount::new(u64::from(choice & 4 != 0));
                    let previous = held;
                    if p.is_positive() {
                        held[0] = v;
                    }
                    if q.is_positive() {
                        held[1] = if held[0] == LOW { HIGH } else { LOW };
                    }
                    let results = machines.iter_mut().enumerate().map(|(index, m)| {
                        let delta = compiled[index].input_delta();
                        let delta = if index == 0 {
                            delta.set(value, v).unwrap().pulse(first_sample, p).unwrap().pulse(second_sample, q).unwrap()
                        } else {
                            delta.pulse(second_sample, q).unwrap().pulse(first_sample, p).unwrap().set(value, v).unwrap()
                        };
                        let result = m.apply(Transaction::advance(Time::from_ticks(at), m.revision(), delta.finish().unwrap())).unwrap();
                        assert_eq!(outputs.map(|o| m.output_level(o).unwrap()), held);
                        let expected = outputs.into_iter().zip(previous).zip(held)
                            .filter_map(|((output, from), to)| (from != to).then_some((output, from, to)))
                            .collect::<Vec<_>>();
                        let actual = result.output_events().iter().map(|event| match event {
                            OutputEvent::LevelChanged { output, from, to, .. } => (*output, *from, *to),
                            _ => panic!("composed history must publish only actual Level transitions"),
                        }).collect::<Vec<_>>();
                        assert_eq!(actual, expected);
                        result
                    }).collect::<Vec<_>>();
                    // Equal stable topology and inputs also preserve causal meaning and event identity.
                    let event_facts = |result: &TransactionResult<Domain>| {
                        result
                            .output_events()
                            .iter()
                            .map(|event| match event {
                                OutputEvent::LevelChanged {
                                    output,
                                    from,
                                    to,
                                    stamp: at,
                                    revision,
                                    cause,
                                } => (*output, *from, *to, at.time().ticks(), *revision, *cause),
                                _ => panic!(
                                    "composed history must publish only actual Level transitions"
                                ),
                            })
                            .collect::<Vec<_>>()
                    };
                    assert_eq!(event_facts(&results[0]), event_facts(&results[1]));
                }
            }
        }
    }
}

#[test]
fn mixed_stored_level_nodes_keep_independent_state_across_captures_and_retention() {
    use mossignal::{ConflictPolicy, LevelSetResetConfig, PulseSetResetConfig};
    let mut b = NetworkBuilder::new(TimeDomainId::from_u128(1));
    let (value, v) = b.level_input("value");
    let (sample, p) = b.pulse_input("sample");
    let (reset, r) = b.pulse_input("reset");
    let inverted = b.not(v).unwrap();
    let toggled = b.toggle(p, ToggleConfig::new(LOW)).unwrap();
    let pulse_latched = b
        .pulse_set_reset_latch(
            p,
            r,
            PulseSetResetConfig::new(LOW, ConflictPolicy::ResetDominant),
        )
        .unwrap();
    let level_latched = b
        .level_set_reset_latch(
            v,
            inverted,
            LevelSetResetConfig::new(HIGH, ConflictPolicy::SetDominant),
        )
        .unwrap();
    let held = b.sample_hold(v, p, SampleHoldConfig::new(HIGH)).unwrap();
    let outputs = [
        b.level_output("toggle", toggled).unwrap(),
        b.level_output("pulse latch", pulse_latched).unwrap(),
        b.level_output("level latch", level_latched).unwrap(),
        b.level_output("held", held).unwrap(),
    ];
    let c = compile(b);
    let mut m = c.spawn(policy());
    m.apply(Transaction::initialize(
        Time::from_ticks(0),
        m.revision(),
        c.input_snapshot()
            .set(value, LOW)
            .unwrap()
            .finish()
            .unwrap(),
    ))
    .unwrap();
    assert_eq!(
        outputs.map(|o| m.output_level(o).unwrap()),
        [LOW, LOW, LOW, HIGH]
    );
    for (at, v, p, r, expected) in [
        (1, HIGH, 2, 0, [LOW, HIGH, HIGH, HIGH]),
        (2, LOW, 0, 1, [LOW, LOW, LOW, HIGH]),
        (3, LOW, 1, 0, [HIGH, HIGH, LOW, LOW]),
        (4, HIGH, 0, 0, [HIGH, HIGH, HIGH, LOW]),
    ] {
        m.apply(Transaction::advance(
            Time::from_ticks(at),
            m.revision(),
            c.input_delta()
                .set(value, v)
                .unwrap()
                .pulse(sample, PulseCount::new(p))
                .unwrap()
                .pulse(reset, PulseCount::new(r))
                .unwrap()
                .finish()
                .unwrap(),
        ))
        .unwrap();
        assert_eq!(outputs.map(|o| m.output_level(o).unwrap()), expected);
    }
}

fn leaf_module(initial: LogicLevel, raw: bool) -> mossignal::ModuleDef<Domain> {
    use mossignal::authored::{
        InputPortRole, ModuleInputDef, ModuleInterfaceMapping, ModuleOutputDef, NodeDef, NodeKind,
        NodePorts, UncheckedModule,
    };
    let vi = ModuleInputKey::<Level>::from_u128(1);
    let si = ModuleInputKey::<Pulse>::from_u128(2);
    let out = ModuleOutputKey::<Level>::from_u128(40);
    let vp = InPortKey::<Level>::from_u128(20);
    let sp = InPortKey::<Pulse>::from_u128(21);
    let op = OutPortKey::<Level>::from_u128(30);
    if raw {
        return UncheckedModule::new_user(
            DiagnosticMeta::default(),
            vec![
                ModuleInputDef::new(si.into(), DiagnosticMeta::default()),
                ModuleInputDef::new(vi.into(), DiagnosticMeta::default()),
            ],
            vec![ModuleOutputDef::new(out.into(), DiagnosticMeta::default())],
            vec![
                ModuleInterfaceMapping::output(out.into(), op.into()),
                ModuleInterfaceMapping::input(si.into(), sp.into()),
                ModuleInterfaceMapping::input(vi.into(), vp.into()),
            ],
            vec![NodeDef::new(
                NodeKey::from_u128(10),
                NodeKind::sample_hold(SampleHoldConfig::new(initial)),
                NodePorts::with_input_roles(
                    vec![sp.into(), vp.into()],
                    vec![InputPortRole::Sample, InputPortRole::Value],
                    vec![op.into()],
                ),
                DiagnosticMeta::default(),
            )],
            vec![],
        )
        .validate()
        .require_artifact()
        .unwrap();
    }
    let mut b = mossignal::ModuleBuilder::new();
    let v = b.add_level_input(vi, DiagnosticMeta::default()).unwrap();
    let p = b.add_pulse_input(si, DiagnosticMeta::default()).unwrap();
    let held = b
        .add_sample_hold_with_ports(
            NodeKey::from_u128(10),
            vp,
            sp,
            op,
            v,
            p,
            SampleHoldConfig::new(initial),
            DiagnosticMeta::default(),
        )
        .unwrap()
        .into_outputs();
    b.add_level_output(out, held, DiagnosticMeta::default())
        .unwrap();
    b.finish().require_artifact().unwrap()
}

#[test]
fn module_authoring_preserves_foreign_signal_and_duplicate_key_failures() {
    let mut a = mossignal::ModuleBuilder::<Domain>::new();
    let (_, v) = a.level_input("local value");
    let (_, p) = a.pulse_input("local sample");
    let mut foreign = mossignal::ModuleBuilder::<Domain>::new();
    let (_, foreign_v) = foreign.level_input("foreign value");
    let (_, foreign_p) = foreign.pulse_input("foreign sample");
    let config = SampleHoldConfig::new(LOW);
    for (v, p) in [(foreign_v, p), (v, foreign_p)] {
        assert_eq!(
            a.sample_hold(v, p, config).unwrap_err().code(),
            DiagnosticCode::AuthoringForeignSignal
        );
        assert_eq!(
            a.add_sample_hold(
                NodeKey::from_u128(100),
                v,
                p,
                config,
                DiagnosticMeta::default()
            )
            .unwrap_err()
            .code(),
            DiagnosticCode::AuthoringForeignSignal
        );
        assert_eq!(
            a.add_sample_hold_with_ports(
                NodeKey::from_u128(100),
                InPortKey::from_u128(20),
                InPortKey::from_u128(21),
                OutPortKey::from_u128(30),
                v,
                p,
                config,
                DiagnosticMeta::default()
            )
            .unwrap_err()
            .code(),
            DiagnosticCode::AuthoringForeignSignal
        );
    }
    let first = a
        .add_sample_hold(
            NodeKey::from_u128(100),
            v,
            p,
            config,
            DiagnosticMeta::default(),
        )
        .unwrap()
        .into_outputs();
    assert_eq!(
        a.add_sample_hold(
            NodeKey::from_u128(100),
            v,
            p,
            config,
            DiagnosticMeta::default()
        )
        .unwrap_err()
        .code(),
        DiagnosticCode::ValidationDuplicateKey
    );
    let second = a.sample_hold(first, p, config).unwrap();
    a.level_output("held", second).unwrap();
    assert!(a.finish().require_artifact().is_ok());
}

#[test]
fn modules_preserve_independent_qualified_state_and_owned_capture_evidence() {
    let leaf = leaf_module(LOW, false);
    assert_eq!(leaf.fingerprint(), leaf_module(LOW, true).fingerprint());
    assert_ne!(leaf.fingerprint(), leaf_module(HIGH, false).fingerprint());
    let mut parent = mossignal::ModuleBuilder::new();
    let v = parent
        .add_level_input(ModuleInputKey::from_u128(1), DiagnosticMeta::default())
        .unwrap();
    let p = parent
        .add_pulse_input(ModuleInputKey::from_u128(2), DiagnosticMeta::default())
        .unwrap();
    let instance = parent
        .instantiate(
            &leaf,
            ModuleInstanceKey::from_u128(50),
            DiagnosticMeta::default(),
        )
        .unwrap()
        .bind_level(ModuleInputKey::from_u128(1), v)
        .unwrap()
        .bind_pulse(ModuleInputKey::from_u128(2), p)
        .unwrap()
        .finish()
        .unwrap();
    parent
        .add_level_output(
            ModuleOutputKey::from_u128(40),
            instance
                .level_output(ModuleOutputKey::from_u128(40))
                .unwrap(),
            DiagnosticMeta::default(),
        )
        .unwrap();
    let parent = parent.finish().require_artifact().unwrap();
    let run = |reverse: bool| {
        let mut b = NetworkBuilder::<Domain>::with_key(
            NetworkKey::from_u128(80),
            TimeDomainId::from_u128(2),
        );
        let (value, v) = b.level_input("value");
        let (first, p) = b.pulse_input("first");
        let (second, q) = b.pulse_input("second");
        for id in if reverse { [200, 100] } else { [100, 200] } {
            let instance = b
                .instantiate(
                    &parent,
                    ModuleInstanceKey::from_u128(id),
                    DiagnosticMeta::default(),
                )
                .unwrap()
                .bind_level(ModuleInputKey::from_u128(1), v)
                .unwrap()
                .bind_pulse(ModuleInputKey::from_u128(2), if id == 100 { p } else { q })
                .unwrap()
                .finish()
                .unwrap();
            b.add_level_output(
                ExternalOutputKey::from_u128(id),
                instance
                    .level_output(ModuleOutputKey::from_u128(40))
                    .unwrap(),
                DiagnosticMeta::default(),
            )
            .unwrap();
        }
        let direct = b
            .add_sample_hold(
                NodeKey::from_u128(10),
                v,
                p,
                SampleHoldConfig::new(LOW),
                DiagnosticMeta::default(),
            )
            .unwrap()
            .into_outputs();
        b.add_level_output(
            ExternalOutputKey::from_u128(300),
            direct,
            DiagnosticMeta::default(),
        )
        .unwrap();
        let c = compile(b);
        let mut m = c.spawn(policy());
        let module =
            mossignal::QualifiedModuleRef::from_instances(vec![ModuleInstanceKey::from_u128(100)])
                .unwrap();
        assert!(matches!(
            m.inspect_qualified_module(module.clone()),
            Err(mossignal::ModuleInspectionFailure::NotInitialized)
        ));
        m.apply(Transaction::initialize(
            Time::from_ticks(0),
            m.revision(),
            c.input_snapshot()
                .set(value, LOW)
                .unwrap()
                .finish()
                .unwrap(),
        ))
        .unwrap();
        m.apply(Transaction::advance(
            Time::from_ticks(1),
            m.revision(),
            c.input_delta()
                .set(value, HIGH)
                .unwrap()
                .pulse(first, PulseCount::ONE)
                .unwrap()
                .finish()
                .unwrap(),
        ))
        .unwrap();
        let old = m
            .inspect_qualified_module(module.clone())
            .unwrap()
            .nodes()
            .iter()
            .find_map(|n| n.sample_hold().cloned())
            .unwrap();
        let NodeSubject::Qualified(owner) = old.node() else {
            panic!("module node must be qualified");
        };
        assert_eq!(
            owner.instances(),
            &[
                ModuleInstanceKey::from_u128(100),
                ModuleInstanceKey::from_u128(50)
            ]
        );
        assert_eq!(owner.node(), NodeKey::from_u128(10));
        match old
            .provenance()
            .inspect(old.latest_establishment())
            .unwrap()
        {
            CauseInspection::PulseControlledLevel {
                subject,
                contributions,
                ..
            } => {
                assert_eq!(
                    subject,
                    mossignal::ProvenanceSubject::QualifiedNode(owner.clone())
                );
                let [contribution] = contributions else {
                    panic!("capture must identify its sample port");
                };
                assert_eq!(contribution.count(), PulseCount::ONE);
                let mossignal::PulsePortSubject::Qualified(port) = contribution.port() else {
                    panic!("module sample port must be qualified");
                };
                assert_eq!(port.instances(), owner.instances());
                assert_eq!(port.port(), InPortKey::<Pulse>::from_u128(21).into());
            }
            _ => panic!("capture must identify its qualified node and sample contribution"),
        }
        assert_eq!(old.committed(), HIGH);
        assert_eq!(establishment_time(&old), 1);
        assert_eq!(m.output_level(ExternalOutputKey::from_u128(200)), Some(LOW));
        for (at, new_value, count) in [(2, LOW, 1), (3, HIGH, 2)] {
            m.apply(Transaction::advance(
                Time::from_ticks(at),
                m.revision(),
                c.input_delta()
                    .set(value, new_value)
                    .unwrap()
                    .pulse(second, PulseCount::new(count))
                    .unwrap()
                    .finish()
                    .unwrap(),
            ))
            .unwrap();
            assert_eq!(
                m.output_level(ExternalOutputKey::from_u128(100)),
                Some(HIGH)
            );
            assert_eq!(
                m.output_level(ExternalOutputKey::from_u128(300)),
                Some(HIGH)
            );
        }
        let first_now = m.inspect_qualified_module(module.clone()).unwrap();
        let first_now = first_now
            .nodes()
            .iter()
            .find_map(|n| n.sample_hold())
            .unwrap();
        assert_eq!(establishment_time(first_now), 1);
        assert_eq!(establishment_time(&old), 1);
        let second_module =
            mossignal::QualifiedModuleRef::from_instances(vec![ModuleInstanceKey::from_u128(200)])
                .unwrap();
        let second_now = m.inspect_qualified_module(second_module).unwrap();
        let second_now = second_now
            .nodes()
            .iter()
            .find_map(|n| n.sample_hold())
            .unwrap();
        assert_eq!(
            (second_now.committed(), establishment_time(second_now)),
            (HIGH, 3)
        );
        (
            c.fingerprint(),
            first_now.node().clone(),
            second_now.node().clone(),
        )
    };
    assert_eq!(run(false), run(true));
}
#[test]
fn delayed_samples_capture_at_each_actual_reaction_and_exact_target_batch() {
    let mut b = NetworkBuilder::<Domain>::new(TimeDomainId::from_u128(2));
    let (input, p) = b.pulse_input("schedule");
    let (value, v) = b.level_input("current value override");
    let a = b
        .pulse_delay(
            p,
            mossignal::PulseDelayConfig::new(NonZeroSpan::from_ticks(2).unwrap()),
        )
        .unwrap();
    let z = b
        .pulse_delay(
            p,
            mossignal::PulseDelayConfig::new(NonZeroSpan::from_ticks(4).unwrap()),
        )
        .unwrap();
    let pulses = b.merge([z, a]).unwrap();
    let toggled = b.toggle(pulses, ToggleConfig::new(LOW)).unwrap();
    let current = b.any([v, toggled]).unwrap();
    let node = NodeKey::from_u128(100);
    let held = b
        .add_sample_hold(
            node,
            current,
            pulses,
            SampleHoldConfig::new(LOW),
            DiagnosticMeta::default(),
        )
        .unwrap()
        .into_outputs();
    let output = b.level_output("held", held).unwrap();
    let c = compile(b);
    let changes = |r: &TransactionResult<Domain>| {
        r.output_events()
            .iter()
            .map(|e| match e {
                OutputEvent::LevelChanged {
                    stamp: at,
                    from,
                    to,
                    ..
                } => (at.time().ticks(), *from, *to),
                _ => panic!("only changed Levels expected"),
            })
            .collect::<Vec<_>>()
    };
    for target in [4, 5] {
        let mut direct = c.spawn(policy());
        let mut step = c.spawn(policy());
        for m in [&mut direct, &mut step] {
            m.apply(Transaction::initialize(
                Time::from_ticks(0),
                m.revision(),
                c.input_snapshot()
                    .set(value, LOW)
                    .unwrap()
                    .pulse(input, PulseCount::ONE)
                    .unwrap()
                    .finish()
                    .unwrap(),
            ))
            .unwrap();
        }
        let result = direct
            .apply(Transaction::advance(
                Time::from_ticks(target),
                direct.revision(),
                c.input_delta().set(value, HIGH).unwrap().finish().unwrap(),
            ))
            .unwrap();
        let mut stepped = Vec::new();
        for at in if target == 4 {
            vec![2, 4]
        } else {
            vec![2, 4, 5]
        } {
            let mut delta = c.input_delta();
            if at == target {
                delta = delta.set(value, HIGH).unwrap();
            }
            let r = step
                .apply(Transaction::advance(
                    Time::from_ticks(at),
                    step.revision(),
                    delta.finish().unwrap(),
                ))
                .unwrap();
            stepped.extend(changes(&r));
        }
        assert_eq!(changes(&result), stepped);
        assert_eq!(
            changes(&result),
            if target == 4 {
                vec![(2, LOW, HIGH)]
            } else {
                vec![(2, LOW, HIGH), (4, HIGH, LOW)]
            }
        );
        assert_eq!(direct.output_level(output), step.output_level(output));
        assert_eq!(direct.schedule(), step.schedule());
        let observation = direct.inspect_sample_hold(node).unwrap();
        let step_observation = step.inspect_sample_hold(node).unwrap();
        assert_eq!(establishment_time(&observation), 4);
        assert_eq!(
            facts(observation.provenance(), observation.latest_establishment()),
            facts(
                step_observation.provenance(),
                step_observation.latest_establishment()
            )
        );
    }
}
