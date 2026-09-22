use mossignal::authored::{
    ConnectionDef, ExternalInputDef, ExternalOutputDef, InputPortRole, NodeDef, NodeKind,
    NodePorts, UncheckedNetwork,
};
use mossignal::key::{
    ConnectionKey, ExternalInputKey, ExternalOutputKey, InPortKey, ModuleInputKey,
    ModuleInstanceKey, ModuleOutputKey, NetworkKey, NodeKey, OutPortKey, SignalSourceKey,
};
use mossignal::metadata::DiagnosticMeta;
use mossignal::signal::{Level, LogicLevel, Pulse};
use mossignal::time::{NonZeroSpan, Time};
use mossignal::{
    CauseInspection, FirstEmissionPolicy, ModuleBuilder, NetworkBuilder, NodeSubject, OutputEvent,
    PeriodicConfig, ReenablePhasePolicy, RuntimeFailureEvidence, RuntimePolicy, RuntimePolicyLimit,
    Schedule, TimeDomainId, Transaction,
};

#[derive(Debug, PartialEq)]
enum TestDomain {}

struct Fixture {
    compiled: mossignal::CompiledNetwork<TestDomain>,
    enable: ExternalInputKey<Level>,
    output: ExternalOutputKey<Pulse>,
    node: NodeKey,
}

fn policy() -> RuntimePolicy {
    policy_with([1_000, 100_000, 1_000, 10_000, 100_000])
}

fn policy_with(values: [u64; 5]) -> RuntimePolicy {
    RuntimePolicy::builder()
        .max_internal_reactions(values[0])
        .max_evaluated_operations(values[1])
        .max_pending_events(values[2])
        .max_events_created_per_transaction(values[3])
        .max_required_provenance_growth(values[4])
        .build()
        .unwrap()
}

fn machine_with_policy(
    fixture: &Fixture,
    runtime_policy: RuntimePolicy,
) -> mossignal::Machine<TestDomain> {
    fixture.compiled.spawn(runtime_policy)
}

fn initialize_machine(
    fixture: &Fixture,
    machine: &mut mossignal::Machine<TestDomain>,
    level: LogicLevel,
    at: u64,
) -> Result<mossignal::TransactionResult<TestDomain>, mossignal::RuntimeFailure<TestDomain>> {
    let snapshot = fixture
        .compiled
        .input_snapshot()
        .set(fixture.enable, level)
        .and_then(mossignal::InputSnapshotBuilder::finish)
        .unwrap();
    machine.apply(Transaction::initialize(
        Time::from_ticks(at),
        machine.revision(),
        snapshot,
    ))
}

fn fixture(first: FirstEmissionPolicy, reenable: ReenablePhasePolicy) -> Fixture {
    fixture_with_period(5, first, reenable)
}

fn fixture_with_period(
    period_ticks: u64,
    first: FirstEmissionPolicy,
    reenable: ReenablePhasePolicy,
) -> Fixture {
    let mut builder =
        NetworkBuilder::with_key(NetworkKey::from_u128(1), TimeDomainId::from_u128(2));
    let enable = ExternalInputKey::from_u128(10);
    let enable_signal = builder
        .add_level_input(enable, DiagnosticMeta::default())
        .unwrap();
    let node = NodeKey::from_u128(20);
    let periodic = builder
        .add_periodic_with_ports(
            node,
            InPortKey::from_u128(30),
            OutPortKey::from_u128(31),
            enable_signal,
            PeriodicConfig::new(
                NonZeroSpan::from_ticks(period_ticks).unwrap(),
                first,
                reenable,
            ),
            DiagnosticMeta::default(),
        )
        .unwrap()
        .into_outputs();
    let output = ExternalOutputKey::from_u128(40);
    builder
        .add_pulse_output(output, periodic, DiagnosticMeta::default())
        .unwrap();
    let compiled = builder
        .finish()
        .require_artifact()
        .unwrap()
        .compile()
        .require_artifact()
        .unwrap();
    Fixture {
        compiled,
        enable,
        output,
        node,
    }
}

fn initialize(
    fixture: &Fixture,
    level: LogicLevel,
    at: u64,
) -> (
    mossignal::Machine<TestDomain>,
    mossignal::TransactionResult<TestDomain>,
) {
    let mut machine = fixture.compiled.spawn(policy());
    let result = initialize_machine(fixture, &mut machine, level, at).unwrap();
    (machine, result)
}

fn advance(
    fixture: &Fixture,
    machine: &mut mossignal::Machine<TestDomain>,
    at: u64,
    enable: Option<LogicLevel>,
) -> mossignal::TransactionResult<TestDomain> {
    let delta = match enable {
        Some(level) => fixture
            .compiled
            .input_delta()
            .set(fixture.enable, level)
            .unwrap()
            .finish(),
        None => fixture.compiled.input_delta().finish(),
    }
    .unwrap();
    machine
        .apply(Transaction::advance(
            Time::from_ticks(at),
            machine.revision(),
            delta,
        ))
        .unwrap()
}

fn pulses(result: &mossignal::TransactionResult<TestDomain>) -> Vec<(u128, u64, u64)> {
    result
        .output_events()
        .iter()
        .map(|event| match event {
            OutputEvent::Pulsed {
                output, count, at, ..
            } => (output.as_u128(), count.get(), at.ticks()),
            _ => panic!("Periodic fixture has only a Pulse output"),
        })
        .collect()
}

#[test]
fn first_emission_policies_share_exact_recurring_boundaries() {
    for first in [
        FirstEmissionPolicy::Immediate,
        FirstEmissionPolicy::AfterFirstPeriod,
    ] {
        for reenable in [
            ReenablePhasePolicy::RestartPhase,
            ReenablePhasePolicy::PreservePhase,
        ] {
            let fixture = fixture(first, reenable);
            let (mut machine, initialized) = initialize(&fixture, LogicLevel::High, 10);
            assert_eq!(
                pulses(&initialized),
                if first == FirstEmissionPolicy::Immediate {
                    vec![(fixture.output.as_u128(), 1, 10)]
                } else {
                    Vec::new()
                }
            );
            assert_eq!(
                initialized.schedule(),
                Schedule::WakeAt(Time::from_ticks(15))
            );
            let inspected = machine.inspect_periodic(fixture.node).unwrap();
            assert_eq!(inspected.anchor(), Some(Time::from_ticks(10)));
            assert_eq!(inspected.pending().unwrap().ordinal(), 1);
            assert_eq!(
                inspected.pending().unwrap().deadline(),
                Time::from_ticks(15)
            );
            assert_eq!(inspected.pending().unwrap().first_emission(), first);
            assert_eq!(inspected.pending().unwrap().reenable_phase(), reenable);

            let boundary = advance(&fixture, &mut machine, 15, None);
            assert_eq!(pulses(&boundary), vec![(fixture.output.as_u128(), 1, 15)]);
            assert_eq!(
                machine
                    .inspect_periodic(fixture.node)
                    .unwrap()
                    .pending()
                    .unwrap()
                    .ordinal(),
                2
            );
        }
    }
}

#[test]
fn disabled_initialization_is_anchorless_until_the_first_enable() {
    let fixture = fixture(
        FirstEmissionPolicy::Immediate,
        ReenablePhasePolicy::PreservePhase,
    );
    let (mut machine, initialized) = initialize(&fixture, LogicLevel::Low, 37);
    assert!(pulses(&initialized).is_empty());
    assert_eq!(initialized.schedule(), Schedule::Dormant);
    let disabled = machine.inspect_periodic(fixture.node).unwrap();
    assert_eq!(disabled.anchor(), None);
    assert!(disabled.pending().is_none());

    let enabled = advance(&fixture, &mut machine, 41, Some(LogicLevel::High));
    assert_eq!(pulses(&enabled), vec![(fixture.output.as_u128(), 1, 41)]);
    let active = machine.inspect_periodic(fixture.node).unwrap();
    assert_eq!(active.anchor(), Some(Time::from_ticks(41)));
    assert_eq!(active.pending().unwrap().deadline(), Time::from_ticks(46));
}

#[test]
fn reenable_policies_handle_exact_and_missed_boundaries_without_replay() {
    for first in [
        FirstEmissionPolicy::Immediate,
        FirstEmissionPolicy::AfterFirstPeriod,
    ] {
        for reenable in [
            ReenablePhasePolicy::RestartPhase,
            ReenablePhasePolicy::PreservePhase,
        ] {
            let fixture = fixture(first, reenable);
            let (mut machine, _) = initialize(&fixture, LogicLevel::High, 0);
            advance(&fixture, &mut machine, 2, Some(LogicLevel::Low));
            assert_eq!(machine.schedule().unwrap(), Schedule::Dormant);
            let disabled = machine.inspect_periodic(fixture.node).unwrap();
            assert!(disabled.pending().is_none());
            assert_eq!(
                disabled.anchor(),
                if reenable == ReenablePhasePolicy::PreservePhase {
                    Some(Time::from_ticks(0))
                } else {
                    None
                }
            );
            assert!(disabled.last_cancellation().is_some());

            let exact = advance(&fixture, &mut machine, 5, Some(LogicLevel::High));
            let emits = first == FirstEmissionPolicy::Immediate;
            assert_eq!(pulses(&exact).len(), usize::from(emits));
            let inspection = machine.inspect_periodic(fixture.node).unwrap();
            assert_eq!(
                inspection.pending().unwrap().deadline(),
                Time::from_ticks(10)
            );
            assert_eq!(
                inspection.pending().unwrap().ordinal(),
                if reenable == ReenablePhasePolicy::RestartPhase {
                    1
                } else {
                    2
                }
            );
        }
    }

    let fixture = fixture(
        FirstEmissionPolicy::Immediate,
        ReenablePhasePolicy::PreservePhase,
    );
    let (mut before, _) = initialize(&fixture, LogicLevel::High, 0);
    advance(&fixture, &mut before, 2, Some(LogicLevel::Low));
    let reenabled_before = advance(&fixture, &mut before, 4, Some(LogicLevel::High));
    assert!(pulses(&reenabled_before).is_empty());
    assert_eq!(
        before
            .inspect_periodic(fixture.node)
            .unwrap()
            .pending()
            .unwrap()
            .deadline(),
        Time::from_ticks(5)
    );

    let (mut machine, _) = initialize(&fixture, LogicLevel::High, 0);
    advance(&fixture, &mut machine, 2, Some(LogicLevel::Low));
    let reenabled = advance(&fixture, &mut machine, 7, Some(LogicLevel::High));
    assert!(pulses(&reenabled).is_empty());
    let pending = machine.inspect_periodic(fixture.node).unwrap();
    assert_eq!(pending.pending().unwrap().deadline(), Time::from_ticks(10));
    assert_eq!(pending.pending().unwrap().ordinal(), 2);
}

fn cause_reaches_periodic_event(
    provenance: &mossignal::ProvenanceView<TestDomain>,
    cause: mossignal::CauseRef,
    serial: u64,
) -> bool {
    match provenance.inspect(cause).unwrap() {
        CauseInspection::PendingPeriodicBoundary {
            event, supporters, ..
        } => {
            event.value() == serial
                || supporters
                    .iter()
                    .any(|supporter| cause_reaches_periodic_event(provenance, *supporter, serial))
        }
        CauseInspection::Derived { supporters, .. }
        | CauseInspection::PulseDerived { supporters, .. }
        | CauseInspection::PulseControlledLevel { supporters, .. }
        | CauseInspection::PendingPulseDelay { supporters, .. }
        | CauseInspection::PendingTransportDelay { supporters, .. }
        | CauseInspection::PendingInertialDelay { supporters, .. } => supporters
            .iter()
            .any(|supporter| cause_reaches_periodic_event(provenance, *supporter, serial)),
        _ => false,
    }
}

#[test]
fn pending_identity_and_emission_provenance_distinguish_due_from_immediate_work() {
    let delayed = fixture(
        FirstEmissionPolicy::AfterFirstPeriod,
        ReenablePhasePolicy::PreservePhase,
    );
    let (mut machine, _) = initialize(&delayed, LogicLevel::High, 0);
    let first_pending = machine.inspect_periodic(delayed.node).unwrap();
    let event = first_pending.pending().unwrap().event();
    assert_eq!(
        machine
            .inspect_periodic(delayed.node)
            .unwrap()
            .pending()
            .unwrap()
            .event(),
        event
    );
    assert!(matches!(
        first_pending
            .provenance()
            .inspect(first_pending.pending().unwrap().cause())
            .unwrap(),
        CauseInspection::PendingPeriodicBoundary {
            event: inspected,
            anchor,
            ordinal: 1,
            first_emission: FirstEmissionPolicy::AfterFirstPeriod,
            reenable_phase: ReenablePhasePolicy::PreservePhase,
            ..
        } if inspected == event && anchor == Time::from_ticks(0)
    ));
    let due = advance(&delayed, &mut machine, 5, None);
    let due_cause = match due.output_events() {
        [OutputEvent::Pulsed { cause, .. }] => *cause,
        _ => panic!("one due boundary must emit once"),
    };
    assert!(cause_reaches_periodic_event(
        due.provenance(),
        due_cause,
        event.value()
    ));

    let immediate = fixture(
        FirstEmissionPolicy::Immediate,
        ReenablePhasePolicy::RestartPhase,
    );
    let (_, initialized) = initialize(&immediate, LogicLevel::High, 0);
    let immediate_cause = match initialized.output_events() {
        [OutputEvent::Pulsed { cause, .. }] => *cause,
        _ => panic!("immediate initialization must emit once"),
    };
    assert!(!cause_reaches_periodic_event(
        initialized.provenance(),
        immediate_cause,
        0
    ));
}

fn assert_jump_budget_boundary(
    budget: RuntimePolicyLimit,
    value_index: usize,
    exact: u64,
    consumed: u64,
) {
    let fixture = fixture(
        FirstEmissionPolicy::AfterFirstPeriod,
        ReenablePhasePolicy::PreservePhase,
    );
    for (limit, succeeds) in [(exact - 1, false), (exact, true), (exact + 1, true)] {
        let mut values = [1_000, 100_000, 1_000, 10_000, 100_000];
        values[value_index] = limit;
        let mut machine = machine_with_policy(&fixture, policy_with(values));
        initialize_machine(&fixture, &mut machine, LogicLevel::High, 0).unwrap();
        let before = machine.inspect_periodic(fixture.node).unwrap();
        let before_event = before.pending().unwrap().event();
        let before_cause = before.pending().unwrap().cause();
        let delta = fixture.compiled.input_delta().finish().unwrap();
        let result = machine.apply(Transaction::advance(
            Time::from_ticks(16),
            machine.revision(),
            delta,
        ));
        if succeeds {
            assert_eq!(pulses(&result.unwrap()).len(), 3);
        } else {
            let failure = result.unwrap_err();
            assert_eq!(
                failure.evidence(),
                &RuntimeFailureEvidence::BudgetExceeded {
                    budget,
                    limit,
                    consumed,
                }
            );
            assert_eq!(machine.now(), Some(Time::from_ticks(0)));
            assert_eq!(
                machine.schedule().unwrap(),
                Schedule::WakeAt(Time::from_ticks(5))
            );
            let after = machine.inspect_periodic(fixture.node).unwrap();
            assert_eq!(after.anchor(), Some(Time::from_ticks(0)));
            assert_eq!(after.pending().unwrap().event(), before_event);
            assert_eq!(after.pending().unwrap().cause(), before_cause);
            assert_eq!(after.pending().unwrap().ordinal(), 1);
        }
    }
}

#[test]
fn recurring_progress_obeys_every_runtime_budget_at_its_boundary() {
    assert_jump_budget_boundary(RuntimePolicyLimit::MaxInternalReactions, 0, 4, 4);
    assert_jump_budget_boundary(RuntimePolicyLimit::MaxEvaluatedOperations, 1, 16, 16);
    assert_jump_budget_boundary(RuntimePolicyLimit::MaxEventsCreatedPerTransaction, 3, 6, 6);
    assert_jump_budget_boundary(RuntimePolicyLimit::MaxRequiredProvenanceGrowth, 4, 19, 19);

    let fixture = fixture(
        FirstEmissionPolicy::AfterFirstPeriod,
        ReenablePhasePolicy::PreservePhase,
    );
    for (limit, succeeds) in [(0, false), (1, true), (2, true)] {
        let mut machine = machine_with_policy(
            &fixture,
            policy_with([1_000, 100_000, limit, 10_000, 100_000]),
        );
        let result = initialize_machine(&fixture, &mut machine, LogicLevel::High, 0);
        if succeeds {
            assert_eq!(
                result.unwrap().schedule(),
                Schedule::WakeAt(Time::from_ticks(5))
            );
        } else {
            assert_eq!(
                result.unwrap_err().evidence(),
                &RuntimeFailureEvidence::BudgetExceeded {
                    budget: RuntimePolicyLimit::MaxPendingEvents,
                    limit,
                    consumed: 1,
                }
            );
            assert!(!machine.is_initialized());
        }
    }
}

#[test]
fn direct_jump_emits_each_boundary_and_target_time_disable_suppresses_due_work() {
    let fixture = fixture(
        FirstEmissionPolicy::AfterFirstPeriod,
        ReenablePhasePolicy::PreservePhase,
    );
    let (mut jumped, _) = initialize(&fixture, LogicLevel::High, 0);
    let jumped_result = advance(&fixture, &mut jumped, 16, None);
    assert_eq!(
        pulses(&jumped_result),
        vec![
            (fixture.output.as_u128(), 1, 5),
            (fixture.output.as_u128(), 1, 10),
            (fixture.output.as_u128(), 1, 15),
        ]
    );
    let jumped_pending = jumped.inspect_periodic(fixture.node).unwrap();

    let (mut stepped, _) = initialize(&fixture, LogicLevel::High, 0);
    let mut stepped_pulses = Vec::new();
    for at in [5, 10, 15, 16] {
        stepped_pulses.extend(pulses(&advance(&fixture, &mut stepped, at, None)));
    }
    assert_eq!(stepped_pulses, pulses(&jumped_result));
    let stepped_pending = stepped.inspect_periodic(fixture.node).unwrap();
    assert_eq!(
        stepped_pending.pending().unwrap().event(),
        jumped_pending.pending().unwrap().event()
    );
    assert_eq!(
        stepped_pending.pending().unwrap().deadline(),
        jumped_pending.pending().unwrap().deadline()
    );
    assert_eq!(
        stepped_pending.pending().unwrap().ordinal(),
        jumped_pending.pending().unwrap().ordinal()
    );

    let (mut suppressed, _) = initialize(&fixture, LogicLevel::High, 0);
    let result = advance(&fixture, &mut suppressed, 5, Some(LogicLevel::Low));
    assert!(pulses(&result).is_empty());
    assert_eq!(result.schedule(), Schedule::Dormant);
    let inspection = suppressed.inspect_periodic(fixture.node).unwrap();
    let cancellation = inspection.last_cancellation().unwrap();
    assert!(matches!(
        inspection.provenance().inspect(cancellation).unwrap(),
        CauseInspection::Derived { .. }
    ));
}

#[test]
fn deadline_overflow_rejects_initialization_atomically() {
    let fixture = fixture(
        FirstEmissionPolicy::AfterFirstPeriod,
        ReenablePhasePolicy::RestartPhase,
    );
    let snapshot = fixture
        .compiled
        .input_snapshot()
        .set(fixture.enable, LogicLevel::High)
        .and_then(mossignal::InputSnapshotBuilder::finish)
        .unwrap();
    let mut machine = fixture.compiled.spawn(policy());
    let failure = machine
        .apply(Transaction::initialize(
            Time::from_ticks(u64::MAX - 2),
            machine.revision(),
            snapshot,
        ))
        .unwrap_err();
    assert!(matches!(
        failure.evidence(),
        RuntimeFailureEvidence::PeriodicTimeOverflow { .. }
    ));
    assert!(!machine.is_initialized());
    assert!(matches!(
        machine.inspect_periodic(fixture.node),
        Err(mossignal::PeriodicInspectionFailure::NotInitialized)
    ));
}

#[test]
fn late_deadline_overflow_rolls_back_earlier_internal_boundaries() {
    let fixture = fixture(
        FirstEmissionPolicy::AfterFirstPeriod,
        ReenablePhasePolicy::PreservePhase,
    );
    let start = u64::MAX - 12;
    let (mut machine, _) = initialize(&fixture, LogicLevel::High, start);
    let before = machine.inspect_periodic(fixture.node).unwrap();
    let event = before.pending().unwrap().event();
    let cause = before.pending().unwrap().cause();
    let failure = machine
        .apply(Transaction::advance(
            Time::from_ticks(u64::MAX),
            machine.revision(),
            fixture.compiled.input_delta().finish().unwrap(),
        ))
        .unwrap_err();
    assert!(matches!(
        failure.evidence(),
        RuntimeFailureEvidence::PeriodicTimeOverflow { .. }
    ));
    assert_eq!(machine.now(), Some(Time::from_ticks(start)));
    assert_eq!(
        machine.schedule().unwrap(),
        Schedule::WakeAt(Time::from_ticks(u64::MAX - 7))
    );
    let after = machine.inspect_periodic(fixture.node).unwrap();
    assert_eq!(after.pending().unwrap().event(), event);
    assert_eq!(after.pending().unwrap().cause(), cause);
    assert_eq!(after.pending().unwrap().ordinal(), 1);
}

#[test]
fn dynamic_and_typed_construction_have_the_same_semantic_identity() {
    let typed = fixture(
        FirstEmissionPolicy::Immediate,
        ReenablePhasePolicy::PreservePhase,
    );
    let enable = ExternalInputKey::<Level>::from_u128(10);
    let node = NodeKey::from_u128(20);
    let input = InPortKey::<Level>::from_u128(30);
    let output_port = OutPortKey::<Pulse>::from_u128(31);
    let output = ExternalOutputKey::<Pulse>::from_u128(40);
    let dynamic = UncheckedNetwork::<TestDomain>::new(
        NetworkKey::from_u128(1),
        TimeDomainId::from_u128(2),
        DiagnosticMeta::default(),
        vec![NodeDef::new(
            node,
            NodeKind::periodic(PeriodicConfig::new(
                NonZeroSpan::from_ticks(5).unwrap(),
                FirstEmissionPolicy::Immediate,
                ReenablePhasePolicy::PreservePhase,
            )),
            NodePorts::with_input_roles(
                vec![input.into()],
                vec![InputPortRole::Enable],
                vec![output_port.into()],
            ),
            DiagnosticMeta::default(),
        )],
        vec![ExternalInputDef::new(
            enable.into(),
            DiagnosticMeta::default(),
        )],
        vec![ExternalOutputDef::new(
            output.into(),
            SignalSourceKey::NodeOutput(output_port).into(),
            DiagnosticMeta::default(),
        )],
        vec![ConnectionDef::new(
            ConnectionKey::from_u128(0),
            enable.into(),
            input.into(),
            DiagnosticMeta::default(),
        )],
    )
    .validate()
    .require_artifact()
    .unwrap();
    assert_eq!(typed.compiled.fingerprint(), dynamic.fingerprint());

    let changed_first = fixture(
        FirstEmissionPolicy::AfterFirstPeriod,
        ReenablePhasePolicy::PreservePhase,
    );
    let changed_phase = fixture(
        FirstEmissionPolicy::Immediate,
        ReenablePhasePolicy::RestartPhase,
    );
    assert_ne!(
        typed.compiled.fingerprint(),
        changed_first.compiled.fingerprint()
    );
    assert_ne!(
        typed.compiled.fingerprint(),
        changed_phase.compiled.fingerprint()
    );
    let changed_period = fixture_with_period(
        6,
        FirstEmissionPolicy::Immediate,
        ReenablePhasePolicy::PreservePhase,
    );
    assert_ne!(
        typed.compiled.fingerprint(),
        changed_period.compiled.fingerprint()
    );
}

#[test]
fn module_wrapped_periodic_retains_qualified_inspection_and_execution() {
    let input = ModuleInputKey::<Level>::from_u128(1);
    let output = ModuleOutputKey::<Pulse>::from_u128(2);
    let node = NodeKey::from_u128(3);
    let mut module = ModuleBuilder::<TestDomain>::new();
    let enable = module
        .add_level_input(input, DiagnosticMeta::default())
        .unwrap();
    let periodic = module
        .add_periodic(
            node,
            enable,
            PeriodicConfig::new(
                NonZeroSpan::from_ticks(3).unwrap(),
                FirstEmissionPolicy::AfterFirstPeriod,
                ReenablePhasePolicy::PreservePhase,
            ),
            DiagnosticMeta::default(),
        )
        .unwrap()
        .into_outputs();
    module
        .add_pulse_output(output, periodic, DiagnosticMeta::default())
        .unwrap();
    let module = module.finish().require_artifact().unwrap();

    let instance = ModuleInstanceKey::from_u128(10);
    let external_input = ExternalInputKey::<Level>::from_u128(11);
    let external_output = ExternalOutputKey::<Pulse>::from_u128(12);
    let mut network = NetworkBuilder::<TestDomain>::new(TimeDomainId::from_u128(13));
    let external = network
        .add_level_input(external_input, DiagnosticMeta::default())
        .unwrap();
    let added = network
        .instantiate(&module, instance, DiagnosticMeta::default())
        .unwrap()
        .bind_level(input, external)
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
    let mut machine = compiled.spawn(policy());
    let snapshot = compiled
        .input_snapshot()
        .set(external_input, LogicLevel::High)
        .and_then(mossignal::InputSnapshotBuilder::finish)
        .unwrap();
    machine
        .apply(Transaction::initialize(
            Time::from_ticks(4),
            machine.revision(),
            snapshot,
        ))
        .unwrap();
    let module_inspection = machine.inspect_module(instance).unwrap();
    let periodic = module_inspection
        .nodes()
        .iter()
        .find_map(|candidate| candidate.periodic())
        .unwrap();
    assert!(matches!(
        periodic.node(),
        NodeSubject::Qualified(qualified) if qualified.instances() == [instance]
    ));
    assert_eq!(periodic.pending().unwrap().deadline(), Time::from_ticks(7));
    let result = machine
        .apply(Transaction::advance(
            Time::from_ticks(7),
            machine.revision(),
            compiled.input_delta().finish().unwrap(),
        ))
        .unwrap();
    assert_eq!(
        result.output_events().iter().filter(|event| matches!(event, OutputEvent::Pulsed { output, .. } if *output == external_output)).count(),
        1
    );
}

fn equal_deadline_periodics(reverse: bool) -> mossignal::CompiledNetwork<TestDomain> {
    let enable = ExternalInputKey::<Level>::from_u128(10);
    let first_node = NodeKey::from_u128(20);
    let second_node = NodeKey::from_u128(21);
    let merge_node = NodeKey::from_u128(22);
    let first_enable = InPortKey::<Level>::from_u128(30);
    let second_enable = InPortKey::<Level>::from_u128(31);
    let first_output = OutPortKey::<Pulse>::from_u128(40);
    let second_output = OutPortKey::<Pulse>::from_u128(41);
    let merge_first = InPortKey::<Pulse>::from_u128(32);
    let merge_second = InPortKey::<Pulse>::from_u128(33);
    let merge_output = OutPortKey::<Pulse>::from_u128(42);
    let config = PeriodicConfig::new(
        NonZeroSpan::from_ticks(5).unwrap(),
        FirstEmissionPolicy::AfterFirstPeriod,
        ReenablePhasePolicy::PreservePhase,
    );
    let mut nodes = vec![
        NodeDef::new(
            first_node,
            NodeKind::periodic(config),
            NodePorts::with_input_roles(
                vec![first_enable.into()],
                vec![InputPortRole::Enable],
                vec![first_output.into()],
            ),
            DiagnosticMeta::default(),
        ),
        NodeDef::new(
            second_node,
            NodeKind::periodic(config),
            NodePorts::with_input_roles(
                vec![second_enable.into()],
                vec![InputPortRole::Enable],
                vec![second_output.into()],
            ),
            DiagnosticMeta::default(),
        ),
        NodeDef::new(
            merge_node,
            NodeKind::merge(),
            NodePorts::with_input_roles(
                vec![merge_first.into(), merge_second.into()],
                vec![InputPortRole::Input, InputPortRole::Input],
                vec![merge_output.into()],
            ),
            DiagnosticMeta::default(),
        ),
    ];
    let mut connections = vec![
        ConnectionDef::new(
            ConnectionKey::from_u128(50),
            enable.into(),
            first_enable.into(),
            DiagnosticMeta::default(),
        ),
        ConnectionDef::new(
            ConnectionKey::from_u128(51),
            enable.into(),
            second_enable.into(),
            DiagnosticMeta::default(),
        ),
        ConnectionDef::new(
            ConnectionKey::from_u128(52),
            first_output.into(),
            merge_first.into(),
            DiagnosticMeta::default(),
        ),
        ConnectionDef::new(
            ConnectionKey::from_u128(53),
            second_output.into(),
            merge_second.into(),
            DiagnosticMeta::default(),
        ),
    ];
    if reverse {
        nodes.reverse();
        connections.reverse();
    }
    UncheckedNetwork::new(
        NetworkKey::from_u128(1),
        TimeDomainId::from_u128(2),
        DiagnosticMeta::default(),
        nodes,
        vec![ExternalInputDef::new(
            enable.into(),
            DiagnosticMeta::default(),
        )],
        vec![ExternalOutputDef::new(
            ExternalOutputKey::<Pulse>::from_u128(60).into(),
            SignalSourceKey::NodeOutput(merge_output).into(),
            DiagnosticMeta::default(),
        )],
        connections,
    )
    .validate()
    .require_artifact()
    .unwrap()
    .compile()
    .require_artifact()
    .unwrap()
}

#[test]
fn equal_deadline_periodics_are_insertion_order_invariant_and_simultaneous() {
    let forward = equal_deadline_periodics(false);
    let reverse = equal_deadline_periodics(true);
    assert_eq!(forward.fingerprint(), reverse.fingerprint());
    for compiled in [forward, reverse] {
        let mut machine = compiled.spawn(policy());
        let snapshot = compiled
            .input_snapshot()
            .set(ExternalInputKey::from_u128(10), LogicLevel::High)
            .and_then(mossignal::InputSnapshotBuilder::finish)
            .unwrap();
        machine
            .apply(Transaction::initialize(
                Time::from_ticks(0),
                machine.revision(),
                snapshot,
            ))
            .unwrap();
        assert_eq!(
            machine
                .inspect_periodic(NodeKey::from_u128(20))
                .unwrap()
                .pending()
                .unwrap()
                .event()
                .value(),
            0
        );
        assert_eq!(
            machine
                .inspect_periodic(NodeKey::from_u128(21))
                .unwrap()
                .pending()
                .unwrap()
                .event()
                .value(),
            1
        );
        let result = machine
            .apply(Transaction::advance(
                Time::from_ticks(5),
                machine.revision(),
                compiled.input_delta().finish().unwrap(),
            ))
            .unwrap();
        assert!(matches!(
            result.output_events(),
            [OutputEvent::Pulsed { count, at, .. }]
                if count.get() == 2 && *at == Time::from_ticks(5)
        ));
    }
}

#[test]
fn malformed_roles_and_current_reaction_cycles_are_rejected_structurally() {
    let enable = ExternalInputKey::<Level>::from_u128(10);
    let input = InPortKey::<Level>::from_u128(30);
    let output = OutPortKey::<Pulse>::from_u128(40);
    let malformed = UncheckedNetwork::<TestDomain>::new(
        NetworkKey::from_u128(1),
        TimeDomainId::from_u128(2),
        DiagnosticMeta::default(),
        vec![NodeDef::new(
            NodeKey::from_u128(20),
            NodeKind::periodic(PeriodicConfig::new(
                NonZeroSpan::from_ticks(5).unwrap(),
                FirstEmissionPolicy::Immediate,
                ReenablePhasePolicy::RestartPhase,
            )),
            NodePorts::with_input_roles(
                vec![input.into()],
                vec![InputPortRole::Input],
                vec![output.into()],
            ),
            DiagnosticMeta::default(),
        )],
        vec![ExternalInputDef::new(
            enable.into(),
            DiagnosticMeta::default(),
        )],
        Vec::new(),
        vec![ConnectionDef::new(
            ConnectionKey::from_u128(50),
            enable.into(),
            input.into(),
            DiagnosticMeta::default(),
        )],
    )
    .validate();
    assert!(malformed.artifact().is_none());

    let periodic_enable = InPortKey::<Level>::from_u128(31);
    let periodic_output = OutPortKey::<Pulse>::from_u128(41);
    let toggle_input = InPortKey::<Pulse>::from_u128(32);
    let toggle_output = OutPortKey::<Level>::from_u128(42);
    let cycle = UncheckedNetwork::<TestDomain>::new(
        NetworkKey::from_u128(101),
        TimeDomainId::from_u128(2),
        DiagnosticMeta::default(),
        vec![
            NodeDef::new(
                NodeKey::from_u128(120),
                NodeKind::periodic(PeriodicConfig::new(
                    NonZeroSpan::from_ticks(5).unwrap(),
                    FirstEmissionPolicy::Immediate,
                    ReenablePhasePolicy::RestartPhase,
                )),
                NodePorts::with_input_roles(
                    vec![periodic_enable.into()],
                    vec![InputPortRole::Enable],
                    vec![periodic_output.into()],
                ),
                DiagnosticMeta::default(),
            ),
            NodeDef::new(
                NodeKey::from_u128(121),
                NodeKind::toggle(LogicLevel::Low),
                NodePorts::with_input_roles(
                    vec![toggle_input.into()],
                    vec![InputPortRole::Toggle],
                    vec![toggle_output.into()],
                ),
                DiagnosticMeta::default(),
            ),
        ],
        Vec::new(),
        Vec::new(),
        vec![
            ConnectionDef::new(
                ConnectionKey::from_u128(150),
                periodic_output.into(),
                toggle_input.into(),
                DiagnosticMeta::default(),
            ),
            ConnectionDef::new(
                ConnectionKey::from_u128(151),
                toggle_output.into(),
                periodic_enable.into(),
                DiagnosticMeta::default(),
            ),
        ],
    )
    .validate();
    assert!(cycle.artifact().is_none());
}
