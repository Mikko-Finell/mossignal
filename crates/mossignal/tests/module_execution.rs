use mossignal::key::{
    ExternalInputKey, ExternalOutputKey, ModuleInputKey, ModuleInstanceKey, ModuleOutputKey,
    NodeKey,
};
use mossignal::metadata::DiagnosticMeta;
use mossignal::signal::{Level, LogicLevel, Pulse, PulseCount};
use mossignal::time::{NonZeroSpan, Time};
use mossignal::{
    CauseInspection, ModuleBuilder, ModuleDef, NetworkBuilder, NodeSubject, OutputEvent,
    ProvenanceSubject, PulseDelayConfig, PulsePortSubject, RuntimeFailureEvidence, RuntimePolicy,
    Schedule, TimeDomainId, ToggleConfig, Transaction,
};

fn policy(operation_limit: u64) -> RuntimePolicy {
    RuntimePolicy::builder()
        .max_internal_reactions(100)
        .max_evaluated_operations(operation_limit)
        .max_pending_events(100)
        .max_events_created_per_transaction(100)
        .max_required_provenance_growth(10_000)
        .build()
        .unwrap()
}

#[test]
fn every_level_primitive_executes_inside_one_module() {
    let first_key = ModuleInputKey::<Level>::from_u128(1);
    let second_key = ModuleInputKey::<Level>::from_u128(2);
    let mut module = ModuleBuilder::<()>::new();
    let first = module
        .add_level_input(first_key, DiagnosticMeta::default())
        .unwrap();
    let second = module
        .add_level_input(second_key, DiagnosticMeta::default())
        .unwrap();
    let high = module.constant(LogicLevel::High);
    let not = module.not(first).unwrap();
    let all = module.all([first, second]).unwrap();
    let any = module.any([first, second]).unwrap();
    let parity = module.parity([first, second]).unwrap();
    let threshold = module.at_least(2, [first, second]).unwrap();
    let selected = module.select(high, not, all).unwrap();
    let signals = [not, all, any, parity, threshold, selected];
    let module_outputs = signals
        .into_iter()
        .enumerate()
        .map(|(index, signal)| {
            let key = ModuleOutputKey::<Level>::from_u128(10 + index as u128);
            module
                .add_level_output(key, signal, DiagnosticMeta::default())
                .unwrap();
            key
        })
        .collect::<Vec<_>>();
    let module = module.finish().require_artifact().unwrap();

    let instance = ModuleInstanceKey::from_u128(20);
    let first_input = ExternalInputKey::<Level>::from_u128(21);
    let second_input = ExternalInputKey::<Level>::from_u128(22);
    let mut network = NetworkBuilder::<()>::new(TimeDomainId::from_u128(23));
    let first = network
        .add_level_input(first_input, DiagnosticMeta::default())
        .unwrap();
    let second = network
        .add_level_input(second_input, DiagnosticMeta::default())
        .unwrap();
    let added = network
        .instantiate(&module, instance, DiagnosticMeta::default())
        .unwrap()
        .bind_level(first_key, first)
        .unwrap()
        .bind_level(second_key, second)
        .unwrap()
        .finish()
        .unwrap();
    let external_outputs = module_outputs
        .iter()
        .enumerate()
        .map(|(index, output)| {
            let external = ExternalOutputKey::<Level>::from_u128(30 + index as u128);
            network
                .add_level_output(
                    external,
                    added.level_output(*output).unwrap(),
                    DiagnosticMeta::default(),
                )
                .unwrap();
            external
        })
        .collect::<Vec<_>>();
    let validated = network.finish().require_artifact().unwrap();
    let fingerprint = validated.fingerprint();
    let compiled = validated.compile_ref().require_artifact().unwrap();
    assert_eq!(compiled.fingerprint(), fingerprint);
    assert_eq!(compiled.graph().module_instances().len(), 1);
    assert_eq!(
        compiled.graph().qualified_modules()[0].instances(),
        [instance]
    );

    let snapshot = compiled
        .input_snapshot()
        .set(first_input, LogicLevel::Low)
        .and_then(|builder| builder.set(second_input, LogicLevel::High))
        .and_then(mossignal::InputSnapshotBuilder::finish)
        .unwrap();
    let mut machine = compiled.spawn(policy(10_000));
    machine
        .apply(Transaction::initialize(
            Time::from_ticks(0),
            machine.revision(),
            snapshot,
        ))
        .unwrap();
    let actual = external_outputs
        .iter()
        .map(|output| machine.output_level(*output).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(
        actual,
        vec![
            LogicLevel::High,
            LogicLevel::Low,
            LogicLevel::High,
            LogicLevel::High,
            LogicLevel::Low,
            LogicLevel::Low,
        ]
    );
    let inspection = machine.inspect_module(instance).unwrap();
    assert_eq!(inspection.inputs().len(), 2);
    assert_eq!(inspection.outputs().len(), 6);
    assert_eq!(inspection.nodes().len(), 7);
    assert!(inspection.nodes().iter().all(|node| node.level().is_some()));
}

struct StatefulModule {
    module: ModuleDef<()>,
    input: ModuleInputKey<Pulse>,
    toggle_output: ModuleOutputKey<Level>,
    delay_output: ModuleOutputKey<Pulse>,
    toggle_node: NodeKey,
    delay_node: NodeKey,
}

fn stateful_leaf() -> StatefulModule {
    let input = ModuleInputKey::<Pulse>::from_u128(1);
    let toggle_output = ModuleOutputKey::<Level>::from_u128(2);
    let delay_output = ModuleOutputKey::<Pulse>::from_u128(3);
    let toggle_node = NodeKey::from_u128(10);
    let delay_node = NodeKey::from_u128(11);
    let mut module = ModuleBuilder::<()>::new();
    let pulse = module
        .add_pulse_input(input, DiagnosticMeta::default())
        .unwrap();
    let toggled = module
        .add_toggle(
            toggle_node,
            pulse,
            ToggleConfig::new(LogicLevel::Low),
            DiagnosticMeta::default(),
        )
        .unwrap()
        .into_outputs();
    let delayed = module
        .add_pulse_delay(
            delay_node,
            pulse,
            PulseDelayConfig::new(NonZeroSpan::from_ticks(3).unwrap()),
            DiagnosticMeta::default(),
        )
        .unwrap()
        .into_outputs();
    module
        .add_level_output(toggle_output, toggled, DiagnosticMeta::default())
        .unwrap();
    module
        .add_pulse_output(delay_output, delayed, DiagnosticMeta::default())
        .unwrap();
    StatefulModule {
        module: module.finish().require_artifact().unwrap(),
        input,
        toggle_output,
        delay_output,
        toggle_node,
        delay_node,
    }
}

#[test]
fn nested_instances_keep_state_pending_work_and_provenance_independent() {
    let leaf = stateful_leaf();
    let nested = ModuleInstanceKey::from_u128(100);
    let outer_input = ModuleInputKey::<Pulse>::from_u128(101);
    let outer_toggle = ModuleOutputKey::<Level>::from_u128(102);
    let outer_delay = ModuleOutputKey::<Pulse>::from_u128(103);
    let mut outer = ModuleBuilder::<()>::new();
    let pulse = outer
        .add_pulse_input(outer_input, DiagnosticMeta::default())
        .unwrap();
    let added = outer
        .instantiate(&leaf.module, nested, DiagnosticMeta::default())
        .unwrap()
        .bind_pulse(leaf.input, pulse)
        .unwrap()
        .finish()
        .unwrap();
    outer
        .add_level_output(
            outer_toggle,
            added.level_output(leaf.toggle_output).unwrap(),
            DiagnosticMeta::default(),
        )
        .unwrap();
    outer
        .add_pulse_output(
            outer_delay,
            added.pulse_output(leaf.delay_output).unwrap(),
            DiagnosticMeta::default(),
        )
        .unwrap();
    let outer = outer.finish().require_artifact().unwrap();

    let first_instance = ModuleInstanceKey::from_u128(200);
    let second_instance = ModuleInstanceKey::from_u128(300);
    let first_input = ExternalInputKey::<Pulse>::from_u128(201);
    let second_input = ExternalInputKey::<Pulse>::from_u128(301);
    let first_toggle = ExternalOutputKey::<Level>::from_u128(202);
    let second_toggle = ExternalOutputKey::<Level>::from_u128(302);
    let first_delay = ExternalOutputKey::<Pulse>::from_u128(203);
    let second_delay = ExternalOutputKey::<Pulse>::from_u128(303);
    let mut network = NetworkBuilder::<()>::new(TimeDomainId::from_u128(400));
    let first_source = network
        .add_pulse_input(first_input, DiagnosticMeta::default())
        .unwrap();
    let second_source = network
        .add_pulse_input(second_input, DiagnosticMeta::default())
        .unwrap();
    for (instance, source, toggle, delay) in [
        (first_instance, first_source, first_toggle, first_delay),
        (second_instance, second_source, second_toggle, second_delay),
    ] {
        let added = network
            .instantiate(&outer, instance, DiagnosticMeta::default())
            .unwrap()
            .bind_pulse(outer_input, source)
            .unwrap()
            .finish()
            .unwrap();
        network
            .add_level_output(
                toggle,
                added.level_output(outer_toggle).unwrap(),
                DiagnosticMeta::default(),
            )
            .unwrap();
        network
            .add_pulse_output(
                delay,
                added.pulse_output(outer_delay).unwrap(),
                DiagnosticMeta::default(),
            )
            .unwrap();
    }
    let compiled = network
        .finish()
        .require_artifact()
        .unwrap()
        .compile()
        .require_artifact()
        .unwrap();
    assert_eq!(compiled.graph().qualified_modules().len(), 4);
    assert!(
        compiled
            .graph()
            .qualified_nodes()
            .iter()
            .any(|node| node.instances() == [first_instance, nested])
    );

    let snapshot = compiled
        .input_snapshot()
        .pulse(first_input, PulseCount::ONE)
        .and_then(mossignal::InputSnapshotBuilder::finish)
        .unwrap();
    let mut machine = compiled.spawn(policy(100_000));
    let initialized = machine
        .apply(Transaction::initialize(
            Time::from_ticks(0),
            machine.revision(),
            snapshot,
        ))
        .unwrap();
    assert_eq!(machine.output_level(first_toggle), Some(LogicLevel::High));
    assert_eq!(machine.output_level(second_toggle), Some(LogicLevel::Low));
    assert_eq!(
        initialized.schedule(),
        Schedule::WakeAt(Time::from_ticks(3))
    );

    let first = machine.inspect_module(first_instance).unwrap();
    let second = machine.inspect_module(second_instance).unwrap();
    assert_eq!(first.modules().len(), 1);
    let first_toggle_node = first
        .nodes()
        .iter()
        .find(|node| node.node().node() == leaf.toggle_node)
        .unwrap();
    let second_toggle_node = second
        .nodes()
        .iter()
        .find(|node| node.node().node() == leaf.toggle_node)
        .unwrap();
    assert_eq!(
        first_toggle_node.node().instances(),
        [first_instance, nested]
    );
    assert_eq!(first_toggle_node.toggle_state(), Some(LogicLevel::High));
    assert_eq!(second_toggle_node.toggle_state(), Some(LogicLevel::Low));
    assert!(matches!(
        machine.inspect_toggle_definition(NodeKey::from_u128(0)),
        Err(mossignal::ToggleInspectionFailure::UnknownNode(_))
    ));
    let first_delay_node = first
        .nodes()
        .iter()
        .find(|node| node.node().node() == leaf.delay_node)
        .unwrap();
    let second_delay_node = second
        .nodes()
        .iter()
        .find(|node| node.node().node() == leaf.delay_node)
        .unwrap();
    assert_eq!(first_delay_node.pending().len(), 1);
    assert!(second_delay_node.pending().is_empty());
    let pending_cause = first_delay_node.pending()[0].cause();
    let CauseInspection::PendingPulseDelay { owner, .. } =
        initialized.provenance().inspect(pending_cause).unwrap()
    else {
        panic!("module PulseDelay pending work must retain temporal provenance")
    };
    assert!(matches!(
        owner,
        NodeSubject::Qualified(node)
            if node.instances() == [first_instance, nested] && node.node() == leaf.delay_node
    ));

    let delta = compiled
        .input_delta()
        .pulse(second_input, PulseCount::ONE)
        .and_then(mossignal::InputDeltaBuilder::finish)
        .unwrap();
    machine
        .apply(Transaction::advance(
            Time::from_ticks(1),
            machine.revision(),
            delta,
        ))
        .unwrap();
    assert_eq!(machine.output_level(second_toggle), Some(LogicLevel::High));
    assert_eq!(machine.next_deadline(), Ok(Some(Time::from_ticks(3))));

    let result = machine
        .apply(Transaction::advance(
            Time::from_ticks(4),
            machine.revision(),
            compiled.input_delta().finish().unwrap(),
        ))
        .unwrap();
    let pulses = result
        .output_events()
        .iter()
        .filter_map(|event| match event {
            OutputEvent::Pulsed {
                output, count, at, ..
            } => Some((*output, *count, at.ticks())),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        pulses,
        vec![
            (first_delay, PulseCount::ONE, 3),
            (second_delay, PulseCount::ONE, 4),
        ]
    );
    assert_eq!(result.schedule(), Schedule::Dormant);
}

#[test]
fn module_delay_time_overflow_reports_qualified_owner_and_preserves_awaiting_state() {
    let leaf = stateful_leaf();
    let instance = ModuleInstanceKey::from_u128(50);
    let input = ExternalInputKey::<Pulse>::from_u128(51);
    let mut network = NetworkBuilder::<()>::new(TimeDomainId::from_u128(52));
    let source = network
        .add_pulse_input(input, DiagnosticMeta::default())
        .unwrap();
    network
        .instantiate(&leaf.module, instance, DiagnosticMeta::default())
        .unwrap()
        .bind_pulse(leaf.input, source)
        .unwrap()
        .finish()
        .unwrap();
    let compiled = network
        .finish()
        .require_artifact()
        .unwrap()
        .compile()
        .require_artifact()
        .unwrap();
    let snapshot = compiled
        .input_snapshot()
        .pulse(input, PulseCount::ONE)
        .and_then(mossignal::InputSnapshotBuilder::finish)
        .unwrap();
    let mut machine = compiled.spawn(policy(10_000));
    let failure = machine
        .apply(Transaction::initialize(
            Time::from_ticks(u64::MAX - 1),
            machine.revision(),
            snapshot,
        ))
        .expect_err("module-local deadline overflow must reject");
    assert!(matches!(
        failure.evidence(),
        RuntimeFailureEvidence::TimeOverflow {
            node: NodeSubject::Qualified(node),
            origin_ticks,
            delay_ticks,
        } if node.instances() == [instance]
            && node.node() == leaf.delay_node
            && *origin_ticks == u64::MAX - 1
            && *delay_ticks == 3
    ));
    assert!(!machine.is_initialized());
    assert_eq!(
        machine.next_deadline(),
        Err(mossignal::ScheduleFailure::NotInitialized)
    );
    assert!(
        machine
            .inspect_module(instance)
            .unwrap()
            .nodes()
            .iter()
            .all(|node| node.pending().is_empty())
    );
}

#[test]
fn module_merge_overflow_reports_qualified_owner_and_is_atomic() {
    let input = ModuleInputKey::<Pulse>::from_u128(1);
    let output = ModuleOutputKey::<Pulse>::from_u128(2);
    let merge_node = NodeKey::from_u128(3);
    let mut module = ModuleBuilder::<()>::new();
    let pulse = module
        .add_pulse_input(input, DiagnosticMeta::default())
        .unwrap();
    let merged = module
        .add_merge(merge_node, [pulse, pulse], DiagnosticMeta::default())
        .unwrap()
        .into_outputs();
    module
        .add_pulse_output(output, merged, DiagnosticMeta::default())
        .unwrap();
    let module = module.finish().require_artifact().unwrap();

    let instance = ModuleInstanceKey::from_u128(10);
    let external = ExternalInputKey::<Pulse>::from_u128(11);
    let external_output = ExternalOutputKey::<Pulse>::from_u128(13);
    let mut network = NetworkBuilder::<()>::new(TimeDomainId::from_u128(12));
    let source = network
        .add_pulse_input(external, DiagnosticMeta::default())
        .unwrap();
    let added = network
        .instantiate(&module, instance, DiagnosticMeta::default())
        .unwrap()
        .bind_pulse(input, source)
        .unwrap()
        .finish()
        .unwrap();
    network
        .add_pulse_output(
            external_output,
            added.pulse_output(output).unwrap(),
            DiagnosticMeta::default(),
        )
        .unwrap();
    let compiled = network
        .finish()
        .require_artifact()
        .unwrap()
        .compile()
        .require_artifact()
        .unwrap();
    let snapshot = compiled
        .input_snapshot()
        .pulse(external, PulseCount::new(u64::MAX))
        .and_then(mossignal::InputSnapshotBuilder::finish)
        .unwrap();
    let mut machine = compiled.spawn(policy(10_000));
    let failure = machine
        .apply(Transaction::initialize(
            Time::from_ticks(0),
            machine.revision(),
            snapshot,
        ))
        .expect_err("module-local overflow must reject");
    assert!(matches!(
        failure.evidence(),
        RuntimeFailureEvidence::PulseCountOverflow {
            node: NodeSubject::Qualified(node),
        } if node.instances() == [instance] && node.node() == merge_node
    ));
    assert!(!machine.is_initialized());
    assert!(
        machine
            .inspect_module(instance)
            .unwrap()
            .nodes()
            .iter()
            .all(|node| {
                node.level().is_none() && node.toggle_state().is_none() && node.pending().is_empty()
            })
    );

    let retry = compiled
        .input_snapshot()
        .pulse(external, PulseCount::ONE)
        .and_then(mossignal::InputSnapshotBuilder::finish)
        .unwrap();
    let result = machine
        .apply(Transaction::initialize(
            Time::from_ticks(0),
            machine.revision(),
            retry,
        ))
        .unwrap();
    let cause = result
        .output_events()
        .iter()
        .find_map(|event| match event {
            OutputEvent::Pulsed { cause, .. } => Some(*cause),
            _ => None,
        })
        .unwrap();
    let mut pending = vec![cause];
    let mut qualified_port = false;
    while let Some(cause) = pending.pop() {
        match result.provenance().inspect(cause).unwrap() {
            CauseInspection::Derived { supporters, .. }
            | CauseInspection::PendingPulseDelay { supporters, .. } => {
                pending.extend_from_slice(supporters);
            }
            CauseInspection::PulseDerived {
                contributions,
                supporters,
                ..
            } => {
                qualified_port = contributions.iter().all(|contribution| {
                    matches!(
                        contribution.port(),
                        PulsePortSubject::Qualified(port) if port.instances() == [instance]
                    )
                });
                pending.extend_from_slice(supporters);
            }
            _ => {}
        }
    }
    assert!(qualified_port);
}

#[test]
fn explicit_and_module_wrapped_work_hit_the_same_operation_budget() {
    let input = ExternalInputKey::<Level>::from_u128(1);
    let output = ExternalOutputKey::<Level>::from_u128(2);
    let mut explicit = NetworkBuilder::<()>::new(TimeDomainId::from_u128(3));
    let source = explicit
        .add_level_input(input, DiagnosticMeta::default())
        .unwrap();
    let inverted = explicit.not(source).unwrap();
    explicit
        .add_level_output(output, inverted, DiagnosticMeta::default())
        .unwrap();
    let explicit = explicit
        .finish()
        .require_artifact()
        .unwrap()
        .compile()
        .require_artifact()
        .unwrap();

    let module_input = ModuleInputKey::<Level>::from_u128(10);
    let module_output = ModuleOutputKey::<Level>::from_u128(11);
    let mut definition = ModuleBuilder::<()>::new();
    let source = definition
        .add_level_input(module_input, DiagnosticMeta::default())
        .unwrap();
    let inverted = definition.not(source).unwrap();
    definition
        .add_level_output(module_output, inverted, DiagnosticMeta::default())
        .unwrap();
    let definition = definition.finish().require_artifact().unwrap();
    let mut wrapped = NetworkBuilder::<()>::new(TimeDomainId::from_u128(3));
    let source = wrapped
        .add_level_input(input, DiagnosticMeta::default())
        .unwrap();
    let added = wrapped
        .instantiate(
            &definition,
            ModuleInstanceKey::from_u128(12),
            DiagnosticMeta::default(),
        )
        .unwrap()
        .bind_level(module_input, source)
        .unwrap()
        .finish()
        .unwrap();
    wrapped
        .add_level_output(
            output,
            added.level_output(module_output).unwrap(),
            DiagnosticMeta::default(),
        )
        .unwrap();
    let wrapped = wrapped
        .finish()
        .require_artifact()
        .unwrap()
        .compile()
        .require_artifact()
        .unwrap();

    let run = |compiled: &mossignal::CompiledNetwork<()>| {
        let snapshot = compiled
            .input_snapshot()
            .set(input, LogicLevel::High)
            .and_then(mossignal::InputSnapshotBuilder::finish)
            .unwrap();
        let mut machine = compiled.spawn(policy(3));
        machine
            .apply(Transaction::initialize(
                Time::from_ticks(0),
                machine.revision(),
                snapshot,
            ))
            .expect_err("four-operation graph must exceed a limit of three")
            .evidence()
            .clone()
    };
    assert_eq!(run(&explicit), run(&wrapped));
}

#[test]
fn qualified_node_provenance_is_discoverable_without_private_keys() {
    let (module, input, output) = {
        let mut module = ModuleBuilder::<()>::new();
        let input = ModuleInputKey::<Level>::from_u128(1);
        let output = ModuleOutputKey::<Level>::from_u128(2);
        let source = module
            .add_level_input(input, DiagnosticMeta::default())
            .unwrap();
        let inverted = module.not(source).unwrap();
        module
            .add_level_output(output, inverted, DiagnosticMeta::default())
            .unwrap();
        (module.finish().require_artifact().unwrap(), input, output)
    };
    let instance = ModuleInstanceKey::from_u128(3);
    let external_input = ExternalInputKey::<Level>::from_u128(4);
    let external_output = ExternalOutputKey::<Level>::from_u128(5);
    let mut network = NetworkBuilder::<()>::new(TimeDomainId::from_u128(6));
    let source = network
        .add_level_input(external_input, DiagnosticMeta::default())
        .unwrap();
    let added = network
        .instantiate(&module, instance, DiagnosticMeta::default())
        .unwrap()
        .bind_level(input, source)
        .unwrap()
        .finish()
        .unwrap();
    network
        .add_level_output(
            external_output,
            added.level_output(output).unwrap(),
            DiagnosticMeta::default(),
        )
        .unwrap();
    let compiled = network
        .finish()
        .require_artifact()
        .unwrap()
        .compile()
        .require_artifact()
        .unwrap();
    let snapshot = compiled
        .input_snapshot()
        .set(external_input, LogicLevel::High)
        .and_then(mossignal::InputSnapshotBuilder::finish)
        .unwrap();
    let mut machine = compiled.spawn(policy(10_000));
    let result = machine
        .apply(Transaction::initialize(
            Time::from_ticks(0),
            machine.revision(),
            snapshot,
        ))
        .unwrap();
    let mut pending = vec![machine.output_cause(external_output).unwrap()];
    let mut found = false;
    while let Some(cause) = pending.pop() {
        match result.provenance().inspect(cause).unwrap() {
            CauseInspection::Derived {
                subject: ProvenanceSubject::QualifiedNode(node),
                supporters,
            } => {
                assert_eq!(node.instances(), [instance]);
                found = true;
                pending.extend_from_slice(supporters);
            }
            CauseInspection::Derived { supporters, .. }
            | CauseInspection::PendingPulseDelay { supporters, .. } => {
                pending.extend_from_slice(supporters);
            }
            CauseInspection::PulseDerived { supporters, .. } => {
                pending.extend_from_slice(supporters);
            }
            CauseInspection::InitializationTransaction { .. }
            | CauseInspection::ReadyTransaction { .. }
            | CauseInspection::ExternalObservation { .. }
            | CauseInspection::ExternalPulseObservation { .. } => {}
            _ => {}
        }
    }
    assert!(found);
}
