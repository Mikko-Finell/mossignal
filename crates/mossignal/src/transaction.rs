//! Restricted initialization and ready-machine level transactions.

use crate::compile::{EvaluationCause, FullEvaluation};
use crate::diagnostics::{Responsibility, Severity};
use crate::identity::{InputSchemaFingerprint, NetworkFingerprint};
use crate::input::{InputDelta, InputSnapshot};
use crate::key::{ExternalInputKey, ExternalOutputKey, NetworkKey, NodeKey};
use crate::machine::{Machine, MachineStatus, NetworkRevision};
use crate::policy::{RuntimePolicy, RuntimePolicyLimit};
use crate::signal::{Level, LogicLevel};
use crate::time::Time;
use core::fmt;
use core::marker::PhantomData;
use std::collections::BTreeMap;
use std::sync::Arc;

const PROVENANCE_SCOPE_DOMAIN: &[u8] = b"mossignal/transaction_provenance_scope/v1";

enum TransactionKind<D> {
    Initialize(InputSnapshot<D>),
    Advance(InputDelta<D>),
}

/// An owned explicit runtime transaction.
pub struct Transaction<D> {
    at: Time<D>,
    expected_revision: NetworkRevision,
    kind: TransactionKind<D>,
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
            kind: TransactionKind::Initialize(input),
        }
    }

    /// Constructs a ready-machine advancement transaction.
    #[must_use]
    pub const fn advance(
        at: Time<D>,
        expected_revision: NetworkRevision,
        input: InputDelta<D>,
    ) -> Self {
        Self {
            at,
            expected_revision,
            kind: TransactionKind::Advance(input),
        }
    }

    /// Returns the requested logical time.
    #[must_use]
    pub const fn requested_time(&self) -> Time<D> {
        self.at
    }

    /// Returns the exact machine revision expected by this transaction.
    #[must_use]
    pub const fn expected_revision(&self) -> NetworkRevision {
        self.expected_revision
    }

    /// Returns the initialization input when this is an initialization transaction.
    #[must_use]
    pub const fn initialization_input(&self) -> Option<&InputSnapshot<D>> {
        match &self.kind {
            TransactionKind::Initialize(input) => Some(input),
            TransactionKind::Advance(_) => None,
        }
    }

    /// Returns the level delta when this is a ready-machine transaction.
    #[must_use]
    pub const fn advance_input(&self) -> Option<&InputDelta<D>> {
        match &self.kind {
            TransactionKind::Initialize(_) => None,
            TransactionKind::Advance(input) => Some(input),
        }
    }
}

/// One structured runtime rejection with an exact catalogue-backed category.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeFailureEvidence {
    AlreadyInitialized,
    DeltaBeforeInitialization,
    TimeNotStrictlyIncreasing {
        current_ticks: u64,
        requested_ticks: u64,
    },
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
    StaleInputSchema {
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
            RuntimeFailureEvidence::DeltaBeforeInitialization => {
                "lifecycle.delta_before_initialization"
            }
            RuntimeFailureEvidence::TimeNotStrictlyIncreasing { .. } => {
                "runtime.time_not_strictly_increasing"
            }
            RuntimeFailureEvidence::StaleRevision { .. } => "runtime.stale_revision",
            RuntimeFailureEvidence::WrongNetwork { .. } => "input.wrong_network",
            RuntimeFailureEvidence::ForeignInputSchema { .. } => "input.foreign_schema",
            RuntimeFailureEvidence::StaleInputSchema { .. } => "input.stale_schema",
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
            RuntimeFailureEvidence::AlreadyInitialized
            | RuntimeFailureEvidence::DeltaBeforeInitialization
            | RuntimeFailureEvidence::TimeNotStrictlyIncreasing { .. } => {
                Responsibility::CallerInput
            }
            RuntimeFailureEvidence::StaleRevision { .. }
            | RuntimeFailureEvidence::WrongNetwork { .. }
            | RuntimeFailureEvidence::ForeignInputSchema { .. }
            | RuntimeFailureEvidence::StaleInputSchema { .. } => Responsibility::Compatibility,
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
    ReadyTransaction {
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
    ReadyTransaction {
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

struct ProvenanceBuild<D> {
    provenance: ProvenanceView<D>,
    input_causes: BTreeMap<ExternalInputKey<Level>, CauseRef>,
    output_causes: BTreeMap<ExternalOutputKey<Level>, CauseRef>,
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
            ProvenanceRecord::ReadyTransaction { at, revision } => {
                CauseInspection::ReadyTransaction {
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
    LevelChanged {
        output: ExternalOutputKey<Level>,
        from: LogicLevel,
        to: LogicLevel,
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
            Self::LevelChanged {
                output,
                from,
                to,
                at,
                cause,
                revision,
            } => formatter
                .debug_struct("LevelChanged")
                .field("output", output)
                .field("from", from)
                .field("to", to)
                .field("at", at)
                .field("cause", cause)
                .field("revision", revision)
                .finish(),
        }
    }
}

/// The owned immutable result of a successful transaction.
pub struct TransactionResult<D> {
    requested_time: Time<D>,
    before_revision: NetworkRevision,
    after_revision: NetworkRevision,
    output_events: Vec<OutputEvent<D>>,
    provenance: ProvenanceView<D>,
}

impl<D> TransactionResult<D> {
    /// Returns the logical time requested by the applied transaction.
    #[must_use]
    pub const fn requested_time(&self) -> Time<D> {
        self.requested_time
    }

    /// Returns the machine revision observed before application.
    #[must_use]
    pub const fn before_revision(&self) -> NetworkRevision {
        self.before_revision
    }

    /// Returns the machine revision retained after application.
    #[must_use]
    pub const fn after_revision(&self) -> NetworkRevision {
        self.after_revision
    }

    /// Returns the deterministic external output event stream.
    #[must_use]
    pub fn output_events(&self) -> &[OutputEvent<D>] {
        &self.output_events
    }

    /// Returns the immutable view that resolves every event cause.
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
    /// Applies an owned transaction atomically.
    pub fn apply(
        &mut self,
        transaction: Transaction<D>,
    ) -> Result<TransactionResult<D>, RuntimeFailure<D>> {
        let Transaction {
            at,
            expected_revision,
            kind,
        } = transaction;
        match kind {
            TransactionKind::Initialize(input) => {
                self.apply_initialization(at, expected_revision, input)
            }
            TransactionKind::Advance(input) => self.apply_advance(at, expected_revision, input),
        }
    }

    fn apply_initialization(
        &mut self,
        at: Time<D>,
        expected_revision: NetworkRevision,
        input: InputSnapshot<D>,
    ) -> Result<TransactionResult<D>, RuntimeFailure<D>> {
        if self.is_initialized() {
            return Err(RuntimeFailure::new(
                RuntimeFailureEvidence::AlreadyInitialized,
            ));
        }
        if expected_revision != self.store.revision {
            return Err(RuntimeFailure::new(RuntimeFailureEvidence::StaleRevision {
                expected: expected_revision,
                actual: self.store.revision,
            }));
        }
        validate_snapshot_binding::<D>(&self.compiled, &input)?;

        enforce_reaction_budgets::<D>(&self.policy, &self.compiled)?;
        enforce_budget::<D>(
            &self.policy,
            RuntimePolicyLimit::MaxEventsCreatedPerTransaction,
            count_as_u64(self.compiled.external_output_count()),
        )?;

        let revision = self.store.revision;
        let levels = input.into_levels();
        let Some(evaluation) = self.compiled.evaluate_full(&levels) else {
            panic!("validated restricted topology and complete snapshot must evaluate fully");
        };
        let ProvenanceBuild {
            provenance,
            input_causes,
            output_causes,
        } = build_initialization_provenance(
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

        publish_candidate(
            self,
            at,
            levels,
            evaluation,
            input_causes,
            output_causes,
            provenance,
        );
        Ok(result)
    }

    fn apply_advance(
        &mut self,
        at: Time<D>,
        expected_revision: NetworkRevision,
        input: InputDelta<D>,
    ) -> Result<TransactionResult<D>, RuntimeFailure<D>> {
        let MachineStatus::Ready { now } = self.store.status else {
            return Err(RuntimeFailure::new(
                RuntimeFailureEvidence::DeltaBeforeInitialization,
            ));
        };
        if expected_revision != self.store.revision {
            return Err(RuntimeFailure::new(RuntimeFailureEvidence::StaleRevision {
                expected: expected_revision,
                actual: self.store.revision,
            }));
        }
        validate_delta_binding::<D>(&self.compiled, &input)?;
        if at <= now {
            return Err(RuntimeFailure::new(
                RuntimeFailureEvidence::TimeNotStrictlyIncreasing {
                    current_ticks: now.ticks(),
                    requested_ticks: at.ticks(),
                },
            ));
        }

        enforce_reaction_budgets::<D>(&self.policy, &self.compiled)?;

        let revision = self.store.revision;
        let explicit_levels = input.into_levels();
        let mut levels = self.store.external_levels.clone();
        levels.extend(explicit_levels.iter().map(|(key, value)| (*key, *value)));
        let Some(evaluation) = self.compiled.evaluate_full(&levels) else {
            panic!("ready machine and compatible delta must provide complete external facts");
        };
        let previous_provenance = match self.store.provenance.as_ref() {
            Some(provenance) => provenance,
            None => panic!("ready machine must retain committed provenance"),
        };
        let ProvenanceBuild {
            provenance,
            input_causes,
            output_causes,
        } = build_ready_provenance(
            self.compiled.network_key(),
            self.compiled.fingerprint(),
            revision,
            at,
            &levels,
            &explicit_levels,
            &evaluation,
            previous_provenance,
            &self.store.input_causes,
            &self.store.output_causes,
            &self.store.output_baselines,
        );
        let provenance_growth = provenance.len().saturating_sub(previous_provenance.len());
        enforce_budget::<D>(
            &self.policy,
            RuntimePolicyLimit::MaxRequiredProvenanceGrowth,
            count_as_u64(provenance_growth),
        )?;

        let output_events = changed_events(
            at,
            revision,
            &self.store.output_baselines,
            &evaluation.external_outputs,
            &output_causes,
        );
        enforce_budget::<D>(
            &self.policy,
            RuntimePolicyLimit::MaxEventsCreatedPerTransaction,
            count_as_u64(output_events.len()),
        )?;
        let result = TransactionResult {
            requested_time: at,
            before_revision: revision,
            after_revision: revision,
            output_events,
            provenance: provenance.clone(),
        };

        publish_candidate(
            self,
            at,
            levels,
            evaluation,
            input_causes,
            output_causes,
            provenance,
        );
        Ok(result)
    }
}

fn validate_snapshot_binding<D>(
    compiled: &crate::CompiledNetwork<D>,
    input: &InputSnapshot<D>,
) -> Result<(), RuntimeFailure<D>> {
    if input.network_key() != compiled.network_key() {
        return Err(RuntimeFailure::new(RuntimeFailureEvidence::WrongNetwork {
            expected_key: compiled.network_key(),
            actual_key: input.network_key(),
            expected_fingerprint: compiled.fingerprint(),
            actual_fingerprint: input.network_fingerprint(),
        }));
    }
    if input.input_schema_fingerprint() != compiled.input_schema_fingerprint() {
        return Err(RuntimeFailure::new(
            RuntimeFailureEvidence::ForeignInputSchema {
                expected: compiled.input_schema_fingerprint(),
                actual: input.input_schema_fingerprint(),
            },
        ));
    }
    if input.network_fingerprint() != compiled.fingerprint() {
        return Err(RuntimeFailure::new(RuntimeFailureEvidence::WrongNetwork {
            expected_key: compiled.network_key(),
            actual_key: input.network_key(),
            expected_fingerprint: compiled.fingerprint(),
            actual_fingerprint: input.network_fingerprint(),
        }));
    }
    Ok(())
}

fn validate_delta_binding<D>(
    compiled: &crate::CompiledNetwork<D>,
    input: &InputDelta<D>,
) -> Result<(), RuntimeFailure<D>> {
    if input.network_key() != compiled.network_key() {
        return Err(RuntimeFailure::new(RuntimeFailureEvidence::WrongNetwork {
            expected_key: compiled.network_key(),
            actual_key: input.network_key(),
            expected_fingerprint: compiled.fingerprint(),
            actual_fingerprint: input.network_fingerprint(),
        }));
    }
    let fingerprint_matches = input.network_fingerprint() == compiled.fingerprint();
    let schema_matches = input.input_schema_fingerprint() == compiled.input_schema_fingerprint();
    if !fingerprint_matches && !schema_matches {
        return Err(RuntimeFailure::new(
            RuntimeFailureEvidence::StaleInputSchema {
                expected: compiled.input_schema_fingerprint(),
                actual: input.input_schema_fingerprint(),
            },
        ));
    }
    if !schema_matches {
        return Err(RuntimeFailure::new(
            RuntimeFailureEvidence::ForeignInputSchema {
                expected: compiled.input_schema_fingerprint(),
                actual: input.input_schema_fingerprint(),
            },
        ));
    }
    if !fingerprint_matches {
        return Err(RuntimeFailure::new(RuntimeFailureEvidence::WrongNetwork {
            expected_key: compiled.network_key(),
            actual_key: input.network_key(),
            expected_fingerprint: compiled.fingerprint(),
            actual_fingerprint: input.network_fingerprint(),
        }));
    }
    Ok(())
}

fn enforce_reaction_budgets<D>(
    policy: &RuntimePolicy,
    compiled: &crate::CompiledNetwork<D>,
) -> Result<(), RuntimeFailure<D>> {
    enforce_budget::<D>(policy, RuntimePolicyLimit::MaxInternalReactions, 1)?;
    enforce_budget::<D>(
        policy,
        RuntimePolicyLimit::MaxEvaluatedOperations,
        count_as_u64(compiled.operation_count()),
    )
}

fn publish_candidate<D>(
    machine: &mut Machine<D>,
    at: Time<D>,
    levels: BTreeMap<ExternalInputKey<Level>, LogicLevel>,
    evaluation: FullEvaluation,
    input_causes: BTreeMap<ExternalInputKey<Level>, CauseRef>,
    output_causes: BTreeMap<ExternalOutputKey<Level>, CauseRef>,
    provenance: ProvenanceView<D>,
) {
    // SPEC: docs/specs/processor_and_runtime_architecture.md §50 "Reference execution strategy"
    // Every fallible step precedes replacement of the complete private candidate.
    let mut candidate = machine.store.clone();
    candidate.status = MachineStatus::Ready { now: at };
    candidate.external_levels = levels;
    candidate.settled_levels = evaluation.values;
    candidate.output_baselines = evaluation.external_outputs;
    candidate.input_causes = input_causes;
    candidate.output_causes = output_causes;
    candidate.provenance = Some(provenance);
    machine.store = candidate;
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

fn build_initialization_provenance<D>(
    network_key: NetworkKey,
    fingerprint: NetworkFingerprint,
    revision: NetworkRevision,
    at: Time<D>,
    levels: &BTreeMap<ExternalInputKey<Level>, LogicLevel>,
    evaluation: &FullEvaluation,
) -> ProvenanceBuild<D> {
    let scope = provenance_scope(network_key, fingerprint, revision, at, levels);
    let mut records = Vec::new();
    let transaction_cause = push_record(
        scope,
        &mut records,
        ProvenanceRecord::InitializationTransaction { at, revision },
    );
    let input_causes = levels
        .iter()
        .map(|(input, value)| {
            let cause = push_record(
                scope,
                &mut records,
                ProvenanceRecord::ExternalObservation {
                    input: *input,
                    value: *value,
                },
            );
            (*input, cause)
        })
        .collect::<BTreeMap<_, _>>();
    let output_causes = append_evaluation_provenance(
        scope,
        &mut records,
        transaction_cause,
        evaluation,
        &input_causes,
        None,
        None,
    );

    ProvenanceBuild {
        provenance: ProvenanceView {
            scope,
            records: Arc::new(records),
        },
        input_causes,
        output_causes,
    }
}

#[allow(clippy::too_many_arguments)]
fn build_ready_provenance<D>(
    network_key: NetworkKey,
    fingerprint: NetworkFingerprint,
    revision: NetworkRevision,
    at: Time<D>,
    levels: &BTreeMap<ExternalInputKey<Level>, LogicLevel>,
    explicit_levels: &BTreeMap<ExternalInputKey<Level>, LogicLevel>,
    evaluation: &FullEvaluation,
    previous: &ProvenanceView<D>,
    previous_input_causes: &BTreeMap<ExternalInputKey<Level>, CauseRef>,
    previous_output_causes: &BTreeMap<ExternalOutputKey<Level>, CauseRef>,
    previous_output_baselines: &BTreeMap<ExternalOutputKey<Level>, LogicLevel>,
) -> ProvenanceBuild<D> {
    let scope = provenance_scope(network_key, fingerprint, revision, at, levels);
    let mut records = previous
        .records
        .iter()
        .map(|record| remap_record(record, scope))
        .collect::<Vec<_>>();
    let transaction_cause = push_record(
        scope,
        &mut records,
        ProvenanceRecord::ReadyTransaction { at, revision },
    );
    let mut input_causes = previous_input_causes
        .iter()
        .map(|(input, cause)| (*input, remap_cause(*cause, scope)))
        .collect::<BTreeMap<_, _>>();
    for (input, value) in explicit_levels {
        let cause = push_record(
            scope,
            &mut records,
            ProvenanceRecord::ExternalObservation {
                input: *input,
                value: *value,
            },
        );
        input_causes.insert(*input, cause);
    }
    let remapped_output_causes = previous_output_causes
        .iter()
        .map(|(output, cause)| (*output, remap_cause(*cause, scope)))
        .collect::<BTreeMap<_, _>>();
    let output_causes = append_evaluation_provenance(
        scope,
        &mut records,
        transaction_cause,
        evaluation,
        &input_causes,
        Some(&remapped_output_causes),
        Some(previous_output_baselines),
    );

    ProvenanceBuild {
        provenance: ProvenanceView {
            scope,
            records: Arc::new(records),
        },
        input_causes,
        output_causes,
    }
}

fn append_evaluation_provenance<D>(
    scope: [u8; 16],
    records: &mut Vec<ProvenanceRecord<D>>,
    transaction_cause: CauseRef,
    evaluation: &FullEvaluation,
    input_causes: &BTreeMap<ExternalInputKey<Level>, CauseRef>,
    previous_output_causes: Option<&BTreeMap<ExternalOutputKey<Level>, CauseRef>>,
    previous_output_baselines: Option<&BTreeMap<ExternalOutputKey<Level>, LogicLevel>>,
) -> BTreeMap<ExternalOutputKey<Level>, CauseRef> {
    let mut operation_causes = Vec::with_capacity(evaluation.causes.len());
    let mut output_causes = BTreeMap::new();

    for cause in &evaluation.causes {
        let resolved = match cause {
            EvaluationCause::ExternalInput(input) => {
                let Some(cause) = input_causes.get(input).copied() else {
                    panic!("evaluated external input must retain one authoritative cause");
                };
                cause
            }
            EvaluationCause::Constant(node) => push_record(
                scope,
                records,
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
                    records,
                    ProvenanceRecord::Derived {
                        subject: ProvenanceSubject::Node(*node),
                        supporters,
                    },
                )
            }
            EvaluationCause::Alias(source) => operation_cause(&operation_causes, *source),
            EvaluationCause::ExternalOutput { output, source } => {
                let unchanged_cause = previous_output_baselines
                    .and_then(|baselines| baselines.get(output))
                    .zip(evaluation.external_outputs.get(output))
                    .filter(|(before, after)| before == after)
                    .and(previous_output_causes)
                    .and_then(|causes| causes.get(output))
                    .copied();
                if let Some(cause) = unchanged_cause {
                    output_causes.insert(*output, cause);
                    operation_causes.push(cause);
                    continue;
                }
                let mut supporters = vec![
                    transaction_cause,
                    operation_cause(&operation_causes, *source),
                ];
                supporters.sort();
                supporters.dedup();
                let reference = push_record(
                    scope,
                    records,
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
    output_causes
}

fn remap_cause(cause: CauseRef, scope: [u8; 16]) -> CauseRef {
    CauseRef {
        scope,
        ordinal: cause.ordinal,
    }
}

fn remap_record<D>(record: &ProvenanceRecord<D>, scope: [u8; 16]) -> ProvenanceRecord<D> {
    match record {
        ProvenanceRecord::InitializationTransaction { at, revision } => {
            ProvenanceRecord::InitializationTransaction {
                at: *at,
                revision: *revision,
            }
        }
        ProvenanceRecord::ReadyTransaction { at, revision } => ProvenanceRecord::ReadyTransaction {
            at: *at,
            revision: *revision,
        },
        ProvenanceRecord::ExternalObservation { input, value } => {
            ProvenanceRecord::ExternalObservation {
                input: *input,
                value: *value,
            }
        }
        ProvenanceRecord::Derived {
            subject,
            supporters,
        } => ProvenanceRecord::Derived {
            subject: *subject,
            supporters: supporters
                .iter()
                .map(|cause| remap_cause(*cause, scope))
                .collect(),
        },
    }
}

fn push_record<D>(
    scope: [u8; 16],
    records: &mut Vec<ProvenanceRecord<D>>,
    record: ProvenanceRecord<D>,
) -> CauseRef {
    let ordinal = match u32::try_from(records.len()) {
        Ok(value) => value,
        Err(_) => panic!("transaction provenance exceeds the supported reference space"),
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

fn changed_events<D>(
    at: Time<D>,
    revision: NetworkRevision,
    previous: &BTreeMap<ExternalOutputKey<Level>, LogicLevel>,
    settled: &BTreeMap<ExternalOutputKey<Level>, LogicLevel>,
    causes: &BTreeMap<ExternalOutputKey<Level>, CauseRef>,
) -> Vec<OutputEvent<D>> {
    settled
        .iter()
        .filter_map(|(output, to)| {
            let Some(from) = previous.get(output).copied() else {
                panic!("ready transaction must preserve every external output baseline");
            };
            if from == *to {
                return None;
            }
            let Some(cause) = causes.get(output).copied() else {
                panic!("every changed external output must retain one committed cause");
            };
            Some(OutputEvent::LevelChanged {
                output: *output,
                from,
                to: *to,
                at,
                cause,
                revision,
            })
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

    fn initialized_machine(
        compiled: &crate::CompiledNetwork<()>,
        value: LogicLevel,
    ) -> crate::Machine<()> {
        let mut machine = compiled.spawn(policy_with([10, 100, 0, 100, 1_000]));
        machine
            .apply(Transaction::initialize(
                crate::time::Time::from_ticks(10),
                machine.revision(),
                compiled
                    .input_snapshot()
                    .set(ExternalInputKey::from_u128(1), value)
                    .and_then(crate::InputSnapshotBuilder::finish)
                    .unwrap_or_else(|_| panic!("snapshot must build")),
            ))
            .unwrap_or_else(|failure| panic!("initialization must succeed: {failure}"));
        machine
    }

    #[derive(Debug, PartialEq, Eq)]
    struct MachineObservation {
        network_key: NetworkKey,
        network_fingerprint: crate::NetworkFingerprint,
        input_schema_fingerprint: crate::InputSchemaFingerprint,
        policy_id: crate::RuntimePolicyId,
        status: MachineStatus<()>,
        revision: NetworkRevision,
        external_levels: std::collections::BTreeMap<ExternalInputKey<Level>, LogicLevel>,
        settled_levels: Vec<LogicLevel>,
        output_baselines: std::collections::BTreeMap<ExternalOutputKey<Level>, LogicLevel>,
        input_causes: std::collections::BTreeMap<ExternalInputKey<Level>, CauseRef>,
        output_causes: std::collections::BTreeMap<ExternalOutputKey<Level>, CauseRef>,
        provenance: Option<([u8; 16], usize, usize)>,
    }

    fn observe(machine: &crate::Machine<()>) -> MachineObservation {
        MachineObservation {
            network_key: machine.compiled.network_key(),
            network_fingerprint: machine.compiled.fingerprint(),
            input_schema_fingerprint: machine.compiled.input_schema_fingerprint(),
            policy_id: machine.policy.id(),
            status: machine.store.status,
            revision: machine.store.revision,
            external_levels: machine.store.external_levels.clone(),
            settled_levels: machine.store.settled_levels.clone(),
            output_baselines: machine.store.output_baselines.clone(),
            input_causes: machine.store.input_causes.clone(),
            output_causes: machine.store.output_causes.clone(),
            provenance: machine.store.provenance.as_ref().map(|view| {
                (
                    view.scope,
                    view.len(),
                    std::sync::Arc::as_ptr(&view.records) as usize,
                )
            }),
        }
    }

    fn complex_compiled(reverse_claims: bool) -> crate::CompiledNetwork<()> {
        complex_compiled_with_name(reverse_claims, None)
    }

    fn complex_compiled_with_name(
        reverse_claims: bool,
        network_name: Option<&str>,
    ) -> crate::CompiledNetwork<()> {
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
            DiagnosticMeta {
                name: network_name.map(String::from),
                ..DiagnosticMeta::default()
            },
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
            match view
                .inspect(cause)
                .unwrap_or_else(|failure| panic!("cause must resolve: {failure}"))
            {
                CauseInspection::Derived { supporters, .. } => {
                    assert!(
                        !supporters.is_empty(),
                        "derived provenance must terminate in authoritative roots"
                    );
                    for supporter in supporters {
                        visit(view, *supporter, visiting, visited);
                    }
                }
                CauseInspection::InitializationTransaction { .. }
                | CauseInspection::ReadyTransaction { .. }
                | CauseInspection::ExternalObservation { .. } => {}
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
        assert_eq!(machine.runtime_policy_id(), policy().id());
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
        let before = observe(&machine);

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
        assert_eq!(observe(&machine), before);
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
                    OutputEvent::LevelChanged { .. } => {
                        panic!("initialization must not emit level changes")
                    }
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
    fn equivalent_initializations_differ_only_by_requested_time() {
        let compiled = complex_compiled(false);
        let input = || {
            compiled
                .input_snapshot()
                .set(ExternalInputKey::from_u128(1), LogicLevel::Low)
                .and_then(|builder| builder.set(ExternalInputKey::from_u128(2), LogicLevel::High))
                .and_then(crate::InputSnapshotBuilder::finish)
                .unwrap_or_else(|_| panic!("snapshot must build"))
        };
        let expected_policy = policy_with([1, 9, 0, 3, 8]);
        let expected_policy_id = expected_policy.id();
        let mut at_zero = compiled.spawn(expected_policy.clone());
        let mut later = compiled.spawn(expected_policy);

        let zero_result = at_zero
            .apply(Transaction::initialize(
                crate::time::Time::from_ticks(0),
                at_zero.revision(),
                input(),
            ))
            .unwrap_or_else(|failure| panic!("zero-time initialization must succeed: {failure}"));
        let later_result = later
            .apply(Transaction::initialize(
                crate::time::Time::from_ticks(91),
                later.revision(),
                input(),
            ))
            .unwrap_or_else(|failure| {
                panic!("nonzero-time initialization must succeed: {failure}")
            });

        assert_eq!(at_zero.now(), Some(crate::time::Time::from_ticks(0)));
        assert_eq!(later.now(), Some(crate::time::Time::from_ticks(91)));
        assert_eq!(at_zero.revision(), later.revision());
        assert_eq!(at_zero.runtime_policy_id(), expected_policy_id);
        assert_eq!(later.runtime_policy_id(), expected_policy_id);
        assert_eq!(at_zero.store.external_levels, later.store.external_levels);
        assert_eq!(at_zero.store.settled_levels, later.store.settled_levels);
        assert_eq!(at_zero.store.output_baselines, later.store.output_baselines);
        assert_eq!(
            zero_result.output_events().len(),
            later_result.output_events().len()
        );
        for (zero, nonzero) in zero_result
            .output_events()
            .iter()
            .zip(later_result.output_events())
        {
            match (zero, nonzero) {
                (
                    OutputEvent::LevelEstablished {
                        output: zero_output,
                        value: zero_value,
                        revision: zero_revision,
                        ..
                    },
                    OutputEvent::LevelEstablished {
                        output: later_output,
                        value: later_value,
                        revision: later_revision,
                        ..
                    },
                ) => {
                    assert_eq!(zero_output, later_output);
                    assert_eq!(zero_value, later_value);
                    assert_eq!(zero_revision, later_revision);
                }
                _ => panic!("initialization comparison must contain establishments only"),
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
        let before = observe(&stale);
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
        assert_eq!(observe(&stale), before);
        assert_eq!(stale.revision(), NetworkRevision::from_value(9));

        let same_schema_other_topology = compiled(10, 31);
        let mut fingerprint_machine = local.spawn(policy());
        let before = observe(&fingerprint_machine);
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
        assert_eq!(observe(&fingerprint_machine), before);

        let other_schema = compiled_with_input(10, 2, 30);
        let mut schema_machine = local.spawn(policy());
        let before = observe(&schema_machine);
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
        assert_eq!(observe(&schema_machine), before);

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
        let before = observe(&ready);
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
        assert_eq!(observe(&ready), before);
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
            let before = observe(&machine);
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
            assert_eq!(observe(&machine), before);
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

    #[test]
    fn transaction_inspection_is_lifecycle_aware_and_delta_requires_ready() {
        let compiled = compiled(10, 30);
        let input = ExternalInputKey::<Level>::from_u128(1);
        let snapshot = compiled
            .input_snapshot()
            .set(input, LogicLevel::Low)
            .and_then(crate::InputSnapshotBuilder::finish)
            .unwrap_or_else(|_| panic!("snapshot must build"));
        let initialize = Transaction::initialize(
            crate::time::Time::from_ticks(10),
            NetworkRevision::from_value(0),
            snapshot,
        );
        assert!(initialize.initialization_input().is_some());
        assert!(initialize.advance_input().is_none());

        let delta = compiled
            .input_delta()
            .set(input, LogicLevel::High)
            .and_then(crate::InputDeltaBuilder::finish)
            .unwrap_or_else(|_| panic!("delta must build"));
        let advance = Transaction::advance(
            crate::time::Time::from_ticks(11),
            NetworkRevision::from_value(0),
            delta,
        );
        assert!(advance.initialization_input().is_none());
        assert!(advance.advance_input().is_some());

        let mut machine = compiled.spawn(policy_with([10, 100, 0, 100, 1_000]));
        let before = observe(&machine);
        let failure = machine.apply(advance).unwrap_err();
        assert_eq!(failure.code(), "lifecycle.delta_before_initialization");
        assert!(matches!(
            failure.evidence(),
            RuntimeFailureEvidence::DeltaBeforeInitialization
        ));
        assert_eq!(failure.responsibility(), Responsibility::CallerInput);
        assert_eq!(observe(&machine), before);
    }

    #[test]
    fn ready_advancement_overlays_delta_and_emits_only_genuine_changes() {
        let compiled = compiled(10, 30);
        let input = ExternalInputKey::<Level>::from_u128(1);
        let output = ExternalOutputKey::<Level>::from_u128(30);
        let mut machine = compiled.spawn(policy_with([10, 100, 0, 100, 1_000]));
        let initialization = machine
            .apply(Transaction::initialize(
                crate::time::Time::from_ticks(10),
                machine.revision(),
                compiled
                    .input_snapshot()
                    .set(input, LogicLevel::Low)
                    .and_then(crate::InputSnapshotBuilder::finish)
                    .unwrap_or_else(|_| panic!("snapshot must build")),
            ))
            .unwrap_or_else(|failure| panic!("initialization must succeed: {failure}"));
        let initialization_cause = match initialization.output_events() {
            [OutputEvent::LevelEstablished { cause, .. }] => *cause,
            _ => panic!("initialization must establish one output"),
        };
        let revision = machine.revision();
        let policy_id = machine.runtime_policy_id();

        let changed = machine
            .apply(Transaction::advance(
                crate::time::Time::from_ticks(20),
                revision,
                compiled
                    .input_delta()
                    .set(input, LogicLevel::High)
                    .and_then(crate::InputDeltaBuilder::finish)
                    .unwrap_or_else(|_| panic!("delta must build")),
            ))
            .unwrap_or_else(|failure| panic!("advance must succeed: {failure}"));
        let changed_cause = match changed.output_events() {
            [
                OutputEvent::LevelChanged {
                    output: actual_output,
                    from,
                    to,
                    at,
                    cause,
                    revision: actual_revision,
                },
            ] => {
                assert_eq!(*actual_output, output);
                assert_eq!(*from, LogicLevel::Low);
                assert_eq!(*to, LogicLevel::High);
                assert_eq!(*at, crate::time::Time::from_ticks(20));
                assert_eq!(*actual_revision, revision);
                *cause
            }
            _ => panic!("one changed output must be published"),
        };
        recursively_assert_acyclic(changed.provenance(), changed_cause);
        assert_eq!(machine.external_level(input), Some(LogicLevel::High));
        assert_eq!(machine.output_level(output), Some(LogicLevel::High));
        assert_eq!(machine.output_cause(output), Some(changed_cause));
        assert_eq!(machine.now(), Some(crate::time::Time::from_ticks(20)));
        assert_eq!(machine.revision(), revision);
        assert_eq!(machine.runtime_policy_id(), policy_id);
        assert_eq!(changed.before_revision(), changed.after_revision());

        let reasserted = machine
            .apply(Transaction::advance(
                crate::time::Time::from_ticks(21),
                revision,
                compiled
                    .input_delta()
                    .set(input, LogicLevel::High)
                    .and_then(crate::InputDeltaBuilder::finish)
                    .unwrap_or_else(|_| panic!("delta must build")),
            ))
            .unwrap_or_else(|failure| panic!("reassertion must succeed: {failure}"));
        assert!(reasserted.output_events().is_empty());
        let reasserted_cause = machine
            .output_cause(output)
            .unwrap_or_else(|| panic!("unchanged output must retain a cause"));
        assert!(reasserted.provenance().inspect(reasserted_cause).is_ok());

        let empty = machine
            .apply(Transaction::advance(
                crate::time::Time::from_ticks(22),
                revision,
                compiled
                    .input_delta()
                    .finish()
                    .unwrap_or_else(|_| panic!("empty delta must build")),
            ))
            .unwrap_or_else(|failure| panic!("empty advance must succeed: {failure}"));
        assert!(empty.output_events().is_empty());
        assert_eq!(machine.external_level(input), Some(LogicLevel::High));
        let empty_cause = machine
            .output_cause(output)
            .unwrap_or_else(|| panic!("unchanged output must retain a cause"));
        assert!(empty.provenance().inspect(empty_cause).is_ok());
        assert!(
            initialization
                .provenance()
                .inspect(initialization_cause)
                .is_ok()
        );
        assert!(changed.provenance().inspect(changed_cause).is_ok());
    }

    #[test]
    fn partial_and_complete_deltas_fully_resettle_reconvergent_graphs() {
        let compiled = complex_compiled(false);
        let first = ExternalInputKey::<Level>::from_u128(1);
        let second = ExternalInputKey::<Level>::from_u128(2);
        let mut machine = compiled.spawn(policy_with([10, 100, 0, 100, 1_000]));
        machine
            .apply(Transaction::initialize(
                crate::time::Time::from_ticks(4),
                machine.revision(),
                compiled
                    .input_snapshot()
                    .set(first, LogicLevel::Low)
                    .and_then(|builder| builder.set(second, LogicLevel::High))
                    .and_then(crate::InputSnapshotBuilder::finish)
                    .unwrap_or_else(|_| panic!("snapshot must build")),
            ))
            .unwrap_or_else(|failure| panic!("initialization must succeed: {failure}"));

        let partial = machine
            .apply(Transaction::advance(
                crate::time::Time::from_ticks(9),
                machine.revision(),
                compiled
                    .input_delta()
                    .set(first, LogicLevel::High)
                    .and_then(crate::InputDeltaBuilder::finish)
                    .unwrap_or_else(|_| panic!("partial delta must build")),
            ))
            .unwrap_or_else(|failure| panic!("partial advance must succeed: {failure}"));
        assert_eq!(machine.external_level(first), Some(LogicLevel::High));
        assert_eq!(machine.external_level(second), Some(LogicLevel::High));
        assert_eq!(machine.store.settled_levels.len(), 9);
        assert_eq!(
            partial
                .output_events()
                .iter()
                .filter_map(|event| match event {
                    OutputEvent::LevelChanged { output, .. } => Some(*output),
                    OutputEvent::LevelEstablished { .. } => None,
                })
                .collect::<Vec<_>>(),
            vec![
                ExternalOutputKey::from_u128(10),
                ExternalOutputKey::from_u128(50),
            ]
        );

        let complete = machine
            .apply(Transaction::advance(
                crate::time::Time::from_ticks(15),
                machine.revision(),
                compiled
                    .input_delta()
                    .set(second, LogicLevel::Low)
                    .and_then(|builder| builder.set(first, LogicLevel::Low))
                    .and_then(crate::InputDeltaBuilder::finish)
                    .unwrap_or_else(|_| panic!("complete delta must build")),
            ))
            .unwrap_or_else(|failure| panic!("complete advance must succeed: {failure}"));
        assert_eq!(machine.external_level(first), Some(LogicLevel::Low));
        assert_eq!(machine.external_level(second), Some(LogicLevel::Low));
        assert_eq!(machine.store.settled_levels.len(), 9);
        for event in complete.output_events() {
            if let OutputEvent::LevelChanged { cause, .. } = event {
                recursively_assert_acyclic(complete.provenance(), *cause);
            }
        }
    }

    #[test]
    fn ready_rejections_are_structured_and_preserve_complete_machine() {
        let local = compiled(10, 30);
        let foreign_network = compiled(11, 30);
        let other_schema = compiled_with_input(10, 2, 30);
        let input = ExternalInputKey::<Level>::from_u128(1);

        for requested in [9, 10] {
            let mut machine = initialized_machine(&local, LogicLevel::Low);
            let before = observe(&machine);
            let failure = machine
                .apply(Transaction::advance(
                    crate::time::Time::from_ticks(requested),
                    machine.revision(),
                    local
                        .input_delta()
                        .finish()
                        .unwrap_or_else(|_| panic!("delta must build")),
                ))
                .unwrap_err();
            assert_eq!(failure.code(), "runtime.time_not_strictly_increasing");
            assert!(matches!(
                failure.evidence(),
                RuntimeFailureEvidence::TimeNotStrictlyIncreasing { .. }
            ));
            assert_eq!(observe(&machine), before);
        }

        let mut stale_revision = initialized_machine(&local, LogicLevel::Low);
        let expected = stale_revision.revision();
        stale_revision.store.revision = NetworkRevision::from_value(7);
        let before = observe(&stale_revision);
        let failure = stale_revision
            .apply(Transaction::advance(
                crate::time::Time::from_ticks(11),
                expected,
                local
                    .input_delta()
                    .finish()
                    .unwrap_or_else(|_| panic!("delta must build")),
            ))
            .unwrap_err();
        assert!(matches!(
            failure.evidence(),
            RuntimeFailureEvidence::StaleRevision { .. }
        ));
        assert_eq!(observe(&stale_revision), before);

        let mut wrong_network = initialized_machine(&local, LogicLevel::Low);
        let before = observe(&wrong_network);
        let failure = wrong_network
            .apply(Transaction::advance(
                crate::time::Time::from_ticks(11),
                wrong_network.revision(),
                foreign_network
                    .input_delta()
                    .set(input, LogicLevel::High)
                    .and_then(crate::InputDeltaBuilder::finish)
                    .unwrap_or_else(|_| panic!("delta must build")),
            ))
            .unwrap_err();
        assert!(matches!(
            failure.evidence(),
            RuntimeFailureEvidence::WrongNetwork { .. }
        ));
        assert_eq!(observe(&wrong_network), before);

        let mut stale_schema = initialized_machine(&local, LogicLevel::Low);
        let before = observe(&stale_schema);
        let failure = stale_schema
            .apply(Transaction::advance(
                crate::time::Time::from_ticks(11),
                stale_schema.revision(),
                other_schema
                    .input_delta()
                    .set(ExternalInputKey::from_u128(2), LogicLevel::High)
                    .and_then(crate::InputDeltaBuilder::finish)
                    .unwrap_or_else(|_| panic!("delta must build")),
            ))
            .unwrap_err();
        assert_eq!(failure.code(), "input.stale_schema");
        assert!(matches!(
            failure.evidence(),
            RuntimeFailureEvidence::StaleInputSchema { .. }
        ));
        assert_eq!(observe(&stale_schema), before);

        let mixed = local
            .input_delta()
            .finish()
            .unwrap_or_else(|_| panic!("delta must build"))
            .with_test_bindings(local.fingerprint(), other_schema.input_schema_fingerprint());
        let mut foreign_schema = initialized_machine(&local, LogicLevel::Low);
        let before = observe(&foreign_schema);
        let failure = foreign_schema
            .apply(Transaction::advance(
                crate::time::Time::from_ticks(11),
                foreign_schema.revision(),
                mixed,
            ))
            .unwrap_err();
        assert_eq!(failure.code(), "input.foreign_schema");
        assert!(matches!(
            failure.evidence(),
            RuntimeFailureEvidence::ForeignInputSchema { .. }
        ));
        assert_eq!(observe(&foreign_schema), before);
    }

    #[test]
    fn equivalent_ready_batches_ignore_insertion_authored_and_metadata_order() {
        let mut outcomes = Vec::new();
        for (reverse_claims, network_name, reverse_delta) in
            [(false, None, false), (true, Some("renamed only"), true)]
        {
            let compiled = complex_compiled_with_name(reverse_claims, network_name);
            let first = ExternalInputKey::<Level>::from_u128(1);
            let second = ExternalInputKey::<Level>::from_u128(2);
            let mut machine = compiled.spawn(policy_with([10, 100, 0, 100, 1_000]));
            machine
                .apply(Transaction::initialize(
                    crate::time::Time::from_ticks(3),
                    machine.revision(),
                    compiled
                        .input_snapshot()
                        .set(first, LogicLevel::Low)
                        .and_then(|builder| builder.set(second, LogicLevel::High))
                        .and_then(crate::InputSnapshotBuilder::finish)
                        .unwrap_or_else(|_| panic!("snapshot must build")),
                ))
                .unwrap_or_else(|failure| panic!("initialization must succeed: {failure}"));
            let delta = if reverse_delta {
                compiled
                    .input_delta()
                    .set(second, LogicLevel::Low)
                    .and_then(|builder| builder.set(first, LogicLevel::High))
            } else {
                compiled
                    .input_delta()
                    .set(first, LogicLevel::High)
                    .and_then(|builder| builder.set(second, LogicLevel::Low))
            }
            .and_then(crate::InputDeltaBuilder::finish)
            .unwrap_or_else(|_| panic!("delta must build"));
            let result = machine
                .apply(Transaction::advance(
                    crate::time::Time::from_ticks(8),
                    machine.revision(),
                    delta,
                ))
                .unwrap_or_else(|failure| panic!("advance must succeed: {failure}"));
            let events = result
                .output_events()
                .iter()
                .filter_map(|event| match event {
                    OutputEvent::LevelChanged {
                        output,
                        from,
                        to,
                        cause,
                        ..
                    } => Some((*output, *from, *to, *cause)),
                    OutputEvent::LevelEstablished { .. } => None,
                })
                .collect::<Vec<_>>();
            outcomes.push((
                compiled.fingerprint(),
                compiled.input_schema_fingerprint(),
                machine.store.external_levels.clone(),
                machine.store.settled_levels.clone(),
                machine.store.output_baselines.clone(),
                events,
            ));
        }
        assert_eq!(outcomes[0], outcomes[1]);
    }

    #[test]
    fn ready_budget_boundaries_cover_below_exact_and_above() {
        let cases = [
            (RuntimePolicyLimit::MaxInternalReactions, 1_u64),
            (RuntimePolicyLimit::MaxEvaluatedOperations, 2_u64),
            (RuntimePolicyLimit::MaxEventsCreatedPerTransaction, 1_u64),
            (RuntimePolicyLimit::MaxRequiredProvenanceGrowth, 3_u64),
        ];
        for (budget, consumed) in cases {
            for limit in [consumed - 1, consumed, consumed + 1] {
                let compiled = compiled(10, 30);
                let mut machine = initialized_machine(&compiled, LogicLevel::Low);
                let mut limits = [10, 100, 0, 100, 1_000];
                match budget {
                    RuntimePolicyLimit::MaxInternalReactions => limits[0] = limit,
                    RuntimePolicyLimit::MaxEvaluatedOperations => limits[1] = limit,
                    RuntimePolicyLimit::MaxEventsCreatedPerTransaction => limits[3] = limit,
                    RuntimePolicyLimit::MaxRequiredProvenanceGrowth => limits[4] = limit,
                    RuntimePolicyLimit::MaxPendingEvents => {
                        panic!("pending-event accounting is outside this slice")
                    }
                }
                machine.policy = policy_with(limits);
                let before = observe(&machine);
                let result = machine.apply(Transaction::advance(
                    crate::time::Time::from_ticks(11),
                    machine.revision(),
                    compiled
                        .input_delta()
                        .set(ExternalInputKey::from_u128(1), LogicLevel::High)
                        .and_then(crate::InputDeltaBuilder::finish)
                        .unwrap_or_else(|_| panic!("delta must build")),
                ));
                if limit < consumed {
                    let failure = result.unwrap_err();
                    assert_eq!(
                        failure.evidence(),
                        &RuntimeFailureEvidence::BudgetExceeded {
                            budget,
                            limit,
                            consumed,
                        }
                    );
                    assert_eq!(observe(&machine), before);
                } else {
                    result.unwrap_or_else(|failure| {
                        panic!("within-boundary advance must succeed: {failure}")
                    });
                    assert_eq!(machine.now(), Some(crate::time::Time::from_ticks(11)));
                }
            }
        }
    }
}
