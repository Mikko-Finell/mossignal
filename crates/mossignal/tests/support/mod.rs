use mossignal::key::{ExternalInputKey, ExternalOutputKey};
use mossignal::signal::{Level, LogicLevel};
use mossignal::time::Time;
use mossignal::{
    CompiledNetwork, Machine, NetworkBuilder, NetworkRevision, OutputEvent, RuntimePolicy,
    Transaction, TransactionResult,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TestTicks;

pub struct Circuit {
    pub compiled: CompiledNetwork<TestTicks>,
    pub inputs: Vec<ExternalInputKey<Level>>,
    pub outputs: Vec<ExternalOutputKey<Level>>,
}

impl Circuit {
    pub fn compile(
        builder: NetworkBuilder<TestTicks>,
        inputs: Vec<ExternalInputKey<Level>>,
        outputs: Vec<ExternalOutputKey<Level>>,
    ) -> Self {
        let validated = builder
            .finish()
            .require_artifact()
            .unwrap_or_else(|failure| panic!("test circuit must validate: {failure:?}"));
        let compiled = validated
            .compile()
            .require_artifact()
            .unwrap_or_else(|failure| panic!("test circuit must compile: {failure:?}"));
        Self {
            compiled,
            inputs,
            outputs,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NormalizedEvent {
    Established {
        output: ExternalOutputKey<Level>,
        value: LogicLevel,
        at: u64,
        revision: NetworkRevision,
    },
    Changed {
        output: ExternalOutputKey<Level>,
        from: LogicLevel,
        to: LogicLevel,
        at: u64,
        revision: NetworkRevision,
    },
}

#[derive(Debug, PartialEq, Eq)]
struct TransactionObservation {
    requested_time: u64,
    before_revision: NetworkRevision,
    after_revision: NetworkRevision,
    events: Vec<NormalizedEvent>,
    settled_outputs: Vec<LogicLevel>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct BehaviorTrace {
    transactions: Vec<TransactionObservation>,
}

impl BehaviorTrace {
    pub fn settled_outputs(&self) -> impl Iterator<Item = &[LogicLevel]> {
        self.transactions
            .iter()
            .map(|observation| observation.settled_outputs.as_slice())
    }
}

pub fn run_complete_trace(circuit: &Circuit, valuations: &[Vec<LogicLevel>]) -> BehaviorTrace {
    assert!(
        !valuations.is_empty(),
        "an equivalence trace needs an initial valuation"
    );
    for valuation in valuations {
        assert_eq!(
            valuation.len(),
            circuit.inputs.len(),
            "every valuation must cover the circuit's complete input schema"
        );
    }

    let mut machine = circuit.compiled.spawn(policy());
    let initial = snapshot(circuit, &valuations[0]);
    let revision = machine.revision();
    let result = machine
        .apply(Transaction::initialize(
            Time::from_ticks(0),
            revision,
            initial,
        ))
        .unwrap_or_else(|failure| panic!("initialization must succeed: {failure}"));
    let mut transactions = vec![observe(circuit, &machine, &result)];

    for (index, valuation) in valuations.iter().enumerate().skip(1) {
        let delta = delta(circuit, valuation);
        let revision = machine.revision();
        let result = machine
            .apply(Transaction::advance(
                Time::from_ticks(index as u64),
                revision,
                delta,
            ))
            .unwrap_or_else(|failure| panic!("level transaction must succeed: {failure}"));
        transactions.push(observe(circuit, &machine, &result));
    }

    BehaviorTrace { transactions }
}

pub fn assert_behaviorally_equivalent(circuits: &[Circuit], valuations: &[Vec<LogicLevel>]) {
    assert!(
        circuits.len() >= 2,
        "equivalence needs at least two circuits"
    );
    let reference = run_complete_trace(&circuits[0], valuations);
    for candidate in &circuits[1..] {
        assert_eq!(run_complete_trace(candidate, valuations), reference);
    }
}

fn snapshot(circuit: &Circuit, valuation: &[LogicLevel]) -> mossignal::InputSnapshot<TestTicks> {
    let mut builder = circuit.compiled.input_snapshot();
    for (input, value) in circuit.inputs.iter().zip(valuation) {
        builder = builder
            .set(*input, *value)
            .unwrap_or_else(|failure| panic!("snapshot input must be accepted: {failure}"));
    }
    builder
        .finish()
        .unwrap_or_else(|failure| panic!("complete snapshot must build: {failure}"))
}

fn delta(circuit: &Circuit, valuation: &[LogicLevel]) -> mossignal::InputDelta<TestTicks> {
    let mut builder = circuit.compiled.input_delta();
    for (input, value) in circuit.inputs.iter().zip(valuation) {
        builder = builder
            .set(*input, *value)
            .unwrap_or_else(|failure| panic!("delta input must be accepted: {failure}"));
    }
    builder
        .finish()
        .unwrap_or_else(|failure| panic!("complete delta must build: {failure}"))
}

fn observe(
    circuit: &Circuit,
    machine: &Machine<TestTicks>,
    result: &TransactionResult<TestTicks>,
) -> TransactionObservation {
    TransactionObservation {
        requested_time: result.requested_time().ticks(),
        before_revision: result.before_revision(),
        after_revision: result.after_revision(),
        events: result.output_events().iter().map(normalize_event).collect(),
        settled_outputs: circuit
            .outputs
            .iter()
            .map(|output| {
                machine
                    .output_level(*output)
                    .unwrap_or_else(|| panic!("output {output:?} must be settled"))
            })
            .collect(),
    }
}

fn normalize_event(event: &OutputEvent<TestTicks>) -> NormalizedEvent {
    match event {
        OutputEvent::LevelEstablished {
            output,
            value,
            at,
            revision,
            ..
        } => NormalizedEvent::Established {
            output: *output,
            value: *value,
            at: at.ticks(),
            revision: *revision,
        },
        OutputEvent::LevelChanged {
            output,
            from,
            to,
            at,
            revision,
            ..
        } => NormalizedEvent::Changed {
            output: *output,
            from: *from,
            to: *to,
            at: at.ticks(),
            revision: *revision,
        },
        _ => panic!("composed level circuits must emit level output events"),
    }
}

fn policy() -> RuntimePolicy {
    RuntimePolicy::builder()
        .max_internal_reactions(10_000)
        .max_evaluated_operations(10_000)
        .max_pending_events(10_000)
        .max_events_created_per_transaction(10_000)
        .max_required_provenance_growth(10_000)
        .build()
        .unwrap_or_else(|failure| panic!("complete test policy must build: {failure}"))
}
