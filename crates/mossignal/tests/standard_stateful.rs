use mossignal::diagnostics::DiagnosticCode;
use mossignal::key::*;
use mossignal::metadata::DiagnosticMeta;
use mossignal::signal::{Level, LogicLevel, Pulse, PulseCount};
use mossignal::standard::*;
use mossignal::time::Time;
use mossignal::*;
use std::collections::BTreeSet;

const LOW: LogicLevel = LogicLevel::Low;
const HIGH: LogicLevel = LogicLevel::High;
fn level(value: bool) -> LogicLevel {
    if value { HIGH } else { LOW }
}
fn policy() -> RuntimePolicy {
    RuntimePolicy::builder()
        .max_internal_reactions(100)
        .max_evaluated_operations(100_000)
        .max_pending_events(100)
        .max_events_created_per_transaction(10_000)
        .max_required_provenance_growth(100_000)
        .build()
        .unwrap()
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Pulse,
    Level,
    Hold,
}
impl Kind {
    fn reference(self) -> StandardModuleRef {
        match self {
            Self::Pulse => StandardModuleRef::pulse_resettable_toggle(),
            Self::Level => StandardModuleRef::level_resettable_toggle(),
            Self::Hold => StandardModuleRef::level_resettable_sample_hold(),
        }
    }
    fn output(self) -> ModuleOutputKey<Level> {
        match self {
            Self::Pulse => pulse_resettable_toggle_state_key(),
            Self::Level => level_resettable_toggle_state_key(),
            Self::Hold => level_resettable_sample_hold_held_key(),
        }
    }
    fn request(self, initial: LogicLevel, target: LogicLevel) -> StandardModuleRequest<()> {
        let request = StandardModuleRequest::new(self.reference()).with_parameter(
            StandardParameterKey::new("initial"),
            StandardParameterValue::LogicLevel(initial),
        );
        if self == Self::Hold {
            request.with_parameter(
                StandardParameterKey::new("reset_to"),
                StandardParameterValue::LogicLevel(target),
            )
        } else {
            request
        }
    }
}
#[derive(Clone, Copy, Debug)]
struct Batch {
    value: LogicLevel,
    reset: LogicLevel,
    count: u64,
    reset_count: u64,
}
fn batch(reset: LogicLevel, count: u64, reset_count: u64) -> Batch {
    Batch {
        value: HIGH,
        reset,
        count,
        reset_count,
    }
}
struct Fixture {
    compiled: CompiledNetwork<()>,
    kind: Kind,
    instance: ModuleInstanceKey,
    output: ExternalOutputKey<Level>,
}
impl Fixture {
    fn new(kind: Kind, initial: LogicLevel, target: LogicLevel, dynamic: bool) -> Self {
        Self::with_delay(kind, initial, target, dynamic, false)
    }
    fn with_delay(
        kind: Kind,
        initial: LogicLevel,
        target: LogicLevel,
        dynamic: bool,
        delayed: bool,
    ) -> Self {
        let mut b = NetworkBuilder::with_key(NetworkKey::from_u128(1), TimeDomainId::from_u128(2));
        let pulse = b
            .add_pulse_input(ExternalInputKey::from_u128(10), DiagnosticMeta::default())
            .unwrap();
        let pulse = if delayed {
            b.pulse_delay(
                pulse,
                PulseDelayConfig::new(mossignal::time::NonZeroSpan::from_ticks(1).unwrap()),
            )
            .unwrap()
        } else {
            pulse
        };
        let reset_pulse = b
            .add_pulse_input(ExternalInputKey::from_u128(11), DiagnosticMeta::default())
            .unwrap();
        let reset = b
            .add_level_input(ExternalInputKey::from_u128(12), DiagnosticMeta::default())
            .unwrap();
        let value = b
            .add_level_input(ExternalInputKey::from_u128(13), DiagnosticMeta::default())
            .unwrap();
        let instance = ModuleInstanceKey::from_u128(20);
        let output = if dynamic {
            let module = StandardCatalogue::current()
                .build(kind.request(initial, target))
                .require_artifact()
                .unwrap();
            let pending = b
                .instantiate(&module, instance, DiagnosticMeta::default())
                .unwrap();
            let added = match kind {
                Kind::Pulse => pending
                    .bind_pulse(pulse_resettable_toggle_toggle_key(), pulse)
                    .unwrap()
                    .bind_pulse(pulse_resettable_toggle_reset_key(), reset_pulse)
                    .unwrap()
                    .finish()
                    .unwrap(),
                Kind::Level => pending
                    .bind_pulse(level_resettable_toggle_toggle_key(), pulse)
                    .unwrap()
                    .bind_level(level_resettable_toggle_reset_key(), reset)
                    .unwrap()
                    .finish()
                    .unwrap(),
                Kind::Hold => pending
                    .bind_pulse(level_resettable_sample_hold_sample_key(), pulse)
                    .unwrap()
                    .bind_level(level_resettable_sample_hold_reset_key(), reset)
                    .unwrap()
                    .bind_level(level_resettable_sample_hold_value_key(), value)
                    .unwrap()
                    .finish()
                    .unwrap(),
            };
            added.level_output(kind.output()).unwrap()
        } else {
            match kind {
                Kind::Pulse => b
                    .add_pulse_resettable_toggle(
                        instance,
                        pulse,
                        reset_pulse,
                        initial,
                        DiagnosticMeta::default(),
                    )
                    .unwrap()
                    .into_outputs(),
                Kind::Level => b
                    .add_level_resettable_toggle(
                        instance,
                        pulse,
                        reset,
                        initial,
                        DiagnosticMeta::default(),
                    )
                    .unwrap()
                    .into_outputs(),
                Kind::Hold => b
                    .add_level_resettable_sample_hold(
                        instance,
                        value,
                        pulse,
                        reset,
                        initial,
                        target,
                        DiagnosticMeta::default(),
                    )
                    .unwrap()
                    .into_outputs(),
            }
        };
        let out = ExternalOutputKey::from_u128(30);
        b.add_level_output(out, output, DiagnosticMeta::default())
            .unwrap();
        Self {
            compiled: b
                .finish()
                .require_artifact()
                .unwrap()
                .compile()
                .require_artifact()
                .unwrap(),
            kind,
            instance,
            output: out,
        }
    }
    fn apply(
        &self,
        m: &mut Machine<()>,
        at: u64,
        b: Batch,
    ) -> Result<TransactionResult<()>, RuntimeFailure<()>> {
        let tx = if m.is_initialized() {
            Transaction::advance(
                Time::from_ticks(at),
                m.revision(),
                self.compiled
                    .input_delta()
                    .set(ExternalInputKey::from_u128(12), b.reset)
                    .unwrap()
                    .set(ExternalInputKey::from_u128(13), b.value)
                    .unwrap()
                    .pulse(ExternalInputKey::from_u128(10), PulseCount::new(b.count))
                    .unwrap()
                    .pulse(
                        ExternalInputKey::from_u128(11),
                        PulseCount::new(b.reset_count),
                    )
                    .unwrap()
                    .finish()
                    .unwrap(),
            )
        } else {
            Transaction::initialize(
                Time::from_ticks(at),
                m.revision(),
                self.compiled
                    .input_snapshot()
                    .set(ExternalInputKey::from_u128(12), b.reset)
                    .unwrap()
                    .set(ExternalInputKey::from_u128(13), b.value)
                    .unwrap()
                    .pulse(ExternalInputKey::from_u128(10), PulseCount::new(b.count))
                    .unwrap()
                    .pulse(
                        ExternalInputKey::from_u128(11),
                        PulseCount::new(b.reset_count),
                    )
                    .unwrap()
                    .finish()
                    .unwrap(),
            )
        };
        m.apply(tx)
    }
}

// Independent public-state oracle: no primitive graph, operation order or evaluator calls.
#[derive(Clone, Copy)]
struct Reference {
    held: LogicLevel,
    reset: LogicLevel,
    gauge: LogicLevel,
}
impl Reference {
    fn step(&mut self, kind: Kind, target: LogicLevel, b: Batch) -> StatefulStandardReaction {
        let previous = self.held;
        let rose = self.reset == LOW && b.reset == HIGH;
        let result = if kind == Kind::Hold {
            if b.count > 0 || rose {
                if b.reset == HIGH { target } else { b.value }
            } else {
                previous
            }
        } else if (kind == Kind::Pulse && b.reset_count > 0)
            || (kind == Kind::Level && b.reset == HIGH)
        {
            LOW
        } else {
            level((previous == HIGH) ^ (b.count % 2 == 1))
        };
        if kind != Kind::Hold && (kind == Kind::Pulse || b.reset == LOW) && b.count % 2 == 1 {
            self.gauge = level(self.gauge == LOW);
        }
        self.held = result;
        self.reset = b.reset;
        if kind == Kind::Hold {
            let capture = match (b.count > 0, rose) {
                (false, false) => CaptureKind::None,
                (true, false) => CaptureKind::Sample,
                (false, true) => CaptureKind::Reset,
                (true, true) => CaptureKind::Both,
            };
            StatefulStandardReaction::SampleHold {
                previous,
                value: b.value,
                sample_count: PulseCount::new(b.count),
                reset: b.reset,
                reset_rose: rose,
                capture,
                selected: if b.reset == HIGH { target } else { b.value },
                result,
            }
        } else {
            let active = if kind == Kind::Pulse {
                b.reset_count > 0
            } else {
                b.reset == HIGH
            };
            StatefulStandardReaction::Toggle {
                previous,
                toggle_count: PulseCount::new(b.count),
                reset: if kind == Kind::Pulse {
                    ResetObservation::Pulse(PulseCount::new(b.reset_count))
                } else {
                    ResetObservation::Level {
                        level: b.reset,
                        rose,
                    }
                },
                accepted: PulseCount::new(if active { 0 } else { b.count }),
                suppressed: PulseCount::new(if active { b.count } else { 0 }),
                result,
            }
        }
    }
}
fn verify(
    f: &Fixture,
    m: &Machine<()>,
    reference: Reference,
    expected: StatefulStandardReaction,
    result: &TransactionResult<()>,
) {
    assert_eq!(m.output_level(f.output), Some(reference.held));
    assert_eq!(result.schedule(), Schedule::Dormant);
    assert!(result.occurrences().is_empty());
    assert!(result.diagnostic_episode_changes().is_empty());
    let view = m.inspect_module(f.instance).unwrap();
    verify_reference_support(&view, expected);
    let obs = view.stateful_standard().unwrap();
    assert_eq!(obs.state, reference.held);
    assert_eq!(obs.last_reaction, expected);
    assert!(obs.provenance.inspect(obs.internal_causes[0].1).is_ok());
    for node in view.nodes() {
        assert!(node.standard_role().is_some());
        assert!(obs.provenance.inspect(node.cause().unwrap()).is_ok());
    }
    for (_, cause) in &obs.public_causes {
        assert!(obs.provenance.inspect(*cause).is_ok());
    }
    for cause in [
        obs.latest_reset_cause,
        obs.latest_capture_cause,
        obs.latest_accepted_toggle_cause,
    ]
    .into_iter()
    .flatten()
    {
        assert!(obs.provenance.inspect(cause).is_ok());
    }
    if f.kind != Kind::Hold {
        assert_eq!(obs.toggle_state, Some(reference.gauge));
        assert_eq!(
            obs.reset_baseline,
            Some(level(reference.gauge != reference.held))
        );
        assert_eq!(
            level(obs.toggle_state.unwrap() != obs.reset_baseline.unwrap()),
            reference.held
        );
        if obs.remembered_reset == Some(HIGH) {
            assert_eq!(obs.toggle_state, obs.reset_baseline);
        }
    }
    assert_eq!(
        view.inputs()
            .iter()
            .filter(|p| p.key().kind() == mossignal::signal::SignalKind::Pulse)
            .filter(|p| p.level().is_some())
            .count(),
        0
    );
}
#[test]
fn exhaustive_public_laws_and_histories_match_canonical_state() {
    let counts = [0, 1, 2, 3, 16, 17, u64::MAX];
    for kind in [Kind::Pulse, Kind::Level, Kind::Hold] {
        for initial in [LOW, HIGH] {
            for target in [LOW, HIGH] {
                if kind != Kind::Hold && target == HIGH {
                    continue;
                }
                let f = Fixture::new(kind, initial, target, false);
                let mut batches = Vec::new();
                match kind {
                    Kind::Pulse => {
                        for count in counts {
                            for reset_count in [0, 1, 2, u64::MAX] {
                                batches.push(batch(LOW, count, reset_count));
                            }
                        }
                    }
                    Kind::Level => {
                        for count in counts {
                            for reset in [LOW, HIGH] {
                                batches.push(batch(reset, count, 0));
                            }
                        }
                    }
                    Kind::Hold => {
                        for count in [0, 1, 2, 17] {
                            for reset in [LOW, HIGH] {
                                for value in [LOW, HIGH] {
                                    batches.push(Batch {
                                        value,
                                        reset,
                                        count,
                                        reset_count: 0,
                                    });
                                }
                            }
                        }
                    }
                }
                let mut internal_pairs = BTreeSet::new();
                for first in &batches {
                    for second in &batches {
                        let mut m = f.compiled.spawn(policy());
                        let mut reference = Reference {
                            held: initial,
                            reset: LOW,
                            gauge: initial,
                        };
                        assert!(matches!(
                            m.inspect_module(f.instance),
                            Err(ModuleInspectionFailure::NotInitialized)
                        ));
                        for (index, b) in
                            [*first, *second, batch(LOW, 0, 0)].into_iter().enumerate()
                        {
                            let expected = reference.step(kind, target, b);
                            let result = f.apply(&mut m, index as u64 + 1, b).unwrap();
                            verify(&f, &m, reference, expected, &result);
                            if kind != Kind::Hold {
                                let view = m.inspect_module(f.instance).unwrap();
                                let obs = view.stateful_standard().unwrap();
                                internal_pairs.insert((
                                    obs.toggle_state.unwrap(),
                                    obs.reset_baseline.unwrap(),
                                ));
                            }
                        }
                    }
                }
                if kind == Kind::Pulse {
                    assert_eq!(internal_pairs.len(), 4);
                }
            }
        }
    }
}

#[test]
fn dynamic_and_keyed_paths_have_identical_identity_and_observations() {
    for kind in [Kind::Pulse, Kind::Level, Kind::Hold] {
        for initial in [LOW, HIGH] {
            for target in [LOW, HIGH] {
                let a = Fixture::new(kind, initial, target, false);
                let b = Fixture::new(kind, initial, target, true);
                assert_eq!(a.compiled.fingerprint(), b.compiled.fingerprint());
                let mut left = a.compiled.spawn(policy());
                let mut right = b.compiled.spawn(policy());
                for (at, batch) in [
                    batch(LOW, 3, 0),
                    batch(HIGH, 2, 1),
                    batch(HIGH, 3, 0),
                    batch(LOW, 1, 0),
                ]
                .into_iter()
                .enumerate()
                {
                    let l = a.apply(&mut left, at as u64, batch).unwrap();
                    let r = b.apply(&mut right, at as u64, batch).unwrap();
                    assert_eq!(format!("{:?}", l), format!("{:?}", r));
                    let li = left.inspect_module(a.instance).unwrap();
                    let ri = right.inspect_module(b.instance).unwrap();
                    assert_eq!(
                        li.stateful_standard().unwrap().last_reaction,
                        ri.stateful_standard().unwrap().last_reaction
                    );
                    assert_eq!(
                        observation(&left, a.instance),
                        observation(&right, b.instance)
                    );
                }
            }
        }
    }
}

#[test]
fn descriptors_parameters_and_canonical_roles_are_exact() {
    let catalogue = StandardCatalogue::<()>::current();
    for kind in [Kind::Pulse, Kind::Level, Kind::Hold] {
        let reference = kind.reference();
        let descriptor = catalogue.descriptor(&reference).unwrap();
        assert!(descriptor.is_stateful());
        assert!(!descriptor.is_temporal());
        assert!(
            descriptor
                .inputs()
                .iter()
                .all(|p| !p.is_variadic() && p.fixed_input().is_some())
        );
        assert!(
            descriptor
                .parameters()
                .iter()
                .all(|p| p.is_required() && p.kind() == StandardParameterKind::LogicLevel)
        );
        let mut identities = BTreeSet::new();
        for initial in [LOW, HIGH] {
            for target in [LOW, HIGH] {
                if kind != Kind::Hold && target == HIGH {
                    continue;
                }
                let module = catalogue
                    .build(kind.request(initial, target))
                    .require_artifact()
                    .unwrap();
                assert!(identities.insert(module.fingerprint()));
                let declaration = module.standard_declaration().unwrap();
                assert_eq!(
                    declaration.public_dependencies().len(),
                    descriptor.inputs().len()
                );
                let roles: BTreeSet<_> = declaration
                    .internal_roles()
                    .filter(|r| r.category() == StandardInternalCategory::Node)
                    .map(|r| r.role())
                    .collect();
                let expected: BTreeSet<_> = match kind {
                    Kind::Pulse => vec!["toggle_state", "reset_baseline", "relative_state"],
                    Kind::Level => vec![
                        "toggle_state",
                        "reset_baseline",
                        "relative_state",
                        "reset_inverter",
                        "accepted_toggle",
                        "low_constant",
                        "reset_edge",
                        "reset_select",
                    ],
                    Kind::Hold => vec![
                        "reset_value_constant",
                        "capture_value",
                        "reset_edge",
                        "capture_pulses",
                        "held_state",
                    ],
                }
                .into_iter()
                .collect();
                assert_eq!(roles, expected);
                let unique: BTreeSet<_> = declaration
                    .internal_roles()
                    .map(|r| (r.category(), r.key()))
                    .collect();
                assert_eq!(unique.len(), declaration.internal_roles().len());
                let reordered = if kind == Kind::Hold {
                    StandardModuleRequest::new(reference.clone())
                        .with_parameter(
                            StandardParameterKey::new("reset_to"),
                            StandardParameterValue::LogicLevel(target),
                        )
                        .with_parameter(
                            StandardParameterKey::new("initial"),
                            StandardParameterValue::LogicLevel(initial),
                        )
                } else {
                    kind.request(initial, target)
                };
                assert_eq!(
                    module.fingerprint(),
                    catalogue
                        .build(reordered)
                        .require_artifact()
                        .unwrap()
                        .fingerprint()
                );
            }
        }
        for request in [
            StandardModuleRequest::new(reference.clone()),
            kind.request(LOW, HIGH).with_parameter(
                StandardParameterKey::new("initial"),
                StandardParameterValue::LogicLevel(HIGH),
            ),
            kind.request(LOW, HIGH).with_parameter(
                StandardParameterKey::new("unknown"),
                StandardParameterValue::U64(1),
            ),
            kind.request(LOW, HIGH)
                .with_variadic_input(ModuleInputKey::<Pulse>::from_u128(1).into()),
        ] {
            assert!(catalogue.build(request).require_artifact().is_err());
        }
        let report = catalogue.build(StandardModuleRequest::new(reference).with_parameter(
            StandardParameterKey::new("initial"),
            StandardParameterValue::U64(1),
        ));
        assert!(
            report
                .diagnostics()
                .iter()
                .any(|d| d.problem().code() == DiagnosticCode::StandardModuleParameterKindMismatch)
        );
    }
}

#[test]
fn nested_standard_roles_remain_visible_from_the_user_module() {
    let mut wrapper = ModuleBuilder::<()>::new();
    let (public, sig) = wrapper.pulse_input("toggle");
    let (_, reset) = wrapper.pulse_input("reset");
    let inner = wrapper
        .add_pulse_resettable_toggle(
            ModuleInstanceKey::from_u128(100),
            sig,
            reset,
            HIGH,
            DiagnosticMeta::default(),
        )
        .unwrap();
    let out = wrapper.level_output("state", inner.into_outputs()).unwrap();
    let module = wrapper.finish().require_artifact().unwrap();
    let mut network = NetworkBuilder::new(TimeDomainId::from_u128(1));
    let (input, pulse) = network.pulse_input("pulse");
    let key = ModuleInstanceKey::from_u128(200);
    let mut inst = network
        .instantiate(&module, key, DiagnosticMeta::default())
        .unwrap();
    for def in module.inputs() {
        if let AnyModuleInputKey::Pulse(k) = def.key() {
            inst = inst.bind_pulse(k, pulse).unwrap();
        }
    }
    let added = inst.finish().unwrap();
    let output = network
        .level_output("out", added.level_output(out).unwrap())
        .unwrap();
    let compiled = network
        .finish()
        .require_artifact()
        .unwrap()
        .compile()
        .require_artifact()
        .unwrap();
    let mut m = compiled.spawn(policy());
    m.apply(Transaction::initialize(
        Time::from_ticks(3),
        m.revision(),
        compiled
            .input_snapshot()
            .pulse(input, PulseCount::ONE)
            .unwrap()
            .finish()
            .unwrap(),
    ))
    .unwrap();
    assert_eq!(m.output_level(output), Some(LOW));
    let view = m.inspect_module(key).unwrap();
    assert!(
        view.nodes().iter().all(|n| n.standard_role().is_some()),
        "ancestor expanded view must preserve descendant standard roles"
    );
    assert!(view.stateful_standard().is_none());
    let path =
        QualifiedModuleRef::from_instances(vec![key, ModuleInstanceKey::from_u128(100)]).unwrap();
    let child = m.inspect_qualified_module(path.clone()).unwrap();
    let obs = child.stateful_standard().unwrap();
    assert_eq!(obs.module, path);
    assert_eq!(obs.state, LOW);
    assert_eq!(obs.internal_causes.len(), 3);
    assert!(
        obs.internal_causes
            .iter()
            .all(|(node, _)| node.instances() == path.instances())
    );
    assert!(module.inputs().any(|p| p.key() == public.into()));
}

fn normalized_causes(
    view: &ProvenanceView<()>,
    roots: impl IntoIterator<Item = CauseRef>,
) -> Vec<String> {
    let mut pending: Vec<_> = roots.into_iter().collect();
    let mut seen = BTreeSet::new();
    let mut records = Vec::new();
    while let Some(cause) = pending.pop() {
        if !seen.insert(cause) {
            continue;
        }
        match view.inspect(cause).unwrap() {
            CauseInspection::InitializationTransaction { at, .. } => {
                records.push(format!("init {}", at.time().ticks()))
            }
            CauseInspection::ReadyTransaction { at, .. } => {
                records.push(format!("ready {}", at.time().ticks()))
            }
            CauseInspection::ExternalObservation { input, value, .. } => {
                records.push(format!("level {input:?} {value:?}"))
            }
            CauseInspection::ExternalPulseObservation { input, count, .. } => {
                records.push(format!("pulse {input:?} {}", count.get()))
            }
            CauseInspection::Derived {
                subject,
                supporters,
            } => {
                records.push(format!("derived {subject:?}"));
                pending.extend_from_slice(supporters);
            }
            CauseInspection::PulseDerived {
                subject,
                contributions,
                result,
                supporters,
            } => {
                records.push(format!(
                    "pulse-derived {subject:?} {} {:?}",
                    result.get(),
                    contributions
                        .iter()
                        .map(|c| (c.port().clone(), c.count()))
                        .collect::<Vec<_>>()
                ));
                pending.extend_from_slice(supporters);
            }
            CauseInspection::PulseControlledLevel {
                subject,
                contributions,
                result,
                supporters,
            } => {
                records.push(format!(
                    "capture {subject:?} {result:?} {:?}",
                    contributions
                        .iter()
                        .map(|c| (c.port().clone(), c.count()))
                        .collect::<Vec<_>>()
                ));
                pending.extend_from_slice(supporters);
            }
            CauseInspection::PendingPulseDelay {
                owner,
                origin,
                deadline,
                count,
                supporters,
                ..
            } => {
                records.push(format!(
                    "pending {owner:?} {} {} {}",
                    origin.ticks(),
                    deadline.ticks(),
                    count.get()
                ));
                pending.extend_from_slice(supporters);
            }
            _ => panic!("unexpected pending work"),
        }
    }
    records.sort();
    records
}
fn observation(
    m: &Machine<()>,
    instance: ModuleInstanceKey,
) -> (StatefulStandardReaction, Vec<String>) {
    let view = m.inspect_module(instance).unwrap();
    let obs = view.stateful_standard().unwrap();
    (
        obs.last_reaction,
        normalized_causes(
            &obs.provenance,
            obs.internal_causes
                .iter()
                .map(|(_, c)| *c)
                .chain(obs.public_causes.iter().map(|(_, c)| *c)),
        ),
    )
}

#[test]
fn observations_retain_owned_provenance_and_latest_accepted_even_batches() {
    for kind in [Kind::Pulse, Kind::Level, Kind::Hold] {
        let f = Fixture::new(kind, HIGH, LOW, false);
        let mut m = f.compiled.spawn(policy());
        f.apply(&mut m, 1, batch(LOW, 2, 0)).unwrap();
        let retained = m.inspect_module(f.instance).unwrap();
        let obs = retained.stateful_standard().unwrap();
        let roots = obs
            .internal_causes
            .iter()
            .map(|(_, cause)| *cause)
            .collect::<Vec<_>>();
        let before = normalized_causes(&obs.provenance, roots.clone());
        if kind != Kind::Hold {
            assert!(obs.latest_accepted_toggle_cause.is_some());
            assert_eq!(
                obs.last_reaction.why_not_change(),
                Some(StatefulWhyNot::EvenToggleParity)
            );
        }
        f.apply(&mut m, 2, batch(HIGH, 3, 7)).unwrap();
        let reset = m.inspect_module(f.instance).unwrap();
        let current = reset.stateful_standard().unwrap();
        assert!(current.latest_reset_cause.is_some());
        if kind != Kind::Hold {
            assert_eq!(
                current.last_reaction.why_not_high(),
                Some(StatefulWhyNot::ResetDominated)
            );
        } else {
            assert_eq!(
                current.last_reaction.why_not_sample_value(),
                Some(StatefulWhyNot::ResetSelectedTarget)
            );
            assert!(current.latest_capture_cause.is_some());
        }
        f.apply(&mut m, 3, batch(LOW, 0, 0)).unwrap();
        assert_eq!(normalized_causes(&obs.provenance, roots), before);
        let quiet = m.inspect_module(f.instance).unwrap();
        let quiet = quiet.stateful_standard().unwrap();
        assert!(quiet.latest_reset_cause.is_some());
        assert!(
            quiet
                .provenance
                .inspect(quiet.latest_reset_cause.unwrap())
                .is_ok()
        );
        if kind != Kind::Hold {
            assert_eq!(
                quiet.last_reaction.why_not_change(),
                Some(StatefulWhyNot::NoToggle)
            );
        } else {
            assert_eq!(
                quiet.last_reaction.why_not_change(),
                Some(StatefulWhyNot::NoCapture)
            );
        }
    }
}

#[test]
fn checked_merge_overflow_rolls_back_every_observable_and_allows_retry() {
    let f = Fixture::new(Kind::Hold, HIGH, LOW, false);
    let mut machine = f.compiled.spawn(policy());
    let mut control = f.compiled.spawn(policy());
    let failed = f
        .apply(&mut machine, 1, batch(HIGH, u64::MAX, 0))
        .unwrap_err();
    assert_eq!(failed.code(), DiagnosticCode::RuntimePulseCountOverflow);
    assert!(!machine.is_initialized());
    for m in [&mut machine, &mut control] {
        f.apply(m, 1, batch(LOW, 0, 0)).unwrap();
    }
    let before = observation(&machine, f.instance);
    let failure = f
        .apply(&mut machine, 2, batch(HIGH, u64::MAX, 0))
        .unwrap_err();
    assert_eq!(failure.code(), DiagnosticCode::RuntimePulseCountOverflow);
    assert_eq!(machine.now(), Some(Time::from_ticks(1)));
    assert_eq!(observation(&machine, f.instance), before);
    let a = f.apply(&mut machine, 2, batch(HIGH, 1, 0)).unwrap();
    let b = f.apply(&mut control, 2, batch(HIGH, 1, 0)).unwrap();
    assert_eq!(format!("{a:?}"), format!("{b:?}"));
    assert_eq!(
        observation(&machine, f.instance),
        observation(&control, f.instance)
    );
    // A held reset has no edge to add, so the full representable count succeeds.
    f.apply(&mut machine, 3, batch(HIGH, u64::MAX, 0)).unwrap();
    assert_eq!(machine.output_level(f.output), Some(LOW));
}

#[test]
fn budget_failures_do_not_publish_aggregate_history() {
    for kind in [Kind::Pulse, Kind::Level, Kind::Hold] {
        let f = Fixture::with_delay(kind, HIGH, LOW, false, true);
        let mut saw_late_failure = false;
        for limit in 1..160 {
            let policy = RuntimePolicy::builder()
                .max_internal_reactions(100)
                .max_evaluated_operations(100_000)
                .max_pending_events(100)
                .max_events_created_per_transaction(100)
                .max_required_provenance_growth(limit)
                .build()
                .unwrap();
            let mut m = f.compiled.spawn(policy);
            if f.apply(&mut m, 1, batch(LOW, 1, 0)).is_err() {
                assert!(!m.is_initialized());
                continue;
            }
            let before = observation(&m, f.instance);
            if let Err(error) = f.apply(&mut m, 3, batch(HIGH, 3, 1)) {
                assert_eq!(error.code(), DiagnosticCode::RuntimeBudgetExceeded);
                assert_eq!(m.now(), Some(Time::from_ticks(1)));
                assert_eq!(observation(&m, f.instance), before);
                saw_late_failure = true;
                break;
            }
        }
        assert!(
            saw_late_failure,
            "must exercise rejection after initialized history exists: {kind:?}"
        );
    }
}

#[test]
fn foreign_signals_and_duplicate_instances_leave_builders_usable() {
    let mut foreign = NetworkBuilder::<()>::new(TimeDomainId::from_u128(1));
    let (_, bad) = foreign.pulse_input("foreign");
    for kind in [Kind::Pulse, Kind::Level, Kind::Hold] {
        let mut b = NetworkBuilder::<()>::new(TimeDomainId::from_u128(1));
        let (_, pulse) = b.pulse_input("p");
        let (_, reset) = b.level_input("reset");
        let (_, value) = b.level_input("value");
        let key = ModuleInstanceKey::from_u128(90);
        let result = match kind {
            Kind::Pulse => {
                b.add_pulse_resettable_toggle(key, bad, pulse, LOW, DiagnosticMeta::default())
            }
            Kind::Level => {
                b.add_level_resettable_toggle(key, bad, reset, LOW, DiagnosticMeta::default())
            }
            Kind::Hold => b.add_level_resettable_sample_hold(
                key,
                value,
                bad,
                reset,
                LOW,
                HIGH,
                DiagnosticMeta::default(),
            ),
        };
        assert!(matches!(
            result,
            Err(AuthoringFailure::ForeignSignal { .. })
        ));
        let mut construct = || match kind {
            Kind::Pulse => {
                b.add_pulse_resettable_toggle(key, pulse, pulse, LOW, DiagnosticMeta::default())
            }
            Kind::Level => {
                b.add_level_resettable_toggle(key, pulse, reset, LOW, DiagnosticMeta::default())
            }
            Kind::Hold => b.add_level_resettable_sample_hold(
                key,
                value,
                pulse,
                reset,
                LOW,
                HIGH,
                DiagnosticMeta::default(),
            ),
        };
        assert!(construct().is_ok());
        assert!(matches!(
            construct(),
            Err(AuthoringFailure::DuplicateModuleInstanceKey(_))
        ));
        assert_eq!(
            b.finish()
                .require_artifact()
                .unwrap()
                .graph()
                .module_instances()
                .len(),
            1
        );
    }
}

#[test]
fn canonical_wiring_configs_and_user_origin_are_verified_independently() {
    use mossignal::authored::*;
    for kind in [Kind::Pulse, Kind::Level, Kind::Hold] {
        let module = StandardCatalogue::<()>::current()
            .build(kind.request(HIGH, LOW))
            .require_artifact()
            .unwrap();
        let declaration = module.standard_declaration().unwrap();
        let graph = module.graph();
        let role = |key: NodeKey| {
            declaration
                .internal_roles()
                .find(|r| {
                    r.category() == StandardInternalCategory::Node && r.key() == key.as_u128()
                })
                .unwrap()
                .role()
                .to_owned()
        };
        let reference = kind.reference();
        let catalogue = StandardCatalogue::<()>::current();
        let descriptor = catalogue.descriptor(&reference).unwrap();
        let endpoint = |endpoint| match endpoint {
            ConnectionEndpoint::NodeInput(key) => {
                let n = graph
                    .nodes()
                    .iter()
                    .find(|n| n.ports().inputs().contains(&key))
                    .unwrap();
                let index = n.ports().inputs().iter().position(|p| *p == key).unwrap();
                format!("{}.{:?}", role(n.key()), n.ports().input_roles()[index])
            }
            ConnectionEndpoint::NodeOutput(key) => role(
                graph
                    .nodes()
                    .iter()
                    .find(|n| n.ports().outputs().contains(&key))
                    .unwrap()
                    .key(),
            ),
            ConnectionEndpoint::ModuleInput(key) => format!(
                "public.{}",
                descriptor
                    .inputs()
                    .iter()
                    .find(|p| p.fixed_input() == Some(key))
                    .unwrap()
                    .role()
            ),
            _ => panic!("canonical fixed expansion endpoint"),
        };
        let mut edges: BTreeSet<_> = graph
            .connections()
            .iter()
            .map(|c| format!("{} -> {}", endpoint(c.from()), endpoint(c.to())))
            .collect();
        for mapping in graph.mappings() {
            match *mapping {
                ModuleInterfaceMapping::Input { input, target } => {
                    edges.insert(format!(
                        "{} -> {}",
                        endpoint(ConnectionEndpoint::module_input(input)),
                        endpoint(target)
                    ));
                }
                ModuleInterfaceMapping::Output { source, .. } => {
                    edges.insert(format!("{} -> public.output", endpoint(source)));
                }
                _ => panic!("unexpected mapping"),
            }
        }
        let expected = match kind {
            Kind::Pulse => vec![
                "public.toggle -> toggle_state.Toggle",
                "public.reset -> reset_baseline.Sample",
                "toggle_state -> reset_baseline.Value",
                "toggle_state -> relative_state.Input",
                "reset_baseline -> relative_state.Input",
                "relative_state -> public.output",
            ],
            Kind::Level => vec![
                "public.reset -> reset_inverter.Input",
                "public.toggle -> accepted_toggle.Pulses",
                "reset_inverter -> accepted_toggle.Enable",
                "accepted_toggle -> toggle_state.Toggle",
                "public.reset -> reset_edge.Input",
                "toggle_state -> reset_baseline.Value",
                "reset_edge -> reset_baseline.Sample",
                "toggle_state -> relative_state.Input",
                "reset_baseline -> relative_state.Input",
                "public.reset -> reset_select.Selector",
                "relative_state -> reset_select.WhenLow",
                "low_constant -> reset_select.WhenHigh",
                "reset_select -> public.output",
            ],
            Kind::Hold => vec![
                "public.reset -> capture_value.Selector",
                "public.value -> capture_value.WhenLow",
                "reset_value_constant -> capture_value.WhenHigh",
                "public.reset -> reset_edge.Input",
                "public.sample -> capture_pulses.Input",
                "reset_edge -> capture_pulses.Input",
                "capture_value -> held_state.Value",
                "capture_pulses -> held_state.Sample",
                "held_state -> public.output",
            ],
        };
        assert_eq!(edges, expected.into_iter().map(str::to_owned).collect());
        for node in graph.nodes() {
            let expected = match role(node.key()).as_str() {
                "toggle_state" => NodeKind::toggle(HIGH),
                "reset_baseline" => NodeKind::sample_hold(SampleHoldConfig::new(LOW)),
                "relative_state" => NodeKind::parity(),
                "reset_inverter" => NodeKind::not(),
                "accepted_toggle" => NodeKind::pulse_gate(),
                "low_constant" | "reset_value_constant" => NodeKind::constant(LOW),
                "reset_edge" => {
                    NodeKind::rising_edge(EdgeConfig::new(EdgeInitialization::Assume(LOW)))
                }
                "reset_select" | "capture_value" => NodeKind::select(),
                "capture_pulses" => NodeKind::merge(),
                "held_state" => NodeKind::sample_hold(SampleHoldConfig::new(HIGH)),
                _ => panic!("unexpected role"),
            };
            assert_eq!(node.kind(), &expected);
        }
        let mut nodes = graph.nodes().to_vec();
        nodes.reverse();
        let mut connections = graph.connections().to_vec();
        connections.reverse();
        let user = UncheckedModule::new_user(
            DiagnosticMeta::default(),
            module.inputs().cloned().collect(),
            module.outputs().cloned().collect(),
            graph.mappings().to_vec(),
            nodes,
            connections,
        )
        .validate()
        .require_artifact()
        .unwrap();
        assert_ne!(user.fingerprint(), module.fingerprint());
    }
}

#[test]
fn concise_construction_in_both_builders_uses_the_same_canonical_modules() {
    for kind in [Kind::Pulse, Kind::Level, Kind::Hold] {
        let expected = StandardCatalogue::<()>::current()
            .build(kind.request(HIGH, LOW))
            .require_artifact()
            .unwrap();
        let mut module = ModuleBuilder::<()>::new();
        let (_, p) = module.pulse_input("pulse");
        let (_, r) = module.level_input("reset");
        let (_, v) = module.level_input("value");
        let out = match kind {
            Kind::Pulse => module.pulse_resettable_toggle(p, p, HIGH),
            Kind::Level => module.level_resettable_toggle(p, r, HIGH),
            Kind::Hold => module.level_resettable_sample_hold(v, p, r, HIGH, LOW),
        }
        .unwrap();
        module.level_output("result", out).unwrap();
        let built = module.finish().require_artifact().unwrap();
        assert_eq!(
            built.graph().module_instances()[0].module().fingerprint(),
            expected.fingerprint()
        );
        let mut network = NetworkBuilder::<()>::new(TimeDomainId::from_u128(1));
        let (_, p) = network.pulse_input("pulse");
        let (_, r) = network.level_input("reset");
        let (_, v) = network.level_input("value");
        let out = match kind {
            Kind::Pulse => network.pulse_resettable_toggle(p, p, HIGH),
            Kind::Level => network.level_resettable_toggle(p, r, HIGH),
            Kind::Hold => network.level_resettable_sample_hold(v, p, r, HIGH, LOW),
        }
        .unwrap();
        network.level_output("result", out).unwrap();
        let built = network.finish().require_artifact().unwrap();
        assert_eq!(
            built.graph().module_instances()[0].module().fingerprint(),
            expected.fingerprint()
        );
    }
}

#[test]
fn current_dependencies_cross_standard_boundaries_and_reject_cycles() {
    use mossignal::authored::*;
    for kind in [Kind::Pulse, Kind::Level, Kind::Hold] {
        let module = StandardCatalogue::<()>::current()
            .build(kind.request(LOW, HIGH))
            .require_artifact()
            .unwrap();
        let instance = ModuleInstanceKey::from_u128(1);
        let input = InPortKey::<Level>::from_u128(2);
        let output = OutPortKey::<Pulse>::from_u128(3);
        let bindings = module
            .inputs()
            .map(|p| {
                ModuleBinding::new(
                    p.key(),
                    match p.key().kind() {
                        mossignal::signal::SignalKind::Pulse => {
                            ConnectionEndpoint::node_output(output.into())
                        }
                        mossignal::signal::SignalKind::Level => {
                            ConnectionEndpoint::module_output(instance, kind.output().into())
                        }
                        _ => panic!("unexpected signal"),
                    },
                )
            })
            .collect();
        let unchecked = UncheckedNetwork::new_with_instances(
            NetworkKey::from_u128(4),
            TimeDomainId::from_u128(5),
            DiagnosticMeta::default(),
            vec![NodeDef::new(
                NodeKey::from_u128(6),
                NodeKind::rising_edge(EdgeConfig::new(EdgeInitialization::Assume(LOW))),
                NodePorts::new(vec![input.into()], vec![output.into()]),
                DiagnosticMeta::default(),
            )],
            vec![],
            vec![],
            vec![ConnectionDef::new(
                ConnectionKey::from_u128(7),
                ConnectionEndpoint::module_output(instance, kind.output().into()),
                ConnectionEndpoint::node_input(input.into()),
                DiagnosticMeta::default(),
            )],
            vec![ModuleInstanceDef::new(
                instance,
                module,
                ModuleBindingSet::new(bindings),
                None,
                DiagnosticMeta::default(),
            )],
        );
        let report = unchecked.validate();
        assert!(report.artifact().is_none());
        assert!(
            report
                .diagnostics()
                .iter()
                .any(|d| d.problem().code() == DiagnosticCode::ValidationCurrentReactionCycle)
        );
    }
}

#[test]
fn earlier_deadline_observations_survive_a_quiet_target_reaction() {
    for kind in [Kind::Pulse, Kind::Level, Kind::Hold] {
        let f = Fixture::with_delay(kind, LOW, LOW, false, true);
        let mut direct = f.compiled.spawn(policy());
        let mut stepped = f.compiled.spawn(policy());
        for m in [&mut direct, &mut stepped] {
            f.apply(m, 1, batch(LOW, 1, 0)).unwrap();
        }
        f.apply(&mut direct, 4, batch(LOW, 0, 0)).unwrap();
        f.apply(&mut stepped, 2, batch(LOW, 0, 0)).unwrap();
        f.apply(&mut stepped, 4, batch(LOW, 0, 0)).unwrap();
        let a = direct.inspect_module(f.instance).unwrap();
        let b = stepped.inspect_module(f.instance).unwrap();
        let a = a.stateful_standard().unwrap();
        let b = b.stateful_standard().unwrap();
        assert_eq!(a.state, HIGH);
        assert_eq!(a.last_reaction, b.last_reaction);
        let cause = if kind == Kind::Hold {
            a.latest_capture_cause
        } else {
            a.latest_accepted_toggle_cause
        }
        .unwrap();
        assert!(
            normalized_causes(&a.provenance, [cause])
                .iter()
                .any(|s| s.starts_with("pending "))
        );
        assert_eq!(
            a.last_reaction.why_not_change(),
            Some(if kind == Kind::Hold {
                StatefulWhyNot::NoCapture
            } else {
                StatefulWhyNot::NoToggle
            })
        );
    }
}

#[test]
fn distinct_instances_keep_independent_state_and_qualified_causes() {
    for kind in [Kind::Pulse, Kind::Level, Kind::Hold] {
        let mut module = ModuleBuilder::<()>::new();
        let (pulse_key, pulse) = module.pulse_input("work");
        let (reset_pulse_key, reset_pulse) = module.pulse_input("reset pulse");
        let (reset_key, reset) = module.level_input("reset");
        let (value_key, value) = module.level_input("value");
        let inner = ModuleInstanceKey::from_u128(5);
        let state = match kind {
            Kind::Pulse => module.add_pulse_resettable_toggle(
                inner,
                pulse,
                reset_pulse,
                LOW,
                DiagnosticMeta::default(),
            ),
            Kind::Level => module.add_level_resettable_toggle(
                inner,
                pulse,
                reset,
                LOW,
                DiagnosticMeta::default(),
            ),
            Kind::Hold => module.add_level_resettable_sample_hold(
                inner,
                value,
                pulse,
                reset,
                LOW,
                LOW,
                DiagnosticMeta::default(),
            ),
        }
        .unwrap()
        .into_outputs();
        let output_key = module.level_output("state", state).unwrap();
        let module = module.finish().require_artifact().unwrap();
        let mut b = NetworkBuilder::<()>::new(TimeDomainId::from_u128(1));
        let (left, p) = b.pulse_input("left");
        let (right, q) = b.pulse_input("right");
        let (_, rp) = b.pulse_input("reset pulse");
        let (r, reset) = b.level_input("reset");
        let (v, value) = b.level_input("value");
        let mut outputs = Vec::new();
        let mut paths = Vec::new();
        for (index, work) in [p, q].into_iter().enumerate() {
            let instance = ModuleInstanceKey::from_u128(30 + index as u128);
            let added = b
                .instantiate(&module, instance, DiagnosticMeta::default())
                .unwrap()
                .bind_pulse(pulse_key, work)
                .unwrap()
                .bind_pulse(reset_pulse_key, rp)
                .unwrap()
                .bind_level(reset_key, reset)
                .unwrap()
                .bind_level(value_key, value)
                .unwrap()
                .finish()
                .unwrap();
            outputs.push(
                b.level_output("result", added.level_output(output_key).unwrap())
                    .unwrap(),
            );
            paths.push(QualifiedModuleRef::from_instances(vec![instance, inner]).unwrap());
        }
        let compiled = b
            .finish()
            .require_artifact()
            .unwrap()
            .compile()
            .require_artifact()
            .unwrap();
        let mut machine = compiled.spawn(policy());
        machine
            .apply(Transaction::initialize(
                Time::from_ticks(1),
                machine.revision(),
                compiled
                    .input_snapshot()
                    .set(r, LOW)
                    .unwrap()
                    .set(v, HIGH)
                    .unwrap()
                    .pulse(left, PulseCount::ONE)
                    .unwrap()
                    .finish()
                    .unwrap(),
            ))
            .unwrap();
        assert_eq!(machine.output_level(outputs[0]), Some(HIGH));
        assert_eq!(machine.output_level(outputs[1]), Some(LOW));
        machine
            .apply(Transaction::advance(
                Time::from_ticks(2),
                machine.revision(),
                compiled
                    .input_delta()
                    .pulse(right, PulseCount::ONE)
                    .unwrap()
                    .finish()
                    .unwrap(),
            ))
            .unwrap();
        for (output, path) in outputs.iter().zip(&paths) {
            assert_eq!(machine.output_level(*output), Some(HIGH));
            let inspection = machine.inspect_qualified_module(path.clone()).unwrap();
            let obs = inspection.stateful_standard().unwrap();
            assert_eq!(obs.state, HIGH);
            for (node, cause) in &obs.internal_causes {
                assert_eq!(node.instances(), path.instances());
                match obs.provenance.inspect(*cause).unwrap() {
                    CauseInspection::Derived {
                        subject: ProvenanceSubject::QualifiedNode(subject),
                        ..
                    }
                    | CauseInspection::PulseDerived {
                        subject: ProvenanceSubject::QualifiedNode(subject),
                        ..
                    }
                    | CauseInspection::PulseControlledLevel {
                        subject: ProvenanceSubject::QualifiedNode(subject),
                        ..
                    } => assert_eq!(&subject, node),
                    _ => panic!("standard primitive provenance must remain qualified"),
                }
            }
        }
    }
}

// Compare causal counts against the independent public oracle, stripping storage IDs.
fn verify_reference_support(view: &ModuleInspection<()>, expected: StatefulStandardReaction) {
    let obs = view.stateful_standard().unwrap();
    let (work, reset_count, accepted, capture_count) = match expected {
        StatefulStandardReaction::Toggle {
            toggle_count,
            reset,
            accepted,
            ..
        } => {
            let count = match reset {
                ResetObservation::Pulse(count) => count.get(),
                ResetObservation::Level { rose, .. } => u64::from(rose),
            };
            (toggle_count.get(), count, accepted.get(), count)
        }
        StatefulStandardReaction::SampleHold {
            sample_count,
            reset_rose,
            ..
        } => (
            sample_count.get(),
            u64::from(reset_rose),
            0,
            sample_count.get() + u64::from(reset_rose),
        ),
    };
    for (key, cause) in &obs.public_causes {
        if let AnyModuleInputKey::Pulse(_) = key {
            let reset_input = *key == pulse_resettable_toggle_reset_key().into();
            let expected = if reset_input { reset_count } else { work };
            let actual = match obs.provenance.inspect(*cause).unwrap() {
                CauseInspection::ExternalPulseObservation { count, .. } => count.get(),
                CauseInspection::InitializationTransaction { .. }
                | CauseInspection::ReadyTransaction { .. } => 0,
                _ => panic!("direct fixture input must retain its external pulse observation"),
            };
            assert_eq!(actual, expected);
        }
    }
    for node in view.nodes() {
        match node.standard_role().unwrap() {
            "accepted_toggle" => match obs.provenance.inspect(node.cause().unwrap()).unwrap() {
                CauseInspection::PulseDerived { result, .. } => assert_eq!(result.get(), accepted),
                _ => panic!("gate provenance must retain accepted multiplicity"),
            },
            "reset_baseline" | "held_state" => {
                match obs.provenance.inspect(node.cause().unwrap()).unwrap() {
                    CauseInspection::PulseControlledLevel {
                        contributions,
                        result,
                        ..
                    } => {
                        assert_eq!(contributions.len(), 1);
                        assert_eq!(contributions[0].count().get(), capture_count);
                        assert_eq!(Some(result), node.level());
                        assert!(matches!(
                            contributions[0].port(),
                            PulsePortSubject::Qualified(_)
                        ));
                    }
                    _ => panic!("capture provenance must retain the complete capture count"),
                }
            }
            "capture_pulses" => match obs.provenance.inspect(node.cause().unwrap()).unwrap() {
                CauseInspection::PulseDerived {
                    contributions,
                    result,
                    ..
                } => {
                    assert_eq!(result.get(), capture_count);
                    let mut actual = contributions
                        .iter()
                        .map(|c| c.count().get())
                        .collect::<Vec<_>>();
                    actual.sort();
                    let mut expected = vec![work, reset_count];
                    expected.sort();
                    assert_eq!(actual, expected);
                }
                _ => panic!("merge provenance must retain both grouped contributors"),
            },
            _ => {}
        }
    }
}

#[test]
fn preserved_standard_history_resolves_through_a_quiet_patch() {
    for kind in [Kind::Pulse, Kind::Level, Kind::Hold] {
        let f = Fixture::new(kind, HIGH, LOW, false);
        let mut machine = f.compiled.spawn(policy());
        f.apply(&mut machine, 1, batch(HIGH, 2, 1)).unwrap();
        let old = machine.inspect_module(f.instance).unwrap();
        let source = f.compiled.graph().external_outputs()[0].source();
        let prepared = machine
            .prepare_patch(
                machine
                    .patch()
                    .add_external_output(mossignal::authored::ExternalOutputDef::new(
                        ExternalOutputKey::<Level>::from_u128(31).into(),
                        source,
                        DiagnosticMeta::default(),
                    ))
                    .unwrap()
                    .finish(),
            )
            .require_artifact()
            .unwrap();
        let delta = prepared
            .resulting_compiled()
            .input_delta()
            .finish()
            .unwrap();
        machine
            .apply(
                Transaction::advance(Time::from_ticks(2), machine.revision(), delta)
                    .with_patch(prepared, ReconfigurationPolicy::RejectStateLoss)
                    .unwrap(),
            )
            .unwrap();
        let current = machine.inspect_module(f.instance).unwrap();
        let standard = current.stateful_standard().unwrap();
        for cause in [
            standard.latest_reset_cause,
            standard.latest_accepted_toggle_cause,
            standard.latest_capture_cause,
        ]
        .into_iter()
        .flatten()
        {
            standard.provenance.inspect(cause).unwrap();
            current.provenance().explain_cause(cause).unwrap();
        }
        machine.explain(Explain::CurrentModule(f.instance)).unwrap();
        drop(machine);
        let standard = old.stateful_standard().unwrap();
        for (_, cause) in &standard.internal_causes {
            standard.provenance.explain_cause(*cause).unwrap();
        }
    }
}
