#[allow(dead_code)]
mod support;

use mossignal::authored::{NodeKind, UncheckedModule, UncheckedNetwork};
use mossignal::builder::Signal;
use mossignal::diagnostics::DiagnosticCode;
use mossignal::key::{
    ExternalInputKey, ExternalOutputKey, ModuleInputKey, ModuleOutputKey, NetworkKey,
};
use mossignal::metadata::DiagnosticMeta;
use mossignal::signal::{Level, LogicLevel};
use mossignal::time::Time;
use mossignal::{
    AuthoringFailure, CauseInspection, CauseRef, ModuleBuilder, NetworkBuilder, OutputEvent,
    ProvenanceSubject, RuntimePolicy, Schedule, TimeDomainId, Transaction,
};
use std::collections::BTreeSet;
use support::{Circuit, TestTicks, run_complete_trace};

const DOMAIN_ID: u128 = 0x4e56;

#[test]
fn network_conveniences_are_exact_ordinary_primitive_authoring() {
    let convenience = network_definition(true);
    let direct = network_definition(false);

    assert_eq!(convenience, direct);
    assert!(convenience.module_instances().is_empty());
    assert_eq!(convenience.nodes().len(), 9);
    assert_eq!(convenience.connections().len(), 16);
    assert!(matches!(convenience.nodes()[0].kind(), NodeKind::Parity));
    assert!(matches!(convenience.nodes()[1].kind(), NodeKind::All));
    assert!(matches!(
        convenience.nodes()[2].kind(),
        NodeKind::AtLeast(config) if config.threshold == 2
    ));
    assert!(matches!(convenience.nodes()[3].kind(), NodeKind::All));
    assert!(matches!(convenience.nodes()[4].kind(), NodeKind::Not));
    assert!(matches!(convenience.nodes()[5].kind(), NodeKind::Any));
    assert!(matches!(convenience.nodes()[6].kind(), NodeKind::Not));
    assert!(matches!(convenience.nodes()[7].kind(), NodeKind::Parity));
    assert!(matches!(convenience.nodes()[8].kind(), NodeKind::Not));

    let convenience = convenience.validate();
    let direct = direct.validate();
    assert_eq!(convenience.diagnostics(), direct.diagnostics());
    assert_eq!(
        convenience.artifact().unwrap().fingerprint(),
        direct.artifact().unwrap().fingerprint()
    );
}

#[test]
fn module_builder_conveniences_match_direct_primitive_structure_and_identity() {
    let convenience = module_definition(true);
    let direct = module_definition(false);

    assert_eq!(convenience, direct);
    assert!(convenience.module_instances().is_empty());
    assert_eq!(convenience.nodes().len(), 9);

    let convenience = convenience.validate();
    let direct = direct.validate();
    assert_eq!(convenience.diagnostics(), direct.diagnostics());
    assert_eq!(
        convenience.artifact().unwrap().fingerprint(),
        direct.artifact().unwrap().fingerprint()
    );
}

#[test]
fn majority_uses_one_exact_strict_threshold_for_arities_zero_through_five() {
    for arity in 0..=5 {
        let mut builder = NetworkBuilder::<TestTicks>::new(TimeDomainId::from_u128(DOMAIN_ID));
        let inputs = (0..arity)
            .map(|index| builder.level_input(format!("input-{index}")).1)
            .collect::<Vec<_>>();
        let result = builder.majority(inputs).unwrap();
        builder.level_output("result", result).unwrap();
        let definition = builder.into_unchecked();

        assert_eq!(definition.nodes().len(), 1);
        assert_eq!(definition.nodes()[0].ports().inputs().len(), arity);
        assert!(matches!(
            definition.nodes()[0].kind(),
            NodeKind::AtLeast(config)
                if config.threshold == arity as u64 / 2 + 1
        ));
    }
}

#[test]
fn all_conveniences_obey_binary_and_variadic_boundary_laws() {
    let circuit = boundary_circuit();
    let valuations = binary_valuations();
    let trace = run_complete_trace(&circuit, &valuations);

    for (values, settled) in valuations.iter().zip(trace.settled_outputs()) {
        let a = values[0] == LogicLevel::High;
        let b = values[1] == LogicLevel::High;
        let expected = [
            a ^ b,
            a && b,
            a && b,
            !(a && b),
            !(a || b),
            a == b,
            false,
            a,
            a,
            false,
            !a,
            true,
            !a,
            false,
            a,
            true,
        ]
        .map(level);
        assert_eq!(settled, expected);
    }
}

#[test]
fn convenience_boundaries_preserve_only_ordinary_primitive_diagnostics() {
    let convenience = boundary_definition(true).validate();
    let direct = boundary_definition(false).validate();

    assert_eq!(convenience.diagnostics(), direct.diagnostics());
    let codes = convenience
        .diagnostics()
        .iter()
        .map(|diagnostic| diagnostic.problem().code())
        .collect::<Vec<_>>();
    assert!(codes.contains(&DiagnosticCode::ValidationEmptyVariadicNode));
    assert!(codes.contains(&DiagnosticCode::ValidationUnaryDegenerateNode));
    assert!(codes.contains(&DiagnosticCode::ValidationDuplicateSource));
    assert!(codes.iter().all(|code| matches!(
        code,
        DiagnosticCode::ValidationEmptyVariadicNode
            | DiagnosticCode::ValidationUnaryDegenerateNode
            | DiagnosticCode::ValidationDuplicateSource
    )));
}

#[test]
fn execution_provenance_contains_only_canonical_primitive_subjects_and_no_pending_work() {
    let definition = network_definition(true);
    let node_keys = definition
        .nodes()
        .iter()
        .map(|node| node.key())
        .collect::<BTreeSet<_>>();
    let validated = definition
        .validate()
        .require_artifact()
        .unwrap_or_else(|failure| panic!("convenience network must validate: {failure:?}"));
    let compiled = validated
        .compile()
        .require_artifact()
        .unwrap_or_else(|failure| panic!("convenience network must compile: {failure:?}"));
    let snapshot = compiled
        .input_snapshot()
        .set(ExternalInputKey::from_u128(1), LogicLevel::High)
        .and_then(|builder| builder.set(ExternalInputKey::from_u128(2), LogicLevel::Low))
        .and_then(|builder| builder.set(ExternalInputKey::from_u128(3), LogicLevel::High))
        .and_then(|builder| builder.finish())
        .unwrap_or_else(|failure| panic!("complete convenience snapshot must build: {failure}"));
    let mut machine = compiled.spawn(runtime_policy());
    let result = machine
        .apply(Transaction::initialize(
            Time::from_ticks(0),
            machine.revision(),
            snapshot,
        ))
        .unwrap_or_else(|failure| panic!("convenience initialization must succeed: {failure}"));

    assert_eq!(result.schedule(), Schedule::Dormant);
    assert_eq!(result.output_events().len(), 6);
    for event in result.output_events() {
        let cause = match event {
            OutputEvent::LevelEstablished { cause, .. } => *cause,
            _ => panic!("level conveniences must emit only established level outputs"),
        };
        assert_canonical_level_provenance(result.provenance(), cause, &node_keys);
    }
}

#[test]
fn select_provenance_contains_only_the_selector_and_selected_branch() {
    let mut builder = NetworkBuilder::<TestTicks>::new(TimeDomainId::from_u128(DOMAIN_ID));
    let (selector_key, selector) = builder.level_input("selector");
    let (when_low_key, when_low) = builder.level_input("when-low");
    let (when_high_key, when_high) = builder.level_input("when-high");
    let selected = builder
        .select(selector, when_low, when_high)
        .unwrap_or_else(|failure| panic!("select must author: {failure:?}"));
    let output = builder
        .level_output("selected", selected)
        .unwrap_or_else(|failure| panic!("select output must author: {failure:?}"));
    let compiled = builder
        .finish()
        .require_artifact()
        .unwrap_or_else(|failure| panic!("select must validate: {failure:?}"))
        .compile()
        .require_artifact()
        .unwrap_or_else(|failure| panic!("select must compile: {failure:?}"));

    let snapshot = compiled
        .input_snapshot()
        .set(selector_key, LogicLevel::Low)
        .and_then(|builder| builder.set(when_low_key, LogicLevel::Low))
        .and_then(|builder| builder.set(when_high_key, LogicLevel::High))
        .and_then(mossignal::InputSnapshotBuilder::finish)
        .unwrap_or_else(|failure| panic!("select snapshot must build: {failure}"));
    let mut machine = compiled.spawn(runtime_policy());
    let initialized = machine
        .apply(Transaction::initialize(
            Time::from_ticks(0),
            machine.revision(),
            snapshot,
        ))
        .unwrap_or_else(|failure| panic!("select initialization must succeed: {failure}"));
    let initial_cause = match initialized.output_events() {
        [
            OutputEvent::LevelEstablished {
                output: established,
                value,
                cause,
                ..
            },
        ] => {
            assert_eq!(*established, output);
            assert_eq!(*value, LogicLevel::Low);
            *cause
        }
        _ => panic!("select initialization must establish one low output"),
    };
    let initial_support = observed_level_inputs(initialized.provenance(), initial_cause);
    assert_eq!(
        initial_support,
        BTreeSet::from([selector_key.as_u128(), when_low_key.as_u128()])
    );
    assert!(!initial_support.contains(&when_high_key.as_u128()));

    let changed = machine
        .apply(Transaction::advance(
            Time::from_ticks(1),
            machine.revision(),
            compiled
                .input_delta()
                .set(selector_key, LogicLevel::High)
                .and_then(mossignal::InputDeltaBuilder::finish)
                .unwrap_or_else(|failure| panic!("select delta must build: {failure}")),
        ))
        .unwrap_or_else(|failure| panic!("select branch switch must succeed: {failure}"));
    let changed_cause = match changed.output_events() {
        [
            OutputEvent::LevelChanged {
                output: changed_output,
                from,
                to,
                cause,
                ..
            },
        ] => {
            assert_eq!(*changed_output, output);
            assert_eq!(*from, LogicLevel::Low);
            assert_eq!(*to, LogicLevel::High);
            *cause
        }
        _ => panic!("select branch switch must change one output"),
    };
    let changed_support = observed_level_inputs(changed.provenance(), changed_cause);
    assert_eq!(
        changed_support,
        BTreeSet::from([selector_key.as_u128(), when_high_key.as_u128()])
    );
    assert!(!changed_support.contains(&when_low_key.as_u128()));
}

#[test]
fn all_provenance_contains_low_blockers_or_all_high_inputs() {
    let mut builder = NetworkBuilder::<TestTicks>::new(TimeDomainId::from_u128(DOMAIN_ID));
    let (first_key, first) = builder.level_input("first");
    let (second_key, second) = builder.level_input("second");
    let (third_key, third) = builder.level_input("third");
    let conjunction = builder
        .all([first, second, third, first])
        .unwrap_or_else(|failure| panic!("all must author: {failure:?}"));
    let output = builder
        .level_output("all", conjunction)
        .unwrap_or_else(|failure| panic!("all output must author: {failure:?}"));
    let identity = builder
        .all([])
        .unwrap_or_else(|failure| panic!("nullary all must author: {failure:?}"));
    let identity_output = builder
        .level_output("empty-all", identity)
        .unwrap_or_else(|failure| panic!("nullary all output must author: {failure:?}"));
    let compiled = builder
        .finish()
        .require_artifact()
        .unwrap_or_else(|failure| panic!("all must validate: {failure:?}"))
        .compile()
        .require_artifact()
        .unwrap_or_else(|failure| panic!("all must compile: {failure:?}"));

    let snapshot = compiled
        .input_snapshot()
        .set(first_key, LogicLevel::Low)
        .and_then(|builder| builder.set(second_key, LogicLevel::High))
        .and_then(|builder| builder.set(third_key, LogicLevel::Low))
        .and_then(mossignal::InputSnapshotBuilder::finish)
        .unwrap_or_else(|failure| panic!("all snapshot must build: {failure}"));
    let mut machine = compiled.spawn(runtime_policy());
    let initialized = machine
        .apply(Transaction::initialize(
            Time::from_ticks(0),
            machine.revision(),
            snapshot,
        ))
        .unwrap_or_else(|failure| panic!("all initialization must succeed: {failure}"));
    let (initial_value, initial_cause) = initialized
        .output_events()
        .iter()
        .find_map(|event| match event {
            OutputEvent::LevelEstablished {
                output: established,
                value,
                cause,
                ..
            } if *established == output => Some((*value, *cause)),
            _ => None,
        })
        .unwrap_or_else(|| panic!("All initialization must establish its output"));
    assert_eq!(initial_value, LogicLevel::Low);
    assert_eq!(
        observed_level_inputs(initialized.provenance(), initial_cause),
        BTreeSet::from([first_key.as_u128(), third_key.as_u128()])
    );

    let (identity_value, identity_cause) = initialized
        .output_events()
        .iter()
        .find_map(|event| match event {
            OutputEvent::LevelEstablished {
                output: established,
                value,
                cause,
                ..
            } if *established == identity_output => Some((*value, *cause)),
            _ => None,
        })
        .unwrap_or_else(|| panic!("nullary All must establish its output"));
    assert_eq!(identity_value, LogicLevel::High);
    assert!(observed_level_inputs(initialized.provenance(), identity_cause).is_empty());

    let changed = machine
        .apply(Transaction::advance(
            Time::from_ticks(1),
            machine.revision(),
            compiled
                .input_delta()
                .set(first_key, LogicLevel::High)
                .and_then(|builder| builder.set(third_key, LogicLevel::High))
                .and_then(mossignal::InputDeltaBuilder::finish)
                .unwrap_or_else(|failure| panic!("all delta must build: {failure}")),
        ))
        .unwrap_or_else(|failure| panic!("all transition to High must succeed: {failure}"));
    let (changed_value, changed_cause) = changed
        .output_events()
        .iter()
        .find_map(|event| match event {
            OutputEvent::LevelChanged {
                output: changed_output,
                to,
                cause,
                ..
            } if *changed_output == output => Some((*to, *cause)),
            _ => None,
        })
        .unwrap_or_else(|| panic!("All must publish its transition to High"));
    assert_eq!(changed_value, LogicLevel::High);
    assert_eq!(
        observed_level_inputs(changed.provenance(), changed_cause),
        BTreeSet::from([
            first_key.as_u128(),
            second_key.as_u128(),
            third_key.as_u128(),
        ])
    );
}

#[test]
fn any_provenance_contains_high_supporters_or_all_low_inputs() {
    let mut builder = NetworkBuilder::<TestTicks>::new(TimeDomainId::from_u128(DOMAIN_ID));
    let (first_key, first) = builder.level_input("first");
    let (second_key, second) = builder.level_input("second");
    let (third_key, third) = builder.level_input("third");
    let disjunction = builder
        .any([first, second, third, first])
        .unwrap_or_else(|failure| panic!("any must author: {failure:?}"));
    let output = builder
        .level_output("any", disjunction)
        .unwrap_or_else(|failure| panic!("any output must author: {failure:?}"));
    let identity = builder
        .any([])
        .unwrap_or_else(|failure| panic!("nullary any must author: {failure:?}"));
    let identity_output = builder
        .level_output("empty-any", identity)
        .unwrap_or_else(|failure| panic!("nullary any output must author: {failure:?}"));
    let compiled = builder
        .finish()
        .require_artifact()
        .unwrap_or_else(|failure| panic!("any must validate: {failure:?}"))
        .compile()
        .require_artifact()
        .unwrap_or_else(|failure| panic!("any must compile: {failure:?}"));

    let snapshot = compiled
        .input_snapshot()
        .set(first_key, LogicLevel::High)
        .and_then(|builder| builder.set(second_key, LogicLevel::Low))
        .and_then(|builder| builder.set(third_key, LogicLevel::High))
        .and_then(mossignal::InputSnapshotBuilder::finish)
        .unwrap_or_else(|failure| panic!("any snapshot must build: {failure}"));
    let mut machine = compiled.spawn(runtime_policy());
    let initialized = machine
        .apply(Transaction::initialize(
            Time::from_ticks(0),
            machine.revision(),
            snapshot,
        ))
        .unwrap_or_else(|failure| panic!("any initialization must succeed: {failure}"));
    let (initial_value, initial_cause) = initialized
        .output_events()
        .iter()
        .find_map(|event| match event {
            OutputEvent::LevelEstablished {
                output: established,
                value,
                cause,
                ..
            } if *established == output => Some((*value, *cause)),
            _ => None,
        })
        .unwrap_or_else(|| panic!("Any initialization must establish its output"));
    assert_eq!(initial_value, LogicLevel::High);
    assert_eq!(
        observed_level_inputs(initialized.provenance(), initial_cause),
        BTreeSet::from([first_key.as_u128(), third_key.as_u128()])
    );

    let (identity_value, identity_cause) = initialized
        .output_events()
        .iter()
        .find_map(|event| match event {
            OutputEvent::LevelEstablished {
                output: established,
                value,
                cause,
                ..
            } if *established == identity_output => Some((*value, *cause)),
            _ => None,
        })
        .unwrap_or_else(|| panic!("nullary Any must establish its output"));
    assert_eq!(identity_value, LogicLevel::Low);
    assert!(observed_level_inputs(initialized.provenance(), identity_cause).is_empty());

    let changed = machine
        .apply(Transaction::advance(
            Time::from_ticks(1),
            machine.revision(),
            compiled
                .input_delta()
                .set(first_key, LogicLevel::Low)
                .and_then(|builder| builder.set(third_key, LogicLevel::Low))
                .and_then(mossignal::InputDeltaBuilder::finish)
                .unwrap_or_else(|failure| panic!("any delta must build: {failure}")),
        ))
        .unwrap_or_else(|failure| panic!("any transition to Low must succeed: {failure}"));
    let (changed_value, changed_cause) = changed
        .output_events()
        .iter()
        .find_map(|event| match event {
            OutputEvent::LevelChanged {
                output: changed_output,
                to,
                cause,
                ..
            } if *changed_output == output => Some((*to, *cause)),
            _ => None,
        })
        .unwrap_or_else(|| panic!("Any must publish its transition to Low"));
    assert_eq!(changed_value, LogicLevel::Low);
    assert_eq!(
        observed_level_inputs(changed.provenance(), changed_cause),
        BTreeSet::from([
            first_key.as_u128(),
            second_key.as_u128(),
            third_key.as_u128(),
        ])
    );
}

#[test]
fn at_least_zero_provenance_is_independent_of_inputs() {
    let mut builder = NetworkBuilder::<TestTicks>::new(TimeDomainId::from_u128(DOMAIN_ID));
    let (left_key, left) = builder.level_input("left");
    let (right_key, right) = builder.level_input("right");
    let constant_high = builder
        .at_least(0, [left, right])
        .unwrap_or_else(|failure| panic!("AtLeast(0) must author: {failure:?}"));
    let output = builder
        .level_output("constant-high", constant_high)
        .unwrap_or_else(|failure| panic!("AtLeast(0) output must author: {failure:?}"));
    let empty_high = builder
        .at_least(0, [])
        .unwrap_or_else(|failure| panic!("nullary AtLeast(0) must author: {failure:?}"));
    let empty_output = builder
        .level_output("empty-high", empty_high)
        .unwrap_or_else(|failure| panic!("nullary AtLeast(0) output must author: {failure:?}"));
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
        .unwrap_or_else(|failure| panic!("AtLeast(0) snapshot must build: {failure}"));
    let mut machine = compiled.spawn(runtime_policy());
    let initialized = machine
        .apply(Transaction::initialize(
            Time::from_ticks(0),
            machine.revision(),
            snapshot,
        ))
        .unwrap_or_else(|failure| panic!("AtLeast(0) initialization must succeed: {failure}"));
    for expected_output in [output, empty_output] {
        let (value, cause) = initialized
            .output_events()
            .iter()
            .find_map(|event| match event {
                OutputEvent::LevelEstablished {
                    output: established,
                    value,
                    cause,
                    ..
                } if *established == expected_output => Some((*value, *cause)),
                _ => None,
            })
            .unwrap_or_else(|| panic!("AtLeast(0) must establish every output"));
        assert_eq!(value, LogicLevel::High);
        assert!(observed_level_inputs(initialized.provenance(), cause).is_empty());
    }

    let changed = machine
        .apply(Transaction::advance(
            Time::from_ticks(1),
            machine.revision(),
            compiled
                .input_delta()
                .set(left_key, LogicLevel::Low)
                .and_then(|builder| builder.set(right_key, LogicLevel::High))
                .and_then(mossignal::InputDeltaBuilder::finish)
                .unwrap_or_else(|failure| panic!("AtLeast(0) delta must build: {failure}")),
        ))
        .unwrap_or_else(|failure| panic!("AtLeast(0) input change must succeed: {failure}"));
    assert!(changed.output_events().is_empty());
    let retained_cause = machine
        .output_cause(output)
        .unwrap_or_else(|| panic!("AtLeast(0) must retain an output cause"));
    assert!(observed_level_inputs(changed.provenance(), retained_cause).is_empty());
}

#[test]
fn at_least_above_arity_provenance_is_independent_of_inputs() {
    let mut builder = NetworkBuilder::<TestTicks>::new(TimeDomainId::from_u128(DOMAIN_ID));
    let (left_key, left) = builder.level_input("left");
    let (right_key, right) = builder.level_input("right");
    let constant_low = builder
        .at_least(3, [left, right])
        .unwrap_or_else(|failure| panic!("AtLeast(3) of two inputs must author: {failure:?}"));
    let output = builder
        .level_output("constant-low", constant_low)
        .unwrap_or_else(|failure| panic!("AtLeast(3) output must author: {failure:?}"));
    let nullary_low = builder
        .at_least(1, [])
        .unwrap_or_else(|failure| panic!("nullary AtLeast(1) must author: {failure:?}"));
    let nullary_output = builder
        .level_output("nullary-low", nullary_low)
        .unwrap_or_else(|failure| panic!("nullary AtLeast(1) output must author: {failure:?}"));
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
        .unwrap_or_else(|failure| panic!("AtLeast(3) snapshot must build: {failure}"));
    let mut machine = compiled.spawn(runtime_policy());
    let initialized = machine
        .apply(Transaction::initialize(
            Time::from_ticks(0),
            machine.revision(),
            snapshot,
        ))
        .unwrap_or_else(|failure| panic!("AtLeast(3) initialization must succeed: {failure}"));
    for expected_output in [output, nullary_output] {
        let (value, cause) = initialized
            .output_events()
            .iter()
            .find_map(|event| match event {
                OutputEvent::LevelEstablished {
                    output: established,
                    value,
                    cause,
                    ..
                } if *established == expected_output => Some((*value, *cause)),
                _ => None,
            })
            .unwrap_or_else(|| panic!("constant Low output must establish"));
        assert_eq!(value, LogicLevel::Low);
        assert!(observed_level_inputs(initialized.provenance(), cause).is_empty());
    }

    for (time, left_level, right_level) in [
        (1, LogicLevel::Low, LogicLevel::High),
        (2, LogicLevel::High, LogicLevel::Low),
    ] {
        let changed = machine
            .apply(Transaction::advance(
                Time::from_ticks(time),
                machine.revision(),
                compiled
                    .input_delta()
                    .set(left_key, left_level)
                    .and_then(|builder| builder.set(right_key, right_level))
                    .and_then(mossignal::InputDeltaBuilder::finish)
                    .unwrap_or_else(|failure| panic!("AtLeast delta must build: {failure}")),
            ))
            .unwrap_or_else(|failure| panic!("AtLeast input change must succeed: {failure}"));
        assert!(changed.output_events().is_empty());
        for expected_output in [output, nullary_output] {
            let cause = machine
                .output_cause(expected_output)
                .unwrap_or_else(|| panic!("constant Low output must retain a cause"));
            assert!(observed_level_inputs(changed.provenance(), cause).is_empty());
        }
    }
}

#[test]
fn at_least_high_provenance_contains_all_high_inputs_only() {
    let mut builder = NetworkBuilder::<TestTicks>::new(TimeDomainId::from_u128(DOMAIN_ID));
    let (first_key, first) = builder.level_input("first");
    let (second_key, second) = builder.level_input("second");
    let (third_key, third) = builder.level_input("third");
    let (fourth_key, fourth) = builder.level_input("fourth");
    let threshold = builder
        .at_least(2, [first, second, third, fourth])
        .unwrap_or_else(|failure| panic!("AtLeast(2) must author: {failure:?}"));
    let output = builder
        .level_output("at-least", threshold)
        .unwrap_or_else(|failure| panic!("AtLeast output must author: {failure:?}"));
    let compiled = builder
        .finish()
        .require_artifact()
        .unwrap_or_else(|failure| panic!("AtLeast must validate: {failure:?}"))
        .compile()
        .require_artifact()
        .unwrap_or_else(|failure| panic!("AtLeast must compile: {failure:?}"));

    let snapshot = compiled
        .input_snapshot()
        .set(first_key, LogicLevel::High)
        .and_then(|builder| builder.set(second_key, LogicLevel::Low))
        .and_then(|builder| builder.set(third_key, LogicLevel::Low))
        .and_then(|builder| builder.set(fourth_key, LogicLevel::Low))
        .and_then(mossignal::InputSnapshotBuilder::finish)
        .unwrap_or_else(|failure| panic!("AtLeast snapshot must build: {failure}"));
    let mut machine = compiled.spawn(runtime_policy());
    let initialized = machine
        .apply(Transaction::initialize(
            Time::from_ticks(0),
            machine.revision(),
            snapshot,
        ))
        .unwrap_or_else(|failure| panic!("AtLeast initialization must succeed: {failure}"));
    let (initial_value, initial_cause) = initialized
        .output_events()
        .iter()
        .find_map(|event| match event {
            OutputEvent::LevelEstablished {
                output: established,
                value,
                cause,
                ..
            } if *established == output => Some((*value, *cause)),
            _ => None,
        })
        .unwrap_or_else(|| panic!("AtLeast initialization must establish its output"));
    assert_eq!(initial_value, LogicLevel::Low);
    assert_eq!(
        observed_level_inputs(initialized.provenance(), initial_cause),
        BTreeSet::from([
            first_key.as_u128(),
            second_key.as_u128(),
            third_key.as_u128(),
            fourth_key.as_u128(),
        ])
    );

    let first_high = machine
        .apply(Transaction::advance(
            Time::from_ticks(1),
            machine.revision(),
            compiled
                .input_delta()
                .set(second_key, LogicLevel::High)
                .and_then(mossignal::InputDeltaBuilder::finish)
                .unwrap_or_else(|failure| panic!("AtLeast High delta must build: {failure}")),
        ))
        .unwrap_or_else(|failure| panic!("AtLeast transition to High must succeed: {failure}"));
    let first_high_cause = first_high
        .output_events()
        .iter()
        .find_map(|event| match event {
            OutputEvent::LevelChanged {
                output: changed_output,
                to,
                cause,
                ..
            } if *changed_output == output => Some((*to, *cause)),
            _ => None,
        })
        .unwrap_or_else(|| panic!("AtLeast must publish its transition to High"));
    assert_eq!(first_high_cause.0, LogicLevel::High);
    assert_eq!(
        observed_level_inputs(first_high.provenance(), first_high_cause.1),
        BTreeSet::from([first_key.as_u128(), second_key.as_u128()])
    );

    let low_again = machine
        .apply(Transaction::advance(
            Time::from_ticks(2),
            machine.revision(),
            compiled
                .input_delta()
                .set(second_key, LogicLevel::Low)
                .and_then(mossignal::InputDeltaBuilder::finish)
                .unwrap_or_else(|failure| panic!("AtLeast Low delta must build: {failure}")),
        ))
        .unwrap_or_else(|failure| panic!("AtLeast transition back to Low must succeed: {failure}"));
    let low_again_cause = low_again
        .output_events()
        .iter()
        .find_map(|event| match event {
            OutputEvent::LevelChanged {
                output: changed_output,
                to,
                cause,
                ..
            } if *changed_output == output => Some((*to, *cause)),
            _ => None,
        })
        .unwrap_or_else(|| panic!("AtLeast must publish its Low transition"));
    assert_eq!(low_again_cause.0, LogicLevel::Low);
    assert_eq!(
        observed_level_inputs(low_again.provenance(), low_again_cause.1),
        BTreeSet::from([
            first_key.as_u128(),
            second_key.as_u128(),
            third_key.as_u128(),
            fourth_key.as_u128(),
        ])
    );

    let more_high = machine
        .apply(Transaction::advance(
            Time::from_ticks(3),
            machine.revision(),
            compiled
                .input_delta()
                .set(first_key, LogicLevel::Low)
                .and_then(|builder| builder.set(second_key, LogicLevel::High))
                .and_then(|builder| builder.set(third_key, LogicLevel::High))
                .and_then(|builder| builder.set(fourth_key, LogicLevel::High))
                .and_then(mossignal::InputDeltaBuilder::finish)
                .unwrap_or_else(|failure| panic!("AtLeast supporter delta must build: {failure}")),
        ))
        .unwrap_or_else(|failure| panic!("AtLeast supporter change must succeed: {failure}"));
    let more_high_cause = more_high
        .output_events()
        .iter()
        .find_map(|event| match event {
            OutputEvent::LevelChanged {
                output: changed_output,
                to,
                cause,
                ..
            } if *changed_output == output => Some((*to, *cause)),
            _ => None,
        })
        .unwrap_or_else(|| panic!("AtLeast must publish its High transition"));
    assert_eq!(more_high_cause.0, LogicLevel::High);
    assert_eq!(
        observed_level_inputs(more_high.provenance(), more_high_cause.1),
        BTreeSet::from([
            second_key.as_u128(),
            third_key.as_u128(),
            fourth_key.as_u128(),
        ])
    );

    let permuted_low = machine
        .apply(Transaction::advance(
            Time::from_ticks(4),
            machine.revision(),
            compiled
                .input_delta()
                .set(second_key, LogicLevel::Low)
                .and_then(|builder| builder.set(third_key, LogicLevel::Low))
                .and_then(|builder| builder.set(fourth_key, LogicLevel::Low))
                .and_then(mossignal::InputDeltaBuilder::finish)
                .unwrap_or_else(|failure| panic!("AtLeast Low delta must build: {failure}")),
        ))
        .unwrap_or_else(|failure| panic!("AtLeast permutation Low change must succeed: {failure}"));
    let permuted_low_cause = permuted_low
        .output_events()
        .iter()
        .find_map(|event| match event {
            OutputEvent::LevelChanged {
                output: changed_output,
                to,
                cause,
                ..
            } if *changed_output == output => Some((*to, *cause)),
            _ => None,
        })
        .unwrap_or_else(|| panic!("AtLeast must publish its permutation Low transition"));
    assert_eq!(permuted_low_cause.0, LogicLevel::Low);
    assert_eq!(
        observed_level_inputs(permuted_low.provenance(), permuted_low_cause.1),
        BTreeSet::from([
            first_key.as_u128(),
            second_key.as_u128(),
            third_key.as_u128(),
            fourth_key.as_u128(),
        ])
    );

    let permuted_high = machine
        .apply(Transaction::advance(
            Time::from_ticks(5),
            machine.revision(),
            compiled
                .input_delta()
                .set(third_key, LogicLevel::High)
                .and_then(|builder| builder.set(fourth_key, LogicLevel::High))
                .and_then(mossignal::InputDeltaBuilder::finish)
                .unwrap_or_else(|failure| {
                    panic!("AtLeast permutation High delta must build: {failure}")
                }),
        ))
        .unwrap_or_else(|failure| {
            panic!("AtLeast permutation High change must succeed: {failure}")
        });
    let permuted_high_cause = permuted_high
        .output_events()
        .iter()
        .find_map(|event| match event {
            OutputEvent::LevelChanged {
                output: changed_output,
                to,
                cause,
                ..
            } if *changed_output == output => Some((*to, *cause)),
            _ => None,
        })
        .unwrap_or_else(|| panic!("AtLeast must publish its permutation High transition"));
    assert_eq!(permuted_high_cause.0, LogicLevel::High);
    assert_eq!(
        observed_level_inputs(permuted_high.provenance(), permuted_high_cause.1),
        BTreeSet::from([third_key.as_u128(), fourth_key.as_u128()])
    );
}

#[test]
fn parity_provenance_cancels_even_source_multiplicity() {
    let mut builder = NetworkBuilder::<TestTicks>::new(TimeDomainId::from_u128(DOMAIN_ID));
    let (x_key, x) = builder.level_input("x");
    let (y_key, y) = builder.level_input("y");
    let even_x = builder
        .parity([x, x])
        .unwrap_or_else(|failure| panic!("Parity(x, x) must author: {failure:?}"));
    let even_output = builder
        .level_output("even-x", even_x)
        .unwrap_or_else(|failure| panic!("even Parity output must author: {failure:?}"));
    let odd_x = builder
        .parity([x, x, x])
        .unwrap_or_else(|failure| panic!("Parity(x, x, x) must author: {failure:?}"));
    let odd_output = builder
        .level_output("odd-x", odd_x)
        .unwrap_or_else(|failure| panic!("odd Parity output must author: {failure:?}"));
    let independent = builder
        .parity([x, y])
        .unwrap_or_else(|failure| panic!("Parity(x, y) must author: {failure:?}"));
    let independent_output = builder
        .level_output("independent", independent)
        .unwrap_or_else(|failure| panic!("independent Parity output must author: {failure:?}"));
    let even_plus_y = builder
        .parity([x, x, y])
        .unwrap_or_else(|failure| panic!("Parity(x, x, y) must author: {failure:?}"));
    let even_plus_y_output = builder
        .level_output("even-x-plus-y", even_plus_y)
        .unwrap_or_else(|failure| panic!("mixed Parity output must author: {failure:?}"));
    let compiled = builder
        .finish()
        .require_artifact()
        .unwrap_or_else(|failure| panic!("Parity network must validate: {failure:?}"))
        .compile()
        .require_artifact()
        .unwrap_or_else(|failure| panic!("Parity network must compile: {failure:?}"));
    let empty_support = BTreeSet::new();
    let x_support = BTreeSet::from([x_key.as_u128()]);
    let y_support = BTreeSet::from([y_key.as_u128()]);
    let xy_support = BTreeSet::from([x_key.as_u128(), y_key.as_u128()]);
    let assert_output = |machine: &mossignal::Machine<TestTicks>,
                         result: &mossignal::TransactionResult<TestTicks>,
                         output: ExternalOutputKey<Level>,
                         expected: LogicLevel,
                         expected_support: BTreeSet<u128>| {
        assert_eq!(machine.output_level(output), Some(expected));
        let cause = machine
            .output_cause(output)
            .unwrap_or_else(|| panic!("Parity output must retain a cause"));
        assert_eq!(
            observed_level_inputs(result.provenance(), cause),
            expected_support
        );
    };

    let initialized = {
        let snapshot = compiled
            .input_snapshot()
            .set(x_key, LogicLevel::Low)
            .and_then(|builder| builder.set(y_key, LogicLevel::Low))
            .and_then(mossignal::InputSnapshotBuilder::finish)
            .unwrap_or_else(|failure| panic!("Parity snapshot must build: {failure}"));
        let mut machine = compiled.spawn(runtime_policy());
        let result = machine
            .apply(Transaction::initialize(
                Time::from_ticks(0),
                machine.revision(),
                snapshot,
            ))
            .unwrap_or_else(|failure| panic!("Parity initialization must succeed: {failure}"));
        assert_output(
            &machine,
            &result,
            even_output,
            LogicLevel::Low,
            empty_support.clone(),
        );
        assert_output(
            &machine,
            &result,
            odd_output,
            LogicLevel::Low,
            x_support.clone(),
        );
        assert_output(
            &machine,
            &result,
            independent_output,
            LogicLevel::Low,
            xy_support.clone(),
        );
        assert_output(
            &machine,
            &result,
            even_plus_y_output,
            LogicLevel::Low,
            y_support.clone(),
        );
        (machine, result)
    };
    let (mut machine, _initialized) = initialized;

    let x_high = machine
        .apply(Transaction::advance(
            Time::from_ticks(1),
            machine.revision(),
            compiled
                .input_delta()
                .set(x_key, LogicLevel::High)
                .and_then(mossignal::InputDeltaBuilder::finish)
                .unwrap_or_else(|failure| panic!("Parity x-high delta must build: {failure}")),
        ))
        .unwrap_or_else(|failure| panic!("Parity x-high transition must succeed: {failure}"));
    assert_output(
        &machine,
        &x_high,
        even_output,
        LogicLevel::Low,
        empty_support.clone(),
    );
    assert_output(
        &machine,
        &x_high,
        odd_output,
        LogicLevel::High,
        x_support.clone(),
    );
    assert_output(
        &machine,
        &x_high,
        independent_output,
        LogicLevel::High,
        xy_support.clone(),
    );
    assert_output(
        &machine,
        &x_high,
        even_plus_y_output,
        LogicLevel::Low,
        y_support.clone(),
    );

    let y_high = machine
        .apply(Transaction::advance(
            Time::from_ticks(2),
            machine.revision(),
            compiled
                .input_delta()
                .set(y_key, LogicLevel::High)
                .and_then(mossignal::InputDeltaBuilder::finish)
                .unwrap_or_else(|failure| panic!("Parity y-high delta must build: {failure}")),
        ))
        .unwrap_or_else(|failure| panic!("Parity y-high transition must succeed: {failure}"));
    assert_output(
        &machine,
        &y_high,
        even_output,
        LogicLevel::Low,
        empty_support.clone(),
    );
    assert_output(
        &machine,
        &y_high,
        odd_output,
        LogicLevel::High,
        x_support.clone(),
    );
    assert_output(
        &machine,
        &y_high,
        independent_output,
        LogicLevel::Low,
        xy_support.clone(),
    );
    assert_output(
        &machine,
        &y_high,
        even_plus_y_output,
        LogicLevel::High,
        y_support.clone(),
    );

    let x_low = machine
        .apply(Transaction::advance(
            Time::from_ticks(3),
            machine.revision(),
            compiled
                .input_delta()
                .set(x_key, LogicLevel::Low)
                .and_then(mossignal::InputDeltaBuilder::finish)
                .unwrap_or_else(|failure| panic!("Parity x-low delta must build: {failure}")),
        ))
        .unwrap_or_else(|failure| panic!("Parity x-low transition must succeed: {failure}"));
    assert_output(
        &machine,
        &x_low,
        even_output,
        LogicLevel::Low,
        empty_support,
    );
    assert_output(&machine, &x_low, odd_output, LogicLevel::Low, x_support);
    assert_output(
        &machine,
        &x_low,
        independent_output,
        LogicLevel::High,
        xy_support,
    );
    assert_output(
        &machine,
        &x_low,
        even_plus_y_output,
        LogicLevel::High,
        y_support,
    );
}

#[test]
fn foreign_signals_fail_before_either_builder_authors_a_convenience_node() {
    let mut foreign_network = NetworkBuilder::<TestTicks>::new(TimeDomainId::from_u128(DOMAIN_ID));
    let foreign = foreign_network.level_input("foreign").1;
    let mut network = NetworkBuilder::<TestTicks>::new(TimeDomainId::from_u128(DOMAIN_ID));
    let local = network.level_input("local").1;
    assert_foreign_failures(&mut network, local, foreign);
    assert!(network.into_unchecked().nodes().is_empty());

    let mut foreign_module = ModuleBuilder::<TestTicks>::new();
    let foreign = foreign_module.level_input("foreign").1;
    let mut module = ModuleBuilder::<TestTicks>::new();
    let local = module.level_input("local").1;
    assert_foreign(module.xor(local, foreign));
    assert_foreign(module.level_gate(local, foreign));
    assert_foreign(module.majority([local, foreign]));
    assert_foreign(module.nand([local, foreign]));
    assert_foreign(module.nor([local, foreign]));
    assert_foreign(module.xnor(local, foreign));
    assert!(module.into_unchecked().nodes().is_empty());
}

fn network_definition(conveniences: bool) -> UncheckedNetwork<TestTicks> {
    let mut builder =
        NetworkBuilder::with_key(NetworkKey::from_u128(1), TimeDomainId::from_u128(DOMAIN_ID));
    let inputs = add_network_inputs(&mut builder);
    let outputs = author_network_outputs(&mut builder, inputs, conveniences);
    for (index, output) in outputs.into_iter().enumerate() {
        builder
            .add_level_output(
                ExternalOutputKey::from_u128(index as u128 + 1),
                output,
                DiagnosticMeta::default(),
            )
            .unwrap();
    }
    builder.into_unchecked()
}

fn module_definition(conveniences: bool) -> UncheckedModule<TestTicks> {
    let mut builder = ModuleBuilder::new();
    let inputs = [
        builder
            .add_level_input(ModuleInputKey::from_u128(1), DiagnosticMeta::default())
            .unwrap(),
        builder
            .add_level_input(ModuleInputKey::from_u128(2), DiagnosticMeta::default())
            .unwrap(),
        builder
            .add_level_input(ModuleInputKey::from_u128(3), DiagnosticMeta::default())
            .unwrap(),
    ];
    let outputs = if conveniences {
        author_module_conveniences(&mut builder, inputs)
    } else {
        author_module_direct(&mut builder, inputs)
    };
    for (index, output) in outputs.into_iter().enumerate() {
        builder
            .add_level_output(
                ModuleOutputKey::from_u128(index as u128 + 1),
                output,
                DiagnosticMeta::default(),
            )
            .unwrap();
    }
    builder.into_unchecked()
}

fn add_network_inputs(builder: &mut NetworkBuilder<TestTicks>) -> [Signal<Level>; 3] {
    [
        builder
            .add_level_input(ExternalInputKey::from_u128(1), DiagnosticMeta::default())
            .unwrap(),
        builder
            .add_level_input(ExternalInputKey::from_u128(2), DiagnosticMeta::default())
            .unwrap(),
        builder
            .add_level_input(ExternalInputKey::from_u128(3), DiagnosticMeta::default())
            .unwrap(),
    ]
}

fn author_network_outputs(
    builder: &mut NetworkBuilder<TestTicks>,
    [a, b, c]: [Signal<Level>; 3],
    conveniences: bool,
) -> [Signal<Level>; 6] {
    if conveniences {
        [
            builder.xor(a, b).unwrap(),
            builder.level_gate(a, b).unwrap(),
            builder.majority([a, b, c]).unwrap(),
            builder.nand([a, b]).unwrap(),
            builder.nor([a, b]).unwrap(),
            builder.xnor(a, b).unwrap(),
        ]
    } else {
        let xor = builder.parity([a, b]).unwrap();
        let gate = builder.all([a, b]).unwrap();
        let majority = builder.at_least(2, [a, b, c]).unwrap();
        let conjunction = builder.all([a, b]).unwrap();
        let nand = builder.not(conjunction).unwrap();
        let disjunction = builder.any([a, b]).unwrap();
        let nor = builder.not(disjunction).unwrap();
        let parity = builder.parity([a, b]).unwrap();
        let xnor = builder.not(parity).unwrap();
        [xor, gate, majority, nand, nor, xnor]
    }
}

fn author_module_conveniences(
    builder: &mut ModuleBuilder<TestTicks>,
    [a, b, c]: [Signal<Level>; 3],
) -> [Signal<Level>; 6] {
    [
        builder.xor(a, b).unwrap(),
        builder.level_gate(a, b).unwrap(),
        builder.majority([a, b, c]).unwrap(),
        builder.nand([a, b]).unwrap(),
        builder.nor([a, b]).unwrap(),
        builder.xnor(a, b).unwrap(),
    ]
}

fn author_module_direct(
    builder: &mut ModuleBuilder<TestTicks>,
    [a, b, c]: [Signal<Level>; 3],
) -> [Signal<Level>; 6] {
    let xor = builder.parity([a, b]).unwrap();
    let gate = builder.all([a, b]).unwrap();
    let majority = builder.at_least(2, [a, b, c]).unwrap();
    let conjunction = builder.all([a, b]).unwrap();
    let nand = builder.not(conjunction).unwrap();
    let disjunction = builder.any([a, b]).unwrap();
    let nor = builder.not(disjunction).unwrap();
    let parity = builder.parity([a, b]).unwrap();
    let xnor = builder.not(parity).unwrap();
    [xor, gate, majority, nand, nor, xnor]
}

fn boundary_circuit() -> Circuit {
    let mut builder =
        NetworkBuilder::with_key(NetworkKey::from_u128(2), TimeDomainId::from_u128(DOMAIN_ID));
    let (a_key, a) = builder.level_input("a");
    let (b_key, b) = builder.level_input("b");
    let outputs = boundary_outputs(&mut builder, a, b, true);
    let mut output_keys = Vec::with_capacity(outputs.len());
    for (index, output) in outputs.into_iter().enumerate() {
        let key = ExternalOutputKey::from_u128(index as u128 + 1);
        builder
            .add_level_output(key, output, DiagnosticMeta::default())
            .unwrap();
        output_keys.push(key);
    }
    Circuit::compile(builder, vec![a_key, b_key], output_keys)
}

fn boundary_definition(conveniences: bool) -> UncheckedNetwork<TestTicks> {
    let mut builder =
        NetworkBuilder::with_key(NetworkKey::from_u128(3), TimeDomainId::from_u128(DOMAIN_ID));
    let (a_key, a) = builder.level_input("a");
    let (b_key, b) = builder.level_input("b");
    let outputs = boundary_outputs(&mut builder, a, b, conveniences);
    for (index, output) in outputs.into_iter().enumerate() {
        builder
            .add_level_output(
                ExternalOutputKey::from_u128(index as u128 + 1),
                output,
                DiagnosticMeta::default(),
            )
            .unwrap();
    }
    let definition = builder.into_unchecked();
    assert_eq!(definition.external_inputs()[0].key(), a_key.into());
    assert_eq!(definition.external_inputs()[1].key(), b_key.into());
    definition
}

fn boundary_outputs(
    builder: &mut NetworkBuilder<TestTicks>,
    a: Signal<Level>,
    b: Signal<Level>,
    conveniences: bool,
) -> Vec<Signal<Level>> {
    if conveniences {
        vec![
            builder.xor(a, b).unwrap(),
            builder.level_gate(a, b).unwrap(),
            builder.majority([a, b]).unwrap(),
            builder.nand([a, b]).unwrap(),
            builder.nor([a, b]).unwrap(),
            builder.xnor(a, b).unwrap(),
            builder.majority([]).unwrap(),
            builder.majority([a]).unwrap(),
            builder.majority([a, a, b]).unwrap(),
            builder.nand([]).unwrap(),
            builder.nand([a]).unwrap(),
            builder.nor([]).unwrap(),
            builder.nor([a]).unwrap(),
            builder.xor(a, a).unwrap(),
            builder.level_gate(a, a).unwrap(),
            builder.xnor(a, a).unwrap(),
        ]
    } else {
        vec![
            builder.parity([a, b]).unwrap(),
            builder.all([a, b]).unwrap(),
            builder.at_least(2, [a, b]).unwrap(),
            not_all(builder, [a, b]),
            not_any(builder, [a, b]),
            not_parity(builder, [a, b]),
            builder.at_least(1, []).unwrap(),
            builder.at_least(1, [a]).unwrap(),
            builder.at_least(2, [a, a, b]).unwrap(),
            not_all(builder, []),
            not_all(builder, [a]),
            not_any(builder, []),
            not_any(builder, [a]),
            builder.parity([a, a]).unwrap(),
            builder.all([a, a]).unwrap(),
            not_parity(builder, [a, a]),
        ]
    }
}

fn not_all<I>(builder: &mut NetworkBuilder<TestTicks>, inputs: I) -> Signal<Level>
where
    I: IntoIterator<Item = Signal<Level>>,
{
    let value = builder.all(inputs).unwrap();
    builder.not(value).unwrap()
}

fn not_any<I>(builder: &mut NetworkBuilder<TestTicks>, inputs: I) -> Signal<Level>
where
    I: IntoIterator<Item = Signal<Level>>,
{
    let value = builder.any(inputs).unwrap();
    builder.not(value).unwrap()
}

fn not_parity<I>(builder: &mut NetworkBuilder<TestTicks>, inputs: I) -> Signal<Level>
where
    I: IntoIterator<Item = Signal<Level>>,
{
    let value = builder.parity(inputs).unwrap();
    builder.not(value).unwrap()
}

fn assert_foreign_failures(
    builder: &mut NetworkBuilder<TestTicks>,
    local: Signal<Level>,
    foreign: Signal<Level>,
) {
    assert_foreign(builder.xor(local, foreign));
    assert_foreign(builder.level_gate(local, foreign));
    assert_foreign(builder.majority([local, foreign]));
    assert_foreign(builder.nand([local, foreign]));
    assert_foreign(builder.nor([local, foreign]));
    assert_foreign(builder.xnor(local, foreign));
}

fn observed_level_inputs(
    provenance: &mossignal::ProvenanceView<TestTicks>,
    root: CauseRef,
) -> BTreeSet<u128> {
    let mut found = BTreeSet::new();
    let mut pending = vec![root];
    let mut visited = BTreeSet::new();
    while let Some(cause) = pending.pop() {
        if !visited.insert(cause) {
            continue;
        }
        match provenance
            .inspect(cause)
            .unwrap_or_else(|failure| panic!("select cause must resolve: {failure}"))
        {
            CauseInspection::ExternalObservation { input, .. } => {
                found.insert(input.as_u128());
            }
            CauseInspection::Derived { supporters, .. }
            | CauseInspection::PulseDerived { supporters, .. }
            | CauseInspection::PulseControlledLevel { supporters, .. }
            | CauseInspection::PendingPulseDelay { supporters, .. } => {
                pending.extend_from_slice(supporters);
            }
            _ => {}
        }
    }
    found
}

fn assert_foreign(result: Result<Signal<Level>, AuthoringFailure>) {
    assert!(matches!(
        result,
        Err(AuthoringFailure::ForeignSignal { .. })
    ));
}

fn assert_canonical_level_provenance(
    provenance: &mossignal::ProvenanceView<TestTicks>,
    root: CauseRef,
    node_keys: &BTreeSet<mossignal::key::NodeKey>,
) {
    let mut pending = vec![root];
    let mut visited = BTreeSet::new();
    while let Some(cause) = pending.pop() {
        if !visited.insert(cause) {
            continue;
        }
        match provenance
            .inspect(cause)
            .unwrap_or_else(|failure| panic!("convenience cause must resolve: {failure}"))
        {
            CauseInspection::Derived {
                subject: ProvenanceSubject::Node(node),
                supporters,
            } => {
                assert!(node_keys.contains(&node));
                pending.extend_from_slice(supporters);
            }
            CauseInspection::Derived {
                subject: ProvenanceSubject::ExternalOutput(_),
                supporters,
            } => pending.extend_from_slice(supporters),
            CauseInspection::InitializationTransaction { .. }
            | CauseInspection::ExternalObservation { .. } => {}
            CauseInspection::Derived {
                subject: ProvenanceSubject::QualifiedNode(_),
                ..
            } => panic!("builder conveniences must not create module-qualified provenance"),
            CauseInspection::PendingPulseDelay { .. }
            | CauseInspection::PulseDerived { .. }
            | CauseInspection::ExternalPulseObservation { .. }
            | CauseInspection::ReadyTransaction { .. }
            | CauseInspection::Derived {
                subject: ProvenanceSubject::PulseExternalOutput(_),
                ..
            } => panic!("level conveniences must not create pulse or pending provenance"),
            _ => panic!("level conveniences exposed an unexpected provenance subject"),
        }
    }
}

fn runtime_policy() -> RuntimePolicy {
    RuntimePolicy::builder()
        .max_internal_reactions(10_000)
        .max_evaluated_operations(10_000)
        .max_pending_events(10_000)
        .max_events_created_per_transaction(10_000)
        .max_required_provenance_growth(10_000)
        .build()
        .unwrap_or_else(|failure| panic!("complete runtime policy must build: {failure}"))
}

fn binary_valuations() -> Vec<Vec<LogicLevel>> {
    (0..4)
        .map(|bits| vec![level(bits & 1 != 0), level(bits & 2 != 0)])
        .collect()
}

const fn level(value: bool) -> LogicLevel {
    if value {
        LogicLevel::High
    } else {
        LogicLevel::Low
    }
}
