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
    builder
        .add_level_output(
            ExternalOutputKey::from_u128(31),
            result,
            DiagnosticMeta::default(),
        )
        .unwrap();

    let validated = builder.finish().require_artifact().unwrap();
    assert_eq!(validated.graph().module_instances().len(), 1);
    let compile_ref = validated.compile_ref();
    assert!(compile_ref.artifact().is_none());
    assert_eq!(compile_ref.diagnostics().len(), 1);
    let finding = compile_ref.diagnostics().iter().next().unwrap();
    assert_eq!(
        finding.problem().code(),
        DiagnosticCode::CompilationUnsupportedModuleInstances
    );
    assert!(matches!(
        finding.problem().evidence(),
        ProblemEvidence::CompilationUnsupportedModuleInstances { instances, .. }
            if instances == &[instance]
    ));
    let compile = validated.compile();
    assert!(compile.artifact().is_none());
    assert_eq!(compile.diagnostics(), compile_ref.diagnostics());
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
fn mixed_nested_typed_and_dynamic_instances_are_canonically_equivalent() {
    let (inner, inner_input, inner_output) = inverter_module();
    let level_input = ModuleInputKey::<Level>::from_u128(1);
    let pulse_input = ModuleInputKey::<Pulse>::from_u128(2);
    let level_output = ModuleOutputKey::<Level>::from_u128(3);
    let pulse_output = ModuleOutputKey::<Pulse>::from_u128(4);
    let mut outer = ModuleBuilder::<()>::new();
    let level = outer
        .add_level_input(level_input, DiagnosticMeta::default())
        .unwrap();
    let pulse = outer
        .add_pulse_input(pulse_input, DiagnosticMeta::default())
        .unwrap();
    let nested = outer
        .instantiate(
            &inner,
            ModuleInstanceKey::from_u128(5),
            DiagnosticMeta::default(),
        )
        .unwrap()
        .bind_level(inner_input, level)
        .unwrap()
        .finish()
        .unwrap();
    outer
        .add_level_output(
            level_output,
            nested.level_output(inner_output).unwrap(),
            DiagnosticMeta::default(),
        )
        .unwrap();
    let merged = outer.merge([pulse]).unwrap();
    outer
        .add_pulse_output(pulse_output, merged, DiagnosticMeta::default())
        .unwrap();
    let outer = outer.finish().require_artifact().unwrap();

    let network = NetworkKey::from_u128(10);
    let domain = TimeDomainId::from_u128(11);
    let level_endpoint = ExternalInputKey::<Level>::from_u128(12);
    let pulse_endpoint = ExternalInputKey::<Pulse>::from_u128(13);
    let instance = ModuleInstanceKey::from_u128(14);
    let mut typed = NetworkBuilder::<()>::with_key(network, domain);
    let level = typed
        .add_level_input(level_endpoint, DiagnosticMeta::default())
        .unwrap();
    let pulse = typed
        .add_pulse_input(pulse_endpoint, DiagnosticMeta::default())
        .unwrap();
    let added = typed
        .instantiate(&outer, instance, DiagnosticMeta::default())
        .unwrap()
        .bind_level(level_input, level)
        .unwrap()
        .bind_pulse(pulse_input, pulse)
        .unwrap()
        .finish()
        .unwrap();
    typed
        .add_level_output(
            ExternalOutputKey::from_u128(15),
            added.level_output(level_output).unwrap(),
            DiagnosticMeta::default(),
        )
        .unwrap();
    typed
        .add_pulse_output(
            ExternalOutputKey::from_u128(16),
            added.pulse_output(pulse_output).unwrap(),
            DiagnosticMeta::default(),
        )
        .unwrap();
    let typed = typed.into_unchecked();

    let dynamic = UncheckedNetwork::new_with_instances(
        network,
        domain,
        DiagnosticMeta::default(),
        vec![],
        vec![
            ExternalInputDef::new(level_endpoint.into(), DiagnosticMeta::default()),
            ExternalInputDef::new(pulse_endpoint.into(), DiagnosticMeta::default()),
        ],
        vec![
            ExternalOutputDef::new(
                ExternalOutputKey::<Level>::from_u128(15).into(),
                SignalSourceKey::ModuleOutput {
                    instance,
                    output: level_output,
                }
                .into(),
                DiagnosticMeta::default(),
            ),
            ExternalOutputDef::new(
                ExternalOutputKey::<Pulse>::from_u128(16).into(),
                SignalSourceKey::ModuleOutput {
                    instance,
                    output: pulse_output,
                }
                .into(),
                DiagnosticMeta::default(),
            ),
        ],
        vec![],
        vec![ModuleInstanceDef::new(
            instance,
            outer,
            ModuleBindingSet::new(vec![
                ModuleBinding::new(
                    level_input.into(),
                    ConnectionEndpoint::external_input(level_endpoint.into()),
                ),
                ModuleBinding::new(
                    pulse_input.into(),
                    ConnectionEndpoint::external_input(pulse_endpoint.into()),
                ),
            ]),
            None,
            DiagnosticMeta::default(),
        )],
    );

    assert_eq!(typed, dynamic);
    let typed = typed.validate().require_artifact().unwrap();
    let dynamic = dynamic.validate().require_artifact().unwrap();
    assert_eq!(typed.fingerprint(), dynamic.fingerprint());
    assert_eq!(
        typed.graph().qualified_nodes(),
        dynamic.graph().qualified_nodes()
    );
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
    assert!(report.diagnostics().internal_defects().is_empty());
    assert!(report.diagnostics().iter().any(|finding| {
        finding.problem().code() == DiagnosticCode::ValidationInvalidModuleBinding
    }));
    assert!(report.diagnostics().iter().any(|finding| matches!(
        finding.problem().evidence(),
        ProblemEvidence::ValidationInvalidModuleBinding {
            input: found,
            sources,
            ..
        } if *found == input.into() && sources.len() == 2
    )));
}

#[test]
fn explicit_parent_hierarchy_qualifies_instance_internals() {
    let mut leaf = ModuleBuilder::<()>::new();
    let constant = leaf.constant(mossignal::signal::LogicLevel::High);
    let output = leaf.not(constant).unwrap();
    leaf.level_output("output", output).unwrap();
    let leaf = leaf.finish().require_artifact().unwrap();

    let parent = ModuleInstanceKey::from_u128(10);
    let child = ModuleInstanceKey::from_u128(20);
    let mut network = NetworkBuilder::<()>::new(TimeDomainId::from_u128(1));
    network
        .instantiate(&leaf, parent, DiagnosticMeta::default())
        .unwrap()
        .finish()
        .unwrap();
    network
        .instantiate(&leaf, child, DiagnosticMeta::default())
        .unwrap()
        .parent(parent)
        .unwrap()
        .finish()
        .unwrap();

    let validated = network.finish().require_artifact().unwrap();
    assert!(
        validated
            .graph()
            .qualified_nodes()
            .iter()
            .any(|node| { node.instances() == [parent, child] })
    );
    assert!(
        validated
            .graph()
            .qualified_connections()
            .iter()
            .any(|connection| { connection.instances() == [parent, child] })
    );
}

#[test]
fn staged_compile_evidence_includes_nested_instance_keys() {
    let mut leaf = ModuleBuilder::<()>::new();
    let output = leaf.constant(mossignal::signal::LogicLevel::High);
    let leaf_output = leaf.level_output("output", output).unwrap();
    let leaf = leaf.finish().require_artifact().unwrap();

    let nested = ModuleInstanceKey::from_u128(10);
    let mut outer = ModuleBuilder::<()>::new();
    let nested_output = outer
        .instantiate(&leaf, nested, DiagnosticMeta::default())
        .unwrap()
        .finish()
        .unwrap()
        .level_output(leaf_output)
        .unwrap();
    outer.level_output("output", nested_output).unwrap();
    let outer = outer.finish().require_artifact().unwrap();
    assert_eq!(
        outer.fingerprint().to_string(),
        "cca0626201725a0b87c438873016f180e51917ad1b59a051e15d26dcf8a841cc"
    );

    let top = ModuleInstanceKey::from_u128(20);
    let mut network = NetworkBuilder::<()>::new(TimeDomainId::from_u128(1));
    network
        .instantiate(&outer, top, DiagnosticMeta::default())
        .unwrap()
        .finish()
        .unwrap();
    let validated = network.finish().require_artifact().unwrap();
    assert_eq!(
        validated.fingerprint().to_string(),
        "d12911bb5de70e571f448759888c268d62dae3884e80d02ed708d63070f85a41"
    );
    let report = validated.compile_ref();
    let evidence = report
        .diagnostics()
        .iter()
        .next()
        .unwrap()
        .problem()
        .evidence();
    assert!(matches!(
        evidence,
        ProblemEvidence::CompilationUnsupportedModuleInstances { instances, .. }
            if instances == &[nested, top]
    ));
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
fn current_reaction_cycles_cross_several_instances() {
    let (module, input, output) = inverter_module();
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
                ModuleBindingSet::new(vec![ModuleBinding::new(
                    input.into(),
                    ConnectionEndpoint::module_output(second, output.into()),
                )]),
                None,
                DiagnosticMeta::default(),
            ),
            ModuleInstanceDef::new(
                second,
                module,
                ModuleBindingSet::new(vec![ModuleBinding::new(
                    input.into(),
                    ConnectionEndpoint::module_output(first, output.into()),
                )]),
                None,
                DiagnosticMeta::default(),
            ),
        ],
    );

    let report = definition.validate();
    assert!(report.artifact().is_none());
    assert!(report.diagnostics().iter().any(|finding| {
        finding.problem().code() == DiagnosticCode::ValidationCurrentReactionCycle
    }));
}

#[test]
fn temporal_module_projection_breaks_current_reaction_cycles() {
    let mut builder = ModuleBuilder::<()>::new();
    let (input, pulse) = builder.pulse_input("input");
    let delayed = builder
        .pulse_delay(
            pulse,
            mossignal::PulseDelayConfig::new(mossignal::time::NonZeroSpan::from_ticks(1).unwrap()),
        )
        .unwrap();
    let output = builder.pulse_output("output", delayed).unwrap();
    let module = builder.finish().require_artifact().unwrap();
    let instance = ModuleInstanceKey::from_u128(1);
    let definition = UncheckedNetwork::new_with_instances(
        NetworkKey::from_u128(2),
        TimeDomainId::from_u128(3),
        DiagnosticMeta::default(),
        vec![],
        vec![],
        vec![],
        vec![],
        vec![ModuleInstanceDef::new(
            instance,
            module,
            ModuleBindingSet::new(vec![ModuleBinding::new(
                input.into(),
                ConnectionEndpoint::module_output(instance, output.into()),
            )]),
            None,
            DiagnosticMeta::default(),
        )],
    );

    assert!(definition.validate().artifact().is_some());
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
    assert!(
        pulse_instance
            .level_output(ModuleOutputKey::from_u128(pulse_output.as_u128()))
            .is_err()
    );
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
    let metadata = definition(vec![
        ModuleInstanceDef::new(
            ModuleInstanceKey::from_u128(10),
            module.clone(),
            ModuleBindingSet::default(),
            None,
            DiagnosticMeta {
                name: Some("presentation only".into()),
                ..DiagnosticMeta::default()
            },
        ),
        instance(20),
    ])
    .validate()
    .require_artifact()
    .unwrap();
    let hierarchy = definition(vec![
        instance(10),
        ModuleInstanceDef::new(
            ModuleInstanceKey::from_u128(20),
            module.clone(),
            ModuleBindingSet::default(),
            Some(ModuleInstanceKey::from_u128(10)),
            DiagnosticMeta::default(),
        ),
    ])
    .validate()
    .require_artifact()
    .unwrap();

    let mut changed_module = ModuleBuilder::<()>::new();
    let low = changed_module.constant(mossignal::signal::LogicLevel::Low);
    changed_module.level_output("level", low).unwrap();
    let changed_module = changed_module.finish().require_artifact().unwrap();
    let module_definition = definition(vec![
        ModuleInstanceDef::new(
            ModuleInstanceKey::from_u128(10),
            changed_module,
            ModuleBindingSet::default(),
            None,
            DiagnosticMeta::default(),
        ),
        instance(20),
    ])
    .validate()
    .require_artifact()
    .unwrap();

    assert_eq!(forward.fingerprint(), reverse.fingerprint());
    assert_eq!(
        forward
            .graph()
            .module_instances()
            .iter()
            .map(ModuleInstanceDef::key)
            .collect::<Vec<_>>(),
        vec![
            ModuleInstanceKey::from_u128(10),
            ModuleInstanceKey::from_u128(20)
        ]
    );
    assert_eq!(
        forward
            .graph()
            .module_instances()
            .iter()
            .map(ModuleInstanceDef::key)
            .collect::<Vec<_>>(),
        reverse
            .graph()
            .module_instances()
            .iter()
            .map(ModuleInstanceDef::key)
            .collect::<Vec<_>>()
    );
    assert_eq!(forward.fingerprint(), metadata.fingerprint());
    assert_ne!(forward.fingerprint(), changed.fingerprint());
    assert_ne!(forward.fingerprint(), hierarchy.fingerprint());
    assert_ne!(forward.fingerprint(), module_definition.fingerprint());
}

#[test]
fn binding_incidence_changes_network_fingerprint() {
    let (module, input, _) = inverter_module();
    let first = ExternalInputKey::<Level>::from_u128(1);
    let second = ExternalInputKey::<Level>::from_u128(2);
    let instance = ModuleInstanceKey::from_u128(3);
    let definition = |source| {
        UncheckedNetwork::new_with_instances(
            NetworkKey::from_u128(4),
            TimeDomainId::from_u128(5),
            DiagnosticMeta::default(),
            vec![],
            vec![
                ExternalInputDef::new(first.into(), DiagnosticMeta::default()),
                ExternalInputDef::new(second.into(), DiagnosticMeta::default()),
            ],
            vec![],
            vec![],
            vec![ModuleInstanceDef::new(
                instance,
                module.clone(),
                ModuleBindingSet::new(vec![ModuleBinding::new(
                    input.into(),
                    ConnectionEndpoint::external_input(source),
                )]),
                None,
                DiagnosticMeta::default(),
            )],
        )
        .validate()
        .require_artifact()
        .unwrap()
    };

    assert_ne!(
        definition(first.into()).fingerprint(),
        definition(second.into()).fingerprint()
    );
}

#[test]
fn nested_instance_identity_changes_module_fingerprint() {
    let mut leaf = ModuleBuilder::<()>::new();
    let output = leaf.constant(mossignal::signal::LogicLevel::High);
    leaf.level_output("output", output).unwrap();
    let leaf = leaf.finish().require_artifact().unwrap();
    let build = |key| {
        let mut outer = ModuleBuilder::<()>::new();
        outer
            .instantiate(
                &leaf,
                ModuleInstanceKey::from_u128(key),
                DiagnosticMeta::default(),
            )
            .unwrap()
            .finish()
            .unwrap();
        outer.finish().require_artifact().unwrap()
    };

    assert_ne!(build(1).fingerprint(), build(2).fingerprint());
}
