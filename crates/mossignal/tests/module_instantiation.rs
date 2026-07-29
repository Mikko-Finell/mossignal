use mossignal::authored::{
    ConnectionDef, ConnectionEndpoint, ExternalInputDef, ExternalOutputDef, ModuleBinding,
    ModuleBindingSet, ModuleInstanceDef, NodeDef, NodeKind, NodePorts, UncheckedNetwork,
};
use mossignal::diagnostics::{DiagnosticCode, ProblemEvidence};
use mossignal::key::{
    AnySignalSourceKey, ConnectionKey, ExternalInputKey, ExternalOutputKey, InPortKey,
    ModuleInputKey, ModuleInstanceKey, ModuleOutputKey, NetworkKey, NodeKey, OutPortKey,
    SignalSourceKey,
};
use mossignal::metadata::DiagnosticMeta;
use mossignal::signal::{Level, Pulse};
use mossignal::{ModuleBuilder, ModuleDef, NetworkBuilder, TimeDomainId};

fn inverter_module() -> (ModuleDef<()>, ModuleInputKey<Level>, ModuleOutputKey<Level>) {
    let mut builder = ModuleBuilder::<()>::new();
    let input = ModuleInputKey::from_u128(1);
    let output = ModuleOutputKey::from_u128(2);
    let signal = builder
        .add_level_input(input, DiagnosticMeta::default())
        .unwrap();
    let inverted = builder.not(signal).unwrap();
    builder
        .add_level_output(output, inverted, DiagnosticMeta::default())
        .unwrap();
    (builder.finish().require_artifact().unwrap(), input, output)
}

#[test]
fn typed_instantiation_retains_exact_output_identity_and_staged_compile_boundary() {
    let (module, input, output) = inverter_module();
    let instance = ModuleInstanceKey::from_u128(10);
    let external_input = ExternalInputKey::from_u128(20);
    let external_output = ExternalOutputKey::from_u128(30);
    let mut builder =
        NetworkBuilder::<()>::with_key(NetworkKey::from_u128(40), TimeDomainId::from_u128(50));
    let source = builder
        .add_level_input(external_input, DiagnosticMeta::default())
        .unwrap();
    let added = builder
        .instantiate(&module, instance, DiagnosticMeta::default())
        .unwrap()
        .bind_level(input, source)
        .unwrap()
        .finish()
        .unwrap();
    let result = added.level_output(output).unwrap();
    assert_eq!(
        result.source_key(),
        SignalSourceKey::ModuleOutput { instance, output }
    );
    builder
        .add_level_output(external_output, result, DiagnosticMeta::default())
        .unwrap();

    let validated = builder.finish().require_artifact().unwrap();
    assert_eq!(validated.graph().module_instances().len(), 1);
    let compile = validated.compile_ref();
    assert!(compile.artifact().is_none());
    assert_eq!(compile.diagnostics().len(), 1);
    let finding = compile.diagnostics().iter().next().unwrap();
    assert_eq!(
        finding.problem().code(),
        DiagnosticCode::CompilationUnsupportedModuleInstances
    );
    assert!(matches!(
        finding.problem().evidence(),
        ProblemEvidence::CompilationUnsupportedModuleInstances { instances, .. }
            if instances == &[instance]
    ));
}

#[test]
fn rejected_typed_instantiation_never_commits_a_partial_instance() {
    let (module, input, _) = inverter_module();
    let instance = ModuleInstanceKey::from_u128(10);
    let mut builder = NetworkBuilder::<()>::new(TimeDomainId::from_u128(1));
    assert!(
        builder
            .instantiate(&module, instance, DiagnosticMeta::default())
            .unwrap()
            .finish()
            .is_err()
    );
    let source = builder.level_input("input").1;
    assert!(
        builder
            .instantiate(&module, instance, DiagnosticMeta::default())
            .unwrap()
            .bind_level(input, source)
            .unwrap()
            .finish()
            .is_ok()
    );
    assert_eq!(builder.into_unchecked().module_instances().len(), 1);
}

#[test]
fn typed_binding_membership_scope_and_duplicate_identity_fail_structurally() {
    let (module, input, output) = inverter_module();
    let instance = ModuleInstanceKey::from_u128(10);
    let mut foreign_builder = NetworkBuilder::<()>::new(TimeDomainId::from_u128(1));
    let foreign = foreign_builder.level_input("foreign").1;
    let mut builder = NetworkBuilder::<()>::new(TimeDomainId::from_u128(1));
    let local = builder.level_input("local").1;

    assert!(
        builder
            .instantiate(&module, instance, DiagnosticMeta::default())
            .unwrap()
            .bind_level(input, foreign)
            .is_err()
    );
    assert!(
        builder
            .instantiate(&module, instance, DiagnosticMeta::default())
            .unwrap()
            .bind_level(ModuleInputKey::from_u128(999), local)
            .is_err()
    );
    assert!(
        builder
            .instantiate(&module, instance, DiagnosticMeta::default())
            .unwrap()
            .bind_level(input, local)
            .unwrap()
            .bind_level(input, local)
            .is_err()
    );

    let added = builder
        .instantiate(&module, instance, DiagnosticMeta::default())
        .unwrap()
        .bind_level(input, local)
        .unwrap()
        .finish()
        .unwrap();
    assert!(added.level_output(ModuleOutputKey::from_u128(999)).is_err());
    assert!(added.level_output(output).is_ok());
    assert!(
        builder
            .instantiate(&module, instance, DiagnosticMeta::default())
            .is_err()
    );
}

#[test]
fn typed_and_dynamic_instance_claims_are_canonically_equivalent() {
    let (module, input, output) = inverter_module();
    let network = NetworkKey::from_u128(1);
    let domain = TimeDomainId::from_u128(2);
    let external_input = ExternalInputKey::from_u128(3);
    let external_output = ExternalOutputKey::from_u128(4);
    let instance = ModuleInstanceKey::from_u128(5);

    let mut typed = NetworkBuilder::<()>::with_key(network, domain);
    let source = typed
        .add_level_input(external_input, DiagnosticMeta::default())
        .unwrap();
    let added = typed
        .instantiate(&module, instance, DiagnosticMeta::default())
        .unwrap()
        .bind_level(input, source)
        .unwrap()
        .finish()
        .unwrap();
    typed
        .add_level_output(
            external_output,
            added.level_output(output).unwrap(),
            DiagnosticMeta::default(),
        )
        .unwrap();
    let typed = typed.into_unchecked();

    let dynamic = UncheckedNetwork::new_with_instances(
        network,
        domain,
        DiagnosticMeta::default(),
        vec![],
        vec![ExternalInputDef::new(
            external_input.into(),
            DiagnosticMeta::default(),
        )],
        vec![ExternalOutputDef::new(
            external_output.into(),
            AnySignalSourceKey::Level(SignalSourceKey::ModuleOutput { instance, output }),
            DiagnosticMeta::default(),
        )],
        vec![],
        vec![ModuleInstanceDef::new(
            instance,
            module,
            ModuleBindingSet::new(vec![ModuleBinding::new(
                input.into(),
                ConnectionEndpoint::external_input(external_input.into()),
            )]),
            None,
            DiagnosticMeta::default(),
        )],
    );
    assert_eq!(typed, dynamic);
    let typed = typed.validate().require_artifact().unwrap();
    let dynamic = dynamic.validate().require_artifact().unwrap();
    assert_eq!(typed.fingerprint(), dynamic.fingerprint());
}

#[test]
fn module_builder_retains_nested_instances_and_public_dependency() {
    let (inner, inner_input, inner_output) = inverter_module();
    let nested = ModuleInstanceKey::from_u128(10);
    let mut outer = ModuleBuilder::<()>::new();
    let (outer_input, source) = outer.level_input("input");
    let added = outer
        .instantiate(&inner, nested, DiagnosticMeta::default())
        .unwrap()
        .bind_level(inner_input, source)
        .unwrap()
        .finish()
        .unwrap();
    let outer_output = outer
        .level_output("output", added.level_output(inner_output).unwrap())
        .unwrap();
    let outer = outer.finish().require_artifact().unwrap();
    assert_eq!(outer.graph().module_instances().len(), 1);
    assert_eq!(outer.graph().module_instances()[0].key(), nested);

    let outer_instance = ModuleInstanceKey::from_u128(20);
    let mut network = NetworkBuilder::<()>::new(TimeDomainId::from_u128(1));
    let source = network.level_input("input").1;
    let output = network
        .instantiate(&outer, outer_instance, DiagnosticMeta::default())
        .unwrap()
        .bind_level(outer_input, source)
        .unwrap()
        .finish()
        .unwrap()
        .level_output(outer_output)
        .unwrap();
    network.level_output("output", output).unwrap();
    let network = network.finish().require_artifact().unwrap();
    assert!(
        network
            .graph()
            .qualified_nodes()
            .iter()
            .any(|node| { node.instances() == [outer_instance, nested] })
    );
}

#[test]
fn malformed_bindings_and_hierarchy_are_blocking() {
    let (module, _, _) = inverter_module();
    let first = ModuleInstanceKey::from_u128(1);
    let second = ModuleInstanceKey::from_u128(2);
    let definition = UncheckedNetwork::new_with_instances(
        NetworkKey::from_u128(3),
        TimeDomainId::from_u128(4),
        DiagnosticMeta::default(),
        vec![],
        vec![],
        vec![],
        vec![],
        vec![
            ModuleInstanceDef::new(
                first,
                module.clone(),
                ModuleBindingSet::default(),
                Some(second),
                DiagnosticMeta::default(),
            ),
            ModuleInstanceDef::new(
                second,
                module.clone(),
                ModuleBindingSet::default(),
                Some(first),
                DiagnosticMeta::default(),
            ),
            ModuleInstanceDef::new(
                ModuleInstanceKey::from_u128(3),
                module.clone(),
                ModuleBindingSet::default(),
                Some(ModuleInstanceKey::from_u128(999)),
                DiagnosticMeta::default(),
            ),
            ModuleInstanceDef::new(
                first,
                module,
                ModuleBindingSet::default(),
                None,
                DiagnosticMeta::default(),
            ),
        ],
    );
    let report = definition.validate();
    assert!(report.artifact().is_none());
    let codes: Vec<_> = report
        .diagnostics()
        .iter()
        .map(|finding| finding.problem().code())
        .collect();
    assert!(codes.contains(&DiagnosticCode::ValidationInvalidModuleBinding));
    assert!(codes.contains(&DiagnosticCode::ValidationHierarchyCycle));
    assert!(codes.contains(&DiagnosticCode::ValidationMalformedHierarchy));
    assert!(codes.contains(&DiagnosticCode::ValidationDuplicateKey));
}

#[test]
fn dynamic_binding_validation_rejects_unknown_wrong_kind_direction_and_dangling_sources() {
    let (module, input, _) = inverter_module();
    let instance = ModuleInstanceKey::from_u128(1);
    let pulse = ExternalInputKey::<Pulse>::from_u128(2);
    let dangling = OutPortKey::<Level>::from_u128(3);
    let direction_invalid = InPortKey::<Level>::from_u128(4);
    let definition = UncheckedNetwork::new_with_instances(
        NetworkKey::from_u128(5),
        TimeDomainId::from_u128(6),
        DiagnosticMeta::default(),
        vec![],
        vec![ExternalInputDef::new(
            pulse.into(),
            DiagnosticMeta::default(),
        )],
        vec![],
        vec![],
        vec![ModuleInstanceDef::new(
            instance,
            module,
            ModuleBindingSet::new(vec![
                ModuleBinding::new(
                    input.into(),
                    ConnectionEndpoint::external_input(pulse.into()),
                ),
                ModuleBinding::new(
                    ModuleInputKey::<Level>::from_u128(999).into(),
                    ConnectionEndpoint::node_output(dangling.into()),
                ),
                ModuleBinding::new(
                    input.into(),
                    ConnectionEndpoint::node_input(direction_invalid.into()),
                ),
            ]),
            None,
            DiagnosticMeta::default(),
        )],
    );
    let report = definition.validate();
    assert!(report.artifact().is_none());
    assert!(report.diagnostics().iter().any(|finding| {
        finding.problem().code() == DiagnosticCode::ValidationInvalidModuleBinding
    }));
}

#[test]
fn current_reaction_cycles_cross_module_boundaries() {
    let (module, input, output) = inverter_module();
    let instance = ModuleInstanceKey::from_u128(1);
    let node = NodeKey::from_u128(2);
    let node_input = InPortKey::<Level>::from_u128(3);
    let node_output = OutPortKey::<Level>::from_u128(4);
    let definition = UncheckedNetwork::new_with_instances(
        NetworkKey::from_u128(5),
        TimeDomainId::from_u128(6),
        DiagnosticMeta::default(),
        vec![NodeDef::new(
            node,
            NodeKind::not(),
            NodePorts::new(vec![node_input.into()], vec![node_output.into()]),
            DiagnosticMeta::default(),
        )],
        vec![],
        vec![],
        vec![ConnectionDef::new(
            ConnectionKey::from_u128(7),
            ConnectionEndpoint::module_output(instance, output.into()),
            ConnectionEndpoint::node_input(node_input.into()),
            DiagnosticMeta::default(),
        )],
        vec![ModuleInstanceDef::new(
            instance,
            module,
            ModuleBindingSet::new(vec![ModuleBinding::new(
                input.into(),
                ConnectionEndpoint::node_output(node_output.into()),
            )]),
            None,
            DiagnosticMeta::default(),
        )],
    );
    let report = definition.validate();
    assert!(report.artifact().is_none());
    assert!(report.diagnostics().iter().any(|finding| {
        finding.problem().code() == DiagnosticCode::ValidationCurrentReactionCycle
    }));
}

#[test]
fn pulse_and_zero_input_instances_use_the_same_generic_path() {
    let mut pulse_module = ModuleBuilder::<()>::new();
    let (pulse_input, pulse) = pulse_module.pulse_input("pulse");
    let merged = pulse_module.merge([pulse]).unwrap();
    let pulse_output = pulse_module.pulse_output("pulse", merged).unwrap();
    let pulse_module = pulse_module.finish().require_artifact().unwrap();

    let mut constant_module = ModuleBuilder::<()>::new();
    let constant = constant_module.constant(mossignal::signal::LogicLevel::High);
    let constant_output = constant_module.level_output("level", constant).unwrap();
    let constant_module = constant_module.finish().require_artifact().unwrap();

    let mut network = NetworkBuilder::<()>::new(TimeDomainId::from_u128(1));
    let pulse = network.pulse_input("pulse").1;
    let pulse_instance = network
        .instantiate(
            &pulse_module,
            ModuleInstanceKey::from_u128(1),
            DiagnosticMeta::default(),
        )
        .unwrap()
        .bind_pulse(pulse_input, pulse)
        .unwrap()
        .finish()
        .unwrap();
    assert!(pulse_instance.pulse_output(pulse_output).is_ok());
    let constant_instance = network
        .instantiate(
            &constant_module,
            ModuleInstanceKey::from_u128(2),
            DiagnosticMeta::default(),
        )
        .unwrap()
        .finish()
        .unwrap();
    assert!(constant_instance.level_output(constant_output).is_ok());
    assert!(network.finish().artifact().is_some());
}

#[test]
fn instance_fingerprints_ignore_claim_order_and_retain_semantic_identity() {
    let mut module = ModuleBuilder::<()>::new();
    let constant = module.constant(mossignal::signal::LogicLevel::High);
    module.level_output("level", constant).unwrap();
    let module = module.finish().require_artifact().unwrap();
    let instance = |key| {
        ModuleInstanceDef::new(
            ModuleInstanceKey::from_u128(key),
            module.clone(),
            ModuleBindingSet::default(),
            None,
            DiagnosticMeta::default(),
        )
    };
    let definition = |instances| {
        UncheckedNetwork::new_with_instances(
            NetworkKey::from_u128(1),
            TimeDomainId::from_u128(2),
            DiagnosticMeta::default(),
            vec![],
            vec![],
            vec![],
            vec![],
            instances,
        )
    };
    let forward = definition(vec![instance(10), instance(20)])
        .validate()
        .require_artifact()
        .unwrap();
    let reverse = definition(vec![instance(20), instance(10)])
        .validate()
        .require_artifact()
        .unwrap();
    let changed = definition(vec![instance(10), instance(21)])
        .validate()
        .require_artifact()
        .unwrap();
    assert_eq!(forward.fingerprint(), reverse.fingerprint());
    assert_ne!(forward.fingerprint(), changed.fingerprint());
}
