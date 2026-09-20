//! Regression tests for production defects found by tracing public contracts
//! through compilation, evaluation, diagnostics, and provenance.
//!
//! These tests are ignored so the ordinary development gate stays green. Run
//! them with `cargo test -p mossignal --test regression_discovered_bugs -- --ignored`.

use std::collections::BTreeSet;

use mossignal::signal::{LogicLevel, PulseCount};
use mossignal::time::Time;
use mossignal::{
    CauseInspection, CauseRef, NetworkBuilder, OutputEvent, ProvenanceView, RuntimePolicy,
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
