mod support;

use std::collections::BTreeMap;

use mossignal::builder::Signal;
use mossignal::key::{
    ExternalInputKey, ExternalOutputKey, InPortKey, NetworkKey, NodeKey, OutPortKey,
};
use mossignal::metadata::DiagnosticMeta;
use mossignal::signal::{Level, LogicLevel};
use mossignal::{NetworkBuilder, TimeDomainId};
use support::{Circuit, TestTicks, assert_behaviorally_equivalent, run_complete_trace};

const DOMAIN_ID: u128 = 0x2441;

#[test]
fn half_adders_are_equivalent_over_the_complete_domain() {
    let valuations = binary_valuations(2);
    let circuits = [half_adder_direct(), half_adder_conventional()];

    // Cross-topology comparison deliberately observes behavior rather than identity.
    assert_behaviorally_equivalent(&circuits, &valuations);
    assert_truth_table(&circuits[0], &valuations, |bits| {
        vec![level(bits[0] ^ bits[1]), level(bits[0] && bits[1])]
    });
}

#[test]
fn full_adders_are_equivalent_over_the_complete_domain() {
    let valuations = binary_valuations(3);
    let circuits = [
        full_adder_direct(false),
        full_adder_conventional(),
        full_adder_nand_only(),
    ];

    // Direct primitives, conventional gates, and NAND-only gates have distinct topology.
    assert_behaviorally_equivalent(&circuits, &valuations);
    assert_truth_table(&circuits[0], &valuations, |bits| {
        let high_count = bits.iter().filter(|bit| **bit).count();
        vec![level(high_count % 2 == 1), level(high_count >= 2)]
    });
}

#[test]
fn stable_keyed_full_adder_ignores_public_endpoint_insertion_order() {
    let canonical = full_adder_direct(false);
    let reordered = full_adder_direct(true);

    assert_eq!(
        canonical.compiled.fingerprint(),
        reordered.compiled.fingerprint()
    );
    assert_behaviorally_equivalent(&[canonical, reordered], &binary_valuations(3));
}

#[test]
fn four_bit_ripple_adders_exhaust_both_carry_levels_and_every_operand_pair() {
    let valuations = ripple_valuations();
    let canonical = ripple_adder(false);
    let reordered = ripple_adder(true);

    assert_eq!(
        canonical.compiled.fingerprint(),
        reordered.compiled.fingerprint()
    );
    assert_behaviorally_equivalent(&[canonical, reordered], &valuations);
    assert_truth_table(&ripple_adder(false), &valuations, |bits| {
        let left = nibble(bits, 0);
        let right = nibble(bits, 4);
        let total = left + right + u16::from(bits[8]);
        vec![
            level(total & 1 != 0),
            level(total & 2 != 0),
            level(total & 4 != 0),
            level(total & 8 != 0),
            level(total & 16 != 0),
        ]
    });
}

fn half_adder_direct() -> Circuit {
    let mut builder = circuit_builder(1);
    let inputs = add_inputs(&mut builder, 2, false);
    let sum = parity(&mut builder, 1, &[inputs[0], inputs[1]]);
    let carry = all(&mut builder, 2, &[inputs[0], inputs[1]]);
    let outputs = add_outputs(&mut builder, &[sum, carry], false);
    Circuit::compile(builder, input_keys(2), outputs)
}

fn half_adder_conventional() -> Circuit {
    let mut builder = circuit_builder(1);
    let inputs = add_inputs(&mut builder, 2, false);
    let sum = xor_gates(&mut builder, 10, inputs[0], inputs[1]);
    let carry = all(&mut builder, 20, &[inputs[0], inputs[1]]);
    let outputs = add_outputs(&mut builder, &[sum, carry], false);
    Circuit::compile(builder, input_keys(2), outputs)
}

fn full_adder_direct(reordered_endpoints: bool) -> Circuit {
    let mut builder = circuit_builder(2);
    let inputs = add_inputs(&mut builder, 3, reordered_endpoints);
    let sum = parity(&mut builder, 1, &inputs);
    let carry = at_least(&mut builder, 2, 2, &inputs);
    let outputs = add_outputs(&mut builder, &[sum, carry], reordered_endpoints);
    Circuit::compile(builder, input_keys(3), outputs)
}

fn full_adder_conventional() -> Circuit {
    let mut builder = circuit_builder(2);
    let inputs = add_inputs(&mut builder, 3, false);
    let first_sum = xor_gates(&mut builder, 10, inputs[0], inputs[1]);
    let sum = xor_gates(&mut builder, 20, first_sum, inputs[2]);
    let first_carry = all(&mut builder, 30, &[inputs[0], inputs[1]]);
    let second_carry = all(&mut builder, 31, &[first_sum, inputs[2]]);
    let carry = any(&mut builder, 32, &[first_carry, second_carry]);
    let outputs = add_outputs(&mut builder, &[sum, carry], false);
    Circuit::compile(builder, input_keys(3), outputs)
}

fn full_adder_nand_only() -> Circuit {
    let mut builder = circuit_builder(2);
    let inputs = add_inputs(&mut builder, 3, false);
    let first_ab = nand(&mut builder, 1, inputs[0], inputs[1]);
    let first_a = nand(&mut builder, 2, inputs[0], first_ab);
    let first_b = nand(&mut builder, 3, inputs[1], first_ab);
    let first_sum = nand(&mut builder, 4, first_a, first_b);
    let second_ab = nand(&mut builder, 5, first_sum, inputs[2]);
    let second_a = nand(&mut builder, 6, first_sum, second_ab);
    let second_b = nand(&mut builder, 7, inputs[2], second_ab);
    let sum = nand(&mut builder, 8, second_a, second_b);
    let carry = nand(&mut builder, 9, first_ab, second_ab);
    let outputs = add_outputs(&mut builder, &[sum, carry], false);
    Circuit::compile(builder, input_keys(3), outputs)
}

fn ripple_adder(reordered_endpoints: bool) -> Circuit {
    let mut builder = circuit_builder(3);
    let inputs = add_inputs(&mut builder, 9, reordered_endpoints);
    let mut carry = inputs[8];
    let mut sums = Vec::with_capacity(4);
    for bit in 0..4 {
        let operands = [inputs[bit], inputs[4 + bit], carry];
        sums.push(parity(&mut builder, 10 + bit as u128 * 2, &operands));
        carry = at_least(&mut builder, 11 + bit as u128 * 2, 2, &operands);
    }
    sums.push(carry);
    let outputs = add_outputs(&mut builder, &sums, reordered_endpoints);
    Circuit::compile(builder, input_keys(9), outputs)
}

fn circuit_builder(network: u128) -> NetworkBuilder<TestTicks> {
    NetworkBuilder::with_key(
        NetworkKey::from_u128(network),
        TimeDomainId::from_u128(DOMAIN_ID),
    )
}

fn add_inputs(
    builder: &mut NetworkBuilder<TestTicks>,
    count: usize,
    reverse: bool,
) -> Vec<Signal<Level>> {
    let order: Box<dyn Iterator<Item = usize>> = if reverse {
        Box::new((0..count).rev())
    } else {
        Box::new(0..count)
    };
    let mut signals = BTreeMap::new();
    for index in order {
        let signal = builder
            .add_level_input(external_input(index), DiagnosticMeta::default())
            .unwrap_or_else(|failure| panic!("stable input must be accepted: {failure:?}"));
        signals.insert(index, signal);
    }
    (0..count).map(|index| signals[&index]).collect()
}

fn add_outputs(
    builder: &mut NetworkBuilder<TestTicks>,
    signals: &[Signal<Level>],
    reverse: bool,
) -> Vec<ExternalOutputKey<Level>> {
    let order: Box<dyn Iterator<Item = usize>> = if reverse {
        Box::new((0..signals.len()).rev())
    } else {
        Box::new(0..signals.len())
    };
    for index in order {
        builder
            .add_level_output(
                external_output(index),
                signals[index],
                DiagnosticMeta::default(),
            )
            .unwrap_or_else(|failure| panic!("stable output must be accepted: {failure:?}"));
    }
    (0..signals.len()).map(external_output).collect()
}

fn not(builder: &mut NetworkBuilder<TestTicks>, id: u128, input: Signal<Level>) -> Signal<Level> {
    builder
        .add_not_with_ports(
            node(id),
            in_port(id, 0),
            out_port(id),
            input,
            DiagnosticMeta::default(),
        )
        .unwrap_or_else(|failure| panic!("Not node must author: {failure:?}"))
        .into_outputs()
}

fn all(
    builder: &mut NetworkBuilder<TestTicks>,
    id: u128,
    inputs: &[Signal<Level>],
) -> Signal<Level> {
    builder
        .add_all_with_ports(
            node(id),
            out_port(id),
            inputs
                .iter()
                .enumerate()
                .map(|(slot, signal)| (in_port(id, slot), *signal)),
            DiagnosticMeta::default(),
        )
        .unwrap_or_else(|failure| panic!("All node must author: {failure:?}"))
        .into_outputs()
}

fn any(
    builder: &mut NetworkBuilder<TestTicks>,
    id: u128,
    inputs: &[Signal<Level>],
) -> Signal<Level> {
    builder
        .add_any_with_ports(
            node(id),
            out_port(id),
            inputs
                .iter()
                .enumerate()
                .map(|(slot, signal)| (in_port(id, slot), *signal)),
            DiagnosticMeta::default(),
        )
        .unwrap_or_else(|failure| panic!("Any node must author: {failure:?}"))
        .into_outputs()
}

fn parity(
    builder: &mut NetworkBuilder<TestTicks>,
    id: u128,
    inputs: &[Signal<Level>],
) -> Signal<Level> {
    builder
        .add_parity_with_ports(
            node(id),
            out_port(id),
            inputs
                .iter()
                .enumerate()
                .map(|(slot, signal)| (in_port(id, slot), *signal)),
            DiagnosticMeta::default(),
        )
        .unwrap_or_else(|failure| panic!("Parity node must author: {failure:?}"))
        .into_outputs()
}

fn at_least(
    builder: &mut NetworkBuilder<TestTicks>,
    id: u128,
    threshold: u64,
    inputs: &[Signal<Level>],
) -> Signal<Level> {
    builder
        .add_at_least_with_ports(
            node(id),
            out_port(id),
            threshold,
            inputs
                .iter()
                .enumerate()
                .map(|(slot, signal)| (in_port(id, slot), *signal)),
            DiagnosticMeta::default(),
        )
        .unwrap_or_else(|failure| panic!("AtLeast node must author: {failure:?}"))
        .into_outputs()
}

fn xor_gates(
    builder: &mut NetworkBuilder<TestTicks>,
    base: u128,
    left: Signal<Level>,
    right: Signal<Level>,
) -> Signal<Level> {
    let not_left = not(builder, base, left);
    let not_right = not(builder, base + 1, right);
    let left_only = all(builder, base + 2, &[left, not_right]);
    let right_only = all(builder, base + 3, &[not_left, right]);
    any(builder, base + 4, &[left_only, right_only])
}

fn nand(
    builder: &mut NetworkBuilder<TestTicks>,
    index: u128,
    left: Signal<Level>,
    right: Signal<Level>,
) -> Signal<Level> {
    let conjunction = all(builder, 100 + index * 2, &[left, right]);
    not(builder, 101 + index * 2, conjunction)
}

fn external_input(index: usize) -> ExternalInputKey<Level> {
    ExternalInputKey::from_u128(100 + index as u128)
}

fn external_output(index: usize) -> ExternalOutputKey<Level> {
    ExternalOutputKey::from_u128(200 + index as u128)
}

fn input_keys(count: usize) -> Vec<ExternalInputKey<Level>> {
    (0..count).map(external_input).collect()
}

fn node(id: u128) -> NodeKey {
    NodeKey::from_u128(1_000 + id)
}

fn in_port(id: u128, slot: usize) -> InPortKey<Level> {
    InPortKey::from_u128(10_000 + id * 10 + slot as u128)
}

fn out_port(id: u128) -> OutPortKey<Level> {
    OutPortKey::from_u128(20_000 + id)
}

fn binary_valuations(width: usize) -> Vec<Vec<LogicLevel>> {
    (0..(1_usize << width))
        .map(|value| {
            (0..width)
                .map(|bit| level(value & (1 << bit) != 0))
                .collect()
        })
        .collect()
}

fn ripple_valuations() -> Vec<Vec<LogicLevel>> {
    let mut valuations = Vec::with_capacity(2 * 16 * 16);
    for carry in [false, true] {
        for left in 0..16 {
            for right in 0..16 {
                let mut bits = (0..4)
                    .map(|bit| level(left & (1 << bit) != 0))
                    .chain((0..4).map(|bit| level(right & (1 << bit) != 0)))
                    .collect::<Vec<_>>();
                bits.push(level(carry));
                valuations.push(bits);
            }
        }
    }
    valuations
}

fn assert_truth_table(
    circuit: &Circuit,
    valuations: &[Vec<LogicLevel>],
    expected: impl Fn(&[bool]) -> Vec<LogicLevel>,
) {
    let trace = run_complete_trace(circuit, valuations);
    for (valuation, actual) in valuations.iter().zip(trace.settled_outputs()) {
        let bits = valuation
            .iter()
            .map(|value| *value == LogicLevel::High)
            .collect::<Vec<_>>();
        assert_eq!(actual, expected(&bits));
    }
}

fn nibble(bits: &[bool], offset: usize) -> u16 {
    (0..4).fold(0, |value, bit| {
        value | (u16::from(bits[offset + bit]) << bit)
    })
}

const fn level(high: bool) -> LogicLevel {
    if high {
        LogicLevel::High
    } else {
        LogicLevel::Low
    }
}
