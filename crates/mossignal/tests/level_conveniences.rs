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
use mossignal::{AuthoringFailure, ModuleBuilder, NetworkBuilder, TimeDomainId};
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

fn assert_foreign(result: Result<Signal<Level>, AuthoringFailure>) {
    assert!(matches!(result, Err(AuthoringFailure::ForeignSignal)));
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
