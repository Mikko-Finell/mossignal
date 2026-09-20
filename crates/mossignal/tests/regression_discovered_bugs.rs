//! Regression tests for production defects found by tracing public contracts
//! through compilation, evaluation, diagnostics, and provenance.
//!
//! These tests are ignored so the ordinary development gate stays green. Run
//! them with `cargo test -p mossignal --test regression_discovered_bugs -- --ignored`.

use std::collections::BTreeSet;

use mossignal::diagnostics::SubjectRef;
use mossignal::key::{ModuleInputKey, ModuleInstanceKey, ModuleOutputKey, NodeKey};
use mossignal::metadata::DiagnosticMeta;
use mossignal::signal::{Level, LogicLevel, Pulse, PulseCount};
use mossignal::time::Time;
use mossignal::{
    CauseInspection, CauseRef, ConflictPolicy, LevelSetResetConfig, ModuleBuilder, NetworkBuilder,
    OutputEvent, ProvenanceView, PulseSetResetConfig, RuntimeFailureEvidence, RuntimePolicy,
    TimeDomainId, Transaction,
};

#[derive(Debug, PartialEq, Eq)]
enum TestDomain {}

fn policy() -> RuntimePolicy {
    RuntimePolicy::builder()
        .max_internal_reactions(1_000)
        .max_evaluated_operations(100_000)
        .max_pending_events(1_000)
        .max_events_created_per_transaction(10_000)
        .max_required_provenance_growth(100_000)
        .build()
        .unwrap_or_else(|failure| panic!("complete policy must build: {failure}"))
}

fn observed_level_inputs<D>(provenance: &ProvenanceView<D>, root: CauseRef) -> BTreeSet<u128> {
    let mut found = BTreeSet::new();
    collect_observed_level_inputs(provenance, root, &mut found, &mut BTreeSet::new());
    found
}

fn collect_observed_level_inputs<D>(
    provenance: &ProvenanceView<D>,
    cause: CauseRef,
    found: &mut BTreeSet<u128>,
    visited: &mut BTreeSet<CauseRef>,
) {
    if !visited.insert(cause) {
        return;
    }
    match provenance
        .inspect(cause)
        .unwrap_or_else(|failure| panic!("cause must resolve inside its view: {failure}"))
    {
        CauseInspection::ExternalObservation { input, .. } => {
            found.insert(input.as_u128());
        }
        CauseInspection::Derived { supporters, .. }
        | CauseInspection::PulseDerived { supporters, .. }
        | CauseInspection::PulseControlledLevel { supporters, .. }
        | CauseInspection::PendingPulseDelay { supporters, .. } => {
            for supporter in supporters {
                collect_observed_level_inputs(provenance, *supporter, found, visited);
            }
        }
        _ => {}
    }
}

fn observed_pulse_inputs<D>(
    provenance: &ProvenanceView<D>,
    root: CauseRef,
) -> BTreeSet<(u128, u64)> {
    let mut found = BTreeSet::new();
    collect_observed_pulse_inputs(provenance, root, &mut found, &mut BTreeSet::new());
    found
}

fn collect_observed_pulse_inputs<D>(
    provenance: &ProvenanceView<D>,
    cause: CauseRef,
    found: &mut BTreeSet<(u128, u64)>,
    visited: &mut BTreeSet<CauseRef>,
) {
    if !visited.insert(cause) {
        return;
    }
    match provenance
        .inspect(cause)
        .unwrap_or_else(|failure| panic!("cause must resolve inside its view: {failure}"))
    {
        CauseInspection::ExternalPulseObservation { input, count } => {
            found.insert((input.as_u128(), count.get()));
        }
        CauseInspection::Derived { supporters, .. }
        | CauseInspection::PendingPulseDelay { supporters, .. } => {
            for supporter in supporters {
                collect_observed_pulse_inputs(provenance, *supporter, found, visited);
            }
        }
        CauseInspection::PulseDerived {
            contributions,
            supporters,
            ..
        }
        | CauseInspection::PulseControlledLevel {
            contributions,
            supporters,
            ..
        } => {
            for contribution in contributions {
                collect_observed_pulse_inputs(provenance, contribution.cause(), found, visited);
            }
            for supporter in supporters {
                collect_observed_pulse_inputs(provenance, *supporter, found, visited);
            }
        }
        _ => {}
    }
}

#[test]
#[ignore = "bug: module-internal episode primary is a private flattened NodeKey"]
fn module_level_latch_episode_primary_is_the_authored_module_local_node() {
    let module_set = ModuleInputKey::<Level>::from_u128(1);
    let module_reset = ModuleInputKey::<Level>::from_u128(2);
    let module_output = ModuleOutputKey::<Level>::from_u128(3);
    let latch = NodeKey::from_u128(10);
    let mut module = ModuleBuilder::<TestDomain>::new();
    let set = module
        .add_level_input(module_set, DiagnosticMeta::default())
        .unwrap_or_else(|failure| panic!("module set must author: {failure:?}"));
    let reset = module
        .add_level_input(module_reset, DiagnosticMeta::default())
        .unwrap_or_else(|failure| panic!("module reset must author: {failure:?}"));
    let state = module
        .add_level_set_reset_latch(
            latch,
            set,
            reset,
            LevelSetResetConfig::new(LogicLevel::Low, ConflictPolicy::RetainAndDiagnose),
            DiagnosticMeta::default(),
        )
        .unwrap_or_else(|failure| panic!("module latch must author: {failure:?}"))
        .into_outputs();
    module
        .add_level_output(module_output, state, DiagnosticMeta::default())
        .unwrap_or_else(|failure| panic!("module output must author: {failure:?}"));
    let module = module
        .finish()
        .require_artifact()
        .unwrap_or_else(|failure| panic!("module must validate: {failure:?}"));

    let instance = ModuleInstanceKey::from_u128(100);
    let mut network = NetworkBuilder::<TestDomain>::new(TimeDomainId::from_u128(60));
    let (set_key, set) = network.level_input("set");
    let (reset_key, reset) = network.level_input("reset");
    let added = network
        .instantiate(&module, instance, DiagnosticMeta::default())
        .unwrap_or_else(|failure| panic!("instance must begin: {failure:?}"))
        .bind_level(module_set, set)
        .and_then(|builder| builder.bind_level(module_reset, reset))
        .and_then(|builder| builder.finish())
        .unwrap_or_else(|failure| panic!("instance must bind: {failure:?}"));
    network
        .level_output(
            "state",
            added
                .level_output(module_output)
                .unwrap_or_else(|failure| panic!("module output must exist: {failure:?}")),
        )
        .unwrap_or_else(|failure| panic!("network output must author: {failure:?}"));
    let compiled = network
        .finish()
        .require_artifact()
        .unwrap_or_else(|failure| panic!("network must validate: {failure:?}"))
        .compile()
        .require_artifact()
        .unwrap_or_else(|failure| panic!("network must compile: {failure:?}"));
    let snapshot = compiled
        .input_snapshot()
        .set(set_key, LogicLevel::High)
        .and_then(|builder| builder.set(reset_key, LogicLevel::High))
        .and_then(mossignal::InputSnapshotBuilder::finish)
        .unwrap_or_else(|failure| panic!("snapshot must build: {failure}"));
    let mut machine = compiled.spawn(policy());
    machine
        .apply(Transaction::initialize(
            Time::from_ticks(0),
            machine.revision(),
            snapshot,
        ))
        .unwrap_or_else(|failure| panic!("module network must initialize: {failure}"));
    let episodes = machine
        .active_diagnostic_episodes()
        .unwrap_or_else(|failure| panic!("ready machine must expose episodes: {failure:?}"));
    let [episode] = episodes.as_slice() else {
        panic!("one module-local retained conflict must create one episode");
    };

    assert_eq!(
        episode.current().primary(),
        &SubjectRef::Node(latch),
        "episode identity must name the authored module-local node, not a private flattening key"
    );
    let SubjectRef::Node(primary) = episode.current().primary() else {
        panic!("episode primary must be a node");
    };
    assert!(
        machine.inspect_level_set_reset_latch(*primary).is_ok(),
        "the episode primary must identify an inspectable latch, got {primary:?}: {:?}",
        machine.inspect_level_set_reset_latch(*primary)
    );
}

#[test]
#[ignore = "bug: module-internal reject failure primary is a private flattened NodeKey"]
fn module_pulse_latch_reject_failure_primary_is_the_authored_module_local_node() {
    let module_set = ModuleInputKey::<Pulse>::from_u128(1);
    let module_reset = ModuleInputKey::<Pulse>::from_u128(2);
    let module_output = ModuleOutputKey::<Level>::from_u128(3);
    let latch = NodeKey::from_u128(10);
    let mut module = ModuleBuilder::<TestDomain>::new();
    let set = module
        .add_pulse_input(module_set, DiagnosticMeta::default())
        .unwrap_or_else(|failure| panic!("module set must author: {failure:?}"));
    let reset = module
        .add_pulse_input(module_reset, DiagnosticMeta::default())
        .unwrap_or_else(|failure| panic!("module reset must author: {failure:?}"));
    let state = module
        .add_pulse_set_reset_latch(
            latch,
            set,
            reset,
            PulseSetResetConfig::new(LogicLevel::Low, ConflictPolicy::RejectTransaction),
            DiagnosticMeta::default(),
        )
        .unwrap_or_else(|failure| panic!("module latch must author: {failure:?}"))
        .into_outputs();
    module
        .add_level_output(module_output, state, DiagnosticMeta::default())
        .unwrap_or_else(|failure| panic!("module output must author: {failure:?}"));
    let module = module
        .finish()
        .require_artifact()
        .unwrap_or_else(|failure| panic!("module must validate: {failure:?}"));

    let instance = ModuleInstanceKey::from_u128(100);
    let mut network = NetworkBuilder::<TestDomain>::new(TimeDomainId::from_u128(60));
    let (set_key, set) = network.pulse_input("set");
    let (reset_key, reset) = network.pulse_input("reset");
    let added = network
        .instantiate(&module, instance, DiagnosticMeta::default())
        .unwrap_or_else(|failure| panic!("instance must begin: {failure:?}"))
        .bind_pulse(module_set, set)
        .and_then(|builder| builder.bind_pulse(module_reset, reset))
        .and_then(|builder| builder.finish())
        .unwrap_or_else(|failure| panic!("instance must bind: {failure:?}"));
    network
        .level_output(
            "state",
            added
                .level_output(module_output)
                .unwrap_or_else(|failure| panic!("module output must exist: {failure:?}")),
        )
        .unwrap_or_else(|failure| panic!("network output must author: {failure:?}"));
    let compiled = network
        .finish()
        .require_artifact()
        .unwrap_or_else(|failure| panic!("network must validate: {failure:?}"))
        .compile()
        .require_artifact()
        .unwrap_or_else(|failure| panic!("network must compile: {failure:?}"));
    let snapshot = compiled
        .input_snapshot()
        .pulse(set_key, PulseCount::ONE)
        .and_then(|builder| builder.pulse(reset_key, PulseCount::ONE))
        .and_then(mossignal::InputSnapshotBuilder::finish)
        .unwrap_or_else(|failure| panic!("snapshot must build: {failure}"));
    let mut machine = compiled.spawn(policy());
    let failure = machine
        .apply(Transaction::initialize(
            Time::from_ticks(0),
            machine.revision(),
            snapshot,
        ))
        .expect_err("simultaneous set and reset under RejectTransaction must fail");
    match failure.evidence() {
        RuntimeFailureEvidence::PulseLatchConflict { primary, node, .. } => {
            assert_eq!(
                *primary, latch,
                "rejected-transaction evidence must name the authored module-local node"
            );
            assert_eq!(
                failure.problem().primary(),
                &SubjectRef::Node(latch),
                "the projected problem primary must be the authored module-local node"
            );
            match node {
                mossignal::NodeSubject::Qualified(qualified) => {
                    assert_eq!(qualified.instances(), &[instance]);
                    assert_eq!(qualified.node(), latch);
                }
                other => panic!("reject evidence must retain the qualified owner, got {other:?}"),
            }
        }
        other => panic!("unexpected reject evidence: {other:?}"),
    }
}

#[test]
#[ignore = "bug: Select provenance treats the unselected branch as current causal support"]
fn select_provenance_excludes_the_unselected_branch() {
    let mut builder = NetworkBuilder::<TestDomain>::new(TimeDomainId::from_u128(7));
    let (selector_key, selector) = builder.level_input("selector");
    let (when_low_key, when_low) = builder.level_input("when_low");
    let (when_high_key, when_high) = builder.level_input("when_high");
    let selected = builder
        .select(selector, when_low, when_high)
        .unwrap_or_else(|failure| panic!("select must author: {failure:?}"));
    let output = builder
        .level_output("out", selected)
        .unwrap_or_else(|failure| panic!("output must author: {failure:?}"));
    let compiled = builder
        .finish()
        .require_artifact()
        .unwrap_or_else(|failure| panic!("select must validate: {failure:?}"))
        .compile()
        .require_artifact()
        .unwrap_or_else(|failure| panic!("select must compile: {failure:?}"));

    let snapshot = compiled
        .input_snapshot()
        .set(selector_key, LogicLevel::High)
        .and_then(|builder| builder.set(when_low_key, LogicLevel::Low))
        .and_then(|builder| builder.set(when_high_key, LogicLevel::High))
        .and_then(mossignal::InputSnapshotBuilder::finish)
        .unwrap_or_else(|failure| panic!("snapshot must build: {failure}"));
    let mut machine = compiled.spawn(policy());
    let result = machine
        .apply(Transaction::initialize(
            Time::from_ticks(0),
            machine.revision(),
            snapshot,
        ))
        .unwrap_or_else(|failure| panic!("select must initialize: {failure}"));
    let [
        OutputEvent::LevelEstablished {
            output: established,
            value,
            cause,
            ..
        },
    ] = result.output_events()
    else {
        panic!("initialization must establish the selected High branch");
    };
    assert_eq!(*established, output);
    assert_eq!(*value, LogicLevel::High);

    let observed = observed_level_inputs(result.provenance(), *cause);
    assert!(
        observed.contains(&selector_key.as_u128()),
        "selector must remain in current causal support: {observed:?}"
    );
    assert!(
        observed.contains(&when_high_key.as_u128()),
        "selected when-high input must remain in current causal support: {observed:?}"
    );
    assert!(
        !observed.contains(&when_low_key.as_u128()),
        "unselected when-low input must be absent from current causal support, found {observed:?}"
    );
}

#[test]
#[ignore = "bug: All Low provenance treats High inputs as current supporters"]
fn all_low_provenance_excludes_high_inputs() {
    let mut builder = NetworkBuilder::<TestDomain>::new(TimeDomainId::from_u128(8));
    let (high_key, high) = builder.level_input("high");
    let (low_key, low) = builder.level_input("low");
    let conjunction = builder
        .all([high, low])
        .unwrap_or_else(|failure| panic!("all must author: {failure:?}"));
    let output = builder
        .level_output("out", conjunction)
        .unwrap_or_else(|failure| panic!("output must author: {failure:?}"));
    let compiled = builder
        .finish()
        .require_artifact()
        .unwrap_or_else(|failure| panic!("all must validate: {failure:?}"))
        .compile()
        .require_artifact()
        .unwrap_or_else(|failure| panic!("all must compile: {failure:?}"));

    let snapshot = compiled
        .input_snapshot()
        .set(high_key, LogicLevel::High)
        .and_then(|builder| builder.set(low_key, LogicLevel::Low))
        .and_then(mossignal::InputSnapshotBuilder::finish)
        .unwrap_or_else(|failure| panic!("snapshot must build: {failure}"));
    let mut machine = compiled.spawn(policy());
    let result = machine
        .apply(Transaction::initialize(
            Time::from_ticks(0),
            machine.revision(),
            snapshot,
        ))
        .unwrap_or_else(|failure| panic!("all must initialize: {failure}"));
    let [
        OutputEvent::LevelEstablished {
            output: established,
            value,
            cause,
            ..
        },
    ] = result.output_events()
    else {
        panic!("initialization must establish All as Low");
    };
    assert_eq!(*established, output);
    assert_eq!(*value, LogicLevel::Low);

    let observed = observed_level_inputs(result.provenance(), *cause);
    assert!(
        observed.contains(&low_key.as_u128()),
        "each Low input must support a Low All result: {observed:?}"
    );
    assert!(
        !observed.contains(&high_key.as_u128()),
        "High inputs are not current supporters of a Low All result, found {observed:?}"
    );
}

#[test]
#[ignore = "bug: Any High provenance treats Low inputs as current supporters"]
fn any_high_provenance_excludes_low_inputs() {
    let mut builder = NetworkBuilder::<TestDomain>::new(TimeDomainId::from_u128(9));
    let (high_key, high) = builder.level_input("high");
    let (low_key, low) = builder.level_input("low");
    let disjunction = builder
        .any([high, low])
        .unwrap_or_else(|failure| panic!("any must author: {failure:?}"));
    let output = builder
        .level_output("out", disjunction)
        .unwrap_or_else(|failure| panic!("output must author: {failure:?}"));
    let compiled = builder
        .finish()
        .require_artifact()
        .unwrap_or_else(|failure| panic!("any must validate: {failure:?}"))
        .compile()
        .require_artifact()
        .unwrap_or_else(|failure| panic!("any must compile: {failure:?}"));

    let snapshot = compiled
        .input_snapshot()
        .set(high_key, LogicLevel::High)
        .and_then(|builder| builder.set(low_key, LogicLevel::Low))
        .and_then(mossignal::InputSnapshotBuilder::finish)
        .unwrap_or_else(|failure| panic!("snapshot must build: {failure}"));
    let mut machine = compiled.spawn(policy());
    let result = machine
        .apply(Transaction::initialize(
            Time::from_ticks(0),
            machine.revision(),
            snapshot,
        ))
        .unwrap_or_else(|failure| panic!("any must initialize: {failure}"));
    let [
        OutputEvent::LevelEstablished {
            output: established,
            value,
            cause,
            ..
        },
    ] = result.output_events()
    else {
        panic!("initialization must establish Any as High");
    };
    assert_eq!(*established, output);
    assert_eq!(*value, LogicLevel::High);

    let observed = observed_level_inputs(result.provenance(), *cause);
    assert!(
        observed.contains(&high_key.as_u128()),
        "each High input must support a High Any result: {observed:?}"
    );
    assert!(
        !observed.contains(&low_key.as_u128()),
        "Low inputs are not current supporters of a High Any result, found {observed:?}"
    );
}

#[test]
#[ignore = "bug: AtLeast(0) provenance retains inputs that cannot affect the constant High result"]
fn at_least_zero_constant_high_provenance_excludes_inputs() {
    let mut builder = NetworkBuilder::<TestDomain>::new(TimeDomainId::from_u128(10));
    let (left_key, left) = builder.level_input("left");
    let (right_key, right) = builder.level_input("right");
    let constant_high = builder
        .at_least(0, [left, right])
        .unwrap_or_else(|failure| panic!("AtLeast(0) must author: {failure:?}"));
    let output = builder
        .level_output("out", constant_high)
        .unwrap_or_else(|failure| panic!("output must author: {failure:?}"));
    let compiled = builder
        .finish()
        .require_artifact()
        .unwrap_or_else(|failure| panic!("AtLeast(0) must validate: {failure:?}"))
        .compile()
        .require_artifact()
        .unwrap_or_else(|failure| panic!("AtLeast(0) must compile: {failure:?}"));
    let snapshot = compiled
        .input_snapshot()
        .set(left_key, LogicLevel::High)
        .and_then(|builder| builder.set(right_key, LogicLevel::Low))
        .and_then(mossignal::InputSnapshotBuilder::finish)
        .unwrap_or_else(|failure| panic!("snapshot must build: {failure}"));
    let mut machine = compiled.spawn(policy());
    let result = machine
        .apply(Transaction::initialize(
            Time::from_ticks(0),
            machine.revision(),
            snapshot,
        ))
        .unwrap_or_else(|failure| panic!("AtLeast(0) must initialize: {failure}"));
    let [
        OutputEvent::LevelEstablished {
            output: established,
            value,
            cause,
            ..
        },
    ] = result.output_events()
    else {
        panic!("initialization must establish constant High");
    };
    assert_eq!(*established, output);
    assert_eq!(*value, LogicLevel::High);

    let observed = observed_level_inputs(result.provenance(), *cause);
    assert!(
        observed.is_empty(),
        "AtLeast(0) is independent of its inputs, but provenance retained {observed:?}"
    );
}

#[test]
#[ignore = "bug: module fingerprint panics on connections sourced from nested module outputs"]
fn nested_module_output_feeding_an_internal_node_must_fingerprint() {
    let mut inner = ModuleBuilder::<TestDomain>::new();
    let inner_in = ModuleInputKey::<Level>::from_u128(1);
    let inner_out = ModuleOutputKey::<Level>::from_u128(2);
    let source = inner
        .add_level_input(inner_in, DiagnosticMeta::default())
        .unwrap_or_else(|failure| panic!("inner input must author: {failure:?}"));
    inner
        .add_level_output(inner_out, source, DiagnosticMeta::default())
        .unwrap_or_else(|failure| panic!("inner output must author: {failure:?}"));
    let inner = inner
        .finish()
        .require_artifact()
        .unwrap_or_else(|failure| panic!("passthrough module must validate: {failure:?}"));

    let nested = ModuleInstanceKey::from_u128(10);
    let mut outer = ModuleBuilder::<TestDomain>::new();
    let (_, outer_source) = outer.level_input("in");
    let added = outer
        .instantiate(&inner, nested, DiagnosticMeta::default())
        .unwrap_or_else(|failure| panic!("nested instance must begin: {failure:?}"))
        .bind_level(inner_in, outer_source)
        .and_then(|builder| builder.finish())
        .unwrap_or_else(|failure| panic!("nested instance must bind: {failure:?}"));
    let nested_out = added
        .level_output(inner_out)
        .unwrap_or_else(|failure| panic!("nested output must exist: {failure:?}"));
    let inverted = outer
        .not(nested_out)
        .unwrap_or_else(|failure| panic!("internal Not of nested output must author: {failure:?}"));
    outer
        .level_output("out", inverted)
        .unwrap_or_else(|failure| panic!("outer output must author: {failure:?}"));
    let report = outer.finish();
    assert!(
        report.artifact().is_some(),
        "a nested module output feeding an internal node must validate and fingerprint; diagnostics: {:?}",
        report
            .diagnostics()
            .iter()
            .map(|diagnostic| diagnostic.problem().code())
            .collect::<Vec<_>>()
    );
}

#[test]
#[ignore = "bug: PulseRoute zero-branch provenance retains the batch routed to the other output"]
fn pulse_route_zero_branch_must_not_import_the_routed_batch_into_downstream_support() {
    let mut builder = NetworkBuilder::<TestDomain>::new(TimeDomainId::from_u128(21));
    let (selector_key, selector) = builder.level_input("selector");
    let (pulses_key, pulses) = builder.pulse_input("pulses");
    let (extra_key, extra) = builder.pulse_input("extra");
    let routed = builder
        .pulse_route(selector, pulses)
        .unwrap_or_else(|failure| panic!("PulseRoute must author: {failure:?}"));
    let merged = builder
        .merge([routed.when_low, extra])
        .unwrap_or_else(|failure| panic!("merge must author: {failure:?}"));
    let merge_out = builder
        .pulse_output("merged", merged)
        .unwrap_or_else(|failure| panic!("merge output must author: {failure:?}"));
    builder
        .pulse_output("routed-high", routed.when_high)
        .unwrap_or_else(|failure| panic!("routed-high output must author: {failure:?}"));
    let compiled = builder
        .finish()
        .require_artifact()
        .unwrap_or_else(|failure| panic!("route network must validate: {failure:?}"))
        .compile()
        .require_artifact()
        .unwrap_or_else(|failure| panic!("route network must compile: {failure:?}"));
    let snapshot = compiled
        .input_snapshot()
        .set(selector_key, LogicLevel::High)
        .and_then(|builder| builder.pulse(pulses_key, PulseCount::new(7)))
        .and_then(|builder| builder.pulse(extra_key, PulseCount::ONE))
        .and_then(mossignal::InputSnapshotBuilder::finish)
        .unwrap_or_else(|failure| panic!("snapshot must build: {failure}"));
    let mut machine = compiled.spawn(policy());
    let result = machine
        .apply(Transaction::initialize(
            Time::from_ticks(0),
            machine.revision(),
            snapshot,
        ))
        .unwrap_or_else(|failure| panic!("route network must initialize: {failure}"));
    let merge_event = result
        .output_events()
        .iter()
        .find_map(|event| match event {
            OutputEvent::Pulsed {
                output,
                count,
                cause,
                ..
            } if *output == merge_out => Some((*count, *cause)),
            _ => None,
        })
        .unwrap_or_else(|| panic!("merge output must emit the extra pulse"));
    assert_eq!(merge_event.0, PulseCount::ONE);
    let observed = observed_pulse_inputs(result.provenance(), merge_event.1);
    assert!(
        observed.contains(&(extra_key.as_u128(), 1)),
        "the extra pulse must support the merge: {observed:?}"
    );
    assert!(
        !observed.contains(&(pulses_key.as_u128(), 7)),
        "pulses routed to when-high must not support the zero when-low merge, found {observed:?}"
    );
}

#[test]
#[ignore = "bug: PulseGate-suppressed pulses leak into downstream merge provenance"]
fn pulse_gate_suppressed_pulses_must_not_support_downstream_merge() {
    let mut builder = NetworkBuilder::<TestDomain>::new(TimeDomainId::from_u128(22));
    let (enable_key, enable) = builder.level_input("enable");
    let (gated_key, gated) = builder.pulse_input("gated");
    let (extra_key, extra) = builder.pulse_input("extra");
    let closed = builder
        .pulse_gate(gated, enable)
        .unwrap_or_else(|failure| panic!("PulseGate must author: {failure:?}"));
    let merged = builder
        .merge([closed, extra])
        .unwrap_or_else(|failure| panic!("merge must author: {failure:?}"));
    let merge_out = builder
        .pulse_output("merged", merged)
        .unwrap_or_else(|failure| panic!("merge output must author: {failure:?}"));
    let compiled = builder
        .finish()
        .require_artifact()
        .unwrap_or_else(|failure| panic!("gate network must validate: {failure:?}"))
        .compile()
        .require_artifact()
        .unwrap_or_else(|failure| panic!("gate network must compile: {failure:?}"));
    let snapshot = compiled
        .input_snapshot()
        .set(enable_key, LogicLevel::Low)
        .and_then(|builder| builder.pulse(gated_key, PulseCount::new(7)))
        .and_then(|builder| builder.pulse(extra_key, PulseCount::ONE))
        .and_then(mossignal::InputSnapshotBuilder::finish)
        .unwrap_or_else(|failure| panic!("snapshot must build: {failure}"));
    let mut machine = compiled.spawn(policy());
    let result = machine
        .apply(Transaction::initialize(
            Time::from_ticks(0),
            machine.revision(),
            snapshot,
        ))
        .unwrap_or_else(|failure| panic!("gate network must initialize: {failure}"));
    let [
        OutputEvent::Pulsed {
            output,
            count,
            cause,
            ..
        },
    ] = result.output_events()
    else {
        panic!("merge must emit exactly the extra pulse");
    };
    assert_eq!(*output, merge_out);
    assert_eq!(*count, PulseCount::ONE);
    let observed = observed_pulse_inputs(result.provenance(), *cause);
    assert!(
        observed.contains(&(extra_key.as_u128(), 1)),
        "the extra pulse must support the merge: {observed:?}"
    );
    assert!(
        !observed.contains(&(gated_key.as_u128(), 7)),
        "PulseGate-suppressed pulses must not support downstream merge, found {observed:?}"
    );
}

#[test]
#[ignore = "bug: Zip unmatched pulses leak into downstream merge provenance"]
fn zip_unmatched_pulses_must_not_support_a_zero_result_downstream_merge() {
    let mut builder = NetworkBuilder::<TestDomain>::new(TimeDomainId::from_u128(23));
    let (left_key, left) = builder.pulse_input("left");
    let (right_key, right) = builder.pulse_input("right");
    let (extra_key, extra) = builder.pulse_input("extra");
    let zipped = builder
        .zip([left, right])
        .unwrap_or_else(|failure| panic!("Zip must author: {failure:?}"));
    let merged = builder
        .merge([zipped, extra])
        .unwrap_or_else(|failure| panic!("merge must author: {failure:?}"));
    let merge_out = builder
        .pulse_output("merged", merged)
        .unwrap_or_else(|failure| panic!("merge output must author: {failure:?}"));
    let compiled = builder
        .finish()
        .require_artifact()
        .unwrap_or_else(|failure| panic!("zip network must validate: {failure:?}"))
        .compile()
        .require_artifact()
        .unwrap_or_else(|failure| panic!("zip network must compile: {failure:?}"));
    let snapshot = compiled
        .input_snapshot()
        .pulse(left_key, PulseCount::new(7))
        .and_then(|builder| builder.pulse(right_key, PulseCount::ZERO))
        .and_then(|builder| builder.pulse(extra_key, PulseCount::ONE))
        .and_then(mossignal::InputSnapshotBuilder::finish)
        .unwrap_or_else(|failure| panic!("snapshot must build: {failure}"));
    let mut machine = compiled.spawn(policy());
    let result = machine
        .apply(Transaction::initialize(
            Time::from_ticks(0),
            machine.revision(),
            snapshot,
        ))
        .unwrap_or_else(|failure| panic!("zip network must initialize: {failure}"));
    let merge_event = result
        .output_events()
        .iter()
        .find_map(|event| match event {
            OutputEvent::Pulsed {
                output,
                count,
                cause,
                ..
            } if *output == merge_out => Some((*count, *cause)),
            _ => None,
        })
        .unwrap_or_else(|| panic!("merge output must emit the extra pulse"));
    assert_eq!(merge_event.0, PulseCount::ONE);
    let observed = observed_pulse_inputs(result.provenance(), merge_event.1);
    assert!(
        observed.contains(&(extra_key.as_u128(), 1)),
        "the extra pulse must support the merge: {observed:?}"
    );
    assert!(
        !observed.contains(&(left_key.as_u128(), 7)),
        "unmatched Zip pulses must not support a zero-group downstream merge, found {observed:?}"
    );
}

#[test]
#[ignore = "bug: AtLeast(threshold > arity) provenance retains inputs that cannot affect the constant Low result"]
fn at_least_above_arity_constant_low_provenance_excludes_inputs() {
    let mut builder = NetworkBuilder::<TestDomain>::new(TimeDomainId::from_u128(25));
    let (left_key, left) = builder.level_input("left");
    let (right_key, right) = builder.level_input("right");
    let constant_low = builder
        .at_least(3, [left, right])
        .unwrap_or_else(|failure| panic!("AtLeast(3) of two inputs must author: {failure:?}"));
    let output = builder
        .level_output("out", constant_low)
        .unwrap_or_else(|failure| panic!("output must author: {failure:?}"));
    let compiled = builder
        .finish()
        .require_artifact()
        .unwrap_or_else(|failure| panic!("AtLeast(3) must validate: {failure:?}"))
        .compile()
        .require_artifact()
        .unwrap_or_else(|failure| panic!("AtLeast(3) must compile: {failure:?}"));
    let snapshot = compiled
        .input_snapshot()
        .set(left_key, LogicLevel::High)
        .and_then(|builder| builder.set(right_key, LogicLevel::High))
        .and_then(mossignal::InputSnapshotBuilder::finish)
        .unwrap_or_else(|failure| panic!("snapshot must build: {failure}"));
    let mut machine = compiled.spawn(policy());
    let result = machine
        .apply(Transaction::initialize(
            Time::from_ticks(0),
            machine.revision(),
            snapshot,
        ))
        .unwrap_or_else(|failure| panic!("AtLeast(3) must initialize: {failure}"));
    let [
        OutputEvent::LevelEstablished {
            output: established,
            value,
            cause,
            ..
        },
    ] = result.output_events()
    else {
        panic!("initialization must establish constant Low");
    };
    assert_eq!(*established, output);
    assert_eq!(*value, LogicLevel::Low);
    let observed = observed_level_inputs(result.provenance(), *cause);
    assert!(
        observed.is_empty(),
        "AtLeast(3) of two inputs is independent of those inputs, but provenance retained {observed:?}"
    );
}

#[test]
#[ignore = "bug: AtLeast High provenance treats Low inputs as current supporters after the threshold is met"]
fn at_least_high_provenance_excludes_low_inputs_once_the_threshold_is_met() {
    let mut builder = NetworkBuilder::<TestDomain>::new(TimeDomainId::from_u128(27));
    let (high_a_key, high_a) = builder.level_input("high-a");
    let (high_b_key, high_b) = builder.level_input("high-b");
    let (low_key, low) = builder.level_input("low");
    let threshold = builder
        .at_least(2, [high_a, high_b, low])
        .unwrap_or_else(|failure| panic!("AtLeast(2) must author: {failure:?}"));
    let output = builder
        .level_output("out", threshold)
        .unwrap_or_else(|failure| panic!("output must author: {failure:?}"));
    let compiled = builder
        .finish()
        .require_artifact()
        .unwrap_or_else(|failure| panic!("AtLeast(2) must validate: {failure:?}"))
        .compile()
        .require_artifact()
        .unwrap_or_else(|failure| panic!("AtLeast(2) must compile: {failure:?}"));
    let snapshot = compiled
        .input_snapshot()
        .set(high_a_key, LogicLevel::High)
        .and_then(|builder| builder.set(high_b_key, LogicLevel::High))
        .and_then(|builder| builder.set(low_key, LogicLevel::Low))
        .and_then(mossignal::InputSnapshotBuilder::finish)
        .unwrap_or_else(|failure| panic!("snapshot must build: {failure}"));
    let mut machine = compiled.spawn(policy());
    let result = machine
        .apply(Transaction::initialize(
            Time::from_ticks(0),
            machine.revision(),
            snapshot,
        ))
        .unwrap_or_else(|failure| panic!("AtLeast(2) must initialize: {failure}"));
    let [
        OutputEvent::LevelEstablished {
            output: established,
            value,
            cause,
            ..
        },
    ] = result.output_events()
    else {
        panic!("initialization must establish High");
    };
    assert_eq!(*established, output);
    assert_eq!(*value, LogicLevel::High);
    let observed = observed_level_inputs(result.provenance(), *cause);
    assert!(
        observed.contains(&high_a_key.as_u128()) && observed.contains(&high_b_key.as_u128()),
        "High inputs that meet the threshold must support the result: {observed:?}"
    );
    assert!(
        !observed.contains(&low_key.as_u128()),
        "Low inputs are not current supporters of an already-met AtLeast High result, found {observed:?}"
    );
}

#[test]
#[ignore = "bug: xor(x, x) is constantly Low but provenance still treats x as current support"]
fn xor_of_a_duplicated_source_is_constant_low_and_independent_of_that_source() {
    let mut builder = NetworkBuilder::<TestDomain>::new(TimeDomainId::from_u128(30));
    let (input_key, input) = builder.level_input("x");
    let xor = builder
        .xor(input, input)
        .unwrap_or_else(|failure| panic!("xor must author: {failure:?}"));
    let output = builder
        .level_output("out", xor)
        .unwrap_or_else(|failure| panic!("output must author: {failure:?}"));
    let compiled = builder
        .finish()
        .require_artifact()
        .unwrap_or_else(|failure| panic!("xor(x, x) must validate: {failure:?}"))
        .compile()
        .require_artifact()
        .unwrap_or_else(|failure| panic!("xor(x, x) must compile: {failure:?}"));
    for value in [LogicLevel::Low, LogicLevel::High] {
        let snapshot = compiled
            .input_snapshot()
            .set(input_key, value)
            .and_then(mossignal::InputSnapshotBuilder::finish)
            .unwrap_or_else(|failure| panic!("snapshot must build: {failure}"));
        let mut machine = compiled.spawn(policy());
        let result = machine
            .apply(Transaction::initialize(
                Time::from_ticks(0),
                machine.revision(),
                snapshot,
            ))
            .unwrap_or_else(|failure| panic!("xor(x, x) must initialize: {failure}"));
        let [
            OutputEvent::LevelEstablished {
                output: established,
                value: established_value,
                cause,
                ..
            },
        ] = result.output_events()
        else {
            panic!("initialization must establish constant Low");
        };
        assert_eq!(*established, output);
        assert_eq!(*established_value, LogicLevel::Low);
        let observed = observed_level_inputs(result.provenance(), *cause);
        assert!(
            observed.is_empty(),
            "xor(x, x) cannot change with x, but provenance retained {observed:?} for {value:?}"
        );
    }
}

#[test]
#[ignore = "bug: AllEqual(x, x) is constantly High but provenance still treats x as current support"]
fn all_equal_of_a_duplicated_source_is_constant_high_and_independent_of_that_source() {
    let mut builder = NetworkBuilder::<TestDomain>::new(TimeDomainId::from_u128(32));
    let (input_key, input) = builder.level_input("x");
    let equal = builder
        .all_equal([input, input])
        .unwrap_or_else(|failure| panic!("AllEqual must author: {failure:?}"));
    let output = builder
        .level_output("out", equal)
        .unwrap_or_else(|failure| panic!("output must author: {failure:?}"));
    let compiled = builder
        .finish()
        .require_artifact()
        .unwrap_or_else(|failure| panic!("AllEqual(x, x) must validate: {failure:?}"))
        .compile()
        .require_artifact()
        .unwrap_or_else(|failure| panic!("AllEqual(x, x) must compile: {failure:?}"));
    for value in [LogicLevel::Low, LogicLevel::High] {
        let snapshot = compiled
            .input_snapshot()
            .set(input_key, value)
            .and_then(mossignal::InputSnapshotBuilder::finish)
            .unwrap_or_else(|failure| panic!("snapshot must build: {failure}"));
        let mut machine = compiled.spawn(policy());
        let result = machine
            .apply(Transaction::initialize(
                Time::from_ticks(0),
                machine.revision(),
                snapshot,
            ))
            .unwrap_or_else(|failure| panic!("AllEqual(x, x) must initialize: {failure}"));
        let [
            OutputEvent::LevelEstablished {
                output: established,
                value: established_value,
                cause,
                ..
            },
        ] = result.output_events()
        else {
            panic!("initialization must establish constant High");
        };
        assert_eq!(*established, output);
        assert_eq!(*established_value, LogicLevel::High);
        let observed = observed_level_inputs(result.provenance(), *cause);
        assert!(
            observed.is_empty(),
            "AllEqual(x, x) cannot change with x, but provenance retained {observed:?} for {value:?}"
        );
    }
}
