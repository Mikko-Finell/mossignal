//! Commitment of one prepared topology patch through an ordinary transaction.

use mossignal::authored::{
    ConnectionDef, ConnectionEndpoint, EdgeConfig, EdgeInitialization, ExternalInputDef,
    ExternalOutputDef, NodeDef, NodeKind, NodePorts,
};
use mossignal::diagnostics::SubjectRef;
use mossignal::key::{
    AnySignalSourceKey, ConnectionKey, ExternalInputKey, ExternalOutputKey, InPortKey,
    ModuleInputKey, ModuleInstanceKey, ModuleOutputKey, NetworkKey, NodeKey, OutPortKey,
    SignalSourceKey,
};
use mossignal::metadata::DiagnosticMeta;
use mossignal::signal::{Level, LogicLevel, Pulse, PulseCount};
use mossignal::time::{NonZeroSpan, Time};
use mossignal::{
    InputDelta, Machine, ModuleBuilder, ModuleMigrationDirective, ModuleNodeMigrationDirective,
    NetworkBuilder, NodeMigrationDirective, OutputEvent, OutputOutcome, PreparedPatch,
    PulseDelayConfig, PulseDelayMigration, ReconfigurationPolicy, RuntimePolicy, StateOutcome,
    StructuralSubjectRef, TimeDomainId, ToggleConfig, Transaction, TransactionBuildFailure,
    TransactionResult, TransportDelayConfig, record_replay_log,
};

#[derive(Debug, PartialEq, Eq)]
enum Domain {}

fn policy() -> RuntimePolicy {
    RuntimePolicy::builder()
        .max_internal_reactions(1_000)
        .max_evaluated_operations(100_000)
        .max_pending_events(1_000)
        .max_events_created_per_transaction(10_000)
        .max_required_provenance_growth(100_000)
        .build()
        .unwrap_or_else(|failure| panic!("policy must build: {failure}"))
}

fn compile(builder: NetworkBuilder<Domain>) -> mossignal::CompiledNetwork<Domain> {
    builder
        .finish()
        .require_artifact()
        .unwrap_or_else(|failure| panic!("network must validate: {failure:?}"))
        .compile()
        .require_artifact()
        .unwrap_or_else(|failure| panic!("network must compile: {failure:?}"))
}

fn delay(ticks: u64) -> NonZeroSpan<Domain> {
    NonZeroSpan::from_ticks(ticks)
        .unwrap_or_else(|failure| panic!("delay must be nonzero: {failure}"))
}

fn meta(name: &str) -> DiagnosticMeta {
    DiagnosticMeta {
        name: Some(name.to_owned()),
        ..DiagnosticMeta::default()
    }
}

fn level_snapshot(
    compiled: &mossignal::CompiledNetwork<Domain>,
    observations: &[(ExternalInputKey<Level>, LogicLevel)],
) -> mossignal::InputSnapshot<Domain> {
    let mut builder = compiled.input_snapshot();
    for (input, level) in observations {
        builder = builder
            .set(*input, *level)
            .unwrap_or_else(|failure| panic!("level observation must bind: {failure}"));
    }
    builder
        .finish()
        .unwrap_or_else(|failure| panic!("snapshot must finish: {failure}"))
}

fn level_delta(
    compiled: &mossignal::CompiledNetwork<Domain>,
    observations: &[(ExternalInputKey<Level>, LogicLevel)],
) -> InputDelta<Domain> {
    let mut builder = compiled.input_delta();
    for (input, level) in observations {
        builder = builder
            .set(*input, *level)
            .unwrap_or_else(|failure| panic!("level delta must bind: {failure}"));
    }
    builder
        .finish()
        .unwrap_or_else(|failure| panic!("delta must finish: {failure}"))
}

fn prepare(
    machine: &Machine<Domain>,
    patch: mossignal::NetworkPatch<Domain>,
) -> PreparedPatch<Domain> {
    let report = machine.prepare_patch(patch);
    report.artifact().cloned().unwrap_or_else(|| {
        let codes = report
            .diagnostics()
            .iter()
            .map(|diagnostic| diagnostic.problem().code().as_str())
            .collect::<Vec<_>>();
        panic!("patch must prepare: {codes:?}")
    })
}

fn metadata_patch(machine: &Machine<Domain>, name: &str) -> PreparedPatch<Domain> {
    prepare(
        machine,
        machine
            .patch()
            .set_diagnostic_meta(
                StructuralSubjectRef::Network(machine.compiled().network_key()),
                meta(name),
            )
            .unwrap_or_else(|failure| panic!("metadata edit must build: {failure:?}"))
            .finish(),
    )
}

fn init(
    machine: &mut Machine<Domain>,
    at: Time<Domain>,
    snapshot: mossignal::InputSnapshot<Domain>,
) -> TransactionResult<Domain> {
    machine
        .apply(Transaction::initialize(at, machine.revision(), snapshot))
        .unwrap_or_else(|failure| panic!("initialization must apply: {}", failure.code().as_str()))
}

fn node_def(compiled: &mossignal::CompiledNetwork<Domain>, key: NodeKey) -> NodeDef<Domain> {
    compiled
        .graph()
        .nodes()
        .iter()
        .find(|node| node.key() == key)
        .unwrap_or_else(|| panic!("node {key:?} must be retained"))
        .clone()
}

fn pulsed(result: &TransactionResult<Domain>, output: ExternalOutputKey<Pulse>) -> bool {
    result.output_events().iter().any(|event| {
        matches!(
            event,
            OutputEvent::Pulsed {
                output: found,
                ..
            } if *found == output
        )
    })
}

fn event_revision(
    result: &TransactionResult<Domain>,
    output: ExternalOutputKey<Pulse>,
) -> Option<mossignal::NetworkRevision> {
    result.output_events().iter().find_map(|event| match event {
        OutputEvent::Pulsed {
            output: found,
            revision,
            ..
        } if *found == output => Some(*revision),
        _ => None,
    })
}

#[test]
fn earlier_deadline_migrates_the_state_it_reached() {
    let mut builder =
        NetworkBuilder::with_key(NetworkKey::from_u128(1), TimeDomainId::from_u128(2));
    let input = ExternalInputKey::<Level>::from_u128(1);
    let signal = builder
        .add_level_input(input, DiagnosticMeta::default())
        .unwrap_or_else(|failure| panic!("level input must author: {failure:?}"));
    let transport = NodeKey::from_u128(2);
    let delayed = builder
        .add_transport_delay(
            transport,
            signal,
            TransportDelayConfig::new(delay(5), LogicLevel::Low),
            DiagnosticMeta::default(),
        )
        .unwrap_or_else(|failure| panic!("transport must author: {failure:?}"))
        .into_outputs();
    let output = ExternalOutputKey::<Level>::from_u128(3);
    builder
        .add_level_output(output, delayed, DiagnosticMeta::default())
        .unwrap_or_else(|failure| panic!("level output must author: {failure:?}"));
    let compiled = compile(builder);
    let mut machine = compiled.spawn(policy());
    init(
        &mut machine,
        Time::from_ticks(0),
        level_snapshot(&compiled, &[(input, LogicLevel::High)]),
    );
    assert_eq!(machine.output_level(output), Some(LogicLevel::Low));
    assert_eq!(
        machine
            .inspect_transport_delay(transport)
            .unwrap()
            .pending()[0]
            .deadline(),
        Time::from_ticks(5)
    );

    let prepared = metadata_patch(&machine, "renamed");
    let delta = level_delta(prepared.resulting_compiled(), &[(input, LogicLevel::High)]);
    let result = machine
        .apply(
            Transaction::advance(Time::from_ticks(10), machine.revision(), delta)
                .with_patch(prepared, ReconfigurationPolicy::RejectStateLoss)
                .unwrap_or_else(|failure| {
                    panic!("preserving patch must attach: {}", failure.code().as_str())
                }),
        )
        .unwrap_or_else(|failure| {
            panic!("preserving patch must commit: {}", failure.code().as_str())
        });

    assert_eq!(machine.output_level(output), Some(LogicLevel::High));
    assert!(
        machine
            .inspect_transport_delay(transport)
            .unwrap()
            .pending()
            .is_empty()
    );
    assert_eq!(
        machine.inspect_transport_delay(transport).unwrap().output(),
        LogicLevel::High
    );
    assert_eq!(
        result.before_revision(),
        result.migration().unwrap().base_revision()
    );
    assert_ne!(result.before_revision(), result.after_revision());
    assert_eq!(result.after_revision(), machine.revision());
    assert_eq!(
        result.migration().unwrap().target_revision(),
        machine.revision()
    );
    assert!(result.migration().unwrap().states().iter().any(|state| {
        state.subject() == &SubjectRef::Node(transport)
            && state.outcome() == StateOutcome::Preserved
    }));
    assert!(result.output_events().iter().any(|event| matches!(
        event,
        OutputEvent::LevelChanged {
            output: found,
            to: LogicLevel::High,
            revision,
            ..
        } if *found == output && *revision == result.before_revision()
    )));
}

#[test]
fn obligations_before_at_and_after_patch_time_keep_their_sides() {
    let mut builder =
        NetworkBuilder::with_key(NetworkKey::from_u128(3), TimeDomainId::from_u128(4));
    let input = ExternalInputKey::<Pulse>::from_u128(1);
    let signal = builder
        .add_pulse_input(input, DiagnosticMeta::default())
        .unwrap_or_else(|failure| panic!("pulse input must author: {failure:?}"));
    let early = NodeKey::from_u128(2);
    let exact = NodeKey::from_u128(3);
    let later = NodeKey::from_u128(4);
    let early_signal = builder
        .add_pulse_delay(
            early,
            signal,
            PulseDelayConfig::new(delay(6)),
            DiagnosticMeta::default(),
        )
        .unwrap_or_else(|failure| panic!("early delay must author: {failure:?}"))
        .into_outputs();
    let exact_signal = builder
        .add_pulse_delay(
            exact,
            signal,
            PulseDelayConfig::new(delay(8)),
            DiagnosticMeta::default(),
        )
        .unwrap_or_else(|failure| panic!("exact delay must author: {failure:?}"))
        .into_outputs();
    let later_signal = builder
        .add_pulse_delay(
            later,
            signal,
            PulseDelayConfig::new(delay(15)),
            DiagnosticMeta::default(),
        )
        .unwrap_or_else(|failure| panic!("later delay must author: {failure:?}"))
        .into_outputs();
    let early_output = ExternalOutputKey::<Pulse>::from_u128(5);
    let exact_output = ExternalOutputKey::<Pulse>::from_u128(6);
    let later_output = ExternalOutputKey::<Pulse>::from_u128(7);
    builder
        .add_pulse_output(early_output, early_signal, DiagnosticMeta::default())
        .unwrap();
    builder
        .add_pulse_output(exact_output, exact_signal, DiagnosticMeta::default())
        .unwrap();
    builder
        .add_pulse_output(later_output, later_signal, DiagnosticMeta::default())
        .unwrap();
    let compiled = compile(builder);
    let mut machine = compiled.spawn(policy());
    let mut snapshot = compiled.input_snapshot();
    snapshot = snapshot
        .pulse(input, PulseCount::ONE)
        .unwrap_or_else(|failure| panic!("pulse must bind: {failure}"));
    init(
        &mut machine,
        Time::from_ticks(0),
        snapshot.finish().unwrap(),
    );

    let prepared = prepare(
        &machine,
        machine
            .patch()
            .replace_node(
                exact,
                node_def(&compiled, exact),
                NodeMigrationDirective::PulseDelay(PulseDelayMigration::CancelPending),
            )
            .unwrap_or_else(|failure| panic!("cancel replacement must build: {failure:?}"))
            .finish(),
    );
    let delta = prepared
        .resulting_compiled()
        .input_delta()
        .finish()
        .unwrap();
    let before = machine.snapshot();
    let rejected = machine
        .apply(
            Transaction::advance(Time::from_ticks(8), machine.revision(), delta.clone())
                .with_patch(prepared.clone(), ReconfigurationPolicy::RejectStateLoss)
                .unwrap(),
        )
        .expect_err("realized pending loss must reject");
    assert_eq!(
        rejected.code().as_str(),
        "reconfiguration.state_loss_rejected"
    );
    assert_eq!(machine.snapshot(), before);
    assert_eq!(
        machine.inspect_pulse_delay(early).unwrap().next_deadline(),
        Some(Time::from_ticks(6))
    );
    assert_eq!(
        machine.inspect_pulse_delay(exact).unwrap().next_deadline(),
        Some(Time::from_ticks(8))
    );

    let result = machine
        .apply(
            Transaction::advance(Time::from_ticks(8), machine.revision(), delta)
                .with_patch(prepared, ReconfigurationPolicy::AllowReportedStateLoss)
                .unwrap(),
        )
        .unwrap_or_else(|failure| panic!("reported loss must commit: {}", failure.code().as_str()));
    assert!(pulsed(&result, early_output));
    assert_eq!(
        event_revision(&result, early_output),
        Some(result.before_revision())
    );
    assert!(!pulsed(&result, exact_output));
    assert!(!pulsed(&result, later_output));
    assert_eq!(
        machine.inspect_pulse_delay(later).unwrap().next_deadline(),
        Some(Time::from_ticks(15))
    );
    assert!(
        machine
            .inspect_pulse_delay(exact)
            .unwrap()
            .pending()
            .is_empty()
    );
    assert!(result.migration().unwrap().losses().iter().any(|loss| {
        loss.fact() == "pending_group"
            && loss.conditional()
            && loss.subject() == &SubjectRef::Node(exact)
    }));
}

#[test]
fn unrealized_conditional_loss_commits_under_both_policies() {
    let mut builder =
        NetworkBuilder::with_key(NetworkKey::from_u128(5), TimeDomainId::from_u128(6));
    let input = ExternalInputKey::<Pulse>::from_u128(1);
    let signal = builder
        .add_pulse_input(input, DiagnosticMeta::default())
        .unwrap();
    let node = NodeKey::from_u128(2);
    builder
        .add_pulse_delay(
            node,
            signal,
            PulseDelayConfig::new(delay(5)),
            DiagnosticMeta::default(),
        )
        .unwrap();
    let compiled = compile(builder);
    for policy_choice in [
        ReconfigurationPolicy::RejectStateLoss,
        ReconfigurationPolicy::AllowReportedStateLoss,
    ] {
        let mut machine = compiled.spawn(policy());
        init(
            &mut machine,
            Time::from_ticks(0),
            compiled.input_snapshot().finish().unwrap(),
        );
        let prepared = prepare(
            &machine,
            machine
                .patch()
                .replace_node(
                    node,
                    node_def(&compiled, node),
                    NodeMigrationDirective::PulseDelay(PulseDelayMigration::CancelPending),
                )
                .unwrap()
                .finish(),
        );
        let result = machine
            .apply(
                Transaction::advance(
                    Time::from_ticks(4),
                    machine.revision(),
                    prepared
                        .resulting_compiled()
                        .input_delta()
                        .finish()
                        .unwrap(),
                )
                .with_patch(prepared, policy_choice)
                .unwrap(),
            )
            .unwrap_or_else(|failure| {
                panic!("unrealized cancel must commit: {}", failure.code().as_str())
            });
        assert!(
            result
                .migration()
                .unwrap()
                .losses()
                .iter()
                .all(|loss| loss.fact() != "pending_group")
        );
        assert_eq!(result.after_revision(), machine.revision());
    }
}

#[test]
fn level_outputs_follow_preserve_establish_and_remove() {
    let mut builder =
        NetworkBuilder::with_key(NetworkKey::from_u128(7), TimeDomainId::from_u128(8));
    let input = ExternalInputKey::<Level>::from_u128(1);
    let signal = builder
        .add_level_input(input, DiagnosticMeta::default())
        .unwrap();
    let inverted = builder
        .add_not_with_ports(
            NodeKey::from_u128(2),
            InPortKey::from_u128(3),
            OutPortKey::from_u128(4),
            signal,
            DiagnosticMeta::default(),
        )
        .unwrap()
        .into_outputs();
    let kept = ExternalOutputKey::<Level>::from_u128(5);
    let removed = ExternalOutputKey::<Level>::from_u128(6);
    builder
        .add_level_output(kept, inverted, DiagnosticMeta::default())
        .unwrap();
    builder
        .add_level_output(removed, inverted, DiagnosticMeta::default())
        .unwrap();
    let compiled = compile(builder);
    let mut machine = compiled.spawn(policy());
    init(
        &mut machine,
        Time::from_ticks(0),
        level_snapshot(&compiled, &[(input, LogicLevel::High)]),
    );
    assert_eq!(machine.output_level(kept), Some(LogicLevel::Low));

    let source = compiled
        .graph()
        .external_outputs()
        .iter()
        .find(|item| item.key() == kept.into())
        .unwrap()
        .source();
    let established = ExternalOutputKey::<Level>::from_u128(50);
    let prepared = prepare(
        &machine,
        machine
            .patch()
            .remove_external_output(removed.into())
            .unwrap()
            .add_external_output(ExternalOutputDef::new(
                established.into(),
                source,
                meta("new"),
            ))
            .unwrap()
            .finish(),
    );
    let result = machine
        .apply(
            Transaction::advance(
                Time::from_ticks(3),
                machine.revision(),
                level_delta(prepared.resulting_compiled(), &[(input, LogicLevel::High)]),
            )
            .with_patch(prepared, ReconfigurationPolicy::AllowReportedStateLoss)
            .unwrap(),
        )
        .unwrap_or_else(|failure| {
            panic!("output migration must commit: {}", failure.code().as_str())
        });
    let report = result.migration().unwrap();
    assert!(report.outputs().iter().any(|output| {
        output.source() == Some(kept.into()) && output.outcome() == OutputOutcome::Preserved
    }));
    assert!(report.outputs().iter().any(|output| {
        output.target() == Some(established.into())
            && output.outcome() == OutputOutcome::Established
    }));
    assert!(report.outputs().iter().any(|output| {
        output.source() == Some(removed.into()) && output.outcome() == OutputOutcome::Removed
    }));
    assert!(result.output_events().iter().any(|event| matches!(
        event,
        OutputEvent::LevelEstablished { output, value: LogicLevel::Low, .. } if *output == established
    )));
    assert!(result.output_events().iter().all(|event| !matches!(
        event,
        OutputEvent::LevelEstablished { output, .. } | OutputEvent::LevelChanged { output, .. } if *output == removed
    )));
    assert!(
        report
            .losses()
            .iter()
            .any(|loss| loss.fact() == "level_baseline")
    );
    assert_eq!(machine.output_level(kept), Some(LogicLevel::Low));
    assert_eq!(machine.output_level(established), Some(LogicLevel::Low));
    assert_eq!(machine.output_level(removed), None);
}

#[test]
fn changed_connectivity_emits_a_target_pulse() {
    let mut builder =
        NetworkBuilder::with_key(NetworkKey::from_u128(9), TimeDomainId::from_u128(10));
    let input = ExternalInputKey::<Level>::from_u128(1);
    let signal = builder
        .add_level_input(input, DiagnosticMeta::default())
        .unwrap();
    let level_output = ExternalOutputKey::<Level>::from_u128(2);
    builder
        .add_level_output(level_output, signal, DiagnosticMeta::default())
        .unwrap();
    let compiled = compile(builder);
    let mut machine = compiled.spawn(policy());
    init(
        &mut machine,
        Time::from_ticks(0),
        level_snapshot(&compiled, &[(input, LogicLevel::High)]),
    );

    let edge = NodeKey::from_u128(20);
    let in_port = InPortKey::<Level>::from_u128(21);
    let out_port = OutPortKey::<Pulse>::from_u128(22);
    let pulse_output = ExternalOutputKey::<Pulse>::from_u128(23);
    let prepared = prepare(
        &machine,
        machine
            .patch()
            .add_node(NodeDef::new(
                edge,
                NodeKind::rising_edge(EdgeConfig::new(EdgeInitialization::Assume(LogicLevel::Low))),
                NodePorts::new(vec![in_port.into()], vec![out_port.into()]),
                DiagnosticMeta::default(),
            ))
            .unwrap()
            .add_connection(ConnectionDef::new(
                ConnectionKey::from_u128(24),
                ConnectionEndpoint::external_input(input.into()),
                ConnectionEndpoint::node_input(in_port.into()),
                DiagnosticMeta::default(),
            ))
            .unwrap()
            .add_external_output(ExternalOutputDef::new(
                pulse_output.into(),
                AnySignalSourceKey::Pulse(SignalSourceKey::NodeOutput(out_port)),
                DiagnosticMeta::default(),
            ))
            .unwrap()
            .finish(),
    );
    let result = machine
        .apply(
            Transaction::advance(
                Time::from_ticks(2),
                machine.revision(),
                level_delta(prepared.resulting_compiled(), &[(input, LogicLevel::High)]),
            )
            .with_patch(prepared, ReconfigurationPolicy::RejectStateLoss)
            .unwrap(),
        )
        .unwrap_or_else(|failure| {
            panic!(
                "connectivity patch must commit: {}",
                failure.code().as_str()
            )
        });
    assert!(pulsed(&result, pulse_output));
    assert_eq!(
        event_revision(&result, pulse_output),
        Some(result.after_revision())
    );
}

#[test]
fn module_internal_loss_rejects_the_whole_transaction() {
    let pulse_in = ModuleInputKey::<Pulse>::from_u128(1);
    let level_out = ModuleOutputKey::<Level>::from_u128(2);
    let toggle = NodeKey::from_u128(3);
    let mut module = ModuleBuilder::<Domain>::new();
    let source = module
        .add_pulse_input(pulse_in, DiagnosticMeta::default())
        .unwrap();
    let stored = module
        .add_toggle(
            toggle,
            source,
            ToggleConfig::new(LogicLevel::High),
            DiagnosticMeta::default(),
        )
        .unwrap()
        .into_outputs();
    module
        .add_level_output(level_out, stored, DiagnosticMeta::default())
        .unwrap();
    let module = module.finish().require_artifact().unwrap();

    let mut builder =
        NetworkBuilder::with_key(NetworkKey::from_u128(11), TimeDomainId::from_u128(12));
    let level = ExternalInputKey::<Level>::from_u128(4);
    let pulse = ExternalInputKey::<Pulse>::from_u128(5);
    let level_signal = builder
        .add_level_input(level, DiagnosticMeta::default())
        .unwrap();
    let pulse_signal = builder
        .add_pulse_input(pulse, DiagnosticMeta::default())
        .unwrap();
    let transport = NodeKey::from_u128(6);
    let delayed = builder
        .add_transport_delay(
            transport,
            level_signal,
            TransportDelayConfig::new(delay(5), LogicLevel::Low),
            DiagnosticMeta::default(),
        )
        .unwrap()
        .into_outputs();
    let transport_output = ExternalOutputKey::<Level>::from_u128(7);
    builder
        .add_level_output(transport_output, delayed, DiagnosticMeta::default())
        .unwrap();
    let instance = ModuleInstanceKey::from_u128(8);
    let added = builder
        .instantiate(&module, instance, DiagnosticMeta::default())
        .unwrap()
        .bind_pulse(pulse_in, pulse_signal)
        .unwrap()
        .finish()
        .unwrap();
    let module_output = ExternalOutputKey::<Level>::from_u128(9);
    builder
        .add_level_output(
            module_output,
            added.level_output(level_out).unwrap(),
            DiagnosticMeta::default(),
        )
        .unwrap();
    let compiled = compile(builder);
    let mut machine = compiled.spawn(policy());
    let snapshot = compiled
        .input_snapshot()
        .set(level, LogicLevel::High)
        .unwrap();
    init(
        &mut machine,
        Time::from_ticks(0),
        snapshot.finish().unwrap(),
    );
    let before = machine.snapshot();
    let replacement = compiled
        .graph()
        .module_instances()
        .iter()
        .find(|item| item.key() == instance)
        .unwrap()
        .clone();
    let prepared = prepare(
        &machine,
        machine
            .patch()
            .replace_module_instance(
                instance,
                replacement,
                ModuleMigrationDirective::Explicit {
                    node_overrides: vec![ModuleNodeMigrationDirective::new(
                        toggle,
                        NodeMigrationDirective::Reset,
                    )],
                    internal_reassociations: Vec::new(),
                },
            )
            .unwrap()
            .finish(),
    );
    let failure = machine
        .apply(
            Transaction::advance(
                Time::from_ticks(10),
                machine.revision(),
                level_delta(prepared.resulting_compiled(), &[(level, LogicLevel::High)]),
            )
            .with_patch(prepared, ReconfigurationPolicy::RejectStateLoss)
            .unwrap(),
        )
        .expect_err("internal reset must reject");
    assert_eq!(
        failure.code().as_str(),
        "reconfiguration.state_loss_rejected"
    );
    assert!(matches!(
        failure.problem().primary(),
        SubjectRef::QualifiedNode(node) if node.node() == toggle
    ));
    assert_eq!(machine.snapshot(), before);
    assert_eq!(
        machine.output_level(transport_output),
        Some(LogicLevel::Low)
    );
    assert_eq!(
        machine
            .inspect_transport_delay(transport)
            .unwrap()
            .next_deadline(),
        Some(Time::from_ticks(5))
    );
}

#[test]
fn build_time_mismatches_reject_before_application() {
    let mut builder =
        NetworkBuilder::with_key(NetworkKey::from_u128(13), TimeDomainId::from_u128(14));
    let input = ExternalInputKey::<Level>::from_u128(1);
    let signal = builder
        .add_level_input(input, DiagnosticMeta::default())
        .unwrap();
    let output = ExternalOutputKey::<Level>::from_u128(2);
    builder
        .add_level_output(output, signal, DiagnosticMeta::default())
        .unwrap();
    let compiled = compile(builder);
    let mut machine = compiled.spawn(policy());
    init(
        &mut machine,
        Time::from_ticks(0),
        level_snapshot(&compiled, &[(input, LogicLevel::Low)]),
    );
    let before = machine.snapshot();
    let added = ExternalInputKey::<Level>::from_u128(9);
    let schema_patch = prepare(
        &machine,
        machine
            .patch()
            .add_external_input(ExternalInputDef::new(
                added.into(),
                DiagnosticMeta::default(),
            ))
            .unwrap()
            .finish(),
    );
    let stale_delta = level_delta(&compiled, &[(input, LogicLevel::Low)]);
    let mismatch = Transaction::advance(Time::from_ticks(1), machine.revision(), stale_delta)
        .with_patch(schema_patch.clone(), ReconfigurationPolicy::RejectStateLoss);
    assert!(matches!(
        mismatch,
        Err(TransactionBuildFailure::TargetSchemaMismatch { .. })
    ));
    let omitted = schema_patch
        .input_delta()
        .set(input, LogicLevel::Low)
        .unwrap()
        .finish()
        .unwrap();
    let missing = Transaction::advance(Time::from_ticks(1), machine.revision(), omitted)
        .with_patch(schema_patch, ReconfigurationPolicy::RejectStateLoss);
    assert!(matches!(
        missing,
        Err(TransactionBuildFailure::TargetSchemaMismatch { .. })
    ));

    let first = metadata_patch(&machine, "first");
    let committed = machine
        .apply(
            Transaction::advance(
                Time::from_ticks(1),
                machine.revision(),
                level_delta(first.resulting_compiled(), &[(input, LogicLevel::Low)]),
            )
            .with_patch(first, ReconfigurationPolicy::RejectStateLoss)
            .unwrap(),
        )
        .unwrap();
    let saved = committed.before_revision();
    let second = metadata_patch(&machine, "second");
    let revision_mismatch = Transaction::advance(
        Time::from_ticks(2),
        saved,
        level_delta(second.resulting_compiled(), &[(input, LogicLevel::Low)]),
    )
    .with_patch(second, ReconfigurationPolicy::RejectStateLoss);
    assert!(matches!(
        revision_mismatch,
        Err(TransactionBuildFailure::BaseRevisionMismatch { .. })
    ));
    assert_eq!(machine.revision(), committed.after_revision());
    assert_ne!(machine.snapshot(), before);
}

#[test]
fn stale_fingerprint_and_revision_fail_at_application() {
    let build = |extra: bool| {
        let mut builder =
            NetworkBuilder::with_key(NetworkKey::from_u128(21), TimeDomainId::from_u128(22));
        let input = ExternalInputKey::<Level>::from_u128(1);
        let signal = builder
            .add_level_input(input, DiagnosticMeta::default())
            .unwrap();
        let output = ExternalOutputKey::<Level>::from_u128(2);
        let driven = if extra {
            builder
                .add_not_with_ports(
                    NodeKey::from_u128(3),
                    InPortKey::from_u128(4),
                    OutPortKey::from_u128(5),
                    signal,
                    DiagnosticMeta::default(),
                )
                .unwrap()
                .into_outputs()
        } else {
            signal
        };
        builder
            .add_level_output(output, driven, DiagnosticMeta::default())
            .unwrap();
        (compile(builder), input)
    };
    let (left, input) = build(false);
    let (right, _) = build(true);
    let mut source = left.spawn(policy());
    init(
        &mut source,
        Time::from_ticks(0),
        level_snapshot(&left, &[(input, LogicLevel::Low)]),
    );
    let prepared = metadata_patch(&source, "renamed");
    let transaction = Transaction::advance(
        Time::from_ticks(1),
        source.revision(),
        level_delta(prepared.resulting_compiled(), &[(input, LogicLevel::Low)]),
    )
    .with_patch(prepared, ReconfigurationPolicy::RejectStateLoss)
    .unwrap();
    let mut other = right.spawn(policy());
    init(
        &mut other,
        Time::from_ticks(0),
        level_snapshot(&right, &[(input, LogicLevel::High)]),
    );
    let before = other.snapshot();
    let stale = other
        .apply(transaction.clone())
        .expect_err("foreign topology must be stale");
    assert_eq!(
        stale.code().as_str(),
        "reconfiguration.stale_prepared_patch"
    );
    assert_eq!(other.snapshot(), before);

    source.apply(transaction.clone()).unwrap_or_else(|failure| {
        panic!("matching patch must commit: {}", failure.code().as_str())
    });
    let stale_revision = source
        .apply(transaction)
        .expect_err("a repeated patch transaction must be stale");
    assert_eq!(stale_revision.code().as_str(), "runtime.stale_revision");
}

#[test]
fn wrong_execution_digest_rejects_before_candidate_work() {
    let mut builder =
        NetworkBuilder::with_key(NetworkKey::from_u128(30), TimeDomainId::from_u128(31));
    let input = ExternalInputKey::<Level>::from_u128(1);
    let signal = builder
        .add_level_input(input, DiagnosticMeta::default())
        .unwrap();
    let transport = NodeKey::from_u128(2);
    let delayed = builder
        .add_transport_delay(
            transport,
            signal,
            TransportDelayConfig::new(delay(4), LogicLevel::Low),
            DiagnosticMeta::default(),
        )
        .unwrap()
        .into_outputs();
    builder
        .add_level_output(
            ExternalOutputKey::from_u128(3),
            delayed,
            DiagnosticMeta::default(),
        )
        .unwrap();
    let compiled = compile(builder);
    let mut machine = compiled.spawn(policy());
    init(
        &mut machine,
        Time::from_ticks(0),
        level_snapshot(&compiled, &[(input, LogicLevel::High)]),
    );
    let prepared = metadata_patch(&machine, "renamed");
    let before = machine.snapshot();
    let mut other = compiled.spawn(policy());
    init(
        &mut other,
        Time::from_ticks(0),
        level_snapshot(&compiled, &[(input, LogicLevel::Low)]),
    );
    let foreign = other.execution_state_digest();
    assert_ne!(foreign, machine.execution_state_digest());
    let failure = machine
        .apply(
            Transaction::advance(
                Time::from_ticks(9),
                machine.revision(),
                level_delta(prepared.resulting_compiled(), &[(input, LogicLevel::High)]),
            )
            .with_patch(prepared, ReconfigurationPolicy::RejectStateLoss)
            .unwrap()
            .expect_execution_state(foreign),
        )
        .expect_err("wrong execution digest must reject");
    assert_eq!(failure.code().as_str(), "runtime.stale_execution_state");
    assert_eq!(machine.snapshot(), before);
    assert_eq!(
        machine
            .inspect_transport_delay(transport)
            .unwrap()
            .next_deadline(),
        Some(Time::from_ticks(4))
    );
}

#[test]
fn forecast_matches_apply_and_does_not_publish() {
    let mut builder =
        NetworkBuilder::with_key(NetworkKey::from_u128(40), TimeDomainId::from_u128(41));
    let input = ExternalInputKey::<Level>::from_u128(1);
    let signal = builder
        .add_level_input(input, DiagnosticMeta::default())
        .unwrap();
    builder
        .add_level_output(
            ExternalOutputKey::from_u128(2),
            signal,
            DiagnosticMeta::default(),
        )
        .unwrap();
    let compiled = compile(builder);
    let mut machine = compiled.spawn(policy());
    init(
        &mut machine,
        Time::from_ticks(0),
        level_snapshot(&compiled, &[(input, LogicLevel::Low)]),
    );
    let prepared = metadata_patch(&machine, "forecast");
    let transaction = Transaction::advance(
        Time::from_ticks(2),
        machine.revision(),
        level_delta(prepared.resulting_compiled(), &[(input, LogicLevel::High)]),
    )
    .with_patch(prepared, ReconfigurationPolicy::RejectStateLoss)
    .unwrap();
    let before = machine.snapshot();
    let forecast = machine
        .forecast(transaction.clone())
        .unwrap_or_else(|failure| panic!("forecast must succeed: {}", failure.code().as_str()));
    assert_eq!(machine.snapshot(), before);
    assert!(forecast.result().migration().is_some());
    assert_ne!(forecast.state().revision(), machine.revision());
    let applied = machine
        .apply(transaction)
        .unwrap_or_else(|failure| panic!("apply must succeed: {}", failure.code().as_str()));
    assert_eq!(applied.after_revision(), forecast.result().after_revision());
    assert_eq!(
        applied.after_execution_digest(),
        forecast.result().after_execution_digest()
    );
    assert_eq!(
        applied.output_events().len(),
        forecast.result().output_events().len()
    );
    assert_eq!(machine.revision(), forecast.state().revision());
    assert_eq!(machine.fingerprint(), forecast.state().fingerprint());
    assert_eq!(
        machine.execution_state_digest(),
        forecast.state().execution_state_digest()
    );
}

#[test]
fn committed_snapshot_restores_and_later_replay_matches() {
    let mut builder =
        NetworkBuilder::with_key(NetworkKey::from_u128(50), TimeDomainId::from_u128(51));
    let input = ExternalInputKey::<Level>::from_u128(1);
    let signal = builder
        .add_level_input(input, DiagnosticMeta::default())
        .unwrap();
    let output = ExternalOutputKey::<Level>::from_u128(2);
    builder
        .add_level_output(output, signal, DiagnosticMeta::default())
        .unwrap();
    let compiled = compile(builder);
    let mut machine = compiled.spawn(policy());
    init(
        &mut machine,
        Time::from_ticks(0),
        level_snapshot(&compiled, &[(input, LogicLevel::Low)]),
    );
    let prepared = metadata_patch(&machine, "committed");
    let target = prepared.resulting_compiled().clone();
    machine
        .apply(
            Transaction::advance(
                Time::from_ticks(1),
                machine.revision(),
                level_delta(&target, &[(input, LogicLevel::Low)]),
            )
            .with_patch(prepared, ReconfigurationPolicy::RejectStateLoss)
            .unwrap(),
        )
        .unwrap();
    let patch_transaction = Transaction::advance(
        Time::from_ticks(1),
        machine.revision(),
        level_delta(&target, &[(input, LogicLevel::High)]),
    )
    .with_patch(
        metadata_patch(&machine, "again"),
        ReconfigurationPolicy::RejectStateLoss,
    )
    .unwrap();
    let before_recording = machine.snapshot();
    let rejected = machine
        .apply_recorded(patch_transaction)
        .expect_err("patch recording must reject");
    assert_eq!(
        rejected.code().as_str(),
        "replay.patch_preparation_diverged"
    );
    assert_eq!(machine.snapshot(), before_recording);

    let restored = target
        .restore(machine.snapshot(), policy())
        .unwrap_or_else(|failure| panic!("target restore must succeed: {failure:?}"));
    assert_eq!(restored.revision(), machine.revision());
    assert_eq!(restored.fingerprint(), machine.fingerprint());
    assert_eq!(
        restored.execution_state_digest(),
        machine.execution_state_digest()
    );
    assert_eq!(restored.output_level(output), machine.output_level(output));

    let later = Transaction::advance(
        Time::from_ticks(3),
        machine.revision(),
        level_delta(&target, &[(input, LogicLevel::High)]),
    );
    let mut continued = target.restore(machine.snapshot(), policy()).unwrap();
    let log = record_replay_log(&mut continued, [later.clone()]).unwrap_or_else(|failure| {
        panic!(
            "patch-free recording must succeed: {}",
            failure.code().as_str()
        )
    });
    let mut replayed = target.restore(machine.snapshot(), policy()).unwrap();
    replayed.replay_log(&log).unwrap_or_else(|failure| {
        panic!(
            "patch-free replay must succeed: {}",
            failure.code().as_str()
        )
    });
    assert_eq!(
        replayed.execution_state_digest(),
        continued.execution_state_digest()
    );
    assert_eq!(replayed.output_level(output), Some(LogicLevel::High));
    assert!(
        log.frames()
            .iter()
            .all(|frame| !frame.transaction().carries_patch())
    );
}

#[test]
fn initialization_patch_commits_revision_with_established_outputs() {
    let mut builder =
        NetworkBuilder::with_key(NetworkKey::from_u128(60), TimeDomainId::from_u128(61));
    let input = ExternalInputKey::<Level>::from_u128(1);
    let signal = builder
        .add_level_input(input, DiagnosticMeta::default())
        .unwrap();
    let inverted = builder
        .add_not(NodeKey::from_u128(2), signal, DiagnosticMeta::default())
        .unwrap()
        .into_outputs();
    let output = ExternalOutputKey::<Level>::from_u128(3);
    builder
        .add_level_output(output, inverted, DiagnosticMeta::default())
        .unwrap();
    let compiled = compile(builder);
    let mut machine = compiled.spawn(policy());
    let prepared = metadata_patch(&machine, "initial");
    let snapshot = level_snapshot(prepared.resulting_compiled(), &[(input, LogicLevel::High)]);
    let result = machine
        .apply(
            Transaction::initialize(Time::from_ticks(0), machine.revision(), snapshot)
                .with_patch(prepared, ReconfigurationPolicy::RejectStateLoss)
                .unwrap(),
        )
        .unwrap_or_else(|failure| {
            panic!(
                "initialization patch must commit: {}",
                failure.code().as_str()
            )
        });
    assert!(result.migration().is_some());
    assert_ne!(result.before_revision(), result.after_revision());
    assert_eq!(machine.output_level(output), Some(LogicLevel::Low));
    assert!(result.output_events().iter().any(|event| matches!(
        event,
        OutputEvent::LevelEstablished { output: found, value: LogicLevel::Low, .. } if *found == output
    )));
    let followed = machine
        .apply(Transaction::advance(
            Time::from_ticks(1),
            machine.revision(),
            level_delta(machine.compiled(), &[(input, LogicLevel::Low)]),
        ))
        .unwrap();
    assert!(followed.migration().is_none());
    assert_eq!(followed.before_revision(), followed.after_revision());
}

#[test]
fn reset_discards_old_state_causes_and_restores_after_source_removal() {
    let mut builder =
        NetworkBuilder::with_key(NetworkKey::from_u128(70), TimeDomainId::from_u128(71));
    let input = ExternalInputKey::<Pulse>::from_u128(1);
    let pulse = builder
        .add_pulse_input(input, DiagnosticMeta::default())
        .unwrap();
    let node = NodeKey::from_u128(2);
    let signal = builder
        .add_toggle(
            node,
            pulse,
            ToggleConfig::new(LogicLevel::Low),
            DiagnosticMeta::default(),
        )
        .unwrap()
        .into_outputs();
    let output = ExternalOutputKey::<Level>::from_u128(3);
    builder
        .add_level_output(output, signal, DiagnosticMeta::default())
        .unwrap();
    let compiled = compile(builder);
    let mut machine = compiled.spawn(policy());
    init(
        &mut machine,
        Time::from_ticks(0),
        compiled
            .input_snapshot()
            .pulse(input, PulseCount::new(1))
            .unwrap()
            .finish()
            .unwrap(),
    );
    assert_eq!(machine.output_level(output), Some(LogicLevel::High));
    let prepared = prepare(
        &machine,
        machine
            .patch()
            .replace_node(
                node,
                node_def(&compiled, node),
                NodeMigrationDirective::Reset,
            )
            .unwrap()
            .finish(),
    );
    let target = prepared.resulting_compiled().clone();
    machine
        .apply(
            Transaction::advance(
                Time::from_ticks(1),
                machine.revision(),
                level_delta(&target, &[]),
            )
            .with_patch(prepared, ReconfigurationPolicy::AllowReportedStateLoss)
            .unwrap(),
        )
        .unwrap();
    assert_eq!(machine.output_level(output), Some(LogicLevel::Low));
    // Reset establishes the target initial state rather than retaining the inversion that set High.
    assert!(
        machine
            .inspect_toggle(node)
            .unwrap()
            .latest_inversion()
            .is_none()
    );
    target.restore(machine.snapshot(), policy()).unwrap();
}

#[test]
fn inherited_input_and_retimed_pending_work_restore_against_target() {
    let mut builder =
        NetworkBuilder::with_key(NetworkKey::from_u128(72), TimeDomainId::from_u128(73));
    let input = ExternalInputKey::<Pulse>::from_u128(1);
    let pulse = builder
        .add_pulse_input(input, DiagnosticMeta::default())
        .unwrap();
    let node = NodeKey::from_u128(2);
    let signal = builder
        .add_pulse_delay(
            node,
            pulse,
            PulseDelayConfig::new(delay(10)),
            DiagnosticMeta::default(),
        )
        .unwrap()
        .into_outputs();
    let output = ExternalOutputKey::<Pulse>::from_u128(3);
    builder
        .add_pulse_output(output, signal, DiagnosticMeta::default())
        .unwrap();
    let compiled = compile(builder);
    let mut machine = compiled.spawn(policy());
    init(
        &mut machine,
        Time::from_ticks(0),
        compiled
            .input_snapshot()
            .pulse(input, PulseCount::new(2))
            .unwrap()
            .finish()
            .unwrap(),
    );
    let prepared = prepare(
        &machine,
        machine
            .patch()
            .replace_node(
                node,
                node_def(&compiled, node),
                NodeMigrationDirective::PulseDelay(PulseDelayMigration::RestartFromPatchTime),
            )
            .unwrap()
            .finish(),
    );
    let target = prepared.resulting_compiled().clone();
    let committed = machine
        .apply(
            Transaction::advance(
                Time::from_ticks(4),
                machine.revision(),
                level_delta(&target, &[]),
            )
            .with_patch(prepared, ReconfigurationPolicy::AllowReportedStateLoss)
            .unwrap(),
        )
        .unwrap();
    assert_eq!(
        machine.inspect_pulse_delay(node).unwrap().pending()[0].deadline(),
        Time::from_ticks(14)
    );
    let inspection = machine.inspect_pulse_delay(node).unwrap();
    let pending = inspection.pending()[0].cause();
    let provenance = committed.provenance();
    let mossignal::CauseInspection::PendingPulseDelay { supporters, .. } =
        provenance.inspect(pending).unwrap()
    else {
        panic!("pending group must expose its scheduling fact");
    };
    assert!(supporters.iter().any(|cause| matches!(provenance.inspect(*cause).unwrap(), mossignal::CauseInspection::Migration { rule, .. } if rule.contains("RestartFromPatchTime"))));
    let snapshot = machine.snapshot();
    let mut restored = target.restore(machine.snapshot(), policy()).unwrap();
    assert_eq!(restored.snapshot(), snapshot);
    let transaction = Transaction::advance(
        Time::from_ticks(14),
        machine.revision(),
        level_delta(&target, &[]),
    );
    let result = restored.apply(transaction.clone()).unwrap();
    assert!(pulsed(&result, output));
    machine.apply(transaction).unwrap();
    assert_eq!(machine.snapshot(), restored.snapshot());
}

#[test]
fn removed_level_input_reports_the_reached_valuation_as_loss() {
    let mut builder =
        NetworkBuilder::with_key(NetworkKey::from_u128(74), TimeDomainId::from_u128(75));
    let input = ExternalInputKey::<Level>::from_u128(1);
    builder
        .add_level_input(input, DiagnosticMeta::default())
        .unwrap();
    let compiled = compile(builder);
    let mut machine = compiled.spawn(policy());
    init(
        &mut machine,
        Time::from_ticks(0),
        level_snapshot(&compiled, &[(input, LogicLevel::High)]),
    );
    let prepared = prepare(
        &machine,
        machine
            .patch()
            .remove_external_input(input.into())
            .unwrap()
            .finish(),
    );
    let target = prepared.resulting_compiled().clone();
    let transaction = Transaction::advance(
        Time::from_ticks(1),
        machine.revision(),
        level_delta(&target, &[]),
    );
    let before = machine.snapshot();
    let failure = machine
        .apply(
            transaction
                .clone()
                .with_patch(prepared.clone(), ReconfigurationPolicy::RejectStateLoss)
                .unwrap(),
        )
        .unwrap_err();
    assert_eq!(
        failure.code().as_str(),
        "reconfiguration.state_loss_rejected"
    );
    assert_eq!(machine.snapshot(), before);
    let result = machine
        .apply(
            transaction
                .with_patch(prepared, ReconfigurationPolicy::AllowReportedStateLoss)
                .unwrap(),
        )
        .unwrap();
    assert!(
        result
            .migration()
            .unwrap()
            .losses()
            .iter()
            .any(|loss| loss.fact() == "level_valuation")
    );
    target.restore(machine.snapshot(), policy()).unwrap();
}

#[test]
fn canceled_groups_have_individual_reported_event_identities() {
    let mut builder =
        NetworkBuilder::with_key(NetworkKey::from_u128(76), TimeDomainId::from_u128(77));
    let input = ExternalInputKey::<Pulse>::from_u128(1);
    let signal = builder
        .add_pulse_input(input, DiagnosticMeta::default())
        .unwrap();
    let node = NodeKey::from_u128(2);
    builder
        .add_pulse_delay(
            node,
            signal,
            PulseDelayConfig::new(delay(10)),
            DiagnosticMeta::default(),
        )
        .unwrap();
    let compiled = compile(builder);
    let mut machine = compiled.spawn(policy());
    init(
        &mut machine,
        Time::from_ticks(0),
        compiled
            .input_snapshot()
            .pulse(input, PulseCount::ONE)
            .unwrap()
            .finish()
            .unwrap(),
    );
    machine
        .apply(Transaction::advance(
            Time::from_ticks(1),
            machine.revision(),
            compiled
                .input_delta()
                .pulse(input, PulseCount::ONE)
                .unwrap()
                .finish()
                .unwrap(),
        ))
        .unwrap();
    let prepared = prepare(
        &machine,
        machine
            .patch()
            .replace_node(
                node,
                node_def(&compiled, node),
                NodeMigrationDirective::PulseDelay(PulseDelayMigration::CancelPending),
            )
            .unwrap()
            .finish(),
    );
    let result = machine
        .apply(
            Transaction::advance(
                Time::from_ticks(2),
                machine.revision(),
                level_delta(prepared.resulting_compiled(), &[]),
            )
            .with_patch(prepared, ReconfigurationPolicy::AllowReportedStateLoss)
            .unwrap(),
        )
        .unwrap();
    assert_eq!(
        result.migration().unwrap().events().len(),
        2,
        "every canceled source event needs one report record"
    );
    assert_eq!(
        result
            .migration()
            .unwrap()
            .losses()
            .iter()
            .filter(|loss| loss.fact() == "pending_group")
            .count(),
        2
    );
    assert_eq!(
        result
            .migration()
            .unwrap()
            .events()
            .iter()
            .map(|event| event.origin())
            .collect::<Vec<_>>(),
        vec![Some(0), Some(1)]
    );
}

#[test]
fn resetting_an_active_latch_publishes_episode_termination() {
    let mut builder =
        NetworkBuilder::with_key(NetworkKey::from_u128(78), TimeDomainId::from_u128(79));
    let set = ExternalInputKey::<Level>::from_u128(1);
    let reset = ExternalInputKey::<Level>::from_u128(2);
    let set_signal = builder
        .add_level_input(set, DiagnosticMeta::default())
        .unwrap();
    let reset_signal = builder
        .add_level_input(reset, DiagnosticMeta::default())
        .unwrap();
    let node = NodeKey::from_u128(3);
    builder
        .add_level_set_reset_latch(
            node,
            set_signal,
            reset_signal,
            mossignal::LevelSetResetConfig::new(
                LogicLevel::Low,
                mossignal::ConflictPolicy::RetainAndDiagnose,
            ),
            DiagnosticMeta::default(),
        )
        .unwrap();
    let compiled = compile(builder);
    let mut machine = compiled.spawn(policy());
    let initial = init(
        &mut machine,
        Time::from_ticks(0),
        level_snapshot(
            &compiled,
            &[(set, LogicLevel::High), (reset, LogicLevel::High)],
        ),
    );
    let episode = initial.diagnostic_episode_changes()[0].identity();
    let prepared = prepare(
        &machine,
        machine
            .patch()
            .replace_node(
                node,
                node_def(&compiled, node),
                NodeMigrationDirective::Reset,
            )
            .unwrap()
            .finish(),
    );
    let result = machine
        .apply(
            Transaction::advance(
                Time::from_ticks(1),
                machine.revision(),
                level_delta(
                    prepared.resulting_compiled(),
                    &[(set, LogicLevel::Low), (reset, LogicLevel::Low)],
                ),
            )
            .with_patch(prepared, ReconfigurationPolicy::AllowReportedStateLoss)
            .unwrap(),
        )
        .unwrap();
    assert!(
        result
            .diagnostic_episode_changes()
            .iter()
            .any(|change| change.identity() == episode
                && change.kind() == mossignal::DiagnosticEpisodeChangeKind::Terminated)
    );
    assert!(
        result
            .migration()
            .unwrap()
            .losses()
            .iter()
            .any(|loss| loss.fact() == "diagnostic_episode")
    );
    machine
        .compiled()
        .restore(machine.snapshot(), policy())
        .unwrap();
}

#[test]
fn reassociated_latch_keeps_its_active_interval_on_the_successor_owner() {
    let mut builder =
        NetworkBuilder::with_key(NetworkKey::from_u128(96), TimeDomainId::from_u128(97));
    let set = ExternalInputKey::<Level>::from_u128(1);
    let reset = ExternalInputKey::<Level>::from_u128(2);
    let set_signal = builder
        .add_level_input(set, DiagnosticMeta::default())
        .unwrap();
    let reset_signal = builder
        .add_level_input(reset, DiagnosticMeta::default())
        .unwrap();
    let source = NodeKey::from_u128(3);
    builder
        .add_level_set_reset_latch(
            source,
            set_signal,
            reset_signal,
            mossignal::LevelSetResetConfig::new(
                LogicLevel::Low,
                mossignal::ConflictPolicy::RetainAndDiagnose,
            ),
            DiagnosticMeta::default(),
        )
        .unwrap();
    let second_source = NodeKey::from_u128(8);
    builder
        .add_level_set_reset_latch(
            second_source,
            set_signal,
            reset_signal,
            mossignal::LevelSetResetConfig::new(
                LogicLevel::Low,
                mossignal::ConflictPolicy::RetainAndDiagnose,
            ),
            DiagnosticMeta::default(),
        )
        .unwrap();
    let compiled = compile(builder);
    let mut machine = compiled.spawn(policy());
    init(
        &mut machine,
        Time::from_ticks(0),
        level_snapshot(
            &compiled,
            &[(set, LogicLevel::High), (reset, LogicLevel::High)],
        ),
    );
    let old = node_def(&compiled, source);
    let second_old = node_def(&compiled, second_source);
    let successor = NodeKey::from_u128(4);
    let second_successor = NodeKey::from_u128(2);
    let prepared = prepare(
        &machine,
        machine
            .patch()
            .remove_node(source)
            .unwrap()
            .add_node(NodeDef::new(
                successor,
                old.kind().clone(),
                old.ports().clone(),
                DiagnosticMeta::default(),
            ))
            .unwrap()
            .reassociate(mossignal::SubjectReassociation::Node {
                from: source,
                to: successor,
                migration: NodeMigrationDirective::Standard,
            })
            .unwrap()
            .remove_node(second_source)
            .unwrap()
            .add_node(NodeDef::new(
                second_successor,
                second_old.kind().clone(),
                second_old.ports().clone(),
                DiagnosticMeta::default(),
            ))
            .unwrap()
            .reassociate(mossignal::SubjectReassociation::Node {
                from: second_source,
                to: second_successor,
                migration: NodeMigrationDirective::Standard,
            })
            .unwrap()
            .finish(),
    );
    let target = prepared.resulting_compiled().clone();
    let result = machine
        .apply(
            Transaction::advance(
                Time::from_ticks(5),
                machine.revision(),
                level_delta(&target, &[]),
            )
            .with_patch(prepared, ReconfigurationPolicy::RejectStateLoss)
            .unwrap(),
        )
        .unwrap();
    assert!(
        !result
            .diagnostic_episode_changes()
            .iter()
            .any(|change| change.kind() == mossignal::DiagnosticEpisodeChangeKind::Terminated)
    );
    let episodes = machine.active_diagnostic_episodes().unwrap();
    assert_eq!(episodes.len(), 2);
    assert!(
        episodes
            .iter()
            .all(|episode| episode.began_at() == Time::from_ticks(0))
    );
    assert!(
        episodes
            .iter()
            .any(|episode| episode.condition().owner() == &mossignal::NodeSubject::Node(successor))
    );
    assert!(
        episodes.iter().any(|episode| episode.condition().owner()
            == &mossignal::NodeSubject::Node(second_successor))
    );
    // Report order follows source identity, even when successor keys reverse that order.
    assert_eq!(
        result
            .migration()
            .unwrap()
            .states()
            .iter()
            .map(|state| state.subject().clone())
            .collect::<Vec<_>>(),
        vec![
            SubjectRef::Node(successor),
            SubjectRef::Node(second_successor)
        ]
    );
    let restored = target.restore(machine.snapshot(), policy()).unwrap();
    assert_eq!(restored.snapshot(), machine.snapshot());
}

#[test]
fn periodic_recomputation_keeps_the_anchor_relative_ordinal() {
    let mut builder =
        NetworkBuilder::with_key(NetworkKey::from_u128(80), TimeDomainId::from_u128(81));
    let input = ExternalInputKey::<Level>::from_u128(1);
    let enable = builder
        .add_level_input(input, DiagnosticMeta::default())
        .unwrap();
    let node = NodeKey::from_u128(2);
    builder
        .add_periodic(
            node,
            enable,
            mossignal::PeriodicConfig::new(
                delay(5),
                mossignal::FirstEmissionPolicy::AfterFirstPeriod,
                mossignal::ReenablePhasePolicy::RestartPhase,
            ),
            DiagnosticMeta::default(),
        )
        .unwrap();
    let compiled = compile(builder);
    let mut machine = compiled.spawn(policy());
    init(
        &mut machine,
        Time::from_ticks(0),
        level_snapshot(&compiled, &[(input, LogicLevel::High)]),
    );
    let prepared = prepare(
        &machine,
        machine
            .patch()
            .replace_node(
                node,
                node_def(&compiled, node),
                NodeMigrationDirective::Periodic(
                    mossignal::PeriodicMigration::RecomputeFromExistingAnchor,
                ),
            )
            .unwrap()
            .finish(),
    );
    machine
        .apply(
            Transaction::advance(
                Time::from_ticks(12),
                machine.revision(),
                level_delta(prepared.resulting_compiled(), &[]),
            )
            .with_patch(prepared, ReconfigurationPolicy::AllowReportedStateLoss)
            .unwrap(),
        )
        .unwrap();
    let pending = machine
        .inspect_periodic(node)
        .unwrap()
        .pending()
        .unwrap()
        .clone();
    assert_eq!(pending.ordinal(), 3);
    assert_eq!(pending.deadline(), Time::from_ticks(15));
    let mut restored = machine
        .compiled()
        .restore(machine.snapshot(), policy())
        .unwrap();
    let transaction = Transaction::advance(
        Time::from_ticks(15),
        machine.revision(),
        level_delta(machine.compiled(), &[]),
    );
    machine.apply(transaction.clone()).unwrap();
    restored.apply(transaction).unwrap();
    assert_eq!(machine.snapshot(), restored.snapshot());
}

fn resettable_hold_network(reset_to: LogicLevel) -> mossignal::CompiledNetwork<Domain> {
    let mut builder =
        NetworkBuilder::with_key(NetworkKey::from_u128(82), TimeDomainId::from_u128(83));
    let value = builder
        .add_level_input(ExternalInputKey::from_u128(1), DiagnosticMeta::default())
        .unwrap();
    let reset = builder
        .add_level_input(ExternalInputKey::from_u128(2), DiagnosticMeta::default())
        .unwrap();
    let sample = builder
        .add_pulse_input(ExternalInputKey::from_u128(3), DiagnosticMeta::default())
        .unwrap();
    let held = builder
        .add_level_resettable_sample_hold(
            ModuleInstanceKey::from_u128(4),
            value,
            sample,
            reset,
            LogicLevel::Low,
            reset_to,
            DiagnosticMeta::default(),
        )
        .unwrap()
        .into_outputs();
    builder
        .add_level_output(
            ExternalOutputKey::from_u128(5),
            held,
            DiagnosticMeta::default(),
        )
        .unwrap();
    compile(builder)
}

#[test]
fn reset_to_migration_uses_pre_patch_reset_before_same_time_input() {
    let base = resettable_hold_network(LogicLevel::Low);
    let target_template = resettable_hold_network(LogicLevel::High);
    let replacement = target_template.graph().module_instances()[0].clone();
    for previous_reset in [LogicLevel::Low, LogicLevel::High] {
        let mut machine = base.spawn(policy());
        init(
            &mut machine,
            Time::from_ticks(0),
            level_snapshot(
                &base,
                &[
                    (ExternalInputKey::from_u128(1), LogicLevel::Low),
                    (ExternalInputKey::from_u128(2), previous_reset),
                ],
            ),
        );
        let prepared = prepare(
            &machine,
            machine
                .patch()
                .replace_module_instance(
                    ModuleInstanceKey::from_u128(4),
                    replacement.clone(),
                    ModuleMigrationDirective::Standard,
                )
                .unwrap()
                .finish(),
        );
        let target = prepared.resulting_compiled().clone();
        machine
            .apply(
                Transaction::advance(
                    Time::from_ticks(1),
                    machine.revision(),
                    level_delta(
                        &target,
                        &[(ExternalInputKey::from_u128(2), LogicLevel::Low)],
                    ),
                )
                .with_patch(prepared, ReconfigurationPolicy::RejectStateLoss)
                .unwrap(),
            )
            .unwrap();
        assert_eq!(
            machine.output_level(ExternalOutputKey::from_u128(5)),
            Some(previous_reset),
            "migration reads the reached reset level before the target delta"
        );
        let mut restored = target
            .restore(machine.snapshot(), policy())
            .unwrap_or_else(|failure| panic!("restore failed: {:?}", failure.problem()));
        let transaction = Transaction::advance(
            Time::from_ticks(2),
            machine.revision(),
            level_delta(&target, &[]),
        );
        machine.apply(transaction.clone()).unwrap();
        restored.apply(transaction).unwrap();
        assert_eq!(machine.snapshot(), restored.snapshot());
    }
}

#[test]
fn removed_state_owner_has_an_explicit_state_report() {
    let mut builder =
        NetworkBuilder::with_key(NetworkKey::from_u128(84), TimeDomainId::from_u128(85));
    let input = ExternalInputKey::<Pulse>::from_u128(1);
    let pulse = builder
        .add_pulse_input(input, DiagnosticMeta::default())
        .unwrap();
    let node = NodeKey::from_u128(2);
    builder
        .add_toggle(
            node,
            pulse,
            ToggleConfig::new(LogicLevel::High),
            DiagnosticMeta::default(),
        )
        .unwrap();
    let compiled = compile(builder);
    let mut machine = compiled.spawn(policy());
    init(
        &mut machine,
        Time::from_ticks(0),
        compiled.input_snapshot().finish().unwrap(),
    );
    let mut patch = machine.patch().remove_node(node).unwrap();
    for connection in compiled.graph().connections() {
        patch = patch.remove_connection(connection.key()).unwrap();
    }
    let prepared = prepare(&machine, patch.finish());
    let result = machine
        .apply(
            Transaction::advance(
                Time::from_ticks(1),
                machine.revision(),
                level_delta(prepared.resulting_compiled(), &[]),
            )
            .with_patch(prepared, ReconfigurationPolicy::AllowReportedStateLoss)
            .unwrap(),
        )
        .unwrap();
    assert!(
        result
            .migration()
            .unwrap()
            .states()
            .iter()
            .any(|state| state.subject() == &SubjectRef::Node(node)
                && state.outcome() == StateOutcome::Removed)
    );
    machine
        .compiled()
        .restore(machine.snapshot(), policy())
        .unwrap();
}

#[test]
fn target_input_establishment_is_required_and_retained() {
    let builder = NetworkBuilder::with_key(NetworkKey::from_u128(86), TimeDomainId::from_u128(87));
    let compiled = compile(builder);
    let mut machine = compiled.spawn(policy());
    init(
        &mut machine,
        Time::from_ticks(0),
        compiled.input_snapshot().finish().unwrap(),
    );
    let input = ExternalInputKey::<Level>::from_u128(1);
    let prepared = prepare(
        &machine,
        machine
            .patch()
            .add_external_input(ExternalInputDef::new(
                input.into(),
                DiagnosticMeta::default(),
            ))
            .unwrap()
            .finish(),
    );
    let target = prepared.resulting_compiled().clone();
    let before = machine.snapshot();
    assert!(
        Transaction::advance(
            Time::from_ticks(1),
            machine.revision(),
            level_delta(&target, &[])
        )
        .with_patch(prepared.clone(), ReconfigurationPolicy::RejectStateLoss)
        .is_err()
    );
    assert_eq!(machine.snapshot(), before);
    machine
        .apply(
            Transaction::advance(
                Time::from_ticks(1),
                machine.revision(),
                level_delta(&target, &[(input, LogicLevel::High)]),
            )
            .with_patch(prepared, ReconfigurationPolicy::RejectStateLoss)
            .unwrap(),
        )
        .unwrap();
    machine
        .apply(Transaction::advance(
            Time::from_ticks(2),
            machine.revision(),
            level_delta(&target, &[]),
        ))
        .unwrap();
    target.restore(machine.snapshot(), policy()).unwrap();
}

fn operation_cost(compiled: &mossignal::CompiledNetwork<Domain>) -> u64 {
    let constrained = RuntimePolicy::builder()
        .max_internal_reactions(1000)
        .max_evaluated_operations(0)
        .max_pending_events(1000)
        .max_events_created_per_transaction(10000)
        .max_required_provenance_growth(100000)
        .build()
        .unwrap();
    let mut machine = compiled.spawn(constrained);
    let failure = machine
        .apply(Transaction::initialize(
            Time::from_ticks(0),
            machine.revision(),
            compiled.input_snapshot().finish().unwrap(),
        ))
        .unwrap_err();
    match failure.evidence() {
        mossignal::RuntimeFailureEvidence::BudgetExceeded { consumed, .. } => *consumed,
        _ => panic!("zero operation budget must reject evaluation"),
    }
}

#[test]
fn operation_budget_counts_old_and_target_graphs_separately() {
    let mut builder =
        NetworkBuilder::with_key(NetworkKey::from_u128(88), TimeDomainId::from_u128(89));
    let input = ExternalInputKey::<Pulse>::from_u128(1);
    let pulse = builder
        .add_pulse_input(input, DiagnosticMeta::default())
        .unwrap();
    builder
        .add_pulse_delay(
            NodeKey::from_u128(2),
            pulse,
            PulseDelayConfig::new(delay(5)),
            DiagnosticMeta::default(),
        )
        .unwrap();
    let removed = NodeKey::from_u128(3);
    builder
        .add_constant(removed, LogicLevel::Low, DiagnosticMeta::default())
        .unwrap();
    let compiled = compile(builder);
    let preparer = compiled.spawn(policy());
    let prepared = prepare(
        &preparer,
        preparer.patch().remove_node(removed).unwrap().finish(),
    );
    let target = prepared.resulting_compiled();
    let source_cost = operation_cost(&compiled);
    let target_cost = operation_cost(target);
    assert!(source_cost > target_cost);
    let limit = source_cost + target_cost - 1;
    let constrained = RuntimePolicy::builder()
        .max_internal_reactions(1000)
        .max_evaluated_operations(limit)
        .max_pending_events(1000)
        .max_events_created_per_transaction(10000)
        .max_required_provenance_growth(100000)
        .build()
        .unwrap();
    let mut machine = compiled.spawn(constrained);
    init(
        &mut machine,
        Time::from_ticks(0),
        compiled
            .input_snapshot()
            .pulse(input, PulseCount::ONE)
            .unwrap()
            .finish()
            .unwrap(),
    );
    let before = machine.snapshot();
    let failure = machine
        .apply(
            Transaction::advance(
                Time::from_ticks(10),
                machine.revision(),
                level_delta(target, &[]),
            )
            .with_patch(prepared, ReconfigurationPolicy::RejectStateLoss)
            .unwrap(),
        )
        .unwrap_err();
    assert_eq!(failure.code().as_str(), "runtime.budget_exceeded");
    assert_eq!(machine.snapshot(), before);
}

#[test]
fn dense_slot_reassignment_does_not_transfer_state_to_an_added_node() {
    let mut builder =
        NetworkBuilder::with_key(NetworkKey::from_u128(90), TimeDomainId::from_u128(91));
    let input = ExternalInputKey::<Pulse>::from_u128(1);
    let pulse = builder
        .add_pulse_input(input, DiagnosticMeta::default())
        .unwrap();
    let old = NodeKey::from_u128(20);
    let old_signal = builder
        .add_toggle(
            old,
            pulse,
            ToggleConfig::new(LogicLevel::Low),
            DiagnosticMeta::default(),
        )
        .unwrap()
        .into_outputs();
    let old_output = ExternalOutputKey::from_u128(30);
    builder
        .add_level_output(old_output, old_signal, DiagnosticMeta::default())
        .unwrap();
    let compiled = compile(builder);
    let mut machine = compiled.spawn(policy());
    init(
        &mut machine,
        Time::from_ticks(0),
        compiled
            .input_snapshot()
            .pulse(input, PulseCount::ONE)
            .unwrap()
            .finish()
            .unwrap(),
    );
    let new = NodeKey::from_u128(10);
    let trigger = InPortKey::<Pulse>::from_u128(11);
    let state = OutPortKey::<Level>::from_u128(12);
    let new_output = ExternalOutputKey::from_u128(13);
    let prepared = prepare(
        &machine,
        machine
            .patch()
            .add_node(NodeDef::new(
                new,
                NodeKind::toggle(LogicLevel::Low),
                NodePorts::with_input_roles(
                    vec![trigger.into()],
                    vec![mossignal::authored::InputPortRole::Toggle],
                    vec![state.into()],
                ),
                DiagnosticMeta::default(),
            ))
            .unwrap()
            .add_connection(ConnectionDef::new(
                ConnectionKey::from_u128(14),
                ConnectionEndpoint::external_input(input.into()),
                ConnectionEndpoint::node_input(trigger.into()),
                DiagnosticMeta::default(),
            ))
            .unwrap()
            .add_external_output(ExternalOutputDef::new(
                new_output.into(),
                AnySignalSourceKey::Level(SignalSourceKey::NodeOutput(state)),
                DiagnosticMeta::default(),
            ))
            .unwrap()
            .finish(),
    );
    let target = prepared.resulting_compiled().clone();
    machine
        .apply(
            Transaction::advance(
                Time::from_ticks(1),
                machine.revision(),
                level_delta(&target, &[]),
            )
            .with_patch(prepared, ReconfigurationPolicy::RejectStateLoss)
            .unwrap(),
        )
        .unwrap();
    assert_eq!(machine.output_level(old_output), Some(LogicLevel::High));
    assert_eq!(machine.output_level(new_output), Some(LogicLevel::Low));
    let mut restored = target.restore(machine.snapshot(), policy()).unwrap();
    let transaction = Transaction::advance(
        Time::from_ticks(2),
        machine.revision(),
        target
            .input_delta()
            .pulse(input, PulseCount::ONE)
            .unwrap()
            .finish()
            .unwrap(),
    );
    machine.apply(transaction.clone()).unwrap();
    restored.apply(transaction).unwrap();
    assert_eq!(machine.output_level(old_output), Some(LogicLevel::Low));
    assert_eq!(machine.output_level(new_output), Some(LogicLevel::High));
    assert_eq!(machine.snapshot(), restored.snapshot());
}

#[test]
fn recomputed_delay_due_exactly_at_patch_time_is_not_overdue() {
    let mut builder =
        NetworkBuilder::with_key(NetworkKey::from_u128(92), TimeDomainId::from_u128(93));
    let input = ExternalInputKey::<Pulse>::from_u128(1);
    let pulse = builder
        .add_pulse_input(input, DiagnosticMeta::default())
        .unwrap();
    let node = NodeKey::from_u128(2);
    let delayed = builder
        .add_pulse_delay(
            node,
            pulse,
            PulseDelayConfig::new(delay(10)),
            DiagnosticMeta::default(),
        )
        .unwrap()
        .into_outputs();
    let output = ExternalOutputKey::<Pulse>::from_u128(3);
    builder
        .add_pulse_output(output, delayed, DiagnosticMeta::default())
        .unwrap();
    let compiled = compile(builder);
    let mut machine = compiled.spawn(policy());
    init(
        &mut machine,
        Time::from_ticks(0),
        compiled
            .input_snapshot()
            .pulse(input, PulseCount::ONE)
            .unwrap()
            .finish()
            .unwrap(),
    );
    let old = node_def(&compiled, node);
    let replacement = NodeDef::new(
        node,
        NodeKind::pulse_delay(delay(5)),
        old.ports().clone(),
        DiagnosticMeta::default(),
    );
    let prepared = prepare(
        &machine,
        machine
            .patch()
            .replace_node(
                node,
                replacement,
                NodeMigrationDirective::PulseDelay(PulseDelayMigration::RecomputeFromOrigin {
                    overdue: mossignal::OverdueMigrationPolicy::Reject,
                }),
            )
            .unwrap()
            .finish(),
    );
    let result = machine
        .apply(
            Transaction::advance(
                Time::from_ticks(5),
                machine.revision(),
                level_delta(prepared.resulting_compiled(), &[]),
            )
            .with_patch(prepared, ReconfigurationPolicy::RejectStateLoss)
            .unwrap(),
        )
        .unwrap();
    assert!(pulsed(&result, output));
}

#[test]
fn periodic_preserved_boundary_starts_target_cadence_and_reanchor_can_emit_now() {
    for directive in [
        mossignal::PeriodicMigration::PreserveNextDeadline,
        mossignal::PeriodicMigration::ReanchorAtPatchTime,
    ] {
        let mut builder =
            NetworkBuilder::with_key(NetworkKey::from_u128(94), TimeDomainId::from_u128(95));
        let input = ExternalInputKey::<Level>::from_u128(1);
        let enable = builder
            .add_level_input(input, DiagnosticMeta::default())
            .unwrap();
        let node = NodeKey::from_u128(2);
        let pulse = builder
            .add_periodic(
                node,
                enable,
                mossignal::PeriodicConfig::new(
                    delay(5),
                    mossignal::FirstEmissionPolicy::AfterFirstPeriod,
                    mossignal::ReenablePhasePolicy::PreservePhase,
                ),
                DiagnosticMeta::default(),
            )
            .unwrap()
            .into_outputs();
        let output = ExternalOutputKey::<Pulse>::from_u128(3);
        builder
            .add_pulse_output(output, pulse, DiagnosticMeta::default())
            .unwrap();
        let compiled = compile(builder);
        let mut machine = compiled.spawn(policy());
        init(
            &mut machine,
            Time::from_ticks(0),
            level_snapshot(&compiled, &[(input, LogicLevel::High)]),
        );
        let old = node_def(&compiled, node);
        let replacement = NodeDef::new(
            node,
            NodeKind::periodic(mossignal::PeriodicConfig::new(
                delay(7),
                mossignal::FirstEmissionPolicy::Immediate,
                mossignal::ReenablePhasePolicy::PreservePhase,
            )),
            old.ports().clone(),
            DiagnosticMeta::default(),
        );
        let prepared = prepare(
            &machine,
            machine
                .patch()
                .replace_node(
                    node,
                    replacement,
                    NodeMigrationDirective::Periodic(directive),
                )
                .unwrap()
                .finish(),
        );
        let target = prepared.resulting_compiled().clone();
        let result = machine
            .apply(
                Transaction::advance(
                    Time::from_ticks(2),
                    machine.revision(),
                    level_delta(&target, &[]),
                )
                .with_patch(prepared, ReconfigurationPolicy::AllowReportedStateLoss)
                .unwrap(),
            )
            .unwrap();
        if matches!(directive, mossignal::PeriodicMigration::ReanchorAtPatchTime) {
            assert!(pulsed(&result, output));
            assert_eq!(
                machine
                    .inspect_periodic(node)
                    .unwrap()
                    .pending()
                    .unwrap()
                    .deadline(),
                Time::from_ticks(9)
            );
        } else {
            assert!(!pulsed(&result, output));
            let mut restored = target.restore(machine.snapshot(), policy()).unwrap();
            let transaction = Transaction::advance(
                Time::from_ticks(5),
                machine.revision(),
                level_delta(&target, &[]),
            );
            let fired = machine.apply(transaction.clone()).unwrap();
            restored.apply(transaction).unwrap();
            assert!(pulsed(&fired, output));
            assert_eq!(
                machine
                    .inspect_periodic(node)
                    .unwrap()
                    .pending()
                    .unwrap()
                    .deadline(),
                Time::from_ticks(12)
            );
            assert_eq!(machine.snapshot(), restored.snapshot());
        }
        target.restore(machine.snapshot(), policy()).unwrap();
    }
}

fn periodic_migration_machine(
    first: mossignal::FirstEmissionPolicy,
    enabled: LogicLevel,
) -> (
    Machine<Domain>,
    ExternalInputKey<Level>,
    NodeKey,
    ExternalOutputKey<Pulse>,
) {
    periodic_migration_machine_with_phase(
        first,
        enabled,
        mossignal::ReenablePhasePolicy::PreservePhase,
    )
}

fn periodic_migration_machine_with_phase(
    first: mossignal::FirstEmissionPolicy,
    enabled: LogicLevel,
    phase: mossignal::ReenablePhasePolicy,
) -> (
    Machine<Domain>,
    ExternalInputKey<Level>,
    NodeKey,
    ExternalOutputKey<Pulse>,
) {
    let mut builder =
        NetworkBuilder::with_key(NetworkKey::from_u128(96), TimeDomainId::from_u128(97));
    let input = ExternalInputKey::from_u128(1);
    let enable = builder
        .add_level_input(input, DiagnosticMeta::default())
        .unwrap();
    let node = NodeKey::from_u128(2);
    let pulse = builder
        .add_periodic(
            node,
            enable,
            mossignal::PeriodicConfig::new(delay(5), first, phase),
            DiagnosticMeta::default(),
        )
        .unwrap()
        .into_outputs();
    let output = ExternalOutputKey::from_u128(3);
    builder
        .add_pulse_output(output, pulse, DiagnosticMeta::default())
        .unwrap();
    let compiled = compile(builder);
    let mut machine = compiled.spawn(policy());
    init(
        &mut machine,
        Time::from_ticks(0),
        level_snapshot(&compiled, &[(input, enabled)]),
    );
    (machine, input, node, output)
}

fn replace_periodic(
    machine: &Machine<Domain>,
    node: NodeKey,
    period: u64,
    first: mossignal::FirstEmissionPolicy,
    directive: NodeMigrationDirective<Domain>,
) -> PreparedPatch<Domain> {
    let old = node_def(machine.compiled(), node);
    let replacement = NodeDef::new(
        node,
        NodeKind::periodic(mossignal::PeriodicConfig::new(
            delay(period),
            first,
            mossignal::ReenablePhasePolicy::PreservePhase,
        )),
        old.ports().clone(),
        DiagnosticMeta::default(),
    );
    prepare(
        machine,
        machine
            .patch()
            .replace_node(node, replacement, directive)
            .unwrap()
            .finish(),
    )
}

#[test]
fn periodic_migration_rejects_a_retained_anchor_without_pending_work() {
    // Section 51: disabled PreservePhase retains state even with an empty calendar.
    for first in [
        mossignal::FirstEmissionPolicy::Immediate,
        mossignal::FirstEmissionPolicy::AfterFirstPeriod,
    ] {
        for directive in [
            NodeMigrationDirective::Standard,
            NodeMigrationDirective::Periodic(mossignal::PeriodicMigration::RejectIfAnchored),
        ] {
            for loss_policy in [
                ReconfigurationPolicy::RejectStateLoss,
                ReconfigurationPolicy::AllowReportedStateLoss,
            ] {
                let (mut machine, input, node, _) =
                    periodic_migration_machine(first, LogicLevel::High);
                machine
                    .apply(Transaction::advance(
                        Time::from_ticks(1),
                        machine.revision(),
                        level_delta(machine.compiled(), &[(input, LogicLevel::Low)]),
                    ))
                    .unwrap();
                let inspected = machine.inspect_periodic(node).unwrap();
                assert_eq!(inspected.anchor(), Some(Time::from_ticks(0)));
                assert!(inspected.pending().is_none());
                let prepared = replace_periodic(&machine, node, 7, first, directive);
                // Same-time re-enable is target input, not source migration state.
                let transaction = Transaction::advance(
                    Time::from_ticks(2),
                    machine.revision(),
                    level_delta(prepared.resulting_compiled(), &[(input, LogicLevel::High)]),
                )
                .with_patch(prepared, loss_policy)
                .unwrap();
                let before = machine.snapshot();
                let forecast_failure = machine.forecast(transaction.clone()).unwrap_err();
                assert_eq!(machine.snapshot(), before);
                let failure = machine.apply(transaction).unwrap_err();
                assert_eq!(
                    failure.code().as_str(),
                    "reconfiguration.pending_event_migration_rejected"
                );
                assert_eq!(failure.code(), forecast_failure.code());
                assert!(
                    matches!(failure.evidence(), mossignal::RuntimeFailureEvidence::PendingEventMigrationRejected { evidence }
                    if evidence.subject == SubjectRef::Node(node) && evidence.fact == "periodic_anchor")
                );
                assert_eq!(machine.snapshot(), before);
            }
        }
    }
}

#[test]
fn periodic_reanchor_reports_disabled_phase_loss_and_obeys_loss_policy() {
    for first in [
        mossignal::FirstEmissionPolicy::Immediate,
        mossignal::FirstEmissionPolicy::AfterFirstPeriod,
    ] {
        let (mut machine, input, node, output) =
            periodic_migration_machine(first, LogicLevel::High);
        machine
            .apply(Transaction::advance(
                Time::from_ticks(1),
                machine.revision(),
                level_delta(machine.compiled(), &[(input, LogicLevel::Low)]),
            ))
            .unwrap();
        let prepared = replace_periodic(
            &machine,
            node,
            7,
            first,
            NodeMigrationDirective::Periodic(mossignal::PeriodicMigration::ReanchorAtPatchTime),
        );
        let target = prepared.resulting_compiled().clone();
        let transaction = |loss_policy| {
            Transaction::advance(
                Time::from_ticks(2),
                machine.revision(),
                level_delta(&target, &[]),
            )
            .with_patch(prepared.clone(), loss_policy)
            .unwrap()
        };
        let rejecting = transaction(ReconfigurationPolicy::RejectStateLoss);
        let allowing = transaction(ReconfigurationPolicy::AllowReportedStateLoss);
        let before = machine.snapshot();
        let failure = machine.apply(rejecting).unwrap_err();
        assert_eq!(
            failure.code().as_str(),
            "reconfiguration.state_loss_rejected"
        );
        assert_eq!(machine.snapshot(), before);
        let forecast = machine.forecast(allowing.clone()).unwrap();
        assert_eq!(machine.snapshot(), before);
        let applied = machine.apply(allowing).unwrap();
        assert_eq!(
            applied.after_execution_digest(),
            forecast.result().after_execution_digest()
        );
        assert!(
            applied
                .migration()
                .unwrap()
                .losses()
                .iter()
                .any(|loss| loss.subject() == &SubjectRef::Node(node)
                    && loss.fact() == "periodic_schedule"
                    && loss.event().is_none())
        );
        assert!(!pulsed(&applied, output));
        let inspected = machine.inspect_periodic(node).unwrap();
        assert_eq!(inspected.anchor(), Some(Time::from_ticks(2)));
        assert!(inspected.pending().is_none());
        let mut restored = target.restore(machine.snapshot(), policy()).unwrap();
        let enabling = Transaction::advance(
            Time::from_ticks(3),
            machine.revision(),
            level_delta(&target, &[(input, LogicLevel::High)]),
        );
        let applied = machine.apply(enabling.clone()).unwrap();
        restored.apply(enabling).unwrap();
        assert!(!pulsed(&applied, output));
        assert_eq!(
            machine
                .inspect_periodic(node)
                .unwrap()
                .pending()
                .unwrap()
                .deadline(),
            Time::from_ticks(9)
        );
        assert_eq!(machine.snapshot(), restored.snapshot());
    }
}

#[test]
fn periodic_recomputation_keeps_a_preserved_future_phase_reference() {
    for patch_time in [3, 5] {
        let (mut machine, _, node, output) = periodic_migration_machine(
            mossignal::FirstEmissionPolicy::AfterFirstPeriod,
            LogicLevel::High,
        );
        let preserved = replace_periodic(
            &machine,
            node,
            7,
            mossignal::FirstEmissionPolicy::Immediate,
            NodeMigrationDirective::Periodic(mossignal::PeriodicMigration::PreserveNextDeadline),
        );
        machine
            .apply(
                Transaction::advance(
                    Time::from_ticks(2),
                    machine.revision(),
                    level_delta(preserved.resulting_compiled(), &[]),
                )
                .with_patch(preserved, ReconfigurationPolicy::RejectStateLoss)
                .unwrap(),
            )
            .unwrap();
        let inspected = machine.inspect_periodic(node).unwrap();
        assert_eq!(inspected.anchor(), Some(Time::from_ticks(5)));
        assert_eq!(inspected.pending().unwrap().ordinal(), 0);
        let recomputed = replace_periodic(
            &machine,
            node,
            11,
            mossignal::FirstEmissionPolicy::Immediate,
            NodeMigrationDirective::Periodic(
                mossignal::PeriodicMigration::RecomputeFromExistingAnchor,
            ),
        );
        let target = recomputed.resulting_compiled().clone();
        let applied = machine
            .apply(
                Transaction::advance(
                    Time::from_ticks(patch_time),
                    machine.revision(),
                    level_delta(&target, &[]),
                )
                .with_patch(recomputed, ReconfigurationPolicy::RejectStateLoss)
                .unwrap(),
            )
            .unwrap();
        assert_eq!(pulsed(&applied, output), patch_time == 5);
        if patch_time == 5 {
            assert!(
                matches!(applied.output_events(), [OutputEvent::Pulsed { at, count, output: found, .. }]
                if *at == Time::from_ticks(5) && *count == PulseCount::ONE && *found == output)
            );
        }
        let inspected = machine.inspect_periodic(node).unwrap();
        assert_eq!(inspected.anchor(), Some(Time::from_ticks(5)));
        assert_eq!(
            inspected.pending().unwrap().deadline(),
            Time::from_ticks(if patch_time == 3 { 5 } else { 16 })
        );
        assert_eq!(
            inspected.pending().unwrap().ordinal(),
            u64::from(patch_time == 5)
        );
        let mut restored = target.restore(machine.snapshot(), policy()).unwrap();
        if patch_time == 3 {
            let firing = Transaction::advance(
                Time::from_ticks(5),
                machine.revision(),
                level_delta(&target, &[]),
            );
            let fired = machine.apply(firing.clone()).unwrap();
            restored.apply(firing).unwrap();
            assert!(
                matches!(fired.output_events(), [OutputEvent::Pulsed { at, count, output: found, .. }]
                if *at == Time::from_ticks(5) && *count == PulseCount::ONE && *found == output)
            );
            assert_eq!(
                machine
                    .inspect_periodic(node)
                    .unwrap()
                    .pending()
                    .unwrap()
                    .deadline(),
                Time::from_ticks(16)
            );
            assert_eq!(machine.snapshot(), restored.snapshot());
        }
    }
}

#[test]
fn periodic_migration_retains_distinct_anchor_and_cancellation_causes() {
    let (mut machine, input, node, output) = periodic_migration_machine(
        mossignal::FirstEmissionPolicy::AfterFirstPeriod,
        LogicLevel::High,
    );
    machine
        .apply(Transaction::advance(
            Time::from_ticks(1),
            machine.revision(),
            level_delta(machine.compiled(), &[(input, LogicLevel::Low)]),
        ))
        .unwrap();
    // Repeated patches checkpoint prior cancellation evidence without losing its role.
    for at in [2, 3] {
        let prepared = metadata_patch(&machine, &format!("checkpoint-{at}"));
        machine
            .apply(
                Transaction::advance(
                    Time::from_ticks(at),
                    machine.revision(),
                    level_delta(prepared.resulting_compiled(), &[]),
                )
                .with_patch(prepared, ReconfigurationPolicy::RejectStateLoss)
                .unwrap(),
            )
            .unwrap();
        let inspected = machine.inspect_periodic(node).unwrap();
        assert_eq!(inspected.anchor(), Some(Time::from_ticks(0)));
        assert!(inspected.pending().is_none());
        let restored = machine
            .compiled()
            .restore(machine.snapshot(), policy())
            .unwrap();
        assert_eq!(machine.snapshot(), restored.snapshot());
        let restored_inspection = restored.inspect_periodic(node).unwrap();
        assert!(restored_inspection.anchor_cause().is_some());
        assert!(restored_inspection.last_cancellation().is_some());
    }
    let mut restored = machine
        .compiled()
        .restore(machine.snapshot(), policy())
        .unwrap();
    let enabling = Transaction::advance(
        Time::from_ticks(4),
        machine.revision(),
        level_delta(machine.compiled(), &[(input, LogicLevel::High)]),
    );
    machine.apply(enabling.clone()).unwrap();
    restored.apply(enabling).unwrap();
    let firing = Transaction::advance(
        Time::from_ticks(5),
        machine.revision(),
        level_delta(machine.compiled(), &[]),
    );
    assert!(pulsed(&machine.apply(firing.clone()).unwrap(), output));
    restored.apply(firing).unwrap();
    assert_eq!(machine.snapshot(), restored.snapshot());
}

#[test]
fn periodic_migration_allows_fresh_anchorless_state() {
    for directive in [
        NodeMigrationDirective::Standard,
        NodeMigrationDirective::Periodic(mossignal::PeriodicMigration::RejectIfAnchored),
        NodeMigrationDirective::Periodic(mossignal::PeriodicMigration::ReanchorAtPatchTime),
    ] {
        let first = mossignal::FirstEmissionPolicy::AfterFirstPeriod;
        let (mut machine, _, node, _) = periodic_migration_machine(first, LogicLevel::Low);
        assert!(machine.inspect_periodic(node).unwrap().anchor().is_none());
        let prepared = replace_periodic(&machine, node, 7, first, directive);
        let result = machine
            .apply(
                Transaction::advance(
                    Time::from_ticks(2),
                    machine.revision(),
                    level_delta(prepared.resulting_compiled(), &[]),
                )
                .with_patch(prepared, ReconfigurationPolicy::RejectStateLoss)
                .unwrap(),
            )
            .unwrap();
        assert!(result.migration().unwrap().losses().is_empty());
        let inspected = machine.inspect_periodic(node).unwrap();
        if inspected.anchor().is_some() {
            assert!(inspected.anchor_cause().is_some());
        }
        let restored = machine
            .compiled()
            .restore(machine.snapshot(), policy())
            .unwrap();
        assert_eq!(machine.snapshot(), restored.snapshot());
        let inspected = restored.inspect_periodic(node).unwrap();
        assert!(
            inspected
                .provenance()
                .inspect(inspected.current_support())
                .is_ok()
        );
        if inspected.anchor().is_some() {
            assert!(inspected.anchor_cause().is_some());
        }
    }
}

#[test]
fn periodic_restoration_retains_cancellation_without_an_anchor() {
    let (mut machine, input, node, _) = periodic_migration_machine_with_phase(
        mossignal::FirstEmissionPolicy::AfterFirstPeriod,
        LogicLevel::High,
        mossignal::ReenablePhasePolicy::RestartPhase,
    );
    machine
        .apply(Transaction::advance(
            Time::from_ticks(1),
            machine.revision(),
            level_delta(machine.compiled(), &[(input, LogicLevel::Low)]),
        ))
        .unwrap();
    let inspected = machine.inspect_periodic(node).unwrap();
    assert!(inspected.anchor().is_none());
    assert!(inspected.anchor_cause().is_none());
    assert!(inspected.last_cancellation().is_some());
    let restored = machine
        .compiled()
        .restore(machine.snapshot(), policy())
        .unwrap();
    let inspected = restored.inspect_periodic(node).unwrap();
    assert!(inspected.anchor().is_none());
    assert!(inspected.anchor_cause().is_none());
    assert!(inspected.last_cancellation().is_some());
    assert_eq!(machine.snapshot(), restored.snapshot());
    let prepared = metadata_patch(&machine, "cancellation-without-anchor");
    machine
        .apply(
            Transaction::advance(
                Time::from_ticks(2),
                machine.revision(),
                level_delta(prepared.resulting_compiled(), &[]),
            )
            .with_patch(prepared, ReconfigurationPolicy::RejectStateLoss)
            .unwrap(),
        )
        .unwrap();
    let restored = machine
        .compiled()
        .restore(machine.snapshot(), policy())
        .unwrap();
    let inspected = restored.inspect_periodic(node).unwrap();
    assert!(inspected.anchor().is_none());
    assert!(inspected.anchor_cause().is_none());
    assert!(inspected.last_cancellation().is_some());
    assert_eq!(machine.snapshot(), restored.snapshot());
}

fn inertial_migration_machine() -> (Machine<Domain>, ExternalInputKey<Level>, NodeKey) {
    let mut builder =
        NetworkBuilder::with_key(NetworkKey::from_u128(98), TimeDomainId::from_u128(99));
    let input = ExternalInputKey::from_u128(1);
    let signal = builder
        .add_level_input(input, DiagnosticMeta::default())
        .unwrap();
    let node = NodeKey::from_u128(2);
    builder
        .add_inertial_delay(
            node,
            signal,
            mossignal::InertialDelayConfig::new(delay(5), LogicLevel::Low),
            DiagnosticMeta::default(),
        )
        .unwrap();
    let compiled = compile(builder);
    let mut machine = compiled.spawn(policy());
    init(
        &mut machine,
        Time::from_ticks(0),
        level_snapshot(&compiled, &[(input, LogicLevel::Low)]),
    );
    (machine, input, node)
}

#[test]
fn inertial_migration_retains_the_cancellation_role_through_checkpoints() {
    let (mut machine, input, node) = inertial_migration_machine();
    for (at, level) in [(1, LogicLevel::High), (2, LogicLevel::Low)] {
        machine
            .apply(Transaction::advance(
                Time::from_ticks(at),
                machine.revision(),
                level_delta(machine.compiled(), &[(input, level)]),
            ))
            .unwrap();
    }
    assert!(
        machine
            .inspect_inertial_delay(node)
            .unwrap()
            .last_cancellation()
            .is_some()
    );
    for at in [3, 4] {
        let prepared = metadata_patch(&machine, &format!("inertial-checkpoint-{at}"));
        machine
            .apply(
                Transaction::advance(
                    Time::from_ticks(at),
                    machine.revision(),
                    level_delta(prepared.resulting_compiled(), &[]),
                )
                .with_patch(prepared, ReconfigurationPolicy::RejectStateLoss)
                .unwrap(),
            )
            .unwrap();
        let restored = machine
            .compiled()
            .restore(machine.snapshot(), policy())
            .unwrap();
        assert_eq!(machine.snapshot(), restored.snapshot());
        assert!(
            restored
                .inspect_inertial_delay(node)
                .unwrap()
                .last_cancellation()
                .is_some()
        );
    }
}

#[test]
fn inertial_restoration_keeps_a_matured_transition_distinct_from_cancellation() {
    for canceled in [true, false] {
        let (mut machine, input, node) = inertial_migration_machine();
        machine
            .apply(Transaction::advance(
                Time::from_ticks(1),
                machine.revision(),
                level_delta(machine.compiled(), &[(input, LogicLevel::High)]),
            ))
            .unwrap();
        machine
            .apply(Transaction::advance(
                Time::from_ticks(6),
                machine.revision(),
                level_delta(machine.compiled(), &[]),
            ))
            .unwrap();
        let inspected = machine.inspect_inertial_delay(node).unwrap();
        assert_eq!(inspected.output(), LogicLevel::High);
        assert!(inspected.last_cancellation().is_none());
        if canceled {
            for (at, level) in [(8, LogicLevel::Low), (9, LogicLevel::High)] {
                machine
                    .apply(Transaction::advance(
                        Time::from_ticks(at),
                        machine.revision(),
                        level_delta(machine.compiled(), &[(input, level)]),
                    ))
                    .unwrap();
            }
        }
        let restored = machine
            .compiled()
            .restore(machine.snapshot(), policy())
            .unwrap();
        let inspected = restored.inspect_inertial_delay(node).unwrap();
        assert_eq!(inspected.output(), LogicLevel::High);
        assert_eq!(inspected.last_cancellation().is_some(), canceled);
        assert_eq!(machine.snapshot(), restored.snapshot());
    }
}
