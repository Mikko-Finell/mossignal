use mossignal::authored::NodeKind;
use mossignal::diagnostics::DiagnosticCode;
use mossignal::key::{
    ExternalInputKey, ExternalOutputKey, ModuleInputKey, ModuleInstanceKey, ModuleOutputKey,
};
use mossignal::metadata::DiagnosticMeta;
use mossignal::signal::{Level, LogicLevel, Pulse};
use mossignal::time::Time;
use mossignal::{
    ExactlyDependency, ExactlyExplanation, KeyedModuleInput, ModuleBuilder, ModuleOrigin,
    NetworkBuilder, RuntimePolicy, StandardCatalogue, StandardModuleExpansionVersion,
    StandardModuleId, StandardModuleRef, StandardModuleRequest, StandardModuleSemanticVersion,
    StandardParameterKey, StandardParameterValue, TimeDomainId, Transaction, exactly_result_key,
};

fn policy() -> RuntimePolicy {
    RuntimePolicy::builder()
        .max_internal_reactions(100)
        .max_evaluated_operations(100_000)
        .max_pending_events(100)
        .max_events_created_per_transaction(100)
        .max_required_provenance_growth(100_000)
        .build()
        .unwrap()
}

fn exactly_request(
    threshold: u64,
    inputs: impl IntoIterator<Item = ModuleInputKey<Level>>,
) -> StandardModuleRequest<()> {
    let mut request = StandardModuleRequest::new(StandardModuleRef::exactly()).with_parameter(
        StandardParameterKey::threshold(),
        StandardParameterValue::U64(threshold),
    );
    for input in inputs {
        request = request.with_variadic_input(input.into());
    }
    request
}

fn diagnostic_codes(
    report: &mossignal::diagnostics::Report<mossignal::ModuleDef<()>, ()>,
) -> Vec<DiagnosticCode> {
    report
        .diagnostics()
        .iter()
        .map(|diagnostic| diagnostic.problem().code())
        .collect()
}

#[test]
fn catalogue_discovery_and_dynamic_failures_are_structured() {
    let catalogue = StandardCatalogue::<()>::current();
    assert_eq!(catalogue.version().get(), 1);
    let descriptors = catalogue.descriptors().collect::<Vec<_>>();
    assert_eq!(descriptors.len(), 1);
    let descriptor = descriptors[0];
    assert_eq!(descriptor.module_ref(), &StandardModuleRef::exactly());
    assert_eq!(descriptor.display_name(), "Exactly");
    assert_eq!(descriptor.inputs().len(), 1);
    assert!(descriptor.inputs()[0].is_variadic());
    assert_eq!(descriptor.outputs().len(), 1);
    assert_eq!(
        descriptor.outputs()[0].fixed_output(),
        Some(exactly_result_key().into())
    );
    assert!(!descriptor.is_stateful());
    assert!(!descriptor.is_temporal());
    assert_eq!(descriptor.parameters().len(), 1);
    assert!(descriptor.parameters()[0].is_required());
    assert_eq!(
        catalogue
            .latest(StandardModuleRef::exactly().id())
            .unwrap()
            .module_ref(),
        &StandardModuleRef::exactly()
    );

    let unknown = StandardModuleRef::new(
        StandardModuleId::new("mossignal.standard.unknown").unwrap(),
        StandardModuleSemanticVersion::new(1).unwrap(),
        StandardModuleExpansionVersion::new(1).unwrap(),
    );
    assert!(matches!(
        catalogue.descriptor(&unknown),
        Err(mossignal::CatalogueFailure::UnknownId(_))
    ));

    let unsupported = StandardModuleRef::new(
        StandardModuleRef::exactly().id().clone(),
        StandardModuleSemanticVersion::new(2).unwrap(),
        StandardModuleExpansionVersion::new(1).unwrap(),
    );
    assert!(matches!(
        catalogue.descriptor(&unsupported),
        Err(mossignal::CatalogueFailure::UnsupportedVersion(_))
    ));

    let missing = catalogue.build(StandardModuleRequest::new(StandardModuleRef::exactly()));
    assert!(missing.artifact().is_none());
    assert_eq!(
        diagnostic_codes(&missing),
        [DiagnosticCode::StandardModuleMissingParameter]
    );

    let wrong_kind = catalogue.build(
        StandardModuleRequest::new(StandardModuleRef::exactly()).with_parameter(
            StandardParameterKey::threshold(),
            StandardParameterValue::LogicLevel(LogicLevel::High),
        ),
    );
    assert!(wrong_kind.artifact().is_none());
    assert!(
        diagnostic_codes(&wrong_kind)
            .contains(&DiagnosticCode::StandardModuleParameterKindMismatch)
    );

    let wrong_interface = catalogue.build(
        exactly_request(0, []).with_variadic_input(ModuleInputKey::<Pulse>::from_u128(1).into()),
    );
    assert!(wrong_interface.artifact().is_none());
    assert!(
        diagnostic_codes(&wrong_interface)
            .contains(&DiagnosticCode::StandardModuleInterfaceMismatch)
    );
}

#[test]
fn declarations_canonicalize_inputs_and_distinguish_semantics() {
    let catalogue = StandardCatalogue::<()>::current();
    let keys = [
        ModuleInputKey::<Level>::from_u128(30),
        ModuleInputKey::<Level>::from_u128(10),
        ModuleInputKey::<Level>::from_u128(20),
    ];
    let reordered = catalogue
        .build(exactly_request(2, keys))
        .require_artifact()
        .unwrap();
    let canonical = catalogue
        .build(exactly_request(2, [keys[1], keys[2], keys[0]]))
        .require_artifact()
        .unwrap();
    let changed = catalogue
        .build(exactly_request(1, [keys[1], keys[2], keys[0]]))
        .require_artifact()
        .unwrap();

    assert_eq!(reordered.fingerprint(), canonical.fingerprint());
    assert_eq!(
        reordered
            .standard_declaration()
            .unwrap()
            .expansion_fingerprint(),
        canonical
            .standard_declaration()
            .unwrap()
            .expansion_fingerprint()
    );
    assert_eq!(
        canonical
            .standard_declaration()
            .unwrap()
            .variadic_inputs()
            .collect::<Vec<_>>(),
        [keys[1], keys[2], keys[0]]
    );
    assert_ne!(canonical.fingerprint(), changed.fingerprint());

    let unary_key = ModuleInputKey::<Level>::from_u128(40);
    let standard_unary = catalogue
        .build(exactly_request(1, [unary_key]))
        .require_artifact()
        .unwrap();
    let mut user_builder = ModuleBuilder::<()>::new();
    let user_input = user_builder
        .add_level_input(unary_key, DiagnosticMeta::default())
        .unwrap();
    user_builder
        .add_level_output(exactly_result_key(), user_input, DiagnosticMeta::default())
        .unwrap();
    let user_unary = user_builder.finish().require_artifact().unwrap();
    assert!(matches!(standard_unary.origin(), ModuleOrigin::Standard(_)));
    assert!(matches!(user_unary.origin(), ModuleOrigin::User));
    assert_ne!(standard_unary.fingerprint(), user_unary.fingerprint());

    let declaration = canonical.standard_declaration().unwrap();
    let roles = declaration.internal_roles().collect::<Vec<_>>();
    for (index, role) in roles.iter().enumerate() {
        assert!(
            !roles[..index]
                .iter()
                .any(|other| other.category() == role.category() && other.key() == role.key())
        );
    }
}

#[test]
fn canonical_expansion_uses_the_fixed_case_table() {
    let key = |value| ModuleInputKey::<Level>::from_u128(value);
    let cases = [
        (1, vec![], 1, vec!["Constant"]),
        (0, vec![], 1, vec!["Constant"]),
        (0, vec![key(1)], 1, vec!["Not"]),
        (0, vec![key(1), key(2)], 2, vec!["Any", "Not"]),
        (1, vec![key(1)], 0, vec![]),
        (2, vec![key(1), key(2)], 1, vec!["All"]),
        (
            2,
            vec![key(1), key(2), key(3)],
            4,
            vec!["All", "AtLeast", "AtLeast", "Not"],
        ),
    ];
    for (threshold, inputs, expected_count, mut expected_kinds) in cases {
        let module = StandardCatalogue::<()>::current()
            .build(exactly_request(threshold, inputs))
            .require_artifact()
            .unwrap();
        let mut actual = module
            .graph()
            .nodes()
            .iter()
            .map(|node| match node.kind() {
                NodeKind::Constant(_) => "Constant",
                NodeKind::Not => "Not",
                NodeKind::All => "All",
                NodeKind::Any => "Any",
                NodeKind::AtLeast(_) => "AtLeast",
                other => panic!("unexpected Exactly primitive: {other:?}"),
            })
            .collect::<Vec<_>>();
        actual.sort_unstable();
        expected_kinds.sort_unstable();
        assert_eq!(actual.len(), expected_count);
        assert_eq!(actual, expected_kinds);
    }
}

#[test]
fn exactly_executes_all_boundary_truth_tables_and_exposes_inspection() {
    for arity in 0..=5_u128 {
        let mut thresholds = vec![0, 1, arity.saturating_sub(1) as u64, arity as u64];
        thresholds.push(arity as u64 + 1);
        thresholds.push(u64::MAX);
        thresholds.sort_unstable();
        thresholds.dedup();
        for threshold in thresholds {
            for mask in 0..(1_u128 << arity) {
                let mut network = NetworkBuilder::<()>::new(TimeDomainId::from_u128(
                    1_000 + arity * 100 + threshold as u128 % 97,
                ));
                let mut external_keys = Vec::new();
                let mut bindings = Vec::new();
                for index in 0..arity {
                    let external = ExternalInputKey::<Level>::from_u128(10 + index);
                    let source = network
                        .add_level_input(external, DiagnosticMeta::default())
                        .unwrap();
                    external_keys.push(external);
                    bindings.push(KeyedModuleInput {
                        key: ModuleInputKey::<Level>::from_u128(100 + index),
                        source,
                    });
                }
                let instance = ModuleInstanceKey::from_u128(500);
                let added = network
                    .add_exactly(instance, threshold, bindings, DiagnosticMeta::default())
                    .unwrap();
                assert_eq!(added.module_ref(), &StandardModuleRef::exactly());
                assert_eq!(added.key(), instance);
                let output = ExternalOutputKey::<Level>::from_u128(600);
                network
                    .add_level_output(output, *added.outputs(), DiagnosticMeta::default())
                    .unwrap();
                let compiled = network
                    .finish()
                    .require_artifact()
                    .unwrap()
                    .compile_ref()
                    .require_artifact()
                    .unwrap();
                let mut snapshot = compiled.input_snapshot();
                for (index, external) in external_keys.iter().copied().enumerate() {
                    let level = if mask & (1 << index) == 0 {
                        LogicLevel::Low
                    } else {
                        LogicLevel::High
                    };
                    snapshot = snapshot.set(external, level).unwrap();
                }
                let mut machine = compiled.spawn(policy());
                machine
                    .apply(Transaction::initialize(
                        Time::from_ticks(0),
                        machine.revision(),
                        snapshot.finish().unwrap(),
                    ))
                    .unwrap();
                let expected = if mask.count_ones() as u64 == threshold {
                    LogicLevel::High
                } else {
                    LogicLevel::Low
                };
                assert_eq!(machine.output_level(output), Some(expected));
                let inspection = machine.inspect_module(instance).unwrap();
                assert!(inspection.standard_declaration().is_some());
                let exactly = inspection.exactly().unwrap();
                assert_eq!(exactly.threshold(), threshold);
                assert_eq!(exactly.arity(), arity as usize);
                assert_eq!(exactly.high_count(), mask.count_ones() as usize);
                assert_eq!(exactly.result(), expected);
                assert_eq!(
                    inspection
                        .standard_declaration()
                        .unwrap()
                        .public_dependencies()
                        .len(),
                    if arity == 0 || threshold > arity as u64 {
                        0
                    } else {
                        arity as usize
                    }
                );
                assert_eq!(
                    exactly.dependency(),
                    if arity == 0 || threshold > arity as u64 {
                        ExactlyDependency::Constant
                    } else {
                        ExactlyDependency::EveryInput
                    }
                );
                if expected.is_high() {
                    assert_eq!(
                        exactly.explanation(),
                        ExactlyExplanation::Matched {
                            high_contributors: exactly.high_inputs().to_vec(),
                            low_non_contributors: exactly.low_inputs().to_vec(),
                        }
                    );
                }
                assert!(
                    inspection
                        .nodes()
                        .iter()
                        .all(|node| node.standard_role().is_some())
                );
            }
        }
    }
}

#[test]
fn duplicate_sources_warn_and_exactly_nests_inside_user_modules() {
    let input = ExternalInputKey::<Level>::from_u128(1);
    let mut network = NetworkBuilder::<()>::new(TimeDomainId::from_u128(2));
    let source = network
        .add_level_input(input, DiagnosticMeta::default())
        .unwrap();
    network
        .add_exactly(
            ModuleInstanceKey::from_u128(3),
            1,
            [
                KeyedModuleInput {
                    key: ModuleInputKey::<Level>::from_u128(10),
                    source,
                },
                KeyedModuleInput {
                    key: ModuleInputKey::<Level>::from_u128(11),
                    source,
                },
            ],
            DiagnosticMeta::default(),
        )
        .unwrap();
    let report = network.finish();
    assert!(report.artifact().is_some());
    assert!(report.diagnostics().iter().any(|diagnostic| {
        diagnostic.problem().code() == DiagnosticCode::StandardModuleDuplicateSource
    }));

    let public_input = ModuleInputKey::<Level>::from_u128(20);
    let public_output = ModuleOutputKey::<Level>::from_u128(21);
    let mut module = ModuleBuilder::<()>::new();
    let source = module
        .add_level_input(public_input, DiagnosticMeta::default())
        .unwrap();
    let result = module.exactly(1, [source]).unwrap();
    module
        .add_level_output(public_output, result, DiagnosticMeta::default())
        .unwrap();
    let module = module.finish().require_artifact().unwrap();
    assert_eq!(module.graph().module_instances().len(), 1);
    assert!(
        module.graph().module_instances()[0]
            .module()
            .standard_declaration()
            .is_some()
    );

    let mut outer = NetworkBuilder::<()>::new(TimeDomainId::from_u128(30));
    let outer_input = ExternalInputKey::<Level>::from_u128(31);
    let outer_output = ExternalOutputKey::<Level>::from_u128(32);
    let outer_source = outer
        .add_level_input(outer_input, DiagnosticMeta::default())
        .unwrap();
    let added = outer
        .instantiate(
            &module,
            ModuleInstanceKey::from_u128(33),
            DiagnosticMeta::default(),
        )
        .unwrap()
        .bind_level(public_input, outer_source)
        .unwrap()
        .finish()
        .unwrap();
    outer
        .add_level_output(
            outer_output,
            added.level_output(public_output).unwrap(),
            DiagnosticMeta::default(),
        )
        .unwrap();
    let compiled = outer
        .finish()
        .require_artifact()
        .unwrap()
        .compile_ref()
        .require_artifact()
        .unwrap();
    let snapshot = compiled
        .input_snapshot()
        .set(outer_input, LogicLevel::High)
        .and_then(mossignal::InputSnapshotBuilder::finish)
        .unwrap();
    let mut machine = compiled.spawn(policy());
    machine
        .apply(Transaction::initialize(
            Time::from_ticks(0),
            machine.revision(),
            snapshot,
        ))
        .unwrap();
    assert_eq!(machine.output_level(outer_output), Some(LogicLevel::High));
}

#[test]
fn exactly_result_key_is_fixed_for_the_descriptor() {
    assert_eq!(exactly_result_key(), exactly_result_key());
}
