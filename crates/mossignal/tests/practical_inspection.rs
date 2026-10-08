use mossignal::authored::{
    ConnectionDef, ConnectionEndpoint, ExternalInputDef, ExternalOutputDef, NodeDef, NodeKind,
    NodePorts, UncheckedNetwork,
};
use mossignal::key::*;
use mossignal::metadata::DiagnosticMeta;
use mossignal::signal::{Level, LogicLevel, Pulse, PulseCount};
use mossignal::time::{NonZeroSpan, Time};
use mossignal::*;

fn meta() -> DiagnosticMeta {
    DiagnosticMeta::default()
}
fn policy() -> RuntimePolicy {
    RuntimePolicy::builder()
        .max_internal_reactions(100)
        .max_evaluated_operations(10_000)
        .max_pending_events(100)
        .max_events_created_per_transaction(100)
        .max_required_provenance_growth(10_000)
        .build()
        .unwrap()
}
fn builder() -> NetworkBuilder<()> {
    NetworkBuilder::with_key(NetworkKey::from_u128(1), TimeDomainId::from_u128(2))
}
fn compiled(builder: NetworkBuilder<()>) -> CompiledNetwork<()> {
    builder
        .finish()
        .require_artifact()
        .unwrap()
        .compile()
        .require_artifact()
        .unwrap()
}
fn subject(element: GraphElement) -> GraphSubjectRef {
    GraphSubjectRef::root(element)
}
fn inputs_in(explanation: &CausalExplanation<()>) -> Vec<ExternalInputKey<Level>> {
    let mut inputs = explanation
        .edges
        .iter()
        .filter_map(
            |edge| match explanation.provenance().inspect(edge.cause).unwrap() {
                CauseInspection::ExternalObservation { input, .. } => Some(input),
                _ => None,
            },
        )
        .collect::<Vec<_>>();
    inputs.sort();
    inputs.dedup();
    inputs
}

#[test]
fn unchanged_output_updates_current_support_without_new_transition_and_reads_are_pure() {
    let mut b = builder();
    let a = ExternalInputKey::from_u128(10);
    let c = ExternalInputKey::from_u128(11);
    let output = ExternalOutputKey::from_u128(20);
    let node = NodeKey::from_u128(30);
    let sa = b.add_level_input(a, meta()).unwrap();
    let sc = b.add_level_input(c, meta()).unwrap();
    let signal = b
        .add_any_with_ports(
            node,
            OutPortKey::from_u128(31),
            [
                (InPortKey::from_u128(32), sa),
                (InPortKey::from_u128(33), sc),
            ],
            meta(),
        )
        .unwrap()
        .into_outputs();
    b.add_level_output(output, signal, meta()).unwrap();
    let compiled = compiled(b);
    let mut m = compiled.spawn(policy());
    assert!(matches!(
        m.inspect_node(node),
        Err(InspectionFailure::NotInitialized)
    ));
    assert_eq!(
        compiled
            .inspect_node_definition(NodeSubject::Node(node))
            .unwrap()
            .key(),
        node
    );
    let snapshot = compiled
        .input_snapshot()
        .set(a, LogicLevel::High)
        .unwrap()
        .set(c, LogicLevel::Low)
        .unwrap()
        .finish()
        .unwrap();
    let first = m
        .apply(Transaction::initialize(
            Time::from_ticks(0),
            m.revision(),
            snapshot,
        ))
        .unwrap();
    let first_explanation = m.explain(Explain::CurrentOutput(output.into())).unwrap();
    let old_transition = first_explanation.latest_transition.unwrap();
    assert_eq!(inputs_in(&first_explanation.causal), vec![a]);
    let delta = compiled
        .input_delta()
        .set(a, LogicLevel::Low)
        .unwrap()
        .set(c, LogicLevel::High)
        .unwrap()
        .finish()
        .unwrap();
    let second = m
        .apply(Transaction::advance(
            Time::from_ticks(1),
            m.revision(),
            delta,
        ))
        .unwrap();
    assert!(second.output_events().is_empty());
    let before = (m.execution_state_digest(), m.observable_state_digest());
    let explanation = m.explain(Explain::CurrentOutput(output.into())).unwrap();
    assert!(inputs_in(&explanation.causal).contains(&c));
    // The causal closure also includes the old transition; inspect current support alone.
    let historical = explanation
        .causal
        .provenance()
        .explain_cause(explanation.latest_transition.unwrap())
        .unwrap();
    assert_eq!(inputs_in(&historical), vec![a]);
    let current = explanation
        .causal
        .provenance()
        .explain_cause(explanation.current_support[0])
        .unwrap();
    assert_eq!(inputs_in(&current), vec![c]);
    let inspected = m.inspect_node(node).unwrap();
    assert_eq!(inspected.inputs.len(), 2);
    assert_eq!(inspected.outputs[0].level, Some(LogicLevel::High));
    let _ = m.compiled().regions();
    m.compiled().slice_affecting(output.into()).unwrap();
    assert_eq!(
        before,
        (m.execution_state_digest(), m.observable_state_digest())
    );
    assert!(
        first_explanation
            .causal
            .provenance()
            .inspect(old_transition)
            .is_ok()
    );
    let event = first.explain_output_event(0).unwrap();
    assert!(matches!(
        event.observed,
        ExplainedObservation::OutputEvent {
            value: OutputEventValue::Established(LogicLevel::High),
            ..
        }
    ));
    assert!(matches!(
        first.explain_output_event(usize::MAX),
        Err(InspectionFailure::UnknownOutputEvent(_))
    ));
}

#[test]
fn temporal_paths_pending_payloads_pulse_history_and_owned_ancestry() {
    let mut b = builder();
    let input = ExternalInputKey::<Pulse>::from_u128(3);
    let output = ExternalOutputKey::<Pulse>::from_u128(4);
    let node = NodeKey::from_u128(5);
    let signal = b.add_pulse_input(input, meta()).unwrap();
    let delayed = b
        .add_pulse_delay(
            node,
            signal,
            PulseDelayConfig::new(NonZeroSpan::from_ticks(3).unwrap()),
            meta(),
        )
        .unwrap()
        .into_outputs();
    b.add_pulse_output(output, delayed, meta()).unwrap();
    let compiled = compiled(b);
    let slice = compiled.slice_affecting(output.into()).unwrap();
    assert!(
        slice
            .subjects()
            .contains(&subject(GraphElement::ExternalInput(input.into())))
    );
    assert!(
        slice
            .subjects()
            .contains(&subject(GraphElement::Node(node)))
    );
    let mut m = compiled.spawn(policy());
    let snapshot = compiled
        .input_snapshot()
        .pulse(input, PulseCount::new(7))
        .unwrap()
        .finish()
        .unwrap();
    m.apply(Transaction::initialize(
        Time::from_ticks(0),
        m.revision(),
        snapshot,
    ))
    .unwrap();
    let pending = m.inspect_pending_events().unwrap().remove(0);
    assert_eq!(pending.deadline.ticks(), 3);
    assert!(matches!(pending.payload, PendingPayload::Pulse(count) if count == PulseCount::new(7)));
    let explanation = m.explain(Explain::Pending(pending.event)).unwrap();
    assert!(!explanation.causal.edges.is_empty());
    let node_view = m.inspect_node(node).unwrap();
    assert!(node_view.inputs[0].level.is_none());
    assert!(node_view.outputs[0].level.is_none());
    assert!(matches!(
        m.explain(Explain::CurrentOutput(output.into())),
        Err(InspectionFailure::NoCurrentPulse(_))
    ));
    let result = m
        .apply(Transaction::advance(
            Time::from_ticks(3),
            m.revision(),
            compiled.input_delta().finish().unwrap(),
        ))
        .unwrap();
    assert!(matches!(
        m.inspect_pending(pending.event),
        Err(InspectionFailure::UnknownPending(_))
    ));
    assert!(pending.provenance().inspect(pending.cause).is_ok());
    assert!(
        matches!(result.explain_output_event(0).unwrap().observed, ExplainedObservation::OutputEvent { value: OutputEventValue::Pulsed(count), .. } if count == PulseCount::new(7))
    );
}

#[test]
fn structural_cycle_and_node_free_regions_are_deterministic() {
    let a = NodeKey::from_u128(10);
    let z = NodeKey::from_u128(20);
    let ai = InPortKey::<Level>::from_u128(11);
    let ao = OutPortKey::<Level>::from_u128(12);
    let zi = InPortKey::<Level>::from_u128(21);
    let zo = OutPortKey::<Level>::from_u128(22);
    let input = ExternalInputKey::<Level>::from_u128(30);
    let output = ExternalOutputKey::<Level>::from_u128(31);
    let make = |reverse: bool| {
        let mut nodes = vec![
            NodeDef::new(
                a,
                NodeKind::transport_delay(
                    NonZeroSpan::<()>::from_ticks(2).unwrap(),
                    LogicLevel::Low,
                ),
                NodePorts::with_input_roles(
                    vec![ai.into()],
                    vec![mossignal::authored::InputPortRole::TransportDelay],
                    vec![ao.into()],
                ),
                meta(),
            ),
            NodeDef::new(
                z,
                NodeKind::not(),
                NodePorts::new(vec![zi.into()], vec![zo.into()]),
                meta(),
            ),
        ];
        let mut connections = vec![
            ConnectionDef::new(
                ConnectionKey::from_u128(40),
                ConnectionEndpoint::NodeOutput(ao.into()),
                ConnectionEndpoint::NodeInput(zi.into()),
                meta(),
            ),
            ConnectionDef::new(
                ConnectionKey::from_u128(41),
                ConnectionEndpoint::NodeOutput(zo.into()),
                ConnectionEndpoint::NodeInput(ai.into()),
                meta(),
            ),
        ];
        if reverse {
            nodes.reverse();
            connections.reverse();
        }
        UncheckedNetwork::new(
            NetworkKey::from_u128(1),
            TimeDomainId::from_u128(2),
            meta(),
            nodes,
            vec![ExternalInputDef::new(input.into(), meta())],
            vec![ExternalOutputDef::new(
                output.into(),
                SignalSourceKey::ExternalInput(input).into(),
                meta(),
            )],
            connections,
        )
        .validate()
        .require_artifact()
        .unwrap()
        .compile()
        .require_artifact()
        .unwrap()
    };
    let left = make(false);
    let right = make(true);
    assert_eq!(left.regions(), right.regions());
    assert_eq!(left.regions().len(), 2);
    let wire = left.region_containing_output(output.into()).unwrap();
    assert_eq!(wire.subjects().len(), 2);
    let between = left
        .slice_between(
            &subject(GraphElement::Node(a)),
            &subject(GraphElement::Node(z)),
        )
        .unwrap();
    assert!(between.subjects().contains(&subject(GraphElement::Node(z))));
    assert!(
        left.slice_between(
            &subject(GraphElement::ExternalInput(input.into())),
            &subject(GraphElement::Node(z))
        )
        .unwrap()
        .subjects()
        .is_empty()
    );
    assert!(left.region_containing(NodeKey::from_u128(999)).is_err());
    assert_eq!(
        left.slice_downstream(&subject(GraphElement::Node(a)))
            .unwrap(),
        right
            .slice_downstream(&subject(GraphElement::Node(a)))
            .unwrap()
    );
}

#[test]
fn qualified_modules_expose_public_law_and_primitive_support() {
    let mut module = ModuleBuilder::<()>::new();
    let module_input = ModuleInputKey::from_u128(10);
    let module_output = ModuleOutputKey::from_u128(11);
    let signal = module.add_level_input(module_input, meta()).unwrap();
    let signal = module.not(signal).unwrap();
    module
        .add_level_output(module_output, signal, meta())
        .unwrap();
    let module = module.finish().require_artifact().unwrap();
    let mut outer = ModuleBuilder::<()>::new();
    let outer_input = ModuleInputKey::from_u128(20);
    let outer_output = ModuleOutputKey::from_u128(21);
    let signal = outer.add_level_input(outer_input, meta()).unwrap();
    let nested = ModuleInstanceKey::from_u128(22);
    let added = outer
        .instantiate(&module, nested, meta())
        .unwrap()
        .bind_level(module_input, signal)
        .unwrap()
        .finish()
        .unwrap();
    outer
        .add_level_output(
            outer_output,
            added.level_output(module_output).unwrap(),
            meta(),
        )
        .unwrap();
    let outer = outer.finish().require_artifact().unwrap();
    let mut b = builder();
    let input = ExternalInputKey::from_u128(30);
    let output = ExternalOutputKey::from_u128(31);
    let signal = b.add_level_input(input, meta()).unwrap();
    let instance = ModuleInstanceKey::from_u128(32);
    let added = b
        .instantiate(&outer, instance, meta())
        .unwrap()
        .bind_level(outer_input, signal)
        .unwrap()
        .finish()
        .unwrap();
    b.add_level_output(output, added.level_output(outer_output).unwrap(), meta())
        .unwrap();
    let compiled = compiled(b);
    let qualified = compiled.graph().qualified_nodes()[0].clone();
    assert_eq!(qualified.instances(), &[instance, nested]);
    let slice = compiled.slice_affecting(output.into()).unwrap();
    assert!(
        slice
            .subjects()
            .contains(&subject(GraphElement::Module(instance)))
    );
    assert!(slice.subjects().contains(&GraphSubjectRef::qualified(
        qualified.instances().to_vec(),
        GraphElement::Node(qualified.node())
    )));
    let mut m = compiled.spawn(policy());
    m.apply(Transaction::initialize(
        Time::from_ticks(0),
        m.revision(),
        compiled
            .input_snapshot()
            .set(input, LogicLevel::Low)
            .unwrap()
            .finish()
            .unwrap(),
    ))
    .unwrap();
    let inspection = m.inspect_qualified_node(qualified.clone()).unwrap();
    assert_eq!(inspection.subject, NodeSubject::Qualified(qualified));
    assert_eq!(inspection.outputs[0].level, Some(LogicLevel::High));
    let explanation = m.explain(Explain::CurrentModule(instance)).unwrap();
    let ExplainedObservation::Module(module) = &explanation.observed else {
        panic!("module law must be primary");
    };
    assert_eq!(module.inputs()[0].level(), Some(LogicLevel::Low));
    assert_eq!(module.nodes().len(), 1);
    assert_eq!(inputs_in(&explanation.causal), vec![input]);
}

#[test]
fn typed_owned_cause_observations_retain_their_view() {
    let mut b = builder();
    let input = ExternalInputKey::from_u128(3);
    let signal = b.add_level_input(input, meta()).unwrap();
    let node = NodeKey::from_u128(4);
    b.add_rising_edge(
        node,
        signal,
        EdgeConfig::new(EdgeInitialization::Baseline),
        meta(),
    )
    .unwrap();
    let compiled = compiled(b);
    let mut m = compiled.spawn(policy());
    m.apply(Transaction::initialize(
        Time::from_ticks(0),
        m.revision(),
        compiled
            .input_snapshot()
            .set(input, LogicLevel::Low)
            .unwrap()
            .finish()
            .unwrap(),
    ))
    .unwrap();
    let inspection = m.inspect_edge_detector(node).unwrap();
    let cause = inspection.observation_cause();
    m.apply(Transaction::advance(
        Time::from_ticks(1),
        m.revision(),
        compiled
            .input_delta()
            .set(input, LogicLevel::High)
            .unwrap()
            .finish()
            .unwrap(),
    ))
    .unwrap();
    assert!(inspection.provenance().inspect(cause).is_ok());
}

#[test]
fn standard_public_law_and_exact_descriptor_precede_primitive_support() {
    let mut b = builder();
    let a = ExternalInputKey::from_u128(10);
    let c = ExternalInputKey::from_u128(11);
    let sa = b.add_level_input(a, meta()).unwrap();
    let sc = b.add_level_input(c, meta()).unwrap();
    let instance = ModuleInstanceKey::from_u128(20);
    let ma = ModuleInputKey::from_u128(30);
    let mc = ModuleInputKey::from_u128(31);
    b.add_exactly(
        instance,
        1,
        [
            KeyedModuleInput {
                key: ma,
                source: sa,
            },
            KeyedModuleInput {
                key: mc,
                source: sc,
            },
        ],
        meta(),
    )
    .unwrap();
    let compiled = compiled(b);
    let mut m = compiled.spawn(policy());
    m.apply(Transaction::initialize(
        Time::from_ticks(0),
        m.revision(),
        compiled
            .input_snapshot()
            .set(a, LogicLevel::High)
            .unwrap()
            .set(c, LogicLevel::Low)
            .unwrap()
            .finish()
            .unwrap(),
    ))
    .unwrap();
    let explanation = m.explain(Explain::CurrentModule(instance)).unwrap();
    let ExplainedObservation::Module(module) = &explanation.observed else {
        panic!("module observation required");
    };
    assert_eq!(
        module.standard_declaration().unwrap().module_ref(),
        &StandardModuleRef::exactly()
    );
    assert!(
        matches!(module.public_behavior(), ModuleBehavior::Exactly(ExactlyExplanation::Matched { high_contributors, low_non_contributors }) if high_contributors == vec![ma] && low_non_contributors == vec![mc])
    );
    assert!(!module.nodes().is_empty());
    assert_eq!(inputs_in(&explanation.causal), vec![a, c]);
    for edge in &explanation.causal.edges {
        assert!(explanation.causal.provenance().inspect(edge.cause).is_ok());
    }
}

#[test]
fn pulse_explanation_preserves_joint_counts_and_port_identity_under_permutation() {
    let a = ExternalInputKey::<Pulse>::from_u128(10);
    let c = ExternalInputKey::<Pulse>::from_u128(11);
    let node = NodeKey::from_u128(20);
    let pa = InPortKey::from_u128(30);
    let pc = InPortKey::from_u128(31);
    let po = OutPortKey::from_u128(32);
    let observe = |reverse: bool| {
        let mut b = builder();
        let sa = b.add_pulse_input(a, meta()).unwrap();
        let sc = b.add_pulse_input(c, meta()).unwrap();
        let mut ports = vec![(pa, sa), (pc, sc)];
        if reverse {
            ports.reverse();
        }
        b.add_merge_with_ports(node, po, ports, meta()).unwrap();
        let compiled = compiled(b);
        let mut m = compiled.spawn(policy());
        m.apply(Transaction::initialize(
            Time::from_ticks(0),
            m.revision(),
            compiled
                .input_snapshot()
                .pulse(a, PulseCount::new(2))
                .unwrap()
                .pulse(c, PulseCount::new(5))
                .unwrap()
                .finish()
                .unwrap(),
        ))
        .unwrap();
        let explanation = m.explain(Explain::CurrentNode(node)).unwrap();
        assert!(explanation.current_support.is_empty());
        let ExplainedObservation::Node(inspected) = &explanation.observed else {
            panic!("node observation required");
        };
        assert!(inspected.outputs[0].level.is_none());
        let CauseInspection::PulseDerived {
            contributions,
            result,
            ..
        } = explanation
            .causal
            .provenance()
            .inspect(inspected.last_reaction_cause)
            .unwrap()
        else {
            panic!("pulse derivation required");
        };
        assert_eq!(result, PulseCount::new(7));
        let mut counts = contributions
            .iter()
            .map(|value| (value.port().clone(), value.count()))
            .collect::<Vec<_>>();
        counts.sort();
        // Cause references are scoped to each artifact. Compare semantic roots,
        // retaining the single joint derivation rather than comparing arena identity.
        let mut observations = Vec::new();
        let mut initializations = 0;
        for edge in &explanation.causal.edges {
            match explanation.causal.provenance().inspect(edge.cause).unwrap() {
                CauseInspection::ExternalPulseObservation { input, count } => {
                    observations.push((input, count))
                }
                CauseInspection::InitializationTransaction { .. } => initializations += 1,
                CauseInspection::PulseDerived { supporters, .. } => assert_eq!(supporters.len(), 3),
                _ => panic!("unexpected root or derivation in the minimal Merge"),
            }
        }
        observations.sort();
        (counts, observations, initializations)
    };
    let (counts, observations, initializations) = observe(false);
    assert_eq!(
        counts,
        vec![
            (PulsePortSubject::Port(pa), PulseCount::new(2)),
            (PulsePortSubject::Port(pc), PulseCount::new(5))
        ]
    );
    let reversed = observe(true);
    assert_eq!(counts, reversed.0);
    assert_eq!(observations, reversed.1);
    assert_eq!(initializations, 1);
    assert_eq!(initializations, reversed.2);
}

#[test]
fn temporal_state_and_forecast_inspection_match_committed_candidate() {
    let mut b = builder();
    let input = ExternalInputKey::from_u128(10);
    let signal = b.add_level_input(input, meta()).unwrap();
    let transport = NodeKey::from_u128(20);
    let inertial = NodeKey::from_u128(21);
    let periodic = NodeKey::from_u128(22);
    b.add_transport_delay(
        transport,
        signal,
        TransportDelayConfig::new(NonZeroSpan::from_ticks(3).unwrap(), LogicLevel::Low),
        meta(),
    )
    .unwrap();
    b.add_inertial_delay(
        inertial,
        signal,
        InertialDelayConfig::new(NonZeroSpan::from_ticks(4).unwrap(), LogicLevel::Low),
        meta(),
    )
    .unwrap();
    b.add_periodic(
        periodic,
        signal,
        PeriodicConfig::new(
            NonZeroSpan::from_ticks(5).unwrap(),
            FirstEmissionPolicy::AfterFirstPeriod,
            ReenablePhasePolicy::PreservePhase,
        ),
        meta(),
    )
    .unwrap();
    let compiled = compiled(b);
    let mut m = compiled.spawn(policy());
    let snapshot = compiled
        .input_snapshot()
        .set(input, LogicLevel::High)
        .unwrap()
        .finish()
        .unwrap();
    let transaction = Transaction::initialize(Time::from_ticks(2), m.revision(), snapshot);
    let before = m.execution_state_digest();
    let forecast = m.forecast(transaction.clone()).unwrap();
    assert_eq!(m.execution_state_digest(), before);
    assert!(matches!(
        forecast.state().inspect_node(transport).unwrap().state,
        NodeStateInspection::TransportDelay(_)
    ));
    assert!(matches!(
        forecast.state().inspect_node(inertial).unwrap().state,
        NodeStateInspection::InertialDelay(_)
    ));
    assert!(matches!(
        forecast.state().inspect_node(periodic).unwrap().state,
        NodeStateInspection::Periodic(_)
    ));
    let pending = forecast.state().inspect_pending_events().unwrap();
    assert_eq!(pending.len(), 3);
    assert!(matches!(
        pending[0].payload,
        PendingPayload::Level(LogicLevel::High)
    ));
    assert_eq!(pending[0].origin.ticks(), 2);
    assert_eq!(pending[0].remaining.ticks(), 3);
    assert!(
        matches!(pending[2].payload, PendingPayload::Periodic { anchor, ordinal: 1, first_emission: FirstEmissionPolicy::AfterFirstPeriod, reenable_phase: ReenablePhasePolicy::PreservePhase } if anchor.ticks() == 2)
    );
    m.apply(transaction).unwrap();
    let actual = m.inspect_pending_events().unwrap();
    assert_eq!(
        pending
            .iter()
            .map(|event| (event.event, event.deadline, event.cause))
            .collect::<Vec<_>>(),
        actual
            .iter()
            .map(|event| (event.event, event.deadline, event.cause))
            .collect::<Vec<_>>()
    );
    assert_eq!(
        forecast
            .state()
            .explain(Explain::Pending(pending[0].event))
            .unwrap()
            .causal
            .edges,
        m.explain(Explain::Pending(pending[0].event))
            .unwrap()
            .causal
            .edges
    );
}

#[test]
fn explanations_report_checkpoint_boundaries_after_topology_changes() {
    let mut b = builder();
    let input = ExternalInputKey::from_u128(10);
    let output = ExternalOutputKey::from_u128(11);
    let signal = b.add_level_input(input, meta()).unwrap();
    b.add_level_output(output, signal, meta()).unwrap();
    let compiled = compiled(b);
    let mut m = compiled.spawn(policy());
    m.apply(Transaction::initialize(
        Time::from_ticks(0),
        m.revision(),
        compiled
            .input_snapshot()
            .set(input, LogicLevel::High)
            .unwrap()
            .finish()
            .unwrap(),
    ))
    .unwrap();
    let old = m.explain(Explain::CurrentOutput(output.into())).unwrap();
    let patch = m
        .patch()
        .set_diagnostic_meta(
            StructuralSubjectRef::Network(compiled.network_key()),
            DiagnosticMeta {
                name: Some("updated".to_owned()),
                ..meta()
            },
        )
        .unwrap()
        .finish();
    let prepared = m.prepare_patch(patch).require_artifact().unwrap();
    let delta = prepared.input_delta().finish().unwrap();
    m.apply(
        Transaction::advance(Time::from_ticks(1), m.revision(), delta)
            .with_patch(prepared, ReconfigurationPolicy::RejectStateLoss)
            .unwrap(),
    )
    .unwrap();
    let explanation = m.explain(Explain::CurrentOutput(output.into())).unwrap();
    let RetentionStatus::CompleteFromCheckpoints { checkpoints } = &explanation.causal.retention
    else {
        panic!("checkpoint boundary must be explicit");
    };
    assert!(!checkpoints.is_empty());
    for cause in checkpoints {
        assert!(matches!(
            explanation.causal.provenance().inspect(*cause).unwrap(),
            CauseInspection::Checkpoint { .. }
        ));
    }
    assert_eq!(
        old.causal.retention,
        RetentionStatus::CompleteFromInitialization
    );
    assert_eq!(inputs_in(&old.causal), vec![input]);
}

#[test]
fn stateful_standard_explanation_retains_public_suppression_and_primitive_paths() {
    let mut b = builder();
    let toggle = ExternalInputKey::<Pulse>::from_u128(10);
    let reset = ExternalInputKey::<Pulse>::from_u128(11);
    let st = b.add_pulse_input(toggle, meta()).unwrap();
    let sr = b.add_pulse_input(reset, meta()).unwrap();
    let instance = ModuleInstanceKey::from_u128(20);
    b.add_pulse_resettable_toggle(instance, st, sr, LogicLevel::Low, meta())
        .unwrap();
    let compiled = compiled(b);
    let mut m = compiled.spawn(policy());
    m.apply(Transaction::initialize(
        Time::from_ticks(0),
        m.revision(),
        compiled
            .input_snapshot()
            .pulse(toggle, PulseCount::new(3))
            .unwrap()
            .pulse(reset, PulseCount::new(1))
            .unwrap()
            .finish()
            .unwrap(),
    ))
    .unwrap();
    let explanation = m.explain(Explain::CurrentModule(instance)).unwrap();
    let ExplainedObservation::Module(module) = &explanation.observed else {
        panic!("module observation required");
    };
    let ModuleBehavior::Stateful(standard) = module.public_behavior() else {
        panic!("public stateful law required");
    };
    assert_eq!(
        standard.module_ref,
        StandardModuleRef::pulse_resettable_toggle()
    );
    assert!(
        matches!(standard.last_reaction, StatefulStandardReaction::Toggle { previous: LogicLevel::Low, toggle_count, accepted, suppressed, result: LogicLevel::Low, .. } if toggle_count == PulseCount::new(3) && accepted.is_zero() && suppressed == PulseCount::new(3))
    );
    assert!(!standard.internal_causes.is_empty());
    for (_, cause) in &standard.public_causes {
        assert!(explanation.evidence_roots.contains(cause));
        assert!(explanation.causal.provenance().inspect(*cause).is_ok());
    }
}

#[test]
fn pulse_module_explanation_has_only_explicitly_historical_support() {
    let mut module = ModuleBuilder::<()>::new();
    let mi = ModuleInputKey::<Pulse>::from_u128(10);
    let mo = ModuleOutputKey::<Pulse>::from_u128(11);
    let signal = module.add_pulse_input(mi, meta()).unwrap();
    let signal = module.coalesce(signal).unwrap();
    module.add_pulse_output(mo, signal, meta()).unwrap();
    let module = module.finish().require_artifact().unwrap();
    let mut b = builder();
    let input = ExternalInputKey::<Pulse>::from_u128(20);
    let signal = b.add_pulse_input(input, meta()).unwrap();
    let instance = ModuleInstanceKey::from_u128(21);
    b.instantiate(&module, instance, meta())
        .unwrap()
        .bind_pulse(mi, signal)
        .unwrap()
        .finish()
        .unwrap();
    let compiled = compiled(b);
    let mut m = compiled.spawn(policy());
    m.apply(Transaction::initialize(
        Time::from_ticks(0),
        m.revision(),
        compiled
            .input_snapshot()
            .pulse(input, PulseCount::new(3))
            .unwrap()
            .finish()
            .unwrap(),
    ))
    .unwrap();
    let explanation = m.explain(Explain::CurrentModule(instance)).unwrap();
    assert!(
        explanation.current_support.is_empty(),
        "completed pulse activity must not be current support"
    );
    assert!(!explanation.evidence_roots.is_empty());
}

#[test]
fn malformed_inspection_requests_identify_stable_subjects_without_mutation() {
    use mossignal::diagnostics::{DiagnosticCode, InspectionSubjectKind, ProblemEvidence};
    let compiled = compiled(builder());
    let m = compiled.spawn(policy());
    let node = NodeKey::from_u128(10);
    assert_eq!(
        m.inspect_node(node).err().unwrap().code(),
        DiagnosticCode::InspectionUnknownSubject
    );
    assert_eq!(
        m.inspect_pending_events().err().unwrap().code(),
        DiagnosticCode::LifecycleNotInitialized
    );
    let requested = GraphSubjectRef::qualified(
        vec![ModuleInstanceKey::from_u128(20)],
        GraphElement::OutPort(OutPortKey::<Pulse>::from_u128(30).into()),
    );
    let failure = compiled.slice_upstream(&requested).unwrap_err();
    let problem = failure.problem::<()>();
    let ProblemEvidence::InspectionUnknownSubject { evidence, .. } = problem.evidence() else {
        panic!("structured request evidence required");
    };
    assert_eq!(evidence.qualified_path, requested.instances());
    assert_eq!(
        evidence.expected,
        InspectionSubjectKind::GraphElement(requested.element())
    );
}

#[test]
fn missing_pending_uses_catalogue_code_and_event_evidence() {
    // SPEC: docs/specs/exhaustive_diagnostic_code_catalogue.md §32 Inspection
    // A once-valid event which matured is missing pending work, not a graph subject.
    let mut b = builder();
    let input = ExternalInputKey::<Pulse>::from_u128(3);
    let signal = b.add_pulse_input(input, meta()).unwrap();
    b.add_pulse_delay(
        NodeKey::from_u128(4),
        signal,
        PulseDelayConfig::new(NonZeroSpan::from_ticks(2).unwrap()),
        meta(),
    )
    .unwrap();
    let compiled = compiled(b);
    let mut m = compiled.spawn(policy());
    m.apply(Transaction::initialize(
        Time::from_ticks(0),
        m.revision(),
        compiled
            .input_snapshot()
            .pulse(input, PulseCount::new(1))
            .unwrap()
            .finish()
            .unwrap(),
    ))
    .unwrap();
    let event = m.inspect_pending_events().unwrap()[0].event;
    m.apply(Transaction::advance(
        Time::from_ticks(2),
        m.revision(),
        compiled.input_delta().finish().unwrap(),
    ))
    .unwrap();
    let before = (m.execution_state_digest(), m.observable_state_digest());
    for failure in [
        m.inspect_pending(event).err().unwrap(),
        m.explain(Explain::Pending(event)).err().unwrap(),
    ] {
        assert_eq!(
            failure.code().as_str(),
            "inspection.pending_event_not_found"
        );
        assert_eq!(
            failure.code().evidence_schema(),
            mossignal::diagnostics::EvidenceSchema::PendingEvent
        );
        let problem = failure.problem::<()>();
        let mossignal::diagnostics::ProblemEvidence::InspectionPendingEventNotFound {
            evidence,
            ..
        } = problem.evidence()
        else {
            panic!("missing events require event-specific evidence");
        };
        assert_eq!(evidence.event, Some(event.value()));
        assert!(evidence.origin.is_none() && evidence.deadline.is_none());
    }
    assert_eq!(
        before,
        (m.execution_state_digest(), m.observable_state_digest())
    );
}

#[test]
fn unknown_explanation_subjects_use_explanation_catalogue_code() {
    // SPEC: docs/specs/exhaustive_diagnostic_code_catalogue.md §33 Explanation and provenance access
    let compiled = compiled(builder());
    let mut m = compiled.spawn(policy());
    let result = m
        .apply(Transaction::initialize(
            Time::from_ticks(0),
            m.revision(),
            compiled.input_snapshot().finish().unwrap(),
        ))
        .unwrap();
    let before = (m.execution_state_digest(), m.observable_state_digest());
    for request in [
        Explain::CurrentNode(NodeKey::from_u128(10)),
        Explain::CurrentModule(ModuleInstanceKey::from_u128(20)),
        Explain::CurrentOutput(ExternalOutputKey::<Level>::from_u128(30).into()),
    ] {
        assert_eq!(
            m.explain(request).err().unwrap().code().as_str(),
            "explanation.unknown_subject"
        );
    }
    assert_eq!(
        result
            .explain_output_event(0)
            .err()
            .unwrap()
            .code()
            .as_str(),
        "explanation.unknown_subject"
    );
    assert_eq!(
        before,
        (m.execution_state_digest(), m.observable_state_digest())
    );
}

#[test]
fn node_retention_accounts_for_inspected_inputs_outside_current_output_support() {
    let mut b = builder();
    let a = ExternalInputKey::from_u128(10);
    let c = ExternalInputKey::from_u128(11);
    let sa = b.add_level_input(a, meta()).unwrap();
    let sc = b.add_level_input(c, meta()).unwrap();
    let delayed = b
        .transport_delay(
            sc,
            TransportDelayConfig::new(NonZeroSpan::from_ticks(5).unwrap(), LogicLevel::Low),
        )
        .unwrap();
    let node = NodeKey::from_u128(20);
    b.add_any(node, [sa, delayed], meta()).unwrap();
    let compiled = compiled(b);
    let mut m = compiled.spawn(policy());
    m.apply(Transaction::initialize(
        Time::from_ticks(0),
        m.revision(),
        compiled
            .input_snapshot()
            .set(a, LogicLevel::High)
            .unwrap()
            .set(c, LogicLevel::Low)
            .unwrap()
            .finish()
            .unwrap(),
    ))
    .unwrap();
    let patch = m
        .patch()
        .set_diagnostic_meta(
            StructuralSubjectRef::Network(compiled.network_key()),
            DiagnosticMeta {
                name: Some("checkpoint".into()),
                ..meta()
            },
        )
        .unwrap()
        .finish();
    let prepared = m.prepare_patch(patch).require_artifact().unwrap();
    let delta = prepared.input_delta().finish().unwrap();
    m.apply(
        Transaction::advance(Time::from_ticks(1), m.revision(), delta)
            .with_patch(prepared, ReconfigurationPolicy::RejectStateLoss)
            .unwrap(),
    )
    .unwrap();
    m.apply(Transaction::advance(
        Time::from_ticks(2),
        m.revision(),
        m.compiled()
            .input_delta()
            .set(a, LogicLevel::High)
            .unwrap()
            .finish()
            .unwrap(),
    ))
    .unwrap();
    let before = (m.execution_state_digest(), m.observable_state_digest());
    let view = m.inspect_node(node).unwrap();
    let input_retention = view
        .inputs
        .iter()
        .map(|input| {
            view.provenance()
                .explain_cause(input.current_support.unwrap())
                .unwrap()
                .retention
        })
        .collect::<Vec<_>>();
    assert!(
        input_retention
            .iter()
            .any(|retention| matches!(retention, RetentionStatus::CompleteFromCheckpoints { .. }))
    );
    assert!(
        matches!(
            view.retention,
            RetentionStatus::CompleteFromCheckpoints { .. }
        ),
        "an owned node observation must disclose the boundary of its inspected input facts"
    );
    let explained = m.explain(Explain::CurrentNode(node)).unwrap();
    assert!(matches!(
        explained.causal.retention,
        RetentionStatus::CompleteFromCheckpoints { .. }
    ));
    assert_eq!(
        before,
        (m.execution_state_digest(), m.observable_state_digest())
    );
}

#[test]
fn module_explanation_includes_pending_work_from_an_earlier_reaction() {
    let mut module = ModuleBuilder::<()>::new();
    let mi = ModuleInputKey::<Pulse>::from_u128(10);
    let mo = ModuleOutputKey::<Pulse>::from_u128(11);
    let signal = module.add_pulse_input(mi, meta()).unwrap();
    let delayed = module
        .pulse_delay(
            signal,
            PulseDelayConfig::new(NonZeroSpan::from_ticks(5).unwrap()),
        )
        .unwrap();
    module.add_pulse_output(mo, delayed, meta()).unwrap();
    let module = module.finish().require_artifact().unwrap();
    let mut b = builder();
    let input = ExternalInputKey::<Pulse>::from_u128(20);
    let signal = b.add_pulse_input(input, meta()).unwrap();
    let instance = ModuleInstanceKey::from_u128(21);
    b.instantiate(&module, instance, meta())
        .unwrap()
        .bind_pulse(mi, signal)
        .unwrap()
        .finish()
        .unwrap();
    let compiled = compiled(b);
    let mut m = compiled.spawn(policy());
    m.apply(Transaction::initialize(
        Time::from_ticks(0),
        m.revision(),
        compiled
            .input_snapshot()
            .pulse(input, PulseCount::new(3))
            .unwrap()
            .finish()
            .unwrap(),
    ))
    .unwrap();
    m.apply(Transaction::advance(
        Time::from_ticks(1),
        m.revision(),
        compiled.input_delta().finish().unwrap(),
    ))
    .unwrap();
    let before = (m.execution_state_digest(), m.observable_state_digest());
    let explanation = m.explain(Explain::CurrentModule(instance)).unwrap();
    let ExplainedObservation::Module(module) = &explanation.observed else {
        panic!("module observation required");
    };
    let pending = module.nodes()[0].pending()[0].cause();
    assert!(
        explanation.evidence_roots.contains(&pending),
        "pending work is an observed module fact even when the last reaction emits nothing"
    );
    assert!(
        explanation
            .causal
            .edges
            .iter()
            .any(|edge| edge.cause == pending)
    );
    assert_eq!(
        before,
        (m.execution_state_digest(), m.observable_state_digest())
    );
}
