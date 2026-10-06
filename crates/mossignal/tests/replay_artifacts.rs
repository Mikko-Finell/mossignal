//! Deterministic replay over ordinary transactions.

use mossignal::key::{
    ExternalInputKey, ExternalOutputKey, InPortKey, ModuleInputKey, ModuleInstanceKey,
    ModuleOutputKey, NetworkKey, NodeKey, OutPortKey,
};
use mossignal::metadata::DiagnosticMeta;
use mossignal::signal::{Level, LogicLevel, Pulse, PulseCount};
use mossignal::time::{NonZeroSpan, Time};
use mossignal::{
    DecodePolicy, EncodeFailure, InputSnapshotBuilder, ModuleBuilder, NetworkBuilder, OutputEvent,
    PersistenceContext, PulseDelayConfig, ReplayFailure, ReplayLog, RuntimePolicy, TimeDomainId,
    Transaction, TransactionResult, decode_replay_frame, decode_replay_log, decode_snapshot,
    encode_replay_frame, encode_replay_log, encode_snapshot, record_replay_log,
};

#[derive(Debug, PartialEq, Eq)]
enum Domain {}

fn policy() -> RuntimePolicy {
    RuntimePolicy::builder()
        .max_internal_reactions(1_000)
        .max_evaluated_operations(100_000)
        .max_pending_events(1_000)
        .max_events_created_per_transaction(10_000)
        .max_required_provenance_growth(100_000)
        .build()
        .unwrap_or_else(|failure| panic!("policy must build: {failure}"))
}

fn other_policy() -> RuntimePolicy {
    RuntimePolicy::builder()
        .max_internal_reactions(8)
        .max_evaluated_operations(80)
        .max_pending_events(8)
        .max_events_created_per_transaction(8)
        .max_required_provenance_growth(80)
        .build()
        .unwrap_or_else(|failure| panic!("alternate policy must build: {failure}"))
}

fn limits(frames: u64) -> DecodePolicy {
    DecodePolicy::new(
        8_000_000, 64, 2_000_000, 8_000_000, 200_000, 10_000, 10_000, 10_000, 1_000, 10_000,
        100_000, 100_000, 10_000, frames, 1_000_000,
    )
}

fn compile_not(
    network: u128,
    domain: u128,
) -> (
    mossignal::CompiledNetwork<Domain>,
    ExternalInputKey<Level>,
    ExternalOutputKey<Level>,
) {
    let mut builder = NetworkBuilder::with_key(
        NetworkKey::from_u128(network),
        TimeDomainId::from_u128(domain),
    );
    let input = ExternalInputKey::from_u128(1);
    let signal = builder
        .add_level_input(input, DiagnosticMeta::default())
        .unwrap_or_else(|failure| panic!("level input must author: {failure:?}"));
    let inverted = builder
        .add_not_with_ports(
            NodeKey::from_u128(2),
            InPortKey::from_u128(3),
            OutPortKey::from_u128(4),
            signal,
            DiagnosticMeta::default(),
        )
        .unwrap_or_else(|failure| panic!("Not must author: {failure:?}"))
        .into_outputs();
    let output = ExternalOutputKey::from_u128(5);
    builder
        .add_level_output(output, inverted, DiagnosticMeta::default())
        .unwrap_or_else(|failure| panic!("level output must author: {failure:?}"));
    let compiled = builder
        .finish()
        .require_artifact()
        .unwrap_or_else(|failure| panic!("Not must validate: {failure:?}"))
        .compile()
        .require_artifact()
        .unwrap_or_else(|failure| panic!("Not must compile: {failure:?}"));
    (compiled, input, output)
}

fn golden_not() -> (
    mossignal::CompiledNetwork<Domain>,
    ExternalInputKey<Level>,
    ExternalOutputKey<Level>,
) {
    compile_not(7, 11)
}

fn context(compiled: &mossignal::CompiledNetwork<Domain>) -> PersistenceContext<Domain> {
    PersistenceContext::new(compiled.time_domain_id())
}

fn level_snapshot(
    compiled: &mossignal::CompiledNetwork<Domain>,
    observations: &[(ExternalInputKey<Level>, LogicLevel)],
) -> mossignal::InputSnapshot<Domain> {
    let mut builder = compiled.input_snapshot();
    for (input, level) in observations {
        builder = builder
            .set(*input, *level)
            .unwrap_or_else(|failure| panic!("level observation must bind: {failure}"));
    }
    builder
        .finish()
        .unwrap_or_else(|failure| panic!("snapshot must finish: {failure}"))
}

fn level_delta(
    compiled: &mossignal::CompiledNetwork<Domain>,
    observations: &[(ExternalInputKey<Level>, LogicLevel)],
) -> mossignal::InputDelta<Domain> {
    let mut builder = compiled.input_delta();
    for (input, level) in observations {
        builder = builder
            .set(*input, *level)
            .unwrap_or_else(|failure| panic!("level delta must bind: {failure}"));
    }
    builder
        .finish()
        .unwrap_or_else(|failure| panic!("delta must finish: {failure}"))
}

fn init_tx(
    compiled: &mossignal::CompiledNetwork<Domain>,
    input: ExternalInputKey<Level>,
    level: LogicLevel,
) -> Transaction<Domain> {
    Transaction::initialize(
        Time::from_ticks(0),
        compiled.spawn(policy()).revision(),
        level_snapshot(compiled, &[(input, level)]),
    )
}

fn advance_tx(
    compiled: &mossignal::CompiledNetwork<Domain>,
    input: ExternalInputKey<Level>,
    at: u64,
    level: LogicLevel,
) -> Transaction<Domain> {
    Transaction::advance(
        Time::from_ticks(at),
        compiled.spawn(policy()).revision(),
        level_delta(compiled, &[(input, level)]),
    )
}

fn golden_transactions(
    compiled: &mossignal::CompiledNetwork<Domain>,
    input: ExternalInputKey<Level>,
) -> [Transaction<Domain>; 2] {
    [
        init_tx(compiled, input, LogicLevel::High),
        advance_tx(compiled, input, 1, LogicLevel::Low),
    ]
}

fn record(
    compiled: &mossignal::CompiledNetwork<Domain>,
    transactions: impl IntoIterator<Item = Transaction<Domain>>,
) -> (mossignal::Machine<Domain>, ReplayLog<Domain>) {
    let mut machine = compiled.spawn(policy());
    let log = record_replay_log(&mut machine, transactions)
        .unwrap_or_else(|failure| panic!("recording must apply: {failure}"));
    (machine, log)
}

fn code(failure: &ReplayFailure<Domain>) -> &'static str {
    failure.code().as_str()
}

fn event_text<D>(results: &[TransactionResult<D>]) -> Vec<String> {
    results
        .iter()
        .map(|result| format!("{:?}", result.output_events()))
        .collect()
}

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(DIGITS[(byte >> 4) as usize] as char);
        encoded.push(DIGITS[(byte & 0x0f) as usize] as char);
    }
    encoded
}

fn assert_golden(name: &str, bytes: &[u8]) {
    let expected = match name {
        "replay_not_gate_frame.hex" => include_str!("golden/replay_not_gate_frame.hex"),
        "replay_not_gate_log.hex" => include_str!("golden/replay_not_gate_log.hex"),
        _ => panic!("unknown replay golden {name}"),
    };
    assert_eq!(hex(bytes), expected.trim(), "{name}");
}

#[test]
fn not_gate_log_round_trips_replays_and_matches_goldens() {
    let (compiled, input, output) = golden_not();
    let (recorded, log) = record(&compiled, golden_transactions(&compiled, input));
    let context = context(&compiled);
    let log_bytes = encode_replay_log(&context, &log).unwrap_or_else(|failure| panic!("{failure}"));
    let frame_bytes = encode_replay_frame(&context, &log.frames()[0])
        .unwrap_or_else(|failure| panic!("{failure}"));
    assert_golden("replay_not_gate_log.hex", log_bytes.as_bytes());
    assert_golden("replay_not_gate_frame.hex", frame_bytes.as_bytes());
    assert_ne!(log_bytes.as_bytes(), frame_bytes.as_bytes());

    let decoded = decode_replay_log(&context, log_bytes.as_bytes(), &limits(8))
        .unwrap_or_else(|failure| panic!("log must decode: {failure:?}"));
    assert_eq!(decoded.content_digest(), log.content_digest());
    let again = encode_replay_log(&context, &decoded).unwrap_or_else(|failure| panic!("{failure}"));
    assert_eq!(again.as_bytes(), log_bytes.as_bytes());
    let frame = decode_replay_frame(&context, frame_bytes.as_bytes(), &limits(8))
        .unwrap_or_else(|failure| panic!("frame must decode: {failure:?}"));
    assert_eq!(
        encode_replay_frame(&context, &frame)
            .unwrap_or_else(|error| panic!("{error}"))
            .as_bytes(),
        frame_bytes.as_bytes()
    );

    let mut replayed = compiled.spawn(policy());
    let results = replayed
        .replay_log(&decoded)
        .unwrap_or_else(|failure| panic!("replay must apply: {failure:?}"));
    assert_eq!(
        replayed.execution_state_digest(),
        recorded.execution_state_digest()
    );
    assert_eq!(
        replayed.observable_state_digest(),
        recorded.observable_state_digest()
    );
    assert_eq!(replayed.output_level(output), Some(LogicLevel::High));
    assert_eq!(results.len(), 2);

    let mut folded = compiled.spawn(policy());
    let mut folded_results = Vec::new();
    for transaction in golden_transactions(&compiled, input) {
        folded_results.push(
            folded
                .apply(transaction)
                .unwrap_or_else(|failure| panic!("fold must apply: {failure}")),
        );
    }
    assert_eq!(event_text(&results), event_text(&folded_results));
    assert_eq!(
        replayed.execution_state_digest(),
        folded.execution_state_digest()
    );
    assert_eq!(
        replayed.observable_state_digest(),
        folded.observable_state_digest()
    );
}

#[test]
fn input_insertion_order_does_not_change_the_log_and_frame_order_does() {
    let (compiled, first, second, output) = two_input_nots();
    let forward = level_snapshot(
        &compiled,
        &[(first, LogicLevel::High), (second, LogicLevel::Low)],
    );
    let reverse = level_snapshot(
        &compiled,
        &[(second, LogicLevel::Low), (first, LogicLevel::High)],
    );
    let (_, forward_log) = record(
        &compiled,
        [Transaction::initialize(
            Time::from_ticks(0),
            compiled.spawn(policy()).revision(),
            forward,
        )],
    );
    let (_, reverse_log) = record(
        &compiled,
        [Transaction::initialize(
            Time::from_ticks(0),
            compiled.spawn(policy()).revision(),
            reverse,
        )],
    );
    let context = context(&compiled);
    let forward_bytes = encode_replay_log(&context, &forward_log).unwrap();
    let reverse_bytes = encode_replay_log(&context, &reverse_log).unwrap();
    assert_eq!(forward_bytes.as_bytes(), reverse_bytes.as_bytes());
    assert_eq!(forward_log.content_digest(), reverse_log.content_digest());

    let revision = compiled.spawn(policy()).revision();
    let low_then_high = record(
        &compiled,
        [Transaction::initialize(
            Time::from_ticks(0),
            revision,
            level_snapshot(
                &compiled,
                &[(first, LogicLevel::Low), (second, LogicLevel::High)],
            ),
        )],
    )
    .1;
    let high_then_low = record(
        &compiled,
        [Transaction::initialize(
            Time::from_ticks(0),
            revision,
            level_snapshot(
                &compiled,
                &[(first, LogicLevel::High), (second, LogicLevel::Low)],
            ),
        )],
    )
    .1;
    assert_ne!(
        encode_replay_log(&context, &low_then_high)
            .unwrap()
            .as_bytes(),
        encode_replay_log(&context, &high_then_low)
            .unwrap()
            .as_bytes()
    );
    let _ = output;
}

fn two_input_nots() -> (
    mossignal::CompiledNetwork<Domain>,
    ExternalInputKey<Level>,
    ExternalInputKey<Level>,
    ExternalOutputKey<Level>,
) {
    let mut builder =
        NetworkBuilder::with_key(NetworkKey::from_u128(17), TimeDomainId::from_u128(19));
    let first = ExternalInputKey::from_u128(1);
    let second = ExternalInputKey::from_u128(2);
    let first_signal = builder
        .add_level_input(first, DiagnosticMeta::default())
        .unwrap();
    let second_signal = builder
        .add_level_input(second, DiagnosticMeta::default())
        .unwrap();
    let inverted = builder
        .add_not_with_ports(
            NodeKey::from_u128(3),
            InPortKey::from_u128(4),
            OutPortKey::from_u128(5),
            first_signal,
            DiagnosticMeta::default(),
        )
        .unwrap()
        .into_outputs();
    let passed = builder
        .add_not_with_ports(
            NodeKey::from_u128(6),
            InPortKey::from_u128(7),
            OutPortKey::from_u128(8),
            second_signal,
            DiagnosticMeta::default(),
        )
        .unwrap()
        .into_outputs();
    let output = ExternalOutputKey::from_u128(9);
    builder
        .add_level_output(output, inverted, DiagnosticMeta::default())
        .unwrap();
    builder
        .add_level_output(
            ExternalOutputKey::from_u128(10),
            passed,
            DiagnosticMeta::default(),
        )
        .unwrap();
    let compiled = builder
        .finish()
        .require_artifact()
        .unwrap()
        .compile()
        .require_artifact()
        .unwrap();
    (compiled, first, second, output)
}

#[test]
fn hostile_framing_is_rejected_before_replay() {
    let (compiled, input, _) = golden_not();
    let (_, log) = record(&compiled, golden_transactions(&compiled, input));
    let context = context(&compiled);
    let bytes = encode_replay_log(&context, &log).unwrap();
    let body = &bytes.as_bytes()[8..];

    let prefix = decode_replay_log(&context, &[0, 1, 2, 3, 4, 5, 6, 7, 8], &limits(8)).unwrap_err();
    assert_eq!(code(&prefix), "persistence.invalid_prefix");
    let truncated = decode_replay_log(&context, &bytes.as_bytes()[..8], &limits(8)).unwrap_err();
    assert_eq!(code(&truncated), "persistence.truncated_artifact");
    let clipped = decode_replay_log(
        &context,
        &bytes.as_bytes()[..bytes.as_bytes().len() - 3],
        &limits(8),
    )
    .unwrap_err();
    assert_eq!(code(&clipped), "persistence.truncated_artifact");
    let mut trailed = bytes.as_bytes().to_vec();
    trailed.push(0xff);
    let trailing = decode_replay_log(&context, &trailed, &limits(8)).unwrap_err();
    assert_eq!(code(&trailing), "persistence.trailing_bytes");
    let mut joined = bytes.as_bytes().to_vec();
    joined.extend_from_slice(bytes.as_bytes());
    let concatenated = decode_replay_log(&context, &joined, &limits(8)).unwrap_err();
    assert_eq!(code(&concatenated), "persistence.trailing_bytes");
    let noncanonical = vec![0x4d, 0x53, 0x49, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x18, 0x01];
    let noncanonical = decode_replay_log(&context, &noncanonical, &limits(8)).unwrap_err();
    assert_eq!(code(&noncanonical), "persistence.noncanonical_encoding");

    let mut corrupted = bytes.as_bytes().to_vec();
    let index = corrupted
        .windows(4)
        .position(|window| window == b"high")
        .unwrap_or_else(|| panic!("encoded log must name the high level"));
    corrupted[index] = b'x';
    let integrity = decode_replay_log(&context, &corrupted, &limits(8)).unwrap_err();
    assert_eq!(code(&integrity), "persistence.integrity_digest_mismatch");

    let limited = decode_replay_log(&context, bytes.as_bytes(), &limits(0)).unwrap_err();
    assert_eq!(code(&limited), "persistence.decode_limit_exceeded");
    let frame_limited = decode_replay_frame(&context, body, &limits(0)).unwrap_err();
    assert_eq!(code(&frame_limited), "persistence.decode_limit_exceeded");
    let _ = input;
}

#[test]
fn empty_log_decodes_when_the_frame_budget_is_zero() {
    let (compiled, _, _) = golden_not();
    let (machine, log) = record(&compiled, Vec::<Transaction<Domain>>::new());
    assert!(log.frames().is_empty());
    let context = context(&compiled);
    let bytes = encode_replay_log(&context, &log).unwrap();
    let decoded = decode_replay_log(&context, bytes.as_bytes(), &limits(0))
        .unwrap_or_else(|failure| panic!("empty log must decode: {failure:?}"));
    assert_eq!(decoded.content_digest(), log.content_digest());
    let mut replayed = compiled.spawn(policy());
    replayed
        .replay_log(&decoded)
        .unwrap_or_else(|failure| panic!("empty replay must succeed: {failure:?}"));
    assert_eq!(
        replayed.execution_state_digest(),
        machine.execution_state_digest()
    );
    assert_eq!(
        replayed.observable_state_digest(),
        machine.observable_state_digest()
    );
}

#[test]
fn checkpoint_mismatches_use_one_replay_code() {
    let (compiled, input, _) = golden_not();
    let (_, log) = record(&compiled, golden_transactions(&compiled, input));
    let mut initialized = compiled.spawn(policy());
    initialized
        .apply(init_tx(&compiled, input, LogicLevel::High))
        .unwrap();
    let execution = initialized.replay_log(&log).unwrap_err();
    assert_eq!(
        code(&execution),
        "replay.starting_execution_digest_mismatch"
    );
    assert!(initialized.is_initialized());

    let mut other_policy_machine = compiled.spawn(other_policy());
    let policy_failure = other_policy_machine.replay_log(&log).unwrap_err();
    assert_eq!(code(&policy_failure), "replay.runtime_policy_mismatch");
    assert!(!other_policy_machine.is_initialized());

    let (other_network, _, _) = compile_not(8, 11);
    let mut foreign = other_network.spawn(policy());
    let fingerprint = foreign.replay_log(&log).unwrap_err();
    assert_eq!(code(&fingerprint), "replay.network_fingerprint_mismatch");

    let (other_domain, _, _) = compile_not(7, 12);
    let mut shifted = other_domain.spawn(policy());
    let time_domain = shifted.replay_log(&log).unwrap_err();
    assert_eq!(code(&time_domain), "replay.time_domain_mismatch");
}

#[test]
fn runtime_failure_keeps_its_code_and_the_prior_frames() {
    let (compiled, input, output) = golden_not();
    let mut machine = compiled.spawn(policy());
    let failure = record_replay_log(
        &mut machine,
        [
            init_tx(&compiled, input, LogicLevel::High),
            advance_tx(&compiled, input, 0, LogicLevel::Low),
        ],
    )
    .unwrap_err();
    assert_eq!(
        failure.code().as_str(),
        "runtime.time_not_strictly_increasing"
    );
    assert!(machine.is_initialized());
    assert_eq!(machine.now(), Some(Time::from_ticks(0)));
    assert_eq!(machine.output_level(output), Some(LogicLevel::Low));

    let mut fresh = compiled.spawn(policy());
    let revision = fresh.revision();
    let early = record_replay_log(
        &mut fresh,
        [Transaction::advance(
            Time::from_ticks(1),
            revision,
            level_delta(&compiled, &[(input, LogicLevel::Low)]),
        )],
    )
    .unwrap_err();
    assert_eq!(
        early.code().as_str(),
        "lifecycle.delta_before_initialization"
    );
    assert!(!fresh.is_initialized());
}

#[test]
fn concatenation_renumbers_frames_and_matches_one_recording() {
    let (compiled, input, _) = golden_not();
    let transactions = [
        init_tx(&compiled, input, LogicLevel::High),
        advance_tx(&compiled, input, 1, LogicLevel::Low),
        advance_tx(&compiled, input, 2, LogicLevel::High),
    ];
    let (_, whole) = record(&compiled, transactions.clone());
    let mut machine = compiled.spawn(policy());
    let prefix = record_replay_log(&mut machine, transactions[..2].iter().cloned()).unwrap();
    let suffix = record_replay_log(&mut machine, transactions[2..].iter().cloned()).unwrap();
    let joined = prefix
        .concatenate(&suffix)
        .unwrap_or_else(|failure| panic!("adjacent logs must concatenate: {failure:?}"));
    let context = context(&compiled);
    assert_eq!(
        encode_replay_log(&context, &joined).unwrap().as_bytes(),
        encode_replay_log(&context, &whole).unwrap().as_bytes()
    );
    assert_eq!(joined.content_digest(), whole.content_digest());
    assert_eq!(
        joined
            .frames()
            .iter()
            .map(|frame| frame.frame_index())
            .collect::<Vec<_>>(),
        vec![0, 1, 2]
    );

    let (foreign_network, foreign_input, _) = compile_not(9, 11);
    let (_, foreign) = record(
        &foreign_network,
        golden_transactions(&foreign_network, foreign_input),
    );
    let rejected = prefix.concatenate(&foreign).unwrap_err();
    assert_eq!(code(&rejected), "replay.logs_not_concatenable");
    let overlap = prefix.concatenate(&prefix).unwrap_err();
    assert_eq!(code(&overlap), "replay.logs_not_concatenable");
}

#[test]
fn direct_and_stepwise_deadline_replay_match_including_concatenation() {
    let (compiled, input, output) = pulse_delay(2);
    let init = || {
        let snapshot = compiled
            .input_snapshot()
            .pulse(input, PulseCount::ONE)
            .and_then(InputSnapshotBuilder::finish)
            .unwrap();
        Transaction::initialize(
            Time::from_ticks(0),
            compiled.spawn(policy()).revision(),
            snapshot,
        )
    };
    let advance = |at| {
        Transaction::advance(
            Time::from_ticks(at),
            compiled.spawn(policy()).revision(),
            compiled.input_delta().finish().unwrap(),
        )
    };
    let direct_transactions = [init(), advance(2)];
    let stepwise_transactions = [init(), advance(1), advance(2)];
    let (direct_machine, direct_log) = record(&compiled, direct_transactions);
    let (step_machine, step_log) = record(&compiled, stepwise_transactions);
    assert_eq!(
        direct_machine.execution_state_digest(),
        step_machine.execution_state_digest()
    );
    assert_eq!(
        direct_machine.observable_state_digest(),
        step_machine.observable_state_digest()
    );

    let mut direct_replay = compiled.spawn(policy());
    let direct_results = direct_replay.replay_log(&direct_log).unwrap();
    let mut step_replay = compiled.spawn(policy());
    let step_results = step_replay.replay_log(&step_log).unwrap();
    assert_eq!(
        direct_replay.execution_state_digest(),
        direct_machine.execution_state_digest()
    );
    assert_eq!(
        step_replay.observable_state_digest(),
        step_machine.observable_state_digest()
    );
    assert_eq!(
        pulse_events(direct_results.last().unwrap().output_events()),
        pulse_events(step_results.last().unwrap().output_events())
    );
    assert_eq!(
        pulse_events(direct_results.last().unwrap().output_events()),
        vec![(output.as_u128(), 1, 2)]
    );

    let boundary = [init(), advance(1), advance(2), advance(3)];
    let (_, whole) = record(&compiled, boundary.clone());
    let mut machine = compiled.spawn(policy());
    let before = record_replay_log(&mut machine, boundary[..2].iter().cloned()).unwrap();
    let after_deadline = record_replay_log(&mut machine, boundary[2..].iter().cloned()).unwrap();
    let joined_before = before.concatenate(&after_deadline).unwrap();
    let context = context(&compiled);
    assert_eq!(
        encode_replay_log(&context, &joined_before)
            .unwrap()
            .as_bytes(),
        encode_replay_log(&context, &whole).unwrap().as_bytes()
    );
    assert_eq!(joined_before.content_digest(), whole.content_digest());

    let mut machine = compiled.spawn(policy());
    let through_deadline = record_replay_log(&mut machine, boundary[..3].iter().cloned()).unwrap();
    let after = record_replay_log(&mut machine, boundary[3..].iter().cloned()).unwrap();
    let joined_after = through_deadline.concatenate(&after).unwrap();
    assert_eq!(
        encode_replay_log(&context, &joined_after)
            .unwrap()
            .as_bytes(),
        encode_replay_log(&context, &whole).unwrap().as_bytes()
    );
    assert_eq!(joined_after.content_digest(), whole.content_digest());
}

fn pulse_delay(
    delay_ticks: u64,
) -> (
    mossignal::CompiledNetwork<Domain>,
    ExternalInputKey<Pulse>,
    ExternalOutputKey<Pulse>,
) {
    let mut builder =
        NetworkBuilder::with_key(NetworkKey::from_u128(1), TimeDomainId::from_u128(2));
    let input = ExternalInputKey::from_u128(10);
    let signal = builder
        .add_pulse_input(input, DiagnosticMeta::default())
        .unwrap();
    let delayed = builder
        .add_pulse_delay_with_ports(
            NodeKey::from_u128(20),
            InPortKey::from_u128(30),
            OutPortKey::from_u128(31),
            signal,
            PulseDelayConfig::new(NonZeroSpan::from_ticks(delay_ticks).unwrap()),
            DiagnosticMeta::default(),
        )
        .unwrap()
        .into_outputs();
    let output = ExternalOutputKey::from_u128(40);
    builder
        .add_pulse_output(output, delayed, DiagnosticMeta::default())
        .unwrap();
    let compiled = builder
        .finish()
        .require_artifact()
        .unwrap()
        .compile()
        .require_artifact()
        .unwrap();
    (compiled, input, output)
}

fn pulse_events<D>(events: &[OutputEvent<D>]) -> Vec<(u128, u64, u64)> {
    events
        .iter()
        .map(|event| match event {
            OutputEvent::Pulsed {
                output, count, at, ..
            } => (output.as_u128(), count.get(), at.ticks()),
            _ => panic!("pulse delay must publish pulse events"),
        })
        .collect()
}

#[test]
fn module_replay_matches_the_ordinary_fold() {
    let input_key = ModuleInputKey::<Level>::from_u128(1);
    let output_key = ModuleOutputKey::<Level>::from_u128(2);
    let mut module = ModuleBuilder::<Domain>::new();
    let source = module
        .add_level_input(input_key, DiagnosticMeta::default())
        .unwrap();
    let inverted = module.not(source).unwrap();
    module
        .add_level_output(output_key, inverted, DiagnosticMeta::default())
        .unwrap();
    let module = module.finish().require_artifact().unwrap();

    let instance = ModuleInstanceKey::from_u128(10);
    let external_input = ExternalInputKey::<Level>::from_u128(11);
    let external_output = ExternalOutputKey::<Level>::from_u128(12);
    let mut network =
        NetworkBuilder::with_key(NetworkKey::from_u128(13), TimeDomainId::from_u128(14));
    let external = network
        .add_level_input(external_input, DiagnosticMeta::default())
        .unwrap();
    let added = network
        .instantiate(&module, instance, DiagnosticMeta::default())
        .unwrap()
        .bind_level(input_key, external)
        .unwrap()
        .finish()
        .unwrap();
    network
        .add_level_output(
            external_output,
            added.level_output(output_key).unwrap(),
            DiagnosticMeta::default(),
        )
        .unwrap();
    let compiled = network
        .finish()
        .require_artifact()
        .unwrap()
        .compile()
        .require_artifact()
        .unwrap();
    let transactions = [
        init_tx(&compiled, external_input, LogicLevel::Low),
        advance_tx(&compiled, external_input, 4, LogicLevel::High),
    ];
    let (folded, log) = record(&compiled, transactions.clone());
    let mut replayed = compiled.spawn(policy());
    let results = replayed.replay_log(&log).unwrap();
    let mut ordinary = compiled.spawn(policy());
    let ordinary_results = transactions
        .into_iter()
        .map(|transaction| ordinary.apply(transaction).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(event_text(&results), event_text(&ordinary_results));
    assert_eq!(
        replayed.execution_state_digest(),
        folded.execution_state_digest()
    );
    assert_eq!(
        replayed.observable_state_digest(),
        folded.observable_state_digest()
    );
    assert_eq!(
        replayed.output_level(external_output),
        Some(LogicLevel::Low)
    );
}

#[test]
fn standard_module_replay_matches_the_ordinary_fold() {
    let mut builder =
        NetworkBuilder::with_key(NetworkKey::from_u128(50), TimeDomainId::from_u128(51));
    let first = ExternalInputKey::<Level>::from_u128(1);
    let second = ExternalInputKey::<Level>::from_u128(2);
    let left = builder
        .add_level_input(first, DiagnosticMeta::default())
        .unwrap();
    let right = builder
        .add_level_input(second, DiagnosticMeta::default())
        .unwrap();
    let result = builder.all_equal([left, right]).unwrap();
    let output = ExternalOutputKey::<Level>::from_u128(3);
    builder
        .add_level_output(output, result, DiagnosticMeta::default())
        .unwrap();
    let compiled = builder
        .finish()
        .require_artifact()
        .unwrap()
        .compile()
        .require_artifact()
        .unwrap();
    let init = Transaction::initialize(
        Time::from_ticks(0),
        compiled.spawn(policy()).revision(),
        level_snapshot(
            &compiled,
            &[(first, LogicLevel::High), (second, LogicLevel::High)],
        ),
    );
    let advance = Transaction::advance(
        Time::from_ticks(3),
        compiled.spawn(policy()).revision(),
        level_delta(&compiled, &[(first, LogicLevel::Low)]),
    );
    let (folded, log) = record(&compiled, [init.clone(), advance.clone()]);
    let mut replayed = compiled.spawn(policy());
    let results = replayed.replay_log(&log).unwrap();
    let mut ordinary = compiled.spawn(policy());
    let ordinary_results = [init, advance]
        .into_iter()
        .map(|transaction| ordinary.apply(transaction).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(event_text(&results), event_text(&ordinary_results));
    assert_eq!(
        replayed.execution_state_digest(),
        folded.execution_state_digest()
    );
    assert_eq!(
        replayed.observable_state_digest(),
        folded.observable_state_digest()
    );
    assert_eq!(replayed.output_level(output), Some(LogicLevel::Low));
}

#[test]
fn restored_snapshot_replays_the_suffix_without_requiring_snapshot_digest_equality() {
    let (compiled, input, output) = golden_not();
    let mut source = compiled.spawn(policy());
    record_replay_log(&mut source, [init_tx(&compiled, input, LogicLevel::Low)]).unwrap();
    let context = context(&compiled);
    let encoded = encode_snapshot(&context, &source.snapshot()).unwrap();
    let decoded = decode_snapshot(&context, encoded.as_bytes(), &limits(8)).unwrap();
    let mut restored = compiled.restore(decoded, policy()).unwrap();
    let suffix = record_replay_log(
        &mut source,
        [advance_tx(&compiled, input, 6, LogicLevel::High)],
    )
    .unwrap();
    restored.replay_log(&suffix).unwrap();

    let (whole, _) = record(
        &compiled,
        [
            init_tx(&compiled, input, LogicLevel::Low),
            advance_tx(&compiled, input, 6, LogicLevel::High),
        ],
    );
    assert_eq!(
        restored.execution_state_digest(),
        whole.execution_state_digest()
    );
    assert_eq!(
        restored.observable_state_digest(),
        whole.observable_state_digest()
    );
    assert_eq!(restored.output_level(output), whole.output_level(output));
    // Replay success is the two state digests. SnapshotDigest is not a success condition.
    let _ = (
        source.snapshot().snapshot_digest(),
        whole.snapshot().snapshot_digest(),
    );
}

#[test]
fn encoding_rejects_a_context_from_another_time_domain() {
    let (compiled, input, _) = golden_not();
    let (_, log) = record(&compiled, golden_transactions(&compiled, input));
    let context = PersistenceContext::<Domain>::new(TimeDomainId::from_u128(99));
    assert_eq!(
        encode_replay_log(&context, &log),
        Err(EncodeFailure::TimeDomainMismatch)
    );
    assert_eq!(
        encode_replay_frame(&context, &log.frames()[0]),
        Err(EncodeFailure::TimeDomainMismatch)
    );
}

#[test]
fn wrong_context_time_domain_is_rejected_on_decode() {
    let (compiled, input, _) = golden_not();
    let (_, log) = record(&compiled, golden_transactions(&compiled, input));
    let bytes = encode_replay_log(&context(&compiled), &log).unwrap();
    let shifted = PersistenceContext::<Domain>::new(TimeDomainId::from_u128(99));
    let failure = decode_replay_log(&shifted, bytes.as_bytes(), &limits(8)).unwrap_err();
    assert_eq!(code(&failure), "persistence.wrong_time_domain");
}
