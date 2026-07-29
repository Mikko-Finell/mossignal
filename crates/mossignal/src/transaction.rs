//! Restricted initialization and ready-machine Level and Pulse transactions.

use crate::compile::{EvaluationCause, EvaluationFailure, FullEvaluation};
use crate::diagnostics::{Responsibility, Severity};
use crate::identity::{InputSchemaFingerprint, NetworkFingerprint};
use crate::input::{InputDelta, InputSnapshot};
use crate::key::{ExternalInputKey, ExternalOutputKey, InPortKey, NetworkKey, NodeKey};
use crate::machine::{
    Machine, MachineStatus, NetworkRevision, PendingEventKey, PendingPulseDelay, Schedule,
};
use crate::policy::{RuntimePolicy, RuntimePolicyLimit};
use crate::signal::{Level, LogicLevel, Pulse, PulseCount};
use crate::time::{Span, Time};
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
    PulseCountOverflow {
        node: NodeKey,
    },
    TimeOverflow {
        node: NodeKey,
        origin_ticks: u64,
        delay_ticks: u64,
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
            RuntimeFailureEvidence::PulseCountOverflow { .. } => "runtime.pulse_count_overflow",
            RuntimeFailureEvidence::TimeOverflow { .. } => "runtime.time_overflow",
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
            RuntimeFailureEvidence::PulseCountOverflow { .. }
            | RuntimeFailureEvidence::TimeOverflow { .. } => Responsibility::SemanticRejection,
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
    PulseExternalOutput(ExternalOutputKey<Pulse>),
}

/// One stable Merge input-port contribution to a simultaneous pulse result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PulseContribution {
    port: InPortKey<Pulse>,
    count: PulseCount,
    cause: CauseRef,
}

impl PulseContribution {
    #[must_use]
    pub const fn port(&self) -> InPortKey<Pulse> {
        self.port
    }

    #[must_use]
    pub const fn count(&self) -> PulseCount {
        self.count
    }

    #[must_use]
    pub const fn cause(&self) -> CauseRef {
        self.cause
    }
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
    ExternalPulseObservation {
        input: ExternalInputKey<Pulse>,
        count: PulseCount,
    },
    PendingPulseDelay {
        event: PendingEventKey,
        node: NodeKey,
        origin: Time<D>,
        deadline: Time<D>,
        count: PulseCount,
        revision: NetworkRevision,
        supporters: Vec<CauseRef>,
    },
    Derived {
        subject: ProvenanceSubject,
        supporters: Vec<CauseRef>,
    },
    PulseDerived {
        subject: ProvenanceSubject,
        contributions: Vec<PulseContribution>,
        result: PulseCount,
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
    ExternalPulseObservation {
        input: ExternalInputKey<Pulse>,
        count: PulseCount,
    },
    PendingPulseDelay {
        event: PendingEventKey,
        node: NodeKey,
        origin: Time<D>,
        deadline: Time<D>,
        count: PulseCount,
        revision: NetworkRevision,
        supporters: &'a [CauseRef],
    },
    Derived {
        subject: ProvenanceSubject,
        supporters: &'a [CauseRef],
    },
    PulseDerived {
        subject: ProvenanceSubject,
        contributions: &'a [PulseContribution],
        result: PulseCount,
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
    pulse_output_causes: BTreeMap<ExternalOutputKey<Pulse>, CauseRef>,
    toggle_inversion_causes: BTreeMap<NodeKey, CauseRef>,
    pulse_delay_schedules: BTreeMap<NodeKey, CauseRef>,
}

struct EvaluationProvenanceInputs<'a> {
    levels: &'a BTreeMap<ExternalInputKey<Level>, CauseRef>,
    pulses: &'a BTreeMap<ExternalInputKey<Pulse>, CauseRef>,
    due_pulse_delays: &'a BTreeMap<NodeKey, Vec<CauseRef>>,
}

struct PreviousLevelOutputs<'a> {
    causes: &'a BTreeMap<ExternalOutputKey<Level>, CauseRef>,
    baselines: &'a BTreeMap<ExternalOutputKey<Level>, LogicLevel>,
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
            ProvenanceRecord::ExternalPulseObservation { input, count } => {
                CauseInspection::ExternalPulseObservation {
                    input: *input,
                    count: *count,
                }
            }
            ProvenanceRecord::PendingPulseDelay {
                event,
                node,
                origin,
                deadline,
                count,
                revision,
                supporters,
            } => CauseInspection::PendingPulseDelay {
                event: *event,
                node: *node,
                origin: *origin,
                deadline: *deadline,
                count: *count,
                revision: *revision,
                supporters,
            },
            ProvenanceRecord::Derived {
                subject,
                supporters,
            } => CauseInspection::Derived {
                subject: *subject,
                supporters,
            },
            ProvenanceRecord::PulseDerived {
                subject,
                contributions,
                result,
                supporters,
            } => CauseInspection::PulseDerived {
                subject: *subject,
                contributions,
                result: *result,
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

    fn append_pending_pulse_delay(
        &mut self,
        pending: &PendingPulseDelay<D>,
        scheduling_cause: CauseRef,
    ) -> CauseRef {
        let Some(records) = Arc::get_mut(&mut self.records) else {
            panic!("new transaction provenance must be uniquely owned before publication");
        };
        push_record(
            self.scope,
            records,
            ProvenanceRecord::PendingPulseDelay {
                event: pending.key,
                node: pending.node,
                origin: pending.origin,
                deadline: pending.deadline,
                count: pending.count,
                revision: pending.revision,
                supporters: vec![scheduling_cause],
            },
        )
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
    Pulsed {
        output: ExternalOutputKey<Pulse>,
        count: PulseCount,
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
            Self::Pulsed {
                output,
                count,
                at,
                cause,
                revision,
            } => formatter
                .debug_struct("Pulsed")
                .field("output", output)
                .field("count", count)
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
    schedule: Schedule<D>,
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

    /// Returns the next temporal wakeup state after this transaction.
    #[must_use]
    pub const fn schedule(&self) -> Schedule<D> {
        self.schedule
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
            .field("schedule", &self.schedule)
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

        enforce_outer_reaction_budgets::<D>(&self.policy, &self.compiled, 1)?;
        let revision = self.store.revision;
        let (levels, pulses) = input.into_parts();
        let evaluation =
            evaluate_reaction::<D>(&self.compiled, &levels, &pulses, &self.store.toggle_states)?;
        let ProvenanceBuild {
            mut provenance,
            input_causes,
            output_causes,
            pulse_output_causes,
            toggle_inversion_causes,
            pulse_delay_schedules,
        } = build_initialization_provenance(
            self.compiled.network_key(),
            self.compiled.fingerprint(),
            revision,
            at,
            &levels,
            &pulses,
            &evaluation,
        );
        let mut created_pending_events = 0_u64;
        let mut pending_pulse_delays = BTreeMap::new();
        let mut next_pending_event_serial = 0;
        schedule_pulse_delays(
            PulseDelayScheduling {
                pending: &mut pending_pulse_delays,
                next_serial: &mut next_pending_event_serial,
                created_events: &mut created_pending_events,
            },
            at,
            revision,
            &evaluation,
            &pulse_delay_schedules,
            &mut provenance,
            &self.policy,
        )?;
        enforce_budget::<D>(
            &self.policy,
            RuntimePolicyLimit::MaxRequiredProvenanceGrowth,
            count_as_u64(provenance.len()),
        )?;

        let output_events = initialization_events(
            at,
            revision,
            &evaluation.external_outputs,
            &output_causes,
            &evaluation.pulse_outputs,
            &pulse_output_causes,
        );
        enforce_created_event_budget::<D>(
            &self.policy,
            created_pending_events,
            output_events.len(),
        )?;
        let schedule = schedule_from_pending(&pending_pulse_delays);
        let result = TransactionResult {
            requested_time: at,
            before_revision: revision,
            after_revision: revision,
            output_events,
            schedule,
            provenance: provenance.clone(),
        };

        publish_candidate(
            self,
            PublishedCandidate {
                at,
                levels,
                evaluation,
                input_causes,
                output_causes,
                provenance,
                toggle_inversion_causes,
                pending_pulse_delays,
                next_pending_event_serial,
            },
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

        let revision = self.store.revision;
        let (explicit_levels, pulses) = input.into_parts();
        let mut levels = self.store.external_levels.clone();
        let mut pending_pulse_delays = self.store.pending_pulse_delays.clone();
        let mut next_pending_event_serial = self.store.next_pending_event_serial;
        let mut toggle_states = self.store.toggle_states.clone();
        let mut output_baselines = self.store.output_baselines.clone();
        let mut input_causes = self.store.input_causes.clone();
        let mut output_causes = self.store.output_causes.clone();
        let mut toggle_inversion_causes = self.store.toggle_inversion_causes.clone();
        let mut provenance = match self.store.provenance.as_ref() {
            Some(provenance) => provenance.clone(),
            None => panic!("ready machine must retain committed provenance"),
        };
        let previous_provenance_len = provenance.len();
        let mut output_events = Vec::new();
        let mut created_pending_events = 0_u64;
        let mut reaction_count = 0_u64;
        let empty_levels = BTreeMap::new();
        let empty_pulses = BTreeMap::new();

        while pending_pulse_delays
            .keys()
            .next()
            .is_some_and(|deadline| *deadline < at)
        {
            let deadline = match pending_pulse_delays.keys().next().copied() {
                Some(deadline) => deadline,
                None => panic!("nonempty temporal calendar must have a least deadline"),
            };
            let batch = match pending_pulse_delays.remove(&deadline) {
                Some(batch) => batch,
                None => panic!("selected temporal deadline must retain its event batch"),
            };
            let due = aggregate_due::<D>(batch)?;
            let internal = self
                .compiled
                .evaluate_temporal_reaction(&levels, &empty_pulses, &toggle_states, &due.counts)
                .map_err(evaluation_failure)?;
            reaction_count = reaction_count.saturating_add(1);
            enforce_outer_reaction_budgets::<D>(&self.policy, &self.compiled, reaction_count)?;

            let mut built = build_ready_provenance(
                self.compiled.network_key(),
                self.compiled.fingerprint(),
                revision,
                deadline,
                &levels,
                &empty_levels,
                &empty_pulses,
                &internal,
                &provenance,
                &input_causes,
                &output_causes,
                &output_baselines,
                &toggle_inversion_causes,
                &due.causes,
            );
            remap_pending_causes(&mut pending_pulse_delays, built.provenance.scope);
            remap_output_event_causes(&mut output_events, built.provenance.scope);
            let mut reaction_events = changed_events(
                deadline,
                revision,
                &output_baselines,
                &internal.external_outputs,
                &built.output_causes,
                &internal.pulse_outputs,
                &built.pulse_output_causes,
            );
            output_events.append(&mut reaction_events);

            input_causes = built.input_causes;
            output_causes = built.output_causes;
            toggle_inversion_causes = built.toggle_inversion_causes;
            toggle_states = internal.proposed_toggle_states.clone();
            output_baselines = internal.external_outputs.clone();
            schedule_pulse_delays(
                PulseDelayScheduling {
                    pending: &mut pending_pulse_delays,
                    next_serial: &mut next_pending_event_serial,
                    created_events: &mut created_pending_events,
                },
                deadline,
                revision,
                &internal,
                &built.pulse_delay_schedules,
                &mut built.provenance,
                &self.policy,
            )?;
            provenance = built.provenance;
            enforce_created_event_budget::<D>(
                &self.policy,
                created_pending_events,
                output_events.len(),
            )?;
            enforce_provenance_growth::<D>(
                &self.policy,
                provenance.len(),
                previous_provenance_len,
            )?;
        }

        // Target-time external levels become authoritative only after every
        // strictly earlier internal deadline has completed on candidate state.
        levels.extend(explicit_levels.iter().map(|(key, value)| (*key, *value)));
        let due = pending_pulse_delays
            .remove(&at)
            .map(aggregate_due::<D>)
            .transpose()?
            .unwrap_or_default();
        let evaluation = self
            .compiled
            .evaluate_temporal_reaction(&levels, &pulses, &toggle_states, &due.counts)
            .map_err(evaluation_failure)?;
        reaction_count = reaction_count.saturating_add(1);
        enforce_outer_reaction_budgets::<D>(&self.policy, &self.compiled, reaction_count)?;

        let mut built = build_ready_provenance(
            self.compiled.network_key(),
            self.compiled.fingerprint(),
            revision,
            at,
            &levels,
            &explicit_levels,
            &pulses,
            &evaluation,
            &provenance,
            &input_causes,
            &output_causes,
            &output_baselines,
            &toggle_inversion_causes,
            &due.causes,
        );
        remap_pending_causes(&mut pending_pulse_delays, built.provenance.scope);
        remap_output_event_causes(&mut output_events, built.provenance.scope);
        let mut final_events = changed_events(
            at,
            revision,
            &output_baselines,
            &evaluation.external_outputs,
            &built.output_causes,
            &evaluation.pulse_outputs,
            &built.pulse_output_causes,
        );
        output_events.append(&mut final_events);
        schedule_pulse_delays(
            PulseDelayScheduling {
                pending: &mut pending_pulse_delays,
                next_serial: &mut next_pending_event_serial,
                created_events: &mut created_pending_events,
            },
            at,
            revision,
            &evaluation,
            &built.pulse_delay_schedules,
            &mut built.provenance,
            &self.policy,
        )?;
        enforce_created_event_budget::<D>(
            &self.policy,
            created_pending_events,
            output_events.len(),
        )?;
        enforce_provenance_growth::<D>(
            &self.policy,
            built.provenance.len(),
            previous_provenance_len,
        )?;
        let schedule = schedule_from_pending(&pending_pulse_delays);
        let result = TransactionResult {
            requested_time: at,
            before_revision: revision,
            after_revision: revision,
            output_events,
            schedule,
            provenance: built.provenance.clone(),
        };

        publish_candidate(
            self,
            PublishedCandidate {
                at,
                levels,
                evaluation,
                input_causes: built.input_causes,
                output_causes: built.output_causes,
                provenance: built.provenance,
                toggle_inversion_causes: built.toggle_inversion_causes,
                pending_pulse_delays,
                next_pending_event_serial,
            },
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

fn evaluate_reaction<D>(
    compiled: &crate::CompiledNetwork<D>,
    levels: &BTreeMap<ExternalInputKey<Level>, LogicLevel>,
    pulses: &BTreeMap<ExternalInputKey<Pulse>, PulseCount>,
    previous_toggle_states: &[LogicLevel],
) -> Result<FullEvaluation, RuntimeFailure<D>> {
    match compiled.evaluate_reaction_with_state(levels, pulses, previous_toggle_states) {
        Ok(evaluation) => Ok(evaluation),
        Err(EvaluationFailure::PulseCountOverflow { node }) => Err(RuntimeFailure::new(
            RuntimeFailureEvidence::PulseCountOverflow { node },
        )),
        Err(EvaluationFailure::Incomplete) => {
            panic!("validated topology and exact-bound inputs must evaluate completely")
        }
    }
}

fn evaluation_failure<D>(failure: EvaluationFailure) -> RuntimeFailure<D> {
    match failure {
        EvaluationFailure::PulseCountOverflow { node } => {
            RuntimeFailure::new(RuntimeFailureEvidence::PulseCountOverflow { node })
        }
        EvaluationFailure::Incomplete => {
            panic!("validated topology and exact-bound inputs must evaluate completely")
        }
    }
}

#[derive(Default)]
struct DuePulseDelays {
    counts: BTreeMap<NodeKey, PulseCount>,
    causes: BTreeMap<NodeKey, Vec<CauseRef>>,
}

struct PulseDelayScheduling<'a, D> {
    pending: &'a mut BTreeMap<Time<D>, Vec<PendingPulseDelay<D>>>,
    next_serial: &'a mut u64,
    created_events: &'a mut u64,
}

fn aggregate_due<D>(batch: Vec<PendingPulseDelay<D>>) -> Result<DuePulseDelays, RuntimeFailure<D>> {
    let mut due = DuePulseDelays::default();
    for event in batch {
        let previous = due
            .counts
            .get(&event.node)
            .copied()
            .unwrap_or(PulseCount::ZERO);
        let combined = previous.checked_add(event.count).map_err(|_| {
            RuntimeFailure::new(RuntimeFailureEvidence::PulseCountOverflow { node: event.node })
        })?;
        due.counts.insert(event.node, combined);
        due.causes.entry(event.node).or_default().push(event.cause);
    }
    for causes in due.causes.values_mut() {
        causes.sort();
        causes.dedup();
    }
    Ok(due)
}

fn schedule_pulse_delays<D>(
    scheduling: PulseDelayScheduling<'_, D>,
    origin: Time<D>,
    revision: NetworkRevision,
    evaluation: &FullEvaluation,
    proposal_causes: &BTreeMap<NodeKey, CauseRef>,
    provenance: &mut ProvenanceView<D>,
    policy: &RuntimePolicy,
) -> Result<(), RuntimeFailure<D>> {
    for proposal in &evaluation.pulse_delay_proposals {
        let deadline = origin
            .checked_add(Span::from_ticks(proposal.delay_ticks))
            .map_err(|_| {
                RuntimeFailure::new(RuntimeFailureEvidence::TimeOverflow {
                    node: proposal.node,
                    origin_ticks: origin.ticks(),
                    delay_ticks: proposal.delay_ticks,
                })
            })?;
        let key = PendingEventKey::from_serial(*scheduling.next_serial);
        *scheduling.next_serial = scheduling.next_serial.checked_add(1).ok_or_else(|| {
            RuntimeFailure::new(RuntimeFailureEvidence::BudgetExceeded {
                budget: RuntimePolicyLimit::MaxEventsCreatedPerTransaction,
                limit: policy.max_events_created_per_transaction(),
                consumed: u64::MAX,
            })
        })?;
        let scheduling_cause = match proposal_causes.get(&proposal.node).copied() {
            Some(cause) => cause,
            None => panic!("every PulseDelay proposal must retain a scheduling cause"),
        };
        let mut pending = PendingPulseDelay {
            key,
            node: proposal.node,
            origin,
            deadline,
            count: proposal.count,
            revision,
            cause: scheduling_cause,
        };
        pending.cause = provenance.append_pending_pulse_delay(&pending, scheduling_cause);
        scheduling
            .pending
            .entry(deadline)
            .or_default()
            .push(pending);
        *scheduling.created_events = scheduling.created_events.saturating_add(1);
        enforce_budget::<D>(
            policy,
            RuntimePolicyLimit::MaxEventsCreatedPerTransaction,
            *scheduling.created_events,
        )?;
    }
    let pending_count = scheduling.pending.values().map(Vec::len).sum::<usize>();
    enforce_budget::<D>(
        policy,
        RuntimePolicyLimit::MaxPendingEvents,
        count_as_u64(pending_count),
    )?;
    Ok(())
}

fn enforce_outer_reaction_budgets<D>(
    policy: &RuntimePolicy,
    compiled: &crate::CompiledNetwork<D>,
    reaction_count: u64,
) -> Result<(), RuntimeFailure<D>> {
    enforce_budget::<D>(
        policy,
        RuntimePolicyLimit::MaxInternalReactions,
        reaction_count,
    )?;
    let operations = reaction_count.saturating_mul(count_as_u64(compiled.operation_count()));
    enforce_budget::<D>(
        policy,
        RuntimePolicyLimit::MaxEvaluatedOperations,
        operations,
    )
}

fn enforce_created_event_budget<D>(
    policy: &RuntimePolicy,
    pending_created: u64,
    output_events: usize,
) -> Result<(), RuntimeFailure<D>> {
    enforce_budget::<D>(
        policy,
        RuntimePolicyLimit::MaxEventsCreatedPerTransaction,
        pending_created.saturating_add(count_as_u64(output_events)),
    )
}

fn enforce_provenance_growth<D>(
    policy: &RuntimePolicy,
    current_len: usize,
    previous_len: usize,
) -> Result<(), RuntimeFailure<D>> {
    enforce_budget::<D>(
        policy,
        RuntimePolicyLimit::MaxRequiredProvenanceGrowth,
        count_as_u64(current_len.saturating_sub(previous_len)),
    )
}

fn schedule_from_pending<D>(pending: &BTreeMap<Time<D>, Vec<PendingPulseDelay<D>>>) -> Schedule<D> {
    match pending.keys().next().copied() {
        Some(deadline) => Schedule::WakeAt(deadline),
        None => Schedule::Dormant,
    }
}

fn remap_pending_causes<D>(
    pending: &mut BTreeMap<Time<D>, Vec<PendingPulseDelay<D>>>,
    scope: [u8; 16],
) {
    for event in pending.values_mut().flatten() {
        event.cause = remap_cause(event.cause, scope);
    }
}

fn remap_output_event_causes<D>(events: &mut [OutputEvent<D>], scope: [u8; 16]) {
    for event in events {
        let cause = match event {
            OutputEvent::LevelEstablished { cause, .. }
            | OutputEvent::LevelChanged { cause, .. }
            | OutputEvent::Pulsed { cause, .. } => cause,
        };
        *cause = remap_cause(*cause, scope);
    }
}

struct PublishedCandidate<D> {
    at: Time<D>,
    levels: BTreeMap<ExternalInputKey<Level>, LogicLevel>,
    evaluation: FullEvaluation,
    input_causes: BTreeMap<ExternalInputKey<Level>, CauseRef>,
    output_causes: BTreeMap<ExternalOutputKey<Level>, CauseRef>,
    provenance: ProvenanceView<D>,
    toggle_inversion_causes: BTreeMap<NodeKey, CauseRef>,
    pending_pulse_delays: BTreeMap<Time<D>, Vec<PendingPulseDelay<D>>>,
    next_pending_event_serial: u64,
}

fn publish_candidate<D>(machine: &mut Machine<D>, published: PublishedCandidate<D>) {
    let PublishedCandidate {
        at,
        levels,
        evaluation,
        input_causes,
        output_causes,
        provenance,
        toggle_inversion_causes,
        pending_pulse_delays,
        next_pending_event_serial,
    } = published;
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
    candidate.toggle_states = evaluation.proposed_toggle_states;
    candidate.toggle_inversion_causes = toggle_inversion_causes;
    candidate.pending_pulse_delays = pending_pulse_delays;
    candidate.next_pending_event_serial = next_pending_event_serial;
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
    pulses: &BTreeMap<ExternalInputKey<Pulse>, PulseCount>,
) -> [u8; 16] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(PROVENANCE_SCOPE_DOMAIN);
    hasher.update(&network_key.as_u128().to_be_bytes());
    hasher.update(&fingerprint.as_bytes());
    hasher.update(&revision.value().to_be_bytes());
    hasher.update(&at.ticks().to_be_bytes());
    for (input, value) in levels {
        hasher.update(&[0]);
        hasher.update(&input.as_u128().to_be_bytes());
        hasher.update(&[u8::from(value.is_high())]);
    }
    for (input, count) in pulses {
        hasher.update(&[1]);
        hasher.update(&input.as_u128().to_be_bytes());
        hasher.update(&count.get().to_be_bytes());
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
    pulses: &BTreeMap<ExternalInputKey<Pulse>, PulseCount>,
    evaluation: &FullEvaluation,
) -> ProvenanceBuild<D> {
    let scope = provenance_scope(network_key, fingerprint, revision, at, levels, pulses);
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
    let pulse_input_causes = pulses
        .iter()
        .map(|(input, count)| {
            let cause = push_record(
                scope,
                &mut records,
                ProvenanceRecord::ExternalPulseObservation {
                    input: *input,
                    count: *count,
                },
            );
            (*input, cause)
        })
        .collect::<BTreeMap<_, _>>();
    let evaluation_causes = append_evaluation_provenance(
        scope,
        &mut records,
        transaction_cause,
        evaluation,
        EvaluationProvenanceInputs {
            levels: &input_causes,
            pulses: &pulse_input_causes,
            due_pulse_delays: &BTreeMap::new(),
        },
        None,
        None,
    );

    ProvenanceBuild {
        provenance: ProvenanceView {
            scope,
            records: Arc::new(records),
        },
        input_causes,
        output_causes: evaluation_causes.level_outputs,
        pulse_output_causes: evaluation_causes.pulse_outputs,
        toggle_inversion_causes: evaluation_causes.toggle_inversions,
        pulse_delay_schedules: evaluation_causes.pulse_delay_schedules,
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
    pulses: &BTreeMap<ExternalInputKey<Pulse>, PulseCount>,
    evaluation: &FullEvaluation,
    previous: &ProvenanceView<D>,
    previous_input_causes: &BTreeMap<ExternalInputKey<Level>, CauseRef>,
    previous_output_causes: &BTreeMap<ExternalOutputKey<Level>, CauseRef>,
    previous_output_baselines: &BTreeMap<ExternalOutputKey<Level>, LogicLevel>,
    previous_toggle_inversion_causes: &BTreeMap<NodeKey, CauseRef>,
    due_pulse_delays: &BTreeMap<NodeKey, Vec<CauseRef>>,
) -> ProvenanceBuild<D> {
    let scope = provenance_scope(network_key, fingerprint, revision, at, levels, pulses);
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
    let pulse_input_causes = pulses
        .iter()
        .map(|(input, count)| {
            let cause = push_record(
                scope,
                &mut records,
                ProvenanceRecord::ExternalPulseObservation {
                    input: *input,
                    count: *count,
                },
            );
            (*input, cause)
        })
        .collect::<BTreeMap<_, _>>();
    let remapped_output_causes = previous_output_causes
        .iter()
        .map(|(output, cause)| (*output, remap_cause(*cause, scope)))
        .collect::<BTreeMap<_, _>>();
    let evaluation_causes = append_evaluation_provenance(
        scope,
        &mut records,
        transaction_cause,
        evaluation,
        EvaluationProvenanceInputs {
            levels: &input_causes,
            pulses: &pulse_input_causes,
            due_pulse_delays,
        },
        Some(PreviousLevelOutputs {
            causes: &remapped_output_causes,
            baselines: previous_output_baselines,
        }),
        Some(previous_toggle_inversion_causes),
    );

    ProvenanceBuild {
        provenance: ProvenanceView {
            scope,
            records: Arc::new(records),
        },
        input_causes,
        output_causes: evaluation_causes.level_outputs,
        pulse_output_causes: evaluation_causes.pulse_outputs,
        toggle_inversion_causes: evaluation_causes.toggle_inversions,
        pulse_delay_schedules: evaluation_causes.pulse_delay_schedules,
    }
}

struct EvaluationCauseMaps {
    level_outputs: BTreeMap<ExternalOutputKey<Level>, CauseRef>,
    pulse_outputs: BTreeMap<ExternalOutputKey<Pulse>, CauseRef>,
    toggle_inversions: BTreeMap<NodeKey, CauseRef>,
    pulse_delay_schedules: BTreeMap<NodeKey, CauseRef>,
}

fn append_evaluation_provenance<D>(
    scope: [u8; 16],
    records: &mut Vec<ProvenanceRecord<D>>,
    transaction_cause: CauseRef,
    evaluation: &FullEvaluation,
    input_causes: EvaluationProvenanceInputs<'_>,
    previous_outputs: Option<PreviousLevelOutputs<'_>>,
    previous_toggle_inversions: Option<&BTreeMap<NodeKey, CauseRef>>,
) -> EvaluationCauseMaps {
    let previous_output_causes = previous_outputs.as_ref().map(|previous| previous.causes);
    let previous_output_baselines = previous_outputs.as_ref().map(|previous| previous.baselines);
    let mut operation_causes = Vec::with_capacity(evaluation.causes.len());
    let mut output_causes = BTreeMap::new();
    let mut pulse_output_causes = BTreeMap::new();
    let mut toggle_inversion_causes = previous_toggle_inversions
        .into_iter()
        .flat_map(|causes| causes.iter())
        .map(|(node, cause)| (*node, remap_cause(*cause, scope)))
        .collect::<BTreeMap<_, _>>();

    for cause in &evaluation.causes {
        let resolved = match cause {
            EvaluationCause::ExternalInput(input) => {
                let Some(cause) = input_causes.levels.get(input).copied() else {
                    panic!("evaluated external input must retain one authoritative cause");
                };
                cause
            }
            EvaluationCause::PulseExternalInput(input) => input_causes
                .pulses
                .get(input)
                .copied()
                .unwrap_or(transaction_cause),
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
            EvaluationCause::PulseMerge {
                node,
                contributions,
                result,
            } => {
                let mut grouped = contributions
                    .iter()
                    .map(|contribution| PulseContribution {
                        port: contribution.port,
                        count: contribution.count,
                        cause: operation_cause(&operation_causes, contribution.source),
                    })
                    .collect::<Vec<_>>();
                grouped.sort_by_key(|contribution| contribution.port);
                let mut supporters = vec![transaction_cause];
                supporters.extend(grouped.iter().map(|contribution| contribution.cause));
                supporters.sort();
                supporters.dedup();
                push_record(
                    scope,
                    records,
                    ProvenanceRecord::PulseDerived {
                        subject: ProvenanceSubject::Node(*node),
                        contributions: grouped,
                        result: *result,
                        supporters,
                    },
                )
            }
            EvaluationCause::Toggle {
                node,
                input,
                count: _,
                previous: _,
                result: _,
                inverted,
            } => {
                let mut supporters = vec![
                    transaction_cause,
                    operation_cause(&operation_causes, *input),
                ];
                if let Some(previous_inversion) = toggle_inversion_causes.get(node).copied() {
                    supporters.push(previous_inversion);
                }
                supporters.sort();
                supporters.dedup();
                let reference = push_record(
                    scope,
                    records,
                    ProvenanceRecord::Derived {
                        subject: ProvenanceSubject::Node(*node),
                        supporters,
                    },
                );
                if *inverted {
                    toggle_inversion_causes.insert(*node, reference);
                }
                reference
            }
            EvaluationCause::PulseDelay { node } => {
                let mut supporters = vec![transaction_cause];
                supporters.extend(
                    input_causes
                        .due_pulse_delays
                        .get(node)
                        .into_iter()
                        .flatten()
                        .map(|cause| remap_cause(*cause, scope)),
                );
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
            EvaluationCause::PulseExternalOutput { output, source } => {
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
                        subject: ProvenanceSubject::PulseExternalOutput(*output),
                        supporters,
                    },
                );
                pulse_output_causes.insert(*output, reference);
                reference
            }
        };
        operation_causes.push(resolved);
    }
    let mut pulse_delay_schedules = BTreeMap::new();
    for proposal in &evaluation.pulse_delay_proposals {
        let mut supporters = vec![
            transaction_cause,
            operation_cause(&operation_causes, proposal.input_source),
        ];
        supporters.sort();
        supporters.dedup();
        let reference = push_record(
            scope,
            records,
            ProvenanceRecord::Derived {
                subject: ProvenanceSubject::Node(proposal.node),
                supporters,
            },
        );
        pulse_delay_schedules.insert(proposal.node, reference);
    }
    EvaluationCauseMaps {
        level_outputs: output_causes,
        pulse_outputs: pulse_output_causes,
        toggle_inversions: toggle_inversion_causes,
        pulse_delay_schedules,
    }
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
        ProvenanceRecord::ExternalPulseObservation { input, count } => {
            ProvenanceRecord::ExternalPulseObservation {
                input: *input,
                count: *count,
            }
        }
        ProvenanceRecord::PendingPulseDelay {
            event,
            node,
            origin,
            deadline,
            count,
            revision,
            supporters,
        } => ProvenanceRecord::PendingPulseDelay {
            event: *event,
            node: *node,
            origin: *origin,
            deadline: *deadline,
            count: *count,
            revision: *revision,
            supporters: supporters
                .iter()
                .map(|cause| remap_cause(*cause, scope))
                .collect(),
        },
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
        ProvenanceRecord::PulseDerived {
            subject,
            contributions,
            result,
            supporters,
        } => ProvenanceRecord::PulseDerived {
            subject: *subject,
            contributions: contributions
                .iter()
                .map(|contribution| PulseContribution {
                    port: contribution.port,
                    count: contribution.count,
                    cause: remap_cause(contribution.cause, scope),
                })
                .collect(),
            result: *result,
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
    pulse_values: &BTreeMap<ExternalOutputKey<Pulse>, PulseCount>,
    pulse_causes: &BTreeMap<ExternalOutputKey<Pulse>, CauseRef>,
) -> Vec<OutputEvent<D>> {
    let mut events = values
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
        .collect::<Vec<_>>();
    events.extend(pulse_events(at, revision, pulse_values, pulse_causes));
    sort_output_events(&mut events);
    events
}

fn changed_events<D>(
    at: Time<D>,
    revision: NetworkRevision,
    previous: &BTreeMap<ExternalOutputKey<Level>, LogicLevel>,
    settled: &BTreeMap<ExternalOutputKey<Level>, LogicLevel>,
    causes: &BTreeMap<ExternalOutputKey<Level>, CauseRef>,
    pulse_values: &BTreeMap<ExternalOutputKey<Pulse>, PulseCount>,
    pulse_causes: &BTreeMap<ExternalOutputKey<Pulse>, CauseRef>,
) -> Vec<OutputEvent<D>> {
    let mut events = settled
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
        .collect::<Vec<_>>();
    events.extend(pulse_events(at, revision, pulse_values, pulse_causes));
    sort_output_events(&mut events);
    events
}

fn pulse_events<D>(
    at: Time<D>,
    revision: NetworkRevision,
    values: &BTreeMap<ExternalOutputKey<Pulse>, PulseCount>,
    causes: &BTreeMap<ExternalOutputKey<Pulse>, CauseRef>,
) -> Vec<OutputEvent<D>> {
    values
        .iter()
        .filter(|(_, count)| count.is_positive())
        .map(|(output, count)| {
            let Some(cause) = causes.get(output).copied() else {
                panic!("every nonzero pulse output must retain one committed cause");
            };
            OutputEvent::Pulsed {
                output: *output,
                count: *count,
                at,
                cause,
                revision,
            }
        })
        .collect()
}

fn sort_output_events<D>(events: &mut [OutputEvent<D>]) {
    events.sort_by_key(|event| match event {
        OutputEvent::LevelEstablished { output, .. } | OutputEvent::LevelChanged { output, .. } => {
            (0_u8, output.as_u128())
        }
        OutputEvent::Pulsed { output, .. } => (1_u8, output.as_u128()),
    });
}

#[cfg(test)]
mod tests {
    use crate::authored::{
        ConnectionDef, ExternalInputDef, ExternalOutputDef, InputPortRole, NodeDef, NodeKind,
        NodePorts, UncheckedNetwork,
    };
    use crate::diagnostics::{Responsibility, Severity};
    use crate::key::{
        ConnectionKey, ExternalInputKey, ExternalOutputKey, InPortKey, NetworkKey, NodeKey,
        OutPortKey, SignalSourceKey,
    };
    use crate::metadata::DiagnosticMeta;
    use crate::signal::{Level, LogicLevel, Pulse, PulseCount};
    use crate::{
        CauseInspection, CauseRef, MachineStatus, NetworkRevision, OutputEvent, ProvenanceSubject,
        ProvenanceView, RuntimeFailureEvidence, RuntimePolicy, RuntimePolicyLimit, TimeDomainId,
        Transaction,
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

    fn compiled_all() -> crate::CompiledNetwork<()> {
        compiled_variadic(NodeKind::all())
    }

    fn compiled_variadic(kind: NodeKind<()>) -> crate::CompiledNetwork<()> {
        let first = ExternalInputKey::<Level>::from_u128(1);
        let second = ExternalInputKey::<Level>::from_u128(2);
        let first_port = InPortKey::<Level>::from_u128(11);
        let second_port = InPortKey::<Level>::from_u128(12);
        let node_output = OutPortKey::<Level>::from_u128(13);
        UncheckedNetwork::new(
            NetworkKey::from_u128(20),
            TimeDomainId::from_u128(2),
            DiagnosticMeta::default(),
            vec![NodeDef::new(
                NodeKey::from_u128(10),
                kind,
                NodePorts::new(
                    vec![first_port.into(), second_port.into()],
                    vec![node_output.into()],
                ),
                DiagnosticMeta::default(),
            )],
            vec![
                ExternalInputDef::new(first.into(), DiagnosticMeta::default()),
                ExternalInputDef::new(second.into(), DiagnosticMeta::default()),
            ],
            vec![ExternalOutputDef::new(
                ExternalOutputKey::<Level>::from_u128(30).into(),
                SignalSourceKey::NodeOutput(node_output).into(),
                DiagnosticMeta::default(),
            )],
            vec![
                ConnectionDef::new(
                    ConnectionKey::from_u128(40),
                    first.into(),
                    first_port.into(),
                    DiagnosticMeta::default(),
                ),
                ConnectionDef::new(
                    ConnectionKey::from_u128(41),
                    second.into(),
                    second_port.into(),
                    DiagnosticMeta::default(),
                ),
            ],
        )
        .validate()
        .require_artifact()
        .unwrap_or_else(|_| panic!("variadic fixture must validate"))
        .compile()
        .require_artifact()
        .unwrap_or_else(|_| panic!("variadic fixture must compile"))
    }

    fn compiled_merge() -> crate::CompiledNetwork<()> {
        let first = ExternalInputKey::<Pulse>::from_u128(1);
        let second = ExternalInputKey::<Pulse>::from_u128(2);
        let first_port = InPortKey::<Pulse>::from_u128(11);
        let second_port = InPortKey::<Pulse>::from_u128(12);
        let node_output = OutPortKey::<Pulse>::from_u128(13);
        UncheckedNetwork::new(
            NetworkKey::from_u128(20),
            TimeDomainId::from_u128(2),
            DiagnosticMeta::default(),
            vec![NodeDef::new(
                NodeKey::from_u128(10),
                NodeKind::merge(),
                NodePorts::new(
                    vec![first_port.into(), second_port.into()],
                    vec![node_output.into()],
                ),
                DiagnosticMeta::default(),
            )],
            vec![
                ExternalInputDef::new(first.into(), DiagnosticMeta::default()),
                ExternalInputDef::new(second.into(), DiagnosticMeta::default()),
            ],
            vec![ExternalOutputDef::new(
                ExternalOutputKey::<Pulse>::from_u128(30).into(),
                SignalSourceKey::NodeOutput(node_output).into(),
                DiagnosticMeta::default(),
            )],
            vec![
                ConnectionDef::new(
                    ConnectionKey::from_u128(40),
                    first.into(),
                    first_port.into(),
                    DiagnosticMeta::default(),
                ),
                ConnectionDef::new(
                    ConnectionKey::from_u128(41),
                    second.into(),
                    second_port.into(),
                    DiagnosticMeta::default(),
                ),
            ],
        )
        .validate()
        .require_artifact()
        .unwrap_or_else(|_| panic!("Merge fixture must validate"))
        .compile()
        .require_artifact()
        .unwrap_or_else(|_| panic!("Merge fixture must compile"))
    }

    fn compiled_select() -> crate::CompiledNetwork<()> {
        let inputs = [
            ExternalInputKey::<Level>::from_u128(1),
            ExternalInputKey::<Level>::from_u128(2),
            ExternalInputKey::<Level>::from_u128(3),
        ];
        let ports = [
            InPortKey::<Level>::from_u128(11),
            InPortKey::<Level>::from_u128(12),
            InPortKey::<Level>::from_u128(13),
        ];
        let node_output = OutPortKey::<Level>::from_u128(14);
        UncheckedNetwork::new(
            NetworkKey::from_u128(20),
            TimeDomainId::from_u128(2),
            DiagnosticMeta::default(),
            vec![NodeDef::new(
                NodeKey::from_u128(10),
                NodeKind::select(),
                NodePorts::with_input_roles(
                    ports.into_iter().map(Into::into).collect(),
                    vec![
                        InputPortRole::Selector,
                        InputPortRole::WhenLow,
                        InputPortRole::WhenHigh,
                    ],
                    vec![node_output.into()],
                ),
                DiagnosticMeta::default(),
            )],
            inputs
                .into_iter()
                .map(|key| ExternalInputDef::new(key.into(), DiagnosticMeta::default()))
                .collect(),
            vec![ExternalOutputDef::new(
                ExternalOutputKey::<Level>::from_u128(30).into(),
                SignalSourceKey::NodeOutput(node_output).into(),
                DiagnosticMeta::default(),
            )],
            ports
                .into_iter()
                .zip(inputs)
                .enumerate()
                .map(|(index, (port, input))| {
                    ConnectionDef::new(
                        ConnectionKey::from_u128(40 + index as u128),
                        input.into(),
                        port.into(),
                        DiagnosticMeta::default(),
                    )
                })
                .collect(),
        )
        .validate()
        .require_artifact()
        .unwrap_or_else(|_| panic!("Select fixture must validate"))
        .compile()
        .require_artifact()
        .unwrap_or_else(|_| panic!("Select fixture must compile"))
    }

    fn compiled_toggle(initial: LogicLevel, downstream_not: bool) -> crate::CompiledNetwork<()> {
        let pulse = ExternalInputKey::<Pulse>::from_u128(1);
        let toggle_input = InPortKey::<Pulse>::from_u128(11);
        let toggle_output = OutPortKey::<Level>::from_u128(12);
        let mut nodes = vec![NodeDef::new(
            NodeKey::from_u128(10),
            NodeKind::toggle(initial),
            NodePorts::with_input_roles(
                vec![toggle_input.into()],
                vec![InputPortRole::Toggle],
                vec![toggle_output.into()],
            ),
            DiagnosticMeta::default(),
        )];
        let mut connections = vec![ConnectionDef::new(
            ConnectionKey::from_u128(40),
            pulse.into(),
            toggle_input.into(),
            DiagnosticMeta::default(),
        )];
        let source = if downstream_not {
            let input = InPortKey::<Level>::from_u128(21);
            let output = OutPortKey::<Level>::from_u128(22);
            nodes.push(NodeDef::new(
                NodeKey::from_u128(20),
                NodeKind::not(),
                NodePorts::new(vec![input.into()], vec![output.into()]),
                DiagnosticMeta::default(),
            ));
            connections.push(ConnectionDef::new(
                ConnectionKey::from_u128(41),
                toggle_output.into(),
                input.into(),
                DiagnosticMeta::default(),
            ));
            output
        } else {
            toggle_output
        };
        UncheckedNetwork::new(
            NetworkKey::from_u128(20),
            TimeDomainId::from_u128(2),
            DiagnosticMeta::default(),
            nodes,
            vec![ExternalInputDef::new(
                pulse.into(),
                DiagnosticMeta::default(),
            )],
            vec![ExternalOutputDef::new(
                ExternalOutputKey::<Level>::from_u128(30).into(),
                SignalSourceKey::NodeOutput(source).into(),
                DiagnosticMeta::default(),
            )],
            connections,
        )
        .validate()
        .require_artifact()
        .unwrap_or_else(|_| panic!("Toggle fixture must validate"))
        .compile()
        .require_artifact()
        .unwrap_or_else(|_| panic!("Toggle fixture must compile"))
    }

    fn compiled_two_toggles() -> crate::CompiledNetwork<()> {
        let inputs = [
            ExternalInputKey::<Pulse>::from_u128(1),
            ExternalInputKey::<Pulse>::from_u128(2),
        ];
        let ports = [
            InPortKey::<Pulse>::from_u128(11),
            InPortKey::<Pulse>::from_u128(21),
        ];
        let outputs = [
            OutPortKey::<Level>::from_u128(12),
            OutPortKey::<Level>::from_u128(22),
        ];
        UncheckedNetwork::new(
            NetworkKey::from_u128(20),
            TimeDomainId::from_u128(2),
            DiagnosticMeta::default(),
            [LogicLevel::Low, LogicLevel::High]
                .into_iter()
                .enumerate()
                .map(|(index, initial)| {
                    NodeDef::new(
                        NodeKey::from_u128(10 + index as u128 * 10),
                        NodeKind::toggle(initial),
                        NodePorts::with_input_roles(
                            vec![ports[index].into()],
                            vec![InputPortRole::Toggle],
                            vec![outputs[index].into()],
                        ),
                        DiagnosticMeta::default(),
                    )
                })
                .collect(),
            inputs
                .into_iter()
                .map(|key| ExternalInputDef::new(key.into(), DiagnosticMeta::default()))
                .collect(),
            outputs
                .into_iter()
                .enumerate()
                .map(|(index, source)| {
                    ExternalOutputDef::new(
                        ExternalOutputKey::<Level>::from_u128(30 + index as u128).into(),
                        SignalSourceKey::NodeOutput(source).into(),
                        DiagnosticMeta::default(),
                    )
                })
                .collect(),
            ports
                .into_iter()
                .zip(inputs)
                .enumerate()
                .map(|(index, (target, source))| {
                    ConnectionDef::new(
                        ConnectionKey::from_u128(40 + index as u128),
                        source.into(),
                        target.into(),
                        DiagnosticMeta::default(),
                    )
                })
                .collect(),
        )
        .validate()
        .require_artifact()
        .unwrap_or_else(|_| panic!("two-Toggle fixture must validate"))
        .compile()
        .require_artifact()
        .unwrap_or_else(|_| panic!("two-Toggle fixture must compile"))
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
        toggle_states: Vec<LogicLevel>,
        toggle_inversion_causes: std::collections::BTreeMap<NodeKey, CauseRef>,
        pending_pulse_delays: std::collections::BTreeMap<
            crate::time::Time<()>,
            Vec<crate::machine::PendingPulseDelay<()>>,
        >,
        next_pending_event_serial: u64,
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
            toggle_states: machine.store.toggle_states.clone(),
            toggle_inversion_causes: machine.store.toggle_inversion_causes.clone(),
            pending_pulse_delays: machine.store.pending_pulse_delays.clone(),
            next_pending_event_serial: machine.store.next_pending_event_serial,
        }
    }

    #[test]
    fn toggle_exhausts_initial_state_and_pulse_parity() {
        for initial in [LogicLevel::Low, LogicLevel::High] {
            for count in [0_u64, 1, 2, 3, 10, 11] {
                let compiled = compiled_toggle(initial, false);
                let mut machine = compiled.spawn(policy_with([10, 100, 0, 100, 1_000]));
                assert_eq!(
                    machine
                        .inspect_toggle_definition(NodeKey::from_u128(10))
                        .unwrap()
                        .initial(),
                    initial
                );
                assert_eq!(
                    machine.inspect_toggle(NodeKey::from_u128(10)),
                    Err(crate::ToggleInspectionFailure::NotInitialized)
                );
                assert_eq!(
                    machine.inspect_toggle_definition(NodeKey::from_u128(999)),
                    Err(crate::ToggleInspectionFailure::UnknownNode(
                        NodeKey::from_u128(999)
                    ))
                );
                let mut snapshot = compiled.input_snapshot();
                if count != 0 {
                    snapshot = snapshot
                        .pulse(ExternalInputKey::from_u128(1), PulseCount::new(count))
                        .unwrap();
                }
                let result = machine
                    .apply(Transaction::initialize(
                        crate::time::Time::from_ticks(10),
                        machine.revision(),
                        snapshot.finish().unwrap(),
                    ))
                    .unwrap();
                let expected = if count % 2 == 1 {
                    initial.invert()
                } else {
                    initial
                };
                let inspected = machine.inspect_toggle(NodeKey::from_u128(10)).unwrap();
                assert_eq!(inspected.node(), NodeKey::from_u128(10));
                assert_eq!(inspected.committed(), expected);
                assert_eq!(inspected.initial(), initial);
                assert_eq!(inspected.revision(), machine.revision());
                assert_eq!(inspected.at(), crate::time::Time::from_ticks(10));
                assert_eq!(
                    machine.output_level(ExternalOutputKey::from_u128(30)),
                    Some(expected)
                );
                assert_eq!(inspected.latest_inversion().is_some(), count % 2 == 1);
                if let Some(cause) = inspected.latest_inversion() {
                    assert!(result.provenance().inspect(cause).is_ok());
                }
            }
        }
    }

    #[test]
    fn toggle_ready_reactions_commit_once_and_retain_latest_inversion_cause() {
        let compiled = compiled_toggle(LogicLevel::Low, false);
        let mut machine = compiled.spawn(policy_with([10, 100, 0, 100, 1_000]));
        let snapshot = compiled
            .input_snapshot()
            .pulse(ExternalInputKey::from_u128(1), PulseCount::ONE)
            .and_then(crate::InputSnapshotBuilder::finish)
            .unwrap();
        machine
            .apply(Transaction::initialize(
                crate::time::Time::from_ticks(5),
                machine.revision(),
                snapshot,
            ))
            .unwrap();
        assert_eq!(
            machine
                .inspect_toggle(NodeKey::from_u128(10))
                .unwrap()
                .committed(),
            LogicLevel::High
        );

        let even = compiled
            .input_delta()
            .pulse(ExternalInputKey::from_u128(1), PulseCount::new(2))
            .and_then(crate::InputDeltaBuilder::finish)
            .unwrap();
        let even_result = machine
            .apply(Transaction::advance(
                crate::time::Time::from_ticks(6),
                machine.revision(),
                even,
            ))
            .unwrap();
        assert!(even_result.output_events().is_empty());
        let retained = machine
            .inspect_toggle(NodeKey::from_u128(10))
            .unwrap()
            .latest_inversion()
            .unwrap();
        assert!(even_result.provenance().inspect(retained).is_ok());

        let odd = compiled
            .input_delta()
            .pulse(ExternalInputKey::from_u128(1), PulseCount::new(3))
            .and_then(crate::InputDeltaBuilder::finish)
            .unwrap();
        let odd_result = machine
            .apply(Transaction::advance(
                crate::time::Time::from_ticks(7),
                machine.revision(),
                odd,
            ))
            .unwrap();
        assert!(matches!(
            odd_result.output_events(),
            [OutputEvent::LevelChanged {
                from: LogicLevel::High,
                to: LogicLevel::Low,
                ..
            }]
        ));
        let inspected = machine.inspect_toggle(NodeKey::from_u128(10)).unwrap();
        assert_eq!(inspected.committed(), LogicLevel::Low);
        let latest = odd_result
            .provenance()
            .inspect(inspected.latest_inversion().unwrap())
            .unwrap();
        let CauseInspection::Derived { supporters, .. } = latest else {
            panic!("latest Toggle inversion must resolve to a derived cause");
        };
        assert!(supporters.iter().any(|supporter| matches!(
            odd_result.provenance().inspect(*supporter),
            Ok(CauseInspection::Derived {
                subject: ProvenanceSubject::Node(node),
                ..
            }) if node == NodeKey::from_u128(10)
        )));
    }

    #[test]
    fn toggle_settles_downstream_in_same_reaction_and_machines_are_independent() {
        let compiled = compiled_toggle(LogicLevel::Low, true);
        let mut first = compiled.spawn(policy_with([10, 100, 0, 100, 1_000]));
        let mut second = compiled.spawn(policy_with([10, 100, 0, 100, 1_000]));
        assert_eq!(
            first.inspect_toggle_definition(NodeKey::from_u128(20)),
            Err(crate::ToggleInspectionFailure::NotToggle(
                NodeKey::from_u128(20)
            ))
        );
        for (machine, count) in [(&mut first, 1_u64), (&mut second, 2_u64)] {
            let snapshot = compiled
                .input_snapshot()
                .pulse(ExternalInputKey::from_u128(1), PulseCount::new(count))
                .and_then(crate::InputSnapshotBuilder::finish)
                .unwrap();
            machine
                .apply(Transaction::initialize(
                    crate::time::Time::from_ticks(10),
                    machine.revision(),
                    snapshot,
                ))
                .unwrap();
        }
        assert_eq!(
            first
                .inspect_toggle(NodeKey::from_u128(10))
                .unwrap()
                .committed(),
            LogicLevel::High
        );
        assert_eq!(
            second
                .inspect_toggle(NodeKey::from_u128(10))
                .unwrap()
                .committed(),
            LogicLevel::Low
        );
        assert_eq!(
            first.output_level(ExternalOutputKey::from_u128(30)),
            Some(LogicLevel::Low)
        );
        assert_eq!(
            second.output_level(ExternalOutputKey::from_u128(30)),
            Some(LogicLevel::High)
        );
    }

    #[test]
    fn toggle_state_and_cause_are_failure_atomic() {
        let compiled = compiled_toggle(LogicLevel::Low, false);
        let mut rejected_initialization = compiled.spawn(policy_with([10, 100, 0, 0, 1_000]));
        let before_initialization = observe(&rejected_initialization);
        let proposed_odd = compiled
            .input_snapshot()
            .pulse(ExternalInputKey::from_u128(1), PulseCount::ONE)
            .and_then(crate::InputSnapshotBuilder::finish)
            .unwrap();
        let failure = rejected_initialization
            .apply(Transaction::initialize(
                crate::time::Time::from_ticks(1),
                rejected_initialization.revision(),
                proposed_odd,
            ))
            .unwrap_err();
        assert!(matches!(
            failure.evidence(),
            RuntimeFailureEvidence::BudgetExceeded {
                budget: RuntimePolicyLimit::MaxEventsCreatedPerTransaction,
                ..
            }
        ));
        assert_eq!(observe(&rejected_initialization), before_initialization);

        let mut machine = compiled.spawn(policy_with([10, 100, 0, 100, 1_000]));
        let snapshot = compiled
            .input_snapshot()
            .pulse(ExternalInputKey::from_u128(1), PulseCount::ONE)
            .and_then(crate::InputSnapshotBuilder::finish)
            .unwrap();
        machine
            .apply(Transaction::initialize(
                crate::time::Time::from_ticks(10),
                machine.revision(),
                snapshot,
            ))
            .unwrap();
        let before = observe(&machine);
        let odd = compiled
            .input_delta()
            .pulse(ExternalInputKey::from_u128(1), PulseCount::ONE)
            .and_then(crate::InputDeltaBuilder::finish)
            .unwrap();
        let failure = machine
            .apply(Transaction::advance(
                crate::time::Time::from_ticks(10),
                machine.revision(),
                odd,
            ))
            .unwrap_err();
        assert!(matches!(
            failure.evidence(),
            RuntimeFailureEvidence::TimeNotStrictlyIncreasing { .. }
        ));
        assert_eq!(observe(&machine), before);
    }

    #[test]
    fn multiple_toggle_cells_commit_atomically_and_ignore_input_claim_order() {
        let compiled = compiled_two_toggles();
        let initialize = |reverse: bool| {
            let mut machine = compiled.spawn(policy_with([10, 100, 0, 100, 1_000]));
            let mut snapshot = compiled.input_snapshot();
            let claims = if reverse {
                [(2_u128, 3_u64), (1, 1)]
            } else {
                [(1_u128, 1_u64), (2, 3)]
            };
            for (key, count) in claims {
                snapshot = snapshot
                    .pulse(ExternalInputKey::from_u128(key), PulseCount::new(count))
                    .unwrap();
            }
            machine
                .apply(Transaction::initialize(
                    crate::time::Time::from_ticks(10),
                    machine.revision(),
                    snapshot.finish().unwrap(),
                ))
                .unwrap();
            machine
        };

        let forward = initialize(false);
        let reverse = initialize(true);
        for machine in [&forward, &reverse] {
            assert_eq!(
                machine
                    .inspect_toggle(NodeKey::from_u128(10))
                    .unwrap()
                    .committed(),
                LogicLevel::High
            );
            assert_eq!(
                machine
                    .inspect_toggle(NodeKey::from_u128(20))
                    .unwrap()
                    .committed(),
                LogicLevel::Low
            );
            assert_eq!(
                machine.store.toggle_states,
                [LogicLevel::High, LogicLevel::Low]
            );
            assert_eq!(machine.store.toggle_inversion_causes.len(), 2);
        }
        assert_eq!(forward.store.toggle_states, reverse.store.toggle_states);
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
                CauseInspection::Derived { supporters, .. }
                | CauseInspection::PendingPulseDelay { supporters, .. } => {
                    assert!(
                        !supporters.is_empty(),
                        "derived provenance must terminate in authoritative roots"
                    );
                    for supporter in supporters {
                        visit(view, *supporter, visiting, visited);
                    }
                }
                CauseInspection::PulseDerived { supporters, .. } => {
                    assert!(!supporters.is_empty());
                    for supporter in supporters {
                        visit(view, *supporter, visiting, visited);
                    }
                }
                CauseInspection::InitializationTransaction { .. }
                | CauseInspection::ReadyTransaction { .. }
                | CauseInspection::ExternalObservation { .. }
                | CauseInspection::ExternalPulseObservation { .. } => {}
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
    fn all_publishes_only_settled_observable_levels() {
        let compiled = compiled_all();
        let first = ExternalInputKey::<Level>::from_u128(1);
        let second = ExternalInputKey::<Level>::from_u128(2);
        let output = ExternalOutputKey::<Level>::from_u128(30);
        let snapshot = compiled
            .input_snapshot()
            .set(first, LogicLevel::Low)
            .and_then(|builder| builder.set(second, LogicLevel::High))
            .and_then(crate::InputSnapshotBuilder::finish)
            .unwrap_or_else(|_| panic!("All snapshot must build"));
        let mut machine = compiled.spawn(policy_with([10, 100, 0, 100, 1_000]));
        let initialized = machine
            .apply(Transaction::initialize(
                crate::time::Time::from_ticks(10),
                machine.revision(),
                snapshot,
            ))
            .unwrap_or_else(|failure| panic!("All initialization must succeed: {failure}"));
        assert!(matches!(
            initialized.output_events(),
            [OutputEvent::LevelEstablished {
                output: actual,
                value: LogicLevel::Low,
                ..
            }] if *actual == output
        ));

        let changed = machine
            .apply(Transaction::advance(
                crate::time::Time::from_ticks(20),
                machine.revision(),
                compiled
                    .input_delta()
                    .set(first, LogicLevel::High)
                    .and_then(|builder| builder.set(second, LogicLevel::Low))
                    .and_then(crate::InputDeltaBuilder::finish)
                    .unwrap_or_else(|_| panic!("All delta must build")),
            ))
            .unwrap_or_else(|failure| panic!("All advance must succeed: {failure}"));
        assert!(changed.output_events().is_empty());
        assert_eq!(machine.output_level(output), Some(LogicLevel::Low));

        let changed = machine
            .apply(Transaction::advance(
                crate::time::Time::from_ticks(30),
                machine.revision(),
                compiled
                    .input_delta()
                    .set(second, LogicLevel::High)
                    .and_then(crate::InputDeltaBuilder::finish)
                    .unwrap_or_else(|_| panic!("All delta must build")),
            ))
            .unwrap_or_else(|failure| panic!("All advance must succeed: {failure}"));
        assert!(matches!(
            changed.output_events(),
            [OutputEvent::LevelChanged {
                output: actual,
                from: LogicLevel::Low,
                to: LogicLevel::High,
                ..
            }] if *actual == output
        ));
        assert_eq!(machine.output_level(output), Some(LogicLevel::High));
    }

    #[test]
    fn remaining_variadics_settle_simultaneous_changes_before_publishing() {
        let first = ExternalInputKey::<Level>::from_u128(1);
        let second = ExternalInputKey::<Level>::from_u128(2);
        let output = ExternalOutputKey::<Level>::from_u128(30);

        for kind in [
            NodeKind::<()>::any(),
            NodeKind::parity(),
            NodeKind::at_least(1),
        ] {
            let compiled = compiled_variadic(kind);
            let snapshot = compiled
                .input_snapshot()
                .set(first, LogicLevel::High)
                .and_then(|builder| builder.set(second, LogicLevel::Low))
                .and_then(crate::InputSnapshotBuilder::finish)
                .unwrap_or_else(|_| panic!("variadic snapshot must build"));
            let mut machine = compiled.spawn(policy_with([10, 100, 0, 100, 1_000]));
            machine
                .apply(Transaction::initialize(
                    crate::time::Time::from_ticks(10),
                    machine.revision(),
                    snapshot,
                ))
                .unwrap_or_else(|failure| {
                    panic!("variadic initialization must succeed: {failure}")
                });
            assert_eq!(machine.output_level(output), Some(LogicLevel::High));

            let unchanged = machine
                .apply(Transaction::advance(
                    crate::time::Time::from_ticks(20),
                    machine.revision(),
                    compiled
                        .input_delta()
                        .set(first, LogicLevel::Low)
                        .and_then(|builder| builder.set(second, LogicLevel::High))
                        .and_then(crate::InputDeltaBuilder::finish)
                        .unwrap_or_else(|_| panic!("simultaneous variadic delta must build")),
                ))
                .unwrap_or_else(|failure| panic!("variadic advance must succeed: {failure}"));
            assert!(unchanged.output_events().is_empty());
            assert_eq!(machine.output_level(output), Some(LogicLevel::High));

            let changed = machine
                .apply(Transaction::advance(
                    crate::time::Time::from_ticks(30),
                    machine.revision(),
                    compiled
                        .input_delta()
                        .set(second, LogicLevel::Low)
                        .and_then(crate::InputDeltaBuilder::finish)
                        .unwrap_or_else(|_| panic!("variadic delta must build")),
                ))
                .unwrap_or_else(|failure| panic!("variadic advance must succeed: {failure}"));
            assert!(matches!(
                changed.output_events(),
                [OutputEvent::LevelChanged {
                    output: actual,
                    from: LogicLevel::High,
                    to: LogicLevel::Low,
                    ..
                }] if *actual == output
            ));
        }
    }

    #[test]
    fn select_publishes_only_the_settled_selected_branch() {
        let compiled = compiled_select();
        let selector = ExternalInputKey::<Level>::from_u128(1);
        let when_low = ExternalInputKey::<Level>::from_u128(2);
        let when_high = ExternalInputKey::<Level>::from_u128(3);
        let output = ExternalOutputKey::<Level>::from_u128(30);
        let snapshot = compiled
            .input_snapshot()
            .set(selector, LogicLevel::Low)
            .and_then(|builder| builder.set(when_low, LogicLevel::Low))
            .and_then(|builder| builder.set(when_high, LogicLevel::High))
            .and_then(crate::InputSnapshotBuilder::finish)
            .unwrap_or_else(|_| panic!("Select snapshot must build"));
        let mut machine = compiled.spawn(policy_with([10, 100, 0, 100, 1_000]));
        machine
            .apply(Transaction::initialize(
                crate::time::Time::from_ticks(10),
                machine.revision(),
                snapshot,
            ))
            .unwrap_or_else(|failure| panic!("Select initialization must succeed: {failure}"));
        assert_eq!(machine.output_level(output), Some(LogicLevel::Low));

        let unchanged = machine
            .apply(Transaction::advance(
                crate::time::Time::from_ticks(20),
                machine.revision(),
                compiled
                    .input_delta()
                    .set(selector, LogicLevel::High)
                    .and_then(|builder| builder.set(when_low, LogicLevel::High))
                    .and_then(|builder| builder.set(when_high, LogicLevel::Low))
                    .and_then(crate::InputDeltaBuilder::finish)
                    .unwrap_or_else(|_| panic!("simultaneous Select delta must build")),
            ))
            .unwrap_or_else(|failure| panic!("Select advance must succeed: {failure}"));
        assert!(unchanged.output_events().is_empty());
        assert_eq!(machine.output_level(output), Some(LogicLevel::Low));

        let changed = machine
            .apply(Transaction::advance(
                crate::time::Time::from_ticks(30),
                machine.revision(),
                compiled
                    .input_delta()
                    .set(when_high, LogicLevel::High)
                    .and_then(crate::InputDeltaBuilder::finish)
                    .unwrap_or_else(|_| panic!("selected-branch delta must build")),
            ))
            .unwrap_or_else(|failure| panic!("Select branch advance must succeed: {failure}"));
        assert!(matches!(
            changed.output_events(),
            [OutputEvent::LevelChanged {
                output: actual,
                from: LogicLevel::Low,
                to: LogicLevel::High,
                ..
            }] if *actual == output
        ));
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
                    OutputEvent::Pulsed { .. } => {
                        panic!("level-only fixture must not emit pulse events")
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
                    OutputEvent::LevelEstablished { .. } | OutputEvent::Pulsed { .. } => None,
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
    fn merge_accepts_the_maximum_count_and_overflow_preserves_complete_machine() {
        let compiled = compiled_merge();
        let first = ExternalInputKey::<Pulse>::from_u128(1);
        let second = ExternalInputKey::<Pulse>::from_u128(2);
        let output = ExternalOutputKey::<Pulse>::from_u128(30);
        let mut machine = compiled.spawn(policy_with([10, 100, 0, 100, 1_000]));
        let initialized = machine
            .apply(Transaction::initialize(
                crate::time::Time::from_ticks(10),
                machine.revision(),
                compiled
                    .input_snapshot()
                    .pulse(first, PulseCount::new(u64::MAX))
                    .and_then(|builder| builder.pulse(second, PulseCount::ZERO))
                    .and_then(crate::InputSnapshotBuilder::finish)
                    .unwrap_or_else(|_| panic!("boundary snapshot must build")),
            ))
            .unwrap_or_else(|failure| {
                panic!("maximum representable count must succeed: {failure}")
            });
        assert!(matches!(
            initialized.output_events(),
            [OutputEvent::Pulsed {
                output: actual_output,
                count,
                ..
            }] if *actual_output == output && count.get() == u64::MAX
        ));

        let before = observe(&machine);
        let failure = machine
            .apply(Transaction::advance(
                crate::time::Time::from_ticks(11),
                machine.revision(),
                compiled
                    .input_delta()
                    .pulse(first, PulseCount::new(u64::MAX))
                    .and_then(|builder| builder.pulse(second, PulseCount::ONE))
                    .and_then(crate::InputDeltaBuilder::finish)
                    .unwrap_or_else(|_| panic!("overflowing delta must build")),
            ))
            .unwrap_err();
        assert_eq!(
            failure.evidence(),
            &RuntimeFailureEvidence::PulseCountOverflow {
                node: NodeKey::from_u128(10),
            }
        );
        assert_eq!(observe(&machine), before);
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
                    OutputEvent::LevelEstablished { .. } | OutputEvent::Pulsed { .. } => None,
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
