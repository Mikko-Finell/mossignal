//! Restricted first-transaction execution and immutable initialization results.

use crate::compile::{EvaluationCause, FullEvaluation};
use crate::diagnostics::{Responsibility, Severity};
use crate::identity::{InputSchemaFingerprint, NetworkFingerprint};
use crate::input::InputSnapshot;
use crate::key::{ExternalInputKey, ExternalOutputKey, NetworkKey, NodeKey};
use crate::machine::{Machine, MachineStatus, NetworkRevision};
use crate::policy::{RuntimePolicy, RuntimePolicyLimit};
use crate::signal::{Level, LogicLevel};
use crate::time::Time;
use core::fmt;
use core::marker::PhantomData;
use std::collections::BTreeMap;
use std::sync::Arc;

const PROVENANCE_SCOPE_DOMAIN: &[u8] = b"mossignal/initialization_provenance_scope/v1";

/// An owned explicit first transaction.
pub struct Transaction<D> {
    at: Time<D>,
    expected_revision: NetworkRevision,
    input: InputSnapshot<D>,
}

impl<D> Transaction<D> {
    /// Constructs the initialization transaction for an uninitialized machine.
    #[must_use]
    pub const fn initialize(
        at: Time<D>,
        expected_revision: NetworkRevision,
        input: InputSnapshot<D>,
    ) -> Self {
        Self {
            at,
            expected_revision,
            input,
        }
    }

    /// Returns the requested initial logical time.
    #[must_use]
    pub const fn requested_time(&self) -> Time<D> {
        self.at
    }

    /// Returns the exact machine revision expected by this transaction.
    #[must_use]
    pub const fn expected_revision(&self) -> NetworkRevision {
        self.expected_revision
    }

    /// Returns the complete exact-bound initialization input.
    #[must_use]
    pub const fn input(&self) -> &InputSnapshot<D> {
        &self.input
    }
}

/// One structured runtime rejection with an exact catalogue-backed category.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeFailureEvidence {
    AlreadyInitialized,
    StaleRevision {
        expected: NetworkRevision,
        actual: NetworkRevision,
    },
    WrongNetwork {
        expected_key: NetworkKey,
        actual_key: NetworkKey,
        expected_fingerprint: NetworkFingerprint,
        actual_fingerprint: NetworkFingerprint,
    },
    ForeignInputSchema {
        expected: InputSchemaFingerprint,
        actual: InputSchemaFingerprint,
    },
    BudgetExceeded {
        budget: RuntimePolicyLimit,
        limit: u64,
        consumed: u64,
    },
}

/// A structured rejection of one runtime transaction.
pub struct RuntimeFailure<D> {
    evidence: RuntimeFailureEvidence,
    domain: PhantomData<fn() -> D>,
}

impl<D> RuntimeFailure<D> {
    fn new(evidence: RuntimeFailureEvidence) -> Self {
        Self {
            evidence,
            domain: PhantomData,
        }
    }

    /// Returns the exact typed failure evidence.
    #[must_use]
    pub const fn evidence(&self) -> &RuntimeFailureEvidence {
        &self.evidence
    }

    /// Returns the exact catalogue code represented by this rejection.
    #[must_use]
    pub const fn code(&self) -> &'static str {
        match self.evidence {
            RuntimeFailureEvidence::AlreadyInitialized => "lifecycle.already_initialized",
            RuntimeFailureEvidence::StaleRevision { .. } => "runtime.stale_revision",
            RuntimeFailureEvidence::WrongNetwork { .. } => "input.wrong_network",
            RuntimeFailureEvidence::ForeignInputSchema { .. } => "input.foreign_schema",
            RuntimeFailureEvidence::BudgetExceeded { .. } => "runtime.budget_exceeded",
        }
    }

    /// Returns the catalogue severity for this rejection.
    #[must_use]
    pub const fn severity(&self) -> Severity {
        Severity::Error
    }

    /// Returns the catalogue responsibility for this rejection.
    #[must_use]
    pub const fn responsibility(&self) -> Responsibility {
        match self.evidence {
            RuntimeFailureEvidence::AlreadyInitialized => Responsibility::CallerInput,
            RuntimeFailureEvidence::StaleRevision { .. }
            | RuntimeFailureEvidence::WrongNetwork { .. }
            | RuntimeFailureEvidence::ForeignInputSchema { .. } => Responsibility::Compatibility,
            RuntimeFailureEvidence::BudgetExceeded { .. } => Responsibility::ResourceLimit,
        }
    }
}

impl<D> fmt::Debug for RuntimeFailure<D> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RuntimeFailure")
            .field("code", &self.code())
            .field("evidence", &self.evidence)
            .finish()
    }
}

impl<D> fmt::Display for RuntimeFailure<D> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "transaction rejected with {}", self.code())
    }
}

impl<D> std::error::Error for RuntimeFailure<D> {}

/// An opaque result-scoped reference to one immutable provenance record.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CauseRef {
    scope: [u8; 16],
    ordinal: u32,
}

impl fmt::Debug for CauseRef {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("CauseRef(..)")
    }
}

/// The semantic subject of one derived initialization cause.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProvenanceSubject {
    Node(NodeKey),
    ExternalOutput(ExternalOutputKey<Level>),
}

enum ProvenanceRecord<D> {
    InitializationTransaction {
        at: Time<D>,
        revision: NetworkRevision,
    },
    ExternalObservation {
        input: ExternalInputKey<Level>,
        value: LogicLevel,
    },
    Derived {
        subject: ProvenanceSubject,
        supporters: Vec<CauseRef>,
    },
}

/// A borrowed structured projection of one immutable causal record.
#[non_exhaustive]
pub enum CauseInspection<'a, D> {
    InitializationTransaction {
        at: Time<D>,
        revision: NetworkRevision,
    },
    ExternalObservation {
        input: ExternalInputKey<Level>,
        value: LogicLevel,
    },
    Derived {
        subject: ProvenanceSubject,
        supporters: &'a [CauseRef],
    },
}

/// Failure to resolve a cause through the owning result's provenance view.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CauseLookupFailure;

impl fmt::Display for CauseLookupFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("cause reference does not belong to this provenance view")
    }
}

impl std::error::Error for CauseLookupFailure {}

/// An immutable result-owned view of initialization derivations.
pub struct ProvenanceView<D> {
    scope: [u8; 16],
    records: Arc<Vec<ProvenanceRecord<D>>>,
}

impl<D> Clone for ProvenanceView<D> {
    fn clone(&self) -> Self {
        Self {
            scope: self.scope,
            records: Arc::clone(&self.records),
        }
    }
}

impl<D> ProvenanceView<D> {
    /// Resolves one result-scoped reference to structured immutable evidence.
    pub fn inspect(&self, cause: CauseRef) -> Result<CauseInspection<'_, D>, CauseLookupFailure> {
        if cause.scope != self.scope {
            return Err(CauseLookupFailure);
        }
        let Some(record) = self.records.get(cause.ordinal as usize) else {
            return Err(CauseLookupFailure);
        };
        Ok(match record {
            ProvenanceRecord::InitializationTransaction { at, revision } => {
                CauseInspection::InitializationTransaction {
                    at: *at,
                    revision: *revision,
                }
            }
            ProvenanceRecord::ExternalObservation { input, value } => {
                CauseInspection::ExternalObservation {
                    input: *input,
                    value: *value,
                }
            }
            ProvenanceRecord::Derived {
                subject,
                supporters,
            } => CauseInspection::Derived {
                subject: *subject,
                supporters,
            },
        })
    }

    /// Returns the number of retained immutable records.
    #[must_use]
    pub fn len(&self) -> usize {
        self.records.len()
    }

    /// Returns whether this view contains no causal records.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }
}

/// One committed external output event.
#[non_exhaustive]
pub enum OutputEvent<D> {
    LevelEstablished {
        output: ExternalOutputKey<Level>,
        value: LogicLevel,
        at: Time<D>,
        cause: CauseRef,
        revision: NetworkRevision,
    },
}

impl<D> fmt::Debug for OutputEvent<D> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LevelEstablished {
                output,
                value,
                at,
                cause,
                revision,
            } => formatter
                .debug_struct("LevelEstablished")
                .field("output", output)
                .field("value", value)
                .field("at", at)
                .field("cause", cause)
                .field("revision", revision)
                .finish(),
        }
    }
}

/// The owned immutable result of a successful initialization transaction.
pub struct TransactionResult<D> {
    requested_time: Time<D>,
    before_revision: NetworkRevision,
    after_revision: NetworkRevision,
    output_events: Vec<OutputEvent<D>>,
    provenance: ProvenanceView<D>,
}

impl<D> TransactionResult<D> {
    #[must_use]
    pub const fn requested_time(&self) -> Time<D> {
        self.requested_time
    }

    #[must_use]
    pub const fn before_revision(&self) -> NetworkRevision {
        self.before_revision
    }

    #[must_use]
    pub const fn after_revision(&self) -> NetworkRevision {
        self.after_revision
    }

    #[must_use]
    pub fn output_events(&self) -> &[OutputEvent<D>] {
        &self.output_events
    }

    #[must_use]
    pub const fn provenance(&self) -> &ProvenanceView<D> {
        &self.provenance
    }
}

impl<D> fmt::Debug for TransactionResult<D> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("TransactionResult")
            .field("requested_time", &self.requested_time)
            .field("before_revision", &self.before_revision)
            .field("after_revision", &self.after_revision)
            .field("output_events", &self.output_events)
            .field("provenance_records", &self.provenance.len())
            .finish()
    }
}

impl<D> Machine<D> {
    /// Applies an owned initialization transaction atomically.
    pub fn apply(
        &mut self,
        transaction: Transaction<D>,
    ) -> Result<TransactionResult<D>, RuntimeFailure<D>> {
        if self.is_initialized() {
            return Err(RuntimeFailure::new(
                RuntimeFailureEvidence::AlreadyInitialized,
            ));
        }
        if transaction.expected_revision != self.store.revision {
            return Err(RuntimeFailure::new(RuntimeFailureEvidence::StaleRevision {
                expected: transaction.expected_revision,
                actual: self.store.revision,
            }));
        }
        if transaction.input.network_key() != self.compiled.network_key() {
            return Err(RuntimeFailure::new(RuntimeFailureEvidence::WrongNetwork {
                expected_key: self.compiled.network_key(),
                actual_key: transaction.input.network_key(),
                expected_fingerprint: self.compiled.fingerprint(),
                actual_fingerprint: transaction.input.network_fingerprint(),
            }));
        }
        if transaction.input.input_schema_fingerprint() != self.compiled.input_schema_fingerprint()
        {
            return Err(RuntimeFailure::new(
                RuntimeFailureEvidence::ForeignInputSchema {
                    expected: self.compiled.input_schema_fingerprint(),
                    actual: transaction.input.input_schema_fingerprint(),
                },
            ));
        }
        if transaction.input.network_fingerprint() != self.compiled.fingerprint() {
            return Err(RuntimeFailure::new(RuntimeFailureEvidence::WrongNetwork {
                expected_key: self.compiled.network_key(),
                actual_key: transaction.input.network_key(),
                expected_fingerprint: self.compiled.fingerprint(),
                actual_fingerprint: transaction.input.network_fingerprint(),
            }));
        }

        enforce_budget::<D>(&self.policy, RuntimePolicyLimit::MaxInternalReactions, 1)?;
        enforce_budget::<D>(
            &self.policy,
            RuntimePolicyLimit::MaxEvaluatedOperations,
            count_as_u64(self.compiled.operation_count()),
        )?;
        enforce_budget::<D>(
            &self.policy,
            RuntimePolicyLimit::MaxEventsCreatedPerTransaction,
            count_as_u64(self.compiled.external_output_count()),
        )?;

        let at = transaction.at;
        let revision = self.store.revision;
        let levels = transaction.input.into_levels();
        let Some(evaluation) = self.compiled.evaluate_full(&levels) else {
            panic!("validated restricted topology and complete snapshot must evaluate fully");
        };
        let (provenance, output_causes) = build_provenance(
            self.compiled.network_key(),
            self.compiled.fingerprint(),
            revision,
            at,
            &levels,
            &evaluation,
        );
        enforce_budget::<D>(
            &self.policy,
            RuntimePolicyLimit::MaxRequiredProvenanceGrowth,
            count_as_u64(provenance.len()),
        )?;

        let output_events =
            initialization_events(at, revision, &evaluation.external_outputs, &output_causes);
        let result = TransactionResult {
            requested_time: at,
            before_revision: revision,
            after_revision: revision,
            output_events,
            provenance: provenance.clone(),
        };

        // SPEC: docs/specs/processor_and_runtime_architecture.md §50 "Reference execution strategy"
        // Every fallible step precedes replacement of the complete private candidate.
        let mut candidate = self.store.clone();
        candidate.status = MachineStatus::Ready { now: at };
        candidate.external_levels = levels;
        candidate.settled_levels = evaluation.values;
        candidate.output_baselines = evaluation.external_outputs;
        candidate.output_causes = output_causes;
        candidate.provenance = Some(provenance);
        self.store = candidate;

        Ok(result)
    }
}

fn count_as_u64(count: usize) -> u64 {
    u64::try_from(count).unwrap_or(u64::MAX)
}

fn enforce_budget<D>(
    policy: &RuntimePolicy,
    budget: RuntimePolicyLimit,
    consumed: u64,
) -> Result<(), RuntimeFailure<D>> {
    let limit = match budget {
        RuntimePolicyLimit::MaxInternalReactions => policy.max_internal_reactions(),
        RuntimePolicyLimit::MaxEvaluatedOperations => policy.max_evaluated_operations(),
        RuntimePolicyLimit::MaxPendingEvents => policy.max_pending_events(),
        RuntimePolicyLimit::MaxEventsCreatedPerTransaction => {
            policy.max_events_created_per_transaction()
        }
        RuntimePolicyLimit::MaxRequiredProvenanceGrowth => policy.max_required_provenance_growth(),
    };
    if consumed > limit {
        return Err(RuntimeFailure::new(
            RuntimeFailureEvidence::BudgetExceeded {
                budget,
                limit,
                consumed,
            },
        ));
    }
    Ok(())
}

fn provenance_scope<D>(
    network_key: NetworkKey,
    fingerprint: NetworkFingerprint,
    revision: NetworkRevision,
    at: Time<D>,
    levels: &BTreeMap<ExternalInputKey<Level>, LogicLevel>,
) -> [u8; 16] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(PROVENANCE_SCOPE_DOMAIN);
    hasher.update(&network_key.as_u128().to_be_bytes());
    hasher.update(&fingerprint.as_bytes());
    hasher.update(&revision.value().to_be_bytes());
    hasher.update(&at.ticks().to_be_bytes());
    for (input, value) in levels {
        hasher.update(&input.as_u128().to_be_bytes());
        hasher.update(&[u8::from(value.is_high())]);
    }
    let digest = hasher.finalize();
    let mut scope = [0_u8; 16];
    scope.copy_from_slice(&digest.as_bytes()[..16]);
    scope
}

fn build_provenance<D>(
    network_key: NetworkKey,
    fingerprint: NetworkFingerprint,
    revision: NetworkRevision,
    at: Time<D>,
    levels: &BTreeMap<ExternalInputKey<Level>, LogicLevel>,
    evaluation: &FullEvaluation,
) -> (
    ProvenanceView<D>,
    BTreeMap<ExternalOutputKey<Level>, CauseRef>,
) {
    let scope = provenance_scope(network_key, fingerprint, revision, at, levels);
    let mut records = Vec::new();
    let transaction_cause = push_record(
        scope,
        &mut records,
        ProvenanceRecord::InitializationTransaction { at, revision },
    );
    let mut operation_causes = Vec::with_capacity(evaluation.causes.len());
    let mut output_causes = BTreeMap::new();

    for cause in &evaluation.causes {
        let resolved = match cause {
            EvaluationCause::ExternalInput(input) => {
                let Some(value) = levels.get(input).copied() else {
                    panic!("evaluated external input must have one authoritative observation");
                };
                push_record(
                    scope,
                    &mut records,
                    ProvenanceRecord::ExternalObservation {
                        input: *input,
                        value,
                    },
                )
            }
            EvaluationCause::Constant(node) => push_record(
                scope,
                &mut records,
                ProvenanceRecord::Derived {
                    subject: ProvenanceSubject::Node(*node),
                    supporters: vec![transaction_cause],
                },
            ),
            EvaluationCause::Node { node, predecessors } => {
                let mut supporters = vec![transaction_cause];
                for predecessor in predecessors {
                    supporters.push(operation_cause(&operation_causes, *predecessor));
                }
                supporters.sort();
                supporters.dedup();
                push_record(
                    scope,
                    &mut records,
                    ProvenanceRecord::Derived {
                        subject: ProvenanceSubject::Node(*node),
                        supporters,
                    },
                )
            }
            EvaluationCause::Alias(source) => operation_cause(&operation_causes, *source),
            EvaluationCause::ExternalOutput { output, source } => {
                let mut supporters = vec![
                    transaction_cause,
                    operation_cause(&operation_causes, *source),
                ];
                supporters.sort();
                supporters.dedup();
                let reference = push_record(
                    scope,
                    &mut records,
                    ProvenanceRecord::Derived {
                        subject: ProvenanceSubject::ExternalOutput(*output),
                        supporters,
                    },
                );
                output_causes.insert(*output, reference);
                reference
            }
        };
        operation_causes.push(resolved);
    }

    (
        ProvenanceView {
            scope,
            records: Arc::new(records),
        },
        output_causes,
    )
}

fn push_record<D>(
    scope: [u8; 16],
    records: &mut Vec<ProvenanceRecord<D>>,
    record: ProvenanceRecord<D>,
) -> CauseRef {
    let ordinal = match u32::try_from(records.len()) {
        Ok(value) => value,
        Err(_) => panic!("initialization provenance exceeds the supported reference space"),
    };
    records.push(record);
    CauseRef { scope, ordinal }
}

fn operation_cause(causes: &[CauseRef], index: usize) -> CauseRef {
    let Some(cause) = causes.get(index).copied() else {
        panic!("evaluation provenance predecessor must precede its dependent operation");
    };
    cause
}

fn initialization_events<D>(
    at: Time<D>,
    revision: NetworkRevision,
    values: &BTreeMap<ExternalOutputKey<Level>, LogicLevel>,
    causes: &BTreeMap<ExternalOutputKey<Level>, CauseRef>,
) -> Vec<OutputEvent<D>> {
    values
        .iter()
        .map(|(output, value)| {
            let Some(cause) = causes.get(output).copied() else {
                panic!("every evaluated external output must retain one committed cause");
            };
            OutputEvent::LevelEstablished {
                output: *output,
                value: *value,
                at,
                cause,
                revision,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use crate::authored::{
        ConnectionDef, ExternalInputDef, ExternalOutputDef, NodeDef, NodeKind, NodePorts,
        UncheckedNetwork,
    };
    use crate::diagnostics::{Responsibility, Severity};
    use crate::key::{
        ConnectionKey, ExternalInputKey, ExternalOutputKey, InPortKey, NetworkKey, NodeKey,
        OutPortKey, SignalSourceKey,
    };
    use crate::metadata::DiagnosticMeta;
    use crate::signal::{Level, LogicLevel};
    use crate::{
        CauseInspection, CauseRef, MachineStatus, NetworkRevision, OutputEvent, ProvenanceView,
        RuntimeFailureEvidence, RuntimePolicy, RuntimePolicyLimit, TimeDomainId, Transaction,
    };
    use std::collections::BTreeSet;

    fn compiled(network_key: u128, output_key: u128) -> crate::CompiledNetwork<()> {
        compiled_with_input(network_key, 1, output_key)
    }

    fn compiled_with_input(
        network_key: u128,
        input_key: u128,
        output_key: u128,
    ) -> crate::CompiledNetwork<()> {
        let input = ExternalInputKey::<Level>::from_u128(input_key);
        UncheckedNetwork::new(
            NetworkKey::from_u128(network_key),
            TimeDomainId::from_u128(2),
            DiagnosticMeta::default(),
            Vec::new(),
            vec![ExternalInputDef::new(
                input.into(),
                DiagnosticMeta::default(),
            )],
            vec![ExternalOutputDef::new(
                ExternalOutputKey::<Level>::from_u128(output_key).into(),
                SignalSourceKey::ExternalInput(input).into(),
                DiagnosticMeta::default(),
            )],
            Vec::new(),
        )
        .validate()
        .require_artifact()
        .unwrap_or_else(|_| panic!("fixture must validate"))
        .compile()
        .require_artifact()
        .unwrap_or_else(|_| panic!("fixture must compile"))
    }

    fn policy() -> RuntimePolicy {
        policy_with([1, 2, 0, 1, 3])
    }

    fn policy_with(values: [u64; 5]) -> RuntimePolicy {
        RuntimePolicy::builder()
            .max_internal_reactions(values[0])
            .max_evaluated_operations(values[1])
            .max_pending_events(values[2])
            .max_events_created_per_transaction(values[3])
            .max_required_provenance_growth(values[4])
            .build()
            .unwrap_or_else(|failure| panic!("policy must build: {failure}"))
    }

    fn assert_pristine(machine: &crate::Machine<()>) {
        assert_eq!(machine.status(), MachineStatus::AwaitingInitialization);
        assert_eq!(machine.now(), None);
        assert!(machine.store.external_levels.is_empty());
        assert!(machine.store.settled_levels.is_empty());
        assert!(machine.store.output_baselines.is_empty());
        assert!(machine.store.output_causes.is_empty());
        assert!(machine.store.provenance.is_none());
    }

    fn complex_compiled(reverse_claims: bool) -> crate::CompiledNetwork<()> {
        let first_input = ExternalInputKey::<Level>::from_u128(1);
        let second_input = ExternalInputKey::<Level>::from_u128(2);
        let first_not_input = InPortKey::<Level>::from_u128(11);
        let first_not_output = OutPortKey::<Level>::from_u128(12);
        let second_not_input = InPortKey::<Level>::from_u128(21);
        let second_not_output = OutPortKey::<Level>::from_u128(22);
        let mut nodes = vec![
            NodeDef::new(
                NodeKey::from_u128(10),
                NodeKind::not(),
                NodePorts::new(vec![first_not_input.into()], vec![first_not_output.into()]),
                DiagnosticMeta::default(),
            ),
            NodeDef::new(
                NodeKey::from_u128(20),
                NodeKind::not(),
                NodePorts::new(
                    vec![second_not_input.into()],
                    vec![second_not_output.into()],
                ),
                DiagnosticMeta::default(),
            ),
        ];
        let mut inputs = vec![
            ExternalInputDef::new(first_input.into(), DiagnosticMeta::default()),
            ExternalInputDef::new(second_input.into(), DiagnosticMeta::default()),
        ];
        let mut outputs = vec![
            ExternalOutputDef::new(
                ExternalOutputKey::<Level>::from_u128(50).into(),
                SignalSourceKey::NodeOutput(first_not_output).into(),
                DiagnosticMeta::default(),
            ),
            ExternalOutputDef::new(
                ExternalOutputKey::<Level>::from_u128(10).into(),
                SignalSourceKey::NodeOutput(second_not_output).into(),
                DiagnosticMeta::default(),
            ),
            ExternalOutputDef::new(
                ExternalOutputKey::<Level>::from_u128(30).into(),
                SignalSourceKey::ExternalInput(second_input).into(),
                DiagnosticMeta::default(),
            ),
        ];
        let mut connections = vec![
            ConnectionDef::new(
                ConnectionKey::from_u128(100),
                first_input.into(),
                first_not_input.into(),
                DiagnosticMeta::default(),
            ),
            ConnectionDef::new(
                ConnectionKey::from_u128(101),
                first_not_output.into(),
                second_not_input.into(),
                DiagnosticMeta::default(),
            ),
        ];
        if reverse_claims {
            nodes.reverse();
            inputs.reverse();
            outputs.reverse();
            connections.reverse();
        }
        UncheckedNetwork::new(
            NetworkKey::from_u128(500),
            TimeDomainId::from_u128(2),
            DiagnosticMeta::default(),
            nodes,
            inputs,
            outputs,
            connections,
        )
        .validate()
        .require_artifact()
        .unwrap_or_else(|_| panic!("fixture must validate"))
        .compile()
        .require_artifact()
        .unwrap_or_else(|_| panic!("fixture must compile"))
    }

    fn recursively_assert_acyclic(view: &ProvenanceView<()>, root: CauseRef) {
        fn visit(
            view: &ProvenanceView<()>,
            cause: CauseRef,
            visiting: &mut BTreeSet<CauseRef>,
            visited: &mut BTreeSet<CauseRef>,
        ) {
            if visited.contains(&cause) {
                return;
            }
            assert!(visiting.insert(cause), "provenance must remain acyclic");
            if let CauseInspection::Derived { supporters, .. } = view
                .inspect(cause)
                .unwrap_or_else(|failure| panic!("cause must resolve: {failure}"))
            {
                for supporter in supporters {
                    visit(view, *supporter, visiting, visited);
                }
            }
            assert!(visiting.remove(&cause));
            visited.insert(cause);
        }

        visit(view, root, &mut BTreeSet::new(), &mut BTreeSet::new());
    }

    #[test]
    fn initialization_commits_ready_state_and_resolvable_establishment() {
        let compiled = compiled(10, 30);
        let input = ExternalInputKey::<Level>::from_u128(1);
        let output = ExternalOutputKey::<Level>::from_u128(30);
        let snapshot = compiled
            .input_snapshot()
            .set(input, LogicLevel::High)
            .and_then(crate::InputSnapshotBuilder::finish)
            .unwrap_or_else(|_| panic!("snapshot must build"));
        let mut machine = compiled.spawn(policy());
        let revision = machine.revision();

        let result = machine
            .apply(Transaction::initialize(
                crate::time::Time::from_ticks(37),
                revision,
                snapshot,
            ))
            .unwrap_or_else(|failure| panic!("initialization must succeed: {failure}"));

        assert_eq!(
            machine.status(),
            MachineStatus::Ready {
                now: crate::time::Time::from_ticks(37)
            }
        );
        assert_eq!(machine.revision(), revision);
        assert_eq!(machine.external_level(input), Some(LogicLevel::High));
        assert_eq!(machine.output_level(output), Some(LogicLevel::High));
        let [
            OutputEvent::LevelEstablished {
                output: actual_output,
                value,
                at,
                cause,
                revision: actual_revision,
            },
        ] = result.output_events()
        else {
            panic!("initialization must return one establishment");
        };
        assert_eq!(*actual_output, output);
        assert_eq!(*value, LogicLevel::High);
        assert_eq!(*at, crate::time::Time::from_ticks(37));
        assert_eq!(*actual_revision, revision);
        assert!(result.provenance().inspect(*cause).is_ok());
        assert_eq!(machine.output_cause(output), Some(*cause));
        recursively_assert_acyclic(result.provenance(), *cause);
        assert_eq!(result.requested_time(), crate::time::Time::from_ticks(37));
        assert_eq!(result.before_revision(), revision);
        assert_eq!(result.after_revision(), revision);
    }

    #[test]
    fn wrong_network_rejection_preserves_awaiting_machine() {
        let local = compiled(10, 30);
        let foreign = compiled(11, 30);
        let input = ExternalInputKey::<Level>::from_u128(1);
        let snapshot = foreign
            .input_snapshot()
            .set(input, LogicLevel::Low)
            .and_then(crate::InputSnapshotBuilder::finish)
            .unwrap_or_else(|_| panic!("snapshot must build"));
        let mut machine = local.spawn(policy());
        let revision = machine.revision();

        let failure = machine
            .apply(Transaction::initialize(
                crate::time::Time::from_ticks(9),
                revision,
                snapshot,
            ))
            .unwrap_err();

        assert!(matches!(
            failure.evidence(),
            RuntimeFailureEvidence::WrongNetwork { .. }
        ));
        assert_pristine(&machine);
        assert_eq!(machine.revision(), revision);
        assert_eq!(machine.external_level(input), None);
        assert_eq!(machine.output_level(ExternalOutputKey::from_u128(30)), None);
    }

    #[test]
    fn ordinary_evaluator_handles_chains_fanout_and_canonical_event_order() {
        for reverse_claims in [false, true] {
            let compiled = complex_compiled(reverse_claims);
            let snapshot = compiled
                .input_snapshot()
                .set(ExternalInputKey::from_u128(1), LogicLevel::Low)
                .and_then(|builder| builder.set(ExternalInputKey::from_u128(2), LogicLevel::High))
                .and_then(crate::InputSnapshotBuilder::finish)
                .unwrap_or_else(|_| panic!("snapshot must build"));
            let mut machine = compiled.spawn(policy_with([1, 9, 0, 3, 8]));
            let result = machine
                .apply(Transaction::initialize(
                    crate::time::Time::from_ticks(0),
                    machine.revision(),
                    snapshot,
                ))
                .unwrap_or_else(|failure| panic!("initialization must succeed: {failure}"));

            let events = result
                .output_events()
                .iter()
                .map(|event| match event {
                    OutputEvent::LevelEstablished {
                        output,
                        value,
                        cause,
                        ..
                    } => (*output, *value, *cause),
                })
                .collect::<Vec<_>>();
            assert_eq!(
                events.iter().map(|event| event.0).collect::<Vec<_>>(),
                vec![
                    ExternalOutputKey::from_u128(10),
                    ExternalOutputKey::from_u128(30),
                    ExternalOutputKey::from_u128(50),
                ]
            );
            assert_eq!(
                events.iter().map(|event| event.1).collect::<Vec<_>>(),
                vec![LogicLevel::Low, LogicLevel::High, LogicLevel::High]
            );
            assert_eq!(machine.store.settled_levels.len(), 9);
            for (_, _, cause) in events {
                recursively_assert_acyclic(result.provenance(), cause);
            }
        }
    }

    #[test]
    fn constant_and_empty_output_networks_initialize_without_special_cases() {
        let constant_output = OutPortKey::<Level>::from_u128(2);
        let output = ExternalOutputKey::<Level>::from_u128(3);
        let constant = UncheckedNetwork::<()>::new(
            NetworkKey::from_u128(1),
            TimeDomainId::from_u128(2),
            DiagnosticMeta::default(),
            vec![NodeDef::new(
                NodeKey::from_u128(4),
                NodeKind::<()>::constant(LogicLevel::High),
                NodePorts::new(Vec::new(), vec![constant_output.into()]),
                DiagnosticMeta::default(),
            )],
            Vec::new(),
            vec![ExternalOutputDef::new(
                output.into(),
                SignalSourceKey::NodeOutput(constant_output).into(),
                DiagnosticMeta::default(),
            )],
            Vec::new(),
        )
        .validate()
        .require_artifact()
        .unwrap_or_else(|_| panic!("fixture must validate"))
        .compile()
        .require_artifact()
        .unwrap_or_else(|_| panic!("fixture must compile"));
        let mut constant_machine = constant.spawn(policy_with([1, 3, 0, 1, 3]));
        let result = constant_machine
            .apply(Transaction::initialize(
                crate::time::Time::from_ticks(5),
                constant_machine.revision(),
                constant
                    .input_snapshot()
                    .finish()
                    .unwrap_or_else(|_| panic!("empty snapshot must build")),
            ))
            .unwrap_or_else(|failure| panic!("constant initialization must succeed: {failure}"));
        assert_eq!(
            constant_machine.output_level(output),
            Some(LogicLevel::High)
        );
        assert_eq!(result.output_events().len(), 1);

        let empty = UncheckedNetwork::<()>::new(
            NetworkKey::from_u128(9),
            TimeDomainId::from_u128(2),
            DiagnosticMeta::default(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
        )
        .validate()
        .require_artifact()
        .unwrap_or_else(|_| panic!("fixture must validate"))
        .compile()
        .require_artifact()
        .unwrap_or_else(|_| panic!("fixture must compile"));
        let mut empty_machine = empty.spawn(policy_with([1, 0, 0, 0, 1]));
        let result = empty_machine
            .apply(Transaction::initialize(
                crate::time::Time::from_ticks(u64::MAX),
                empty_machine.revision(),
                empty
                    .input_snapshot()
                    .finish()
                    .unwrap_or_else(|_| panic!("empty snapshot must build")),
            ))
            .unwrap_or_else(|failure| panic!("empty initialization must succeed: {failure}"));
        assert!(result.output_events().is_empty());
        assert_eq!(
            empty_machine.now(),
            Some(crate::time::Time::from_ticks(u64::MAX))
        );
    }

    #[test]
    fn every_identity_and_lifecycle_rejection_is_structured_and_atomic() {
        let local = compiled(10, 30);
        let input = ExternalInputKey::<Level>::from_u128(1);

        let mut stale = local.spawn(policy());
        let expected = stale.revision();
        stale.store.revision = NetworkRevision::from_value(9);
        let failure = stale
            .apply(Transaction::initialize(
                crate::time::Time::from_ticks(1),
                expected,
                local
                    .input_snapshot()
                    .set(input, LogicLevel::Low)
                    .and_then(crate::InputSnapshotBuilder::finish)
                    .unwrap_or_else(|_| panic!("snapshot must build")),
            ))
            .unwrap_err();
        assert!(matches!(
            failure.evidence(),
            RuntimeFailureEvidence::StaleRevision { .. }
        ));
        assert_pristine(&stale);
        assert_eq!(stale.revision(), NetworkRevision::from_value(9));

        let same_schema_other_topology = compiled(10, 31);
        let mut fingerprint_machine = local.spawn(policy());
        let failure = fingerprint_machine
            .apply(Transaction::initialize(
                crate::time::Time::from_ticks(1),
                fingerprint_machine.revision(),
                same_schema_other_topology
                    .input_snapshot()
                    .set(input, LogicLevel::Low)
                    .and_then(crate::InputSnapshotBuilder::finish)
                    .unwrap_or_else(|_| panic!("snapshot must build")),
            ))
            .unwrap_err();
        assert!(matches!(
            failure.evidence(),
            RuntimeFailureEvidence::WrongNetwork { .. }
        ));
        assert_pristine(&fingerprint_machine);

        let other_schema = compiled_with_input(10, 2, 30);
        let mut schema_machine = local.spawn(policy());
        let failure = schema_machine
            .apply(Transaction::initialize(
                crate::time::Time::from_ticks(1),
                schema_machine.revision(),
                other_schema
                    .input_snapshot()
                    .set(ExternalInputKey::from_u128(2), LogicLevel::Low)
                    .and_then(crate::InputSnapshotBuilder::finish)
                    .unwrap_or_else(|_| panic!("snapshot must build")),
            ))
            .unwrap_err();
        assert!(matches!(
            failure.evidence(),
            RuntimeFailureEvidence::ForeignInputSchema { .. }
        ));
        assert_pristine(&schema_machine);

        let mut ready = local.spawn(policy());
        let first = local
            .input_snapshot()
            .set(input, LogicLevel::High)
            .and_then(crate::InputSnapshotBuilder::finish)
            .unwrap_or_else(|_| panic!("snapshot must build"));
        ready
            .apply(Transaction::initialize(
                crate::time::Time::from_ticks(4),
                ready.revision(),
                first,
            ))
            .unwrap_or_else(|failure| panic!("first initialization must succeed: {failure}"));
        let before_cause = ready.output_cause(ExternalOutputKey::from_u128(30));
        let before_provenance_len = ready
            .store
            .provenance
            .as_ref()
            .map_or(0, ProvenanceView::len);
        let failure = ready
            .apply(Transaction::initialize(
                crate::time::Time::from_ticks(5),
                ready.revision(),
                local
                    .input_snapshot()
                    .set(input, LogicLevel::Low)
                    .and_then(crate::InputSnapshotBuilder::finish)
                    .unwrap_or_else(|_| panic!("snapshot must build")),
            ))
            .unwrap_err();
        assert!(matches!(
            failure.evidence(),
            RuntimeFailureEvidence::AlreadyInitialized
        ));
        assert_eq!(ready.now(), Some(crate::time::Time::from_ticks(4)));
        assert_eq!(ready.external_level(input), Some(LogicLevel::High));
        assert_eq!(
            ready.output_cause(ExternalOutputKey::from_u128(30)),
            before_cause
        );
        assert_eq!(
            ready
                .store
                .provenance
                .as_ref()
                .map_or(0, ProvenanceView::len),
            before_provenance_len
        );
    }

    #[test]
    fn implemented_budget_boundaries_reject_one_over_and_preserve_machine() {
        let cases = [
            (
                RuntimePolicyLimit::MaxInternalReactions,
                [0, 2, 0, 1, 3],
                0,
                1,
            ),
            (
                RuntimePolicyLimit::MaxEvaluatedOperations,
                [1, 1, 0, 1, 3],
                1,
                2,
            ),
            (
                RuntimePolicyLimit::MaxEventsCreatedPerTransaction,
                [1, 2, 0, 0, 3],
                0,
                1,
            ),
            (
                RuntimePolicyLimit::MaxRequiredProvenanceGrowth,
                [1, 2, 0, 1, 2],
                2,
                3,
            ),
        ];
        for (expected_budget, limits, expected_limit, expected_consumed) in cases {
            let compiled = compiled(10, 30);
            let input = ExternalInputKey::<Level>::from_u128(1);
            let mut machine = compiled.spawn(policy_with(limits));
            let failure = machine
                .apply(Transaction::initialize(
                    crate::time::Time::from_ticks(7),
                    machine.revision(),
                    compiled
                        .input_snapshot()
                        .set(input, LogicLevel::High)
                        .and_then(crate::InputSnapshotBuilder::finish)
                        .unwrap_or_else(|_| panic!("snapshot must build")),
                ))
                .unwrap_err();
            assert_eq!(failure.code(), "runtime.budget_exceeded");
            assert_eq!(failure.severity(), Severity::Error);
            assert_eq!(failure.responsibility(), Responsibility::ResourceLimit);
            assert_eq!(
                failure.evidence(),
                &RuntimeFailureEvidence::BudgetExceeded {
                    budget: expected_budget,
                    limit: expected_limit,
                    consumed: expected_consumed,
                }
            );
            assert_pristine(&machine);
        }

        for limits in [[1, 2, 0, 1, 3], [2, 3, 0, 2, 4]] {
            let compiled = compiled(10, 30);
            let mut machine = compiled.spawn(policy_with(limits));
            machine
                .apply(Transaction::initialize(
                    crate::time::Time::from_ticks(8),
                    machine.revision(),
                    compiled
                        .input_snapshot()
                        .set(ExternalInputKey::from_u128(1), LogicLevel::Low)
                        .and_then(crate::InputSnapshotBuilder::finish)
                        .unwrap_or_else(|_| panic!("snapshot must build")),
                ))
                .unwrap_or_else(|failure| panic!("within-budget work must succeed: {failure}"));
            assert!(machine.is_initialized());
        }
    }
}
