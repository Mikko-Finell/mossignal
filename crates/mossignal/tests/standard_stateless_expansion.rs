use mossignal::authored::NodeKind;
use mossignal::diagnostics::DiagnosticCode;
use mossignal::key::{
    ExternalInputKey, ExternalOutputKey, ModuleInputKey, ModuleInstanceKey, ModuleOutputKey,
};
use mossignal::metadata::DiagnosticMeta;
use mossignal::signal::{Level, LogicLevel, Pulse};
use mossignal::time::Time;
use mossignal::{
    AllEqualDependency, AllEqualExplanation, AtMostDependency, AtMostExplanation, KeyedModuleInput,
    ModuleBuilder, NetworkBuilder, RuntimePolicy, StandardCatalogue, StandardInternalCategory,
    StandardModuleRef, StandardModuleRequest, StandardParameterKey, StandardParameterValue,
    TimeDomainId, Transaction, all_equal_result_key, at_most_result_key,
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

fn request(
    module_ref: StandardModuleRef,
    threshold: Option<u64>,
    inputs: impl IntoIterator<Item = ModuleInputKey<Level>>,
) -> StandardModuleRequest<()> {
    let mut request = StandardModuleRequest::new(module_ref);
    if let Some(threshold) = threshold {
        request = request.with_parameter(
            StandardParameterKey::threshold(),
            StandardParameterValue::U64(threshold),
        );
    }
    for input in inputs {
        request = request.with_variadic_input(input.into());
    }
    request
}

fn codes(
    report: &mossignal::diagnostics::Report<mossignal::ModuleDef<()>, ()>,
) -> Vec<DiagnosticCode> {
    report
        .diagnostics()
        .iter()
        .map(|diagnostic| diagnostic.problem().code())
        .collect()
}

fn node_kinds(module: &mossignal::ModuleDef<()>) -> Vec<String> {
    let mut kinds = module
        .graph()
        .nodes()
        .iter()
        .map(|node| match node.kind() {
            NodeKind::Constant(config) => format!("Constant({:?})", config.value()),
            NodeKind::Not => "Not".to_owned(),
            NodeKind::All => "All".to_owned(),
            NodeKind::Any => "Any".to_owned(),
            NodeKind::AtLeast(config) => format!("AtLeast({})", config.threshold),
            other => panic!("unexpected stateless standard primitive: {other:?}"),
        })
        .collect::<Vec<_>>();
    kinds.sort();
    kinds
}

#[test]
fn catalogue_discovers_all_stateless_descriptors_and_validates_requests() {
    let catalogue = StandardCatalogue::<()>::current();
    let descriptors = catalogue.descriptors().collect::<Vec<_>>();
    assert_eq!(
        descriptors
            .iter()
            .map(|descriptor| descriptor.module_ref().clone())
            .collect::<Vec<_>>(),
        [
            StandardModuleRef::exactly(),
            StandardModuleRef::at_most(),
            StandardModuleRef::all_equal(),
        ]
    );

    let at_most = catalogue.descriptor(&StandardModuleRef::at_most()).unwrap();
    assert_eq!(at_most.display_name(), "AtMost");
    assert_eq!(at_most.parameters().len(), 1);
    assert_eq!(
        at_most.outputs()[0].fixed_output(),
        Some(at_most_result_key().into())
    );
    let all_equal = catalogue
        .descriptor(&StandardModuleRef::all_equal())
        .unwrap();
    assert_eq!(all_equal.display_name(), "AllEqual");
    assert!(all_equal.parameters().is_empty());
    assert_eq!(
        all_equal.outputs()[0].fixed_output(),
        Some(all_equal_result_key().into())
    );

    let malformed_at_most = catalogue.build(
        StandardModuleRequest::new(StandardModuleRef::at_most())
            .with_parameter(
                StandardParameterKey::threshold(),
                StandardParameterValue::LogicLevel(LogicLevel::High),
            )
            .with_parameter(
                StandardParameterKey::new("unknown"),
                StandardParameterValue::U64(1),
            )
            .with_variadic_input(ModuleInputKey::<Pulse>::from_u128(9).into()),
    );
    assert!(malformed_at_most.artifact().is_none());
    let malformed_codes = codes(&malformed_at_most);
    assert!(malformed_codes.contains(&DiagnosticCode::StandardModuleParameterKindMismatch));
    assert!(malformed_codes.contains(&DiagnosticCode::StandardModuleUnexpectedParameter));
    assert!(malformed_codes.contains(&DiagnosticCode::StandardModuleMissingParameter));
    assert!(malformed_codes.contains(&DiagnosticCode::StandardModuleInterfaceMismatch));

    let malformed_all_equal = catalogue.build(
        request(StandardModuleRef::all_equal(), None, []).with_parameter(
            StandardParameterKey::threshold(),
            StandardParameterValue::U64(0),
        ),
    );
    assert!(malformed_all_equal.artifact().is_none());
    assert_eq!(
        codes(&malformed_all_equal),
        [DiagnosticCode::StandardModuleUnexpectedParameter]
    );
}

#[test]
fn canonical_case_tables_and_stable_identity_are_exact() {
    let key = |value| ModuleInputKey::<Level>::from_u128(value);
    let catalogue = StandardCatalogue::<()>::current();
    let cases = [
        (
            request(StandardModuleRef::at_most(), Some(0), []),
            vec!["Constant(High)"],
        ),
        (
            request(StandardModuleRef::at_most(), Some(0), [key(1)]),
            vec!["Not"],
        ),
        (
            request(StandardModuleRef::at_most(), Some(0), [key(1), key(2)]),
            vec!["Any", "Not"],
        ),
        (
            request(
                StandardModuleRef::at_most(),
                Some(1),
                [key(1), key(2), key(3)],
            ),
            vec!["AtLeast(2)", "Not"],
        ),
        (
            request(StandardModuleRef::all_equal(), None, []),
            vec!["Constant(High)"],
        ),
        (
            request(StandardModuleRef::all_equal(), None, [key(1)]),
            vec!["Constant(High)"],
        ),
        (
            request(
                StandardModuleRef::all_equal(),
                None,
                [key(1), key(2), key(3)],
            ),
            vec!["All", "Any", "Any", "Not"],
        ),
    ];
    for (request, expected) in cases {
        let module = catalogue.build(request).require_artifact().unwrap();
        assert_eq!(node_kinds(&module), expected);
        let roles = module
            .standard_declaration()
            .unwrap()
            .internal_roles()
            .collect::<Vec<_>>();
        for (index, role) in roles.iter().enumerate() {
            assert!(
                !roles[..index]
                    .iter()
                    .any(|other| other.category() == role.category() && other.key() == role.key())
            );
        }
        assert_eq!(
            roles
                .iter()
                .filter(|role| role.category() == StandardInternalCategory::Export)
                .count(),
            1
        );
    }

    let keys = [key(30), key(10), key(20)];
    for (module_ref, threshold) in [
        (StandardModuleRef::at_most(), Some(1)),
        (StandardModuleRef::all_equal(), None),
    ] {
        let first = catalogue
            .build(request(module_ref.clone(), threshold, keys))
            .require_artifact()
            .unwrap();
        let reordered = catalogue
            .build(request(module_ref, threshold, [keys[1], keys[2], keys[0]]))
            .require_artifact()
            .unwrap();
        assert_eq!(first.fingerprint(), reordered.fingerprint());
        assert_eq!(
            first
                .standard_declaration()
                .unwrap()
                .expansion_fingerprint(),
            reordered
                .standard_declaration()
                .unwrap()
                .expansion_fingerprint()
        );
    }
}

enum Case {
    AtMost(u64),
    AllEqual,
}

fn execute(arity: usize, mask: u64, case: Case) -> (LogicLevel, mossignal::ModuleInspection<()>) {
    let mut network = NetworkBuilder::<()>::new(TimeDomainId::from_u128(1));
    let mut external_inputs = Vec::new();
    let mut bindings = Vec::new();
    for index in 0..arity {
        let external = ExternalInputKey::<Level>::from_u128(10 + index as u128);
        let source = network
            .add_level_input(external, DiagnosticMeta::default())
            .unwrap();
        external_inputs.push(external);
        bindings.push(KeyedModuleInput {
            key: ModuleInputKey::<Level>::from_u128(100 + index as u128),
            source,
        });
    }
    let instance = ModuleInstanceKey::from_u128(500);
    let result = match case {
        Case::AtMost(threshold) => *network
            .add_at_most(instance, threshold, bindings, DiagnosticMeta::default())
            .unwrap()
            .outputs(),
        Case::AllEqual => *network
            .add_all_equal(instance, bindings, DiagnosticMeta::default())
            .unwrap()
            .outputs(),
    };
    let output = ExternalOutputKey::<Level>::from_u128(600);
    network
        .add_level_output(output, result, DiagnosticMeta::default())
        .unwrap();
    let compiled = network
        .finish()
        .require_artifact()
        .unwrap()
        .compile_ref()
        .require_artifact()
        .unwrap();
    let mut snapshot = compiled.input_snapshot();
    for (index, external) in external_inputs.into_iter().enumerate() {
        let value = if mask & (1 << index) == 0 {
            LogicLevel::Low
        } else {
            LogicLevel::High
        };
        snapshot = snapshot.set(external, value).unwrap();
    }
    let mut machine = compiled.spawn(policy());
    machine
        .apply(Transaction::initialize(
            Time::from_ticks(0),
            machine.revision(),
            snapshot.finish().unwrap(),
        ))
        .unwrap();
    (
        machine.output_level(output).unwrap(),
        machine.inspect_module(instance).unwrap(),
    )
}

#[test]
fn exhaustive_bounded_laws_execute_and_inspect_through_ordinary_modules() {
    for arity in 0_usize..=5 {
        let mut thresholds = vec![0, 1, arity.saturating_sub(1) as u64, arity as u64];
        thresholds.push(arity as u64 + 1);
        thresholds.push(u64::MAX);
        thresholds.sort_unstable();
        thresholds.dedup();
        for mask in 0..(1_u64 << arity) {
            let high_count = mask.count_ones() as u64;
            for threshold in thresholds.iter().copied() {
                let (result, inspection) = execute(arity, mask, Case::AtMost(threshold));
                let expected = if high_count <= threshold {
                    LogicLevel::High
                } else {
                    LogicLevel::Low
                };
                assert_eq!(result, expected);
                let at_most = inspection.at_most().unwrap();
                assert_eq!(at_most.threshold(), threshold);
                assert_eq!(at_most.high_count(), high_count as usize);
                assert_eq!(at_most.result(), expected);
                assert_eq!(
                    at_most.dependency(),
                    if threshold >= arity as u64 {
                        AtMostDependency::Constant
                    } else {
                        AtMostDependency::EveryInput
                    }
                );
                assert_eq!(
                    inspection
                        .standard_declaration()
                        .unwrap()
                        .public_dependencies()
                        .len(),
                    if threshold >= arity as u64 { 0 } else { arity }
                );
                if threshold >= arity as u64 {
                    assert!(matches!(
                        at_most.explanation(),
                        AtMostExplanation::ThresholdCoversAll { .. }
                    ));
                }
            }

            let (result, inspection) = execute(arity, mask, Case::AllEqual);
            let expected = if arity <= 1 || high_count == 0 || high_count == arity as u64 {
                LogicLevel::High
            } else {
                LogicLevel::Low
            };
            assert_eq!(result, expected);
            let all_equal = inspection.all_equal().unwrap();
            assert_eq!(all_equal.result(), expected);
            assert_eq!(all_equal.high_count(), high_count as usize);
            assert_eq!(
                all_equal.dependency(),
                if arity <= 1 {
                    AllEqualDependency::Constant
                } else {
                    AllEqualDependency::EveryInput
                }
            );
            if arity <= 1 {
                assert!(matches!(
                    all_equal.explanation(),
                    AllEqualExplanation::Vacuous { .. }
                ));
            } else if high_count != 0 && high_count != arity as u64 {
                assert!(matches!(
                    all_equal.explanation(),
                    AllEqualExplanation::Mixed { .. }
                ));
            }
        }
    }
}

#[test]
fn diagnostics_coexist_and_both_builders_use_generic_module_instantiation() {
    let catalogue = StandardCatalogue::<()>::current();
    let empty = catalogue.build(request(StandardModuleRef::all_equal(), None, []));
    assert!(empty.artifact().is_some());
    assert_eq!(codes(&empty), [DiagnosticCode::StandardModuleEmptyVariadic]);
    let unary = catalogue.build(request(
        StandardModuleRef::at_most(),
        Some(u64::MAX),
        [ModuleInputKey::<Level>::from_u128(1)],
    ));
    assert_eq!(
        codes(&unary),
        [DiagnosticCode::StandardModuleUnaryDegenerate]
    );
    let constant = catalogue.build(request(
        StandardModuleRef::at_most(),
        Some(2),
        [
            ModuleInputKey::<Level>::from_u128(1),
            ModuleInputKey::<Level>::from_u128(2),
        ],
    ));
    assert_eq!(
        codes(&constant),
        [DiagnosticCode::StandardModuleConstantResult]
    );

    let mut network = NetworkBuilder::<()>::new(TimeDomainId::from_u128(2));
    let source = network
        .add_level_input(
            ExternalInputKey::<Level>::from_u128(1),
            DiagnosticMeta::default(),
        )
        .unwrap();
    network
        .add_all_equal(
            ModuleInstanceKey::from_u128(2),
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

    let mut module = ModuleBuilder::<()>::new();
    let input_key = ModuleInputKey::<Level>::from_u128(20);
    let source = module
        .add_level_input(input_key, DiagnosticMeta::default())
        .unwrap();
    let at_most = module.at_most(0, [source]).unwrap();
    let all_equal = module.all_equal([source, at_most]).unwrap();
    module
        .add_level_output(
            ModuleOutputKey::<Level>::from_u128(21),
            all_equal,
            DiagnosticMeta::default(),
        )
        .unwrap();
    let module = module.finish().require_artifact().unwrap();
    assert_eq!(module.graph().module_instances().len(), 2);
    assert!(
        module
            .graph()
            .module_instances()
            .iter()
            .all(|instance| instance.module().standard_declaration().is_some())
    );
}
